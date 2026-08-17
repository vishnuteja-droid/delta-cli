//! Journal of file mutations made by the tool loop (`tools::agent`), so
//! `dlt undo` can revert the most recent one. One JSON file per entry
//! under `.delta/journal/`, named as a zero-padded sequence number
//! derived from the directory's current entry count — so lexicographic
//! order always matches recency, self-correcting after an undo removes
//! an entry (see `undo_last`'s doc comment for why this can't collide).
//! Only `write_file` and `apply_patch` are journalled: `run_command`'s
//! effects aren't file mutations this module owns, and generally aren't
//! reversible at all.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::ToolError;
use crate::workspace::{JOURNAL_DIR, Store};

/// Sibling of `JOURNAL_DIR` (not nested inside it) so undone entries
/// moved here are never counted by `list_dir(JOURNAL_DIR)` when the next
/// entry's filename is chosen.
const JOURNAL_UNDONE_DIR: &str = "journal-undone";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JournalEntry {
    tool: String,
    /// Path to the mutated file, relative to the repository root (not
    /// `.delta/`) — the tool loop only ever mutates real repo files.
    path: String,
    /// `None` means the tool created a file that didn't exist before;
    /// undoing it deletes the file rather than restoring content.
    previous_content: Option<String>,
    recorded_at: DateTime<Utc>,
}

/// Record one file mutation. `path` is repo-root-relative.
pub fn record(
    store: &dyn Store,
    tool: &str,
    path: &str,
    previous_content: Option<String>,
    now: DateTime<Utc>,
) -> Result<(), ToolError> {
    let entry = JournalEntry {
        tool: tool.to_string(),
        path: path.to_string(),
        previous_content,
        recorded_at: now,
    };
    let json = serde_json::to_string_pretty(&entry)
        .map_err(|e| ToolError::Journal(format!("failed to serialize journal entry: {e}")))?;
    let filename = next_filename(store)?;
    store.write_string(&Path::new(JOURNAL_DIR).join(filename), &json)?;
    Ok(())
}

fn next_filename(store: &dyn Store) -> Result<String, ToolError> {
    let existing = store.list_dir(Path::new(JOURNAL_DIR))?;
    Ok(format!("{:06}.json", existing.len() + 1))
}

/// Revert the most recently journalled write: restore the target file's
/// previous content, or delete it if it didn't exist before. The
/// consumed entry moves to `journal-undone/` rather than being deleted,
/// for an audit trail — "reverted," not "erased." Returns the
/// repo-relative path that was reverted.
pub fn undo_last(store: &dyn Store, repo_root: &Path) -> Result<PathBuf, ToolError> {
    let mut entries = store.list_dir(Path::new(JOURNAL_DIR))?;
    entries.retain(|name| name.ends_with(".json"));
    let Some(filename) = entries.into_iter().max() else {
        return Err(ToolError::JournalEmpty);
    };

    let entry_path = Path::new(JOURNAL_DIR).join(&filename);
    let text = store.read_to_string(&entry_path)?;
    let entry: JournalEntry = serde_json::from_str(&text)
        .map_err(|e| ToolError::Journal(format!("corrupt journal entry {filename}: {e}")))?;

    let target = repo_root.join(&entry.path);
    match &entry.previous_content {
        Some(content) => std::fs::write(&target, content).map_err(|source| ToolError::Io {
            path: target.display().to_string(),
            source,
        })?,
        None if target.exists() => {
            std::fs::remove_file(&target).map_err(|source| ToolError::Io {
                path: target.display().to_string(),
                source,
            })?
        }
        None => {}
    }

    store.rename(&entry_path, &Path::new(JOURNAL_UNDONE_DIR).join(&filename))?;
    Ok(PathBuf::from(&entry.path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::FsStore;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FsStore) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join(".delta");
        std::fs::create_dir_all(root.join(JOURNAL_DIR)).unwrap();
        (dir, FsStore::new(root))
    }

    #[test]
    fn undo_with_no_entries_errors() {
        let (dir, store) = setup();
        let err = undo_last(&store, dir.path()).unwrap_err();
        assert!(matches!(err, ToolError::JournalEmpty));
    }

    #[test]
    fn undo_restores_previous_content() {
        let (dir, store) = setup();
        std::fs::write(dir.path().join("src.txt"), "new content").unwrap();
        record(
            &store,
            "write_file",
            "src.txt",
            Some("old content".to_string()),
            Utc::now(),
        )
        .unwrap();

        let reverted = undo_last(&store, dir.path()).unwrap();
        assert_eq!(reverted, PathBuf::from("src.txt"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src.txt")).unwrap(),
            "old content"
        );
    }

    #[test]
    fn undo_deletes_a_file_that_did_not_exist_before() {
        let (dir, store) = setup();
        std::fs::write(dir.path().join("new.txt"), "created by the agent").unwrap();
        record(&store, "write_file", "new.txt", None, Utc::now()).unwrap();

        undo_last(&store, dir.path()).unwrap();
        assert!(!dir.path().join("new.txt").exists());
    }

    #[test]
    fn undo_only_reverts_the_most_recent_entry() {
        let (dir, store) = setup();
        std::fs::write(dir.path().join("a.txt"), "a-new").unwrap();
        std::fs::write(dir.path().join("b.txt"), "b-new").unwrap();
        record(
            &store,
            "write_file",
            "a.txt",
            Some("a-old".into()),
            Utc::now(),
        )
        .unwrap();
        record(
            &store,
            "write_file",
            "b.txt",
            Some("b-old".into()),
            Utc::now(),
        )
        .unwrap();

        let reverted = undo_last(&store, dir.path()).unwrap();
        assert_eq!(reverted, PathBuf::from("b.txt"));
        assert_eq!(
            std::fs::read_to_string(dir.path().join("b.txt")).unwrap(),
            "b-old"
        );
        // a.txt is untouched — only the last entry was reverted.
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "a-new"
        );
    }

    #[test]
    fn consecutive_undos_walk_backwards_and_numbering_never_collides() {
        let (dir, store) = setup();
        for (name, content) in [("a.txt", "a"), ("b.txt", "b"), ("c.txt", "c")] {
            std::fs::write(dir.path().join(name), format!("{content}-new")).unwrap();
            record(
                &store,
                "write_file",
                name,
                Some(format!("{content}-old")),
                Utc::now(),
            )
            .unwrap();
        }

        assert_eq!(
            undo_last(&store, dir.path()).unwrap(),
            PathBuf::from("c.txt")
        );
        assert_eq!(
            undo_last(&store, dir.path()).unwrap(),
            PathBuf::from("b.txt")
        );

        // A fresh write after two undos must not collide with the
        // journal-undone entries left behind by those undos.
        std::fs::write(dir.path().join("d.txt"), "d-new").unwrap();
        record(
            &store,
            "write_file",
            "d.txt",
            Some("d-old".into()),
            Utc::now(),
        )
        .unwrap();
        assert_eq!(
            undo_last(&store, dir.path()).unwrap(),
            PathBuf::from("d.txt")
        );
        assert_eq!(
            undo_last(&store, dir.path()).unwrap(),
            PathBuf::from("a.txt")
        );
        assert!(matches!(
            undo_last(&store, dir.path()).unwrap_err(),
            ToolError::JournalEmpty
        ));
    }
}
