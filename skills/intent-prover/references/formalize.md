# Formalize

Make intended behavior explicit so edits, Scan and proofs have a shared definition
of correctness. Keep requirements current and respected as the work evolves.
Use available user context, documentation, code, comments and
tests to understand intent; existing behavior is not automatically desired
behavior. For a new project, work from the user's goals and decisions.

Resolve unclear or undecided behavior with reasonable, clearly marked assumptions
or focused questions when the choice matters. Stop questioning when the current
work is clear enough. Update requirements when intent changes, not merely to
make incorrect code appear compliant. Record lasting product intent, not temporary
task instructions. Requirements are natural-language intent; formal specifications
and verification evidence are separate.

## Project intent

Maintain `.intent/REQUIREMENTS.md` at the repository root. Preserve its headings
and existing intent when updating content:

```markdown
# Project requirements

## Purpose

## Expected behavior

## Constraints

## Out of scope

## Open questions
```

Purpose explains what the project is for; Expected behavior describes what it
should do. Constraints records restrictions and labeled assumptions. Out of scope
states exclusions; Open questions keeps unresolved decisions visible rather than
presenting guesses as confirmed requirements.

## Scoped requirements

Use the local `requirements` CLI for file/function properties. Input JSON contains
`statement`, nonempty `targets` with repository-relative `path` and optional
`symbol`, and optional `reason` for provenance or an explicitly labeled assumption.
Targets must name existing source files. Keep untargeted intent in the brief
until suitable files exist.

For an existing `wallet.py` with a `withdraw` function:

```sh
intent-prover requirements add --json '{
  "statement": "A zero withdrawal returns the original balance.",
  "targets": [{"path": "wallet.py", "symbol": "withdraw"}],
  "reason": "Documented withdrawal behavior."
}'
intent-prover requirements list --file wallet.py
intent-prover requirements update <id> --json '{"statement":"A zero withdrawal returns the original balance without a fee.","targets":[{"path":"wallet.py","symbol":"withdraw"}]}'
```

Use the returned ID for updates to the same requirement; do not add duplicates
or invent IDs. The CLI writes the scoped `requirements.json` and index records.
Use either inline `--json` or `--input <file>`, not both. Its help owns command
syntax; [Project storage](storage.md) explains placement and evidence links.
