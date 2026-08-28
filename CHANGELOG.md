# Changelog

One entry per shipped change. Newest first. `behaviour change: yes/no` says
whether something that used to work a certain way now works differently for
someone already using delta — not whether the change added something new.

Maintained by `CR-DOCS`, a recurring reconciliation change (see bottom of
file). Every other entry documents delta itself; `CR-DOCS` entries document
a documentation pass, not a code change, so they carry no behaviour-change
line.

## CR-004 — Presence, in the terminal and the UI (2026-08-28)

`delta/bin/verify` gets a live status line while a check runs, a truecolor
gradient rule on terminals that support it, signal-safe cursor/colour
restore on `INT`/`TERM`, and a distinguishable clean-vs-failing closing
frame. `explore`, `propose`, `apply`, `archive` get matching streamed-output
instructions (honest about being composed text, not a live terminal — only
`verify` is a real process). `propose`'s review diff renders as fenced
` ```diff ` blocks. Colours and glyphs move into the new
`delta/bin/palette.sh`, shared by `verify` and `report` so the terminal and
the HTML report render the same run identically. The live workflow UI
(`ui.html`) referenced in this change's own originating spec was never
built — scoped out, terminal-only shipped.

behaviour change: yes — `delta/bin/verify` now hard-requires
`delta/bin/palette.sh` alongside it in `delta/bin/`; a repo that copied in
only `delta/bin/verify` before this change will see it fail to start
(`. delta/bin/palette.sh: No such file`) until `palette.sh` is copied in
too.

## CR-005 — Telemetry report (2026-08-28)

`delta/bin/report` reads `delta/changes/*/run/` and writes a single
self-contained `delta/report.html` — usage, verification failure rate,
testability (checks vs MANUAL), and recurring failures. No server, no
JavaScript, no network request, aggregate-only (no per-developer data).
Gitignored: generated from already-gitignored `run/` data.

behaviour change: no — purely additive; nothing existing changes shape.

## CR-002.R — Revert the runner bootstrap machinery from CR-002 (2026-08-28)

CR-002 added a version-checked, self-updating `delta/bin/verify` bootstrap.
This change reverts that machinery: `delta/bin/verify` goes back to being a
single committed file with no version to compare and nothing to bootstrap.
Adds the `error` result state and exit code **7** for a check that could not
run at all (missing executable bit, or a shebang naming an interpreter that
is missing or not executable) — previously such a check was silently folded
into `failed`, which sent people to debug application code that was fine.

behaviour change: yes — the runner-versioning/self-update behaviour CR-002
introduced is gone; `delta/bin/verify` is a plain committed file again.
Also yes for the new exit 7: a check that could not run used to report as an
ordinary failure (exit 1) and now reports as `error` (exit 7) instead.

## CR-002 — Global commands, lazy per-repo data (2026-08-28)

`delta/bin/install` writes the five command files into every supported
CLI's personal command directory once per machine, so `/delta-explore` and
friends work in any repository without per-repo setup. A repository's own
`delta/` (`truth/`, `changes/`, `constitution.md`) is created lazily by
`propose`, the first time it runs there — no separate init command.

behaviour change: yes — before this change, delta had to be copied into
every repository by hand; after it, installing once makes the commands
available everywhere on that machine, and a repo opts in just by running
`propose`.

## CR-001 — Bug fixes: reproduction-first checks (2026-08-26)

Adds `# EXPECT: fail-until-fixed` for a check that should fail before a fix
and pass after: `reproduced` (fails as expected, exit 0 — the bug is
confirmed, not yet fixed), `fixed` (passes after having reproduced, exit 0),
`suspicious` (passes without ever having reproduced — the repro doesn't
reproduce, exit **6**). The flip is recorded in
`delta/changes/<id>/run/reproductions.md`. `archive` gates on outstanding
reproductions via `delta/bin/verify --archive-gate` (exit **5**).

behaviour change: no — purely additive; a check with no `EXPECT` header
behaves exactly as it always did.

## Initial build — delta (2026-08-26)

First version: the five-command lifecycle (explore, propose, apply, verify,
archive), `delta/bin/verify` as a committed POSIX-sh runner, checks as
executable files bound to criteria by filename, MANUAL criteria with
`run/signoff.md` sign-off, the `delta/commands/` + `delta/adapters.yaml` +
`delta/bin/generate-commands` adapter system for Claude Code, Gemini CLI,
Antigravity CLI, and Codex, and `AGENTS.md` as the durable cross-tool
target.

behaviour change: n/a — nothing existed before this.

---

## Reconciliation log

Each entry below marks a `CR-DOCS` run: the range of shipped changes it
reconciled docs against, and what it found.

### 2026-08-28 — first reconciliation

Range: initial build through CR-004 (everything above; no prior
reconciliation marker existed).

Found and fixed:
- `delta/commands/verify.md` (and all four generated CLI command files)
  never got exit code **7** added — CR-002.R added it to `delta/bin/verify`
  itself but the prompt every `/delta-verify` is generated from still said
  "exits with a code from 0 to 6" and stopped its code list at 6.
- `AGENTS.md`'s exit code summary omitted exit **4** entirely.
- The "copy `delta/bin/verify` in from another repo" instructions
  (`delta/commands/propose.md`, `delta/commands/verify.md`, README) said to
  copy one file. Since CR-004, `delta/bin/verify` sources
  `delta/bin/palette.sh` unconditionally — copying `verify` alone now
  reproduces as a raw shell error (`. delta/bin/palette.sh: No such file`,
  exit 2) instead of delta's own clean error message. Docs now say to copy
  both files. The raw-error behaviour itself is a real defect (exit 2
  collides with the documented meaning of exit 2 — "criterion has no
  check" — which is actively misleading) and is flagged here for a future
  CR rather than fixed in this one: this reconciliation documents shipped
  behaviour, it does not change it.
- README.md was 564 lines; trimmed to under 200 per this CR's proportion
  rule. Depth that didn't belong in a 5-minute read moved to the source
  files' own header comments (already thorough) rather than a new `docs/`
  location, which this CR rules out.
- No `docs/` directory exists in this repository. Nothing to reconcile
  there this run.

Not fixed (correctly out of scope for a docs pass): the palette.sh
copy-error defect noted above.
