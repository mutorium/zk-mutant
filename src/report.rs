use crate::mutant::{Mutant, MutantOutcome};
use crate::project::Project;

/// Print a detailed list of all mutants and their outcomes.
#[allow(dead_code)]
pub fn print_all_mutants(project: &Project, mutants: &[Mutant]) {
    if mutants.is_empty() {
        return;
    }

    let ordered = collect_sorted(mutants.iter());

    println!("--- mutants (detailed) ---");
    for m in ordered {
        let outcome = outcome_label(&m.outcome);
        let duration = duration_label(m.duration_ms);
        let base = format_mutant_with_location(project, m);

        println!("{:>8} {:>8} {}", outcome, duration, base);
    }
}

/// Print a short list of surviving mutants.
///
/// The output includes file path, line/column range, operator name, and the textual
/// replacement (original -> mutated).
pub fn print_surviving_mutants(project: &Project, mutants: &[Mutant]) {
    let survivors = collect_sorted(
        mutants
            .iter()
            .filter(|m| m.outcome == MutantOutcome::Survived),
    );

    if survivors.is_empty() {
        return;
    }

    println!(
        "--- surviving mutants ({} of {}) ---",
        survivors.len(),
        mutants.len()
    );

    for m in survivors {
        println!("{}", format_mutant_with_location(project, m));
    }
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
    fn byte_offset_to_line_col_basic() {
        let code = "a\nbcd\nef";
        // "bcd" starts at byte offset 2
        assert_eq!(byte_offset_to_line_col(code, 0), Some((1, 1))); // 'a'
        assert_eq!(byte_offset_to_line_col(code, 1), Some((1, 2))); // after 'a'
        assert_eq!(byte_offset_to_line_col(code, 2), Some((2, 1))); // 'b'
        assert_eq!(byte_offset_to_line_col(code, 4), Some((2, 3))); // 'd'
        assert_eq!(byte_offset_to_line_col(code, 6), Some((3, 1))); // 'e'
        assert_eq!(byte_offset_to_line_col(code, code.len()), Some((3, 3))); // end of file
    }
}
