//! Executable verification: this is the product. Every other stage in
//! this tool produces text that a human has to read and judge; this
//! module is the one that checks a change's declared acceptance
//! criteria against reality.
//!
//! A change's artifacts may contain a `## Acceptance Criteria` section
//! with a markdown checklist. Any item followed by an indented inline
//! code span of the form `` `verify: <check>` `` becomes an executable
//! [`Criterion`] — plain checklist items with no such annotation are
//! left for a human and are not collected. Three check kinds are
//! supported:
//!
//! ```text
//! cmd "<command>" [expect exit <n>] [contains "<text>"] [not_contains "<text>"]
//! file "<path>" (exists | contains "<text>" | matches "<regex>")
//! git changed "<glob>"
//! ```
//!
//! `cmd` runs through a shell (`sh -c` / `cmd /C`) so redirections like
//! `2>&1` in the check spec work as written — this is intentional and
//! only ever executes commands the repository's own authors wrote into
//! their spec, the same trust boundary as a Makefile or CI config.

use std::path::Path;
use std::time::Duration;

use regex::Regex;

use crate::change;
use crate::error::VerifyError;
use crate::stage::StageDefinition;
use crate::workspace::Store;

pub const DEFAULT_TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Clone)]
pub struct Criterion {
    pub description: String,
    /// `Err` means the `verify:` spec itself failed to parse; it still
    /// becomes a (failing) [`CriterionResult`] so a typo in a check is
    /// visible rather than silently ignored.
    pub check: Result<Check, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    Cmd {
        command: String,
        expect_exit: Option<i32>,
        contains: Option<String>,
        not_contains: Option<String>,
    },
    FileExists {
        path: String,
    },
    FileContains {
        path: String,
        text: String,
    },
    FileMatches {
        path: String,
        pattern: String,
    },
    GitChanged {
        glob: String,
    },
}

#[derive(Debug, Clone)]
pub struct CriterionResult {
    pub description: String,
    pub passed: bool,
    /// Failing output/reason; empty when `passed`.
    pub detail: String,
}

/// Run every acceptance-criterion check declared across `slug`'s existing
/// artifacts (in `stages`' topological order).
pub fn verify_change(
    store: &dyn Store,
    repo_root: &Path,
    slug: &str,
    stages: &[StageDefinition],
    timeout: Duration,
) -> Result<Vec<CriterionResult>, VerifyError> {
    let mut results = Vec::new();
    for stage in stages {
        let Some(body) = change::read_artifact_body(store, slug, &stage.id)? else {
            continue;
        };
        for criterion in parse_criteria(&body) {
            results.push(run_criterion(&criterion, repo_root, timeout));
        }
    }
    Ok(results)
}

fn run_criterion(criterion: &Criterion, repo_root: &Path, timeout: Duration) -> CriterionResult {
    let (passed, detail) = match &criterion.check {
        Err(reason) => (false, reason.clone()),
        Ok(check) => match run_check(check, repo_root, timeout) {
            Ok(()) => (true, String::new()),
            Err(detail) => (false, detail),
        },
    };
    CriterionResult {
        description: criterion.description.clone(),
        passed,
        detail,
    }
}

// ---------------------------------------------------------------------
// Parsing `## Acceptance Criteria` checklists
// ---------------------------------------------------------------------

/// Lines belonging to a level-`level` heading named `name` (case
/// insensitive): from just after the heading to the next heading at the
/// same or a shallower level, or the end of `lines`.
fn find_section<'a>(lines: &[&'a str], name: &str) -> Option<Vec<&'a str>> {
    let (start, level) = lines.iter().enumerate().find_map(|(i, line)| {
        let trimmed = line.trim_start();
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if level == 0 {
            return None;
        }
        if trimmed[level..].trim().eq_ignore_ascii_case(name) {
            Some((i + 1, level))
        } else {
            None
        }
    })?;

    let end = lines[start..]
        .iter()
        .position(|line| {
            let trimmed = line.trim_start();
            let this_level = trimmed.chars().take_while(|c| *c == '#').count();
            this_level != 0 && this_level <= level
        })
        .map_or(lines.len(), |offset| start + offset);

    Some(lines[start..end].to_vec())
}

