# Findings — CR-006 information graphics

Read: delta/bin/verify, delta/bin/report, delta/bin/palette.sh,
delta/commands/{explore,propose,apply,archive,verify}.md, README.md,
AGENTS.md, delta/adapters.yaml.

## Entry points

- `delta/bin/verify` — single-change mode (unchanged shape) and the new
  `--all` dashboard mode, both reachable from the same option-parsing loop.
- `delta/bin/stage-rail` (new) — invoked as a subprocess by `verify` and,
  per prose instruction, literally by explore/propose/apply/archive.
- `delta/bin/report` — Q4's new heatmap is a straight extension of its
  existing per-change, per-criterion aggregation pass.

## Data touched

- `delta/changes/<id>/run/*/results.tsv` — read-only, by stage-rail (last
  run's outcome), `verify --all` (per-criterion history), and report's
  heatmap. No writer changed.
- `delta/changes/<id>/spec.md` — read for checkbox marks (`- [x]`, `- [!]`)
  by stage-rail's apply-stage inference. Nothing writes checkboxes yet;
  none of this repo's own prior changes used the convention apply.md
  documents, so stage-rail falls back to "a run exists" as evidence apply
  happened - see its own header comment.
- `${TMPDIR:-/tmp}/.delta-reveal-seen/<tty>` — new, written by the startup
  reveal to remember it has already played in this terminal session.

## Unknowns

- Whether a real end-user's machine reaches the reveal's ~200ms target in
  practice. In this sandboxed environment, forking `tty`/`stty`/`sleep`
  cost enough (measured 5-80ms per external process) that the full
  animation-plus-verify-run wall time was closer to 450-500ms; a real
  terminal on unshared hardware should fork much faster, but this was not
  measured on one. Documented as a limitation rather than tuned to a number
  this environment cannot actually confirm.
- Whether the flow diagram's Mermaid export renders correctly inside an
  actual GitLab instance - this environment has no network egress to test
  against a real GitLab. Validated structurally instead (C6): a real
  `flowchart` header, every body line an edge, balanced brackets.
- Whether `delta/bin/verify`'s per-second run-directory timestamp
  granularity (pre-existing, not introduced here) could cause two very
  fast successive runs to collide into one directory outside of test
  fixtures - encountered once in this change's own fixture-writing and
  worked around with a sleep there, but real usage rarely runs `verify`
  twice inside the same second.
