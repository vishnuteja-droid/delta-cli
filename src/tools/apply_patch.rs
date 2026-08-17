//! Unified-diff patch application with fuzzy context matching — the
//! hardest correctness problem in the tool loop, per `PLAN.md`. A
//! model-authored patch's declared line numbers routinely drift from the
//! file's actual current state (an earlier hunk already shifted line
//! counts, or the file changed since the model last read it), so each
//! hunk is *located* by matching its context content against the file,
//! starting near the declared line number and expanding outward, rather
//! than trusted at face value. Two match passes: exact first, then a
//! whitespace-tolerant fallback (trailing whitespace stripped both
//! sides) — `str::lines()` already normalizes `\r\n` vs `\n` for us, so
//! CRLF files and LF-only patch text compare equal without special
//! casing. Hunks apply in order against the progressively-patched
//! buffer, so overlapping/adjacent hunks compose correctly.
//!
//! Deliberately single-file: the caller (the `apply_patch` tool) already
//! knows which file it's targeting, so `--- a/...`/`+++ b/...` file
//! headers are tolerated if present but never consulted — only the `@@`
//! hunks and their body lines matter.

use crate::error::ToolError;

#[derive(Debug, Clone, PartialEq, Eq)]
enum HunkLine {
    Context(String),
    Remove(String),
    Add(String),
}

#[derive(Debug, Clone)]
struct Hunk {
    /// 1-based original line number from the `@@ -N,...` header — a
    /// starting point for the fuzzy search, never trusted outright.
    old_start: usize,
    lines: Vec<HunkLine>,
}

/// Apply `diff` (a unified-diff body) to `existing` (`None` if the file
/// doesn't exist yet — the patch is expected to be pure-addition hunks
/// in that case) and return the resulting file content.
pub fn apply(existing: Option<&str>, diff: &str) -> Result<String, ToolError> {
    let hunks = parse_hunks(diff)?;
    let original = existing.unwrap_or("");
    let ending = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let had_trailing_newline = existing.is_none_or(|c| c.ends_with('\n'));

    let mut lines: Vec<String> = original.lines().map(str::to_string).collect();
    for hunk in &hunks {
        apply_hunk(&mut lines, hunk)?;
    }

    let mut result = lines.join(ending);
    if had_trailing_newline && !lines.is_empty() {
        result.push_str(ending);
    }
    Ok(result)
}

fn parse_hunks(diff: &str) -> Result<Vec<Hunk>, ToolError> {
    let mut hunks = Vec::new();
    let mut current: Option<Hunk> = None;
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("@@ ") {
            if let Some(hunk) = current.take() {
                hunks.push(hunk);
            }
            current = Some(Hunk {
                old_start: parse_hunk_header(rest)?,
                lines: Vec::new(),
            });
            continue;
        }
        if line.starts_with("--- ") || line.starts_with("+++ ") || line.starts_with('\\') {
            continue;
        }
        let Some(hunk) = current.as_mut() else {
            if line.trim().is_empty() {
                continue;
            }
            return Err(ToolError::Patch(format!(
                "content before any @@ hunk header: {line:?}"
            )));
        };
        if let Some(text) = line.strip_prefix(' ') {
            hunk.lines.push(HunkLine::Context(text.to_string()));
        } else if let Some(text) = line.strip_prefix('-') {
            hunk.lines.push(HunkLine::Remove(text.to_string()));
        } else if let Some(text) = line.strip_prefix('+') {
            hunk.lines.push(HunkLine::Add(text.to_string()));
        } else if line.is_empty() {
            // A context line can be emitted bare (no leading space) when
            // it's blank — some diff generators do this.
            hunk.lines.push(HunkLine::Context(String::new()));
        } else {
            return Err(ToolError::Patch(format!("malformed hunk line: {line:?}")));
        }
    }
    if let Some(hunk) = current.take() {
        hunks.push(hunk);
    }
    if hunks.is_empty() {
        return Err(ToolError::Patch("diff contains no @@ hunks".to_string()));
    }
    Ok(hunks)
}

/// Parse the old-side start line out of `-N,M +N,M @@ ...` (the text
/// after `"@@ "`). Only the starting line number is needed — it's just a
/// search anchor.
fn parse_hunk_header(rest: &str) -> Result<usize, ToolError> {
    let old_part = rest
        .split_whitespace()
        .next()
        .and_then(|p| p.strip_prefix('-'))
        .ok_or_else(|| ToolError::Patch(format!("malformed hunk header: {rest:?}")))?;
    old_part
        .split(',')
        .next()
        .unwrap_or(old_part)
        .parse::<usize>()
        .map_err(|_| ToolError::Patch(format!("malformed hunk header: {rest:?}")))
}

