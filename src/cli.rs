//! Command-line surface: clap argument/subcommand definitions and dispatch.
//! Maps parsed commands to calls into the other modules and translates
//! their results into process exit codes. Holds no business logic itself.

use std::io::Write as _;
use std::path::Path;
use std::time::Duration;

use chrono::Utc;
use clap::{Parser, Subcommand};
use futures::StreamExt;

use crate::change::{self, ArtifactStatus};
use crate::config::Config;
use crate::error::{CliError, StageError};
use crate::provider::{self, AnyProvider, Message, Provider, Request, Role};
use crate::stage::{self, Rigor, StageDefinition};
use crate::tools::agent::{self, AgentObserver};
use crate::tools::{self, ToolCall, ToolOutcome};
use crate::verify;
use crate::workspace::{Store, Workspace};

/// Tokens requested for a stage's completion. Mirrors the budget
/// `stage::context::assemble` reserves when trimming the prompt to fit.
const RUN_MAX_TOKENS: u32 = 4_096;

#[derive(Parser)]
#[command(
    name = "dlt",
    version,
    about = "Terminal-first AI development lifecycle tool"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a delta workspace (.delta/) in the current repository.
    Init,
    /// Manage in-flight changes.
    Change {
        #[command(subcommand)]
        command: ChangeCommand,
    },
    /// Show the status of all in-flight changes.
    Status,
    /// Apply a change's deltas to truth and move it to .delta/archive/.
    Archive {
        slug: String,
        /// Archive even if acceptance-criteria checks are failing.
        /// Recorded on the archived artifacts' frontmatter.
        #[arg(long)]
        force: bool,
    },
    /// Run a stage: assemble its prompt, call the provider, validate and
    /// write the resulting artifact.
    Run(RunArgs),
    /// Run a change's acceptance-criteria checks.
    Verify(VerifyArgs),
    /// Run the tool loop: let an agent read/write/patch/search the repo
    /// and run commands to implement a change, gated by approval policy.
    Build(BuildArgs),
    /// Revert the most recent tool-loop write (see `.delta/journal/`).
    Undo,
}

#[derive(clap::Args)]
pub struct BuildArgs {
    /// Change slug whose proposal/design/tasks artifacts (whichever
    /// exist) provide the agent's starting context.
    change: String,
    /// Provider name from `[providers.<name>]` in config. Defaults to "default".
    #[arg(long)]
    provider: Option<String>,
    /// Hard cap on tool-call round-trips before stopping and reporting.
    #[arg(long, default_value_t = agent::DEFAULT_MAX_ITERATIONS)]
    max_iterations: u32,
}

#[derive(clap::Args)]
pub struct VerifyArgs {
    /// Change slug to verify. Omit to verify every in-flight change.
    slug: Option<String>,
    /// Rerun on file changes instead of exiting after one pass.
    #[arg(long)]
    watch: bool,
    /// Per-check timeout, in seconds.
    #[arg(long, default_value_t = verify::DEFAULT_TIMEOUT_SECS)]
    timeout: u64,
}

#[derive(clap::Args)]
pub struct RunArgs {
    /// Stage id to run, e.g. "proposal" or "design".
    stage: String,
    /// Change slug to run the stage for.
    #[arg(long)]
    change: String,
    /// Print the assembled prompt without calling the provider — how you
    /// debug context assembly.
    #[arg(long)]
    dry_run: bool,
    /// Provider name from `[providers.<name>]` in config. Defaults to "default".
    #[arg(long)]
    provider: Option<String>,
    /// One-off rigor override for this run only; does not persist to the change.
    #[arg(long)]
    rigor: Option<Rigor>,
}

#[derive(Subcommand)]
pub enum ChangeCommand {
    /// Start a new change.
    New {
        slug: String,
        /// Force a rigor classification instead of inferring it from `git diff`.
        #[arg(long)]
        rigor: Option<Rigor>,
        /// What you want built — seeds the proposal so `dlt run proposal`
        /// has something to expand on instead of a bare placeholder.
        /// Omit it and edit `.delta/changes/<slug>/proposal.md` by hand
        /// instead, if you'd rather.
        #[arg(long)]
        description: Option<String>,
    },
    /// List in-flight changes.
    List,
}

pub fn dispatch(command: &Command, repo_root: &Path, config: &Config) -> Result<(), CliError> {
    match command {
        Command::Init => cmd_init(repo_root),
        Command::Change { command } => match command {
            ChangeCommand::New {
                slug,
                rigor,
                description,
            } => cmd_change_new(repo_root, slug, *rigor, description.as_deref()),
            ChangeCommand::List => cmd_change_list(repo_root),
        },
        Command::Status => cmd_status(repo_root),
        Command::Archive { slug, force } => cmd_archive(repo_root, slug, *force),
        Command::Run(args) => cmd_run(repo_root, config, args),
        Command::Verify(args) => cmd_verify(repo_root, args),
        Command::Build(args) => cmd_build(repo_root, config, args),
        Command::Undo => cmd_undo(repo_root),
    }
}

