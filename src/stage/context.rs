//! Assemble the prompt for a stage: `AGENTS.md`, relevant truth, declared
//! input artifacts, and a repo tree summary, rendered through the
//! stage's MiniJinja template and trimmed to fit the provider's context
//! window. `dlt run --dry-run` prints exactly what this builds without
//! calling anything — this module is how prompt assembly gets debugged.

use std::collections::BTreeMap;
use std::path::Path;

use minijinja::{Value, context};
use serde::Serialize;

use crate::change;
use crate::error::StageError;
use crate::provider::Provider;
use crate::stage::StageDefinition;
use crate::workspace::{Store, TRUTH_DIR};

/// Directories skipped when building the repo tree summary — noise that
/// would burn context budget without informing the model.
const NOISE_DIRS: [&str; 6] = [".git", ".delta", "target", "node_modules", "dist", "build"];
const MAX_TREE_ENTRIES: usize = 500;

/// Tokens reserved for the model's own output, subtracted from
/// `provider.context_window()` to get the prompt's token budget.
const RESERVED_OUTPUT_TOKENS: u32 = 4_096;

#[derive(Debug, Clone, Serialize)]
struct InputRef {
    body: String,
}

/// The result of assembling a stage's prompt: the rendered text plus a
/// record of what (if anything) got dropped to fit the token budget.
#[derive(Debug, Clone)]
pub struct Assembled {
    pub prompt: String,
    pub dropped: Vec<String>,
    pub token_count: u32,
}

/// Assemble the prompt for `stage` on change `slug`. `repo_root` is the
/// repository root (for `AGENTS.md` and the tree walk, both outside
/// `.delta/`, hence read directly rather than through `Store`).
pub fn assemble<P: Provider>(
    store: &dyn Store,
    repo_root: &Path,
    stage: &StageDefinition,
    slug: &str,
    provider: &P,
) -> Result<Assembled, StageError> {
    let mut agents_md = std::fs::read_to_string(repo_root.join("AGENTS.md")).unwrap_or_default();
    let mut truth_relevant = read_truth(store)?;
    let mut repo_tree = build_repo_tree(repo_root);

    let mut inputs = BTreeMap::new();
    for input_id in &stage.inputs {
        let body = change::read_artifact_body(store, slug, input_id)?.ok_or_else(|| {
            StageError::MissingInput {
                stage: stage.id.clone(),
                input: input_id.clone(),
            }
        })?;
        inputs.insert(input_id.clone(), InputRef { body });
    }

    let env = minijinja::Environment::new();
    let render =
        |agents_md: &str, truth_relevant: &str, repo_tree: &str| -> Result<String, StageError> {
            let ctx = context! {
                agents_md => agents_md,
                truth => context! { relevant => truth_relevant },
                inputs => Value::from_serialize(&inputs),
                repo_tree => repo_tree,
            };
            env.render_str(&stage.template, ctx)
                .map_err(|e| StageError::Render {
                    id: stage.id.clone(),
                    reason: e.to_string(),
                })
        };

    let budget = provider
        .context_window()
        .saturating_sub(RESERVED_OUTPUT_TOKENS);
    let mut dropped = Vec::new();
    let mut prompt = render(&agents_md, &truth_relevant, &repo_tree)?;
    let mut token_count = provider.count_tokens(&prompt);

    // Drop optional context in priority order (lowest first) until the
    // prompt fits the provider's context window. Declared `inputs` are
    // never dropped.
    if token_count > budget && !repo_tree.is_empty() {
        repo_tree.clear();
        dropped.push("repo_tree".to_string());
        prompt = render(&agents_md, &truth_relevant, &repo_tree)?;
        token_count = provider.count_tokens(&prompt);
    }
    if token_count > budget && !truth_relevant.is_empty() {
        truth_relevant.clear();
        dropped.push("truth.relevant".to_string());
        prompt = render(&agents_md, &truth_relevant, &repo_tree)?;
        token_count = provider.count_tokens(&prompt);
    }
    if token_count > budget && !agents_md.is_empty() {
        agents_md.clear();
        dropped.push("agents_md".to_string());
        prompt = render(&agents_md, &truth_relevant, &repo_tree)?;
        token_count = provider.count_tokens(&prompt);
    }

    Ok(Assembled {
        prompt,
        dropped,
        token_count,
    })
}

/// Concatenate every `.delta/truth/*.md` file, each under a heading
/// naming its filename.
fn read_truth(store: &dyn Store) -> Result<String, StageError> {
    let mut sections = Vec::new();
    for name in store.list_dir(Path::new(TRUTH_DIR))? {
        if !name.ends_with(".md") {
            continue;
        }
        let body = store.read_to_string(&Path::new(TRUTH_DIR).join(&name))?;
        sections.push(format!("### {name}\n{body}"));
    }
    Ok(sections.join("\n"))
}

