use std::{fs, path::Path};

use anyhow::{Context, Result};
use tempfile::TempDir;

use crate::mutant::{Mutant, MutantOutcome};
use crate::nargo::{NargoTestResult, run_nargo_test};
use crate::patch::apply_checked_patch;
use crate::project::Project;
use crate::run_report::RunSummary;
use crate::ui::Ui;

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

/// Apply a mutant to the corresponding source file inside a temporary project tree.
///
/// This reads the file from the temp directory, applies the recorded span patch,
/// and writes the mutated contents back to disk.
pub fn apply_mutant_in_temp_tree(temp_root: &Path, mutant: &Mutant) -> Result<()> {
    let temp_file_path = temp_root.join(&mutant.span.file);

    let original = fs::read_to_string(&temp_file_path).with_context(|| {
        format!(
            "failed to read temp file {:?} for mutant {}",
            temp_file_path, mutant.id
        )
    })?;

    let mutated = apply_checked_patch(
        &original,
        &mutant.span,
        &mutant.original_snippet,
        &mutant.mutated_snippet,
    );

    fs::write(&temp_file_path, mutated).with_context(|| {
        format!(
            "failed to write mutated temp file {:?} for mutant {}",
            temp_file_path, mutant.id
        )
    })?;

    Ok(())
}

/// Run `nargo test` on a temporary copy of the project with a single mutant applied.
pub fn run_single_mutant_in_temp(project: &Project, mutant: &Mutant) -> Result<NargoTestResult> {
    let temp = copy_project_to_temp(project)?;
    let temp_root = temp.path();

    apply_mutant_in_temp_tree(temp_root, mutant)?;
    let result = run_nargo_test(temp_root)?;

    Ok(result)
}

/// Naive driver: run all mutants, copying the project for each one.
pub fn run_all_mutants_in_temp(
    project: &Project,
    mutants: &mut [Mutant],
    ui: &Ui,
) -> Result<RunSummary> {
    run_all_mutants_with(project, mutants, run_single_mutant_in_temp, ui)
}

