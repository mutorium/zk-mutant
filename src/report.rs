use crate::mutant::{Mutant, MutantOutcome};

/// Print a short list of surviving mutants.
///
/// The output includes file path, byte span, operator name, and the textual
/// replacement (original -> mutated).
pub fn print_surviving_mutants(mutants: &[Mutant]) {
    let mut survivors: Vec<&Mutant> = mutants
        .iter()
        .filter(|m| m.outcome == MutantOutcome::Survived)
        .collect();

    if survivors.is_empty() {
        return;
    }

    survivors.sort_by_key(|m| m.id);

    println!(
        "--- surviving mutants ({} of {}) ---",
        survivors.len(),
        mutants.len()
    );

    for m in survivors {
        println!("{}", format_mutant_short(m));
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutant::{MutationOperator, OperatorCategory};
    use crate::span::SourceSpan;
    use std::path::PathBuf;

    #[test]
    fn format_is_stable() {
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
}
