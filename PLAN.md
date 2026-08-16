# delta — Sequential Build Prompts

Rust, single static binary, single build path. Run these in order in the same repo.
Each prompt assumes the previous is complete and `cargo clippy -- -D warnings` is clean.

**Positioning after research:** the differentiator is not the terminal, the speed, or the stages — every SDD tool has stages. It is **enforcement and verification**, where the entire existing field scores zero. Prompt 4 is the product. Everything before it is table stakes.

**Name:** `dlt` — the primary load-bearing member. Verify availability on crates.io and GitHub before prompt 0; renaming a crate after prompt 3 is painful.

## Session discipline

**Put this file in the repo as `PLAN.md` before prompt 0.** Every session after that is one line:

> Read PLAN.md, AGENTS.md, PROGRESS.md. Run `cargo check` and `cargo test` and report current state. Then implement prompt N. Update PROGRESS.md when done.

The agent pulls its own instructions from the file. Nothing to paste after the first time.

**Batching** — this cannot fit one session; compaction mid-build is where agents lose the plot. Six sessions:

| Session | Prompts |
|---|---|
| 1 | 0 + 1 — skeleton, workspace |
| 2 | 2 — provider (needs the full window) |
| 3 | 3 — stage machine |
| 4 | 4 — verification |
| 5 | 5 — tool loop |
| 6 | 6 + 7 — TUI, ship |

**Rules**

- Prompt 0 must produce an `AGENTS.md` at the repo root: stack, commands, conventions, the `unwrap` ban, and the line **"Never implement beyond the current prompt in PLAN.md."** Agents that can see the whole plan run ahead and half-build prompt 5 before prompt 3 works.
- End every session by updating `PROGRESS.md`: what shipped, what is stubbed, what the next prompt should assume.
- Branch per prompt. A bad session becomes one `git reset`.
- Do not advance until the current prompt's tests pass. These prompts are load-bearing on each other.
- Reject `todo!()` scaffolding of future modules after prompt 0. Code that compiles and does nothing reads as progress and isn't.

**Where you must be present:** prompt 0's module boundaries, prompt 2's interrupt behaviour, prompt 5's patch application. The rest can run unattended and be reviewed as a diff.

---

## Design decisions carried into every prompt

| Decision | Reason |
|---|---|
| Two-space model: `truth/` vs `changes/` | brownfield support; OpenSpec's model, proven better than linear sessions |
| Adaptive rigor — stage set chosen by change size | rigid gates are the #1 documented criticism of Spec Kit and the failure AWS names first |
| Verification is executable, not advisory | the field's universal gap |
| Read `openspec/`, `.kiro/`, `specs/` if present | interop beats a fourth format |
| `AGENTS.md` respected as context | de facto standard |
| No C dependencies — rustls, not openssl | static musl builds, zero setup |

---

## Dependencies by prompt

Add crates only when the prompt that needs them arrives. Do not pull `tokio` or `ratatui` in on day one.

| Prompt | Crates added |
|---|---|
| 0 — Skeleton | `clap` (derive), `serde`, `serde_json`, `serde_yaml`, `anyhow`, `thiserror`, `chrono`, `directories`, `sha2` |
| 1 — Workspace | `walkdir`, `minijinja`; dev: `insta`, `assert_cmd`, `tempfile` |
| 2 — Provider | `tokio` (rt-multi-thread, macros), `reqwest` (json, stream, rustls-tls, **no default-features**), `eventsource-stream`, `futures`, `backoff`; dev: `wiremock` |
| 3 — Stage machine | `tiktoken-rs` or equivalent for token counting |
| 4 — Verification | `globset`, `regex`, `duct`, `notify` |
| 5 — Tool loop | `similar`, `ignore`, `grep-searcher` |
| 6 — TUI | `ratatui`, `crossterm`, `unicode-width`, `textwrap`, `tui-textarea` |
| 7 — Ship | none; `cross` or a CI matrix |

**Hard constraints:** rustls not openssl. No C dependencies anywhere — they break static musl builds and kill the zero-setup promise. If a crate pulls in a `-sys` dependency, find another crate.

**Release profile** (prompt 7):

