//! On-disk layout of the `.delta/` directory: locating and creating
//! `truth/`, `changes/`, and `archive/`. File access is mediated through
//! a `Store` trait so alternate layouts (e.g. an `openspec/` adapter)
//! can be added later without touching call sites. Does not interpret
//! artifact contents.

use std::path::{Path, PathBuf};

use crate::error::WorkspaceError;

pub const DELTA_DIR: &str = ".delta";
pub const TRUTH_DIR: &str = "truth";
pub const CHANGES_DIR: &str = "changes";
pub const ARCHIVE_DIR: &str = "archive";
pub const STAGES_DIR: &str = "stages";

/// Default stage definitions, seeded into `.delta/stages/` on `init` so
/// a fresh workspace is immediately runnable. Once seeded, `stage.rs`
/// reads exclusively from disk — editing or adding a `.delta/stages/
/// *.yaml` file never requires a recompile; these `include_str!`s only
/// supply the starting point.
const DEFAULT_STAGES: [(&str, &str); 3] = [
    ("proposal.yaml", include_str!("../stages/proposal.yaml")),
    ("design.yaml", include_str!("../stages/design.yaml")),
    ("tasks.yaml", include_str!("../stages/tasks.yaml")),
];

/// Directories from other spec-driven-development tools. `init` notes
/// their presence; importing them is explicitly out of scope for now.
const INTEROP_CANDIDATES: [&str; 3] = ["openspec", ".kiro/specs", "specs"];

/// File access for a workspace, kept behind a trait so a future adapter
/// (e.g. reading an existing `openspec/` layout) can implement it
/// without changing any call site that only knows about `Store`.
pub trait Store: std::fmt::Debug {
    fn exists(&self, rel: &Path) -> bool;
    fn read_to_string(&self, rel: &Path) -> Result<String, WorkspaceError>;
    fn write_string(&self, rel: &Path, contents: &str) -> Result<(), WorkspaceError>;
    fn create_dir_all(&self, rel: &Path) -> Result<(), WorkspaceError>;
    /// Non-hidden entry names directly under `rel`, sorted. Empty if `rel` doesn't exist.
    fn list_dir(&self, rel: &Path) -> Result<Vec<String>, WorkspaceError>;
    fn rename(&self, from: &Path, to: &Path) -> Result<(), WorkspaceError>;
}

/// A `Store` rooted at a real directory on disk (always `<repo>/.delta`).
#[derive(Debug)]
pub struct FsStore {
    root: PathBuf,
}

impl FsStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn io_err(path: &Path, source: std::io::Error) -> WorkspaceError {
        WorkspaceError::Io {
            path: path.display().to_string(),
            source,
        }
    }
}

impl Store for FsStore {
    fn exists(&self, rel: &Path) -> bool {
        self.root.join(rel).exists()
    }

    fn read_to_string(&self, rel: &Path) -> Result<String, WorkspaceError> {
        let path = self.root.join(rel);
        std::fs::read_to_string(&path).map_err(|source| Self::io_err(&path, source))
    }

    fn write_string(&self, rel: &Path, contents: &str) -> Result<(), WorkspaceError> {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Self::io_err(parent, source))?;
        }
        std::fs::write(&path, contents).map_err(|source| Self::io_err(&path, source))
    }

    fn create_dir_all(&self, rel: &Path) -> Result<(), WorkspaceError> {
        let path = self.root.join(rel);
        std::fs::create_dir_all(&path).map_err(|source| Self::io_err(&path, source))
    }

    fn list_dir(&self, rel: &Path) -> Result<Vec<String>, WorkspaceError> {
        let path = self.root.join(rel);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let entries = std::fs::read_dir(&path).map_err(|source| Self::io_err(&path, source))?;
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| Self::io_err(&path, source))?;
            if let Some(name) = entry.file_name().to_str()
                && !name.starts_with('.')
            {
                names.push(name.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), WorkspaceError> {
        let from_path = self.root.join(from);
        let to_path = self.root.join(to);
        if let Some(parent) = to_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Self::io_err(parent, source))?;
        }
        std::fs::rename(&from_path, &to_path).map_err(|source| Self::io_err(&from_path, source))
    }
}

