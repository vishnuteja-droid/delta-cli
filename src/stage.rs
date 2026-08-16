//! Runtime-loaded stage definitions (YAML), rigor classification, and
//! prompt/context assembly for each stage of a change. Stage YAML lives
//! in `.delta/stages/`; `load_all` reads exclusively from disk, so
//! adding or editing a stage never requires a recompile.

pub mod classify;
pub mod context;
pub mod validate;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::StageError;
use crate::workspace::{STAGES_DIR, Store};

/// How much verification rigor a change warrants. Ordered (via derived
/// `Ord`) so a stage's `min_rigor` can be compared against a change's
/// classified/overridden rigor with `>`: a stage is skipped when its
/// `min_rigor` exceeds that rigor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rigor {
    Trivial,
    Standard,
    Deep,
}

impl std::fmt::Display for Rigor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Rigor::Trivial => "trivial",
            Rigor::Standard => "standard",
            Rigor::Deep => "deep",
        };
        f.write_str(s)
    }
}

impl std::str::FromStr for Rigor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "trivial" => Ok(Rigor::Trivial),
            "standard" => Ok(Rigor::Standard),
            "deep" => Ok(Rigor::Deep),
            other => Err(format!(
                "invalid rigor {other:?} (expected trivial, standard, or deep)"
            )),
        }
    }
}

/// A single output check, after resolving the YAML's heterogeneous shape
/// (bare strings like `non_empty_sections`, or single-key maps like
/// `min_words: 200`) into a concrete validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validator {
    NonEmptySections,
    NoPlaceholderText,
    MinWords(usize),
}

#[derive(Debug, Clone)]
pub struct OutputSpec {
    pub required_sections: Vec<String>,
    pub validators: Vec<Validator>,
}

#[derive(Debug, Clone)]
pub struct StageDefinition {
    pub id: String,
    pub name: String,
    pub inputs: Vec<String>,
    pub min_rigor: Rigor,
    pub template: String,
    pub output: OutputSpec,
}

#[derive(Debug, Deserialize)]
struct RawStageDefinition {
    id: String,
    name: String,
    #[serde(default)]
    inputs: Vec<String>,
    min_rigor: Rigor,
    template: String,
    output: RawOutputSpec,
}

