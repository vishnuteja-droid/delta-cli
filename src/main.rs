#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod change;
mod cli;
mod config;
mod error;
mod provider;
mod stage;
mod tools;
mod tui;
mod verify;
mod workspace;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> anyhow::Result<ExitCode> {
    let cli = cli::Cli::parse();
    let repo_root = std::env::current_dir()?;
    let config = config::Config::load(&repo_root)?;

    match cli::dispatch(&cli.command, &repo_root, &config) {
        Ok(()) => Ok(ExitCode::SUCCESS),
        Err(err) => {
            eprintln!("error: {err}");
            Ok(ExitCode::from(err.exit_code()))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {
        assert_eq!(2 + 2, 4);
    }
}
