//! Regex search across the repository tree for the `search` tool.
//! Walks via `ignore::WalkBuilder` (respects `.gitignore`/`.ignore` and
//! skips `.git`, same convention as a plain `rg` invocation) and matches
//! each file with `grep-searcher` + `grep-regex`, which handles binary
//! detection and line splitting so this module doesn't have to.

use std::path::Path;

use grep_regex::RegexMatcher;
use grep_searcher::Searcher;
use grep_searcher::sinks::UTF8;
use ignore::WalkBuilder;

use crate::error::ToolError;

/// Hard cap on matches returned — a regex like `.` over a large repo
/// could otherwise return an unbounded, budget-blowing wall of output.
pub const MAX_MATCHES: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Repo-root-relative path.
    pub path: String,
    pub line_number: u64,
    pub line: String,
}

/// Search every non-ignored file under `repo_root.join(dir)` for
/// `pattern`, stopping once `MAX_MATCHES` matches have been found.
pub fn search(repo_root: &Path, dir: &str, pattern: &str) -> Result<Vec<SearchMatch>, ToolError> {
    let matcher = RegexMatcher::new(pattern)
        .map_err(|e| ToolError::Search(format!("invalid pattern {pattern:?}: {e}")))?;
    let root = repo_root.join(dir);

    let mut matches = Vec::new();
    let mut searcher = Searcher::new();
    // `require_git(false)`: `.gitignore` should apply because it's
    // declared, not only when the target happens to sit inside an
    // actual `.git` checkout — `dlt` runs against ordinary repo trees,
    // and gating on `.git`'s presence would make this tool's behavior
    // depend on incidental repo state rather than the ignore file itself.
    let mut builder = WalkBuilder::new(&root);
    builder.hidden(false).require_git(false);
    'walk: for entry in builder.build() {
        let entry = entry.map_err(|e| ToolError::Search(format!("walk error: {e}")))?;
        if entry.file_type().is_none_or(|t| !t.is_file()) {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(repo_root).unwrap_or(path);
        let rel_display = rel.display().to_string();

        let result = searcher.search_path(
            &matcher,
            path,
            UTF8(|line_number, line| {
                matches.push(SearchMatch {
                    path: rel_display.clone(),
                    line_number,
                    line: line.trim_end_matches(['\n', '\r']).to_string(),
                });
                Ok(matches.len() < MAX_MATCHES)
            }),
        );
        // Binary files and unreadable files are skipped, not fatal —
        // a search tool that aborts on the first binary asset in the
        // repo would be useless.
        if result.is_err() {
            continue;
        }
        if matches.len() >= MAX_MATCHES {
            break 'walk;
        }
    }
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(dir: &TempDir, path: &str, content: &str) {
        let full = dir.path().join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, content).unwrap();
    }

    #[test]
    fn finds_matches_across_files() {
        let dir = TempDir::new().unwrap();
        write(&dir, "a.txt", "hello world\nfoo bar\n");
        write(&dir, "sub/b.txt", "another hello here\n");
        write(&dir, "c.txt", "nothing relevant\n");

        let mut results = search(dir.path(), ".", "hello").unwrap();
        results.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "a.txt");
        assert_eq!(results[0].line_number, 1);
        assert_eq!(results[1].path, "sub/b.txt");
    }

    #[test]
    fn respects_gitignore() {
        let dir = TempDir::new().unwrap();
        write(&dir, ".gitignore", "ignored/\n");
        write(&dir, "ignored/secret.txt", "hello\n");
        write(&dir, "kept.txt", "hello\n");

        let results = search(dir.path(), ".", "hello").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "kept.txt");
    }

    #[test]
    fn invalid_regex_is_a_clean_error() {
        let dir = TempDir::new().unwrap();
        let err = search(dir.path(), ".", "(unclosed").unwrap_err();
        assert!(matches!(err, ToolError::Search(_)));
    }

    #[test]
    fn caps_at_max_matches() {
        let dir = TempDir::new().unwrap();
        let content = "match\n".repeat(MAX_MATCHES + 50);
        write(&dir, "many.txt", &content);

        let results = search(dir.path(), ".", "match").unwrap();
        assert_eq!(results.len(), MAX_MATCHES);
    }
}