#[derive(Debug, Deserialize)]
struct RawOutputSpec {
    #[serde(default)]
    required_sections: Vec<String>,
    #[serde(default)]
    validators: Vec<ValidatorSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ValidatorSpec {
    Named(String),
    Parameterized(BTreeMap<String, serde_yaml::Value>),
}

fn load_err(path: &str, reason: impl Into<String>) -> StageError {
    StageError::Load {
        path: path.to_string(),
        reason: reason.into(),
    }
}

fn parse_stage_definition(path: &str, text: &str) -> Result<StageDefinition, StageError> {
    let raw: RawStageDefinition =
        serde_yaml::from_str(text).map_err(|e| load_err(path, e.to_string()))?;

    let mut validators = Vec::with_capacity(raw.output.validators.len());
    for spec in raw.output.validators {
        validators.push(resolve_validator(path, spec)?);
    }

    Ok(StageDefinition {
        id: raw.id,
        name: raw.name,
        inputs: raw.inputs,
        min_rigor: raw.min_rigor,
        template: raw.template,
        output: OutputSpec {
            required_sections: raw.output.required_sections,
            validators,
        },
    })
}

fn resolve_validator(path: &str, spec: ValidatorSpec) -> Result<Validator, StageError> {
    match spec {
        ValidatorSpec::Named(name) => match name.as_str() {
            "non_empty_sections" => Ok(Validator::NonEmptySections),
            "no_placeholder_text" => Ok(Validator::NoPlaceholderText),
            other => Err(load_err(path, format!("unknown validator {other:?}"))),
        },
        ValidatorSpec::Parameterized(map) => {
            let mut iter = map.into_iter();
            let (key, value) = match (iter.next(), iter.next()) {
                (Some(kv), None) => kv,
                _ => {
                    return Err(load_err(
                        path,
                        "parameterized validator must have exactly one key",
                    ));
                }
            };
            match key.as_str() {
                "min_words" => {
                    let n = value.as_u64().ok_or_else(|| {
                        load_err(path, "min_words must be a non-negative integer")
                    })?;
                    Ok(Validator::MinWords(n as usize))
                }
                other => Err(load_err(path, format!("unknown validator {other:?}"))),
            }
        }
    }
}

/// Load every `.delta/stages/*.{yaml,yml}` file and return them in
/// topological order (an input always precedes anything that declares it).
pub fn load_all(store: &dyn Store) -> Result<Vec<StageDefinition>, StageError> {
    let dir = Path::new(STAGES_DIR);
    let mut stages = Vec::new();
    for name in store.list_dir(dir)? {
        if !(name.ends_with(".yaml") || name.ends_with(".yml")) {
            continue;
        }
        let rel = dir.join(&name);
        let text = store.read_to_string(&rel)?;
        stages.push(parse_stage_definition(&rel.display().to_string(), &text)?);
    }
    topological_order(stages)
}

/// Kahn's algorithm over the stage input DAG, with checks for duplicate
/// ids, dangling input references, a missing or ambiguous root (a stage
/// with no inputs), and cycles.
fn topological_order(stages: Vec<StageDefinition>) -> Result<Vec<StageDefinition>, StageError> {
    let mut by_id: BTreeMap<String, StageDefinition> = BTreeMap::new();
    for stage in stages {
        let id = stage.id.clone();
        if by_id.insert(id.clone(), stage).is_some() {
            return Err(StageError::InvalidGraph {
                reason: format!("duplicate stage id {id:?}"),
            });
        }
    }

    if by_id.is_empty() {
        return Err(StageError::InvalidGraph {
            reason: "no stage definitions found".to_string(),
        });
    }

    for stage in by_id.values() {
        for input in &stage.inputs {
            if !by_id.contains_key(input) {
                return Err(StageError::InvalidGraph {
                    reason: format!("stage {:?} declares unknown input {input:?}", stage.id),
                });
            }
        }
    }

    let roots: Vec<&str> = by_id
        .values()
        .filter(|s| s.inputs.is_empty())
        .map(|s| s.id.as_str())
        .collect();
    match roots.len() {
        0 => {
            return Err(StageError::InvalidGraph {
                reason: "no root stage (every stage declares inputs)".to_string(),
            });
        }
        1 => {}
        _ => {
            return Err(StageError::InvalidGraph {
                reason: format!("multiple root stages with no inputs: {}", roots.join(", ")),
            });
        }
    }

    let mut ordered_ids: Vec<String> = Vec::with_capacity(by_id.len());
    let mut remaining: Vec<&StageDefinition> = by_id.values().collect();
    while !remaining.is_empty() {
        let ready: Vec<String> = remaining
            .iter()
            .filter(|s| s.inputs.iter().all(|i| ordered_ids.contains(i)))
            .map(|s| s.id.clone())
            .collect();
        if ready.is_empty() {
            return Err(StageError::InvalidGraph {
                reason: "cycle detected among stage inputs".to_string(),
            });
        }
        ordered_ids.extend(ready.iter().cloned());
        remaining.retain(|s| !ready.contains(&s.id));
    }

    Ok(ordered_ids
        .into_iter()
        .filter_map(|id| by_id.get(&id).cloned())
        .collect())
}

#[cfg(test)]
pub(crate) fn default_stages() -> Vec<StageDefinition> {
    vec![
        StageDefinition {
            id: "proposal".to_string(),
            name: "Proposal".to_string(),
            inputs: vec![],
            min_rigor: Rigor::Trivial,
            template: "{{ agents_md }}\n## Current truth\n{{ truth.relevant }}\n## Repository\n{{ repo_tree }}\nDescribe the change.\n"
                .to_string(),
            output: OutputSpec {
                required_sections: vec!["Problem".to_string(), "Approach".to_string()],
                validators: vec![Validator::NonEmptySections, Validator::MinWords(50)],
            },
        },
        StageDefinition {
            id: "design".to_string(),
            name: "Technical Design".to_string(),
            inputs: vec!["proposal".to_string()],
            min_rigor: Rigor::Standard,
            template: "{{ agents_md }}\n## Current truth\n{{ truth.relevant }}\n## Proposal\n{{ inputs.proposal.body }}\nProduce a technical design.\n"
                .to_string(),
            output: OutputSpec {
                required_sections: vec![
                    "Interfaces".to_string(),
                    "Data".to_string(),
                    "Risks".to_string(),
                    "Alternatives Considered".to_string(),
                ],
                validators: vec![Validator::NonEmptySections, Validator::MinWords(200)],
            },
        },
        StageDefinition {
            id: "tasks".to_string(),
            name: "Task Breakdown".to_string(),
            inputs: vec!["proposal".to_string(), "design".to_string()],
            min_rigor: Rigor::Standard,
            template: "{{ inputs.proposal.body }}\n{{ inputs.design.body }}\nBreak the design into tasks.\n"
                .to_string(),
            output: OutputSpec {
                required_sections: vec!["Tasks".to_string()],
                validators: vec![Validator::NonEmptySections, Validator::MinWords(30)],
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rigor_ordering_and_parsing() {
        assert!(Rigor::Trivial < Rigor::Standard);
        assert!(Rigor::Standard < Rigor::Deep);
        assert_eq!("deep".parse::<Rigor>().unwrap(), Rigor::Deep);
        assert!("bogus".parse::<Rigor>().is_err());
    }

    #[test]
    fn parses_the_literal_design_stage_example() {
        let yaml = r#"
id: design
name: Technical Design
inputs: [proposal]
min_rigor: standard
template: |
  Produce a technical design covering interfaces, data, and risks.
output:
  required_sections: [Interfaces, Data, Risks, Alternatives Considered]
  validators:
    - non_empty_sections
    - no_placeholder_text
    - min_words: 200
"#;
        let stage = parse_stage_definition("design.yaml", yaml).unwrap();
        assert_eq!(stage.id, "design");
        assert_eq!(stage.inputs, vec!["proposal"]);
        assert_eq!(stage.min_rigor, Rigor::Standard);
        assert_eq!(
            stage.output.required_sections,
            vec!["Interfaces", "Data", "Risks", "Alternatives Considered"]
        );
        assert_eq!(
            stage.output.validators,
            vec![
                Validator::NonEmptySections,
                Validator::NoPlaceholderText,
                Validator::MinWords(200),
            ]
        );
    }

    #[test]
    fn topological_order_orders_by_dependency() {
        let ordered = topological_order(default_stages()).unwrap();
        let ids: Vec<&str> = ordered.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["proposal", "design", "tasks"]);
    }

    #[test]
    fn topological_order_rejects_duplicate_ids() {
        let mut stages = default_stages();
        stages.push(default_stages().into_iter().next().unwrap());
        let err = topological_order(stages).unwrap_err();
        assert!(matches!(err, StageError::InvalidGraph { .. }));
    }

    #[test]
    fn topological_order_rejects_dangling_input() {
        let mut stages = default_stages();
        stages[1].inputs.push("nonexistent".to_string());
        let err = topological_order(stages).unwrap_err();
        assert!(matches!(err, StageError::InvalidGraph { .. }));
    }

    #[test]
    fn topological_order_rejects_multiple_roots() {
        let mut stages = default_stages();
        stages[1].inputs.clear();
        let err = topological_order(stages).unwrap_err();
        assert!(matches!(err, StageError::InvalidGraph { .. }));
    }

    #[test]
    fn topological_order_rejects_cycles() {
        // `proposal` stays the sole root, but `design` and `tasks` now
        // depend on each other, closing a cycle downstream of it.
        let mut stages = default_stages();
        stages[1].inputs = vec!["tasks".to_string()];
        stages[2].inputs = vec!["design".to_string()];
        let err = topological_order(stages).unwrap_err();
        assert!(matches!(err, StageError::InvalidGraph { .. }));
    }
}
