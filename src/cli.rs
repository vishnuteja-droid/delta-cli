//! Command-line surface: clap argument/subcommand definitions and dispatch.
//! Maps parsed commands to calls into the other modules and translates
//! their results into process exit codes. Holds no business logic itself.

use std::io::Write as _;
use std::path::Path;

use chrono::Utc;
use clap::{Parser, Subcommand};
use futures::StreamExt;

use crate::change::{self, ArtifactStatus};
use crate::config::Config;
use crate::error::{CliError, StageError};
use crate::provider::{self, AnyProvider, Message, Provider, Request, Role};
use crate::stage::{self, Rigor, StageDefinition};
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
    Archive { slug: String },
    /// Run a stage: assemble its prompt, call the provider, validate and
    /// write the resulting artifact.
    Run(RunArgs),
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
    },
    /// List in-flight changes.
    List,
}

pub fn dispatch(command: &Command, repo_root: &Path, config: &Config) -> Result<(), CliError> {
    match command {
        Command::Init => cmd_init(repo_root),
        Command::Change { command } => match command {
            ChangeCommand::New { slug, rigor } => cmd_change_new(repo_root, slug, *rigor),
            ChangeCommand::List => cmd_change_list(repo_root),
        },
        Command::Status => cmd_status(repo_root),
        Command::Archive { slug } => cmd_archive(repo_root, slug),
        Command::Run(args) => cmd_run(repo_root, config, args),
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
) -> Result<(), CliError> {
    let workspace = Workspace::discover(repo_root)?;
    let stages = stage::load_all(workspace.store())?;
    let rigor = rigor_override.unwrap_or_else(|| stage::classify::classify(repo_root));
    change::new_change(workspace.store(), slug, Utc::now(), &stages, rigor)?;
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

fn cmd_archive(repo_root: &Path, slug: &str) -> Result<(), CliError> {
    let workspace = Workspace::discover(repo_root)?;
    let stages = stage::load_all(workspace.store())?;
    change::archive_change(workspace.store(), slug, &stages)?;
    println!("Archived change '{slug}'");
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
