# CR-DOCS — Reconcile docs with shipped behaviour (recurring)

archived: 2026-08-29

## Context

Six prior changes shipped (initial build, CR-001, CR-002, CR-002.R, CR-005,
CR-004) with no standing process to catch docs drifting from what actually
shipped. Two real drifts had already accumulated by this run — see
explore.md. This change is designed to run again, unchanged, every 2-4
shipped changes from now on: read CHANGELOG.md for the last reconciliation
marker, reconcile everything shipped since, append a new marker.

Read the code, not the CRs: where a doc and a shipped change's own intent
disagree, the code wins. If the code itself turns out wrong, that's a
separate CR — this one only ever changes documentation and the CHANGELOG,
never `delta/bin/*` behaviour.

## ADDED

- `CHANGELOG.md` — one entry per shipped change from the initial build
  through CR-004, each flagging whether it changes prior user-visible
  behaviour, plus a "Reconciliation log" section this change (and every
  future run of it) appends a dated entry to.
- `delta/changes/docs-reconciliation/checks/` — greps that catch the classes
  of drift found this run, meant to be re-run (and extended, never
  replaced) on every future reconciliation pass, so the manual-review burden
  shrinks over time to the two criteria that can't be mechanized: whether
  the shown examples actually run, and whether the README still reads well.

## MODIFIED

- `README.md` — 564 lines to 198. Depth that didn't survive the cut (palette
  internals, telemetry internals beyond the four questions) points readers
  at the already-thorough header comments in `delta/bin/palette.sh` and
  `delta/bin/report` instead of restating them.
- `delta/commands/verify.md` (and the four files `generate-commands` derives
  from it) — exit code 7 added to the documented list; "exits with a code
  from 0 to 6" corrected to "0 to 7"; the root-bootstrap error message now
  says to copy `delta/bin/palette.sh` alongside `delta/bin/verify`.
- `delta/commands/propose.md` — the "copy delta/bin/verify in" instructions
  now say to copy `delta/bin/palette.sh` too, and say why.
- `AGENTS.md` — exit code 4 added to the summary (previously listed 0, 1, 2,
  3, 6, 7 and skipped 4); its own "copy delta/bin/verify" line now covers
  `delta/bin/palette.sh`.

## REMOVED

None.

## RENAMED

None.

## Acceptance criteria

- C1 no document under README.md, AGENTS.md, or delta/commands/ references a
      removed or never-shipped feature (ui.html, /delta:bug, an init command,
      CR-003)
- C2 every path shown in README.md's two Layout trees exists in the repo
- C3 `delta/bin/install` succeeds from a scratch `$HOME` that has never seen
      delta, writing all 20 files (5 commands × 4 adapters) it claims to
- C4 every exit code delta/bin/verify's source can actually produce is
      documented in both README.md and delta/commands/verify.md, and neither
      documents a code the source doesn't produce
- C5 README.md is under 200 lines
- C6 README.md states the Windows/Git Bash platform requirement and lists
      the deliberately-not-built items
- C7 CHANGELOG.md has one entry for every shipped change in `git log`
      between the initial `Add delta` commit and this change, each with a
      `behaviour change:` line
- C8 CHANGELOG.md's reconciliation log has a dated entry, and reconciliation
      is not more than 4 shipped changes overdue (this CR's own cadence)
- C9 MANUAL every command shown as a literal, runnable example in README.md,
      AGENTS.md, and delta/commands/*.md was actually executed this run, not
      eyeballed
      reason: illustrative check bodies (the webhook/psql example) and
        placeholder paths (`/path/to/repo`) can't be executed generically,
        and even the truly runnable ones need a human judgement call about
        what "as written" means when a command takes an optional argument
      look at: this run's transcript — `delta/bin/install` was run for
        real; `delta/bin/verify example-verify-exit-codes` and the full
        six-change regression sweep were run; `delta/bin/generate-commands
        --target project --check` was run after every edit to
        delta/commands/*.md
      confirm: every command that can run without inventing data (not the
        webhook/psql pseudocode, not the DELTA_ROOT=/path/to/repo
        placeholder) exited the way this change's docs say it does
- C10 MANUAL README.md reads as a five-minute introduction that gets a
      newcomer to a working first command, not a reference dump
      reason: readability and proportion are a judgement call, not
        something a grep can confirm
      look at: README.md in full, start to finish
      confirm: a reader with no prior context reaches a working
        `delta/bin/install` within the first third of the file, and nothing
        past that point is load-bearing for getting started
