use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::mutant::{Mutant, MutantOutcome};
use crate::project::Project;
use crate::report::format_mutant_with_location;
use crate::run_report::MutationRunReport;

/// Write `mutants.json` containing all discovered mutants (pre-limit).
pub fn write_mutants_json(out_dir: &Path, mutants: &[Mutant]) -> Result<()> {
    let path = out_dir.join("mutants.json");
    write_pretty_json(&path, mutants)
}

/// Write `outcomes.json` as a compact list of outcomes for executed mutants.
pub fn write_outcomes_json(out_dir: &Path, report: &MutationRunReport) -> Result<()> {
    #[derive(Debug, Serialize)]
    struct OutcomeEntry {
        id: u64,
        file: PathBuf,
        start: u32,
        end: u32,
        category: crate::mutant::OperatorCategory,
        name: String,
        outcome: MutantOutcome,
        duration_ms: Option<u64>,
    }

    #[derive(Debug, Serialize)]
    struct OutcomesFile {
        tool: &'static str,
        version: &'static str,
        project_root: PathBuf,
        discovered: usize,
        executed: usize,
        baseline: crate::run_report::BaselineReport,
        summary: crate::run_report::RunSummary,
        mutants: Vec<OutcomeEntry>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    }

    let mut entries: Vec<OutcomeEntry> = report
        .mutants
        .iter()
        .map(|m| OutcomeEntry {
            id: m.id,
            file: m.span.file.clone(),
            start: m.span.start,
            end: m.span.end,
            category: m.operator.category.clone(),
            name: m.operator.name.clone(),
            outcome: m.outcome.clone(),
            duration_ms: m.duration_ms,
        })
        .collect();

    entries.sort_by_key(|e| e.id);

    let file = OutcomesFile {
        tool: report.tool,
        version: report.version,
        project_root: report.project_root.clone(),
        discovered: report.discovered,
        executed: report.executed,
        baseline: report.baseline.clone(),
        summary: report.summary.clone(),
        mutants: entries,
        error: report.error.clone(),
    };

    let path = out_dir.join("outcomes.json");
    write_pretty_json(&path, &file)
}

/// Write cargo-mutants-style outcome lists:
/// - caught.txt   (killed)
/// - missed.txt   (survived)
/// - unviable.txt (invalid)
pub fn write_outcome_txts(out_dir: &Path, project: &Project, mutants: &[Mutant]) -> Result<()> {
    write_txt_for(
        out_dir.join("caught.txt"),
        project,
        mutants,
        MutantOutcome::Killed,
    )?;
    write_txt_for(
        out_dir.join("missed.txt"),
        project,
        mutants,
        MutantOutcome::Survived,
    )?;
    write_txt_for(
        out_dir.join("unviable.txt"),
        project,
        mutants,
        MutantOutcome::Invalid,
    )?;
    Ok(())
}

/// Write a minimal `diff/000001.diff` file per mutant (snippet-based).
pub fn write_diff_dir(out_dir: &Path, mutants: &[Mutant]) -> Result<()> {
    let diff_dir = out_dir.join("diff");
    fs::create_dir_all(&diff_dir)
        .with_context(|| format!("failed to create diff dir {:?}", diff_dir))?;

    let mut ordered: Vec<&Mutant> = mutants.iter().collect();
    ordered.sort_by_key(|m| m.id);

    for m in ordered {
        // Skip diffs for non-executed mutants.
        if m.outcome == MutantOutcome::NotRun {
            continue;
        }

        let file = m.span.file.display().to_string();
        let op = format!("{:?}/{}", m.operator.category, m.operator.name);

        let content = format!(
            "--- {file}\n+++ {file}\n@@ [{start}..{end}] {op}\n- {orig:?}\n+ {mutated:?}\n",
            start = m.span.start,
            end = m.span.end,
            orig = m.original_snippet,
            mutated = m.mutated_snippet,
        );

        let path = diff_dir.join(format!("{:06}.diff", m.id));
        fs::write(&path, content).with_context(|| format!("failed to write {:?}", path))?;
    }

    Ok(())
}