fn cmd_init(repo_root: &Path) -> Result<(), CliError> {
    let workspace = Workspace::init(repo_root)?;
    println!(
        "Initialized delta workspace at {}",
        workspace.root().display()
    );
    for found in Workspace::detect_interop(repo_root) {
        println!("note: found existing '{found}/' directory; interop import is not yet supported.");
    }
    Ok(())
}

fn cmd_change_new(
    repo_root: &Path,
    slug: &str,
    rigor_override: Option<Rigor>,
    description: Option<&str>,
) -> Result<(), CliError> {
    let workspace = Workspace::discover(repo_root)?;
    let stages = stage::load_all(workspace.store())?;
    let rigor = rigor_override.unwrap_or_else(|| stage::classify::classify(repo_root));
    change::new_change(
        workspace.store(),
        slug,
        Utc::now(),
        &stages,
        rigor,
        description,
    )?;
    println!("Created change '{slug}' (.delta/changes/{slug}), rigor: {rigor}");
    Ok(())
}

fn cmd_change_list(repo_root: &Path) -> Result<(), CliError> {
    let workspace = Workspace::discover(repo_root)?;
    let slugs = change::list_changes(workspace.store())?;
    if slugs.is_empty() {
        println!("no changes");
    } else {
        for slug in slugs {
            println!("{slug}");
        }
    }
    Ok(())
}

fn cmd_status(repo_root: &Path) -> Result<(), CliError> {
    let workspace = Workspace::discover(repo_root)?;
    let stages = stage::load_all(workspace.store())?;
    let slugs = change::list_changes(workspace.store())?;
    println!("{:<24}{:<10}{:<10}AGE", "CHANGE", "STAGE", "STATE");
    for slug in slugs {
        let status = change::change_status(workspace.store(), &slug, Utc::now(), &stages)?;
        println!(
            "{:<24}{:<10}{:<10}{}",
            status.slug,
            status.stage,
            format_state(status.state),
            change::humanize_duration(status.age)
        );
    }
    Ok(())
}

fn cmd_archive(repo_root: &Path, slug: &str, force: bool) -> Result<(), CliError> {
    let workspace = Workspace::discover(repo_root)?;
    let stages = stage::load_all(workspace.store())?;

    let timeout = Duration::from_secs(verify::DEFAULT_TIMEOUT_SECS);
    let results = verify::verify_change(workspace.store(), repo_root, slug, &stages, timeout)?;
    let failed: Vec<&verify::CriterionResult> = results.iter().filter(|r| !r.passed).collect();
    if !failed.is_empty() {
        for result in &failed {
            eprintln!("  [FAIL] {}", result.description);
        }
        if !force {
            return Err(CliError::ChecksFailed {
                failed: failed.len(),
                total: results.len(),
            });
        }
        change::mark_verify_forced(workspace.store(), slug)?;
        eprintln!(
            "note: archiving with {} failing check(s), forced via --force",
            failed.len()
        );
    }

    change::archive_change(workspace.store(), slug, &stages)?;
    println!("Archived change '{slug}'");
    Ok(())
}

fn cmd_verify(repo_root: &Path, args: &VerifyArgs) -> Result<(), CliError> {
    let workspace = Workspace::discover(repo_root)?;
    let stages = stage::load_all(workspace.store())?;
    let timeout = Duration::from_secs(args.timeout);

    if args.watch {
        verify::watch_and_rerun(repo_root, || {
            match run_verify_once(
                &workspace,
                repo_root,
                &stages,
                args.slug.as_deref(),
                timeout,
            ) {
                Ok((total, failed)) => println!(
                    "\n{} checked, {failed} failing ({} passing)",
                    total,
                    total - failed
                ),
                Err(err) => eprintln!("error: {err}"),
            }
        })?;
        return Ok(());
    }

    let (total, failed) = run_verify_once(
        &workspace,
        repo_root,
        &stages,
        args.slug.as_deref(),
        timeout,
    )?;
    if failed > 0 {
        return Err(CliError::ChecksFailed { failed, total });
    }
    Ok(())
}

