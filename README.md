# delta

A spec-driven lifecycle for changing systems that already exist: understand
the code before proposing a change, then prove it worked by running
something, not by asking a model whether it looks right. **explore** reads
the code, **propose** writes a spec plus one check per criterion,
**critique** reviews it independently, **apply** implements it, **verify**
runs the checks, **archive** folds the result into truth — the next change
diffs against what `archive` recorded, not against history. No binary, no
compile step, no Python, no Node — the runner is POSIX `sh`. **Windows needs
Git Bash** (bundled with Git for Windows; unlike WSL it shares the filesystem).

## Install

```sh
delta/bin/install
```

Writes the command files into every supported CLI's personal command
directory once per machine, not once per repo — no network fetch, nothing
else installed. Safe to re-run any time; `rm -rf` those directories to
uninstall. `/delta-explore` then works in **any** repository on this machine.

`delta/bin/verify` is deliberately **not** part of what `install` writes: a
committed file, the only copy that ever exists, copied in from a repo that
already has delta. See [Lazy per-repo data](#lazy-per-repo-data).

## Six commands

| command | what it does |
|---|---|
| `explore <area>` | Read the affected code. Write entry points, call chain, data touched, and what could not be determined. |
| `propose <intent>` | A spec against current truth — ADDED / MODIFIED / REMOVED / RENAMED — plus one check per criterion. Presented as a diff; nothing lands unapproved. Runs `critique` once written. |
| `critique [id]` | A second opinion on a spec, reading only the spec and the constitution — no exploration, no code. Findings, never edits. |
| `apply` | Implement the spec in order, marking progress. |
| `verify` | Run `delta/bin/verify`. No model involvement. |
| `archive` | Fold the applied delta into truth. |

## A check is an executable file

A shebang file that exits 0 or non-zero, bound to a criterion by filename
prefix (`checks/C3-*` serves `C3`):

```sh
#!/bin/sh
# CRITERION: C3 duplicate webhook does not create a second ledger row
curl -sf -X POST localhost:8081/webhook/retry -d @fixtures/dup.json > /dev/null
test "$(psql -tAc "select count(*) from ledger_entry where provider_ref='X1'")" = "1"
```

`chmod +x` it the moment you write it; without it a check reports as
[`error`](#running-verify). `.gitattributes` forces LF on `delta/bin/**`
and `**/checks/**` — a `\r`-terminated shebang isn't one.

## Running verify

```sh
delta/bin/verify [change-id]        # defaults to the most recent change
DELTA_ROOT=/path/to/repo delta/bin/verify [change-id]
```

| exit | meaning | what to do |
|---|---|---|
| **0** | every criterion checked, every check passed, every MANUAL signed off | done |
| **1** | a check failed | fix the code, or the check if it's wrong |
| **2** | a criterion has no check | write one — never a caveat on a pass |
| **3** | a MANUAL criterion has no sign-off | add a line to `run/signoff.md` |
| **4** | could not run — bad change id, no `spec.md`, no criteria, bad `$DELTA_ROOT` | the printed message says which |
| **5** | `--archive-gate` only: a reproduction is still outstanding | fix the bug, don't archive around it |
| **6** | a `fail-until-fixed` check passed without ever failing | the reproduction is wrong — fix it |
| **7** | a check could not run at all (not executable, bad interpreter) | never a criterion failure — fix the check |

Precedence when several apply: `7 > 1 > 6 > 2 > 3 > 5` (see the header in
`delta/bin/verify`). Every run writes `delta/changes/<id>/run/<utc-timestamp>/`:
a log per criterion, `results.tsv`, `meta.txt`, `summary.txt`.

## MANUAL criteria

Real, unautomatable criteria are marked `MANUAL` — never auto-passed; a
sign-off line in `run/signoff.md` is required:

```
C4 signed-off-by: alex 2026-08-26 - read all three, each names the next action
```

Never write a weak proxy check so it technically executes — that turns an
open question into a false green.

## Bug fixes: reproduction-first checks

The first criterion of a bug fix is the reproduction: `# EXPECT: fail-until-fixed`,
expected to fail before the fix lands:

- `reproduced` (exit 0) — fails as expected, bug confirmed, fix not written
- `fixed` (exit 0) — passes, having reproduced earlier; flips permanently
- `suspicious` (exit 6) — passes without ever having reproduced; the repro is wrong

The flip is recorded in `run/reproductions.md`, tracked in git. `archive`'s
`--archive-gate` exits 5 while a reproduction is outstanding. No `/delta:bug`
command — one extra check state, same lifecycle.

## A second pair of eyes

`critique` reads only the spec and the constitution — never the exploration,
the intent, or the code, so it has to ask whether the spec stands on its own
rather than agreeing with reasoning it never saw. Looks for unmeasurable
criteria, a check that would pass while the feature is broken, missing
failure modes, conflicts, and MANUAL criteria that could be automated —
findings only, to `critique.md`, never edits, honest when it finds nothing.
Where an adapter declares `roles`, it runs in an isolated subagent; where
not, sequentially with an instruction to disregard prior context. Nothing
blocks on findings; they're recorded.

## Layout

Per repository, created lazily by `propose`. `bin/verify` and `bin/palette.sh`
are real, committed files, never generated:

```
delta/
  constitution.md                 hand-written, inherited by every change
  bin/verify                      committed - the only copy that exists
  bin/palette.sh                  colours/glyphs verify and report share
  bin/stage-rail                  optional - the lifecycle rail
  bin/report                      optional - writes delta/report.html
  truth/                          current understanding; archive writes here
  changes/<id>/explore.md
  changes/<id>/spec.md
  changes/<id>/checks/
  changes/<id>/run/
```

Per machine, written once by `delta/bin/install`:

```
~/.claude/commands/delta-*.md    (and the equivalent per adapters.yaml entry)
delta/adapters.yaml               this checkout - the CLI format table
delta/bin/install                 writes the above - commands only
delta/bin/generate-commands       emits per-CLI files (--target project|user)
delta/commands/                   canonical command source, one file each
```

## Lazy per-repo data

No `init` command. The first time `propose` runs in a repository — root found
by walking up for `delta/`, then `.git`, honouring `$DELTA_ROOT` — it creates
`delta/{truth,changes,bin}` and writes `delta/constitution.md` from the
template. It does not create `delta/bin/verify`: no global copy to pull
from, so it says to copy `delta/bin/verify` **and** `delta/bin/palette.sh` in
(`stage-rail` is optional — verify runs fine without it, just no rail). A
teammate cloning a repo with `delta/` already needs none of this.

## The constitution

One hand-written file, under 60 lines, non-negotiables only, written verbatim
by `propose`'s template the first time it runs — no generator introspects
the codebase. Replace it before the first real change lands.

## Working with any CLI

`delta/commands/` holds one canonical file per command; `delta/adapters.yaml`
describes each target's format and emits via `delta/bin/generate-commands`.
Adding a CLI is a table entry, never a code change.

| tool | directory | format | invoked as |
|---|---|---|---|
| Claude Code | `.claude/commands/` | markdown + front matter | `/delta-explore` |
| Gemini CLI | `.gemini/commands/delta/` | TOML | `/delta:explore` |
| Antigravity CLI | `.agents/skills/` | markdown + front matter | `/delta-explore` |
| Codex | `.codex/prompts/` | markdown + front matter | `/delta-explore` |

[`AGENTS.md`](AGENTS.md) is the durable, vendor-neutral target these layer over.

## Terminal presentation and the telemetry report

`verify` prints its own frame, streams results, degrades honestly (no colour
off a TTY, ASCII off UTF-8), and opens with `delta/bin/stage-rail`'s
lifecycle position — a failed stage stays red until resolved. Colours/glyphs
live in `delta/bin/palette.sh`. `explore` draws the call chain as a diagram
plus a Mermaid file. `verify --all` is a read-only dashboard: every
criterion's last eight runs as a sparkline, no checks executed.
`delta/bin/report` writes `delta/report.html` — usage, failure rate,
testability, recurring failures as a heatmap — aggregate only, no
per-developer data, no server, no JS.

## Verifying delta itself

`delta/changes/example-verify-exit-codes/` is both the worked example of the
spec format and delta's own test suite:

```sh
delta/bin/verify example-verify-exit-codes
```

## Deliberately not built

A live workflow UI. Also deferred: a code graph, an MCP server, agent
personas (PM/architect/reviewer role-play — unrelated to `adapters.yaml`'s
`roles`), sixel/image graphics, networking.
