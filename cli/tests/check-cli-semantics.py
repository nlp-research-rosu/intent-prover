#!/usr/bin/env python3
"""Core workflow, saved report and input-safety checks against a loopback server."""
import json
import hashlib
import os
import subprocess
import sys
import tempfile
import threading
import unittest
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from semantics_fixture import IDENTITY, seed

BINARY = Path(sys.argv.pop(1)).resolve()


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def respond(self, data, status=200):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        self.server.requests.append(("GET", self.path))
        if self.path == "/semantics":
            self.respond({"semantics": [IDENTITY]})
        elif self.path.endswith(("stdout", "stderr")):
            self.respond("log")
        elif self.path.endswith("metrics"):
            self.respond({})
        elif self.path.startswith("/tasks/"):
            task_id = self.path.rsplit("/", 1)[-1]
            self.respond({"id": task_id, "status": self.server.task_status,
                          "semantics": {key: IDENTITY[key] for key in ("id", "commit")},
                          "toolRuns": [{"tool": "kprove", "exitCode": self.server.exit_code}],
                          "definitionId": "test-definition",
                          "result": self.server.outcome,
                          "newBackendField": {"retained": True}})
        else:
            self.respond({}, 404)

    def do_POST(self):
        raw = self.rfile.read(int(self.headers["Content-Length"]))
        self.server.requests.append(("POST", self.path))
        self.server.uploads.append((self.path, raw.decode("latin-1")))
        key = self.headers.get("Idempotency-Key")
        self.server.keys.append(key)
        task_id = self.server.tasks_by_key.setdefault(key, str(uuid.uuid4()))
        if self.server.submit_failures:
            self.respond({"code": "INTERNAL_ERROR", "message": "controlled outage"},
                         self.server.submit_failures.pop(0))
            return
        if self.server.after_submit:
            self.server.after_submit()
        self.respond({"id": task_id, "status": "queued",
                      "semantics": {key: IDENTITY[key] for key in ("id", "commit")}}, 202)


