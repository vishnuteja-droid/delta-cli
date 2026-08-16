//! Command-line surface: clap argument/subcommand definitions and dispatch.
//! Maps parsed commands to calls into the other modules and translates
//! their results into process exit codes. Holds no business logic itself.

use std::path::Path;

use chrono::Utc;
use clap::{Parser, Subcommand};

use crate::change::{self, ArtifactStatus};
use crate::error::CliError;
use crate::workspace::Workspace;

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
}

#[derive(Subcommand)]
pub enum ChangeCommand {
    /// Start a new change.
    New { slug: String },
    /// List in-flight changes.
    List,
}

pub fn dispatch(command: &Command, repo_root: &Path) -> Result<(), CliError> {
    match command {
        Command::Init => cmd_init(repo_root),
        Command::Change { command } => match command {
            ChangeCommand::New { slug } => cmd_change_new(repo_root, slug),
            ChangeCommand::List => cmd_change_list(repo_root),
        },
        Command::Status => cmd_status(repo_root),
        Command::Archive { slug } => cmd_archive(repo_root, slug),
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

fn cmd_change_new(repo_root: &Path, slug: &str) -> Result<(), CliError> {
    let workspace = Workspace::discover(repo_root)?;
    change::new_change(workspace.store(), slug, Utc::now())?;
    println!("Created change '{slug}' (.delta/changes/{slug})");
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
    let slugs = change::list_changes(workspace.store())?;
    println!("{:<24}{:<10}{:<10}AGE", "CHANGE", "STAGE", "STATE");
    for slug in slugs {
        let status = change::change_status(workspace.store(), &slug, Utc::now())?;
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
    change::archive_change(workspace.store(), slug)?;
    println!("Archived change '{slug}'");
    Ok(())
}

fn format_state(state: ArtifactStatus) -> &'static str {
    match state {
        ArtifactStatus::Pending => "pending",
        ArtifactStatus::Valid => "valid",
        ArtifactStatus::Stale => "stale",
        ArtifactStatus::Failed => "failed",
    }
}
