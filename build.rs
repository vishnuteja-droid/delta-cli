//! Bakes the commit hash and build target triple into the binary at
//! compile time, so `dlt --version` can report both per PLAN.md prompt 7
//! ("dlt --version reports commit hash and build target") without any
//! runtime cost or dependency on `git` being present on the machine that
//! *runs* `dlt` (only the machine that *builds* it).

use std::process::Command;

fn main() {
    let hash = git_short_hash().unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=DLT_GIT_HASH={hash}");

    // Cargo sets TARGET for build scripts to the triple being compiled
    // for — exactly what PLAN.md asks `--version` to report, and stable
    // across cross-compilation (unlike `cfg!(target_...)`, which would
    // describe the build script's own host, not the binary being built).
    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=DLT_BUILD_TARGET={target}");

    // Re-run only when the checked-out commit actually moves, not on
    // every `cargo build` — a release tarball built without a `.git`
    // directory at all just falls back to "unknown" above, which is
    // fine: it's still a truthful answer.
    if let Some(git_dir) = git_dir() {
        println!("cargo:rerun-if-changed={git_dir}/HEAD");
        println!("cargo:rerun-if-changed={git_dir}/index");
    }
}

fn git_short_hash() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn git_dir() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
