use std::ops::Range;
use std::path::Path;

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

        mutants.extend(discover_mutants_in_code(&path, &code));
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

/// Discover mutants in a single file's code (project-relative `path`).
fn discover_mutants_in_code(path: &Path, code: &str) -> Vec<Mutant> {
    let mut mutants = Vec::new();

    // Compute byte ranges that belong to #[test] functions in this file.
    let test_ranges = find_test_code_ranges(code);
    let comment_ranges = find_comment_ranges(code);

    for (pattern, op_name, category, replacement) in comparison_mutation_rules() {
        let mut search_start: usize = 0;

        while let Some(idx) = code[search_start..].find(pattern) {
            let start = search_start + idx;
            let end = start + pattern.len();

            // Keeps the search making progress even if `end` is wrong (e.g. under mutation).
            let mut next_search_start = advance_search_start(start, end, code.len());

            // Ensure forward progress even if `advance_search_start` is wrong (e.g. under mutation).
            if next_search_start <= search_start {
                next_search_start = search_start.saturating_add(1).min(code.len());
            }

            // Avoid overlapping single-character matches inside multi-character operators.
            // Example: `<` should not match the `<` of `<=`, and `>` should not match the `>` of `>=`.
            if should_skip_overlapping_single_char(pattern, code.as_bytes(), start) {
                search_start = next_search_start;
                continue;
            }

            // Skip operators that live inside comments.
            if in_any_range(start, &comment_ranges) {
                search_start = next_search_start;
                continue;
            }

            // Skip operators that live inside #[test] functions.
            if in_any_range(start, &test_ranges) {
                search_start = next_search_start;
                continue;
            }

            let span = SourceSpan {
                file: path.to_path_buf(),
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

    mutants
}

fn should_skip_overlapping_single_char(pattern: &str, bytes: &[u8], start: usize) -> bool {
    if pattern.len() != 1 {
        return false;
    }

    let Some(&next) = bytes.get(start + 1) else {
        return false;
    };

    match pattern {
        "<" => next == b'=',
        ">" => next == b'=',
        _ => false,
    }
}

/// Simple set of comparison mutation rules for v0.1.
///
/// Multi-character operators appear before single-character operators.
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
        } else if (trimmed.starts_with("fn ") || trimmed.starts_with("pub fn "))
            && pending_test_attr
        {
            pending_test_attr = false;
            inside_test = true;
            test_start = Some(line_start);
            brace_depth = 0;
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

/// Return byte ranges corresponding to line (`// ...`) and block (`/* ... */`) comments.
///
/// This is a lightweight lexer:
/// - recognizes `//` to end-of-line
/// - recognizes `/* ... */`
/// - ignores comment openers inside `"..."` and `'...'` (handles simple escapes)
fn find_comment_ranges(code: &str) -> Vec<Range<usize>> {
    let bytes = code.as_bytes();
    let mut ranges = Vec::new();

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    enum State {
        Normal,
        LineComment { start: usize },
        BlockComment { start: usize },
        DoubleString,
        SingleString,
    }

    let mut i = 0usize;
    let mut state = State::Normal;

    while i < bytes.len() {
        match state {
            State::Normal => {
                // start of line or block comment?
                if bytes[i] == b'/' && i + 1 < bytes.len() {
                    let next = bytes[i + 1];
                    if next == b'/' {
                        state = State::LineComment { start: i };
                        i += 2;
                        continue;
                    }
                    if next == b'*' {
                        state = State::BlockComment { start: i };
                        i += 2;
                        continue;
                    }
                }

                // strings (so `//` inside strings doesn't start a comment)
                if bytes[i] == b'"' {
                    state = State::DoubleString;
                    i += 1;
                    continue;
                }
                if bytes[i] == b'\'' {
                    state = State::SingleString;
                    i += 1;
                    continue;
                }

                i += 1;
            }

            State::LineComment { start } => {
                if bytes[i] == b'\n' {
                    ranges.push(start..i); // exclude newline
                    state = State::Normal;
                    i += 1;
                } else {
                    i += 1;
                }
            }

            State::BlockComment { start } => {
                // end `*/`
                if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    i += 2; // include */
                    ranges.push(start..i);
                    state = State::Normal;
                } else {
                    i += 1;
                }
            }

            State::DoubleString => {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2; // skip escaped char
                } else if bytes[i] == b'"' {
                    state = State::Normal;
                    i += 1;
                } else {
                    i += 1;
                }
            }

            State::SingleString => {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                } else if bytes[i] == b'\'' {
                    state = State::Normal;
                    i += 1;
                } else {
                    i += 1;
                }
            }
        }
    }

    // If file ends while still inside a comment, close it at EOF.
    match state {
        State::LineComment { start } | State::BlockComment { start } => {
            ranges.push(start..bytes.len())
        }
        _ => {}
    }

    ranges
}