```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

---

## Prompt 0 — Skeleton

Create a new Rust binary crate `dlt`: a terminal-first AI development lifecycle tool.

Set up the workspace skeleton only — no feature logic yet.

- Edition 2024, binary target `dlt`
- Dependencies: `clap` (derive), `serde`/`serde_json`/`serde_yaml`, `anyhow`, `thiserror`, `chrono`, `directories`, `sha2`
- Module layout: `cli`, `config`, `workspace`, `change`, `stage`, `provider`, `verify`, `tui`, `error` — stub each with a doc comment stating its responsibility
- `error.rs`: `thiserror` enums per module; `anyhow` used only in `main.rs`
- `config.rs`: load layered config — built-in defaults, then `~/.config/delta/config.toml`, then `.delta/config.toml` in the repo, then env vars. Later layers override earlier.
- `cargo clippy -- -D warnings` clean, `cargo test` passing with a placeholder test

Ban `unwrap()` and `expect()` outside `#[cfg(test)]` via a clippy lint in `Cargo.toml`.

Show me the module responsibilities before writing code.

---

## Prompt 1 — Workspace, changes, artifacts

Implement the on-disk model.

**Two spaces.** `.delta/truth/` holds the current agreed behaviour of the system, one markdown file per capability. `.delta/changes/<slug>/` holds an in-flight change: `proposal.md`, `design.md`, `tasks.md`, and `deltas/` containing proposed edits to truth files. A change is archived by applying its deltas to truth and moving the folder to `.delta/archive/`.

**Artifacts** are markdown with YAML frontmatter: `stage`, `created`, `updated`, `source_hash`, `status`. `source_hash` is SHA-256 over the concatenated bodies of the artifact's declared inputs — when an input changes, dependents become `stale`, never silently valid.

**Interop.** On `init`, detect an existing `openspec/`, `.kiro/specs/`, or `specs/` directory and print a note that it was found and import is not yet supported. Do not build importers — there are no users to migrate yet, and those layouts churn. Keep file access behind a small trait so an adapter can be added later without touching call sites.

**Commands:** `init`, `change new <slug>`, `change list`, `status`, `archive <slug>`.

`status` prints a table: change slug, stage, state (pending/valid/stale/failed), age.

Exit codes, load-bearing for CI: `0` ok, `1` internal, `2` validation failed, `3` gate not satisfied, `4` stale inputs.

Integration tests with `tempfile` + `assert_cmd` covering init → new → status, and the stale-detection path.

---

## Prompt 2 — Provider layer

Add configurable LLM providers. Streaming, async, no blocking of the caller.

- Deps: `tokio` (rt-multi-thread, macros), `reqwest` (json, stream, rustls-tls, **no default-features**), `eventsource-stream`, `futures`
- `rustls` with `webpki-roots` — certificates compiled in. Verify this works from a static musl build with no system cert store. Do not defer this.
- `trait Provider`: `async fn stream(&self, req: Request) -> Result<impl Stream<Item = Delta>>`, plus `name()`, `context_window()`, `count_tokens()`
- Implementations: `OpenAiCompatible` (covers OpenAI, most local servers, most gateways) and `Anthropic`. Both configured by `base_url`, `model`, `api_key_env`, `headers`.
- Providers are declared in config, not hardcoded:

```toml
[providers.default]
kind = "openai_compatible"
base_url = "https://api.example.com/v1"
model = "..."
api_key_env = "AIDLC_API_KEY"

[providers.local]
kind = "openai_compatible"
base_url = "http://localhost:11434/v1"
model = "..."
```

- Retry with exponential backoff and jitter on 429 and 5xx; surface rate-limit headers
- Cancellation: every stream must be abortable mid-flight via `CancellationToken`
- SSE framing is line-based — buffer to `\n\n`, strip the field prefix, then parse. Assume frames split across chunk boundaries and test that explicitly with a mock server.

Unit tests against a mock HTTP server. No live API calls in the test suite.

---

## Prompt 3 — Stage machine with adaptive rigor

Stage definitions are runtime-loaded YAML from `stages/` — adding a stage must never require a recompile.

```yaml
id: design
name: Technical Design
inputs: [proposal]
min_rigor: standard        # trivial | standard | deep
template: |
  {{ agents_md }}
  ## Current truth
  {{ truth.relevant }}
  ## Proposal
  {{ inputs.proposal.body }}
  Produce a technical design covering interfaces, data, and risks.
output:
  required_sections: [Interfaces, Data, Risks, Alternatives Considered]
  validators:
    - non_empty_sections
    - no_placeholder_text
    - min_words: 200
```

**Adaptive rigor.** On `change new`, classify the change as trivial / standard / deep from files touched, whether it alters public interfaces, and user override via `--rigor`. Stages whose `min_rigor` exceeds the classification are skipped and marked `n/a` — not failed. A one-line typo fix must not require a design document. Always allow `--rigor deep` to force the full path.

