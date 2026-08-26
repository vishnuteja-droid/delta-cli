# delta

A spec-driven lifecycle for changing systems that already exist.

Two things it adds to what a coding agent does by default: understanding the
code *before* proposing a change to it, and proving the change worked by
*running something* rather than asking a model whether it looks right.

Neither is novel on its own — codebase-onboarding skills exist, spec lifecycles
exist. The loop is what doesn't: explore feeds propose, propose compiles
checks, verify runs them, archive folds the result back into truth, and the
next change diffs against reality instead of history.

## Nothing is installed

No binary, no compile step, no Python, no Node, no package manager. delta is
files that ship with the repository plus one committed POSIX `sh` script.

**Windows needs WSL or Git Bash.** The runner is `sh`, not PowerShell.

## The five commands

| command | what it does |
|---|---|
| `explore <area>` | Read the affected code. Write entry points, call chain, data touched, and explicitly what could not be determined. |
| `propose <intent>` | A delta spec against current truth — ADDED / MODIFIED / REMOVED / RENAMED — plus one executable check per acceptance criterion. Presented as a diff; nothing lands unapproved. |
| `apply` | Implement the spec in order, marking progress. |
| `verify` | Run `delta/bin/verify`. No model involvement at all. |
| `archive` | Fold the applied delta into truth. |