/// Run all mutants using the provided per-mutant runner.
fn run_all_mutants_with(
    project: &Project,
    mutants: &mut [Mutant],
    run_one: fn(&Project, &Mutant) -> Result<NargoTestResult>,
    ui: &Ui,
) -> Result<RunSummary> {
    let mut summary = RunSummary::default();

    for m in mutants.iter_mut() {
        let result = match run_one(project, m) {
            Ok(r) => r,
            Err(e) => {
                ui.runner_error(format!(
                    "failed to run mutant {} in temp project for {:?}: {e}",
                    m.id, m.span.file
                ));
                m.outcome = MutantOutcome::Invalid;
                summary.invalid += 1;
                continue;
            }
        };

        m.duration_ms = Some(result.duration.as_millis() as u64);

        if result.success {
            m.outcome = MutantOutcome::Survived;
            summary.survived += 1;
        } else {
            m.outcome = MutantOutcome::Killed;
            summary.killed += 1;
        }

        ui.mutant_progress(m);
    }

    Ok(summary)
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
    use crate::mutant::{MutationOperator, OperatorCategory};
    use crate::span::SourceSpan;
    use std::path::PathBuf;
    use std::time::Duration;

    fn apply_mutant_in_memory(project: &Project, mutant: &Mutant) -> anyhow::Result<String> {
        let source = project.find_source(&mutant.span.file).ok_or_else(|| {
            anyhow::anyhow!("source file {:?} not part of project", mutant.span.file)
        })?;

        let original = source.read_to_string()?;

        Ok(apply_checked_patch(
            &original,
            &mutant.span,
            &mutant.original_snippet,
            &mutant.mutated_snippet,
        ))
    }

    #[test]
    fn apply_mutant_rewrites_recorded_span() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let project = Project::from_root(root).expect("Project::from_root should succeed");

        let mutants = discover_mutants(&project);
        assert!(!mutants.is_empty());

        let m = &mutants[0];
        let mutated =
            apply_mutant_in_memory(&project, m).expect("apply_mutant_in_memory should succeed");

        let start = m.span.start as usize;
        let end = start + m.mutated_snippet.len();

        let slice = &mutated.as_bytes()[start..end];
        let slice_str = std::str::from_utf8(slice).expect("utf8");
        assert_eq!(slice_str, m.mutated_snippet);
    }

    #[test]
    fn copy_project_creates_temp_tree_with_nr_files() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let project = Project::from_root(root.clone()).expect("Project::from_root should succeed");

        let temp = copy_project_to_temp(&project).expect("copy_project_to_temp should succeed");
        let temp_root = temp.path();

        for fm in &project.metrics.files {
            let orig = project.root().join(&fm.path);
            let copy = temp_root.join(&fm.path);

            assert!(copy.exists(), "expected copied file to exist: {:?}", copy);

            let orig_contents = std::fs::read_to_string(&orig).unwrap();
            let copy_contents = std::fs::read_to_string(&copy).unwrap();

            assert_eq!(
                orig_contents, copy_contents,
                "copied file differs: {:?}",
                fm.path
            );
        }
    }

    #[test]
    fn apply_mutant_in_temp_tree_mutates_copied_file() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let project = Project::from_root(root).expect("Project::from_root should succeed");

        let mutants = discover_mutants(&project);
        assert!(!mutants.is_empty());

        let m = &mutants[0];

        let temp = copy_project_to_temp(&project).expect("copy_project_to_temp should succeed");
        let temp_root = temp.path();

        apply_mutant_in_temp_tree(temp_root, m).expect("apply_mutant_in_temp_tree should succeed");

        let temp_file_path = temp_root.join(&m.span.file);
        let mutated_contents = std::fs::read_to_string(&temp_file_path).unwrap();

        let start = m.span.start as usize;
        let end = start + m.mutated_snippet.len();

        let slice = &mutated_contents.as_bytes()[start..end];
        let slice_str = std::str::from_utf8(slice).expect("utf8");
        assert_eq!(slice_str, m.mutated_snippet);
    }

    #[test]
    fn run_all_mutants_updates_outcomes_and_summary() {
        let root = PathBuf::from("tests/fixtures/simple_noir");
        let project = Project::from_root(root).expect("Project::from_root should succeed");

        let mut mutants = vec![
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
                outcome: MutantOutcome::NotRun,
                duration_ms: None,
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
                outcome: MutantOutcome::NotRun,
                duration_ms: None,
            },
            Mutant {
                id: 3,
                operator: MutationOperator {
                    category: OperatorCategory::Condition,
                    name: "neq_to_eq".to_string(),
                },
                span: SourceSpan {
                    file: PathBuf::from("src/main.nr"),
                    start: 10,
                    end: 12,
                },
                original_snippet: "!=".to_string(),
                mutated_snippet: "==".to_string(),
                outcome: MutantOutcome::NotRun,
                duration_ms: None,
            },
        ];

        fn fake_run_one(_project: &Project, m: &Mutant) -> Result<NargoTestResult> {
            match m.id {
                1 => Ok(NargoTestResult {
                    exit_code: Some(1),
                    success: false,
                    stdout: String::new(),
                    stderr: String::new(),
                    duration: Duration::from_millis(10),
                }),
                2 => Ok(NargoTestResult {
                    exit_code: Some(0),
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                    duration: Duration::from_millis(20),
                }),
                3 => Err(anyhow::anyhow!("simulated failure")),
                _ => unreachable!(),
            }
        }

        let ui = Ui::silent();
        let summary = run_all_mutants_with(&project, &mut mutants, fake_run_one, &ui).unwrap();

        insta::assert_debug_snapshot!("run_all_mutants_summary", summary);
        insta::assert_debug_snapshot!("run_all_mutants_mutants", mutants);
    }
}