class Tests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="intent-workflow-")
        self.base = Path(self.tmp.name).resolve()
        self.root = self.base / "project"
        self.root.mkdir()
        subprocess.run(["git", "init", "--quiet", str(self.root)], check=True)
        self.cwd = self.root / "src"
        self.cwd.mkdir()
        self.data = self.base / "config/intent-prover"
        seed(self.data)
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.requests = []
        self.server.uploads = []
        self.server.keys = []
        self.server.tasks_by_key = {}
        self.server.submit_failures = []
        self.server.after_submit = None
        self.server.task_status = "completed"
        self.server.exit_code = 0
        self.server.outcome = "proved"
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.config = self.data / "config.toml"
        self.config.write_text(
            f'server_url = "http://127.0.0.1:{self.server.server_port}"\n'
            'poll_interval_seconds = 1\n')
        self.env = dict(os.environ, XDG_CONFIG_HOME=str(self.base / "config"))
        (self.root / "program.mpy").write_text("pass\n")
        self.session = self.cli("session", "start", "--semantics", "test")
        self.workspace = Path(self.session["workspaceDir"])
        (self.workspace / "inputs").mkdir()
        (self.workspace / "inputs/spec.k").write_text("module SPEC endmodule\n")

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
        self.tmp.cleanup()

    def cli(self, *args, code=0):
        result = subprocess.run([BINARY, *args], cwd=self.cwd, env=self.env,
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, code, result.stdout + result.stderr)
        return json.loads(result.stdout)

    def operation(self, command, code=0, extra=()):
        args = (["--program", "program.mpy"] if command == "run" else
                ["--spec", "inputs/spec.k", "--spec-module", "SPEC"])
        return self.cli(command, "--session", self.session["sessionId"],
                        *args, *extra, code=code)

    def requirement(self, targets=None):
        file = self.base / "requirement.json"
        file.write_text(json.dumps({"statement": "The selected program preserves the stated invariant.",
                                    "targets": targets or [{"path": "program.mpy"}]}))
        return self.cli("requirements", "add", "--input", str(file))["requirement"]["id"]

    def test_run_validate_prove_retain_results_and_pinned_semantics(self):
        self.assertEqual(self.workspace, self.root / ".intent/prove/sessions" / self.session["sessionId"])
        requirement = self.requirement()
        self.assertRegex(requirement, r"^[a-z0-9]{6}$")
        owner = self.root / ".intent/source/program.mpy"
        for command, kind, outcome in (("run", "run", "success"),
                                       ("validate", "validation", "valid"),
                                       ("prove", "proof", "proved")):
            self.server.outcome = outcome
            result = self.operation(command, extra=() if command == "run" else ("--requirement", requirement))
            evidence = Path(result["evidenceDir"])
            self.assertEqual(evidence.parent, self.root / ".intent/logs")
            self.assertRegex(evidence.name, r"^[a-z0-9]{6}$")
            request = json.loads((evidence / "request.json").read_text())
            self.assertEqual(request["operation"], command)
            self.assertEqual(request["session_id"], self.session["sessionId"])
            for entry in request["inputs"]:
                frozen = self.root / ".intent" / entry["path"]
                self.assertEqual(hashlib.sha256(frozen.read_bytes()).hexdigest(), entry["sha256"])
            saved = json.loads((evidence / "result.json").read_text())
            self.assertEqual(saved["task"], result["task"])
            self.assertEqual(saved["task"]["result"], outcome)
            self.assertEqual(saved["task"]["status"], "completed")
            self.assertTrue(saved["stdout"])
            self.assertTrue(saved["stderr"])
            self.assertEqual(saved["reportUrl"],
                              f'https://prover-console.intentcomputing.org/tasks/{result["task"]["id"]}')
            if command != "run":
                selection = json.loads((owner / "index.json").read_text())["properties"][requirement]
                self.assertEqual(selection["last_run"], evidence.name)
                self.assertTrue(selection["specs"])
                self.assertEqual((self.root / ".intent" / selection["specs"][-1]).read_text(),
                                 "module SPEC endmodule\n")
        self.assertEqual([path for path, _ in self.server.uploads],
                         ["/runs", "/proof-dry-runs", "/proofs"])
        self.assertFalse((self.cwd / ".intent").exists())
        for _, body in self.server.uploads:
            self.assertIn('name="semanticsCommit"\r\n\r\n' + IDENTITY["commit"], body)
            self.assertIn('name="semanticsId"\r\n\r\ntest', body)

    def test_report_link_retains_submission_console_when_waiting_and_exporting(self):
        self.env["INTENT_CONSOLE_URL"] = "https://staging-console.example.test/prover/"
        result = self.operation("prove")
        evidence = Path(result["evidenceDir"]) / "result.json"
        expected = f'https://staging-console.example.test/prover/tasks/{result["task"]["id"]}'
        self.assertEqual(json.loads(evidence.read_text())["reportUrl"], expected)
        self.env["INTENT_CONSOLE_URL"] = "https://production-console.example.test"
        recovered = self.cli("task", "wait", result["task"]["id"],
                             "--session", self.session["sessionId"])
        self.assertEqual(recovered["task"], result["task"])
        self.assertEqual(json.loads(evidence.read_text())["reportUrl"], expected)

        target = Path(result["proofDir"])
        (target / "PROOF.md").write_text("# Proof report\nSelected claims and assumptions.\n")
        (target / "SCOPE.md").write_text("# Scope\nOnly the selected claims.\n")
        legacy = self.workspace / "proof-001"
        legacy.mkdir()
        legacy_snapshot = json.loads(evidence.read_text())
        legacy_id = str(uuid.uuid4())
        legacy_snapshot["task"]["id"] = legacy_id
        legacy_url = f'https://old-console.example.test/tasks/{legacy_id}'
        legacy_snapshot["reportUrl"] = legacy_url
        (legacy / "result.json").write_text(json.dumps(legacy_snapshot))
        before = len(self.server.uploads)
        old = self.cli("task", "wait", legacy_id, "--session", self.session["sessionId"])
        self.assertEqual(Path(old["evidenceDir"]), legacy)
        self.assertEqual(len(self.server.uploads), before)
        self.assertFalse((legacy / "request.json").exists())
        output = self.base / "report.html"
        requests = len(self.server.requests)
        exported = self.cli("view", "--output", str(output),
                            "--plugin-data", str(self.base / "no-plugin-data"))
        self.assertEqual(exported["status"], "exported")
        html = output.read_text()
        payload = json.loads(html.split('<script type="application/json" id="data">', 1)[1]
                             .split('</script>', 1)[0])
        self.assertIsNone(payload["dataUrl"])
        files = payload["snapshot"]["files"]
        saved = json.loads(files[evidence.relative_to(self.root / ".intent").as_posix()])
        self.assertEqual(saved["reportUrl"], expected)
        relative = target.relative_to(self.root / ".intent").as_posix()
        self.assertIn("Selected claims and assumptions.", files[relative + "/PROOF.md"])
        self.assertIn("Only the selected claims.", files[relative + "/SCOPE.md"])
        legacy_path = (legacy / "result.json").relative_to(self.root / ".intent").as_posix()
        self.assertEqual(json.loads(files[legacy_path])["reportUrl"], legacy_url)
        self.assertFalse(any(path.startswith("prove/sessions/") and path != legacy_path
                             or "/inputs/" in path for path in files))
        self.assertEqual(len(self.server.requests), requests)
        self.assertIn("Output already exists", self.cli("view", "--output", str(output), code=1)["message"])

    def test_failed_proof_preserves_backend_result_and_exit_code(self):
        requirement = self.requirement()
        successful = self.operation("prove", extra=("--requirement", requirement))
        initial_request = json.loads((Path(successful["evidenceDir"]) / "request.json").read_text())
        bundle = initial_request["specification"]["id"]
        index = self.root / ".intent/source/program.mpy/index.json"
        initial = json.loads(index.read_text())["properties"][requirement]
        current_spec = self.root / ".intent" / initial["specs"][0]
        frozen = current_spec.read_bytes()
        (self.workspace / "inputs/spec.k").write_text("module CHANGED endmodule\n")
        self.server.outcome = "notProved"
        self.server.exit_code = 42
        result = self.operation("prove", code=42,
                                extra=("--requirement", requirement, "--spec-id", bundle))
        self.assertEqual(result["task"]["result"], "notProved")
        self.assertEqual(result["task"]["toolRuns"][-1]["exitCode"], 42)
        self.assertEqual(result["task"]["newBackendField"], {"retained": True})
        evidence = Path(result["evidenceDir"]) / "result.json"
        saved = json.loads(evidence.read_text())
        self.assertEqual(saved["task"], result["task"])
        selection = json.loads(index.read_text())["properties"][requirement]
        self.assertEqual(selection["last_run"], evidence.parent.name)
        self.assertEqual(current_spec.read_bytes(), frozen)
        submitted = saved["properties"][requirement]["specs"][0]
        self.assertNotEqual(submitted["sha256"], hashlib.sha256(frozen).hexdigest())

        self.server.task_status = "failed"
        self.server.exit_code = None
        self.server.outcome = None
        failed = self.operation("prove", code=1)
        self.assertIsNone(failed["exitCode"])
        self.assertEqual(failed["task"]["status"], "failed")
        self.assertTrue(failed["error"])
        self.assertEqual(json.loads((Path(failed["evidenceDir"]) / "result.json").read_text())["task"],
                         failed["task"])
        self.assertEqual(saved["task"], json.loads(evidence.read_text())["task"])

    def test_retry_reuses_submission_idempotency_key(self):
        self.server.submit_failures = [503]
        result = self.operation("prove")
        self.assertEqual([path for path, _ in self.server.uploads], ["/proofs", "/proofs"])
        self.assertEqual(len(set(self.server.keys)), 1)
        self.assertTrue(self.server.keys[0])
        self.assertEqual(len(self.server.tasks_by_key), 1)
        self.assertEqual(result["task"]["id"], next(iter(self.server.tasks_by_key.values())))

    def test_changed_source_prevents_late_spec_promotion(self):
        requirement = self.requirement()
        self.server.after_submit = lambda: (self.root / "program.mpy").write_text("changed\n")
        result = self.operation("prove", extra=("--requirement", requirement))
        evidence = Path(result["evidenceDir"])
        selection = json.loads((self.root / ".intent/source/program.mpy/index.json").read_text())["properties"][requirement]
        self.assertIsNone(selection["last_run"])
        self.assertEqual(selection["specs"], [])
        self.assertTrue((evidence / "inputs/spec.k").exists())
        before = len(self.server.uploads)
        self.cli("task", "wait", result["task"]["id"], "--session", self.session["sessionId"])
        self.assertEqual(len(self.server.uploads), before)
        self.assertFalse((self.root / ".intent/source/program.mpy/specs").exists())

    def test_attempt_limits_refuse_submission_without_overwriting_evidence(self):
        self.config.write_text(self.config.read_text() +
                               'max_run_attempts = 1\nmax_validation_attempts = 1\nmax_proof_attempts = 1\n')
        for command, outcome in (("run", "success"), ("validate", "valid"), ("prove", "proved")):
            with self.subTest(command=command):
                self.server.outcome = outcome
                result = self.operation(command)
                evidence = Path(result["evidenceDir"]) / "result.json"
                before = evidence.read_bytes()
                uploads = len(self.server.uploads)
                refused = self.operation(command, code=20)
                self.assertEqual(refused["status"], "exhausted")
                self.assertEqual(len(self.server.uploads), uploads)
                self.assertEqual(evidence.read_bytes(), before)

    def test_proof_inputs_reject_paths_and_symlinks_outside_inputs(self):
        outside = self.workspace / "outside.k"
        outside.write_text("module OUTSIDE endmodule\n")
        (self.workspace / "inputs/link.k").symlink_to(outside)
        for path in ("outside.k", "inputs/link.k"):
            result = self.cli("prove", "--session", self.session["sessionId"],
                              "--spec", path, "--spec-module", "OUTSIDE", code=1)
            self.assertIn("escapes session inputs", result["message"])
        self.assertEqual(self.server.uploads, [])

    def test_requirements_keep_one_owner_when_their_scope_changes(self):
        requirement = self.requirement()
        local = self.root / ".intent/source/program.mpy/requirements.json"
        owner = self.root / ".intent/source/program.mpy/index.json"
        self.assertEqual(json.loads(local.read_text())["requirements"][0]["id"], requirement)
        self.assertEqual(self.cli("requirements", "show", requirement)["requirement"]["id"], requirement)
        (self.root / "other.py").write_text("pass\n")
        input_file = self.base / "requirement-update.json"
        input_file.write_text(json.dumps({"statement": "Both files preserve the invariant.",
                                         "targets": [{"path": "program.mpy"}, {"path": "other.py"}]}))
        self.cli("requirements", "update", requirement, "--input", str(input_file))
        cross = self.root / ".intent/source/requirements.json"
        self.assertEqual(json.loads(cross.read_text())["requirements"][0]["id"], requirement)
        self.assertEqual(json.loads(local.read_text())["requirements"], [])
        self.assertIn(requirement, json.loads(owner.read_text())["properties"])
        self.assertEqual(len(self.cli("requirements", "list", "--file", "other.py")["requirements"]), 1)
        before = cross.read_bytes()
        input_file.write_text(json.dumps({"statement": "Invalid scope.", "targets": [{"path": "../outside.py"}]}))
        self.cli("requirements", "update", requirement, "--input", str(input_file), code=1)
        self.assertEqual(cross.read_bytes(), before)
        self.cli("requirements", "remove", requirement)
        self.assertEqual(self.cli("requirements", "list")["requirements"], [])
        self.assertNotIn(requirement, json.loads(owner.read_text())["properties"])
        self.assertEqual(self.server.uploads, [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