fn apply_hunk(lines: &mut Vec<String>, hunk: &Hunk) -> Result<(), ToolError> {
    let search: Vec<&str> = hunk
        .lines
        .iter()
        .filter_map(|l| match l {
            HunkLine::Context(s) | HunkLine::Remove(s) => Some(s.as_str()),
            HunkLine::Add(_) => None,
        })
        .collect();

    if search.is_empty() {
        // Pure insertion: no context or removed lines to locate, so
        // there's nothing to fuzzy-match against. Per unified-diff
        // convention, a zero-length old range's start line is the line
        // *after* which the new lines go (e.g. "@@ -0,0 +1,3 @@" inserts
        // at the very start of an empty file; "@@ -5,0 +6,2 @@" inserts
        // after original line 5) — so the index is `old_start`, not
        // `old_start - 1`.
        let at = hunk.old_start.min(lines.len());
        let additions: Vec<String> = hunk
            .lines
            .iter()
            .filter_map(|l| match l {
                HunkLine::Add(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        lines.splice(at..at, additions);
        return Ok(());
    }

    let anchor = hunk.old_start.saturating_sub(1);
    let start = find_match(lines, &search, anchor).ok_or_else(|| {
        ToolError::Patch(format!(
            "could not locate hunk context near line {}: {:?}",
            hunk.old_start, search[0]
        ))
    })?;

    // Build the replacement from the *actually matched* file lines for
    // context positions (so incidental drift the fuzzy match already
    // tolerated — e.g. trailing whitespace — isn't clobbered by the
    // hunk's own, possibly slightly different, rendering of that same
    // unchanged line) and from the hunk's literal text for additions.
    let mut replace = Vec::with_capacity(hunk.lines.len());
    let mut matched_at = start;
    for hunk_line in &hunk.lines {
        match hunk_line {
            HunkLine::Context(_) => {
                replace.push(lines[matched_at].clone());
                matched_at += 1;
            }
            HunkLine::Remove(_) => matched_at += 1,
            HunkLine::Add(text) => replace.push(text.clone()),
        }
    }
    lines.splice(start..start + search.len(), replace);
    Ok(())
}

/// Locate `search` in `lines`, starting at `anchor` and expanding
/// outward in both directions until found or the whole file is
/// exhausted. Exact match first; if nothing matches exactly anywhere,
/// retry with trailing whitespace ignored on both sides.
fn find_match(lines: &[String], search: &[&str], anchor: usize) -> Option<usize> {
    search_with(lines, search, anchor, |a, b| a == b)
        .or_else(|| search_with(lines, search, anchor, |a, b| a.trim_end() == b.trim_end()))
}

fn search_with(
    lines: &[String],
    search: &[&str],
    anchor: usize,
    eq: impl Fn(&str, &str) -> bool,
) -> Option<usize> {
    if search.is_empty() || search.len() > lines.len() {
        return None;
    }
    let max_start = lines.len() - search.len();
    let anchor = anchor.min(max_start);
    let matches_at = |start: usize| (0..search.len()).all(|i| eq(&lines[start + i], search[i]));

    if matches_at(anchor) {
        return Some(anchor);
    }
    let mut offset = 1usize;
    loop {
        let mut any_in_range = false;
        if let Some(cand) = anchor.checked_sub(offset) {
            any_in_range = true;
            if matches_at(cand) {
                return Some(cand);
            }
        }
        let cand = anchor + offset;
        if cand <= max_start {
            any_in_range = true;
            if matches_at(cand) {
                return Some(cand);
            }
        }
        if !any_in_range {
            return None;
        }
        offset += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_a_clean_single_hunk_patch() {
        let original = "fn main() {\n    println!(\"hi\");\n}\n";
        let diff = "@@ -1,3 +1,3 @@\n fn main() {\n-    println!(\"hi\");\n+    println!(\"hello\");\n }\n";
        let result = apply(Some(original), diff).unwrap();
        assert_eq!(result, "fn main() {\n    println!(\"hello\");\n}\n");
    }

    #[test]
    fn applies_multiple_hunks_in_one_file() {
        let original = "one\ntwo\nthree\nfour\nfive\n";
        let diff = "\
@@ -1,2 +1,2 @@
-one
+ONE
 two
@@ -4,2 +4,2 @@
 four
-five
+FIVE
";
        let result = apply(Some(original), diff).unwrap();
        assert_eq!(result, "ONE\ntwo\nthree\nfour\nFIVE\n");
    }

    /// The declared `@@ -N` anchor is wrong (content shifted since the
    /// patch was generated) — the hunk must still be found by content.
    #[test]
    fn tolerates_line_number_drift() {
        let original = "a\nb\nc\nextra1\nextra2\nextra3\ntarget\nz\n";
        // Header claims line 1, but "target" is actually at line 7 —
        // simulating a patch generated against a slightly different
        // version of the file.
        let diff = "@@ -1,1 +1,1 @@\n-target\n+TARGET\n";
        let result = apply(Some(original), diff).unwrap();
        assert_eq!(result, "a\nb\nc\nextra1\nextra2\nextra3\nTARGET\nz\n");
    }

    #[test]
    fn tolerates_trailing_whitespace_differences() {
        // File has trailing spaces the patch's context line doesn't.
        let original = "fn main() {   \n    body();\n}\n";
        let diff = "@@ -1,2 +1,2 @@\n fn main() {\n-    body();\n+    changed();\n";
        let result = apply(Some(original), diff).unwrap();
        assert_eq!(result, "fn main() {   \n    changed();\n}\n");
    }

    /// The target file is CRLF; the patch text (as an LLM would emit it)
    /// is plain LF. `str::lines()` normalizes both sides, and the
    /// original file's CRLF convention is preserved on output.
    #[test]
    fn matches_across_crlf_vs_lf_and_preserves_original_ending() {
        let original = "one\r\ntwo\r\nthree\r\n";
        let diff = "@@ -2,1 +2,1 @@\n-two\n+TWO\n";
        let result = apply(Some(original), diff).unwrap();
        assert_eq!(result, "one\r\nTWO\r\nthree\r\n");
    }

    /// Two hunks touching adjacent/overlapping regions — the second
    /// hunk's context must be found against the buffer *after* the
    /// first hunk already mutated it.
    #[test]
    fn applies_overlapping_adjacent_hunks_in_order() {
        let original = "alpha\nbeta\ngamma\n";
        let diff = "\
@@ -1,2 +1,2 @@
-alpha
+ALPHA
 beta
@@ -2,2 +2,2 @@
 beta
-gamma
+GAMMA
";
        let result = apply(Some(original), diff).unwrap();
        assert_eq!(result, "ALPHA\nbeta\nGAMMA\n");
    }

    #[test]
    fn pure_insertion_hunk_with_no_context() {
        let original = "one\ntwo\n";
        let diff = "@@ -2,0 +3,1 @@\n+inserted\n";
        let result = apply(Some(original), diff).unwrap();
        assert_eq!(result, "one\ntwo\ninserted\n");
    }

    #[test]
    fn creates_a_new_file_from_a_pure_addition_patch() {
        let diff = "@@ -0,0 +1,2 @@\n+line one\n+line two\n";
        let result = apply(None, diff).unwrap();
        assert_eq!(result, "line one\nline two\n");
    }

    #[test]
    fn errors_clearly_when_hunk_context_cannot_be_found() {
        let original = "a\nb\nc\n";
        let diff = "@@ -1,1 +1,1 @@\n-nonexistent line\n+replacement\n";
        let err = apply(Some(original), diff).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("could not locate"),
            "unexpected message: {msg}"
        );
    }

    #[test]
    fn preserves_absence_of_trailing_newline() {
        let original = "a\nb";
        let diff = "@@ -2,1 +2,1 @@\n-b\n+B\n";
        let result = apply(Some(original), diff).unwrap();
        assert_eq!(result, "a\nB");
    }

    #[test]
    fn rejects_a_diff_with_no_hunks() {
        let err = apply(Some("a\n"), "not a diff at all").unwrap_err();
        assert!(matches!(err, ToolError::Patch(_)));
    }

    #[test]
    fn tolerates_file_header_lines_before_the_first_hunk() {
        let original = "a\nb\n";
        let diff = "--- a/file.txt\n+++ b/file.txt\n@@ -1,1 +1,1 @@\n-a\n+A\n";
        let result = apply(Some(original), diff).unwrap();
        assert_eq!(result, "A\nb\n");
    }
}
