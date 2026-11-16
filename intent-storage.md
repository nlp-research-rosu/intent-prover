# .intent

`REQUIREMENTS.md` is editable project-wide human intent. Requirements are natural-language statements; specifications are formal files, whether checked or unchecked.

Use the `requirements` CLI to add or update structured requirements from JSON. Input contains a statement, nonempty targets (repository-relative path and optional function symbol), and optional reason. The CLI generates six-character IDs from lowercase letters and digits, checking project-local uniqueness and retrying collisions. Requirements, specification bundles and local operations use these IDs; backend task and authentication/session IDs stay unchanged. Updates retain their identity. Read files directly with normal coding-agent tools; no context command is required.

```text
.intent/
  README.md
  REQUIREMENTS.md
  source/
    requirements.json
    src/shop.py/
      requirements.json
      index.json
      specs/
        k7m2qx/
          discount.k
  logs/
    <log-id>/
      inputs/
      artifacts/
      attempts/
        <operation-id>/
          request.json
          result.json
    <scan-id>/
      request.json
      result.json
      inputs/
      artifacts/
```

One-file requirements live in `source/<source-path>/requirements.json`; cross-file requirements live in `source/requirements.json`. Each requirement has one owning per-file index. The first target supplies the initial owner; updates retain that owner while it is still a target. There is no global index or specification folder. Cross-file properties can reference specifications belonging to multiple source files.

An index contains `schema_version: 1` and a `properties` object keyed by requirement ID. Each entry contains only `specs` (paths relative to `.intent`, such as `source/src/shop.py/specs/k7m2qx/discount.k`) and `last_run` (a local operation ID, or null). An empty specs list is valid. Multiple properties can share formal files and operations. Do not create competing ownership entries.

`specs/` holds actual current working formal files, not symlinks. Each new proof session starts directly in `logs/<log-id>/`, with editable formal inputs in `inputs/` and reports, scope records and reproduction commands in `artifacts/`. Retries reuse the same workspace, and files remain there at any stopping point without finalization, moving or cleanup. Each operation retains request/result metadata, submitted hashes, scope and assumptions in `attempts/<operation-id>/`. Inputs are referenced in place, not copied per attempt; earlier input versions are not retained. Concrete runs likewise reference their selected program in place. Scan retains its compressed submitted snapshot in its own operation's `inputs/`. A run reference is not a claim that its property was proved.

Explicit proof `--requirement <id>` associations bind the submitted formal files to those properties. By default, an associated attempt allocates an independent specification bundle under its first property's owning source. Use `--spec-id <id>` to update an existing bundle or share it with another property; matching filenames alone never select a bundle. Files retain their upload-relative paths within `specs/<id>/`, including helpers and relative imports. Applicable formal files are copied to current per-source specifications only after a completed, error-free `proved`, `valid` or `success` result with no failed final tool run; failed attempts may update last_run without replacing specifications. Partial output never deletes an absent specification. A late operation cannot replace changed source, requirements, working specifications, submitted input files or a newer selection. Read freshness from the referenced operation's exact input hashes; edits can make evidence stale without deleting current files.

Scan keeps canonical metadata, source bindings, collection status and evidence level in its operation's `result.json`, with the saved report in `artifacts/report.json`. A completed Scan is LLM-reviewed evidence, not formal proof.

Agents may maintain `REQUIREMENTS.md` and read the whole folder. Use the requirements CLI for structured requirement mutations. Keep current formal files and their index links consistent; do not edit historical request/result metadata or Scan snapshots. Session inputs remain editable between attempts. Merge source, properties and current formal files together; use operation logs to resolve conflicts.

Requirement mutations validate affected documents and indexes before writing. Property freshness compares current files with the formal versions in the referenced operation's submitted-input manifest, independently of success or failure. Different submitted contents are stale; retained specs absent from that submission have unknown freshness for that attempt.

`.sent.log` is an upload audit without contents. It and `.storage.lock` are ignored by Git. `.ignore` excludes generated evidence from ordinary searches. Requirements, current specifications and session log folders may be committed. Viewer/export excludes private conversations and input files. Existing sessions/evidence remain in their original locations; no automatic migration or deletion occurs.
