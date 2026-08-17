//! Stage-keyed status vocabulary: an original word list per stage,
//! rotated roughly every 4s in the header while a stage's provider call
//! or the `dlt build` tool loop is in flight. Stage-aware status text is
//! chrome no competing tool can produce, since none of them own both
//! the lifecycle and the renderer — per `PLAN.md`'s own framing of why
//! this detail matters.

use std::time::Duration;

pub const ROTATE_INTERVAL: Duration = Duration::from_secs(4);

/// The word list for `stage`. `"build"` covers `dlt build`'s tool loop
/// specifically (not the `tasks` YAML stage) — it's the one PLAN.md
/// names directly ("patching/regressing during build"), and it's where
/// `apply_patch`/`write_file`/`run_command` actually run.
pub fn words_for_stage(stage: &str) -> &'static [&'static str] {
    match stage {
        "proposal" => &[
            "interrogating",
            "clarifying",
            "scoping",
            "probing",
            "framing",
        ],
        "design" => &[
            "reconciling",
            "weighing",
            "sketching",
            "triangulating",
            "structuring",
        ],
        "tasks" => &[
            "sequencing",
            "decomposing",
            "itemizing",
            "ordering",
            "chunking",
        ],
        "build" => &["patching", "regressing", "wiring", "grinding", "iterating"],
        "verify" => &["checking", "asserting", "confirming", "auditing"],
        _ => &["working", "thinking", "processing"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_the_literal_plan_word_pairs() {
        assert!(words_for_stage("proposal").contains(&"interrogating"));
        assert!(words_for_stage("proposal").contains(&"clarifying"));
        assert!(words_for_stage("design").contains(&"reconciling"));
        assert!(words_for_stage("design").contains(&"weighing"));
        assert!(words_for_stage("build").contains(&"patching"));
        assert!(words_for_stage("build").contains(&"regressing"));
    }

    #[test]
    fn unknown_stage_falls_back_to_generic_words() {
        assert_eq!(
            words_for_stage("mystery"),
            &["working", "thinking", "processing"]
        );
    }

    #[test]
    fn every_list_has_at_least_two_words_to_actually_rotate_through() {
        for stage in ["proposal", "design", "tasks", "build", "verify", "other"] {
            assert!(
                words_for_stage(stage).len() >= 2,
                "stage {stage:?} has too few words to rotate"
            );
        }
    }
}