/// A flat, newline-separated list of repo-relative paths, skipping
/// `NOISE_DIRS` and capped at `MAX_TREE_ENTRIES` so it can't blow the
/// token budget on its own in a large repo.
fn build_repo_tree(repo_root: &Path) -> String {
    let walker = walkdir::WalkDir::new(repo_root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            entry
                .file_name()
                .to_str()
                .is_none_or(|name| !NOISE_DIRS.contains(&name))
        });

    let mut entries = Vec::new();
    for entry in walker.filter_map(Result::ok) {
        if entry.path() == repo_root {
            continue;
        }
        if let Ok(rel) = entry.path().strip_prefix(repo_root) {
            entries.push(rel.display().to_string());
        }
        if entries.len() >= MAX_TREE_ENTRIES {
            break;
        }
    }
    entries.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{ARCHIVE_DIR, CHANGES_DIR, FsStore};
    use tempfile::TempDir;

    struct StubProvider {
        context_window: u32,
    }

    impl Provider for StubProvider {
        fn name(&self) -> &str {
            "stub"
        }

        fn context_window(&self) -> u32 {
            self.context_window
        }

        fn count_tokens(&self, text: &str) -> u32 {
            text.len() as u32
        }

        async fn stream(
            &self,
            _request: crate::provider::Request,
            _cancel: tokio_util::sync::CancellationToken,
        ) -> Result<
            futures::stream::BoxStream<
                'static,
                Result<crate::provider::Delta, crate::error::ProviderError>,
            >,
            crate::error::ProviderError,
        > {
            unreachable!("not exercised by context assembly tests")
        }
    }

    fn setup() -> (TempDir, FsStore) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join(".delta");
        std::fs::create_dir_all(root.join(TRUTH_DIR)).unwrap();
        std::fs::create_dir_all(root.join(CHANGES_DIR)).unwrap();
        std::fs::create_dir_all(root.join(ARCHIVE_DIR)).unwrap();
        let store = FsStore::new(root);
        (dir, store)
    }

    fn proposal_stage() -> StageDefinition {
        crate::stage::default_stages()
            .into_iter()
            .find(|s| s.id == "proposal")
            .unwrap()
    }

    #[test]
    fn assembles_with_no_truth_or_inputs() {
        let (_dir, store) = setup();
        let provider = StubProvider {
            context_window: 100_000,
        };
        let stage = proposal_stage();
        let assembled = assemble(&store, _dir.path(), &stage, "any-slug", &provider).unwrap();
        assert!(assembled.dropped.is_empty());
        assert!(!assembled.prompt.contains("{{ agents_md }}"));
    }

    #[test]
    fn drops_repo_tree_first_when_over_budget() {
        let (dir, store) = setup();
        store
            .write_string(&Path::new(TRUTH_DIR).join("overview.md"), "some truth")
            .unwrap();
        // Give the repo tree something to drop: an empty temp dir walks to
        // nothing, which would make repo_tree already-empty and thus
        // ineligible to be "dropped" at all.
        std::fs::write(dir.path().join("readme.txt"), "hello").unwrap();
        let provider = StubProvider { context_window: 1 };
        let stage = proposal_stage();
        let assembled = assemble(&store, dir.path(), &stage, "any-slug", &provider).unwrap();
        assert_eq!(assembled.dropped[0], "repo_tree");
    }

    /// A pinned example of exactly what gets sent to the model: the
    /// literal output `--dry-run` prints. Regressions here are prompt
    /// regressions, not just code regressions — worth reviewing by eye.
    #[test]
    fn snapshot_of_assembled_design_prompt() {
        let (dir, store) = setup();
        std::fs::write(dir.path().join("AGENTS.md"), "Follow the house style.\n").unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        std::fs::write(dir.path().join("README.md"), "# Demo\n").unwrap();

        store
            .write_string(
                &Path::new(TRUTH_DIR).join("overview.md"),
                "The system has one module.\n",
            )
            .unwrap();

        let now = chrono::Utc::now();
        let proposal = crate::change::Artifact {
            frontmatter: crate::change::Frontmatter {
                stage: "proposal".to_string(),
                created: now,
                updated: now,
                source_hash: crate::change::source_hash(&[]),
                status: crate::change::ArtifactStatus::Valid,
                rigor: Some(crate::stage::Rigor::Standard),
                verify_forced: None,
            },
            body: "# Proposal\n\nAdd a health check endpoint.\n".to_string(),
        };
        store
            .write_string(
                &Path::new(CHANGES_DIR)
                    .join("demo-change")
                    .join("proposal.md"),
                &proposal.render().unwrap(),
            )
            .unwrap();

        let stage = crate::stage::default_stages()
            .into_iter()
            .find(|s| s.id == "design")
            .unwrap();
        let provider = StubProvider {
            context_window: 100_000,
        };
        let assembled = assemble(&store, dir.path(), &stage, "demo-change", &provider).unwrap();

        insta::assert_snapshot!(assembled.prompt);
    }

    #[test]
    fn missing_declared_input_is_an_error() {
        let (dir, store) = setup();
        let provider = StubProvider {
            context_window: 100_000,
        };
        let stage = crate::stage::default_stages()
            .into_iter()
            .find(|s| s.id == "design")
            .unwrap();
        let err = assemble(&store, dir.path(), &stage, "missing-slug", &provider).unwrap_err();
        assert!(matches!(err, StageError::MissingInput { .. }));
    }
}
