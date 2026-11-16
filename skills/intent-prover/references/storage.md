# Project storage

Keep `.intent/` at the repository root and commit durable intent and evidence
with source. Ordinary chats and edits do not trigger remote work.

```text
.intent/
  REQUIREMENTS.md
  source/
    requirements.json
    <source-path>/
      requirements.json
      index.json
      specs/<bundle-id>/
  logs/<log-id>/
    inputs/
    artifacts/
    attempts/<operation-id>/
      request.json
      result.json
```

`REQUIREMENTS.md` is editable project-wide human intent. Scan consumes that brief,
structured requirements applicable to the selected source files and their selected
current specifications. Attempt records and unrelated workspace files are not uploaded.

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

New sessions start directly in their log folder. Inputs and reports stay there,
including unfinished work; retries edit the same inputs. No finalization, copying
into logs or cleanup is needed. Attempts retain request/result metadata, submitted
hashes, controls, outcomes and remote job links, not historical input copies.
Scan uses a standalone operation log and retains its compressed snapshot under
`inputs/snapshot.json.gz`.
Successful associated formal inputs can become current spec copies. A failed
attempt can update `last_run`, but cannot replace current specs. Changed source,
requirements, specs, submitted input files or a newer selection prevent late promotion.
A reference is not proof: inspect the operation's actual outcome, selected claims,
assumptions and submitted versions. Unsubmitted retained specs have unknown
freshness for that attempt.

Write proof reports, scope and reproduction commands in the returned
`proofDir` (the session log's `artifacts/`, shared across attempts). Never edit
historical request/result metadata or Scan snapshots. The viewer excludes input
files. Existing evidence files are left in place, not migrated.
Previously saved proof-task results remain visible and recoverable in their
original session directories; authoring inputs there are not exported.
