//! The artifact and change model: markdown-with-YAML-frontmatter
//! artifacts, `source_hash` computation and staleness detection, and the
//! change lifecycle (new, list, archive — applying deltas to truth).
//! Stages are runtime-loaded (`stage.rs`); this module never hardcodes
//! which stages exist, only how a change's artifacts relate to whatever
//! stage graph it's given. Does not talk to a provider or perform
//! verification.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ChangeError;
use crate::stage::{Rigor, StageDefinition};
use crate::workspace::{ARCHIVE_DIR, CHANGES_DIR, Store, TRUTH_DIR};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactStatus {
    Pending,
    Valid,
    Stale,
    Failed,
    /// Skipped because the stage's `min_rigor` exceeded the change's
    /// rigor — not a failure, just not applicable to this change.
    #[serde(rename = "n/a")]
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frontmatter {
    pub stage: String,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub source_hash: String,
    pub status: ArtifactStatus,
    /// The change's rigor, recorded on its root artifact at `change new`
    /// time (or overridden per-run). `None` on artifacts written before
    /// this field existed; treated as `Rigor::Deep` — never silently
    /// skip a stage for a change we can't classify.
    #[serde(default)]
    pub rigor: Option<Rigor>,
    /// Set to `true` when this artifact was archived via `dlt archive
    /// --force` while its change had failing acceptance-criteria checks
    /// — the literal "recorded in the archived frontmatter" requirement.
    /// Omitted (not just `false`) on every artifact where `--force`
    /// never bypassed a failure, so its presence alone is the signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_forced: Option<bool>,
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

fn artifact_path(slug: &str, stage_id: &str) -> PathBuf {
    change_dir(slug).join(format!("{stage_id}.md"))
}

fn find_root(stages: &[StageDefinition]) -> Result<&StageDefinition, ChangeError> {
    stages
        .iter()
        .find(|s| s.inputs.is_empty())
        .ok_or_else(|| ChangeError::UnknownStage {
            id: "<root stage>".to_string(),
        })
}

fn find_stage<'a>(
    stages: &'a [StageDefinition],
    id: &str,
) -> Result<&'a StageDefinition, ChangeError> {
    stages
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| ChangeError::UnknownStage { id: id.to_string() })
}

fn placeholder_body(stage: &StageDefinition) -> String {
    format!("# {}\n\nTODO: describe the change.\n", stage.name)
}

/// Read a change's artifact body for `stage_id`, if it exists yet.
/// `Ok(None)` means the stage simply hasn't been run — not an error.
pub(crate) fn read_artifact_body(
    store: &dyn Store,
    slug: &str,
    stage_id: &str,
) -> Result<Option<String>, ChangeError> {
    let path = artifact_path(slug, stage_id);
    if !store.exists(&path) {
        return Ok(None);
    }
    let text = store.read_to_string(&path)?;
    Ok(Some(
        Artifact::parse(&path.display().to_string(), &text)?.body,
    ))
}

/// Create a change: `changes/<slug>/deltas/` plus a placeholder artifact
/// for the stage graph's root stage (the one with no declared inputs),
/// recording `rigor` on it for later stages to read via `change_rigor`.
pub fn new_change(
    store: &dyn Store,
    slug: &str,
    now: DateTime<Utc>,
    stages: &[StageDefinition],
    rigor: Rigor,
) -> Result<(), ChangeError> {
    validate_slug(slug)?;
    if store.exists(&change_dir(slug)) {
        return Err(ChangeError::AlreadyExists {
            slug: slug.to_string(),
        });
    }
    let root = find_root(stages)?;
    store.create_dir_all(&deltas_dir(slug))?;

    let artifact = Artifact {
        frontmatter: Frontmatter {
            stage: root.id.clone(),
            created: now,
            updated: now,
            source_hash: source_hash(&[]),
            status: ArtifactStatus::Pending,
            rigor: Some(rigor),
            verify_forced: None,
        },
        body: placeholder_body(root),
    };
    store.write_string(&artifact_path(slug, &root.id), &artifact.render()?)?;
    Ok(())
}

pub fn list_changes(store: &dyn Store) -> Result<Vec<String>, ChangeError> {
    Ok(store.list_dir(Path::new(CHANGES_DIR))?)
}

