use std::ops::Range;

use crate::mutant::{Mutant, MutantOutcome, MutationOperator, OperatorCategory};
use crate::project::Project;
use crate::span::SourceSpan;

/// Discover comparison-operator mutants in all source files of a project.
pub fn discover_mutants(project: &Project) -> Vec<Mutant> {
    let mut mutants = Vec::new();

    for src in project.source_files() {
        let path = src.relative_path().to_path_buf();
        let code = match src.read_to_string() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("failed to read source file {:?}: {e}", src.path());
                continue;
            }
        };

        // Compute byte ranges that belong to #[test] functions in this file.
        let test_ranges = find_test_code_ranges(&code);

        for (pattern, op_name, category, replacement) in comparison_mutation_rules() {
            let mut search_start: usize = 0;

            while let Some(idx) = code[search_start..].find(pattern) {
                let start = search_start + idx;
                let end = start + pattern.len();

                // Keeps the search making progress even if `end` is wrong (e.g. under mutation).
                let next_search_start = end.max(start.saturating_add(1)).min(code.len());

                // Skip operators that live inside #[test] functions.
                if in_any_range(start, &test_ranges) {
                    search_start = next_search_start;
                    continue;
                }

                let span = SourceSpan {
                    file: path.clone(),
                    start: start as u32,
                    end: end as u32,
                };

                let mutant = Mutant {
                    id: 0, // placeholder, will be overwritten after sorting
                    operator: MutationOperator {
                        category: category.clone(),
                        name: op_name.to_string(),
                    },
                    span,
                    original_snippet: pattern.to_string(),
                    mutated_snippet: replacement.to_string(),
                    outcome: MutantOutcome::NotRun,
                    duration_ms: None,
                };

                mutants.push(mutant);
                search_start = next_search_start;
            }
        }
    }

    // 1) Sort by file, then by start offset
    mutants.sort_by(|a, b| {
        let key_a = (&a.span.file, a.span.start);
        let key_b = (&b.span.file, b.span.start);
        key_a.cmp(&key_b)
    });

    // 2) Reassign IDs to match sorted order (1-based)
    for (idx, m) in mutants.iter_mut().enumerate() {
        m.id = (idx as u64) + 1;
    }

    mutants
}

/// Simple set of comparison mutation rules for v0.1.
///
/// Multi-character operators go first to avoid partially matching them
/// as single-character operators.
fn comparison_mutation_rules()
-> &'static [(&'static str, &'static str, OperatorCategory, &'static str)] {
    use OperatorCategory::Condition;

    &[
        // equality / inequality
        ("==", "eq_to_neq", Condition, "!="),
        ("!=", "neq_to_eq", Condition, "=="),
        // ordered comparisons
        ("<=", "le_to_gt", Condition, ">"),
        (">=", "ge_to_lt", Condition, "<"),
        ("<", "lt_to_ge", Condition, ">="),
        (">", "gt_to_le", Condition, "<="),
    ]
}

/// Return byte ranges corresponding to the bodies of `#[test]` functions.
///
/// This is a simple textual heuristic similar to noir-metrics: it looks for
/// a `#[test...]` attribute followed by `fn ...`, then tracks `{` / `}`
/// brace depth to find the end of that function body.
fn find_test_code_ranges(code: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();

    let mut pending_test_attr = false;
    let mut inside_test = false;
    let mut brace_depth: i32 = 0;

    let mut offset: usize = 0;
    let mut test_start: Option<usize> = None;

    for line in code.lines() {
        let line_len = line.len();
        let line_start = offset;
        let line_end = offset + line_len;

        let trimmed = line.trim_start();

        if trimmed.starts_with("#[test") {
            pending_test_attr = true;
        } else if trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") {
            if pending_test_attr {
                pending_test_attr = false;
                inside_test = true;
                test_start = Some(line_start);
                brace_depth = 0;
            }
        }

        // Track braces on this line.
        for ch in line.chars() {
            match ch {
                '{' => brace_depth += 1,
                '}' => brace_depth -= 1,
                _ => {}
            }
        }

        if inside_test && brace_depth == 0 {
            // End of test function body.
            let end = line_end + 1; // include trailing newline
            if let Some(start) = test_start.take() {
                ranges.push(start..end);
            }
            inside_test = false;
        }

        // Move offset past this line and its newline.
        offset = line_end + 1;
    }

    ranges
}

/// Return true if `pos` lies inside any of the given byte ranges.
fn in_any_range(pos: usize, ranges: &[Range<usize>]) -> bool {
    ranges.iter().any(|r| pos >= r.start && pos < r.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Project;
    use std::path::PathBuf;

    #[test]
    fn discover_simple_noir_fixture() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let project = Project::from_root(root).expect("Project::from_root should succeed");

        let mutants = discover_mutants(&project);

        insta::assert_debug_snapshot!("discover_simple_noir", mutants);
    }
}