/// Run every check for `slug` (or every in-flight change if `None`),
/// printing pass/fail per criterion, and return `(total, failed)`.
fn run_verify_once(
    workspace: &Workspace,
    repo_root: &Path,
    stages: &[StageDefinition],
    slug: Option<&str>,
    timeout: Duration,
) -> Result<(usize, usize), CliError> {
    let slugs: Vec<String> = match slug {
        Some(s) => vec![s.to_string()],
        None => change::list_changes(workspace.store())?,
    };

    let mut total = 0;
    let mut failed = 0;
    for slug in &slugs {
        let results = verify::verify_change(workspace.store(), repo_root, slug, stages, timeout)?;
        if results.is_empty() {
            println!("{slug}: no acceptance criteria with `verify:` checks found");
            continue;
        }
        println!("{slug}:");
        for result in &results {
            total += 1;
            if result.passed {
                println!("  [pass] {}", result.description);
            } else {
                failed += 1;
                println!("  [FAIL] {}", result.description);
                for line in result.detail.lines() {
                    println!("         {line}");
                }
            }
        }
    }
    Ok((total, failed))
}

/// Build the agent's starting context from whichever of a change's
/// proposal/design/tasks artifacts already exist. Not tied to the
/// runtime stage graph the way `dlt run` is — the tool loop just wants
/// whatever spec material is available, in the fixed order a reader
/// would want it, not a stage-YAML-driven dependency walk.
fn build_context(store: &dyn Store, slug: &str) -> Result<String, CliError> {
    let mut context = String::new();
    for stage_id in ["proposal", "design", "tasks"] {
        if let Some(body) = change::read_artifact_body(store, slug, stage_id)? {
            context.push_str(&format!("## {stage_id}\n\n{body}\n\n"));
        }
    }
    if context.is_empty() {
        context = format!("No proposal/design/tasks artifacts exist yet for change '{slug}'.");
    }
    Ok(context)
}

/// Prints tool-loop progress as it happens: text deltas stream straight
/// to stdout (like `run_stage`'s completions do), tool calls/results go
/// to stderr so stdout stays a clean transcript of the model's own words.
#[derive(Default)]
struct StreamingObserver;

impl AgentObserver for StreamingObserver {
    fn on_text_delta(&mut self, text: &str) {
        print!("{text}");
        std::io::stdout().flush().ok();
    }

    fn on_tool_call(&mut self, call: &ToolCall) {
        eprintln!("\n--- tool call: {} {} ---", call.tool, call.input);
    }

    fn on_tool_result(&mut self, outcome: &ToolOutcome) {
        eprintln!(
            "--- tool result ({}) ---\n{}",
            if outcome.success { "ok" } else { "failed" },
            outcome.output
        );
    }
}

fn cmd_build(repo_root: &Path, config: &Config, args: &BuildArgs) -> Result<(), CliError> {
    let workspace = Workspace::discover(repo_root)?;
    if !change::list_changes(workspace.store())?
        .iter()
        .any(|slug| slug == &args.change)
    {
        return Err(crate::error::ChangeError::NotFound {
            slug: args.change.clone(),
        }
        .into());
    }

    let context = build_context(workspace.store(), &args.change)?;
    let agents_md = std::fs::read_to_string(repo_root.join("AGENTS.md")).unwrap_or_default();
    let system_prompt = agent::build_system_prompt(&agents_md);
    let initial_message = format!(
        "Implement the change '{}' described below. Use the available tools to \
         read, search, and modify the repository as needed. When you are done, \
         respond with a final plain-text summary of what you did (no tool_call \
         block).\n\n{context}",
        args.change
    );

    let provider_name = args.provider.as_deref().unwrap_or("default");
    let loaded_provider = provider::load(config, provider_name)?;
    eprintln!(
        "note: running the tool loop for change '{}' via provider '{}' (max {} iterations)",
        args.change,
        loaded_provider.name(),
        args.max_iterations
    );

    let runtime = tokio::runtime::Runtime::new()?;
    let mut observer = StreamingObserver;
    let outcome = runtime.block_on(agent::run_loop(
        &loaded_provider,
        system_prompt,
        initial_message,
        repo_root,
        workspace.store(),
        config,
        &tools::StdinApprover,
        args.max_iterations,
        &mut observer,
    ))?;

    println!();
    eprintln!(
        "note: tool loop finished after {} iteration(s)",
        outcome.iterations
    );
    Ok(())
}

fn cmd_undo(repo_root: &Path) -> Result<(), CliError> {
    let workspace = Workspace::discover(repo_root)?;
    let reverted = tools::journal::undo_last(workspace.store(), repo_root)?;
    println!("Reverted {}", reverted.display());
    Ok(())
}

