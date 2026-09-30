//! Diagnostics collected while loading and linting content, reported as `file:line: message`.

use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        })
    }
}

/// One finding, pointing at a data file (and line) when possible.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Short machine-readable category, e.g. `unknown-ref`, `unreachable`.
    pub code: &'static str,
    pub file: Option<PathBuf>,
    pub line: Option<usize>,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.file, self.line) {
            (Some(p), Some(l)) => write!(f, "{}:{}: ", p.display(), l)?,
            (Some(p), None) => write!(f, "{}: ", p.display())?,
            _ => {}
        }
        write!(f, "{}[{}]: {}", self.severity, self.code, self.message)
    }
}

/// A list of diagnostics.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub diags: Vec<Diagnostic>,
}

impl Report {
    pub fn push(&mut self, d: Diagnostic) {
        self.diags.push(d);
    }

    pub fn error(
        &mut self,
        code: &'static str,
        file: Option<PathBuf>,
        line: Option<usize>,
        message: impl Into<String>,
    ) {
        self.push(Diagnostic {
            severity: Severity::Error,
            code,
            file,
            line,
            message: message.into(),
        });
    }

    pub fn warning(
        &mut self,
        code: &'static str,
        file: Option<PathBuf>,
        line: Option<usize>,
        message: impl Into<String>,
    ) {
        self.push(Diagnostic {
            severity: Severity::Warning,
            code,
            file,
            line,
            message: message.into(),
        });
    }

    pub fn info(&mut self, code: &'static str, message: impl Into<String>) {
        self.push(Diagnostic {
            severity: Severity::Info,
            code,
            file: None,
            line: None,
            message: message.into(),
        });
    }

    pub fn errors(&self) -> usize {
        self.count(Severity::Error)
    }

    pub fn warnings(&self) -> usize {
        self.count(Severity::Warning)
    }

    fn count(&self, s: Severity) -> usize {
        self.diags.iter().filter(|d| d.severity == s).count()
    }

    pub fn extend(&mut self, other: Report) {
        self.diags.extend(other.diags);
    }

    /// Diagnostics sorted by severity, then file and line.
    pub fn sorted(&self) -> Vec<&Diagnostic> {
        let mut v: Vec<&Diagnostic> = self.diags.iter().collect();
        v.sort_by(|a, b| {
            a.severity
                .cmp(&b.severity)
                .then_with(|| a.file.cmp(&b.file))
                .then_with(|| a.line.cmp(&b.line))
        });
        v
    }
}
