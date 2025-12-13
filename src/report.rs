use crate::mutant::{Mutant, MutantOutcome};
use crate::project::Project;

/// Print a detailed list of all mutants and their outcomes.
pub fn print_all_mutants(project: &Project, mutants: &[Mutant]) {
    for line in render_all_mutants(project, mutants) {
        println!("{line}");
    }
}

/// Print a short list of surviving mutants.
pub fn print_surviving_mutants(project: &Project, mutants: &[Mutant]) {
    for line in render_surviving_mutants(project, mutants) {
        println!("{line}");
    }
}

/// Render a detailed list of all mutants and their outcomes.
pub fn render_all_mutants(project: &Project, mutants: &[Mutant]) -> Vec<String> {
    if mutants.is_empty() {
        return Vec::new();
    }

    let ordered = collect_sorted(mutants.iter());

    let mut out = Vec::with_capacity(ordered.len() + 1);
    out.push("--- mutants (detailed) ---".to_string());

    for m in ordered {
        let outcome = outcome_label(&m.outcome);
        let duration = duration_label(m.duration_ms);
        let base = format_mutant_with_location(project, m);

        out.push(format!("{:>8} {:>8} {}", outcome, duration, base));
    }

    out
}

/// Render a short list of surviving mutants.
pub fn render_surviving_mutants(project: &Project, mutants: &[Mutant]) -> Vec<String> {
    let survivors = collect_sorted(
        mutants
            .iter()
            .filter(|m| m.outcome == MutantOutcome::Survived),
    );

    if survivors.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(survivors.len() + 1);
    out.push(format!(
        "--- surviving mutants ({} of {}) ---",
        survivors.len(),
        mutants.len()
    ));

    for m in survivors {
        out.push(format_mutant_with_location(project, m));
    }

    out
}

fn collect_sorted<'a>(iter: impl Iterator<Item = &'a Mutant>) -> Vec<&'a Mutant> {
    let mut v: Vec<&'a Mutant> = iter.collect();
    v.sort_by_key(|m| m.id);
    v
}

fn outcome_label(outcome: &MutantOutcome) -> &'static str {
    match outcome {
        MutantOutcome::NotRun => "not_run",
        MutantOutcome::Killed => "killed",
        MutantOutcome::Survived => "survived",
        MutantOutcome::Invalid => "invalid",
    }
}

fn duration_label(duration_ms: Option<u64>) -> String {
    match duration_ms {
        Some(ms) => format!("{ms}ms"),
        None => "-".to_string(),
    }
}

/// Format one mutant as a single, readable line using line/column positions when possible.
///
/// Falls back to byte spans when the source file cannot be read.
pub fn format_mutant_with_location(project: &Project, m: &Mutant) -> String {
    let source = match project.find_source(&m.span.file) {
        Some(s) => s,
        None => return format_mutant_short(m),
    };

    let code = match source.read_to_string() {
        Ok(c) => c,
        Err(_) => return format_mutant_short(m),
    };

    let start = m.span.start as usize;
    let end = m.span.end as usize;

    let Some((sl, sc)) = byte_offset_to_line_col(&code, start) else {
        return format_mutant_short(m);
    };

    let Some((el, ec)) = byte_offset_to_line_col(&code, end) else {
        return format_mutant_short(m);
    };

    let file = m.span.file.display();

    format!(
        "#{id} {file}:{sl}:{sc}-{el}:{ec} {category:?}/{name}: {orig:?} -> {mutated:?}",
        id = m.id,
        category = m.operator.category,
        name = m.operator.name,
        orig = m.original_snippet,
        mutated = m.mutated_snippet,
    )
}

/// Format one mutant as a single, readable line.
pub fn format_mutant_short(m: &Mutant) -> String {
    let file = m.span.file.display();
    let start = m.span.start;
    let end = m.span.end;

    format!(
        "#{id} {file} [{start}..{end}] {category:?}/{name}: {orig:?} -> {mutated:?}",
        id = m.id,
        category = m.operator.category,
        name = m.operator.name,
        orig = m.original_snippet,
        mutated = m.mutated_snippet,
    )
}

/// Convert a byte offset into a 1-based (line, column) location.
///
/// Column counts Unicode scalar values on the line segment.
fn byte_offset_to_line_col(code: &str, offset: usize) -> Option<(usize, usize)> {
    if offset > code.len() {
        return None;
    }

    let prefix = &code[..offset];

    let line = prefix.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1;
    let line_start = prefix.rfind('\n').map(|pos| pos + 1).unwrap_or(0);
    let col = code[line_start..offset].chars().count() + 1;

    Some((line, col))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutant::{MutationOperator, OperatorCategory};
    use crate::project::Project;
    use crate::span::SourceSpan;
    use std::path::PathBuf;

    #[test]
    fn format_short_is_stable() {
        let m = Mutant {
            id: 7,
            operator: MutationOperator {
                category: OperatorCategory::Condition,
                name: "eq_to_neq".to_string(),
            },
            span: SourceSpan {
                file: PathBuf::from("src/main.nr"),
                start: 12,
                end: 14,
            },
            original_snippet: "==".to_string(),
            mutated_snippet: "!=".to_string(),
            outcome: MutantOutcome::Survived,
            duration_ms: Some(123),
        };

        insta::assert_debug_snapshot!("format_mutant_short", format_mutant_short(&m));
    }

    #[test]
    fn render_survivors_snapshot_fixture() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let project = Project::from_root(root).expect("Project::from_root should succeed");

        let mutants = vec![
            Mutant {
                id: 1,
                operator: MutationOperator {
                    category: OperatorCategory::Condition,
                    name: "lt_to_ge".to_string(),
                },
                span: SourceSpan {
                    file: PathBuf::from("src/main.nr"),
                    start: 0,
                    end: 1,
                },
                original_snippet: "<".to_string(),
                mutated_snippet: ">=".to_string(),
                outcome: MutantOutcome::Killed,
                duration_ms: Some(10),
            },
            Mutant {
                id: 2,
                operator: MutationOperator {
                    category: OperatorCategory::Condition,
                    name: "eq_to_neq".to_string(),
                },
                span: SourceSpan {
                    file: PathBuf::from("src/utils.nr"),
                    start: 0,
                    end: 2,
                },
                original_snippet: "==".to_string(),
                mutated_snippet: "!=".to_string(),
                outcome: MutantOutcome::Survived,
                duration_ms: Some(20),
            },
        ];

        insta::assert_debug_snapshot!(
            "render_surviving_mutants",
            render_surviving_mutants(&project, &mutants)
        );
    }
}
