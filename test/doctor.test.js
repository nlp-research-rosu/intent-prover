import { test, before, after } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { execFile } from "node:child_process";
import { createFakeProver, TEST_KEY } from "./helpers/fake-prover.js";
import { BIN, baseEnv, credentialsPath, saveKey, tmpdir } from "./helpers/env.js";

const fp = createFakeProver();
before(() => fp.listen());
after(() => fp.close());

function doctor(cwd, env) {
  return new Promise((resolve, reject) => {
    execFile(BIN, ["doctor"], { cwd, env, timeout: 8000 }, (error, stdout, stderr) => {
      if (error && !Number.isInteger(error.code)) return reject(error);
      resolve({ code: error?.code || 0, stdout, stderr });
    });
  });
}

test("doctor reports the account and service separately without writing or exposing credentials", async (t) => {
  const cwd = tmpdir("intent-doctor-");
  t.after(() => fs.rmSync(cwd, { recursive: true, force: true }));
  const recovery = path.join(cwd, "missing", "scan-data");
  const env = baseEnv(cwd, fp.url, { CLAUDE_PLUGIN_DATA: recovery });
  delete env.CLAUDE_PLUGIN_ROOT;
  const keyFile = credentialsPath(cwd);
  const override = "fixture-rejected-key-not-for-output";
  const requests = () => fp.state.requests.splice(0).map(({ method, path, headers }) => {
    assert.equal(method, "GET");
    return [path, headers.authorization];
  });
  const report = async (code, extraEnv = {}) => {
    const result = await doctor(cwd, { ...env, ...extraEnv });
    assert.equal(result.code, code, result.stderr || result.stdout);
    assert.equal(result.stderr, "");
    assert.ok(!result.stdout.includes(TEST_KEY));
    assert.ok(!result.stdout.includes(override));
    return result.stdout;
  };

  let out = await report(1);
  assert.match(out, /Connection: service health check passed/);
  assert.match(out, /Account: not connected; run intent-prover login/);
  assert.deepEqual(requests(), [["/healthz", undefined]]);
  assert.deepEqual(fs.readdirSync(cwd), []);

  saveKey(cwd, TEST_KEY, fp.url);
  const saved = fs.readFileSync(keyFile);
  out = await report(0, { DO_NOT_TRACK: "1" });
  assert.match(out, /Account: key accepted/);
  assert.match(out, /Warning: DO_NOT_TRACK=1 blocks plugin Scan/);
  assert.ok(out.includes(`Local Scan state (not verified): ${recovery}`));
  assert.deepEqual(requests(), [["/tasks", `Bearer ${TEST_KEY}`]]);

  out = await report(1, { INTENT_API_KEY: override });
  assert.match(out, /Account: key rejected; correct or unset INTENT_API_KEY/);
  assert.match(out, /Key source: INTENT_API_KEY/);
  assert.doesNotMatch(out, /run intent-prover login/);
  assert.deepEqual(requests(), [["/tasks", `Bearer ${override}`]]);

  fp.state.accountNotApproved = true;
  out = await report(1);
  assert.ok(out.includes(`Apply at ${fp.url}/apply`));
  assert.deepEqual(requests(), [["/tasks", `Bearer ${TEST_KEY}`]]);

  fp.state.accountNotApproved = false;
  fp.state.tasksStatus = 503;
  out = await report(1);
  assert.match(out, /Connection: service health check passed/);
  assert.match(out, /Account: could not check account access \(HTTP 503\)/);
  assert.deepEqual(requests(), [["/tasks", `Bearer ${TEST_KEY}`], ["/healthz", undefined]]);

  fp.state.healthStatus = 503;
  out = await report(1);
  assert.match(out, /Connection: service health check failed \(HTTP 503\)/);
  assert.match(out, /Account: could not check account access \(HTTP 503\)/);
  assert.deepEqual(requests(), [["/tasks", `Bearer ${TEST_KEY}`], ["/healthz", undefined]]);
  assert.deepEqual(fs.readFileSync(keyFile), saved);
  assert.equal(fs.existsSync(recovery), false);

  const invalid = await doctor(cwd, { ...env, INTENT_API_URL: `${fp.url}/?token=${override}` });
  assert.equal(invalid.code, 1);
  assert.match(invalid.stderr, /Service URL must use HTTPS .*without credentials, query or fragment/);
  assert.ok(!(invalid.stdout + invalid.stderr).includes(override));
  assert.deepEqual(requests(), []);
});