Context assembly for each stage: `AGENTS.md` if present, relevant truth files, declared input artifacts, repo tree summary. Enforce a token budget from `provider.context_window()`; if over, drop lowest-priority context first and report what was dropped.

`dlt run <stage>` calls the provider, streams to stdout, validates the result, writes the artifact. `--dry-run` prints the assembled prompt without calling anything — keep this, it is how you debug context assembly forever.

Snapshot tests (`insta`) on prompt assembly.

---

## Prompt 4 — Verification engine

**This is the product.** Every competing tool generates specs and hands off; none checks whether the code satisfies them.

Acceptance criteria in a spec may declare executable checks:

```markdown
## Acceptance Criteria
- [ ] Rejects tokens older than 24h
      `verify: cmd "cargo test auth::expiry" expect exit 0`
- [ ] Endpoint documented
      `verify: file "docs/api.md" contains "POST /auth/refresh"`
- [ ] No new public API without a doc comment
      `verify: cmd "cargo doc 2>&1" not_contains "missing documentation"`
```

Implement check kinds: `cmd` (exit code, stdout contains/not_contains), `file` (exists, contains, matches regex), `git` (files changed within a glob).

- `dlt verify [change]` runs all checks for a change and reports pass/fail per criterion with the failing output
- Exit code 2 on any failure — makes it a CI gate
- Timeout per check, configurable, default 120s
- `dlt verify --watch` reruns on file change
- A change cannot be archived with failing checks unless `--force`, which is recorded in the archived frontmatter

Deps: `duct` or `std::process`, `globset`, `regex`, `notify` for watch mode.

---

## Prompt 5 — Tool loop

Give the agent the ability to act, with gates.

- Tools: `read_file`, `write_file`, `apply_patch`, `list_dir`, `search` (via `ignore` + `grep-searcher`), `run_command`
- `apply_patch` uses `similar` for unified-diff parsing with fuzzy context matching. This is the hardest correctness problem in the build — test it against reordered context, trailing whitespace, CRLF, and overlapping hunks.
- Approval gates: config sets each tool to `auto` / `prompt` / `deny`. Writes and commands default to `prompt`. Print the diff or the exact command before asking.
- Every write is journalled to `.delta/journal/` so `dlt undo` reverts the last agent action.
- `run_command` respects an allowlist and never runs with a shell by default.
- Tool results feed back into the loop with a hard iteration cap and a token budget; on breach, stop and report rather than truncating silently.

---

## Prompt 6 — TUI

Deps: `ratatui`, `crossterm`, `unicode-width`, `textwrap`, `tui-textarea`.

**Threading rule, non-negotiable:** the render loop owns the terminal and ticks on a fixed interval independent of network state. Provider deltas arrive on an `mpsc` channel. Never draw from the network task. If these couple, the UI stutters exactly when the model is slow — which is when the user is watching.

Layout: header with change slug + stage + rigor; main transcript pane; collapsible dim-italic reasoning blocks; sticky footer with elapsed time, token count, and `esc to interrupt`.

Character: a small ASCII/braille sprite with three states — idle, working, done. Cycle frames at 10fps, dirty-check before redraw.

Status vocabulary: sampled word list, rotating every ~4s, **keyed to the current stage** — interrogating/clarifying during proposal, reconciling/weighing during design, patching/regressing during build. Stage-aware status is something no competitor can do, because none of them own both the lifecycle and the renderer. Write an original word list.

Colour: two accent colours, `Color::Rgb` with a 256-colour fallback. Assume some corporate terminals lack truecolor.

Keys: `esc` interrupt, `ctrl-c` twice to quit, `tab` to cycle panes, `/` for commands, `?` for help.

---

## Prompt 7 — Ship

- Cross-compile targets: `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-gnu`
- Verify the Windows build under both ConPTY and legacy console
- Strip + `opt-level = "z"` + LTO; confirm the musl binary runs on a container with no cert store and no glibc
- GitHub Actions matrix build, release artifacts on tag
- `dlt --version` reports commit hash and build target
- README: quickstart, artifact format, authoring a custom stage, writing verification checks
- `dlt doctor` — checks config, provider reachability, terminal capabilities, git presence

---

## Order discipline

Do not start prompt 6 until prompts 1–5 have been used on a real change in a real repository. The TUI is the most satisfying part to build and the least important to whether anyone adopts this.

---

## Naming note (recorded at prompt 0)

`dlt` was unavailable on crates.io (an existing computer-vision crate) and `delta` was also
taken. The binary target is still named `dlt` (binary names don't need to be globally unique
on crates.io), but the *package* is named `delta-cli` to get a free crates.io slot. If this
project is ever published, revisit the package name before release.
