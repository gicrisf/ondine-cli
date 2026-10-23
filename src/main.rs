//! Native command-line renderer for Ondine Mermaid diagrams.
//!
//! The command reads Mermaid source from files or standard input and writes SVG to a file or
//! standard output. The compiler crate owns preprocessing, parsing, layout, and SVG emission.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;

/// Renders Mermaid source to SVG.
#[derive(Debug, Parser)]
#[command(name = "ondine", version, about)]
struct Cli {
    /// Mermaid source files. With none, reads standard input.
    files: Vec<PathBuf>,
    /// Directory for rendered files. Defaults to each source file's directory.
    #[arg(short, long, value_name = "DIR")]
    out_dir: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.files.is_empty() {
        return render_stdin();
    }

    let now = now_millis();
    if let Some(directory) = &cli.out_dir
        && let Err(error) = fs::create_dir_all(directory)
    {
        eprintln!("{}: {error}", directory.display());
        return ExitCode::FAILURE;
    }

    let mut failed = false;
    for file in &cli.files {
        if !render_file(file, cli.out_dir.as_deref(), now) {
            failed = true;
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Renders standard input to standard output.
fn render_stdin() -> ExitCode {
    let mut source = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut source) {
        eprintln!("<stdin>: {error}");
        return ExitCode::FAILURE;
    }

    let Some(svg) = render(&source, "<stdin>", now_millis()) else {
        return ExitCode::FAILURE;
    };

    let mut stdout = io::stdout();
    match stdout
        .write_all(svg.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("<stdout>: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Renders one file and writes the SVG beside it or into `out_dir`. Returns whether it succeeded.
fn render_file(file: &Path, out_dir: Option<&Path>, now: f64) -> bool {
    let source = match fs::read_to_string(file) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{}: {error}", file.display());
            return false;
        }
    };

    let name = file.display().to_string();
    let Some(svg) = render(&source, &name, now) else {
        return false;
    };

    let output = output_path(file, out_dir);
    if let Err(error) = fs::write(&output, svg) {
        eprintln!("{}: {error}", output.display());
        return false;
    }

    eprintln!("{} -> {}", file.display(), output.display());
    true
}

/// Renders `source` and reports a compiler error against `name`.
fn render(source: &str, name: &str, now: f64) -> Option<String> {
    match ondine_compiler::render(source, "", now) {
        Ok(rendered) => Some(rendered.svg),
        Err(error) => {
            if error.line > 0 {
                eprintln!("{name}:{}:{}: {}", error.line, error.column, error.message);
            } else {
                eprintln!("{name}: {}", error.message);
            }
            None
        }
    }
}

/// Returns the SVG path, replacing only a `.mmd` or `.mermaid` suffix.
fn output_path(file: &Path, out_dir: Option<&Path>) -> PathBuf {
    let mut name = file.file_name().map_or_else(OsString::new, OsString::from);

    let extension = file.extension().and_then(|value| value.to_str());
    if extension.is_some_and(|value| {
        value.eq_ignore_ascii_case("mmd") || value.eq_ignore_ascii_case("mermaid")
    }) && let Some(stem) = file.file_stem()
    {
        name = stem.to_os_string();
    }
    name.push(".svg");

    match out_dir {
        Some(directory) => directory.join(name),
        None => file.with_file_name(name),
    }
}

/// Returns the current time in milliseconds since the epoch, as gantt charts expect.
fn now_millis() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64() * 1000.0)
}
