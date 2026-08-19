# dlt

Terminal-first, adaptive-rigor spec-driven development. `dlt` keeps a durable
model of *what's true* about your codebase (`.delta/truth/`) separate from
*what's proposed* (`.delta/changes/<slug>/`), drives an LLM through
proposal → design → tasks → build, and gates every change on **executable**
acceptance criteria instead of a human's read-through. Single static binary,
no C dependencies, works against any OpenAI-compatible, Anthropic, or Gemini
endpoint.

## Quickstart

```sh
# 1. Build (or grab a release binary — see "Installing" below)
cargo build --release
cp target/release/dlt /usr/local/bin/

# 2. Initialize a workspace in your repo
cd your-project
dlt init

# 3. Point dlt at a provider
mkdir -p .delta
cat >> .delta/config.toml <<'EOF'
[providers.default]
kind = "anthropic"
base_url = "https://api.anthropic.com"
model = "claude-sonnet-5"
api_key_env = "ANTHROPIC_API_KEY"
EOF
export ANTHROPIC_API_KEY=sk-...

# 4. Confirm everything is reachable
dlt doctor

# 5. Start a change
dlt change new dark-mode --description "Add a dark mode toggle to settings"
dlt run proposal --change dark-mode      # drafts .delta/changes/dark-mode/proposal.md
dlt run design    --change dark-mode      # skipped ("n/a") automatically if rigor is trivial
dlt run tasks      --change dark-mode

# 6. Let the agent implement it, gated by your approval on every write/command
dlt build dark-mode

# 7. Check its own acceptance criteria, then fold it into truth
dlt verify dark-mode
dlt archive dark-mode
```

Every `dlt run`/`dlt build` also has a `dlt tui` counterpart
(`dlt tui run proposal --change dark-mode`, `dlt tui build dark-mode`) that
runs the same thing behind a live status/transcript view instead of raw
stdout — same provider calls, same approval gate, same journal.

Add `--dry-run` to `dlt run` at any point to print the assembled prompt
without calling a provider — this works with **no API key set** and is how
you debug context assembly (what's in the prompt, what got dropped for
budget) before spending a token on it.

## Installing

**Prebuilt binaries**: see the repo's [Releases](../../releases) page —
`x86_64`/`aarch64` Linux (musl, fully static), `x86_64`/`aarch64` macOS, and
`x86_64` Windows.

**From source**: `cargo build --release` produces `target/release/dlt`
(`.exe` on Windows). Requires only a stable Rust toolchain — no system
libraries, no C toolchain, no OpenSSL.

`dlt --version` reports the crate version, commit hash, and build target
(`dlt 0.1.0 (a1b2c3d, x86_64-unknown-linux-musl)`), so a bug report always
carries exactly what was built and from where.

## Configuration

Config loads in layers, each overriding the previous: built-in defaults →
`~/.config/delta/config.toml` → `.delta/config.toml` (repo-rooted) →
environment variables (`DELTA_FOO__BAR=x` overrides the dotted key
`foo.bar`; double underscore separates nesting since env var names can't
contain dots).

Providers live under `[providers.<name>]`. `dlt run`/`dlt build`/`dlt tui`
all take `--provider NAME` and default to `"default"`.

```toml
[providers.default]
kind = "anthropic"                          # "anthropic" | "openai_compatible" | "gemini"
base_url = "https://api.anthropic.com"
model = "claude-sonnet-5"
api_key_env = "ANTHROPIC_API_KEY"            # name of the env var holding the key — never the key itself
context_window = 200000                      # optional, defaults to 128000
[providers.default.headers]                  # optional, extra request headers
# "anthropic-beta" = "..."

[providers.local]
kind = "openai_compatible"                    # most local servers (Ollama, vLLM, LM Studio, ...) speak this
base_url = "http://localhost:11434/v1"
model = "qwen2.5-coder:32b"
api_key_env = "LOCAL_KEY"                     # set to anything if the server doesn't check it

[providers.gemini]
kind = "gemini"
base_url = "https://generativelanguage.googleapis.com/v1beta"
model = "gemini-2.5-pro"
api_key_env = "GEMINI_API_KEY"
```

`dlt doctor` reads this section and TCP-connects to each configured
provider's `base_url` to report reachability, without spending a token or
requiring the key to actually be valid.

Per-tool approval policy for `dlt build`/`dlt tui build` lives under
`[tools.<name>]`:

```toml
[tools.write_file]
policy = "prompt"   # "auto" | "prompt" | "deny" — read-only tools default to auto, writes/commands default to prompt

[tools.run_command]
policy = "prompt"
allowlist = ["cargo", "npm", "git"]   # checked before the approval prompt; an unlisted program is never even offered
```

## The artifact format

`.delta/` holds two spaces:

- **`.delta/truth/`** — the durable, agreed state of the project. Plain
  markdown files, read directly and concatenated into every prompt.
- **`.delta/changes/<slug>/`** — one in-flight change: `proposal.md`,
  `design.md`, `tasks.md` (whichever stages have run), plus `deltas/` for
  what actually gets folded into truth on archive.

Every artifact is markdown with YAML frontmatter:

```markdown
---
stage: proposal
created: 2026-08-19T11:40:48.597056979Z
updated: 2026-08-19T11:40:48.597056979Z
source_hash: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b8
status: pending          # pending | valid | stale | failed | n/a
rigor: standard           # trivial | standard | deep
---
# Proposal
...
```

`source_hash` is a SHA-256 over the artifact's declared inputs' *current*
bodies. `dlt status`/`dlt archive` recompute it on the fly — if an upstream
artifact (e.g. `proposal.md`) changes after `design.md` was generated from
it, `design.md`'s status flips to `stale` regardless of what its own
frontmatter claims, and `dlt archive` refuses until it's regenerated (or
you pass `--force`, which stamps `verify_forced: true` onto the archived
frontmatter as an audit trail — never silent).

