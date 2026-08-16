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

## Session 2 — Prompt 2 (provider layer)

**Shipped:**

- `provider.rs` (module root) + `provider/openai_compatible.rs` + `provider/anthropic.rs`:
  - `Request`/`Message`/`Role`/`Delta`: minimal chat-completion types — just enough for prompt
    3's stage machine to assemble a prompt and stream a completion. No tool-use, no images.
  - `trait Provider`: `name()`, `context_window()`, `count_tokens()`, and a native `async fn
    stream(...) -> Result<BoxStream<'static, Result<Delta, ProviderError>>, ProviderError>`.
    Native async fn in traits (stable since Rust 1.75) isn't dyn-compatible, so there's no
    `Box<dyn Provider>` — instead `AnyProvider` is a plain enum (`OpenAiCompatible` |
    `Anthropic`) that implements `Provider` by matching and delegating. `provider::load(config,
    name)` reads `[providers.<name>]` out of `config.rs`'s generic table and constructs the
    right variant.
  - `OpenAiCompatible`: POSTs `{base_url}/chat/completions` with `stream: true`, Bearer auth,
    parses `data: {...}` / `data: [DONE]` SSE frames, extracts `choices[0].delta.content`.
  - `AnthropicProvider`: POSTs `{base_url}/messages`, `x-api-key` + `anthropic-version:
    2023-06-01` headers, parses `event: content_block_delta` frames with `delta.type ==
    "text_delta"`; a server-sent `event: error` surfaces as `Err`; every other event type
    (`message_start`, `content_block_start/stop`, `message_delta`, `message_stop`, `ping`) is
    structural and skipped without ending the stream.
  - `send_with_retry`: exponential backoff with jitter (`backoff::future::retry` +
    `ExponentialBackoffBuilder`, 500ms initial / 30s max interval / 120s max elapsed) on `429`
    and `5xx`; honors a `Retry-After: <seconds>` header when present via
    `backoff::Error::retry_after`; every other non-2xx status is permanent (no retry).
  - `with_cancellation`: wraps a boxed+pinned stream in `futures::stream::unfold` racing
    `cancel.cancelled()` against `stream.next()` via `tokio::select!` — the literal "abortable
    mid-flight via `CancellationToken`" requirement. `send_with_retry` also checks
    `cancel.is_cancelled()` before every attempt, so cancelling before the request even goes
    out fails fast with `ProviderError::Cancelled` rather than silently returning an empty
    stream — see `stream_fails_fast_when_token_already_cancelled`.
  - `count_tokens`: a chars/4 heuristic (`approximate_token_count`), explicitly documented as a
    placeholder — real tokenization is prompt 3's `tiktoken-rs or equivalent`, not prompt 2's.
  - Provider config beyond the literal PLAN.md TOML example: `context_window` is an optional
    per-provider integer (default `128_000`) since neither the Chat Completions nor Messages
    API reports its model's context window — prompt 3 needs `Provider::context_window()` for
    budget management, so something has to supply it, and config is the only place available
    without hardcoding a model-name table. `headers` is an optional `[providers.<name>.headers]`
    sub-table applied as extra request headers on every call, per "Both configured by base_url,
    model, api_key_env, headers" in the prompt text.
- `error.rs`: `ProviderError` is now real (`NotConfigured`, `MissingConfig`, `InvalidConfig`,
  `MissingApiKey`, `Request`, `Http`, `MalformedStream`, `Cancelled`) — no `CliError` exit-code
  mapping added yet since nothing in `cli.rs` calls providers until prompt 3.
- `config.rs`: dropped the now-stale `#[allow(dead_code)]` on `Config::get`/`get_str`/`table`
  now that `provider.rs` genuinely reads them.

