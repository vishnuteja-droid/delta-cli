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

## Session 4 — Prompt 4 (verification engine)

**Shipped:**

- `verify.rs`: the product, per `PLAN.md`'s own framing. Parses `## Acceptance Criteria`
  checklists out of **any** of a change's existing artifacts (not tied to a specific stage —
  stage schemas are user-authorable data since prompt 3, so the engine doesn't hardcode where
  criteria live). A checklist item (`- [ ] ...`) only becomes an executable `Criterion` if
  followed by a line that is *only* an inline code span `` `verify: <check>` ``; plain items
  with no such annotation are left for a human and silently skipped, not reported. A malformed
  `verify:` spec still becomes a criterion — a **failing** one, with the parse error as its
  detail — so a typo in a check is visible rather than silently ignored.
  - Three check kinds, hand-tokenized (quote-aware, no regex needed for the DSL itself):
    `cmd "<command>" [expect exit <n>] [contains "<text>"] [not_contains "<text>"]`,
    `file "<path>" (exists | contains "<text>" | matches "<regex>")`,
    `git changed "<glob>"`. A `cmd` check with none of the three assertion clauses is a parse
    error — a check that asserts nothing catches nothing.
  - `cmd` runs through a real shell (`sh -c` / `cmd /C`) so redirections like the literal
    `cargo doc 2>&1` example in `PLAN.md` work as written; this only ever executes commands the
    repo's own authors put in their own spec file, the same trust boundary as a Makefile or CI
    step, not untrusted input.
  - Per-check timeout (`--timeout`, default 120s per `PLAN.md`) implemented with plain
    `std::process` (no `duct`, which `PLAN.md` explicitly allowed skipping): spawn, hand the
    `Child` to a thread that blocks on `wait_with_output()`, `recv_timeout` on a channel back on
    the calling thread. On timeout, the child (spawned in its own process group via
    `CommandExt::process_group(0)` on Unix) is killed as a whole group (`kill -9 -<pid>`) so a
    shell that spawned its own children doesn't orphan them; Windows uses `taskkill /PID /T /F`.
  - `file matches`/`file contains` chomp one trailing newline before matching — Rust's `regex`
    crate's `$` does not match before a final `\n` by default, and a plain text file with one
    trailing newline (the overwhelmingly common case) would otherwise silently fail `matches
    "...$"` patterns in a way that looks like a bug in the check, not the file.
  - `git changed` shells out to `git diff --name-only HEAD` (same "uncommitted + staged vs.
    HEAD" convention `stage::classify` already uses) and matches the result against a `globset`
    pattern.
  - `verify_change(store, repo_root, slug, stages, timeout) -> Vec<CriterionResult>` runs every
    criterion found across the change's artifacts and returns pass/fail + failing detail per
    criterion — this is what both `dlt verify` and `dlt archive`'s gate call into.
  - `watch_and_rerun(repo_root, on_change)`: a generic, notify-backed blocking loop — `notify`
    owns the watching, `cli.rs` owns formatting/printing via the callback, keeping `verify.rs`
    UI-agnostic and `cli.rs` "dispatch only" per its module boundary. Debounces a burst of
    filesystem events (e.g. a `cargo build`) into a single rerun by draining the event channel
    for 300ms after the first relevant event before re-running. Filters out `.git`/`.delta`/
    `target`/`node_modules` noise.
- `dlt verify [slug] [--watch] [--timeout <secs>]`: verifies one change if `slug` is given, else
  every in-flight change (`change::list_changes`). Prints `[pass]`/`[FAIL]` per criterion with
  indented failing detail on failure. Exit code **2** if anything failed — the literal "makes it
  a CI gate" requirement — via a new `CliError::ChecksFailed { failed, total }` variant (not a
  real Rust error, just the vehicle for a non-zero exit with a summary message, since "some
  checks failed" is data `dlt verify` produces successfully, not a failure of verification
  itself).