/// The text after `- [ ]`/`- [x]`/`- [X]`, or `None` if `line` isn't a
/// checklist item.
fn checklist_item_text(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("- [")?;
    let mut chars = rest.chars();
    let mark = chars.next()?;
    if mark != ' ' && mark != 'x' && mark != 'X' {
        return None;
    }
    chars.as_str().strip_prefix(']').map(str::trim)
}

/// The text after `verify:` in a line that is *only* an inline code span
/// `` `verify: ...` `` (leading/trailing whitespace aside).
fn extract_verify_spec(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('`')?.strip_suffix('`')?;
    inner.strip_prefix("verify:").map(|s| s.trim().to_string())
}

pub fn parse_criteria(body: &str) -> Vec<Criterion> {
    let lines: Vec<&str> = body.lines().collect();
    let Some(section) = find_section(&lines, "Acceptance Criteria") else {
        return Vec::new();
    };

    let mut criteria = Vec::new();
    let mut i = 0;
    while i < section.len() {
        let Some(description) = checklist_item_text(section[i]) else {
            i += 1;
            continue;
        };
        let mut j = i + 1;
        while j < section.len() && checklist_item_text(section[j]).is_none() {
            j += 1;
        }
        if let Some(spec) = section[i + 1..j]
            .iter()
            .find_map(|line| extract_verify_spec(line))
        {
            criteria.push(Criterion {
                description: description.to_string(),
                check: parse_check(&spec),
            });
        }
        i = j;
    }
    criteria
}

// ---------------------------------------------------------------------
// Parsing a single `verify:` check spec
// ---------------------------------------------------------------------

fn tokenize(spec: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = spec.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '"' {
            chars.next();
            let mut s = String::new();
            for c in chars.by_ref() {
                if c == '"' {
                    break;
                }
                s.push(c);
            }
            tokens.push(s);
        } else {
            let mut s = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                s.push(c);
                chars.next();
            }
            tokens.push(s);
        }
    }
    tokens
}

fn parse_check(spec: &str) -> Result<Check, String> {
    let tokens = tokenize(spec);
    let mut iter = tokens.iter();
    let kind = iter.next().ok_or("empty verify check")?;
    match kind.as_str() {
        "cmd" => parse_cmd_check(&mut iter),
        "file" => parse_file_check(&mut iter),
        "git" => parse_git_check(&mut iter),
        other => Err(format!(
            "unknown check kind {other:?} (expected cmd, file, or git)"
        )),
    }
}

fn parse_cmd_check(iter: &mut std::slice::Iter<'_, String>) -> Result<Check, String> {
    let command = iter
        .next()
        .ok_or("cmd check missing a command string")?
        .clone();
    let mut expect_exit = None;
    let mut contains = None;
    let mut not_contains = None;
    loop {
        match iter.next().map(String::as_str) {
            None => break,
            Some("expect") => match iter.next().map(String::as_str) {
                Some("exit") => {
                    let n = iter.next().ok_or("\"expect exit\" missing a code")?;
                    expect_exit = Some(
                        n.parse::<i32>()
                            .map_err(|_| format!("invalid exit code {n:?}"))?,
                    );
                }
                other => return Err(format!("expected \"exit\" after \"expect\", got {other:?}")),
            },
            Some("contains") => {
                contains = Some(
                    iter.next()
                        .ok_or("\"contains\" missing a text argument")?
                        .clone(),
                );
            }
            Some("not_contains") => {
                not_contains = Some(
                    iter.next()
                        .ok_or("\"not_contains\" missing a text argument")?
                        .clone(),
                );
            }
            Some(other) => return Err(format!("unknown cmd clause {other:?}")),
        }
    }
    if expect_exit.is_none() && contains.is_none() && not_contains.is_none() {
        return Err(
            "cmd check declares no assertions (expect exit / contains / not_contains)".to_string(),
        );
    }
    Ok(Check::Cmd {
        command,
        expect_exit,
        contains,
        not_contains,
    })
}

fn parse_file_check(iter: &mut std::slice::Iter<'_, String>) -> Result<Check, String> {
    let path = iter.next().ok_or("file check missing a path")?.clone();
    match iter.next().map(String::as_str) {
        Some("exists") => Ok(Check::FileExists { path }),
        Some("contains") => {
            let text = iter
                .next()
                .ok_or("\"contains\" missing a text argument")?
                .clone();
            Ok(Check::FileContains { path, text })
        }
        Some("matches") => {
            let pattern = iter
                .next()
                .ok_or("\"matches\" missing a regex argument")?
                .clone();
            Regex::new(&pattern).map_err(|e| format!("invalid regex {pattern:?}: {e}"))?;
            Ok(Check::FileMatches { path, pattern })
        }
        other => Err(format!(
            "expected exists/contains/matches after the file path, got {other:?}"
        )),
    }
}

