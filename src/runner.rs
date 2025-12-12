use anyhow::{Context, Result};

use crate::mutant::Mutant;
use crate::patch::apply_span_patch;
use crate::project::Project;

/// Apply a single mutant to its source file and return the mutated source code.
///
/// This helper reads the original file from disk and returns a mutated
/// version of its contents as a string. It does not modify any files.
pub fn apply_mutant_in_memory(project: &Project, mutant: &Mutant) -> Result<String> {
    // Look up the corresponding source file in the project.
    let source = project
        .find_source(&mutant.span.file)
        .ok_or_else(|| anyhow::anyhow!("source file {:?} not part of project", mutant.span.file))?;

    // Load the original contents.
    let original = source
        .read_to_string()
        .with_context(|| format!("failed to read source file {:?}", source.path()))?;

    // Apply the textual patch at the recorded span.
    let mutated = apply_span_patch(&original, &mutant.span, &mutant.mutated_snippet);

    Ok(mutated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::discover_mutants;
    use crate::project::Project;
    use std::path::PathBuf;

    #[test]
    fn apply_mutant_rewrites_recorded_span() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let project = Project::from_root(root).expect("Project::from_root should succeed");

        let mutants = discover_mutants(&project);
        assert!(
            !mutants.is_empty(),
            "expected discover_mutants to find at least one mutant"
        );

        // Use the first mutant; ordering is deterministic.
        let m = &mutants[0];

        let mutated =
            apply_mutant_in_memory(&project, m).expect("apply_mutant_in_memory should succeed");

        let start = m.span.start as usize;
        let end = start + m.mutated_snippet.len();

        assert!(
            end <= mutated.len(),
            "mutated source shorter than expected span"
        );

        let slice = &mutated.as_bytes()[start..end];
        let slice_str = std::str::from_utf8(slice).expect("mutated slice should be valid UTF-8");

        assert_eq!(
            slice_str, m.mutated_snippet,
            "replacement not present at expected span"
        );
    }
}
