# AGENTS.md

## What this is

`dlt` (package name `delta-cli`) — a terminal-first AI development lifecycle tool. Rust,
single static binary, single build path. Being built prompt-by-prompt from `PLAN.md`.

## Stack

- Rust, edition 2024
- Binary target: `dlt` (`cargo run` / `cargo build` produce `target/*/dlt`)
- No C dependencies anywhere (rustls, not openssl) — this keeps static musl builds and the
  zero-setup promise intact. If a crate you're about to add pulls in a `-sys` dependency,
  find another crate instead.
- Crates are added only in the prompt that needs them — see the dependency table in
  `PLAN.md`. Do not pre-pull `tokio`, `reqwest`, or `ratatui` ahead of their prompt.

## Commands

- `cargo check` — fast compile check, run this first every session
- `cargo test` — full test suite
- `cargo clippy --all-targets -- -D warnings` — must be clean before a prompt is considered done
- `cargo fmt` — format; `cargo fmt --check` in CI-equivalent checks
- `cargo run -- <args>` — run the binary

## Conventions

- **No `unwrap()` or `expect()` outside `#[cfg(test)]`.** Enforced by `[lints.clippy]` in
  `Cargo.toml` (`unwrap_used = "deny"`, `expect_used = "deny"`). Non-test code must return a
  `Result` with a concrete error type instead.
- One `thiserror` enum per module in `error.rs`. `anyhow` is used only in `main.rs`, to collect
  and report errors at the top level — never inside library code.
- Module boundaries (see `error.rs` and each module's doc comment for the authoritative
  statement of what it owns):
  - `cli` — clap definitions and dispatch only, no business logic
  - `config` — layered config loading (defaults → user file → repo file → env), does not
    construct providers or other runtime objects
  - `workspace` — `.delta/` directory layout, file access behind a `Store` trait
  - `change` — artifact model, `source_hash`/staleness, change lifecycle
  - `stage` — stage definitions, rigor classification, context assembly
  - `provider` — LLM streaming abstraction and implementations
  - `verify` — executable verification of acceptance criteria
  - `tui` — render loop only; never makes network/provider calls itself
- Reject `todo!()` scaffolding of future modules once past prompt 0. Code that compiles and
  does nothing reads as progress and isn't — a stub is a doc comment, not a placeholder
  function body.
- Branch per prompt so a bad session is one `git reset` away from clean.
- Do not advance to the next prompt until the current prompt's tests pass — the prompts are
  load-bearing on each other.
- End every session by updating `PROGRESS.md`: what shipped, what is stubbed, what the next
  prompt should assume.

## Session start ritual

Read `PLAN.md`, `AGENTS.md`, `PROGRESS.md`. Run `cargo check` and `cargo test` and report
current state before writing any code.

**Never implement beyond the current prompt in PLAN.md.**
