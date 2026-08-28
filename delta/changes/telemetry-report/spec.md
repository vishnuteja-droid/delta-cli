# CR-005 — Telemetry report

## Context

The workflow UI is dropped: a static page cannot carry buttons or stream a
live run, and every fix for that costs a runtime dependency. Telemetry has
neither problem — it is entirely backward-looking, so a generated file is
the correct shape, not a compromise.

`delta/bin/report` reads `run/` across the repo's changes and writes
`delta/report.html`. Open it, read it, close it. No server, no dependency,
no live anything. It depends on nothing but `verify`, which already writes
the run records this reads.

Question 2 — the failure rate on verify — is why this is worth building now.
"Agents report completion they cannot substantiate" is the whole argument
for delta; until this, you could only assert it. A count of criteria that
failed after the agent said it was done turns that into evidence, and it
accumulates from the runs already happening, whether or not anyone opens
the report.

## ADDED

- `delta/bin/report` — a POSIX sh + awk script, same portability discipline
  as `delta/bin/verify`. Reads `delta/changes/*/run/*/results.tsv` and
  `delta/changes/*/spec.md` (for the `archived:` marker), and writes
  `delta/report.html`.
- Four sections, nothing else:
  1. **Is delta being used?** — changes started, changes archived, total
     verify runs, and a sparkline of runs per day (once there is enough
     history to call it one).
  2. **Does verification catch anything?** — the percentage of ordinary
     (non-MANUAL) criteria that failed on verify, shown unconditionally
     since it is a real number even at n=1, not a trend claim. MANUAL and
     error counts are shown alongside, explicitly labelled as excluded from
     it.
  3. **How testable are our specs?** — per change, a two-colour bar of
     criteria that compiled to a check versus criteria marked MANUAL,
     ordered chronologically by each change's first run.
  4. **What keeps breaking?** — criteria that failed on more than one run,
     and reproductions (the `EXPECT: fail-until-fixed` lifecycle) that took
     more than one attempt to turn green.
- `--min-runs N` (default 5): the threshold below which Q1's sparkline and
  Q3's "this is a trend" framing are replaced with a plain statement that
  there isn't enough history yet. The underlying numbers still show; only
  the charting is gated.
- An empty-state page (`delta/changes/` absent, or present but nothing has
  ever been verified) — a valid, themed HTML document explaining that,
  exit 0, not an error.
- `delta/report.html` added to `.gitignore` — generated from already-
  gitignored `run/` data, local history, not source.

## Honest counting

`passed`/`failed` (Q2's headline) never includes `manual-open`, `manual-
signed`, `error`, `no-check`, or the reproduction-lifecycle statuses
(`reproduced`/`suspicious`/`fixed`). MANUAL and error are separate, labelled
counts. Q3 counts `error` toward "compiled to a check" — the check exists
and was written, it just could not execute this time — which is a testability
question, not a pass/fail one; that choice is stated so it can be argued with.

## Aggregate only

No author, username, hostname, or per-developer breakdown anywhere in the
output or the script that produces it — see the CR's own "Aggregate by
default" section.

## Out of scope

No server, no live data, no auto-refresh, no export, no scheduled
generation, no CI integration, no cost or token accounting, no cross-repo
aggregation. Does not require CR-003.

## Acceptance criteria

- C1 report runs with no arguments and writes delta/report.html
- C2 the report opens over file:// with zero network requests and no JavaScript
- C3 charts are inline SVG, no library
- C4 criteria marked MANUAL are counted separately and never as failures
- C5 checks that could not run are counted separately from criteria that failed
- C6 a repo with fewer than a handful of runs says the history is too short rather than charting it
- C7 a repo with no run/ data at all produces a valid page explaining that, not an error
- C8 MANUAL state colours match the CLI exactly, from the shared palette
      reason: "the shared palette defined in CR-004" does not exist in this
      repository's history - CR-004 was never produced, so there is nothing
      to diff the report's colours against. This is disclosed rather than
      silently invented. What can be verified is that the report's semantic
      colour mapping matches delta/bin/verify's actual C_PASS/C_FAIL/C_DIM
      usage, which a check cannot judge as a design decision but a read can
      look at: the palette comment block at the top of delta/bin/report, the
      four semantic colour classes (.pass/.fail/.n.accent/dim text) used in
      the generated HTML, and delta/bin/verify's own glyph-to-colour
      assignment (grep for G_PASS/G_FAIL/G_MAN/G_REPRO next to C_PASS/
      C_FAIL/C_DIM in the emit calls)
      confirm: passed and fixed use the same colour family as verify's
      C_PASS; failed, suspicious, and error use the same family as verify's
      C_FAIL; manual and pending use a dim/muted treatment matching C_DIM;
      reproduced uses the neutral foreground colour, matching verify's own
      choice to print it with no colour at all
- C9 no developer name or identifier appears anywhere in the output