`status: n/a` means the stage was deliberately skipped because the change's
rigor didn't require it (e.g. a `trivial` change skips `design`/`tasks`) —
this is treated as permanently non-stale, not as a missing artifact.

## Authoring a custom stage

Stages are runtime-loaded YAML in `.delta/stages/` (seeded from this repo's
own `stages/*.yaml` on `dlt init`; edit or add files there directly — no
recompile needed). A stage looks like:

```yaml
id: security-review               # unique id; referenced by other stages' `inputs` and by `dlt run <id>`
name: Security Review
inputs: [design]                  # other stage ids this one depends on; must exist and be acyclic
min_rigor: deep                   # trivial | standard | deep — changes below this rigor get an `n/a` artifact instead
template: |
  {{ agents_md }}
  ## Design
  {{ inputs.design.body }}
  ## Existing draft, if any (refine it rather than restarting from scratch)
  {{ current }}
  Review the design above for security issues: authn/authz gaps, injection,
  secrets handling. Write up findings under the required sections below.
output:
  required_sections: [Findings, Mitigations]
  validators:
    - non_empty_sections      # every required section has content
    - no_placeholder_text     # rejects TODO/TBD/Lorem ipsum/XXX/FIXME
    - min_words: 100
```

Template variables available (MiniJinja, `{{ ... }}`):

- `agents_md` — this repo's `AGENTS.md`, verbatim
- `truth.relevant` — `.delta/truth/*.md`, concatenated under per-file headings
- `repo_tree` — a summary of the repo's file tree
- `inputs.<stage_id>.body` — the body of each declared input's current artifact
- `current` — this stage's *own* existing artifact body, if any (what the
  user typed at `change new --description`, or a previous run's output) —
  always available so a hand-edited draft or explicit user intent is never
  silently discarded on rerun

If the assembled prompt would exceed the provider's context window, `dlt
run` drops (in order) `repo_tree`, then `truth.relevant`, then `agents_md`
before re-rendering — declared `inputs` and `current` are never dropped —
and reports what it dropped to stderr.

The stage graph must be a DAG with exactly one root (a stage with no
`inputs`); `dlt run`/`dlt change new` will report `InvalidGraph` rather
than silently picking an order if it isn't.

## Writing verification checks

Any artifact (usually `tasks.md`) can carry a `## Acceptance Criteria`
section. A checklist item becomes an **executable** criterion when
immediately followed by an indented inline code span `` `verify: <check>` ``
— plain items with no such annotation are left for a human and aren't run.

```markdown
## Acceptance Criteria
- [ ] Rejects tokens older than 24h
      `verify: cmd "cargo test auth::expiry" expect exit 0`
- [ ] Endpoint documented
      `verify: file "docs/api.md" contains "POST /auth/refresh"`
- [ ] No new public API without a doc comment
      `verify: cmd "cargo doc 2>&1" not_contains "missing documentation"`
```

Three check kinds:

```text
cmd "<command>" [expect exit <n>] [contains "<text>"] [not_contains "<text>"]
file "<path>" (exists | contains "<text>" | matches "<regex>")
git changed "<glob>"
```

- `cmd` runs through a real shell (`sh -c` on Unix, `cmd /C` on Windows), so
  redirections like `cargo doc 2>&1` work as written. This is the same
  trust boundary as a Makefile or CI config — it only ever runs commands
  the repo's own authors wrote into their own spec file.
- A `cmd` check needs at least one of `expect exit`/`contains`/
  `not_contains` — one that asserts nothing would catch nothing.
- A malformed `verify:` spec still becomes a criterion — a **failing** one,
  with the parse error as its detail — so a typo in a check is visible
  instead of silently skipped.

`dlt verify [slug] [--watch] [--timeout SECS]` runs every criterion across
a change's artifacts (or every in-flight change, if `slug` is omitted) and
exits non-zero if anything failed — wire it into CI directly. `--watch`
reruns on file changes with a 300ms debounce, for a tight edit/check loop.
`dlt archive` runs the same gate before folding a change into truth.

## Command reference

| Command | What it does |
|---|---|
| `dlt init` | Create `.delta/` in the current repo |
| `dlt change new <slug> [--rigor R] [--description "..."]` | Start a change; rigor is inferred from `git diff` if not given |
| `dlt change list` / `dlt status` | List in-flight changes / show their stage, state, age |
| `dlt run <stage> --change <slug> [--dry-run] [--provider N] [--rigor R]` | Assemble context, call the provider, validate, write the artifact |
| `dlt build <slug> [--provider N] [--max-iterations N]` | Run the gated tool loop (read/write/patch/search/run commands) to implement a change |
| `dlt undo` | Revert the most recent tool-loop write |
| `dlt verify [slug] [--watch] [--timeout SECS]` | Run acceptance-criteria checks |
| `dlt archive <slug> [--force]` | Verify, apply deltas to truth, move the change to `.delta/archive/` |
| `dlt tui run <stage> --change <slug> [...]` / `dlt tui build <slug> [...]` | Same as `run`/`build`, through the interactive TUI |
| `dlt doctor` | Check config, provider reachability, terminal capabilities, git presence |
| `dlt --version` | Crate version, commit hash, build target |

Exit codes: `0` success · `1` internal (I/O, network) · `2` validation
failed · `3` a gate wasn't satisfied (rigor, output validation, iteration
cap, token budget, `dlt verify`) · `4` stale inputs blocked an archive.

## Development

See `AGENTS.md` for module boundaries and conventions, and `PLAN.md`/
`PROGRESS.md` for how this project was built and what each session shipped.

```sh
cargo check          # fast compile check
cargo test            # full test suite
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