/// A change's rigor, as recorded on its root artifact at `change new`
/// time. Defaults to `Rigor::Deep` if the change or its rigor field is
/// missing — the safe fallback that never causes a stage to be skipped
/// for data we can't classify.
pub fn change_rigor(
    store: &dyn Store,
    stages: &[StageDefinition],
    slug: &str,
) -> Result<Rigor, ChangeError> {
    let root = find_root(stages)?;
    let path = artifact_path(slug, &root.id);
    if !store.exists(&path) {
        return Ok(Rigor::Deep);
    }
    let text = store.read_to_string(&path)?;
    let artifact = Artifact::parse(&path.display().to_string(), &text)?;
    Ok(artifact.frontmatter.rigor.unwrap_or(Rigor::Deep))
}

/// The fields `write_stage_artifact` needs beyond `store`/`stages`/`slug`,
/// grouped so the function doesn't grow an unwieldy argument list.
pub struct StageWrite<'a> {
    pub stage_id: &'a str,
    pub body: &'a str,
    pub status: ArtifactStatus,
    pub rigor: Option<Rigor>,
    pub now: DateTime<Utc>,
}

/// Write (or overwrite) a stage's artifact for a change: computes its
/// `source_hash` fresh from its declared inputs' current bodies,
/// preserves the artifact's original `created` timestamp across reruns
/// if one already exists, and stamps `updated` as `write.now`.
pub fn write_stage_artifact(
    store: &dyn Store,
    stages: &[StageDefinition],
    slug: &str,
    write: StageWrite<'_>,
) -> Result<(), ChangeError> {
    let path = artifact_path(slug, write.stage_id);
    let created = if store.exists(&path) {
        let text = store.read_to_string(&path)?;
        Artifact::parse(&path.display().to_string(), &text)?
            .frontmatter
            .created
    } else {
        write.now
    };
    let source_hash = recompute_hash(store, slug, stages, write.stage_id)?;
    let artifact = Artifact {
        frontmatter: Frontmatter {
            stage: write.stage_id.to_string(),
            created,
            updated: write.now,
            source_hash,
            status: write.status,
            rigor: write.rigor,
            verify_forced: None,
        },
        body: write.body.to_string(),
    };
    store.write_string(&path, &artifact.render()?)?;
    Ok(())
}

pub struct ChangeStatus {
    pub slug: String,
    pub stage: String,
    pub state: ArtifactStatus,
    pub age: chrono::Duration,
}

/// Recompute what `stage_id`'s source_hash *should* be right now, from
/// the current bodies of its declared inputs. A missing input
/// contributes an empty body rather than erroring — the recomputed hash
/// then simply won't match, correctly reporting staleness.
fn recompute_hash(
    store: &dyn Store,
    slug: &str,
    stages: &[StageDefinition],
    stage_id: &str,
) -> Result<String, ChangeError> {
    let stage = find_stage(stages, stage_id)?;
    let mut bodies = Vec::with_capacity(stage.inputs.len());
    for input in &stage.inputs {
        bodies.push(read_artifact_body(store, slug, input)?.unwrap_or_default());
    }
    let refs: Vec<&str> = bodies.iter().map(String::as_str).collect();
    Ok(source_hash(&refs))
}

/// Status of the furthest artifact that exists for a change, per
/// `stages`' topological order. `state` reflects staleness first: if the
/// artifact's stored `source_hash` no longer matches its inputs' current
/// bodies, it is `Stale` regardless of what its frontmatter says — except
/// `NotApplicable` artifacts, which were deliberately skipped and are
/// never reported stale.
pub fn change_status(
    store: &dyn Store,
    slug: &str,
    now: DateTime<Utc>,
    stages: &[StageDefinition],
) -> Result<ChangeStatus, ChangeError> {
    if !store.exists(&change_dir(slug)) {
        return Err(ChangeError::NotFound {
            slug: slug.to_string(),
        });
    }

    let furthest = stages
        .iter()
        .rev()
        .find(|stage| store.exists(&artifact_path(slug, &stage.id)));

    let Some(stage) = furthest else {
        let root = find_root(stages)?;
        return Ok(ChangeStatus {
            slug: slug.to_string(),
            stage: root.id.clone(),
            state: ArtifactStatus::Pending,
            age: chrono::Duration::zero(),
        });
    };

    let path = artifact_path(slug, &stage.id);
    let text = store.read_to_string(&path)?;
    let artifact = Artifact::parse(&path.display().to_string(), &text)?;
    let state = if artifact.frontmatter.status == ArtifactStatus::NotApplicable {
        ArtifactStatus::NotApplicable
    } else {
        let recomputed = recompute_hash(store, slug, stages, &stage.id)?;
        if recomputed != artifact.frontmatter.source_hash {
            ArtifactStatus::Stale
        } else {
            artifact.frontmatter.status
        }
    };
    let age = now - artifact.frontmatter.created;

    Ok(ChangeStatus {
        slug: slug.to_string(),
        stage: stage.id.clone(),
        state,
        age,
    })
}

