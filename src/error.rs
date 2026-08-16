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
pub enum ChangeError {
    #[error("unknown change {slug}")]
    NotFound { slug: String },
    #[error("change {slug} already exists")]
    AlreadyExists { slug: String },
    #[error("invalid artifact frontmatter in {path}: {reason}")]
    InvalidFrontmatter { path: String, reason: String },
    #[error("invalid change slug {slug:?}: use lowercase letters, digits, - and _ only")]
    InvalidSlug { slug: String },
    #[error("change {slug} has stale artifacts and cannot be archived: {artifacts}")]
    Stale { slug: String, artifacts: String },
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
}

/// Top-level error for CLI dispatch. Its only job beyond wrapping the
/// per-module errors is mapping each to the exit code CI depends on:
/// 0 ok, 1 internal, 2 validation failed, 3 gate not satisfied, 4 stale inputs.
#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Change(#[from] ChangeError),
}

impl CliError {
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::Config(_) => 1,
            CliError::Workspace(WorkspaceError::Io { .. }) => 1,
            CliError::Workspace(_) => 2,
            CliError::Change(ChangeError::Workspace(WorkspaceError::Io { .. })) => 1,
            CliError::Change(ChangeError::Stale { .. }) => 4,
            CliError::Change(_) => 2,
        }
    }
}

#[derive(Debug, Error)]
#[allow(dead_code)] // constructed once stage.rs gains real logic in prompt 3
pub enum StageError {
    #[error("stage machine not yet implemented")]
    Unimplemented,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("no provider named {name:?} configured (expected a [providers.{name}] table)")]
    NotConfigured { name: String },
    #[error("provider {name:?} is missing required config key {key:?}")]
    MissingConfig { name: String, key: String },
    #[error("provider {name:?} has invalid config: {reason}")]
    InvalidConfig { name: String, reason: String },
    #[error("environment variable {env_var:?} (provider {name:?}'s api_key_env) is not set")]
    MissingApiKey { name: String, env_var: String },
    #[error("request to provider {name:?} failed: {source}")]
    Request {
        name: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("provider {name:?} returned HTTP {status}: {body}")]
    Http {
        name: String,
        status: u16,
        body: String,
    },
    #[error("provider {name:?} sent a malformed stream event: {reason}")]
    MalformedStream { name: String, reason: String },
    #[error("request to provider {name:?} was cancelled")]
    Cancelled { name: String },
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
