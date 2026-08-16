# PROGRESS

## Session 1 — Prompt 0 (skeleton)

**Shipped:**

- Cargo package `delta-cli`, binary target `dlt`, edition 2024.
- `[lints.clippy]` in `Cargo.toml` denies `unwrap_used` and `expect_used`; `main.rs` carries
  `#![cfg_attr(test, allow(...))]` so the ban only lifts inside `#[cfg(test)]` code.
- Module skeleton: `cli`, `config`, `workspace`, `change`, `stage`, `provider`, `verify`,
  `tui`, `error`, each with a doc comment stating its responsibility (see `AGENTS.md` for the
  short version, or the module files themselves for the authoritative statement).
- `error.rs`: one `thiserror` enum per module (`ConfigError`, `WorkspaceError`, `ChangeError`,
  `StageError`, `ProviderError`, `VerifyError`, `TuiError`). The five stub-module enums
  (`StageError` through `TuiError`) are `#[allow(dead_code)]` placeholders — a single
  `Unimplemented` variant each — until their prompts land. `anyhow` is used only in `main.rs`.
- `config.rs` is **not** a stub — prompt 0 asked for working layered config loading:
  - Layer order: built-in defaults → `~/.config/delta/config.toml` → `.delta/config.toml`
    (repo-rooted) → environment variables (`DELTA_*`, later layers override earlier).
  - Stored internally as a generic merged `toml::Table` (deep-merged, not shallow-replaced)
    rather than a fixed struct, since later prompts (e.g. prompt 2's `[providers.*]`) will add
    sections this module doesn't need to know the shape of.
  - Env var convention: `DELTA_FOO__BAR=x` → dotted key `foo.bar` (double underscore = nesting
    separator, since TOML keys can't contain dots in env var names).
  - `Config::get` / `Config::get_str` read by dotted path; currently only exercised by tests
    and `#[allow(dead_code)]` in the binary itself until `cli.rs` grows in prompt 1.
- `PLAN.md`, `AGENTS.md` at repo root per the plan's session-discipline requirements.

**Verified:**

- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test` — 5 passing (4 `config` unit tests covering defaults/repo-override/env-override/
  nested-env-override, 1 placeholder).
- `cargo fmt --check` clean.
- `cargo run` prints a skeleton banner and exits 0.

**Stubbed (doc-comment only, no logic yet):**

- `cli.rs`, `workspace.rs`, `change.rs`, `stage.rs`, `provider.rs`, `verify.rs`, `tui.rs` —
  each is currently just a module-level doc comment, no items, per prompt 0's "stub each"
  instruction. `main.rs` does not wire a CLI yet; it just loads config and prints a banner.

**Naming deviation from PLAN.md, recorded here since it affects future sessions:**

- Both `dlt` and `delta` are already taken on crates.io (unrelated crates: a computer-vision
  library and an unrelated `delta` CLI). The **binary** is still `dlt` as specified — binary
  names don't need to be globally unique. The **package** is `delta-cli` to get a free
  crates.io slot without a rename mid-build. If this ever gets published, revisit before
  prompt 7's release step.

**What prompt 1 should assume:**

- `Config::load(repo_root)` exists and works; prompt 1's `cli.rs` and `workspace.rs` can call
  it directly. No provider config section exists yet — don't assume `[providers.*]` parses
  into anything typed, it'll just round-trip as a generic table until prompt 2.
- No CLI subcommands are wired yet (`main.rs` is a placeholder banner). Prompt 1 is what
  introduces `clap` subcommands (`init`, `change new`, `change list`, `status`, `archive`) in
  `cli.rs` and the real `.delta/` layout in `workspace.rs`.
- `tempfile` is already a dev-dependency (pulled in early for `config.rs`'s tests) — prompt 1
  can rely on it being present rather than re-adding it, but still needs to add `assert_cmd`
  and `insta` itself.
- Dependency additions for prompt 1 per `PLAN.md`: neither `walkdir` nor `minijinja` is added
  yet (a `walkdir` add slipped into the initial prompt-0 batch by mistake and was removed once
  noticed, since nothing used it — don't re-add it until prompt 1's code actually needs it).
  `minijinja` is more likely needed once `stage.rs` does template rendering in prompt 3 than
  during prompt 1's on-disk model work; add each crate when the code that uses it is written,
  not preemptively.

## Session 1 (cont.) — Prompt 1 (workspace, changes, artifacts)

**Shipped:**

- `workspace.rs`: a `Store` trait (`exists`/`read_to_string`/`write_string`/`create_dir_all`/
  `list_dir`/`rename`, all paths relative to the workspace root) plus `FsStore`, its real
  filesystem-backed implementation. `Workspace::init`/`discover` create or open `.delta/{truth,
  changes,archive}`; `Workspace::detect_interop` checks the repo root for `openspec/`,
  `.kiro/specs/`, or `specs/` and returns which were found — `init` prints a note for each
  ("import is not yet supported"), no importer is built, per the plan.
- `change.rs`: the artifact model and change lifecycle, now real logic instead of a stub:
  - `Artifact` = `Frontmatter` (`stage`, `created`, `updated`, `source_hash`, `status`) + a
    markdown `body`, parsed from / rendered to `---\n<yaml>\n---\n<body>`.
  - `ArtifactStatus` = `Pending | Valid | Stale | Failed`.
  - `ArtifactKind` (`Proposal`/`Design`/`Tasks`) declares a **fixed** input chain — proposal
    has no inputs, design depends on proposal, tasks depends on proposal+design — since the
    runtime stage graph (YAML-driven `inputs:`) doesn't exist until prompt 3. This is a real
    design decision, not a stub: it's what makes `source_hash`/staleness meaningful right now.
    Documented inline so prompt 3 knows to replace it with stage-config-driven inputs.
  - `source_hash(bodies)`: SHA-256 over the literal concatenation of input bodies, hex-encoded.
  - `change_status`: finds the furthest artifact that exists (proposal < design < tasks),
    recomputes its expected hash from its inputs' *current* bodies, and reports `Stale` if that
    no longer matches the artifact's stored `source_hash` — regardless of what the frontmatter
    itself claims. This is what makes staleness never "silently valid."
  - `new_change`: validates the slug (lowercase alnum/`-`/`_`, no leading/trailing `-`),
    creates `changes/<slug>/deltas/` and a placeholder `proposal.md` with `status: pending`.
  - `archive_change`: refuses (returns `ChangeError::Stale`) if *any* existing artifact in the
    change is currently stale; otherwise copies each file in `deltas/` verbatim into `truth/`
    (delta = full replacement content for that truth file — the simplest reading available,
    since no delta format is specified before prompt 5's unified-diff patching, which is a
    different mechanism for code, not truth docs) and moves the change directory into
    `archive/`.
- `cli.rs`: real clap subcommands — `init`, `change new <slug>`, `change list`, `status`,
  `archive <slug>` — dispatching into `workspace.rs`/`change.rs` and formatting output; no
  business logic lives here per the module's stated responsibility.
- `error.rs`: added `ChangeError::InvalidSlug`, `ChangeError::Stale`, and
  `ChangeError::Workspace(#[from] WorkspaceError)`; added a new `CliError` enum wrapping
  `ConfigError`/`WorkspaceError`/`ChangeError` with an `exit_code()` method — this is the single
  place that maps errors to the CI-facing exit codes:
  - `0` success
  - `1` internal (I/O errors reaching the filesystem)
  - `2` validation failed (not initialized, already initialized, bad slug, unknown change,
    already-archived slug collision)
  - `3` gate not satisfied — **defined but not yet reachable**; no gate exists until prompts
    3/4 (rigor gate, verification gate) give it something to report
  - `4` stale inputs — returned by `archive` when the change has stale artifacts
- `main.rs` now parses argv with `clap`, dispatches through `cli::dispatch`, and returns
  `std::process::ExitCode` built from `CliError::exit_code()` (rather than always exiting 1 on
  error, which `anyhow::Result` in `main` would otherwise limit us to). `anyhow` is still used,
  but now only for the handful of genuinely unexpected top-level failures (`current_dir()`,
  config load) — the CLI's own error paths always go through the typed `CliError`.
- `tests/cli.rs`: integration tests via `assert_cmd` + `tempfile`, driving the real `dlt`
  binary as a subprocess (not calling internal functions): init → new → status happy path,
  init-twice and status-without-init exit-2 cases, invalid-slug exit-2, the openspec interop
  notice, archive-moves-and-applies-deltas, and the **stale-detection path**: hand-craft a
  `design.md` whose `source_hash` matches the current `proposal.md` body, confirm `status`
  reports it `valid`, edit `proposal.md`, confirm `status` now reports `stale`, then confirm
  `archive` refuses with exit code `4`.
- Also added matching unit tests inside `workspace.rs` and `change.rs` (`#[cfg(test)]`)
  exercising the same logic directly against `FsStore`, faster and independent of the
  subprocess layer.

**Verified:**

- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test` — 21 passing (14 unit across `config`/`workspace`/`change`/`tests`, 7
  integration in `tests/cli.rs`).
- `cargo fmt --check` clean.
- Manual smoke test of the built binary (`dlt init` → `dlt change new` → `dlt change list` →
  `dlt status` → `dlt archive` on a nonexistent slug → `dlt archive` on a real one) all
  produced the expected output and exit codes.

**Dependency note:** no new crates were needed beyond what prompt 0 already had, plus
`assert_cmd` as a dev-dependency (explicitly required by the prompt's "Integration tests with
tempfile + assert_cmd" instruction) and `chrono`'s `serde` feature (needed so `DateTime<Utc>`
round-trips through the YAML frontmatter). `walkdir` and `minijinja`, listed under prompt 1 in
`PLAN.md`'s dependency table, were **not** added — nothing in this prompt's actual
requirements needs recursive directory walking or templating (interop detection is a
single-level existence check; templating doesn't start until prompt 3's stage machine).
Consistent with the "add crates only when the code that uses them is written" rule recorded at
the end of the prompt 0 session — add them if/when prompt 1 code actually turns out to need
them, otherwise let prompt 3 add `minijinja` when it's actually used.

**What prompt 2 should assume:**

- `.delta/truth/`, `.delta/changes/<slug>/{proposal,design,tasks}.md`, `.delta/changes/<slug>/
  deltas/`, and `.delta/archive/<slug>/` all exist and are exercised by tests. The `Store`
  trait in `workspace.rs` is the file-access seam — prompt 2's provider layer doesn't need it
  (providers talk to an HTTP API, not the workspace), but any future interop adapter should
  implement `Store` rather than reaching for `std::fs` directly.
- `ArtifactKind`'s hardcoded input chain (proposal → design → tasks) in `change.rs` is a
  known-temporary stand-in for the real stage graph. Prompt 3 should replace `ArtifactKind::
  inputs()` with something driven by the YAML `inputs:` field once stages are runtime-loaded,
  and update `source_hash`/staleness call sites accordingly rather than keeping both systems.
- Exit code `3` ("gate not satisfied") is defined on `CliError` but has no live trigger yet —
  prompt 3 (rigor gate) or prompt 4 (verification gate) should be the first to actually return
  it; don't assume it's already wired to something.
- `cli.rs` still doesn't read anything from `config.rs` — no command's behavior depends on
  config yet. That's expected; prompt 2's provider config (`[providers.*]`) will be the first
  thing `cli.rs` (or whatever calls providers) actually needs to read back out.