- `dlt archive <slug> [--force]`: now runs the same `verify::verify_change` gate before
  archiving (in addition to prompt 1's staleness check, which still applies). Without `--force`,
  any failing check refuses the archive with the same exit code 2. With `--force`, a new
  `change::mark_verify_forced` stamps `verify_forced: true` onto every existing artifact of the
  change *before* the move into `archive/` — the literal "recorded in the archived frontmatter"
  requirement — and only when a failure was actually bypassed (an unforced-but-clean archive
  never touches the field). `Frontmatter.verify_forced` is `Option<bool>` with
  `skip_serializing_if = "Option::is_none"`, so it's entirely absent from the YAML on every
  artifact that was never force-archived, not present-and-`false`.
- `error.rs`: `VerifyError` replaced its prompt-0 `Unimplemented` stub with real variants
  (`Workspace`, `Change`, `Watch`). `CliError` gained `Verify(#[from] VerifyError)` and
  `ChecksFailed`; exit-code mapping follows the established nested-nested pattern (`Workspace(Io)`
  → 1, other `Workspace`/`Change` → their existing rules, `Watch` → 1, `ChecksFailed` → 2).

**Dependencies:** `globset`, `regex` (both already transitively present from `stage.rs`'s use of
`serde_yaml`/`minijinja`, now direct dependencies too), and `notify` — the one dependency
decision worth recording. `notify`'s Linux backend pulls in `inotify-sys`, a `-sys`-named crate,
which on its face looks like exactly what `PLAN.md`'s "if a crate pulls in a `-sys` dependency,
find another crate" rule forbids. Checked its `build.rs` before adding it anyway: no `cc`
build-dependency, no vendored/compiled C source — it's `extern "C"` declarations resolved
against the Linux kernel's inotify syscalls via glibc/musl, functionally identical in kind to the
plain `libc` crate already transitively present everywhere in this tree, and it only needs an
external link (`pkg-config` for `libinotify`) on NetBSD/OpenBSD, never on the Linux target this
project actually ships. The "no C dependencies" rule's real target — established in prompt 2's
`aws-lc-sys` rejection — is vendored/compiled crypto libraries and OS-trust-store fallbacks that
break static musl builds and the zero-setup promise; a syscall-binding crate needed for the exact
feature (`notify` for watch mode) that `PLAN.md`'s own dependency table names for this prompt
doesn't implicate either concern. Confirmed by rebuilding `--target x86_64-unknown-linux-musl`
with all three new deps in the tree: still `static-pie linked, statically linked` per `file`/
`ldd`, same as prompt 2's verification.

**Verified:**

- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test` — 74 passing (67 unit including 14 new in `verify`, one new in `change`
  (`mark_verify_forced_stamps_every_existing_artifact`); 7 `tests/cli.rs` integration, unchanged
  from prompt 1 — the existing `archive_moves_change_and_applies_deltas_to_truth` test still
  passes unmodified since a change with no `## Acceptance Criteria` section yields zero
  criteria, so the new archive gate is a no-op for it). Confirmed green under both
  `--test-threads=1` and default parallelism.
- `cargo fmt --check` clean.
- `cargo build --target x86_64-unknown-linux-musl` clean; binary confirmed still fully static.
- Manual end-to-end smoke test in a scratch git repo: hand-wrote five acceptance criteria into a
  change's `proposal.md` (two `file` checks that pass, one `file contains` that fails, one `cmd`
  that passes, one `git changed` that fails since nothing was actually changed) → `dlt verify
  <slug>` printed 3 pass / 2 fail with correct failing detail, exit 2 → `dlt archive <slug>`
  (no `--force`) refused with the same 2 failures printed, exit 2 → `dlt archive <slug> --force`
  archived successfully, printed which checks were bypassed, and the archived `proposal.md`
  frontmatter now literally contains `verify_forced: true` → separately, `dlt verify <slug>
  --watch` backgrounded, printed its initial pass, was triggered by touching a file in the repo,
  printed a second identical pass, then was killed cleanly. All behaved as designed.

**What prompt 5 should assume:**

- `verify::verify_change`/`Criterion`/`Check`/`CriterionResult` are the load-bearing public
  surface of this module; `dlt run`'s stage output validation (`stage::validate`, prompt 3) and
  this module's acceptance-criteria checking remain two distinct, independently-failing gates on
  purpose — prompt 5's tool loop should not conflate them.
