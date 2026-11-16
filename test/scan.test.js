// Explicit Scan submission and collection against the existing controlled service.
import { test, before, after, beforeEach } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { gunzipSync } from "node:zlib";
import { createFakeProver, TEST_KEY } from "./helpers/fake-prover.js";
import { tmpdir, makeRepo, baseEnv, saveKey, BIN, symlink, SKIP_FILE_SYMLINKS } from "./helpers/env.js";
import { validateScan } from "./helpers/schema.js";

let fp;
before(async () => { fp = createFakeProver(); await fp.listen(); });
after(() => fp.close());
beforeEach(() => { fp.state.scan = {}; fp.state.scans = new Map(); fp.state.requests.length = 0; fp.state.corruptArtifact = false; });

function setup({ key = true } = {}) {
  const data = tmpdir("intent-data-");
  if (key) saveKey(data, TEST_KEY, fp.url);
  const { dir: repo } = makeRepo();
  return { data, repo, env: baseEnv(data, fp.url) };
}
const write = (repo, rel, text) => { fs.mkdirSync(path.dirname(path.join(repo, rel)), { recursive: true }); fs.writeFileSync(path.join(repo, rel), text); };
const read = (file) => JSON.parse(fs.readFileSync(file, "utf8"));
const idOf = (text) => /\/tasks\/([0-9a-f-]{36})/.exec(text)[1];
const reportDir = (ctx, id) => path.join(ctx.repo, ".intent", "logs", read(path.join(ctx.data, "scans", `${id}.json`)).operation_id, "artifacts");
function cli(args, ctx, extraEnv = {}) {
  return new Promise((resolve) => {
    const child = spawn(BIN, ["scan", ...args], { cwd: ctx.cwd || ctx.repo, env: { ...ctx.env, ...extraEnv } });
    let out = "";
    let err = "";
    child.stdout.on("data", (bytes) => { out += bytes; });
    child.stderr.on("data", (bytes) => { err += bytes; });
    child.on("close", (code) => resolve({ code, out: out.trim(), err: err.trim() }));
  });
}

test("snapshot filters/redacts tracked and non-ignored untracked files, preserving the contract", async () => {
  const ctx = setup();
  write(ctx.repo, "src/app.py", "def f():\n    return 1\n");
  write(ctx.repo, "src/secret.py", 'TOKEN = "sk-proj-abcdefghijklmnopqrstuvwxyz0123456789ABCD"\nX = 1\n');
  write(ctx.repo, ".env", "KEY=private\n");
  write(ctx.repo, "node_modules/a.js", "vendored\n");
  write(ctx.repo, "package-lock.json", "{}\n");
  write(ctx.repo, "binary.dat", Buffer.from([0, 1, 2]));
  write(ctx.repo, ".intent/REQUIREMENTS.md", "- Intended behaviour.\n");
  write(ctx.repo, ".intent/source/src/app.py/requirements.json", JSON.stringify({ schema_version: 1, requirements: [{ id: "abc123", statement: "Return one", targets: [{ path: "src/app.py" }] }] }));
  write(ctx.repo, ".intent/source/src/app.py/index.json", JSON.stringify({ schema_version: 1, properties: { abc123: { specs: ["source/src/app.py/specs/def456/spec.k"], last_run: null } } }));
  write(ctx.repo, ".intent/source/src/app.py/specs/def456/spec.k", "module SPEC endmodule\n");
  write(ctx.repo, ".intent/private.md", "private evidence\n");
  write(ctx.repo, ".intent/prove/old/PROOF.md", "private evidence\n");
  const result = await cli(["fast", "--host", "antigravity"], ctx);
  assert.equal(result.code, 0, result.err);
  const [upload] = fp.scanUploads();
  assert.deepEqual(validateScan(upload, "ScanUpload"), []);
  assert.deepEqual(upload.files.map((file) => file.path), [".intent/REQUIREMENTS.md", ".intent/source/src/app.py/requirements.json", ".intent/source/src/app.py/specs/def456/spec.k", "README.md", "src/app.py", "src/secret.py"]);
  assert.ok(!JSON.stringify(upload.files).includes("abcdefghijklmnopqrstuvwxyz"));
  assert.equal(upload.depth, "fast");
  assert.equal(upload.meta.host, "antigravity");
});

