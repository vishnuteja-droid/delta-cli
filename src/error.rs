//! Error types, one `thiserror` enum per module. `anyhow` is used only
//! in `main.rs` to collect and report these at the top level; internal
//! code always returns a concrete, typed error.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: toml::de::Error,
    },
}

#[derive(Debug, Error)]
#[allow(dead_code)] // constructed once workspace.rs gains real logic in prompt 1
pub enum WorkspaceError {
    #[error("not a delta workspace (no .delta directory found)")]
    NotInitialized,
    #[error("delta workspace already initialized at {path}")]
    AlreadyInitialized { path: String },
    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Error)]
#[allow(dead_code)] // constructed once change.rs gains real logic in prompt 1
pub enum ChangeError {
    #[error("unknown change {slug}")]
    NotFound { slug: String },
    #[error("change {slug} already exists")]
    AlreadyExists { slug: String },
    #[error("invalid artifact frontmatter in {path}: {reason}")]
    InvalidFrontmatter { path: String, reason: String },
}

#[derive(Debug, Error)]
#[allow(dead_code)] // constructed once stage.rs gains real logic in prompt 3
pub enum StageError {
    #[error("stage machine not yet implemented")]
    Unimplemented,
}

#[derive(Debug, Error)]
#[allow(dead_code)] // constructed once provider.rs gains real logic in prompt 2
pub enum ProviderError {
    #[error("provider layer not yet implemented")]
    Unimplemented,
}

#[derive(Debug, Error)]
#[allow(dead_code)] // constructed once verify.rs gains real logic in prompt 4
pub enum VerifyError {
    #[error("verification engine not yet implemented")]
    Unimplemented,
}

#[derive(Debug, Error)]
#[allow(dead_code)] // constructed once tui.rs gains real logic in prompt 6
pub enum TuiError {
    #[error("tui not yet implemented")]
    Unimplemented,
}