How you invoke them depends on your CLI — `/delta-explore` in Claude Code,
`/delta:explore` in Gemini CLI. See [Working with any CLI](#working-with-any-cli).

## A check is an executable file

Not a description, not a prompt, not a YAML entry. A file with a shebang that
exits 0 or non-zero:

```sh
#!/bin/sh
# CRITERION: C3 duplicate webhook does not create a second ledger row
curl -sf -X POST localhost:8081/webhook/retry -d @fixtures/dup.json > /dev/null
test "$(psql -tAc "select count(*) from ledger_entry where provider_ref='X1'")" = "1"
```

The runner needs no knowledge of check types, because every check is just a
program: curl, psql, mvn, npm, jq, `gh` — whatever is already on the machine.

A check is bound to a criterion by filename prefix: `checks/C3-*` serves
criterion `C3`. An id is an uppercase prefix and a number, so a bug spec can
number its reproductions `B1`, `B2`. Checks run with the working directory at the repository root,
with `$DELTA_CHANGE_DIR`, `$DELTA_CHECK_DIR`, `$DELTA_RUN_DIR`, and
`$DELTA_CRITERION` exported.

### What a check is *for*

Not to catch what the test suite misses — that's the test suite's job. A check
binds a stated criterion to a proof that ran. A check whose entire body is
`mvn test -Dtest=WebhookTest#idempotent` is a good check: the value is the
mapping, and the fact that an agent cannot declare the work done without it.
Expect most checks to be thin wrappers over existing tests. That's correct.

## Running it

```sh
delta/bin/verify [change-id]          # defaults to the most recent change
delta/bin/verify path/to/change/dir   # anything with a slash is a path
```

Exit codes:

| code | meaning |
|---|---|
| **0** | every criterion has a check, every check passed, every MANUAL criterion signed off |
| **1** | at least one check failed |
| **2** | at least one criterion has no corresponding check |
| **3** | at least one MANUAL criterion has no recorded sign-off |
| **4** | could not run: no such change, no spec, or no criteria |
| **5** | `--archive-gate` only: a reproduction is still outstanding |
| **6** | a reproduction did not reproduce |

Codes **2** and **3** are load-bearing. A criterion silently counted as passing
makes the whole tool worthless, so "nothing ran for this" is a distinct,
non-zero outcome — never a caveat attached to a pass.

When several apply, the precedence is 1 > 6 > 2 > 3 > 5.

Every run writes `delta/changes/<id>/run/<utc-timestamp>/` containing a log per
criterion, `results.tsv` (id, status, exit code, start time, duration),
`meta.txt`, and `summary.txt`.

### MANUAL criteria

Some acceptance criteria are real and unautomatable — "error messages are clear
to support staff", "the retry doesn't hammer the downstream". Mark them
`MANUAL` in the spec with a reason and what a human should look at:

```
- C4 MANUAL error messages are readable by support staff
      reason: judgement about wording, with no assertable output
      look at: the three error strings in WebhookController
```

`verify` lists them separately and never counts them as passed. Resolving one
takes a line in `delta/changes/<id>/run/signoff.md`:

```
C4 signed-off-by: alex 2026-08-26 - read all three, each names the next action
```

Never auto-pass a MANUAL criterion, and never write a weak proxy check so it
technically executes — a check asserting an error string is non-empty, standing
in for "the message is clear", turns an open question into a false green.

The count of MANUAL criteria is itself signal about how testable the spec was.

## Bug fixes: reproduction-first checks

A bug fix doesn't begin with knowing what to build. The intent is "make this
stop happening", the cause is unknown when you write the spec, and the most
valuable check is one that **fails before the fix and passes after**.

A check can say so:

```sh
#!/bin/sh
# CRITERION: B1 duplicate webhook creates a second ledger row
# EXPECT: fail-until-fixed
```

The default is `pass`, so a check with no `EXPECT` header behaves exactly as it
always has.

| state | when | exit |
|---|---|---|
| `reproduced` | a `fail-until-fixed` check fails | **0** — nothing is wrong; the bug is confirmed and the fix isn't written yet |
| `fixed` | it passes, having been reproduced earlier | **0** — flips permanently to a normal check |
| `suspicious` | it passes without ever having reproduced | **6** — the repro doesn't reproduce, so the criterion is wrong |

A reproduction is the strongest check delta can hold. Feature criteria are
aspirational; a repro is proof, written before the fix, and once it flips it
stays in `checks/` as a permanent regression guard.

### The flip is recorded

When a reproduction is confirmed and later fixed, `verify` writes to
`delta/changes/<id>/run/reproductions.md` — tracked in git, unlike the per-run
directories, because it has to survive a fresh clone:

```
B1 reproduced: 2026-08-26T14-02-31 - exit 1
B1 fixed: 2026-08-26T16-40-09 - reproduced 2026-08-26T14-02-31
```

That pair of lines is the evidence the fix worked: the check failed before it
and passes after. The flip also rewrites the check's own `EXPECT` line, so the
file records what happened to it.

### Why `suspicious` is a distinct state

A reproduction that passes the first time never captured the bug. Silently
accepting it would let someone "fix" a bug they never reproduced — which is the
failure mode the whole feature exists to prevent. So it isn't a pass, and it
isn't a failure either: it's its own state, with its own exit code.

### Archiving is gated

An outstanding reproduction exits 0 on a normal run — correct, nothing is
wrong. But `archive` runs `delta/bin/verify --archive-gate`, which exits **5**
and names the outstanding criteria, because folding a bug delta into truth
while its reproduction still reproduces would record a bug as fixed that isn't.

There is deliberately **no `/delta:bug` command.** This is the same five-command
lifecycle with one additional check state; a parallel command set for bugs
would duplicate everything and drift.

## Layout

```
delta/
  constitution.md          hand-written, inherited by every change
  bin/verify               committed POSIX sh runner
  bin/generate-commands    emits per-CLI files from the table
  adapters.yaml            CLI format table
  commands/                canonical command source, one file each
  truth/                   current understanding; only archive writes here
  changes/<id>/
    explore.md             findings, including known unknowns
    spec.md                the delta spec
    checks/                executable files, one per criterion
    run/                   results per verification run
```

## The constitution

One hand-written file, under 60 lines, inherited by every change.
Non-negotiables only: layering rules, what must never be touched, error and
logging conventions.

Litmus test per line: *would removing this line cause an agent to make a
mistake it would not otherwise make?* If not, delete it.

There is deliberately **no `init` that generates it by introspecting the
repo.** Auto-generated context files reduce agent success and raise cost;
hand-written ones improve both. `delta/constitution.md` ships as a template
with prompts, not as a generator.

## Working with any CLI

Adapters are data, not code. `delta/commands/` holds one canonical file per
command; `delta/adapters.yaml` describes each target's directory, extension,
and wrapper format; `delta/bin/generate-commands` emits the per-tool files,
which are committed.

```sh
delta/bin/generate-commands           # regenerate all
delta/bin/generate-commands claude    # just one
delta/bin/generate-commands --check   # CI: fail if committed output is stale
```

**Adding a CLI is an entry in `adapters.yaml`. It is never a code change.**
That's the property being built for: this landscape churns, and an adapter
written as a code module rots every time a vendor moves a directory.

Shipped entries, each verified against that tool's current documentation
rather than from memory:

| tool | directory | format | invoked as |
|---|---|---|---|
| Claude Code | `.claude/commands/` | markdown + YAML front matter | `/delta-explore` |
| Gemini CLI | `.gemini/commands/delta/` | TOML | `/delta:explore` |
| Antigravity CLI | `.agents/skills/` | markdown + front matter | `/delta-explore` |
| Codex | `.codex/prompts/` | markdown + front matter | `/delta-explore` |

Two notes worth knowing, both recorded in `adapters.yaml`:

- **Claude Code** files are emitted flat with a `delta-` prefix rather than in
  a `delta/` subdirectory: the docs define a command's name as its file name
  without extension, and subdirectory namespacing has a standing bug report.
- **Codex** loads prompts from `$CODEX_HOME/prompts` only — project-scoped
  `.codex/prompts` is an open feature request, not a shipped feature. The files
  are emitted anyway so they're versioned and reviewable, and so they work
  unchanged if it lands. Today, Codex picks up the lifecycle from `AGENTS.md`.
  To get the slash commands too:
  `ln -s "$PWD/.codex/prompts"/delta-*.md ~/.codex/prompts/`

### AGENTS.md is the durable target

[`AGENTS.md`](AGENTS.md) is hand-written and carries the lifecycle in about
fifteen lines. It's the open standard formalised in August 2025, donated to the
Linux Foundation in December 2025, and supported by 20+ tools — and it isn't
owned by a vendor. The per-CLI command files are a convenience layer over it.

### Not integration targets

`gh` and `glab` are ordinary programs a check can call if a criterion needs
them. delta builds no forge integration.

## Terminal presentation

`verify` prints for itself. The other four are agent output, so their
presentation is instructed rather than executed — which makes the signature
frame both a signature and a compliance check: a closing frame is evidence the
agent ran the command to completion rather than drifting off mid-way.

```
  Δ ─────────────────────────────  delta verify · webhook-idempotency

  ✓ C1  retry returns 200                      0.4s
  ✗ C3  duplicate creates no second row        0.6s
  ○ C4  errors readable by support               manual

  ✗ C3  expected 1 row, got 2
        run/2026-08-26T14-02-31/C3.log

  Δ ──────  4 criteria · 2 passed · 1 failed · 1 manual · 1.5s
```

Glyphs: `✓` passed, `✗` failed, `◆` reproduced, `!` suspicious, `○` manual,
`·` pending, braille spinner while running. Green pass, red fail and suspicious,
dim for pending and manual, default for reproduced — it is neither. Results stream as
each check finishes; failure detail prints above the summary with its log path,
so the summary is what stays on screen.

Degradation is a requirement, not polish:

- **not a TTY** — no colour, no spinner, no carriage-return rewriting, one
  plain line per result. The frame stays; it's the signature. `delta verify |
  tee` and CI logs stay readable.
- **`NO_COLOR` set** — no colour regardless of TTY.
- **non-UTF-8 locale** — ASCII throughout: `d` for `Δ`, `-` for `─`, and
  `[ok]` `[FAIL]` `[rep]` `[!!]` `[man]` `...` for the glyphs. Detected from `LC_ALL`/`LANG`.
- **narrow terminal** — descriptions and detail lines truncate rather than
  wrap, columns stay aligned, and the frame never wraps to a second line.

## Verifying delta itself

`delta/changes/example-verify-exit-codes/` is both the worked example of the
spec format and delta's own test suite: its checks run the runner against
throwaway fixtures and assert each exit code.

```sh
delta/bin/verify example-verify-exit-codes
```

## Deliberately not built

The UI and telemetry over agentic runs — it needs recorded state first, which
is why `verify` writes timestamps, durations, exit codes, captured output, and
criterion ids. Also deferred: a code graph or impact analysis, an MCP server,
multi-agent roles, and any hosted or networked component.