fn parse_git_check(iter: &mut std::slice::Iter<'_, String>) -> Result<Check, String> {
    match iter.next().map(String::as_str) {
        Some("changed") => {
            let glob = iter.next().ok_or("\"git changed\" missing a glob")?.clone();
            globset::Glob::new(&glob).map_err(|e| format!("invalid glob {glob:?}: {e}"))?;
            Ok(Check::GitChanged { glob })
        }
        other => Err(format!("expected \"changed\" after \"git\", got {other:?}")),
    }
}

// ---------------------------------------------------------------------
// Running a single check
// ---------------------------------------------------------------------

fn run_check(check: &Check, repo_root: &Path, timeout: Duration) -> Result<(), String> {
    match check {
        Check::Cmd {
            command,
            expect_exit,
            contains,
            not_contains,
        } => run_cmd_check(
            command,
            *expect_exit,
            contains.as_deref(),
            not_contains.as_deref(),
            repo_root,
            timeout,
        ),
        Check::FileExists { path } => {
            if repo_root.join(path).exists() {
                Ok(())
            } else {
                Err(format!("{path} does not exist"))
            }
        }
        Check::FileContains { path, text } => {
            let content = read_file(repo_root, path)?;
            if content.contains(text.as_str()) {
                Ok(())
            } else {
                Err(format!("{path} does not contain {text:?}"))
            }
        }
        Check::FileMatches { path, pattern } => {
            let content = read_file(repo_root, path)?;
            let regex =
                Regex::new(pattern).map_err(|e| format!("invalid regex {pattern:?}: {e}"))?;
            if regex.is_match(&content) {
                Ok(())
            } else {
                Err(format!("{path} does not match /{pattern}/"))
            }
        }
        Check::GitChanged { glob } => {
            let changed = git_changed_files(repo_root)?;
            let matcher = globset::Glob::new(glob)
                .map_err(|e| format!("invalid glob {glob:?}: {e}"))?
                .compile_matcher();
            if changed.iter().any(|path| matcher.is_match(path)) {
                Ok(())
            } else if changed.is_empty() {
                Err(format!("no files changed (glob {glob:?} matched nothing)"))
            } else {
                Err(format!(
                    "no changed file matched glob {glob:?}; changed: {}",
                    changed.join(", ")
                ))
            }
        }
    }
}

/// Reads `path` and chomps a single trailing newline (`\n` or `\r\n`), so
/// `matches "...$"` behaves the way anyone writing that pattern expects
/// against a normal, one-trailing-newline text file — Rust's `regex`
/// crate's `$` does not match before a final `\n` by default.
fn read_file(repo_root: &Path, path: &str) -> Result<String, String> {
    let content = std::fs::read_to_string(repo_root.join(path))
        .map_err(|e| format!("failed to read {path}: {e}"))?;
    Ok(content
        .strip_suffix('\n')
        .map(|s| s.strip_suffix('\r').unwrap_or(s))
        .unwrap_or(&content)
        .to_string())
}

