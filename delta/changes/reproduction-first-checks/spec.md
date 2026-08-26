# CR-001 — Bug fixes: reproduction-first checks

## Context

delta assumed a change begins with knowing what to build. A bug fix does not:
the intent is "make this stop happening", the cause is unknown at propose time,
and the most valuable check is one that **fails before the fix and passes
after** — the inverse of what `verify` expected.

Run against a bug, two things went wrong. `propose` asked for a change that
cannot yet be described, producing a guess dressed as a spec. And a
reproduction check made `verify` exit 1 on a fresh spec: correct behaviour
reported as failure, so the tool looked broken exactly when it was working.

A bug repro is the strongest check delta can hold. Feature criteria are
aspirational; a repro is proof, written before the fix, and it stays in
`checks/` afterwards as a permanent regression guard.

## ADDED

- Checks may declare an expectation in a header comment:
  `# EXPECT: fail-until-fixed`. The default is `pass`.
- `reproduced` is a distinct result state with its own glyph (`◆` / `[rep]`),
  counted separately in the summary and rendered in the default colour: it is
  neither a pass nor a failure.
- `suspicious` is a distinct result state (`!` / `[!!]`, red) for a
  `fail-until-fixed` check that passes without ever having been reproduced.
- `delta/changes/<id>/run/reproductions.md`, a git-tracked ledger written by
  `verify`, recording `reproduced` and `fixed` events with UTC timestamps. It
  is tracked rather than left in the ignored per-run directories because it is
  what distinguishes a fixed bug from a repro that never reproduced.
- Exit code 5: `--archive-gate` only, a reproduction is still outstanding.
- Exit code 6: a reproduction did not reproduce.
- `--archive-gate` flag, which names the outstanding reproductions and exits 5.

## MODIFIED

- Criterion ids were `C<n>`; they are now any uppercase prefix plus a number,
  so a bug spec can number its criteria `B1`, `B2`. `C<n>` is unaffected.
- A run whose only non-passing checks are reproductions now exits **0** rather
  than 1. Nothing is wrong: the bug is confirmed and the fix is not written yet.
- When a `fail-until-fixed` check passes and the ledger shows it was reproduced
  earlier, `verify` rewrites the check's `EXPECT` line in place, permanently
  making it an ordinary regression guard, and records the flip.
- Detail lines now truncate to the terminal width instead of wrapping, matching
  the existing rule for result lines.
- The summary reads `1 criterion` rather than `1 criteria`.
- `propose` accepts a symptom-only intent: the spec opens with observed and
  expected behaviour, the first criterion is the reproduction, and MODIFIED may
  be left empty at propose time and filled during apply.
- `archive` runs `verify --archive-gate` and refuses while any reproduction is
  outstanding.

## REMOVED

None.

## RENAMED

None.

## Out of scope

No `/delta:bug` command — this is the same five-command lifecycle with one
additional check state, and a parallel command set would duplicate everything
and drift. No automatic root-cause analysis; `explore` already reads the area.

## Acceptance criteria

- C1 a check with no EXPECT header behaves exactly as it did before this change
- C2 a failing fail-until-fixed check renders as reproduced and the run exits 0
- C3 the same check passing flips it to a normal check and writes a timestamped record to run/
- C4 verify --archive-gate exits non-zero while a reproduction is outstanding, naming it
- C5 a passing fail-until-fixed check on a fresh spec is reported as suspicious
- C6 MANUAL propose on a symptom-only intent produces a spec whose first criterion is the reproduction, with no invented MODIFIED entries
      reason: propose is agent output, so the only way to assert this is to read
      the instruction and judge it. A check asserting the presence of the word
      "reproduction" in propose.md would pass while the instruction said
      something useless
      look at: the "Bug fixes" section of delta/commands/propose.md, and the
      same text in the generated .claude/commands/delta-propose.md
      confirm: it requires observed/expected behaviour, makes the reproduction
      the first criterion, and explicitly permits an empty MODIFIED section
