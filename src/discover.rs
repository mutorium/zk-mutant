use crate::mutant::{Mutant, MutantOutcome, MutationOperator, OperatorCategory};
use crate::project::Project;
use crate::span::SourceSpan;

/// Discover mutation opportunities in a Noir project.
///
/// For now this is a stub that returns an empty list. The structure matches
/// where real discovery logic will live later.
pub fn discover_mutants(project: &Project) -> Vec<Mutant> {
    let _sources = project.source_files();

    // Placeholder: no mutants discovered yet.
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn disover_no_mutants_yet_for_simple_project() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let project = Project::from_root(root).expect("Project::from_root should suceed");

        let mutants = discover_mutants(&project);

        // No mutants discovered yet.
        assert!(mutants.is_empty());
    }
}
