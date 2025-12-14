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
}

impl Ui {
    pub fn new(json: bool) -> Self {
        // In --json mode, keep stdout clean for JSON and send all human output to stderr.
        let out = if json { Term::stderr() } else { Term::stdout() };
        let err = Term::stderr();

        // IMPORTANT:
        // Fancy output must only activate when the *actual output stream we write human output to*
        // is a real TTY. Otherwise we might emit ANSI styling into a pipe/file.
        let out_is_tty = out.is_term();

        let no_color = env::var_os("NO_COLOR").is_some();
        let in_ci = env::var_os("CI").is_some();

        let fancy = out_is_tty && !no_color && !in_ci;

        Self {
            out,
            err,
            fancy,
            enabled: true,
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
    /// Important: in non-fancy mode this prints the *exact legacy lines*,
    /// so your snapshot tests stay stable (they set NO_COLOR=1 anyway).
    pub fn mutant_progress(&self, m: &Mutant) {
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
    pub fn runner_error(&self, msg: impl Display) {
        self.error(msg);
    }

    #[allow(dead_code)]
    pub fn is_fancy(&self) -> bool {
        self.fancy && self.enabled
    }
}
