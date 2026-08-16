//! The artifact and change model: markdown-with-YAML-frontmatter
//! artifacts, `source_hash` computation and staleness detection, and the
//! change lifecycle (new, list, archive — applying deltas to truth).
//! Does not talk to a provider or perform verification.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ChangeError;
use crate::workspace::{ARCHIVE_DIR, CHANGES_DIR, Store, TRUTH_DIR};

const PROPOSAL_PLACEHOLDER: &str = "# Proposal\n\nTODO: describe the change.\n";

/// The three artifacts a change can hold, in dependency order. Each
/// stage's declared inputs are fixed for now (there's no runtime stage
/// graph yet — that arrives in prompt 3 and will drive this instead).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Proposal,
    Design,
    Tasks,
}

impl ArtifactKind {
    pub const ORDER: [ArtifactKind; 3] = [
        ArtifactKind::Proposal,
        ArtifactKind::Design,
        ArtifactKind::Tasks,
    ];

    pub fn id(self) -> &'static str {
        match self {
            ArtifactKind::Proposal => "proposal",
            ArtifactKind::Design => "design",
            ArtifactKind::Tasks => "tasks",
        }
    }

    fn filename(self) -> &'static str {
        match self {
            ArtifactKind::Proposal => "proposal.md",
            ArtifactKind::Design => "design.md",
            ArtifactKind::Tasks => "tasks.md",
        }
    }

    fn inputs(self) -> &'static [ArtifactKind] {
        match self {
            ArtifactKind::Proposal => &[],
            ArtifactKind::Design => &[ArtifactKind::Proposal],
            ArtifactKind::Tasks => &[ArtifactKind::Proposal, ArtifactKind::Design],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactStatus {
    Pending,
    Valid,
    Stale,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frontmatter {
    pub stage: String,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub source_hash: String,
    pub status: ArtifactStatus,
}

/// An artifact: YAML frontmatter plus a markdown body.
#[derive(Debug, Clone)]
pub struct Artifact {
    pub frontmatter: Frontmatter,
    pub body: String,
}

impl Artifact {
    /// Parse `text` (the full file contents) as `---\n<yaml>\n---\n<body>`.
    /// `path` is used only to produce a useful error message.
    pub fn parse(path: &str, text: &str) -> Result<Self, ChangeError> {
        let rest = text
            .strip_prefix("---\n")
            .ok_or_else(|| invalid(path, "missing opening frontmatter delimiter"))?;
        let (yaml, body) = rest
            .split_once("\n---\n")
            .ok_or_else(|| invalid(path, "missing closing frontmatter delimiter"))?;
        let frontmatter: Frontmatter = serde_yaml::from_str(yaml)
            .map_err(|e| invalid(path, &format!("invalid YAML frontmatter: {e}")))?;
        Ok(Artifact {
            frontmatter,
            body: body.to_string(),
        })
    }

    pub fn render(&self) -> Result<String, ChangeError> {
        let yaml = serde_yaml::to_string(&self.frontmatter).map_err(|e| {
            invalid(
                "<in-memory artifact>",
                &format!("failed to serialize frontmatter: {e}"),
            )
        })?;
        Ok(format!("---\n{yaml}---\n{}", self.body))
    }
}

fn invalid(path: &str, reason: &str) -> ChangeError {
    ChangeError::InvalidFrontmatter {
        path: path.to_string(),
        reason: reason.to_string(),
    }
}

/// SHA-256 over the concatenated input bodies, hex-encoded.
pub fn source_hash(input_bodies: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for body in input_bodies {
        hasher.update(body.as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn validate_slug(slug: &str) -> Result<(), ChangeError> {
    let is_valid = !slug.is_empty()
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && !slug.starts_with('-')
        && !slug.ends_with('-');
    if is_valid {
        Ok(())
    } else {
        Err(ChangeError::InvalidSlug {
            slug: slug.to_string(),
        })
    }
}

fn change_dir(slug: &str) -> PathBuf {
    Path::new(CHANGES_DIR).join(slug)
}

fn deltas_dir(slug: &str) -> PathBuf {
    change_dir(slug).join("deltas")
}

fn artifact_path(slug: &str, kind: ArtifactKind) -> PathBuf {
    change_dir(slug).join(kind.filename())
}

/// Create a change: `changes/<slug>/deltas/` plus a placeholder `proposal.md`.
pub fn new_change(store: &dyn Store, slug: &str, now: DateTime<Utc>) -> Result<(), ChangeError> {
    validate_slug(slug)?;
    if store.exists(&change_dir(slug)) {
        return Err(ChangeError::AlreadyExists {
            slug: slug.to_string(),
        });
    }
    store.create_dir_all(&deltas_dir(slug))?;

    let artifact = Artifact {
        frontmatter: Frontmatter {
            stage: ArtifactKind::Proposal.id().to_string(),
            created: now,
            updated: now,
            source_hash: source_hash(&[]),
            status: ArtifactStatus::Pending,
        },
        body: PROPOSAL_PLACEHOLDER.to_string(),
    };
    store.write_string(
        &artifact_path(slug, ArtifactKind::Proposal),
        &artifact.render()?,
    )?;
    Ok(())
}

pub fn list_changes(store: &dyn Store) -> Result<Vec<String>, ChangeError> {
    Ok(store.list_dir(Path::new(CHANGES_DIR))?)
}

pub struct ChangeStatus {
    pub slug: String,
    pub stage: &'static str,
    pub state: ArtifactStatus,
    pub age: chrono::Duration,
}

/// Recompute what `kind`'s source_hash *should* be right now, from the
/// current bodies of its declared inputs. A missing input contributes an
/// empty body rather than erroring — the recomputed hash then simply
/// won't match, correctly reporting staleness.
fn recompute_hash(
    store: &dyn Store,
    slug: &str,
    kind: ArtifactKind,
) -> Result<String, ChangeError> {
    let mut bodies = Vec::new();
    for input in kind.inputs() {
        let path = artifact_path(slug, *input);
        let body = if store.exists(&path) {
            let text = store.read_to_string(&path)?;
            Artifact::parse(&path.display().to_string(), &text)?.body
        } else {
            String::new()
        };
        bodies.push(body);
    }
    let refs: Vec<&str> = bodies.iter().map(String::as_str).collect();
    Ok(source_hash(&refs))
}

/// Status of the furthest artifact that exists for a change (proposal <
/// design < tasks). `state` reflects staleness first: if the artifact's
/// stored `source_hash` no longer matches its inputs' current bodies, it
/// is `Stale` regardless of what its frontmatter says.
pub fn change_status(
    store: &dyn Store,
    slug: &str,
    now: DateTime<Utc>,
) -> Result<ChangeStatus, ChangeError> {
    if !store.exists(&change_dir(slug)) {
        return Err(ChangeError::NotFound {
            slug: slug.to_string(),
        });
    }

    let furthest = ArtifactKind::ORDER
        .into_iter()
        .rfind(|kind| store.exists(&artifact_path(slug, *kind)));

    let Some(kind) = furthest else {
        return Ok(ChangeStatus {
            slug: slug.to_string(),
            stage: ArtifactKind::Proposal.id(),
            state: ArtifactStatus::Pending,
            age: chrono::Duration::zero(),
        });
    };

    let path = artifact_path(slug, kind);
    let text = store.read_to_string(&path)?;
    let artifact = Artifact::parse(&path.display().to_string(), &text)?;
    let recomputed = recompute_hash(store, slug, kind)?;
    let state = if recomputed != artifact.frontmatter.source_hash {
        ArtifactStatus::Stale
    } else {
        artifact.frontmatter.status
    };
    let age = now - artifact.frontmatter.created;

    Ok(ChangeStatus {
        slug: slug.to_string(),
        stage: kind.id(),
        state,
        age,
    })
}

/// Every existing artifact whose stored hash no longer matches its
/// inputs' current bodies, by id (e.g. `"design"`).
fn stale_artifacts(store: &dyn Store, slug: &str) -> Result<Vec<&'static str>, ChangeError> {
    let mut stale = Vec::new();
    for kind in ArtifactKind::ORDER {
        let path = artifact_path(slug, kind);
        if !store.exists(&path) {
            continue;
        }
        let text = store.read_to_string(&path)?;
        let artifact = Artifact::parse(&path.display().to_string(), &text)?;
        if recompute_hash(store, slug, kind)? != artifact.frontmatter.source_hash {
            stale.push(kind.id());
        }
    }
    Ok(stale)
}

/// Archive a change: refuse if any of its artifacts are stale, apply
/// `deltas/*` to `truth/` (each delta file replaces the truth file of
/// the same name), then move the change directory into `archive/`.
pub fn archive_change(store: &dyn Store, slug: &str) -> Result<(), ChangeError> {
    if !store.exists(&change_dir(slug)) {
        return Err(ChangeError::NotFound {
            slug: slug.to_string(),
        });
    }

    let stale = stale_artifacts(store, slug)?;
    if !stale.is_empty() {
        return Err(ChangeError::Stale {
            slug: slug.to_string(),
            artifacts: stale.join(", "),
        });
    }

    let deltas = deltas_dir(slug);
    for name in store.list_dir(&deltas)? {
        let content = store.read_to_string(&deltas.join(&name))?;
        store.write_string(&Path::new(TRUTH_DIR).join(&name), &content)?;
    }

    let archive_dest = Path::new(ARCHIVE_DIR).join(slug);
    if store.exists(&archive_dest) {
        return Err(ChangeError::AlreadyExists {
            slug: slug.to_string(),
        });
    }
    store.rename(&change_dir(slug), &archive_dest)?;
    Ok(())
}

/// Render a duration the way `status`'s AGE column wants it: coarse and short.
pub fn humanize_duration(duration: chrono::Duration) -> String {
    let seconds = duration.num_seconds().max(0);
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h", seconds / 3600)
    } else {
        format!("{}d", seconds / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::FsStore;
    use tempfile::TempDir;

    fn store(dir: &TempDir) -> FsStore {
        let root = dir.path().join(".delta");
        std::fs::create_dir_all(root.join(CHANGES_DIR)).unwrap();
        std::fs::create_dir_all(root.join(TRUTH_DIR)).unwrap();
        std::fs::create_dir_all(root.join(ARCHIVE_DIR)).unwrap();
        FsStore::new(root)
    }

    #[test]
    fn rejects_invalid_slugs() {
        assert!(validate_slug("").is_err());
        assert!(validate_slug("-leading-hyphen").is_err());
        assert!(validate_slug("Has Spaces").is_err());
        assert!(validate_slug("valid-slug_123").is_ok());
    }

    #[test]
    fn new_change_creates_pending_proposal() {
        let dir = TempDir::new().unwrap();
        let store = store(&dir);
        let now = Utc::now();
        new_change(&store, "add-widgets", now).unwrap();

        let status = change_status(&store, "add-widgets", now).unwrap();
        assert_eq!(status.stage, "proposal");
        assert_eq!(status.state, ArtifactStatus::Pending);
    }

    #[test]
    fn new_change_twice_fails() {
        let dir = TempDir::new().unwrap();
        let store = store(&dir);
        let now = Utc::now();
        new_change(&store, "dup", now).unwrap();
        let err = new_change(&store, "dup", now).unwrap_err();
        assert!(matches!(err, ChangeError::AlreadyExists { .. }));
    }

    #[test]
    fn design_goes_stale_when_proposal_changes() {
        let dir = TempDir::new().unwrap();
        let store = store(&dir);
        let now = Utc::now();
        new_change(&store, "rework-auth", now).unwrap();

        // Hand-craft a design.md whose hash matches the current proposal body,
        // simulating a design stage that already ran successfully.
        let proposal_hash = recompute_hash(&store, "rework-auth", ArtifactKind::Design).unwrap();
        let design = Artifact {
            frontmatter: Frontmatter {
                stage: "design".to_string(),
                created: now,
                updated: now,
                source_hash: proposal_hash,
                status: ArtifactStatus::Valid,
            },
            body: "Some design body.".to_string(),
        };
        store
            .write_string(
                &artifact_path("rework-auth", ArtifactKind::Design),
                &design.render().unwrap(),
            )
            .unwrap();

        let status = change_status(&store, "rework-auth", now).unwrap();
        assert_eq!(status.stage, "design");
        assert_eq!(status.state, ArtifactStatus::Valid);

        // Now edit the proposal body: design's stored hash no longer matches.
        let proposal_path = artifact_path("rework-auth", ArtifactKind::Proposal);
        let proposal_text = store.read_to_string(&proposal_path).unwrap();
        let mut proposal = Artifact::parse("proposal.md", &proposal_text).unwrap();
        proposal.body = "# Proposal\n\nCompletely different now.\n".to_string();
        store
            .write_string(&proposal_path, &proposal.render().unwrap())
            .unwrap();

        let status = change_status(&store, "rework-auth", now).unwrap();
        assert_eq!(status.stage, "design");
        assert_eq!(status.state, ArtifactStatus::Stale);

        let err = archive_change(&store, "rework-auth").unwrap_err();
        assert!(matches!(err, ChangeError::Stale { .. }));
    }

    #[test]
    fn archive_applies_deltas_and_moves_change() {
        let dir = TempDir::new().unwrap();
        let store = store(&dir);
        let now = Utc::now();
        new_change(&store, "add-widgets", now).unwrap();
        store
            .write_string(
                &deltas_dir("add-widgets").join("widgets.md"),
                "Widgets can now be created.\n",
            )
            .unwrap();

        archive_change(&store, "add-widgets").unwrap();

        assert!(!store.exists(&change_dir("add-widgets")));
        assert!(store.exists(&Path::new(ARCHIVE_DIR).join("add-widgets")));
        assert_eq!(
            store
                .read_to_string(&Path::new(TRUTH_DIR).join("widgets.md"))
                .unwrap(),
            "Widgets can now be created.\n"
        );
    }
}
