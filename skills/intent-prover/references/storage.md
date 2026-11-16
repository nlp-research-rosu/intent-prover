# Project storage

Keep `.intent/` at the repository root and commit durable intent and evidence
with source. Ordinary chats and edits do not trigger remote work.

```text
.intent/
  REQUIREMENTS.md
  NOTES.md
  source/
    requirements.json
    <source-path>/
      requirements.json
      index.json
      specs/<bundle-id>/
  logs/<operation-id>/
    request.json
    result.json
    inputs/
    artifacts/
  prove/sessions/<session-id>/
```

`REQUIREMENTS.md` is editable project-wide human intent. `NOTES.md` remains the
notes file included in explicitly requested Scan uploads when in scope. Scan
does not upload structured requirements, current specs or operation logs.

Use the local `requirements` CLI to add/update/remove structured requirements;
its help owns syntax. Input JSON contains `statement`, nonempty `targets` with
repository-relative `path` and optional `symbol`, and optional `reason`. One-file
requirements live beside that source's current specs. Cross-file requirements
live in `source/requirements.json`; their first target owns the per-source index.
Updates retain that owner while it remains a target. Read the files directly.

Each property has one index entry with `specs` (paths relative to `.intent`) and
`last_run` (an operation ID or null). Properties can share a bundle or operation.
Requirement, bundle and operation IDs use six lowercase letters/digits with
project-local collision checks; backend task and session IDs are unchanged.

Scan and Prove do not require structured requirements. For explicit proof or
validation associations, pass `--requirement <id>` for each selected property.
Each associated submission allocates an independent spec bundle; `--spec-id`
selects an existing bundle for update/sharing. Matching filenames do not choose
a bundle. These local associations are not uploaded or claims of proof coverage.

Logs retain the exact formal inputs, hashes, controls, outcomes and saved remote
job links. Scan retains source hashes and reports, not a second source snapshot.
Successful associated formal inputs can become current spec copies. A failed
attempt can update `last_run`, but cannot replace current specs. Changed source,
requirements, specs, frozen inputs or a newer selection prevent late promotion.
A reference is not proof: inspect the operation's actual outcome, selected claims,
assumptions and submitted versions. Unsubmitted retained specs have unknown
freshness for that attempt.

Write proof reports, scope and reproduction commands in the operation's returned
`proofDir` (`artifacts/`). Never edit frozen inputs or historical results.
`prove/sessions/` remains ignored authoring scratch. The viewer excludes it and
frozen input copies. Existing evidence files are left in place, not migrated.
Previously saved proof-task results remain visible and recoverable in their
original session directories; authoring inputs there are not exported.