**Fixed incidentally (found while sanity-checking test output at session start, unrelated to
prompt 2's scope but a real bug):**

- `config.rs`'s env-var tests (`env_overrides_files`, `nested_env_override`) mutate
  process-global env vars while `defaults_only_when_no_files_present` (which scans all
  `DELTA_*` vars via `Config::load`) can run concurrently on another thread under the default
  parallel `cargo test` — an actual flake, reproduced at the top of this session. Fixed with a
  module-local `static ENV_LOCK: Mutex<()>` that every test in `config::tests` holds for its
  whole body, serializing them against each other regardless of run order. Confirmed fixed
  with three consecutive default-parallelism `cargo test` runs, all green.

**Dependency / TLS-stack notes — the one real surprise this session:**

- `reqwest` had to be **pinned to `0.12`**, not `0.13` (the version `cargo add` picks by
  default). reqwest 0.13 restructured its TLS features: every rustls-enabling feature (`rustls`,
  `rustls-no-provider`, `http3`, `default-tls`) now pulls in `rustls-platform-verifier` (OS
  trust store, not compiled-in roots) and, for `rustls`/`default-tls`/`http3` specifically,
  `aws-lc-rs` → `aws-lc-sys`, a full C crypto library — a hard violation of "no C dependencies
  anywhere." There is no webpki-roots-backed feature combination left in 0.13 at all. reqwest
  0.12 still has the classic `rustls-tls-webpki-roots` feature (ring-backed, no
  platform-verifier, no aws-lc-sys), which is what's actually in `Cargo.toml`:
  `reqwest = { version = "0.12", default-features = false, features = ["json", "stream",
  "rustls-tls-webpki-roots"] }`. Confirmed via `Cargo.lock`: `ring` present, `webpki-roots`
  present, no `aws-lc-sys`/`native-tls`/`openssl` anywhere in the tree.
- `tokio-util` provides `CancellationToken` (used for cancellation) and also re-exports the
  `bytes` crate as `tokio_util::bytes` — used in tests instead of adding `bytes` as a direct
  dependency.
- **The "verify from a static musl build with no system cert store, do not defer this" line
  was taken literally and actually done in this session**, not deferred to prompt 7:
  `rustup target add x86_64-unknown-linux-musl` + `apt-get install musl-tools`, confirmed
  `file`/`ldd` report the `dlt` binary as `static-pie linked, statically linked` (no dynamic
  deps at all). Built a throwaway `examples/tls_smoke_test.rs` (deleted after use, not part of
  the crate) making one real HTTPS request through this exact `reqwest` configuration, ran it
  under `strace -e trace=openat,open,access,stat,newfstatat` with `env -i` (no `HOME`, no
  `SSL_CERT_FILE`/`SSL_CERT_DIR`, nothing an OS-trust-store code path could fall back to). The
  request completed a real TLS handshake and got HTTP 200 back, and the strace log has zero
  matches for `ssl/certs|ca-certificates|/etc/pki|/etc/ssl|share/ca-cert` — the binary never
  touched a filesystem cert store at all, proving webpki-roots' compiled-in roots are what's
  actually being used, not something incidentally reachable in this dev sandbox.

**Verified:**

- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test` — 33 passing (26 unit including 11 new provider tests across `provider`,
  `provider::openai_compatible`, `provider::anthropic`; 7 `tests/cli.rs` integration, unchanged
  from prompt 1). All against a mock HTTP server (`wiremock`) — no live API calls in the
  checked-in test suite, per the prompt's explicit requirement.
- `cargo fmt --check` clean.
- `cargo build --target x86_64-unknown-linux-musl` — clean, see the TLS verification above.
- The SSE frame-split-across-chunk-boundaries requirement is tested directly at the parsing
  seam rather than through wiremock's declarative body API: wiremock has no control over actual
  TCP chunk boundaries (loopback typically coalesces a small body into one read regardless of
  what the mock's `ResponseTemplate` describes), so
  `reassembles_sse_event_split_across_chunk_boundaries` in both provider test modules instead
  feeds `eventsource_stream::Eventsource` a `futures::stream::iter` of two `Bytes` chunks that
  split a single SSE event mid-field, and asserts it reassembles into one complete `Event`. The
  wiremock-based tests separately prove the full HTTP-to-`Delta` pipeline wires together
  correctly end to end.

**What prompt 3 should assume:**

- `provider::load(config, name) -> Result<AnyProvider, ProviderError>` exists and works; the
  stage machine should call this rather than constructing `OpenAiCompatible`/`AnthropicProvider`
  directly. `AnyProvider` implements `Provider`, so generic code can just take `&impl Provider`
  or match on it directly — there's no dyn story to route around.
  `dlt run <stage>` in prompt 3 is the **first real caller** of this module from `cli.rs`; until
  then everything in `provider.rs`/`provider/*.rs` is marked `#![allow(dead_code)]` at the
  module root (fully implemented and tested, just not wired to any command yet) — remove that
  attribute once `cli.rs` calls in.
- `Provider::count_tokens` is a chars/4 approximation, not real tokenization — prompt 3 is
  explicitly where `tiktoken-rs or equivalent` gets added per `PLAN.md`'s dependency table; swap
  the implementation then, the trait signature doesn't need to change.
- `Provider::context_window()` comes from an optional `context_window` key under
  `[providers.<name>]` (default `128_000` if omitted) — this key is **not** in `PLAN.md`'s
  literal example TOML, so example/test configs prompt 3 writes should either set it explicitly
  or accept the default; don't assume it's derived from the model name.
- No `CliError`/exit-code mapping exists yet for `ProviderError` — prompt 3's `dlt run` will be
  the first place that needs to decide, e.g., whether a `ProviderError::Http` is exit code 1
  (internal/network) or something else; nothing in prompt 2 presupposes an answer.
- `CancellationToken` plumbing is in place end-to-end (`Provider::stream` takes one, honors it
  both pre-flight in `send_with_retry` and mid-flight via `with_cancellation`) — prompt 6's `esc
  to interrupt` (or prompt 3's own interrupt handling, if it needs any before the TUI exists)
  can create one, pass it down, and call `.cancel()` from wherever the interrupt signal lands.

## Session 3 — Prompt 3 (stage machine)

**Shipped:**

- `stages/{proposal,design,tasks}.yaml` at the repo root: compile-time-embedded (`include_str!`)
  seed content. `Workspace::init` copies them out to `.delta/stages/*.yaml` on disk; from then
  on `stage::load_all` reads exclusively from `.delta/stages/`, so editing or adding a stage
  file post-init never requires a recompile, per the prompt's explicit requirement. `design.yaml`
  is copied verbatim from `PLAN.md`'s literal example; `proposal.yaml`/`tasks.yaml` were designed
  to fit the same schema and preserve prompt 1's proposal→design→tasks shape (no stage YAML for
  those two is given in the plan text).
- `stage.rs` (module root): `Rigor` (`Trivial < Standard < Deep`, derived `Ord`, `FromStr`/
  `Display`, no `clap` dependency — `cli.rs`'s derive picks it up via `FromStr` automatically,
  keeping stage.rs's module boundary clean), `StageDefinition`/`OutputSpec`/`Validator`
  (`NonEmptySections`/`NoPlaceholderText`/`MinWords(usize)`), two-phase YAML parsing (private
  `RawStageDefinition`/`ValidatorSpec` with `#[serde(untagged)]` resolving both the bare-string
  and single-key-map validator shapes), `load_all` (lists+parses `.delta/stages/*.{yaml,yml}`),
  and `topological_order` — Kahn's algorithm with explicit checks for duplicate ids, dangling
  input references, zero/multiple root stages, and cycles (all via `StageError::InvalidGraph`,
  never a panic).
- `stage/classify.rs`: `classify(repo_root) -> Rigor` from `git diff HEAD` — counts files
  touched (`diff --git` lines) and scans added lines for public-interface markers (`pub fn`,
  `export function`, `def `, etc. across Rust/JS/Python). Thresholds: any interface change or
  >10 files touched → Deep; >1 file → Standard; else Trivial. Falls back to `Rigor::Trivial`
  (never blocks, never panics) if git is unavailable or the repo doesn't exist.
- `stage/validate.rs`: `validate(output, body) -> Vec<String>` — heading-based section
  extraction (a section's content runs until the next heading at the same or shallower level;
  deeper subheadings and plain text both count as content) for `non_empty_sections`,
  case-insensitive substring scan (TODO/TBD/Lorem ipsum/XXX/FIXME) for `no_placeholder_text`,
  whitespace word count for `min_words`.
- `stage/context.rs`: `assemble(store, repo_root, stage, slug, provider) -> Assembled` —
  `AGENTS.md` read directly via `std::fs` (outside `.delta/`, so not through `Store`),
  `.delta/truth/*.md` concatenated under per-file headings, a `walkdir`-based repo tree summary
  (`.sort_by_file_name()` for determinism, `NOISE_DIRS` skips `.git`/`.delta`/`target`/
  `node_modules`/`dist`/`build`, capped at 500 entries), declared `inputs` resolved via
  `change::read_artifact_body` (errors `StageError::MissingInput` if an input hasn't been
  generated yet — "run `dlt run <input>` first"), rendered through MiniJinja
  (`Environment::render_str` + the `context!` macro + `Value::from_serialize` for the inputs
  map — verified against the actual 2.24.0 source before writing any code, not guessed).
  Token budget = `provider.context_window()` minus a 4096-token reserve for the model's own
  output; over budget, drops **repo_tree**, then **truth.relevant**, then **agents_md**, in that
  order (declared `inputs` are never dropped), re-rendering and re-counting after each drop,
  recording what got dropped in `Assembled.dropped`. `insta` snapshot test
  (`snapshot_of_assembled_design_prompt`) pins the literal rendered output of a design-stage
  prompt end to end.
- `provider.rs`: real BPE token counting via `tiktoken-rs`'s bundled (not network-fetched)
  `cl100k_base` vocab, behind a `OnceLock<Option<CoreBPE>>` that falls back to the old chars/4
  heuristic rather than `unwrap()`/`expect()` on `cl100k_base()`'s technically-fallible `Result`
  (it can't actually fail with the vocab compiled in via `include_str!`, but the crate-wide
  `unwrap_used`/`expect_used` ban doesn't care). Added `DryRunProvider` + `load_for_dry_run`: a
  provider stand-in exposing only `context_window`/`count_tokens` (both derivable from config
  alone, no `api_key_env` lookup) so `dlt run --dry-run` works with **zero live credentials** —
  a deliberate elaboration on the plan's "prints the assembled prompt without calling
  anything... this is how you debug context assembly forever," since requiring a real API key
  just to preview a prompt would undercut that. Removed the module's `#![allow(dead_code)]` now
  that `cli.rs` is the real caller.
- `change.rs`: replaced `ArtifactKind` entirely with generic `stage_id: &str` +
  `stages: &[StageDefinition]` parameters threaded through `new_change`, `change_status`,
  `archive_change`, `stale_artifacts`, `recompute_hash`; `ChangeStatus.stage` is now an owned
  `String` (was `&'static str`, impossible now that stage ids come from loaded YAML).
  `Frontmatter` gained `rigor: Option<Rigor>` (`#[serde(default)]`, backward-compatible with
  pre-prompt-3 artifacts — missing/unknown rigor defaults to `Rigor::Deep`, the safe "never
  silently skip" fallback, via the new `change_rigor` helper). `ArtifactStatus` gained
  `NotApplicable` (serialized as `"n/a"`), which `change_status`/`stale_artifacts` both treat as
  a terminal, never-stale state — a stage deliberately skipped for rigor reasons doesn't get
  re-flagged just because its (irrelevant) stored hash drifts. New `read_artifact_body` (used by
  both `recompute_hash` and `stage::context::assemble`) and `write_stage_artifact` (used by
  `cli.rs`'s `cmd_run`; takes a `StageWrite` struct rather than 5 loose params to stay under
  clippy's `too_many_arguments`) — preserves an artifact's original `created` timestamp across
  reruns, always recomputes `source_hash` fresh from current input bodies.
- `cli.rs`: `Command::Run(RunArgs)` — `dlt run <stage> --change <slug> [--dry-run]
  [--provider NAME] [--rigor R]`. `--change` is a deliberate elaboration: `PLAN.md`'s literal
  `dlt run <stage>` doesn't say which change, and a stage run has to target one. Rigor
  resolution: `--rigor` override wins, else `change::change_rigor` (the value recorded at
  `change new` time). If `stage.min_rigor > effective_rigor`, writes an `n/a` artifact and
  prints a skip message — **returns `Ok(())`, not an error** — per "skipped... not failed."
  Otherwise assembles context, prints what (if anything) got dropped and the final token count
  to stderr, and either prints the prompt (`--dry-run`) or spins up a fresh
  `tokio::runtime::Runtime` (`main.rs` stays fully synchronous; only this command pays tokio
  startup cost) to stream the completion to stdout as it arrives, validates the result, and
  writes the artifact as `Valid` or `Failed`. A validation failure returns
  `StageError::ValidationFailed`, mapped to **exit code 3** — the first real trigger for "gate
  not satisfied" in the whole project; every other `StageError`/most `ProviderError` variants
  map to 2, `Provider::{Request,Http,MalformedStream,Cancelled}` and the new `CliError::Runtime`
  map to 1. `ChangeCommand::New` gained `--rigor` (override) with the same "always allow to force
  the full path" semantics; without an override it calls `stage::classify::classify`.

**Verified:**

- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test` — 59 passing (52 unit including 21 new across `stage`/`stage::classify`/
  `stage::context`/`stage::validate`/`change`/`provider`, plus the `insta` snapshot; 7
  `tests/cli.rs` integration, unchanged from prompt 1). Confirmed green under both
  `--test-threads=1` and default parallelism, no flakes.
- `cargo fmt --check` clean.
- Manual end-to-end smoke test of the built binary in a scratch git repo: `dlt init` → `dlt
  change new my-feature --rigor deep` → `dlt status` (shows `pending`) → wrote a
  `[providers.default]` config block → `dlt run proposal --change my-feature --dry-run` (prints
  the assembled prompt, no API key set, works as designed) → `dlt run design --change
  my-feature --dry-run` (pulls in the placeholder proposal body as `{{ inputs.proposal.body }}`)
  → `dlt change new tiny-fix --rigor trivial` → `dlt run design --change tiny-fix --dry-run`
  (writes an `n/a` artifact, prints the skip message, exit 0, confirmed via `dlt status`) → `dlt
  run tasks --change my-feature --dry-run` before design ran (fails with `StageError::
  MissingInput`, exit 2, points at `dlt run design`) → `dlt run bogus-stage ... ` (fails with
  `StageError::NotFound`, exit 2). All behaved as designed.
- Re-confirmed no C dependencies crept in: `minijinja`/`walkdir`/`tiktoken-rs` (real deps) and
  `insta` (dev-only, doesn't affect the release binary) all resolve to pure-Rust dependency
  trees — `cargo tree --edges normal` shows only the pre-existing plain `libc` bindings crate,
  no new `-sys` crates, no `openssl`/`aws-lc`/`native-tls`.

**What prompt 4 should assume:**

- `stage::load_all(store) -> Result<Vec<StageDefinition>, StageError>` returns stages in
  topological order (root first); `change_status`/`stale_artifacts` rely on that ordering rather
  than re-sorting themselves — don't reorder the returned `Vec` without updating those call
  sites.
- `ArtifactStatus::NotApplicable` exists and is load-bearing: it's the only status
  `change_status`/`stale_artifacts` treat as permanently non-stale. Verification (prompt 4)
  should likely skip `n/a` artifacts the same way `dlt run`'s rigor gate does, rather than
  trying to verify a stage that was deliberately never generated.
- `dlt run`'s validation step (`stage::validate::validate`) is purely textual (required
  sections present/non-empty, no placeholder markers, minimum word count) — it does **not**
  execute anything. Prompt 4's "executable verification" is a distinct, additional gate on top
  of this, not a replacement for it; both can fail independently and should probably both be
  checked before a stage counts as `Valid`.
- `DryRunProvider`/`provider::load_for_dry_run` exist specifically so prompt-assembly debugging
  never needs live credentials — if prompt 4 adds its own "preview without calling anything"
  path, prefer extending this rather than inventing a second no-credentials provider path.
- No `CliError`/exit-code mapping exists yet for whatever prompt 4's verification engine
  produces (`VerifyError` is still the prompt-0 `Unimplemented` stub) — exit code 3 is now
  precedented (validation-failure shape), but prompt 4 should decide for itself whether
  verification failures reuse it or need their own code.
