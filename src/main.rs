use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use argus_reviewer::config::Config;
use argus_reviewer::lexer::python;
use argus_reviewer::reporters::{agent, human};
use argus_reviewer::scan::scan;
use clap::{Parser, Subcommand, ValueEnum};

/// Código de salida: 0 sin errores bloqueantes, 1 con errores bloqueantes, 2 fallo de la herramienta.
const EXIT_BLOCKING: u8 = 1;
const EXIT_TOOL_FAILURE: u8 = 2;

#[derive(Parser)]
#[command(
    name = "argus",
    version,
    about = "Auditoría estática determinista para código generado por agentes"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Audita un proyecto o archivo. Sale con código 1 si hay errores bloqueantes.
    Scan {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Human)]
        format: Format,
        /// Archivo de configuración; por defecto `<path>/.argus.toml` si existe.
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// Vuelca los tokens de un archivo Python (herramienta de desarrollo del lexer).
    #[command(hide = true)]
    Tokens { path: PathBuf },
}

#[derive(Copy, Clone, ValueEnum)]
enum Format {
    Human,
    /// JSON determinista para agentes y harness.
    Agent,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Scan {
            path,
            format,
            config,
        } => run_scan(&path, format, config.as_deref()),
        Command::Tokens { path } => dump_tokens(&path),
    }
}

fn tool_failure(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("argus: {message}");
    ExitCode::from(EXIT_TOOL_FAILURE)
}

fn run_scan(path: &Path, format: Format, config: Option<&Path>) -> ExitCode {
    let config = match Config::discover(path, config) {
        Ok(config) => config,
        Err(err) => return tool_failure(err),
    };
    let report = match scan(path, &config) {
        Ok(report) => report,
        Err(err) => return tool_failure(err),
    };
    let output = match format {
        Format::Human => {
            let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
            human::render(&report, color)
        }
        Format::Agent => match agent::render(&report) {
            Ok(json) => json,
            Err(err) => return tool_failure(err),
        },
    };
    println!("{output}");
    if report.has_blocking_issues() {
        ExitCode::from(EXIT_BLOCKING)
    } else {
        ExitCode::SUCCESS
    }
}

fn dump_tokens(path: &Path) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) => return tool_failure(format!("no se pudo leer {}: {err}", path.display())),
    };
    let lexed = match python::lex(&source) {
        Ok(lexed) => lexed,
        Err(err) => return tool_failure(format!("{}: {err}", path.display())),
    };
    for token in &lexed.tokens {
        let range = token.range;
        println!(
            "{}..{} {:?} {:?}",
            range.start,
            range.end,
            token.kind,
            token.text(&source)
        );
    }
    for error in &lexed.errors {
        eprintln!(
            "{}..{} error: {}",
            error.range.start, error.range.end, error.kind
        );
    }
    if lexed.errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