/// Every existing, non-`NotApplicable` artifact whose stored hash no
/// longer matches its inputs' current bodies, by stage id.
fn stale_artifacts(
    store: &dyn Store,
    slug: &str,
    stages: &[StageDefinition],
) -> Result<Vec<String>, ChangeError> {
    let mut stale = Vec::new();
    for stage in stages {
        let path = artifact_path(slug, &stage.id);
        if !store.exists(&path) {
            continue;
        }
        let text = store.read_to_string(&path)?;
        let artifact = Artifact::parse(&path.display().to_string(), &text)?;
        if artifact.frontmatter.status == ArtifactStatus::NotApplicable {
            continue;
        }
        if recompute_hash(store, slug, stages, &stage.id)? != artifact.frontmatter.source_hash {
            stale.push(stage.id.clone());
        }
    }
    Ok(stale)
}

/// Stamp `verify_forced: true` onto every existing artifact of `slug`,
/// for `dlt archive --force` bypassing failing acceptance-criteria
/// checks. Called before `archive_change` so the mark travels with the
/// artifacts into `archive/`.
pub fn mark_verify_forced(store: &dyn Store, slug: &str) -> Result<(), ChangeError> {
    for name in store.list_dir(&change_dir(slug))? {
        if !name.ends_with(".md") {
            continue;
        }
        let path = change_dir(slug).join(&name);
        let text = store.read_to_string(&path)?;
        let mut artifact = Artifact::parse(&path.display().to_string(), &text)?;
        artifact.frontmatter.verify_forced = Some(true);
        store.write_string(&path, &artifact.render()?)?;
    }
    Ok(())
}

