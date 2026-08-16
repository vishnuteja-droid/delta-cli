//! Layered configuration loading: built-in defaults, then
//! `~/.config/delta/config.toml`, then `.delta/config.toml` in the repo,
//! then environment variables, each layer overriding the previous. Does
//! not construct providers or other runtime objects from the config.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use directories::BaseDirs;
use toml::Value;
use toml::value::Table;

use crate::error::ConfigError;

/// Merged configuration, stored as a generic TOML table so that
/// forward-looking sections (e.g. `[providers.*]`, added in a later
/// prompt) round-trip and merge correctly without this module needing
/// to know their shape.
#[derive(Debug, Clone, Default)]
pub struct Config {
    table: Table,
}

impl Config {
    /// Load defaults, then the user config, then the repo config
    /// (rooted at `repo_root`), then environment variable overrides.
    pub fn load(repo_root: &Path) -> Result<Self, ConfigError> {
        let mut table = Self::defaults();

        if let Some(base_dirs) = BaseDirs::new() {
            let user_config = base_dirs.home_dir().join(".config/delta/config.toml");
            merge_file(&mut table, &user_config)?;
        }

        let repo_config = repo_root.join(".delta/config.toml");
        merge_file(&mut table, &repo_config)?;

        apply_env(&mut table, "DELTA_");

        Ok(Self { table })
    }

    fn defaults() -> Table {
        let mut table = Table::new();
        table.insert("workspace_dir".into(), Value::String(".delta".into()));
        table.insert("default_rigor".into(), Value::String("standard".into()));
        table
    }

    /// Fetch a string value by dotted key path, e.g. `"providers.default.model"`.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    /// Fetch a raw value by dotted key path.
    pub fn get(&self, key: &str) -> Option<&Value> {
        let mut current = self.table.get(key.split('.').next()?)?;
        for segment in key.split('.').skip(1) {
            current = current.as_table()?.get(segment)?;
        }
        Some(current)
    }
}

fn merge_file(table: &mut Table, path: &Path) -> Result<(), ConfigError> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let overlay: Table = toml::from_str(&raw).map_err(|source| ConfigError::Parse {
        path: path.display().to_string(),
        source,
    })?;
    merge_table(table, overlay);
    Ok(())
}

/// Deep-merge `overlay` into `base`, with `overlay` values winning on
/// conflict. Nested tables are merged recursively; other value types are
/// replaced wholesale.
fn merge_table(base: &mut Table, overlay: Table) {
    for (key, overlay_value) in overlay {
        match (base.get_mut(&key), overlay_value) {
            (Some(Value::Table(base_table)), Value::Table(overlay_table)) => {
                merge_table(base_table, overlay_table);
            }
            (_, overlay_value) => {
                base.insert(key, overlay_value);
            }
        }
    }
}

/// Apply environment variable overrides. A variable `DELTA_FOO__BAR=x`
/// (double underscore separates nesting levels) overrides the dotted key
/// `foo.bar`.
fn apply_env(table: &mut Table, prefix: &str) {
    let mut overrides: BTreeMap<String, String> = BTreeMap::new();
    for (key, value) in std::env::vars() {
        if let Some(stripped) = key.strip_prefix(prefix) {
            overrides.insert(stripped.to_ascii_lowercase(), value);
        }
    }
    for (key, value) in overrides {
        let path: Vec<&str> = key.split("__").collect();
        set_nested(table, &path, Value::String(value));
    }
}

fn set_nested(table: &mut Table, path: &[&str], value: Value) {
    match path {
        [] => {}
        [last] => {
            table.insert((*last).to_string(), value);
        }
        [head, rest @ ..] => {
            let entry = table
                .entry((*head).to_string())
                .or_insert_with(|| Value::Table(Table::new()));
            if let Value::Table(nested) = entry {
                set_nested(nested, rest, value);
            }
        }
    }
}

#[allow(dead_code)]
pub fn user_config_path() -> Option<PathBuf> {
    BaseDirs::new().map(|dirs| dirs.home_dir().join(".config/delta/config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // `Config::load` scans every `DELTA_*` env var, and env vars are
    // process-global — two of these tests running in parallel on
    // different threads (the cargo test default) can otherwise observe
    // each other's `set_var`/`remove_var` mid-test. Every test here
    // holds this lock for its whole body so they serialize against each
    // other regardless of which ones happen to touch env vars.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn defaults_only_when_no_files_present() {
        let _guard = ENV_LOCK.lock().unwrap();
        let repo = TempDir::new().unwrap();
        let config = Config::load(repo.path()).unwrap();
        assert_eq!(config.get_str("workspace_dir"), Some(".delta"));
    }

    #[test]
    fn repo_config_overrides_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        let repo = TempDir::new().unwrap();
        std::fs::create_dir_all(repo.path().join(".delta")).unwrap();
        let mut file = std::fs::File::create(repo.path().join(".delta/config.toml")).unwrap();
        writeln!(file, r#"workspace_dir = "custom""#).unwrap();
        let config = Config::load(repo.path()).unwrap();
        assert_eq!(config.get_str("workspace_dir"), Some("custom"));
    }

    #[test]
    fn env_overrides_files() {
        let _guard = ENV_LOCK.lock().unwrap();
        let repo = TempDir::new().unwrap();
        // SAFETY: test-only, serialized against other env-mutating tests via ENV_LOCK.
        unsafe {
            std::env::set_var("DELTA_WORKSPACE_DIR", "from-env");
        }
        let config = Config::load(repo.path()).unwrap();
        unsafe {
            std::env::remove_var("DELTA_WORKSPACE_DIR");
        }
        assert_eq!(config.get_str("workspace_dir"), Some("from-env"));
    }

    #[test]
    fn nested_env_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        let repo = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("DELTA_PROVIDERS__DEFAULT__MODEL", "gpt-test");
        }
        let config = Config::load(repo.path()).unwrap();
        unsafe {
            std::env::remove_var("DELTA_PROVIDERS__DEFAULT__MODEL");
        }
        assert_eq!(config.get_str("providers.default.model"), Some("gpt-test"));
    }
}