test("snapshot scope is literal, rejects outside paths and does not follow symlinks", { skip: SKIP_FILE_SYMLINKS }, async () => {
  const ctx = setup();
  write(ctx.repo, "src/app.py", "x = 1\n");
  const outside = tmpdir();
  write(outside, "private.py", "private = 1\n");
  symlink(outside, path.join(ctx.repo, "link"));
  assert.match((await cli(["link"], ctx)).out, /outside the repo/);
  assert.equal(fp.scanUploads().length, 0);
  const result = await cli(["src"], ctx);
  assert.equal(result.code, 0, result.err);
  assert.deepEqual(fp.scanUploads()[0].files.map((file) => file.path), ["src/app.py"]);
});

test("Scan from a subdirectory submits once and saves the report at the repository root", async () => {
  const ctx = setup();
  write(ctx.repo, "src/app.py", "x = 1\n");
  ctx.cwd = path.join(ctx.repo, "src");
  fp.state.scan.statuses = ["done"];
  const result = await cli([], ctx);
  assert.equal(result.code, 0, result.err);
  const id = idOf(result.out);
  const record = path.join(ctx.data, "scans", `${id}.json`);
  assert.equal(read(record).status, "done");
  assert.match(result.err, /If interrupted, resume with scan collect/);
  assert.match(result.out, /Scan finished/);
  assert.equal(read(record).report_status, "saved");
  assert.match(result.out, /Local HTML report:.*intent-prover view --output <new-file.html>/);
  const operation = read(record).operation_id;
  assert.match(operation, /^[a-z0-9]{6}$/);
  assert.ok(result.out.includes(`#view=scans&scan=${encodeURIComponent(`logs/${operation}/artifacts/meta.json`)}`));
  const collected = await cli(["collect", id], ctx);
  assert.equal(collected.code, 0, collected.err);
  assert.match(collected.out, /Scan finished/);
  assert.match(collected.out, /Local HTML report:/);
  assert.equal(read(record).report_status, "saved");
  const artifacts = reportDir(ctx, id);
  assert.ok(read(path.join(artifacts, "report.json")).coverage);
  assert.equal(read(path.join(artifacts, "meta.json")).report.status, "saved");
  const request = read(path.join(artifacts, "..", "request.json"));
  assert.equal(request.operation, "scan");
  assert.equal(request.upload_sha256, read(path.join(artifacts, "meta.json")).source.snapshot_sha256);
  assert.equal(request.inputs.length, 1);
  const snapshot = fs.readFileSync(path.join(ctx.repo, ".intent", request.inputs[0].path));
  assert.equal(createHash("sha256").update(snapshot).digest("hex"), request.upload_sha256);
  assert.equal(request.inputs[0].sha256, request.upload_sha256);
  assert.deepEqual(JSON.parse(gunzipSync(snapshot)), fp.scanUploads()[0]);
  assert.equal(fs.existsSync(path.join(ctx.cwd, ".intent")), false);
  assert.equal(fp.scanUploads().length, 1);
  assert.match((await cli(["collect"], ctx)).out, /No uncollected scans/);

  // Recovery of a pre-log record leaves its existing evidence untouched.
  const old = path.join(ctx.repo, ".intent", "scan", id);
  fs.mkdirSync(old, { recursive: true });
  fs.copyFileSync(path.join(artifacts, "report.json"), path.join(old, "report.json"));
  const oldBytes = fs.readFileSync(path.join(old, "report.json"));
  const legacy = read(record);
  delete legacy.operation_id;
  fs.writeFileSync(record, JSON.stringify(legacy));
  assert.equal((await cli(["collect", id], ctx)).code, 0);
  assert.notEqual(read(record).operation_id, operation);
  assert.equal(read(path.join(reportDir(ctx, id), "..", "request.json")).recovered_task, id);
  assert.deepEqual(fs.readFileSync(path.join(old, "report.json")), oldBytes);
  assert.ok(fs.existsSync(path.join(artifacts, "report.json")));
  assert.equal(fp.scanUploads().length, 1);
});