/// Archive a change: refuse if any of its artifacts are stale, apply
/// `deltas/*` to `truth/` (each delta file replaces the truth file of
/// the same name), then move the change directory into `archive/`.
pub fn archive_change(
    store: &dyn Store,
    slug: &str,
    stages: &[StageDefinition],
) -> Result<(), ChangeError> {
    if !store.exists(&change_dir(slug)) {
        return Err(ChangeError::NotFound {
            slug: slug.to_string(),
        });
    }

    let stale = stale_artifacts(store, slug, stages)?;
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
    use crate::stage::default_stages;
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
        let stages = default_stages();
        new_change(&store, "add-widgets", now, &stages, Rigor::Standard).unwrap();

        let status = change_status(&store, "add-widgets", now, &stages).unwrap();
        assert_eq!(status.stage, "proposal");
        assert_eq!(status.state, ArtifactStatus::Pending);
    }

    #[test]
    fn new_change_twice_fails() {
        let dir = TempDir::new().unwrap();
        let store = store(&dir);
        let now = Utc::now();
        let stages = default_stages();
        new_change(&store, "dup", now, &stages, Rigor::Standard).unwrap();
        let err = new_change(&store, "dup", now, &stages, Rigor::Standard).unwrap_err();
        assert!(matches!(err, ChangeError::AlreadyExists { .. }));
    }

    #[test]
    fn design_goes_stale_when_proposal_changes() {
        let dir = TempDir::new().unwrap();
        let store = store(&dir);
        let now = Utc::now();
        let stages = default_stages();
        new_change(&store, "rework-auth", now, &stages, Rigor::Standard).unwrap();

        // Hand-craft a design.md whose hash matches the current proposal body,
        // simulating a design stage that already ran successfully.
        let proposal_hash = recompute_hash(&store, "rework-auth", &stages, "design").unwrap();
        let design = Artifact {
            frontmatter: Frontmatter {
                stage: "design".to_string(),
                created: now,
                updated: now,
                source_hash: proposal_hash,
                status: ArtifactStatus::Valid,
                rigor: None,
                verify_forced: None,
            },
            body: "Some design body.".to_string(),
        };
        store
            .write_string(
                &artifact_path("rework-auth", "design"),
                &design.render().unwrap(),
            )
            .unwrap();

        let status = change_status(&store, "rework-auth", now, &stages).unwrap();
        assert_eq!(status.stage, "design");
        assert_eq!(status.state, ArtifactStatus::Valid);

        // Now edit the proposal body: design's stored hash no longer matches.
        let proposal_path = artifact_path("rework-auth", "proposal");
        let proposal_text = store.read_to_string(&proposal_path).unwrap();
        let mut proposal = Artifact::parse("proposal.md", &proposal_text).unwrap();
        proposal.body = "# Proposal\n\nCompletely different now.\n".to_string();
        store
            .write_string(&proposal_path, &proposal.render().unwrap())
            .unwrap();

        let status = change_status(&store, "rework-auth", now, &stages).unwrap();
        assert_eq!(status.stage, "design");
        assert_eq!(status.state, ArtifactStatus::Stale);

        let err = archive_change(&store, "rework-auth", &stages).unwrap_err();
        assert!(matches!(err, ChangeError::Stale { .. }));
    }

    #[test]
    fn archive_applies_deltas_and_moves_change() {
        let dir = TempDir::new().unwrap();
        let store = store(&dir);
        let now = Utc::now();
        let stages = default_stages();
        new_change(&store, "add-widgets", now, &stages, Rigor::Standard).unwrap();
        store
            .write_string(
                &deltas_dir("add-widgets").join("widgets.md"),
                "Widgets can now be created.\n",
            )
            .unwrap();

        archive_change(&store, "add-widgets", &stages).unwrap();

        assert!(!store.exists(&change_dir("add-widgets")));
        assert!(store.exists(&Path::new(ARCHIVE_DIR).join("add-widgets")));
        assert_eq!(
            store
                .read_to_string(&Path::new(TRUTH_DIR).join("widgets.md"))
                .unwrap(),
            "Widgets can now be created.\n"
        );
    }

    #[test]
    fn change_rigor_defaults_to_deep_when_missing() {
        let dir = TempDir::new().unwrap();
        let store = store(&dir);
        let stages = default_stages();
        assert_eq!(
            change_rigor(&store, &stages, "nonexistent").unwrap(),
            Rigor::Deep
        );
    }

    #[test]
    fn change_rigor_reads_stored_value() {
        let dir = TempDir::new().unwrap();
        let store = store(&dir);
        let now = Utc::now();
        let stages = default_stages();
        new_change(&store, "add-widgets", now, &stages, Rigor::Trivial).unwrap();
        assert_eq!(
            change_rigor(&store, &stages, "add-widgets").unwrap(),
            Rigor::Trivial
        );
    }

    #[test]
    fn not_applicable_artifact_is_never_reported_stale() {
        let dir = TempDir::new().unwrap();
        let store = store(&dir);
        let now = Utc::now();
        let stages = default_stages();
        new_change(&store, "tiny-fix", now, &stages, Rigor::Trivial).unwrap();

        let skipped = Artifact {
            frontmatter: Frontmatter {
                stage: "design".to_string(),
                created: now,
                updated: now,
                source_hash: "does-not-matter".to_string(),
                status: ArtifactStatus::NotApplicable,
                rigor: None,
                verify_forced: None,
            },
            body: "Skipped: rigor too low.".to_string(),
        };
        store
            .write_string(
                &artifact_path("tiny-fix", "design"),
                &skipped.render().unwrap(),
            )
            .unwrap();

        let status = change_status(&store, "tiny-fix", now, &stages).unwrap();
        assert_eq!(status.state, ArtifactStatus::NotApplicable);

        archive_change(&store, "tiny-fix", &stages).unwrap();
        assert!(store.exists(&Path::new(ARCHIVE_DIR).join("tiny-fix")));
    }

    #[test]
    fn mark_verify_forced_stamps_every_existing_artifact() {
        let dir = TempDir::new().unwrap();
        let store = store(&dir);
        let now = Utc::now();
        let stages = default_stages();
        new_change(&store, "add-widgets", now, &stages, Rigor::Standard).unwrap();

        let before = store
            .read_to_string(&artifact_path("add-widgets", "proposal"))
            .unwrap();
        // Absent, not `false`, until --force actually bypasses a failure.
        assert!(!before.contains("verify_forced"));

        mark_verify_forced(&store, "add-widgets").unwrap();

        let after = store
            .read_to_string(&artifact_path("add-widgets", "proposal"))
            .unwrap();
        let artifact = Artifact::parse("proposal.md", &after).unwrap();
        assert_eq!(artifact.frontmatter.verify_forced, Some(true));
        assert!(after.contains("verify_forced"));
    }
}
