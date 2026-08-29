# CR-006 — Information graphics

## Context

CR-004 gave delta motion and colour. This gives it shape: a lifecycle rail
on every command, a call-chain diagram (plus a Mermaid export) from
`explore`, sparklines in a new `verify --all` dashboard, a failure heatmap
in the report, and a once-per-session startup reveal. Same constraints as
CR-004: no dependencies, no TTY means plain output, `NO_COLOR` respected,
ASCII fallback, cursor restored on every exit path, unicode block/box
characters only — no sixel, no terminal-specific image protocols.

While building the rail, found and fixed a real bug in `delta/bin/verify`
itself (not a docs issue, a code issue, fixed in this same change since it
directly affects the rail): the run directory for the *current* invocation
was being created before the opening frame printed, so a check that
reads "the last run" — which is exactly what the rail does — read the
in-progress, still-empty run as a completed, all-passed one. Fixed by moving
the frame-and-rail print to before run-directory creation, matching the
rail's own premise: it answers "where am I" from what already happened,
not from the run this command is itself about to make.

## ADDED

- `delta/bin/stage-rail` — computes and prints the two-line lifecycle rail.
  The single source of the stage logic: `verify` calls it directly, and
  explore/propose/apply/archive's prose instructs running it literally
  (mirroring the existing root-bootstrap pattern in `verify.md`), so no
  command's rail can say something different from another's.
- `delta/bin/verify --all` — a read-only dashboard across every change,
  never executes a check, reads existing `run/*/results.tsv` history, shows
  each non-MANUAL criterion's last eight runs as a duration/pass-fail
  sparkline (Unicode block characters, ASCII numeric fallback).
- A failure heatmap in `delta/bin/report`'s fourth question, one per change
  with two or more runs: checks down the side, runs across, oldest first,
  passed/failed/could-not-run/manual each a distinct palette colour.
- Flow-diagram instructions in `delta/commands/explore.md`: box-drawing
  call-chain diagram with unknowns marked inline, an 80-column width rule
  that falls back to a prose chain, and an unconditional companion
  `delta/changes/<id>/flow.mmd` Mermaid export.
- A startup reveal in `delta/bin/verify`: the opening frame draws with a
  brief left-to-right reveal the first time in a terminal session (keyed by
  `tty`'s device path), instantly on every command after. Skipped off a
  TTY; interruptible by any keypress via a non-blocking `read` against the
  controlling tty (no `dd`/`read -t` — see delta/bin/verify's own header).
  Real only for `verify`, the one command among the five that is an actual
  process; explore/propose/apply/archive get the rail (a real script's
  output, safe to run from a prompt) but not a fake animated reveal, which
  would claim live terminal control none of them actually have.
- `PALETTE_STAGE_*`, `PALETTE_RAIL_CONN_*`, `PALETTE_SPARK_*` in
  `delta/bin/palette.sh` — the rail's dots/connector and the sparkline's
  eight levels, unicode-first with an ASCII fallback pair, same as
  everything already there.

## MODIFIED

- `delta/bin/verify` — opening-frame print reordered to before its own
  run-directory creation (see Context above); this is a real behaviour fix,
  not new information-graphics behaviour, but ships in this change since it
  was found while building the rail and the rail is meaningless without it.

## REMOVED

None.

## RENAMED

None.

## Acceptance criteria

- C1 the lifecycle rail appears on `verify`'s own output and shows the
      correct stage for each of the five positions
- C2 a failed stage stays red on subsequent commands until resolved, not
      only on the command whose run produced the failure
- C3 explore, propose, apply, and archive's canonical and generated prompt
      files all instruct running `delta/bin/stage-rail` literally, for
      every shipped CLI adapter
- C4 explore's instructions require inline unknowns, state the 80-column
      width rule with a prose-chain fallback, and require the Mermaid
      export, for every shipped CLI adapter
- C5 MANUAL the flow diagram in explore's worked example reads clearly,
      the unknown sits under the node it questions rather than in a
      trailing list, and the Mermaid version renders correctly in GitLab
      reason: diagram legibility and real GitLab rendering are not
        mechanically checkable — this environment has no GitLab to render
        against and no way to score "reads clearly"
      look at: the flow-diagram section of delta/commands/explore.md
      confirm: a newcomer could learn the notation from the legend line
        alone, and the Mermaid block uses the same node names as the
        diagram (checked structurally, not rendered, by C6)
- C6 explore's own worked Mermaid example is syntactically well-formed
      flowchart syntax (a valid header, every body line an edge, balanced
      brackets)
- C7 sparklines in `verify --all` use only Unicode block characters and
      fall back to a numeric summary in ASCII mode
- C8 the report's heatmap distinguishes failed, could-not-run, and manual
      with three distinct colours from the shared palette
- C9 the startup reveal plays once per terminal session, is skipped
      entirely when piped, and completes immediately on any keypress
      instead of finishing the full animation
- C10 verify, verify --all, and stage-rail all emit zero escape sequences
      when their output is not a TTY
- C11 no literal hex colour or raw truecolor escape appears in
      delta/bin/verify, delta/bin/report, or delta/bin/stage-rail outside
      what palette.sh's own RGB triples and helpers produce
- C12 MANUAL the flow diagram is, on its own, the graphic in this change
      worth showing a colleague — the one a person who does not use delta
      would still want to see
      reason: this is the CR's own stated bar for the one graphic it says
        is worth spending time on, and it is a judgement call, not a
        measurement
      look at: the flow-diagram section of delta/commands/explore.md
      confirm: the worked example is a realistic multi-hop chain, not a
        toy, and the notation is introduced clearly enough to read without
        the surrounding prose