/// Return true if `pos` lies inside any of the given byte ranges.
fn in_any_range(pos: usize, ranges: &[Range<usize>]) -> bool {
    ranges.iter().any(|r| pos >= r.start && pos < r.end)
}

/// Compute the next `search_start` for a textual pattern scan.
fn advance_search_start(start: usize, end: usize, code_len: usize) -> usize {
    end.max(start.saturating_add(1)).min(code_len)
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

    #[test]
    fn find_test_code_ranges_snapshot_basic() {
        let code = "fn helper() {\n\
                    \tassert(1 == 1);\n\
                    }\n\
                    \n\
                    #[test]\n\
                    fn t() {\n\
                    \tassert(2 == 2);\n\
                    }\n";

        let ranges = find_test_code_ranges(code);

        insta::assert_debug_snapshot!("find_test_code_ranges_basic", ranges);
    }

    fn find_all_positions(haystack: &str, needle: &str) -> Vec<usize> {
        let mut out = Vec::new();
        let mut pos = 0;

        while let Some(idx) = haystack[pos..].find(needle) {
            let abs = pos + idx;
            out.push(abs);
            pos = abs + needle.len();
        }

        out
    }

    #[test]
    fn discover_does_not_create_overlapping_single_char_mutants() {
        let code = r#"
fn a(x: u32, y: u32) {
    assert(x <= y);
    assert(x >= y);
    assert(x < y);
    assert(x > y);
}
"#;

        let path = PathBuf::from("src/main.nr");
        let mutants = discover_mutants_in_code(&path, code);

        // One mutant per operator occurrence: <=, >=, <, >
        assert_eq!(mutants.len(), 4);

        let le_positions = find_all_positions(code, "<=");
        for p in le_positions {
            assert!(
                !mutants
                    .iter()
                    .any(|m| m.span.start as usize == p && m.original_snippet == "<"),
                "unexpected overlapping '<' mutant at '<=' start position {p}"
            );
        }

        let ge_positions = find_all_positions(code, ">=");
        for p in ge_positions {
            assert!(
                !mutants
                    .iter()
                    .any(|m| m.span.start as usize == p && m.original_snippet == ">"),
                "unexpected overlapping '>' mutant at '>=' start position {p}"
            );
        }
    }

    #[test]
    fn test_ranges_cover_test_function_and_exclude_non_test() {
        // Intentionally include the same operator inside a test function and a non-test function.
        let code = r#"
#[test]
fn test_one() {
    let x = 1;
    let y = 2;
    if x < y {
        assert(x == y);
    }
}

fn helper() {
    let a = 1;
    let b = 1;
    assert(a == b);
}
"#;

        let ranges = find_test_code_ranges(code);
        assert_eq!(ranges.len(), 1, "expected exactly one test range");

        let positions = find_all_positions(code, "==");
        assert_eq!(positions.len(), 2, "expected two '==' occurrences");

        let pos_in_test = positions[0];
        let pos_in_non_test = positions[1];

        assert!(
            in_any_range(pos_in_test, &ranges),
            "expected first '==' to be inside test range"
        );

        assert!(
            !in_any_range(pos_in_non_test, &ranges),
            "expected second '==' to be outside test range"
        );
    }

    #[test]
    fn in_any_range_is_start_inclusive_end_exclusive() {
        let code = r#"
#[test]
fn t() {
    assert(1 == 2);
}
"#;

        let ranges = find_test_code_ranges(code);
        assert_eq!(ranges.len(), 1);

        let r = &ranges[0];
        assert!(in_any_range(r.start, &ranges), "start should be included");
        assert!(!in_any_range(r.end, &ranges), "end should be excluded");
    }

    #[test]
    fn advance_search_start_makes_progress_and_stays_in_bounds() {
        assert_eq!(advance_search_start(10, 12, 100), 12);
        assert_eq!(advance_search_start(10, 10, 100), 11);
        assert_eq!(advance_search_start(10, 9, 100), 11);
        assert_eq!(advance_search_start(10, 500, 100), 100);
        assert_eq!(advance_search_start(99, 99, 100), 100);
        assert_eq!(advance_search_start(100, 100, 100), 100);
    }

    #[test]
    fn discover_ignores_line_and_block_comments() {
        let code = r#"
fn helper() {
    // comment with operators: == != <= >=
    /* block comment with operators:
       leaf != 0
       leaf == 0
    */

    let a = 1;
    let b = 2;
    assert(a == b);
    assert(a != b);
}
"#;

        let path = PathBuf::from("src/main.nr");
        let mutants = discover_mutants_in_code(&path, code);

        // Only the two operators in real code should be mutated.
        assert_eq!(
            mutants.len(),
            2,
            "expected only code operators to be mutated"
        );

        let comment_ranges = find_comment_ranges(code);
        for m in &mutants {
            assert!(
                !in_any_range(m.span.start as usize, &comment_ranges),
                "mutant should not be inside a comment: {:?}",
                m
            );
        }
    }
}
