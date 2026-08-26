# Verify runner exit codes

A worked example of a delta spec. It is also delta's own test suite: the
checks below exercise `delta/bin/verify` against throwaway fixtures, so
running `delta/bin/verify` in a fresh clone proves the runner works.

Read this file for the shape of a spec, not for the subject matter.

## Context

Truth: `delta/truth/` is empty on a fresh clone. This change describes the
runner as built, so the sections below are all ADDED.

## ADDED

- `delta/bin/generate-commands` emits per-CLI command files from
  `delta/commands/` using the table in `delta/adapters.yaml`, and `--check`
  fails when the committed output has drifted.
- `delta/bin/verify` iterates a change's `checks/` directory, runs each check
  as a subprocess, records the result under `run/<utc-timestamp>/`, and exits
  with a code that distinguishes "passed" from "nothing ran".
- Criteria are declared in this file under `## Acceptance criteria` as
  `- C<n> <text>`, or `- C<n> MANUAL <text>` for criteria that cannot be
  automated.
- A check is bound to a criterion by filename prefix: `checks/C3-*` serves
  criterion `C3`.
- MANUAL criteria are resolved by a line in `run/signoff.md`.

## MODIFIED

None. Nothing existed before this change.

## REMOVED

None.

## RENAMED

None.

## Acceptance criteria

- C1 verify exits 0 when every criterion has a passing check
- C2 verify exits 1 when a check fails
- C3 verify exits 2 when a criterion has no corresponding check
- C4 verify exits 3 when a MANUAL criterion has no recorded sign-off
- C5 generated per-CLI command files are in sync with delta/commands/
- C6 MANUAL summary and columns stay aligned in a narrow, non-UTF-8, non-TTY terminal
      reason: legibility is a judgement about rendered output, and a check that
      asserted a byte-exact layout would break on every wording change
      look at: `COLUMNS=46 LANG=C delta/bin/verify example-verify-exit-codes | cat`
      confirm: the frame never wraps to a second line, descriptions truncate
      rather than wrap, and the closing line carries real counts