- `change::mark_verify_forced` and `Frontmatter.verify_forced` exist specifically for the
  archive-bypass audit trail; if prompt 5's `dlt undo`/journal needs to know whether an archived
  change's checks were actually clean, this field is the source of truth (its *absence* means
  clean or never-archived, not "assume false" — it's `Option<bool>`, so check for `Some(true)`
  specifically, not just truthiness).
- `verify::watch_and_rerun`'s debounce/noise-filtering pattern (a raw `notify` channel drained
  for a short window after the first relevant event, `WATCH_NOISE_DIRS` skipping `.git`/`.delta`/
  `target`/`node_modules`) is the only place `notify` is used so far — if prompt 5's journal or
  prompt 6's TUI ever need filesystem watching too, this is the reference implementation, not a
  one-off.
- `dlt verify`'s check DSL (`cmd`/`file`/`git`) is intentionally minimal and text-based, matching
  `PLAN.md`'s literal three examples plus one invented `git changed` syntax (the plan only
  describes "files changed within a glob" prose, no literal syntax) — no plans exist for a fourth
  check kind; if one is needed later it likely wants its own `parse_*_check`/`Check` variant
  following the same pattern, not a generalized plugin system that hasn't been asked for.
- `CliError::ChecksFailed` is the second "non-error error" on `CliError` (after nothing else,
  really — it's the first of its kind) used purely to carry an exit code and summary message for
  a condition that isn't a Rust `Err` anywhere else in the call chain; prompt 5's approval-gate
  denials (`auto`/`prompt`/`deny`) may want the same shape rather than inventing a new error
  variant per tool that doesn't wrap a real failure.

## Session 5 — Prompt 5 (tool loop)

**The one real design decision this session, made up front and worth reading before anything
else below:** `PLAN.md`'s dependency table for this prompt lists only `similar`, `ignore`,
`grep-searcher` — no provider- or SSE-related crate — even though "tool results feed back into
the loop" clearly describes a real multi-turn, model-driven agent loop. Native function-calling
(Anthropic's `tool_use` content blocks, OpenAI's `tool_calls`) would have meant adding streaming
partial-JSON-argument accumulation to **both** `provider/anthropic.rs` and
`provider/openai_compatible.rs` — a real protocol difference between the two vendors — for a tool
that explicitly targets arbitrary `openai_compatible` backends including "most local servers,"
many of which don't implement either vendor's native tool-calling wire format at all. Instead,
`tools::agent` asks the model to express a tool call as a fenced `` ```tool_call `` code block
containing `{"tool": "...", "input": {...}}` inside its ordinary streamed text, and to respond
with plain prose (no such block) once it has a final answer. This works identically against
every backend `Provider` already supports and needed **zero changes** to `provider.rs` —
`Request`/`Message`/`Role`/`Delta` are exactly what prompt 3 left them; `Role::Assistant`, marked
"not yet constructed anywhere" back in prompt 2, is now genuinely used to hold each turn's full
text in the growing conversation. If a live model proves unreliable at this convention in
practice, the fallback would be exactly the native-tool-calling extension described above — the
loop's public shape (`run_loop`, `AgentObserver`, `ToolCall`/`ToolOutcome`) wouldn't need to
change, only `parse_tool_call`'s extraction and how the request is built.

**Shipped:**

- `tools.rs` (module root): the six gated tools `read_file`, `write_file`, `apply_patch`,
  `list_dir`, `search`, `run_command`, each behind a per-tool `Approval` (`auto`/`prompt`/`deny`)
  read from `[tools.<name>].policy` in config — falling back to `auto` for the three read-only
  tools and `prompt` for the three that mutate/execute (the literal "writes and commands default
  to prompt" requirement). `execute()` is the single dispatch entry point; a denied/declined/
  malformed-input call comes back as a **failing `ToolOutcome`**, not an `Err` — the agent loop
  always gets something concrete to react to and keep going, the same "malformed input becomes a
  visible failure, not a silent drop or a crash" pattern `verify.rs` established for its check
  DSL. `Approver` is a trait (`StdinApprover` prints the diff/command to stderr and reads y/n from
  stdin; tests use fakes) — the same "abstract the effectful boundary" move as `Store`/`Provider`.
  `run_command` never runs through a shell (`Command::new(program).args(args)` directly, no
  `sh -c`), checks its `[tools.run_command].allowlist` **before** the approval gate (so an
  unlisted program is never even prompted for), and reuses prompt 4's spawn/timeout/kill-process-
  group technique from `verify.rs`'s `run_cmd` (own process group via `CommandExt::process_group`,
  a channel + `recv_timeout`, `kill -9 -<pid>` / `taskkill /T /F` on timeout) minus the shell
  wrapping, which `run_command` must never use.
- `tools/apply_patch.rs`: unified-diff hunk parsing (`@@ -l,s +l,s @@`, tolerating leading
  `--- a/...`/`+++ b/...` headers and `\ No newline at end of file` markers, since the caller
  already knows the target path — only the hunks matter) plus **fuzzy context matching**, the
  prompt's own "hardest correctness problem" call-out. Each hunk's context+removed lines are
  located in the file by content, not trusted line numbers: an expanding-ring search starting at
  the hunk's declared line and walking outward, exact match first, then a trailing-whitespace-
  tolerant fallback. Hunks apply in order against the progressively-patched buffer (not the
  original), so later hunks correctly find content shifted by earlier ones. The replacement is
  assembled from the **actually-matched file lines** for context positions and the hunk's literal
  text only for genuine additions — this survived a real bug (see below) and matters because it's
  what keeps a fuzzy-matched but otherwise-unchanged line's incidental whitespace/CRLF from being
  silently clobbered by the patch author's own rendering of that same line. `str::lines()` already
  normalizes `\r\n` vs `\n` on both the file and the patch text, so CRLF tolerance falls out of the
  parsing for free; the file's original line-ending convention and trailing-newline-or-not are
  detected once and preserved on write. Explicitly tested against every scenario `PLAN.md` names —
  reordered/drifted context, trailing whitespace, CRLF, and overlapping hunks — plus insertion-only
  hunks, new-file creation, a clear `hunk not found` error, and header-line tolerance.
- `tools/search.rs`: `ignore::WalkBuilder` (with `require_git(false)` — `.gitignore` should apply
  because it's declared, not only inside an actual `.git` checkout) walking into `grep-searcher` +
  `grep-regex::RegexMatcher`, capped at `MAX_MATCHES` (200) so an unanchored pattern over a big
  repo can't blow the loop's token budget on its own. `grep-regex`/`grep-matcher` aren't in
  `PLAN.md`'s literal dependency list (only `grep-searcher` is) but are required to construct a
  `Matcher` at all — the same kind of gap-filling the `git changed` syntax invention was in prompt
  4, added and documented rather than blocking on it.
- `tools/journal.rs`: one JSON file per write under `.delta/journal/`, filename a zero-padded
  sequence number derived from the directory's *current* entry count (self-correcting after an
  undo removes an entry — see the module doc comment for why this can't collide). `undo_last`
  restores the target file's previous content, or deletes it if the write created a new file
  (`previous_content: Option<String>`, `None` = didn't exist before), and moves the consumed entry
  to a **sibling** `journal-undone/` directory (not nested inside `journal/`, so it's never counted
  when the next entry's filename is chosen) rather than deleting it — "reverted," not "erased,"
  matching the project's established archive-not-delete instinct. Only `write_file`/`apply_patch`
  are journalled; `run_command`'s effects aren't file mutations this module owns and generally
  aren't reversible at all — `PLAN.md` says "every **write** is journalled," not every tool call.
- `tools/agent.rs`: `run_loop` — the multi-turn conversation described above. Before every
  provider call it sums `count_tokens` over the system prompt and the whole running conversation
  against `provider.context_window() - RESERVED_OUTPUT_TOKENS`; over budget stops with
  `AgentError::TokenBudgetExceeded` rather than truncating anything, the literal requirement.
  After `max_iterations` turns without a plain-text final answer, stops with
  `AgentError::IterationCapReached`. A malformed `tool_call` block is fed back to the model as a
  parse-error message (one more turn, still counted against the cap) rather than aborting the
  whole loop. `AgentObserver` (default no-op methods, three callbacks: text delta, tool call, tool
  result) keeps this module UI-agnostic — `cli.rs`'s `StreamingObserver` prints text live to
  stdout and tool calls/results to stderr, the same callback-based seam `verify::watch_and_rerun`
  established in prompt 4 for keeping a module out of the UI-printing business. Tested via a
  `FakeProvider` (a scripted `VecDeque<&str>` of turn responses) rather than `wiremock` — this
  loop's logic is entirely in how it reacts to already-parsed text, not in HTTP/SSE framing, so a
  fake at the `Provider` trait boundary is the right layer, not another mock HTTP server.
- `dlt build <change> [--provider NAME] [--max-iterations N]`: assembles the initial context from
  whichever of the change's `proposal`/`design`/`tasks` artifacts already exist (concatenated in
  that fixed order — not stage-YAML-driven the way `dlt run`'s context assembly is, since the tool
  loop just wants whatever spec material exists, not a dependency walk) plus `AGENTS.md`, and
  drives `tools::agent::run_loop` with `StdinApprover` and the real loaded provider.
- `dlt undo`: reverts the single most-recent journalled write via `tools::journal::undo_last`.
- `error.rs`: `ToolError` (six tool's + the journal's internal-failure variants — a tool
  *reporting* failure to the model is a `ToolOutcome`, never this type) and `AgentError`
  (wraps `ProviderError`/`ChangeError`/`ToolError`/`Workspace`, plus the two loop-specific
  variants). `CliError` gained `Tool`/`Agent`; `IterationCapReached`/`TokenBudgetExceeded` map to
  exit code **3** (the established "gate not satisfied" precedent from prompt 3's rigor gate and
  the validation-failure shape). Extracted `provider_exit_code`/`tool_exit_code` helpers so
  `CliError::Provider` and `CliError::Agent(AgentError::Provider(_))` (etc.) share one mapping
  instead of duplicating the match arms.
- `workspace.rs`: `JOURNAL_DIR` constant, seeded (`create_dir_all`) alongside `truth/`/`changes/`/
  `archive/`/`stages/` on `init`.

**Fixed during this session (real bugs, not just test artifacts, caught by the apply_patch/search
test suite itself before anything shipped):**

- Pure-insertion hunks (`old_start` with a zero-length old range, e.g. `@@ -2,0 +3,1 @@`) were
  inserting one line too early — off-by-one against unified diff's own convention, where a
  zero-length old range's start line means "insert **after** this original line," not
  "old_start - 1" the way a real (non-empty) range's start does. Fixed by using `old_start`
  directly (clamped to the file's length) as the insertion index for this case only.
- The original `apply_hunk` built its replacement purely from the hunk's own literal text for both
  context and added lines. Combined with the whitespace-tolerant fuzzy match pass, this meant a
  context line matched *despite* a trailing-whitespace difference would still get overwritten with
  the hunk's (whitespace-stripped) version — silently deleting real trailing whitespace the patch
  never intended to touch. Fixed by rebuilding the replacement from the **actually-matched file
  lines** for context positions, using the hunk's literal text only for genuine `+` additions.
- `tools::search`'s `.gitignore` handling: `ignore::WalkBuilder` defaults to `require_git(true)`,
  meaning `.gitignore` files are only honored when the target directory is actually inside a
  `.git` checkout — a temp-dir test (and, in principle, any repo tree `dlt` is pointed at before
  `git init`) silently ignored `.gitignore` entirely. Fixed with `.require_git(false)`.

**Verified:**

- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test` — 117 passing (110 unit including 63 new across `tools`/`tools::apply_patch`/
  `tools::journal`/`tools::search`/`tools::agent`; 7 `tests/cli.rs` integration, unchanged). Green
  under both `--test-threads=1` and default parallelism.
- `cargo fmt --check` clean.
- `cargo build --target x86_64-unknown-linux-musl` clean; `file`/`ldd` confirm the binary is still
  fully static (`static-pie linked, statically linked`) with `similar`/`ignore`/`grep-searcher`/
  `grep-regex`/`grep-matcher` all in the tree — no new `-sys` crates beyond the pre-existing
  `dirs-sys`/`inotify-sys`, confirmed via `cargo tree --edges normal`.
- Manual smoke test of the built binary in a scratch git repo, covering everything reachable
  without live provider credentials (the loop itself is exercised by the `FakeProvider` unit
  tests above, consistent with prompt 2's "no live API calls in the test suite" precedent): `dlt
  undo` with an empty journal → exit 2, clear message; `dlt build <nonexistent-slug>` → exit 2
  (`ChangeError::NotFound`, checked explicitly before the provider is even loaded); `dlt build
  <real-slug>` with no `[providers.default]` configured → exit 2 (`ProviderError::NotConfigured`);
  `dlt --help` lists `build` and `undo` with their doc-comment descriptions. The tool substrate
  itself (approval gates, journal write/undo, `apply_patch`'s fuzzy matching against drift/
  whitespace/CRLF/overlapping hunks, the allowlist gate on `run_command`) is covered end-to-end by
  the unit test suite against real temporary filesystems, not just mocked.

**What prompt 6 should assume:**

- The tool-call protocol is text-embedded JSON in a fenced `` ```tool_call `` block, parsed by
  `tools::agent::parse_tool_call` out of the model's plain streamed text — **not** either vendor's
  native function-calling wire format. `provider.rs`/`provider/anthropic.rs`/
  `provider/openai_compatible.rs` are byte-for-byte unchanged from prompt 3. If the TUI wants to
  render tool calls/results distinctly from prose (likely, for a good UX), it should render off
  `AgentObserver`'s three callbacks (`on_text_delta`/`on_tool_call`/`on_tool_result`) rather than
  re-parsing `` ```tool_call `` blocks itself — `cli.rs`'s `StreamingObserver` is the reference
  implementation of "something that reacts to these callbacks," and the TUI needs the same seam
  with a different renderer, exactly like `verify::watch_and_rerun`'s callback was reused as a
  reference pattern last session.
- `tools::execute`/`ToolCall`/`ToolOutcome`/`Approver` are the load-bearing public surface if a
  future prompt wants to invoke a single tool outside the full agent loop (e.g. a TUI keybinding
  that runs `search` directly) — call `tools::execute` the same way `tools::agent::run_loop` does,
  don't reimplement approval-gate/journal logic at a new call site.
- `AgentOutcome.final_answer` exists but nothing in `cli.rs` reads it today (the text was already
  streamed live via `on_text_delta`); it's there for a future non-streaming caller or the TUI,
  which may want the accumulated final text without re-deriving it from callbacks.
- The journal only records `write_file`/`apply_patch`; `dlt undo` cannot revert a `run_command`
  call's side effects (a test suite run, a build, anything a command actually did to the
  filesystem or beyond). This is a deliberate scope reading of "every **write** is journalled,"
  not an oversight — worth flagging to a user, but not something prompt 6 needs to fix.
- `[tools.<name>].policy` and `[tools.run_command].allowlist` are the two new config surfaces this
  session added; neither has a default value written into `config.rs`'s `Config::defaults()` (the
  policy fallback lives in `tools::default_policy`, the allowlist's absence just means "nothing
  runnable" by design) — if prompt 6's TUI exposes a settings/config view, these are real,
  user-facing knobs worth surfacing, not just internal wiring.

## Between sessions — Gemini provider (user request, not a PLAN.md prompt)

Before starting prompt 6, the user asked to hold off on the TUI and validate prompts 1–5 on a
real change with a real provider first — the literal "Order discipline" gate in `PLAN.md`
("Do not start prompt 6 until prompts 1–5 have been used on a real change in a real repository")
had not actually been satisfied by any session so far; every prior manual check was `--dry-run`
or an error-path check, never a live model call. In the same breath, the user asked to add more
providers ("like gemini") so they could do that live test themselves, and asked how to run `dlt`.

**Shipped:** `provider/gemini.rs`, a third `AnyProvider` variant (`kind = "gemini"`) alongside
`openai_compatible`/`anthropic`, following the exact structure of `provider/anthropic.rs`: a
`Provider` impl, `send_with_retry`/`with_cancellation` reused unchanged, `eventsource-stream` for
SSE framing. Specifics that differ from the other two:
- Streaming endpoint is `{base_url}/models/{model}:streamGenerateContent?alt=sse` — the `alt=sse`
  query param is load-bearing; without it the Generative Language API returns one large chunked
  JSON array instead of discrete SSE events, which `eventsource-stream` can't parse.
- Auth is the `x-goog-api-key` header (not `x-api-key`, not `Bearer`).
- Request shape is Gemini's own `contents: [{role, parts: [{text}]}]` +
  `systemInstruction: {parts: [{text}]}` + `generationConfig.maxOutputTokens` — nothing like the
  other two providers' bodies. Gemini has no "assistant" role; prior model turns use `"model"`.
- Each SSE frame is a **complete** `GenerateContentResponse`, not a small incremental delta the
  way Anthropic's `text_delta` or OpenAI's `delta.content` are — `parse_event` extracts
  `candidates[0].content.parts[].text`, concatenating multiple `parts` in one frame. An inline
  `{"error": ...}` body (Gemini can send one even inside a nominally-200 SSE stream) surfaces as
  `ProviderError::MalformedStream`, same as a genuinely malformed JSON body.
- No new crate dependencies — `reqwest`/`eventsource-stream`/`serde_json`/`tokio-util` were
  already present from prompt 2.

`provider.rs`'s `load()` gained a `"gemini"` arm; the `InvalidConfig` error message for an
unrecognized `kind` now lists all three. `AnyProvider`'s four trait-method match statements each
gained a `Gemini(p) =>` arm.

**Verified:** `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check` clean,
`cargo test` — 124 passing (117 unit including 7 new `provider::gemini` tests mirroring the
Anthropic suite's structure — streaming across multiple frames, multi-`parts` concatenation,
SSE-split-across-chunk-boundaries, inline error body, 503 retry, non-retryable 400, fail-fast on
pre-cancellation; 7 `tests/cli.rs` integration, unchanged). No live Gemini API call in the checked-
in suite, consistent with the "no live API calls in the test suite" rule from prompt 2 — the user
is doing that live validation themselves, outside this session, with their own API key.

**Order-discipline status:** still open, not skipped. The user is now running the actual live
validation `PLAN.md` asks for (`dlt init` → `change new` → `run` through the stages → `dlt build`
against a real provider, likely Gemini given this addition) outside this session. Prompt 6 should
not start until that's confirmed done — check for a follow-up message reporting success (or new
bugs to fix) before writing any TUI code.

**Running `dlt` — for reference, since there's no README yet (prompt 7's job):**
1. `cargo build --release` (or `cargo run --` during development).
2. In the target repo: `dlt init`, then `dlt change new <slug>`.
3. Add a provider to `.delta/config.toml` (repo-rooted) or `~/.config/delta/config.toml`, e.g. for
   Gemini:
   ```toml
   [providers.default]
   kind = "gemini"
   base_url = "https://generativelanguage.googleapis.com/v1beta"
   model = "gemini-2.0-flash"
   api_key_env = "GEMINI_API_KEY"
   ```
   (`export GEMINI_API_KEY=...` in the shell — never written to config itself.)
4. `dlt run proposal --change <slug>` (then `design`, then `tasks`) — each streams the
   completion to stdout and writes the artifact; add `--dry-run` first to sanity-check the
   assembled prompt without spending any tokens.
5. `dlt verify <slug>` once a stage's artifact has `## Acceptance Criteria` checks in it.
6. `dlt build <slug>` to run the tool loop — it will print a diff or command and ask `[y/N]`
   before any write or command execution (`[tools.<name>].policy` in config changes that).
7. `dlt undo` reverts the most recent `write_file`/`apply_patch` from `dlt build` if needed.
