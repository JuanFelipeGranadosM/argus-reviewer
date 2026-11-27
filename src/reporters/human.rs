use std::fmt::Write;

use super::{Diagnostic, Report, Severity};

const RED: &str = "\x1b[31;1m";
const YELLOW: &str = "\x1b[33;1m";
const CYAN: &str = "\x1b[36m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

struct Palette {
    color: bool,
}

impl Palette {
    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("{code}{text}{RESET}")
        } else {
            text.to_owned()
        }
    }
}

pub fn render(report: &Report, color: bool) -> String {
    let palette = Palette { color };
    let mut out = String::new();
    for diagnostic in &report.diagnostics {
        write_diagnostic(&mut out, diagnostic, &palette);
    }
    let summary = &report.summary;
    let _ = write!(out, "{} archivos analizados", summary.files_scanned);
    if summary.files_generated > 0 {
        let _ = write!(out, ", {} generados omitidos", summary.files_generated);
    }
    let _ = writeln!(
        out,
        " · {} errores, {} avisos",
        summary.blocking_issues, summary.warnings
    );
    out
}

fn write_diagnostic(out: &mut String, diagnostic: &Diagnostic, palette: &Palette) {
    let (label, code) = match diagnostic.severity {
        Severity::Error => ("error", RED),
        Severity::Warning => ("aviso", YELLOW),
    };
    let head = format!("{label}[{}]", diagnostic.id);
    let _ = writeln!(out, "{} {}", palette.paint(code, &head), diagnostic.rule);
    let location = if diagnostic.line_end > diagnostic.line_start {
        format!(
            "{}:{}-{}",
            diagnostic.file, diagnostic.line_start, diagnostic.line_end
        )
    } else {
        format!("{}:{}", diagnostic.file, diagnostic.line_start)
    };
    let _ = writeln!(out, "  {} {location}", palette.paint(CYAN, "-->"));
    let _ = writeln!(out, "  {}", diagnostic.message);
    if let Some(action) = &diagnostic.agent_action {
        let _ = writeln!(
            out,
            "  {}",
            palette.paint(DIM, &format!("= {}", action.instruction))
        );
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reporters::{AgentAction, Summary};

    #[test]
    fn plain_output_has_no_escape_codes() {
        let mut report = Report {
            summary: Summary::default(),
            diagnostics: vec![Diagnostic {
                id: "ARGUS-TEST-001",
                severity: Severity::Error,
                rule: "test-rule",
                file: "a.py".into(),
                line_start: 3,
                line_end: 5,
                message: "mensaje".into(),
                agent_action: Some(AgentAction::new("fix", "arreglar")),
            }],
        };
        report.finalize();
        let text = render(&report, false);
        assert!(!text.contains('\x1b'));
        assert!(text.contains("error[ARGUS-TEST-001] test-rule"));
        assert!(text.contains("--> a.py:3-5"));
        assert!(text.contains("= arreglar"));
        assert!(render(&report, true).contains(RED));
    }
}
