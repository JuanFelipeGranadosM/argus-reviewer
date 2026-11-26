use std::path::PathBuf;
use std::process::ExitCode;

use argus_reviewer::lexer::python;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "argus", version, about = "Auditoría estática determinista para código generado por agentes")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Vuelca los tokens de un archivo Python (herramienta de desarrollo del lexer).
    #[command(hide = true)]
    Tokens { path: PathBuf },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Tokens { path } => dump_tokens(&path),
    }
}

fn dump_tokens(path: &PathBuf) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("argus: no se pudo leer {}: {err}", path.display());
            return ExitCode::from(2);
        }
    };
    let lexed = match python::lex(&source) {
        Ok(lexed) => lexed,
        Err(err) => {
            eprintln!("argus: {}: {err}", path.display());
            return ExitCode::from(2);
        }
    };
    for token in &lexed.tokens {
        let range = token.range;
        println!("{}..{} {:?} {:?}", range.start, range.end, token.kind, token.text(&source));
    }
    for error in &lexed.errors {
        eprintln!("{}..{} error: {}", error.range.start, error.range.end, error.kind);
    }
    if lexed.errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
