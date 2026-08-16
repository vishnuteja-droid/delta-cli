//! Output validation: does a stage's generated body satisfy its declared
//! `output.required_sections` and `output.validators`?

use crate::stage::{OutputSpec, Validator};

const PLACEHOLDER_MARKERS: [&str; 5] = ["TODO", "TBD", "LOREM IPSUM", "XXX", "FIXME"];

/// Check `body` against `output`'s required sections and validators.
/// Returns one human-readable failure description per problem found;
/// an empty result means the output passed.
pub fn validate(output: &OutputSpec, body: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for validator in &output.validators {
        match validator {
            Validator::NonEmptySections => {
                failures.extend(check_non_empty_sections(&output.required_sections, body));
            }
            Validator::NoPlaceholderText => {
                if let Some(marker) = find_placeholder(body) {
                    failures.push(format!("body contains placeholder text ({marker})"));
                }
            }
            Validator::MinWords(min) => {
                let words = body.split_whitespace().count();
                if words < *min {
                    failures.push(format!("body has {words} words, expected at least {min}"));
                }
            }
        }
    }
    failures
}

struct Heading {
    level: usize,
    /// Index of the first line of the section's content (just after the
    /// heading line itself).
    start: usize,
}

fn find_heading(lines: &[&str], name: &str) -> Option<Heading> {
    lines.iter().enumerate().find_map(|(i, line)| {
        let trimmed = line.trim_start();
        let level = trimmed.chars().take_while(|c| *c == '#').count();
        if level == 0 {
            return None;
        }
        let title = trimmed[level..].trim();
        if title.eq_ignore_ascii_case(name) {
            Some(Heading {
                level,
                start: i + 1,
            })
        } else {
            None
        }
    })
}

fn check_non_empty_sections(required_sections: &[String], body: &str) -> Vec<String> {
    let lines: Vec<&str> = body.lines().collect();
    let mut failures = Vec::new();
    for section in required_sections {
        let Some(heading) = find_heading(&lines, section) else {
            failures.push(format!("missing required section {section:?}"));
            continue;
        };
        // A section's content runs until the next heading at the same or a
        // shallower level; deeper subheadings and plain text both count as
        // content.
        let content_is_empty = lines[heading.start..]
            .iter()
            .take_while(|line| {
                let trimmed = line.trim_start();
                let level = trimmed.chars().take_while(|c| *c == '#').count();
                level == 0 || level > heading.level
            })
            .all(|line| line.trim().is_empty());
        if content_is_empty {
            failures.push(format!("section {section:?} is empty"));
        }
    }
    failures
}

fn find_placeholder(body: &str) -> Option<&'static str> {
    let upper = body.to_ascii_uppercase();
    PLACEHOLDER_MARKERS
        .into_iter()
        .find(|marker| upper.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::default_stages;

    fn design_output() -> OutputSpec {
        default_stages()
            .into_iter()
            .find(|s| s.id == "design")
            .unwrap()
            .output
    }

    #[test]
    fn passes_a_well_formed_body() {
        let body = "## Interfaces\nsome interface notes\n## Data\nsome data notes\n## Risks\nsome risk notes\n## Alternatives Considered\nan alternative\n".to_string()
            + &"word ".repeat(200);
        let failures = validate(&design_output(), &body);
        assert!(failures.is_empty(), "unexpected failures: {failures:?}");
    }

    #[test]
    fn flags_missing_section() {
        let body = "## Interfaces\nnotes\n";
        let failures = validate(&design_output(), body);
        assert!(failures.iter().any(|f| f.contains("Data")));
    }

    #[test]
    fn flags_empty_section() {
        let body =
            "## Interfaces\n\n## Data\nnotes\n## Risks\nnotes\n## Alternatives Considered\nnotes\n";
        let failures = validate(&design_output(), body);
        assert!(failures.iter().any(|f| f.contains("\"Interfaces\"")));
    }

    #[test]
    fn flags_placeholder_text() {
        let output = OutputSpec {
            required_sections: vec![],
            validators: vec![Validator::NoPlaceholderText],
        };
        let failures = validate(&output, "Some text with a TODO left in it.");
        assert_eq!(failures.len(), 1);
    }

    #[test]
    fn flags_too_few_words() {
        let output = OutputSpec {
            required_sections: vec![],
            validators: vec![Validator::MinWords(5)],
        };
        let failures = validate(&output, "only two words");
        assert_eq!(failures.len(), 1);
    }
}
