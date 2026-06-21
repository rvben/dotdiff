//! dotdiff CLI.
//!
//! Follows The CLI Spec (clispec.dev): text on a TTY, JSON when piped,
//! structured error envelopes on the last line of stderr, a `schema`
//! subcommand. Read-only. Exit 0 = identical, 1 = differences found (the
//! `differences_found` outcome), 2 = parse/IO, 3 = usage.

use std::io::{IsTerminal, Read, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::error::ErrorKind as ClapErrorKind;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use dotdiff::{Error, Format, Input, OutputFormat, Request, run, schema};
use serde_json::json;

#[derive(Parser)]
#[command(
    name = "dotdiff",
    version,
    about = "Semantic diff for JSON, YAML, TOML, and NDJSON: a structured, agent-friendly change-list instead of line noise.",
    long_about = "Semantic diff for JSON, YAML, TOML, and NDJSON. Compares two documents \
                  structurally (not line by line) and prints a path-addressed change \
                  list. Cross-format works (a.yaml vs b.json). Either path may be `-` \
                  for stdin.\n\n\
                  Exit codes: 0 identical, 1 differences found, 2 parse/IO error, 3 usage.\n\n\
                  Run `dotdiff schema` for the machine-readable contract (clispec.dev).",
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true
)]
struct Cli {
    /// First (left) input: a file path, or `-` for stdin.
    #[arg(value_name = "A")]
    a: Option<String>,

    /// Second (right) input: a file path, or `-` for stdin.
    #[arg(value_name = "B")]
    b: Option<String>,

    /// Match arrays of objects by this key field instead of by index.
    #[arg(long, value_name = "FIELD")]
    array_key: Option<String>,

    /// Force the input format for both sides (default: detect per file).
    #[arg(long, value_enum, value_name = "FORMAT")]
    format: Option<CliFormat>,

    /// Output format; auto = text on a TTY, JSON when piped.
    #[arg(long, short = 'o', value_enum, default_value = "auto", global = true)]
    output: CliOutput,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Print the machine-readable contract (clispec.dev) as JSON.
    Schema,
    /// Generate a shell completion script.
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum CliFormat {
    Json,
    Yaml,
    Toml,
    Ndjson,
}

impl From<CliFormat> for Format {
    fn from(f: CliFormat) -> Self {
        match f {
            CliFormat::Json => Format::Json,
            CliFormat::Yaml => Format::Yaml,
            CliFormat::Toml => Format::Toml,
            CliFormat::Ndjson => Format::Ndjson,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum CliOutput {
    Auto,
    Json,
    Text,
}

impl CliOutput {
    fn resolve(self) -> OutputFormat {
        match self {
            CliOutput::Json => OutputFormat::Json,
            CliOutput::Text => OutputFormat::Text,
            CliOutput::Auto => {
                if std::io::stdout().is_terminal() {
                    OutputFormat::Text
                } else {
                    OutputFormat::Json
                }
            }
        }
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => return handle_clap_error(e),
    };

    match &cli.command {
        Some(Command::Schema) => {
            println!("{}", schema::contract_json());
            return ExitCode::SUCCESS;
        }
        Some(Command::Completions { shell }) => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(*shell, &mut cmd, name, &mut std::io::stdout());
            return ExitCode::SUCCESS;
        }
        None => {}
    }

    let (Some(a), Some(b)) = (cli.a.clone(), cli.b.clone()) else {
        return fail(&Error::Usage {
            message: "two inputs required (try `dotdiff a.json b.json`)".into(),
        });
    };

    let forced = cli.format.map(Format::from);
    let left = match read_input(&a, forced) {
        Ok(input) => input,
        Err(err) => return fail(&err),
    };
    let right = match read_input(&b, forced) {
        Ok(input) => input,
        Err(err) => return fail(&err),
    };

    let request = Request {
        left,
        right,
        array_key: cli.array_key.clone(),
        output: cli.output.resolve(),
    };
    match run(&request) {
        Ok(outcome) => {
            if !outcome.rendered.is_empty() {
                let _ = writeln!(std::io::stdout(), "{}", outcome.rendered);
            }
            if outcome.identical {
                ExitCode::SUCCESS
            } else {
                // `differences_found` outcome - a data state, not a failure.
                ExitCode::from(1)
            }
        }
        Err(err) => fail(&err),
    }
}

/// Read an input from a path (or stdin for `-`), choosing its format from the
/// forced override, else the file extension, else content detection (`None`).
fn read_input(arg: &str, forced: Option<Format>) -> Result<Input, Error> {
    if arg == "-" {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|e| Error::Io {
                path: "<stdin>".into(),
                message: e.to_string(),
            })?;
        return Ok(Input {
            text,
            format: forced,
            label: "<stdin>".into(),
        });
    }
    let text = std::fs::read_to_string(arg).map_err(|e| Error::Io {
        path: arg.to_string(),
        message: e.to_string(),
    })?;
    Ok(Input {
        text,
        format: forced.or_else(|| format_from_path(arg)),
        label: arg.to_string(),
    })
}

/// Map a file extension to a format, when recognised.
fn format_from_path(path: &str) -> Option<Format> {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())?
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => Some(Format::Json),
        "yaml" | "yml" => Some(Format::Yaml),
        "toml" => Some(Format::Toml),
        "ndjson" | "jsonl" => Some(Format::Ndjson),
        _ => None,
    }
}

fn fail(err: &Error) -> ExitCode {
    emit_error(err);
    ExitCode::from(err.exit_code() as u8)
}

/// Help and version print normally and exit 0; every other clap failure becomes
/// a structured `usage` error envelope.
fn handle_clap_error(e: clap::Error) -> ExitCode {
    match e.kind() {
        ClapErrorKind::DisplayHelp
        | ClapErrorKind::DisplayVersion
        | ClapErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            let _ = e.print();
            ExitCode::SUCCESS
        }
        _ => fail(&Error::Usage {
            message: e.to_string().trim().to_string(),
        }),
    }
}

/// Write the clispec error envelope as the last line of stderr.
fn emit_error(err: &Error) {
    let mut error = serde_json::Map::new();
    error.insert("kind".into(), json!(err.kind()));
    error.insert("message".into(), json!(err.to_string()));
    error.insert("exit_code".into(), json!(err.exit_code()));
    if let Some(hint) = err.hint() {
        error.insert("hint".into(), json!(hint));
    }
    eprintln!("{}", json!({ "error": error }));
}
