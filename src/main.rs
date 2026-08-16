#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod change;
mod cli;
mod config;
mod error;
mod provider;
mod stage;
mod tui;
mod verify;
mod workspace;

use anyhow::Result;

fn main() -> Result<()> {
    let repo_root = std::env::current_dir()?;
    let _config = config::Config::load(&repo_root)?;
    println!("dlt — skeleton (prompt 0)");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {
        assert_eq!(2 + 2, 4);
    }
}