fn git_changed_files(repo_root: &Path) -> Result<Vec<String>, String> {
    let output = std::process::Command::new("git")
        .arg("diff")
        .arg("--name-only")
        .arg("HEAD")
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("failed to run git diff: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn run_cmd_check(
    command: &str,
    expect_exit: Option<i32>,
    contains: Option<&str>,
    not_contains: Option<&str>,
    repo_root: &Path,
    timeout: Duration,
) -> Result<(), String> {
    let output = run_cmd(command, repo_root, timeout)?;

    if let Some(expected) = expect_exit {
        let actual = output.status.code();
        if actual != Some(expected) {
            let got = actual.map_or_else(|| "terminated by signal".to_string(), |c| c.to_string());
            return Err(format!(
                "expected exit {expected}, got {got}\nstdout:\n{}\nstderr:\n{}",
                output.stdout, output.stderr
            ));
        }
    }
    if let Some(text) = contains
        && !output.stdout.contains(text)
    {
        return Err(format!(
            "stdout did not contain {text:?}\nstdout:\n{}",
            output.stdout
        ));
    }
    if let Some(text) = not_contains
        && output.stdout.contains(text)
    {
        return Err(format!(
            "stdout unexpectedly contained {text:?}\nstdout:\n{}",
            output.stdout
        ));
    }
    Ok(())
}

struct CmdOutput {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

/// Run `command` through a shell (so redirections in the spec work),
/// killing it if it doesn't finish within `timeout`.
fn run_cmd(command: &str, repo_root: &Path, timeout: Duration) -> Result<CmdOutput, String> {
    let mut cmd = shell_command(command);
    cmd.current_dir(repo_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn {command:?}: {e}"))?;
    let pid = child.id();

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => Ok(CmdOutput {
            status: output.status,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }),
        Ok(Err(io_err)) => Err(format!("failed to run {command:?}: {io_err}")),
        Err(_elapsed) => {
            kill_pid(pid);
            Err(format!("{command:?} timed out after {timeout:?}"))
        }
    }
}

#[cfg(unix)]
fn shell_command(command: &str) -> std::process::Command {
    use std::os::unix::process::CommandExt;
    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c").arg(command);
    // Its own process group, so a timeout can kill the whole subtree
    // (e.g. a shell that spawned a long-running child) rather than
    // orphaning it.
    cmd.process_group(0);
    cmd
}

#[cfg(windows)]
fn shell_command(command: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new("cmd");
    cmd.arg("/C").arg(command);
    cmd
}

#[cfg(unix)]
fn kill_pid(pid: u32) {
    // A negative pid targets the whole process group created above.
    let _ = std::process::Command::new("kill")
        .arg("-9")
        .arg(format!("-{pid}"))
        .status();
}

#[cfg(windows)]
fn kill_pid(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}

// ---------------------------------------------------------------------
// `dlt verify --watch`
// ---------------------------------------------------------------------

const WATCH_NOISE_DIRS: [&str; 4] = [".git", ".delta", "target", "node_modules"];

/// Watch `repo_root` and call `on_change` once immediately, then again
/// each time a relevant file changes (debounced so a burst of writes —
/// a `cargo build`, an editor save — triggers one rerun, not dozens).
/// Blocks until the watcher itself fails or its channel disconnects.
pub fn watch_and_rerun(repo_root: &Path, mut on_change: impl FnMut()) -> Result<(), VerifyError> {
    use notify::Watcher;

    let watch_err = |e: notify::Error| VerifyError::Watch {
        path: repo_root.display().to_string(),
        reason: e.to_string(),
    };

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = tx.send(event);
    })
    .map_err(watch_err)?;
    watcher
        .watch(repo_root, notify::RecursiveMode::Recursive)
        .map_err(watch_err)?;

    on_change();

    loop {
        let Ok(event) = rx.recv() else {
            return Ok(());
        };
        if !is_relevant_change(&event, repo_root) {
            continue;
        }
        while rx.recv_timeout(Duration::from_millis(300)).is_ok() {}
        on_change();
    }
}

