use std::path::Path;

use crate::mutant::{Mutant, MutantOutcome, MutationOperator, OperatorCategory};
use crate::project::Project;
use crate::span::SourceSpan;

/// Discover simple condition-operator mutants in all `.nr` files of the project.
///
/// Currently this looks for basic comparison operators like `==`, `!=`, `<`, `>`,
/// `<=`, `>=` and creates one mutant per occurrence by flipping the operator.
pub fn discover_mutants(project: &Project) -> Vec<Mutant> {
    let mut mutants = Vec::new();
    let mut next_id: u64 = 1;

    for source in project.source_files() {
        let code = match source.read_to_string() {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "warning: failed to read source file {:?}: {e}",
                    source.path()
                );
                continue;
            }
        };

        let file_rel = source.relative_path();
        discover_condition_mutants(&code, file_rel, &mut mutants, &mut next_id);
    }

    mutants
}

/// Helper to discover mutants for comparison operators in a single file.
///
/// This is intentionally simple: it uses plain string matching on the source text
/// and avoids overlapping matches (for example between `<` and `<=`).
fn discover_condition_mutants(
    code: &str,
    file_rel: &Path,
    out: &mut Vec<Mutant>,
    next_id: &mut u64,
) {
    // Handle multi-character operators first so they are not split into
    // overlapping single-character matches.
    let patterns: &[(&str, &str, &str)] = &[
        // original, operator name, replacement
        ("==", "eq_to_neq", "!="),
        ("!=", "neq_to_eq", "=="),
        ("<=", "le_to_gt", ">"),
        (">=", "ge_to_lt", "<"),
        ("<", "lt_to_ge", ">="),
        (">", "gt_to_le", "<="),
    ];

    // Track spans that were already used to avoid overlapping mutants
    let mut used_spans: Vec<(usize, usize)> = Vec::new();

    for (original, op_name, replacement) in patterns {
        for (start, _) in code.match_indices(original) {
            let end = start + original.len();

            if overlaps_existing_span(start, end, &used_spans) {
                continue;
            }

            used_spans.push((start, end));

            let span = SourceSpan {
                file: file_rel.to_path_buf(),
                start: start as u32,
                end: end as u32,
            };

            let operator = MutationOperator {
                category: OperatorCategory::Condition,
                name: op_name.to_string(),
            };

            let mutant = Mutant {
                id: *next_id,
                operator,
                span,
                original_snippet: original.to_string(),
                mutated_snippet: replacement.to_string(),
                outcome: MutantOutcome::NotRun,
                duration_ms: None,
            };

            *next_id += 1;
            out.push(mutant);
        }
    }
}

/// Return true if the candidate span overlaps any span in `used_spans`.
fn overlaps_existing_span(start: usize, end: usize, used_spans: &[(usize, usize)]) -> bool {
    used_spans.iter().any(|(s, e)| !(end <= *s || start >= *e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;
    use std::path::PathBuf;

    /// Snapshot of discovered mutants in the `simple_noir` fixture.
    #[test]
    fn discover_mutants_simple_noir_fixture() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let project = Project::from_root(root).expect("Project::from_root should succeed");

        let mutants = discover_mutants(&project);

        insta::assert_debug_snapshot!("discover_simple_noir_mutants", mutants);
    }

    #[test]
    fn overlaps_existing_span_works() {
        let used = vec![(10, 12), (20, 25)];

        // Completely before
        assert!(!overlaps_existing_span(0, 5, &used));

        // Touching but not overlapping
        assert!(!overlaps_existing_span(5, 10, &used));
        assert!(!overlaps_existing_span(12, 15, &used));

        // Overlapping first span
        assert!(overlaps_existing_span(11, 13, &used));

        // Overlapping second span
        assert!(overlaps_existing_span(22, 30, &used));
    }
}