/// Write a stable `log` file (no timestamps) with baseline + summary + error.
pub fn write_log(out_dir: &Path, report: &MutationRunReport) -> Result<()> {
    let path = out_dir.join("log");

    let mut lines = Vec::new();
    lines.push(format!("tool: {}", report.tool));
    lines.push(format!("version: {}", report.version));
    lines.push(format!("project_root: {}", report.project_root.display()));
    lines.push(format!("discovered: {}", report.discovered));
    lines.push(format!("executed: {}", report.executed));
    lines.push(format!(
        "baseline: success={} exit_code={:?} duration_ms={}",
        report.baseline.success, report.baseline.exit_code, report.baseline.duration_ms
    ));
    lines.push(format!(
        "summary: killed={} survived={} invalid={}",
        report.summary.killed, report.summary.survived, report.summary.invalid
    ));
    if let Some(err) = &report.error {
        lines.push(format!("error: {err}"));
    }

    let content = lines.join("\n") + "\n";
    fs::write(&path, content).with_context(|| format!("failed to write {:?}", path))?;
    Ok(())
}

fn write_txt_for(
    path: PathBuf,
    project: &Project,
    mutants: &[Mutant],
    want: MutantOutcome,
) -> Result<()> {
    let mut ordered: Vec<&Mutant> = mutants.iter().filter(|m| m.outcome == want).collect();
    ordered.sort_by_key(|m| m.id);

    // The file is created even when the list is empty.
    let mut out = String::new();
    for m in ordered {
        out.push_str(&format_mutant_with_location(project, m));
        out.push('\n');
    }

    fs::write(&path, out).with_context(|| format!("failed to write {:?}", path))?;
    Ok(())
}

// `?Sized` allows passing unsized values such as slices (e.g. `&[Mutant]` where `T = [Mutant]`).
fn write_pretty_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let json = serde_json::to_string_pretty(value).context("serialize json")?;
    fs::write(path, json).with_context(|| format!("failed to write {:?}", path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::discover_mutants;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn non_empty_lines(s: &str) -> usize {
        s.lines().filter(|l| !l.trim().is_empty()).count()
    }

    #[test]
    fn outcome_txts_bucket_exactly_matching_outcomes() {
        let project = Project::from_root(PathBuf::from("tests/fixtures/simple_noir"))
            .expect("fixture project should load");
        let mut discovered = discover_mutants(&project);
        assert!(
            discovered.len() >= 4,
            "expected at least 4 mutants in fixture"
        );

        // Keep it small and deterministic: 4 mutants with 4 distinct outcomes.
        let mut m1 = discovered.remove(0);
        let mut m2 = discovered.remove(0);
        let mut m3 = discovered.remove(0);
        let mut m4 = discovered.remove(0);

        m1.outcome = MutantOutcome::Killed;
        m2.outcome = MutantOutcome::Survived;
        m3.outcome = MutantOutcome::Invalid;
        m4.outcome = MutantOutcome::NotRun;

        let mutants = vec![m1, m2, m3, m4];

        let td = TempDir::new().expect("TempDir should create");
        write_outcome_txts(td.path(), &project, &mutants)
            .expect("write_outcome_txts should succeed");

        let caught = fs::read_to_string(td.path().join("caught.txt")).expect("read caught.txt");
        let missed = fs::read_to_string(td.path().join("missed.txt")).expect("read missed.txt");
        let unviable =
            fs::read_to_string(td.path().join("unviable.txt")).expect("read unviable.txt");

        assert_eq!(
            non_empty_lines(&caught),
            1,
            "caught.txt should list only killed"
        );
        assert_eq!(
            non_empty_lines(&missed),
            1,
            "missed.txt should list only survived"
        );
        assert_eq!(
            non_empty_lines(&unviable),
            1,
            "unviable.txt should list only invalid"
        );
    }
}