fn cmd_run(repo_root: &Path, config: &Config, args: &RunArgs) -> Result<(), CliError> {
    let workspace = Workspace::discover(repo_root)?;
    let stages = stage::load_all(workspace.store())?;
    let stage_def =
        stages
            .iter()
            .find(|s| s.id == args.stage)
            .ok_or_else(|| StageError::NotFound {
                id: args.stage.clone(),
            })?;

    let effective_rigor = match args.rigor {
        Some(rigor) => rigor,
        None => change::change_rigor(workspace.store(), &stages, &args.change)?,
    };

    if stage_def.min_rigor > effective_rigor {
        write_not_applicable(workspace.store(), &stages, &args.change, stage_def)?;
        println!(
            "Skipped stage '{}' for change '{}': requires {} rigor, change is {} (n/a)",
            stage_def.id, args.change, stage_def.min_rigor, effective_rigor
        );
        return Ok(());
    }

    let provider_name = args.provider.as_deref().unwrap_or("default");

    if args.dry_run {
        // No API key lookup here on purpose: --dry-run must work without
        // live credentials, since it's how context assembly gets debugged.
        let dry_run_provider = provider::load_for_dry_run(config, provider_name)?;
        let assembled = stage::context::assemble(
            workspace.store(),
            repo_root,
            stage_def,
            &args.change,
            &dry_run_provider,
        )?;
        report_assembly(&assembled);
        println!("{}", assembled.prompt);
        return Ok(());
    }

    let loaded_provider = provider::load(config, provider_name)?;
    let assembled = stage::context::assemble(
        workspace.store(),
        repo_root,
        stage_def,
        &args.change,
        &loaded_provider,
    )?;
    report_assembly(&assembled);

    eprintln!(
        "note: running stage '{}' via provider '{}'",
        stage_def.id,
        loaded_provider.name()
    );

    let runtime = tokio::runtime::Runtime::new()?;
    let body = runtime.block_on(run_stage(&loaded_provider, &assembled.prompt))?;

    let failures = stage::validate::validate(&stage_def.output, &body);
    let status = if failures.is_empty() {
        ArtifactStatus::Valid
    } else {
        ArtifactStatus::Failed
    };
    change::write_stage_artifact(
        workspace.store(),
        &stages,
        &args.change,
        change::StageWrite {
            stage_id: &stage_def.id,
            body: &body,
            status,
            rigor: None,
            now: Utc::now(),
        },
    )?;

    if !failures.is_empty() {
        for failure in &failures {
            eprintln!("  - {failure}");
        }
        return Err(StageError::ValidationFailed {
            id: stage_def.id.clone(),
            failures: failures.join("; "),
        }
        .into());
    }

    println!(
        "Stage '{}' complete for change '{}'.",
        stage_def.id, args.change
    );
    Ok(())
}

fn report_assembly(assembled: &stage::context::Assembled) {
    if !assembled.dropped.is_empty() {
        eprintln!(
            "note: dropped from context to fit the token budget: {}",
            assembled.dropped.join(", ")
        );
    }
    eprintln!("note: assembled prompt is {} tokens", assembled.token_count);
}

fn write_not_applicable(
    store: &dyn Store,
    stages: &[StageDefinition],
    slug: &str,
    stage: &StageDefinition,
) -> Result<(), CliError> {
    let body = format!(
        "Skipped: this change's rigor does not require the '{}' stage.\n",
        stage.id
    );
    change::write_stage_artifact(
        store,
        stages,
        slug,
        change::StageWrite {
            stage_id: &stage.id,
            body: &body,
            status: ArtifactStatus::NotApplicable,
            rigor: None,
            now: Utc::now(),
        },
    )?;
    Ok(())
}

/// Stream a completion for `prompt`, printing it to stdout as it arrives
/// and returning the accumulated body.
async fn run_stage(provider: &AnyProvider, prompt: &str) -> Result<String, CliError> {
    let cancel = tokio_util::sync::CancellationToken::new();
    let request = Request {
        system: None,
        messages: vec![Message {
            role: Role::User,
            content: prompt.to_string(),
        }],
        max_tokens: RUN_MAX_TOKENS,
    };
    let mut stream = provider.stream(request, cancel).await?;
    let mut body = String::new();
    while let Some(delta) = stream.next().await {
        let delta = delta?;
        print!("{}", delta.text);
        std::io::stdout().flush().ok();
        body.push_str(&delta.text);
    }
    println!();
    Ok(body)
}

fn format_state(state: ArtifactStatus) -> &'static str {
    match state {
        ArtifactStatus::Pending => "pending",
        ArtifactStatus::Valid => "valid",
        ArtifactStatus::Stale => "stale",
        ArtifactStatus::Failed => "failed",
        ArtifactStatus::NotApplicable => "n/a",
    }
}