test("corrupt reports are rejected without replacing a saved report", async () => {
  const ctx = setup();
  fp.state.corruptArtifact = true;
  const result = await cli([], ctx);
  const id = idOf(result.out);
  const artifacts = reportDir(ctx, id);
  const reportFile = path.join(artifacts, "report.json");
  assert.equal(read(path.join(artifacts, "meta.json")).report.status, "pending");
  assert.equal(read(path.join(artifacts, "meta.json")).report.reason, "REPORT_HASH_MISMATCH");
  assert.equal(fs.existsSync(reportFile), false);

  fp.state.corruptArtifact = false;
  const recovered = await cli(["collect", id], ctx);
  assert.equal(recovered.code, 0, recovered.err);
  assert.equal(read(path.join(artifacts, "meta.json")).report.status, "saved");
  const saved = fs.readFileSync(reportFile);

  // A changed artifact reference forces a download instead of reusing the saved copy.
  fp.state.scan.summary = {
    counts: {}, cards: [], oneLiners: [],
    report: { ...read(path.join(artifacts, "meta.json")).task.report, sha256: "0".repeat(64) },
  };
  fp.state.corruptArtifact = true;
  await cli(["collect", id], ctx);
  assert.equal(read(path.join(artifacts, "meta.json")).report.status, "pending");
  assert.equal(read(path.join(artifacts, "meta.json")).report.reason, "REPORT_HASH_MISMATCH");
  assert.deepEqual(fs.readFileSync(reportFile), saved);
  assert.equal(fp.scanUploads().length, 1);
});

test("an interrupted Scan keeps its task record and collect resumes without another upload", async () => {
  const ctx = setup();
  const pluginRoot = path.join(ctx.data, ".gemini", "config", "plugins", "intent");
  fs.mkdirSync(pluginRoot, { recursive: true });
  delete ctx.env.CLAUDE_PLUGIN_DATA;
  delete ctx.env.PLUGIN_DATA;
  const args = ["--host", "antigravity", "--plugin-root", pluginRoot];
  const records = path.join(ctx.data, ".gemini", "config", "plugin-data", "intent", "scans");
  fp.state.scan.statuses = ["running"];
  const child = spawn(BIN, ["scan", ...args], { cwd: ctx.repo, env: ctx.env });
  let err = "";
  let id;
  child.stderr.on("data", (bytes) => {
    err += bytes;
    if (!id && /\/tasks\/[0-9a-f-]{36}/.test(err)) {
      id = idOf(err);
      child.kill("SIGINT");
    }
  });
  await new Promise((resolve) => child.on("close", resolve));
  assert.ok(id, err);
  assert.ok(fs.existsSync(path.join(records, `${id}.json`)));
  assert.equal(read(path.join(records, `${id}.json`)).host, "antigravity");
  fp.state.scan.statuses = ["done"];
  const recovered = await cli(["collect", id, ...args], ctx);
  assert.equal(recovered.code, 0, recovered.err);
  assert.match(recovered.out, /Saved report/);
  assert.equal(fp.scanUploads().length, 1);
});

test("missing auth, environment opt-outs and invalid targets never submit", async () => {
  const ctx = setup({ key: false });
  assert.equal((await cli([], ctx)).code, 1);
  assert.match((await cli([], ctx)).err, /intent-prover login/);
  const connected = setup();
  assert.equal((await cli([], connected, { INTENT_DISABLED: "1" })).code, 1);
  assert.match((await cli(["missing.py"], connected)).out, /not found/);
  assert.deepEqual(fp.scanUploads(), []);
  assert.equal(fp.state.requests.filter((request) => request.method === "POST").length, 0);
});

test("busy uploads retry once with the same idempotency key", async () => {
  const ctx = setup();
  fp.state.scan.submit = [{ status: 429, code: "RATE_LIMITED", reason: "scan_ingest_busy", retryAfter: 1 }];
  assert.equal((await cli([], ctx)).code, 0);
  const uploads = fp.state.requests.filter((request) => request.method === "POST" && request.path === "/scan");
  assert.equal(uploads.length, 2);
  assert.equal(uploads[0].headers["idempotency-key"], uploads[1].headers["idempotency-key"]);
});

test("failed scans and malformed summaries do not claim clean results", async () => {
  const ctx = setup();
  fp.state.scan.statuses = ["failed"];
  const result = await cli([], ctx);
  assert.equal(result.code, 1);
  assert.match(result.err, /could not finish/);
  const id = idOf(result.err);
  const collected = await cli(["collect", id], ctx);
  assert.equal(collected.code, 1);
  assert.match(collected.err, /could not finish/);
  assert.doesNotMatch(collected.out + collected.err, /no findings/);
  fp.state.scan = { summary: {} };
  const malformed = await cli([], ctx);
  assert.doesNotMatch(malformed.out, /no findings/);
  assert.match(malformed.out, /open the report/);
});
