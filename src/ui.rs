use console::{Term, style};
use std::{env, fmt::Display};

use crate::mutant::{Mutant, MutantOutcome};

/// Small UI helper:
/// - normal mode: human output to stdout, errors to stderr
/// - `--json` mode: ALL human output to stderr (stdout stays machine-readable JSON)
/// - fancy styling only on a real TTY and when NO_COLOR/CI are not set
#[derive(Debug, Clone)]
pub struct Ui {
    out: Term,
    err: Term,
    fancy: bool,
    enabled: bool,

    // Observability hooks (used by unit tests and to make behavior measurable for mutation testing).
    // These do not affect output formatting.
    progress_killed: u64,
    progress_survived: u64,
    progress_invalid: u64,
    runner_errors: u64,
}

impl Ui {
    pub fn new(json: bool) -> Self {
        // In --json mode, keep stdout clean for JSON and send all human output to stderr.
        let out = if json { Term::stderr() } else { Term::stdout() };
        let err = Term::stderr();

        // Fancy output must only activate when the actual stream used for human output is a TTY.
        let out_is_tty = out.is_term();

        let no_color = env::var_os("NO_COLOR").is_some();
        let in_ci = env::var_os("CI").is_some();

        let fancy = out_is_tty && !no_color && !in_ci;

        Self {
            out,
            err,
            fancy,
            enabled: true,
            progress_killed: 0,
            progress_survived: 0,
            progress_invalid: 0,
            runner_errors: 0,
        }
    }

    /// Useful for unit tests to avoid noisy output.
    /// Kept behind cfg(test) so it doesn't trigger dead_code in `cargo run`.
    #[cfg(test)]
    pub fn silent() -> Self {
        Self {
            out: Term::stdout(),
            err: Term::stderr(),
            fancy: false,
            enabled: false,
            progress_killed: 0,
            progress_survived: 0,
            progress_invalid: 0,
            runner_errors: 0,
        }
    }

    fn write_out(&self, s: &str) {
        if self.enabled {
            let _ = self.out.write_line(s);
        }
    }

    fn write_err(&self, s: &str) {
        if self.enabled {
            let _ = self.err.write_line(s);
        }
    }

    pub fn line(&self, msg: impl Display) {
        self.write_out(&msg.to_string());
    }

    pub fn title(&self, msg: impl Display) {
        let s = msg.to_string();
        if self.fancy {
            self.write_out(&style(s).bold().to_string());
        } else {
            self.write_out(&s);
        }
    }

    #[allow(dead_code)]
    pub fn warn(&self, msg: impl Display) {
        let s = msg.to_string();
        if self.fancy {
            self.write_err(&style(s).yellow().to_string());
        } else {
            self.write_err(&s);
        }
    }

    pub fn error(&self, msg: impl Display) {
        let s = msg.to_string();
        if self.fancy {
            self.write_err(&style(s).red().bold().to_string());
        } else {
            self.write_err(&s);
        }
    }

    /// Per-mutant progress line.
    ///
    /// Important: in non-fancy mode this prints the exact legacy lines,
    /// so your snapshot tests stay stable (they set NO_COLOR=1 anyway).
    pub fn mutant_progress(&mut self, m: &Mutant) {
        // Track outcomes regardless of output mode.
        match m.outcome {
            MutantOutcome::Killed => self.progress_killed = self.progress_killed.saturating_add(1),
            MutantOutcome::Survived => {
                self.progress_survived = self.progress_survived.saturating_add(1)
            }
            MutantOutcome::Invalid => {
                self.progress_invalid = self.progress_invalid.saturating_add(1)
            }
            MutantOutcome::NotRun => return,
        }

        if !self.fancy {
            match m.outcome {
                MutantOutcome::Survived => {
                    self.line(format!("mutant {} survived (tests still pass)", m.id));
                }
                MutantOutcome::Killed => {
                    self.line(format!(
                        "mutant {} killed (tests failed under mutation)",
                        m.id
                    ));
                }
                _ => {}
            }
            return;
        }

        let tag = match m.outcome {
            MutantOutcome::Killed => style("KILLED").red().bold(),
            MutantOutcome::Survived => style("SURVIVED").green().bold(),
            MutantOutcome::Invalid => style("INVALID").yellow().bold(),
            MutantOutcome::NotRun => return,
        };

        let dur = m
            .duration_ms
            .map(|ms| format!("{ms}ms"))
            .unwrap_or_else(|| "-".to_string());

        let file = m.span.file.display();
        let op = format!("{:?}/{}", m.operator.category, m.operator.name);
        let change = format!("{:?} -> {:?}", m.original_snippet, m.mutated_snippet);

        self.line(format!(
            "{tag} {dur:>6}  #{id} {file} [{start}..{end}] {op}: {change}",
            tag = tag,
            id = m.id,
            start = m.span.start,
            end = m.span.end,
        ));
    }

    /// Used for runner errors; keeps stderr/stdout routing consistent.
    pub fn runner_error(&mut self, msg: impl Display) {
        self.runner_errors += 1;
        self.error(msg);
    }

    #[allow(dead_code)]
    pub fn is_fancy(&self) -> bool {
        self.fancy && self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::discover_mutants;
    use crate::project::Project;
    use std::path::PathBuf;

    #[test]
    fn is_fancy_requires_fancy_and_enabled() {
        let base = Ui {
            out: Term::stdout(),
            err: Term::stderr(),
            fancy: false,
            enabled: false,
            progress_killed: 0,
            progress_survived: 0,
            progress_invalid: 0,
            runner_errors: 0,
        };

        let mut a = base.clone();
        a.fancy = false;
        a.enabled = false;
        assert!(!a.is_fancy());

        let mut b = base.clone();
        b.fancy = true;
        b.enabled = false;
        assert!(!b.is_fancy());

        let mut c = base.clone();
        c.fancy = false;
        c.enabled = true;
        assert!(!c.is_fancy());

        let mut d = base.clone();
        d.fancy = true;
        d.enabled = true;
        assert!(d.is_fancy());
    }

    #[test]
    fn runner_error_increments_counter() {
        let mut ui = Ui::silent();
        assert_eq!(ui.runner_errors, 0);
        ui.runner_error("boom");
        assert_eq!(ui.runner_errors, 1);
        ui.runner_error("boom2");
        assert_eq!(ui.runner_errors, 2);
    }

    #[test]
    fn mutant_progress_tracks_killed_and_survived() {
        let project = Project::from_root(PathBuf::from("tests/fixtures/simple_noir"))
            .expect("fixture project should load");
        let mut mutants = discover_mutants(&project);
        assert!(!mutants.is_empty(), "expected at least one mutant");

        let mut m = mutants.remove(0);

        let mut ui = Ui::silent();

        m.outcome = MutantOutcome::Killed;
        ui.mutant_progress(&m);
        assert_eq!(ui.progress_killed, 1);
        assert_eq!(ui.progress_survived, 0);

        m.outcome = MutantOutcome::Survived;
        ui.mutant_progress(&m);
        assert_eq!(ui.progress_killed, 1);
        assert_eq!(ui.progress_survived, 1);
    }
}
