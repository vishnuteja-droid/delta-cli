# delta

A spec-driven lifecycle for changing systems that already exist: understand
the code before proposing a change, then prove it worked by running
something, not by asking a model whether it looks right. Five commands:
**explore** reads the code, **propose** writes a spec plus one check per
acceptance criterion, **apply** implements it, **verify** runs the checks,
**archive** folds the result into truth — the next change diffs against what
`archive` recorded, not against history.

No binary, no compile step, no Python, no Node, no package manager — the
runner is POSIX `sh`. **Windows needs Git Bash** (bundled with Git for
Windows, and unlike WSL it shares the Windows filesystem).

## Install

```sh
delta/bin/install
```

Writes the five command files into every supported CLI's personal command
directory (`~/.claude/commands/`, `~/.gemini/commands/`, …) once per machine,
not once per repo. Copies files already in this checkout; no network fetch,
nothing else installed. Safe to re-run any time; `rm -rf` those directories to
uninstall. From then on, `/delta-explore` works in **any** repository on this
machine, including one that has never seen delta.

`delta/bin/verify` is deliberately **not** part of what `install` writes: it
is committed inside each repository — the only copy that ever exists, nothing
tracks a version of it. A repo gets it the way it gets any other file: copied
in from a repo that already has delta. See [Lazy per-repo
data](#lazy-per-repo-data).

## Five commands

| command | what it does |
|---|---|
| `explore <area>` | Read the affected code. Write entry points, call chain, data touched, and what could not be determined. |
| `propose <intent>` | A spec against current truth — ADDED / MODIFIED / REMOVED / RENAMED — plus one check per criterion. Presented as a diff; nothing lands unapproved. |
| `apply` | Implement the spec in order, marking progress. |
| `verify` | Run `delta/bin/verify`. No model involvement. |
| `archive` | Fold the applied delta into truth. |

## A check is an executable file

Not a description, not a YAML entry — a shebang file that exits 0 or non-zero,
bound to a criterion by filename prefix (`checks/C3-*` serves `C3`):

```sh
#!/bin/sh
# CRITERION: C3 duplicate webhook does not create a second ledger row
curl -sf -X POST localhost:8081/webhook/retry -d @fixtures/dup.json > /dev/null
test "$(psql -tAc "select count(*) from ledger_entry where provider_ref='X1'")" = "1"
```

Most checks are thin wrappers over existing tests — that's correct. `chmod +x`
it the moment you write it (write-a-file tools leave the bit off); without it
a check reports as [`error`](#running-verify). `.gitattributes` forces LF on
`delta/bin/**` and `**/checks/**` — a `\r`-terminated shebang isn't one.

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

When several apply, precedence is `7 > 1 > 6 > 2 > 3 > 5` (see the header
comment in `delta/bin/verify`). Every run writes
`delta/changes/<id>/run/<utc-timestamp>/`: a log per criterion, `results.tsv`,
`meta.txt`, `summary.txt`.

## MANUAL criteria

Real, unautomatable criteria are marked `MANUAL` with a reason. `verify`
never auto-passes one; it needs a sign-off line in `run/signoff.md`:

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

The flip is recorded in `delta/changes/<id>/run/reproductions.md`, tracked in
git so it survives a clone. `archive`'s `--archive-gate` exits 5 while a
reproduction is outstanding, so archiving can't record a bug as fixed that
isn't. No `/delta:bug` command — same lifecycle, one extra check state.

## Layout

Per repository, created lazily by `propose`. `bin/verify` and `bin/palette.sh`
are real, committed files, never generated:

```
delta/
  constitution.md                 hand-written, inherited by every change
  bin/verify                      committed - the only copy that exists
  bin/palette.sh                  colours/glyphs verify and report share
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
from, so it says to copy `delta/bin/verify` **and** `delta/bin/palette.sh`
in — verify sources palette.sh unconditionally, so one without the other
fails to start. A teammate cloning a repo with `delta/` already needs none
of this: the runner is a committed file, nothing to compare a version against.

## The constitution

One hand-written file, under 60 lines, non-negotiables only, inherited by
every change. `propose` writes the template verbatim the first time it runs
in a repo — no generator introspects the codebase to fill it in. Replace it
with your own rules before the first real change lands.

## Working with any CLI

`delta/commands/` holds one canonical file per command; `delta/adapters.yaml`
describes each target's format; `delta/bin/generate-commands` emits the
per-tool files (`--check` fails CI on stale output). Adding a CLI is a table
entry, never a code change.

| tool | directory | format | invoked as |
|---|---|---|---|
| Claude Code | `.claude/commands/` | markdown + front matter | `/delta-explore` |
| Gemini CLI | `.gemini/commands/delta/` | TOML | `/delta:explore` |
| Antigravity CLI | `.agents/skills/` | markdown + front matter | `/delta-explore` |
| Codex | `.codex/prompts/` | markdown + front matter | `/delta-explore` |

[`AGENTS.md`](AGENTS.md) is the durable, vendor-neutral target the per-CLI
files layer over. `gh`/`glab` are ordinary check-callable programs; delta
builds no forge integration.

## Terminal presentation and the telemetry report

`verify` prints its own frame and streams results as each check finishes,
degrading honestly: no colour off a TTY or with `NO_COLOR`, ASCII glyphs off
UTF-8, truncation instead of wrapping on a narrow terminal. Colours/glyphs
live in `delta/bin/palette.sh` (see its header), shared with `delta/bin/report`.

`delta/bin/report` reads `delta/changes/*/run/` and writes a self-contained
`delta/report.html`: usage, failure rate, testability, recurring failures,
aggregate only, no per-developer data, no server, no JavaScript, gitignored
like the `run/` data it's generated from.

## Verifying delta itself

`delta/changes/example-verify-exit-codes/` is both the worked example of the
spec format and delta's own test suite:

```sh
delta/bin/verify example-verify-exit-codes
```

## Deliberately not built

A live workflow UI — a static page can't carry buttons or stream a run, and
every fix costs a dependency delta otherwise has none of. Also deferred: a
code graph, an MCP server, multi-agent roles, any networked component.
