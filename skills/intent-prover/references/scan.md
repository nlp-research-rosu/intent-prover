# Scan

Start with the [CLI update check](onboarding.md#check-cli-updates) once for this
requested workflow. Reuse saved sign-in.

Use the global native CLI as described in [Intent Prover](../SKILL.md#commands).

## Before submitting

For a new Scan, run `intent-prover scan --preflight` with the requested path
and depth. This is a local check and does not require sign-in. Do not run it
for `scan collect`.

If preflight says no requirements will be uploaded, explain that inferred
requirements can miss expected behavior or mistake intentional behavior for
a bug. Offer to use [Formalize](formalize.md) first or to scan without supplied
requirements. Wait for the user's choice unless they already asked to proceed
without requirements. Do not adopt inferred requirements automatically.
If the check cannot determine what would be uploaded, report the problem and
do not submit. A direct CLI scan without `--preflight` does not pause.

Before submitting, explain that the requested code will be uploaded to the
configured service with obvious secrets masked. Mention that the upload can
include applicable requirements and their selected specifications. A generic
local-review request is not permission to upload code.

## Submit

- One file: `scan <file>`.
- Folder: `scan <folder>`.
- Repository: `scan`.
- Depth: `fast` (default, about 15–20 minutes), `balanced` (30–35), or `full` (40–45).

Run inside a Git repository. The snapshot includes tracked and non-ignored
untracked text files within the requested path. It excludes secrets, binary,
generated and vendored files, lockfiles and historical `.intent/` evidence.
It includes `.intent/REQUIREMENTS.md`, applicable structured requirements and their
selected current specifications. These receive the same secret masking and size
limits; context that exceeds the limits stops submission rather than being trimmed.
All submissions use `/scan`. No command reads chat transcripts, prompts,
plans or edit history. Upload metadata is recorded in `.intent/.sent.log`.

The command prints the remote job link and task ID, polls until completion, saves
evidence and returns findings. Keep it running using the host's long-running
command support. Do not submit a second Scan or implement a separate polling loop.

## Collect and report

If waiting is interrupted or a request fails, the task may still be running.
Use `scan collect <task-id>` to resume polling and save evidence without
resubmitting source. `scan collect` handles outstanding scans in this project.
Ordinary messages do not resume scans or deliver results.

Reports live in `.intent/logs/<operation-id>/artifacts/` at the repository root.
`request.json` and `result.json` beside that folder record the operation and
source hashes. The backend task ID is unchanged. Preserve
existing evidence. Commit project knowledge with source; do not blanket-ignore
`.intent/`.

Check findings against the submitted version, then [deliver one local HTML
report](../SKILL.md#deliver-results) using `intent-prover view --output <new-report.html>`.
The saved remote job URL is a secondary link inside the viewer. State coverage
and exclusions; Scan findings are not reproduced bugs or formal proofs.
Offer to reproduce or fix; findings do not authorize edits. After a
fix, another remote Scan requires an explicit request. For requirements changes,
follow [Formalize](formalize.md).
