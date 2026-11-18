# Prove

A K proof connects a program's initial state to a required final state under
explicit assumptions. Write the claims and supporting mathematics; the Intent
Prover service executes the pinned language semantics and attempts the proof.
Reachability claims establish partial correctness, not termination.

## Knowledge map

| Topic | Reference |
|---|---|
| Turning a requested property into claims | [Spec authoring](spec-authoring.md) |
| K notation and a spec-file example | [K glossary](k-glossary.md) |
| Syntax, sorts, modules and equations | [K syntax](k-syntax.md), [K claims](k-claims.md) |
| What makes a proof about the actual program | [Proof correctness](proof-correctness.md) |
| Loops, induction and summaries | [Invariants](deriving-invariants.md) |
| Stuck states, unapplied invariants and expanding helpers | [Proof debugging](proof-debugging.md) |
| Dynamic sorts, nested containers or external primitives | [Sort refinement](sort-refinement.md), [nested sequences](nested-sequences.md), [opaque primitives](opaque-primitives.md) |
| Ambiguous requirements | [Contract interpretation](reading-the-contract.md) |
| Sessions, input preparation, flags and results | [Running K](running-k.md) |
| Commands, connection, limits and task recovery | [CLI help](proof-help.md) |

## Working context

Use the global `intent-prover-staging` CLI. Start with the
[CLI update check](onboarding.md#check-cli-updates) once for this requested
workflow; complete onboarding only if setup is missing.
`semantics` lists supported definitions; `session start` pins and downloads one.
Construction and repairs share that session and its limits. Unsupported language
behavior is a model boundary, not a reason to author replacement semantics.

Prepare formal inputs directly in `.intent/logs/<log-id>/inputs/` at `projectRoot`,
using the returned `workspaceDir`. Each attempt saves request/result metadata
and submitted hashes under `attempts/<operation-id>/`, not input copies.
Write `SCOPE.md`, `prove.sh` and `PROOF.md` in the returned `proofDir` (`artifacts/`)
to explain the theorem, actual commands and outcome. Files stay in that same
workspace, including unfinished work. Retries edit the same inputs; earlier
versions are not retained. For optional requirement associations and current
formal files, read [Project storage](storage.md).

Completion means the requested property is established for the agreed domain,
with matching program/claim bytes, terminal evidence and explicit assumptions.
Otherwise report the narrower result or unresolved obligation. Tool success
alone does not show that the claims describe the intended program behavior.

## Report and handoff

Update `PROOF.md` and `SCOPE.md` with the actual outcome, selected claims, program
version, pinned semantics, assumptions/trusted claims, reused results, remaining
obligations and assurance checks actually performed. Cite the supporting task IDs
and local evidence; saved `result.json` records include each new job's `reportUrl`.
Do not infer a historical job's console from today's configuration if no URL was saved.

Then [deliver one local HTML report](../SKILL.md#deliver-results) with
`intent-prover-staging view --output <new-report.html>`, linking directly to this target's
Proof report. Keep remote jobs and raw evidence secondary. An HTML report or
successful CLI exit is not additional proof assurance.
