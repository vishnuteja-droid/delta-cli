//! Adaptive rigor classification: how "trivial", "standard", or "deep" a
//! change looks from its uncommitted diff. Used as the default when
//! `dlt change new`/`dlt run` aren't given an explicit `--rigor`.

use std::path::Path;

use crate::stage::Rigor;

/// Markers that a line adds (or changes) a language's public interface.
/// Checked only against added lines (`+`-prefixed, excluding the `+++`
/// file header) in `git diff HEAD`.
const PUBLIC_MARKERS: [&str; 10] = [
    "pub fn",
    "pub struct",
    "pub enum",
    "pub trait",
    "pub const",
    "export function",
    "export class",
    "export const",
    "export interface",
    "def ",
];

/// Classify the change currently reflected in `repo_root`'s working tree
/// (uncommitted changes vs. `HEAD`). Falls back to `Rigor::Trivial` if
/// git is unavailable or the diff can't be read — never blocks progress,
/// and trivial is the cheapest/safest guess in that case.
pub fn classify(repo_root: &Path) -> Rigor {
    let Some(diff) = git_diff(repo_root) else {
        return Rigor::Trivial;
    };

    let files_touched = diff
        .lines()
        .filter(|line| line.starts_with("diff --git"))
        .count();

    let public_interface_changed = diff
        .lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .any(|line| PUBLIC_MARKERS.iter().any(|marker| line.contains(marker)));

    if public_interface_changed || files_touched > 10 {
        Rigor::Deep
    } else if files_touched > 1 {
        Rigor::Standard
    } else {
        Rigor::Trivial
    }
}

fn git_diff(repo_root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("diff")
        .arg("HEAD")
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn git(repo: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(repo)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn init_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        git(dir.path(), &["init", "-q"]);
        git(dir.path(), &["config", "user.email", "test@example.com"]);
        git(dir.path(), &["config", "user.name", "Test"]);
        std::fs::write(dir.path().join("a.txt"), "hello\n").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-q", "-m", "init"]);
        dir
    }

    #[test]
    fn no_diff_is_trivial() {
        let repo = init_repo();
        assert_eq!(classify(repo.path()), Rigor::Trivial);
    }

    #[test]
    fn single_file_non_interface_change_is_trivial() {
        let repo = init_repo();
        std::fs::write(repo.path().join("a.txt"), "hello again\n").unwrap();
        assert_eq!(classify(repo.path()), Rigor::Trivial);
    }

    #[test]
    fn multiple_files_touched_is_standard() {
        let repo = init_repo();
        std::fs::write(repo.path().join("a.txt"), "changed\n").unwrap();
        std::fs::write(repo.path().join("b.txt"), "new\n").unwrap();
        git(repo.path(), &["add", "b.txt"]);
        assert_eq!(classify(repo.path()), Rigor::Standard);
    }

    #[test]
    fn public_interface_change_is_deep() {
        let repo = init_repo();
        std::fs::write(repo.path().join("a.txt"), "pub fn new_thing() {}\n").unwrap();
        assert_eq!(classify(repo.path()), Rigor::Deep);
    }

    #[test]
    fn nonexistent_repo_falls_back_to_trivial() {
        assert_eq!(classify(Path::new("/nonexistent/path/xyz")), Rigor::Trivial);
    }
}