/// A located `.delta/` workspace: its filesystem root plus the `Store`
/// used to read and write within it.
#[derive(Debug)]
pub struct Workspace {
    root: PathBuf,
    store: Box<dyn Store>,
}

impl Workspace {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn store(&self) -> &dyn Store {
        self.store.as_ref()
    }

    fn delta_dir(repo_root: &Path) -> PathBuf {
        repo_root.join(DELTA_DIR)
    }

    pub fn is_initialized(repo_root: &Path) -> bool {
        Self::delta_dir(repo_root).is_dir()
    }

    /// Create a new workspace under `repo_root`. Fails if one already exists.
    pub fn init(repo_root: &Path) -> Result<Self, WorkspaceError> {
        if Self::is_initialized(repo_root) {
            return Err(WorkspaceError::AlreadyInitialized {
                path: Self::delta_dir(repo_root).display().to_string(),
            });
        }
        let root = Self::delta_dir(repo_root);
        let store: Box<dyn Store> = Box::new(FsStore::new(root.clone()));
        store.create_dir_all(Path::new(TRUTH_DIR))?;
        store.create_dir_all(Path::new(CHANGES_DIR))?;
        store.create_dir_all(Path::new(ARCHIVE_DIR))?;
        store.create_dir_all(Path::new(STAGES_DIR))?;
        for (filename, contents) in DEFAULT_STAGES {
            store.write_string(&Path::new(STAGES_DIR).join(filename), contents)?;
        }
        Ok(Self { root, store })
    }

    /// Open an existing workspace under `repo_root`. Fails if none exists.
    pub fn discover(repo_root: &Path) -> Result<Self, WorkspaceError> {
        if !Self::is_initialized(repo_root) {
            return Err(WorkspaceError::NotInitialized);
        }
        let root = Self::delta_dir(repo_root);
        let store: Box<dyn Store> = Box::new(FsStore::new(root.clone()));
        Ok(Self { root, store })
    }

    /// Interop directories from other tools found directly under `repo_root`.
    pub fn detect_interop(repo_root: &Path) -> Vec<&'static str> {
        INTEROP_CANDIDATES
            .into_iter()
            .filter(|candidate| repo_root.join(candidate).is_dir())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn init_creates_truth_changes_archive() {
        let repo = TempDir::new().unwrap();
        let workspace = Workspace::init(repo.path()).unwrap();
        assert!(workspace.store().exists(Path::new(TRUTH_DIR)));
        assert!(workspace.store().exists(Path::new(CHANGES_DIR)));
        assert!(workspace.store().exists(Path::new(ARCHIVE_DIR)));
    }

    #[test]
    fn init_seeds_default_stages() {
        let repo = TempDir::new().unwrap();
        let workspace = Workspace::init(repo.path()).unwrap();
        for filename in ["proposal.yaml", "design.yaml", "tasks.yaml"] {
            let path = Path::new(STAGES_DIR).join(filename);
            assert!(workspace.store().exists(&path), "missing {filename}");
            assert!(
                workspace
                    .store()
                    .read_to_string(&path)
                    .unwrap()
                    .contains("id:")
            );
        }
    }

    #[test]
    fn init_twice_fails() {
        let repo = TempDir::new().unwrap();
        Workspace::init(repo.path()).unwrap();
        let err = Workspace::init(repo.path()).unwrap_err();
        assert!(matches!(err, WorkspaceError::AlreadyInitialized { .. }));
    }

    #[test]
    fn discover_without_init_fails() {
        let repo = TempDir::new().unwrap();
        let err = Workspace::discover(repo.path()).unwrap_err();
        assert!(matches!(err, WorkspaceError::NotInitialized));
    }

    #[test]
    fn detects_interop_directories() {
        let repo = TempDir::new().unwrap();
        std::fs::create_dir_all(repo.path().join("openspec")).unwrap();
        let found = Workspace::detect_interop(repo.path());
        assert_eq!(found, vec!["openspec"]);
    }
}