fn is_relevant_change(event: &notify::Result<notify::Event>, repo_root: &Path) -> bool {
    let Ok(event) = event else { return false };
    event.paths.iter().any(|path| {
        path.strip_prefix(repo_root)
            .map(|rel| {
                !rel.components().any(|c| {
                    c.as_os_str()
                        .to_str()
                        .is_some_and(|name| WATCH_NOISE_DIRS.contains(&name))
                })
            })
            .unwrap_or(true)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{ARCHIVE_DIR, CHANGES_DIR, FsStore, TRUTH_DIR};
    use tempfile::TempDir;

    const PLAN_EXAMPLE: &str = r#"
## Acceptance Criteria
- [ ] Rejects tokens older than 24h
      `verify: cmd "cargo test auth::expiry" expect exit 0`
- [ ] Endpoint documented
      `verify: file "docs/api.md" contains "POST /auth/refresh"`
- [ ] No new public API without a doc comment
      `verify: cmd "cargo doc 2>&1" not_contains "missing documentation"`
"#;

    #[test]
    fn parses_the_plan_example_verbatim() {
        let criteria = parse_criteria(PLAN_EXAMPLE);
        assert_eq!(criteria.len(), 3);

        assert_eq!(criteria[0].description, "Rejects tokens older than 24h");
        assert_eq!(
            criteria[0].check.as_ref().unwrap(),
            &Check::Cmd {
                command: "cargo test auth::expiry".to_string(),
                expect_exit: Some(0),
                contains: None,
                not_contains: None,
            }
        );

        assert_eq!(criteria[1].description, "Endpoint documented");
        assert_eq!(
            criteria[1].check.as_ref().unwrap(),
            &Check::FileContains {
                path: "docs/api.md".to_string(),
                text: "POST /auth/refresh".to_string(),
            }
        );

        assert_eq!(
            criteria[2].description,
            "No new public API without a doc comment"
        );
        assert_eq!(
            criteria[2].check.as_ref().unwrap(),
            &Check::Cmd {
                command: "cargo doc 2>&1".to_string(),
                expect_exit: None,
                contains: None,
                not_contains: Some("missing documentation".to_string()),
            }
        );
    }

    #[test]
    fn checklist_items_without_verify_are_skipped() {
        let body = "## Acceptance Criteria\n- [ ] A human has to judge this one\n- [ ] This one is automated\n      `verify: file \"x\" exists`\n";
        let criteria = parse_criteria(body);
        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria[0].description, "This one is automated");
    }

    #[test]
    fn no_acceptance_criteria_section_yields_no_criteria() {
        assert!(parse_criteria("# Proposal\n\nJust some prose.\n").is_empty());
    }

    #[test]
    fn parse_check_rejects_unknown_kind() {
        assert!(parse_check("bogus \"x\"").is_err());
    }

    #[test]
    fn parse_check_rejects_cmd_with_no_assertions() {
        assert!(parse_check("cmd \"echo hi\"").is_err());
    }

    #[test]
    fn parse_check_rejects_invalid_regex() {
        assert!(parse_check("file \"x\" matches \"(unclosed\"").is_err());
    }

    #[test]
    fn parse_check_accepts_git_changed() {
        assert_eq!(
            parse_check("git changed \"src/**/*.rs\"").unwrap(),
            Check::GitChanged {
                glob: "src/**/*.rs".to_string(),
            }
        );
    }

    #[test]
    fn cmd_check_exit_code_pass_and_fail() {
        let repo = TempDir::new().unwrap();
        assert!(
            run_check(
                &Check::Cmd {
                    command: "exit 0".to_string(),
                    expect_exit: Some(0),
                    contains: None,
                    not_contains: None,
                },
                repo.path(),
                Duration::from_secs(5),
            )
            .is_ok()
        );

        assert!(
            run_check(
                &Check::Cmd {
                    command: "exit 1".to_string(),
                    expect_exit: Some(0),
                    contains: None,
                    not_contains: None,
                },
                repo.path(),
                Duration::from_secs(5),
            )
            .is_err()
        );
    }

    #[test]
    fn cmd_check_stdout_contains_and_not_contains() {
        let repo = TempDir::new().unwrap();
        assert!(
            run_check(
                &Check::Cmd {
                    command: "echo hello world".to_string(),
                    expect_exit: None,
                    contains: Some("hello".to_string()),
                    not_contains: None,
                },
                repo.path(),
                Duration::from_secs(5),
            )
            .is_ok()
        );

        assert!(
            run_check(
                &Check::Cmd {
                    command: "echo hello world".to_string(),
                    expect_exit: None,
                    contains: None,
                    not_contains: Some("hello".to_string()),
                },
                repo.path(),
                Duration::from_secs(5),
            )
            .is_err()
        );
    }

    #[test]
    fn cmd_check_times_out() {
        let repo = TempDir::new().unwrap();
        let result = run_check(
            &Check::Cmd {
                command: "sleep 5".to_string(),
                expect_exit: Some(0),
                contains: None,
                not_contains: None,
            },
            repo.path(),
            Duration::from_millis(100),
        );
        let err = result.unwrap_err();
        assert!(err.contains("timed out"), "unexpected error: {err}");
    }

    #[test]
    fn file_checks_exist_contains_matches() {
        let repo = TempDir::new().unwrap();
        std::fs::write(repo.path().join("docs.md"), "POST /auth/refresh\n").unwrap();

        assert!(
            run_check(
                &Check::FileExists {
                    path: "docs.md".to_string()
                },
                repo.path(),
                Duration::from_secs(5)
            )
            .is_ok()
        );
        assert!(
            run_check(
                &Check::FileExists {
                    path: "missing.md".to_string()
                },
                repo.path(),
                Duration::from_secs(5)
            )
            .is_err()
        );

        assert!(
            run_check(
                &Check::FileContains {
                    path: "docs.md".to_string(),
                    text: "POST /auth/refresh".to_string(),
                },
                repo.path(),
                Duration::from_secs(5)
            )
            .is_ok()
        );

        assert!(
            run_check(
                &Check::FileMatches {
                    path: "docs.md".to_string(),
                    pattern: r"^POST /auth/\w+$".to_string(),
                },
                repo.path(),
                Duration::from_secs(5)
            )
            .is_ok()
        );
    }

    fn git(repo: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn git_changed_check() {
        let repo = TempDir::new().unwrap();
        git(repo.path(), &["init", "-q"]);
        git(repo.path(), &["config", "user.email", "test@example.com"]);
        git(repo.path(), &["config", "user.name", "Test"]);
        std::fs::write(repo.path().join("a.txt"), "1\n").unwrap();
        std::fs::create_dir_all(repo.path().join("src")).unwrap();
        std::fs::write(repo.path().join("src/lib.rs"), "1\n").unwrap();
        git(repo.path(), &["add", "."]);
        git(repo.path(), &["commit", "-q", "-m", "init"]);

        std::fs::write(repo.path().join("src/lib.rs"), "2\n").unwrap();

        assert!(
            run_check(
                &Check::GitChanged {
                    glob: "src/**/*.rs".to_string()
                },
                repo.path(),
                Duration::from_secs(5)
            )
            .is_ok()
        );
        assert!(
            run_check(
                &Check::GitChanged {
                    glob: "*.md".to_string()
                },
                repo.path(),
                Duration::from_secs(5)
            )
            .is_err()
        );
    }

    fn store(dir: &TempDir) -> FsStore {
        let root = dir.path().join(".delta");
        std::fs::create_dir_all(root.join(CHANGES_DIR)).unwrap();
        std::fs::create_dir_all(root.join(TRUTH_DIR)).unwrap();
        std::fs::create_dir_all(root.join(ARCHIVE_DIR)).unwrap();
        FsStore::new(root)
    }

    #[test]
    fn verify_change_runs_checks_declared_in_any_artifact() {
        let dir = TempDir::new().unwrap();
        let store = store(&dir);
        let stages = crate::stage::default_stages();
        let now = chrono::Utc::now();

        change::new_change(
            &store,
            "add-widgets",
            now,
            &stages,
            crate::stage::Rigor::Standard,
        )
        .unwrap();

        std::fs::write(dir.path().join("README.md"), "widgets go here\n").unwrap();

        let artifact = change::Artifact {
            frontmatter: change::Frontmatter {
                stage: "design".to_string(),
                created: now,
                updated: now,
                source_hash: change::source_hash(&[]),
                status: change::ArtifactStatus::Valid,
                rigor: None,
                verify_forced: None,
            },
            body: "## Acceptance Criteria\n- [ ] Docs exist\n      `verify: file \"README.md\" exists`\n- [ ] Docs mention widgets\n      `verify: file \"README.md\" contains \"gizmos\"`\n".to_string(),
        };
        store
            .write_string(
                &Path::new(CHANGES_DIR).join("add-widgets").join("design.md"),
                &artifact.render().unwrap(),
            )
            .unwrap();

        let results = verify_change(
            &store,
            dir.path(),
            "add-widgets",
            &stages,
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].passed, "expected pass: {:?}", results[0]);
        assert!(!results[1].passed);
    }

    #[test]
    fn is_relevant_change_filters_noise_dirs() {
        let repo_root = Path::new("/repo");
        let noisy = Ok(notify::Event::new(notify::EventKind::Any)
            .add_path(repo_root.join(".delta/changes/x/design.md")));
        assert!(!is_relevant_change(&noisy, repo_root));

        let relevant =
            Ok(notify::Event::new(notify::EventKind::Any).add_path(repo_root.join("src/main.rs")));
        assert!(is_relevant_change(&relevant, repo_root));
    }
}
