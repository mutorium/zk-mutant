use std::{fs, path::Path};

use anyhow::{Context, Result};
use tempfile::TempDir;

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

/// Copy the entire Noir project into a fresh temporary directory.
///
/// The returned [`TempDir`] keeps the directory alive for the duration of its
/// lifetime and removes it on drop.
pub fn copy_project_to_temp(project: &Project) -> Result<TempDir> {
    let temp = TempDir::new().context("failed to create temporary directory")?;

    copy_dir_recursive(project.root(), temp.path()).with_context(|| {
        format!(
            "failed to copy project from {:?} to {:?}",
            project.root(),
            temp.path()
        )
    })?;

    Ok(temp)
}

/// Recursively copy all files and directories from `src` into `dst`.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("failed to create dir {:?}", dst))?;

    for entry in fs::read_dir(src).with_context(|| format!("failed to read dir {:?}", src))? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target)
                .with_context(|| format!("failed to copy file {:?} to {:?}", path, target))?;
        }
    }

    Ok(())
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

    #[test]
    fn copy_project_creates_temp_tree_with_nr_files() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let project = Project::from_root(root.clone()).expect("Project::from_root should succeed");

        let temp = copy_project_to_temp(&project).expect("copy_project_to_temp should succeed");
        let temp_root = temp.path();

        for fm in &project.metrics().files {
            let orig = project.root().join(&fm.path);
            let copy = temp_root.join(&fm.path);

            assert!(copy.exists(), "expected copied file to exist: {:?}", copy);

            let orig_contents = std::fs::read_to_string(&orig)
                .expect("failed to read original file for comparison");
            let copy_contents =
                std::fs::read_to_string(&copy).expect("failed to read copied file for comparison");

            assert_eq!(
                orig_contents, copy_contents,
                "copied file contents differ for {:?}",
                fm.path
            );
        }
    }
}
