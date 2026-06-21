//! dotdiff: a semantic diff for JSON, YAML, TOML, and NDJSON.
//!
//! `dotpick` extracts fields; dotdiff tells you what changed between two
//! structured documents - a path-addressed change list instead of line noise.
//! The whole pipeline is reachable through [`run`], which the CLI and tests
//! both use.

mod diff;
mod error;
mod load;
mod output;
pub mod schema;

pub use diff::{Change, DiffOptions, Op, diff};
pub use error::Error;
pub use load::load;
pub use output::render;

/// Rendered output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

/// An input data format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Yaml,
    Toml,
    Ndjson,
}

/// One side of the diff: its raw text, an optional forced format, and a label
/// used in error messages (a filename or `<stdin>`).
#[derive(Debug, Clone)]
pub struct Input {
    pub text: String,
    pub format: Option<Format>,
    pub label: String,
}

/// A complete diff request.
#[derive(Debug, Clone)]
pub struct Request {
    pub left: Input,
    pub right: Input,
    /// Match arrays of objects by this key field instead of by index.
    pub array_key: Option<String>,
    pub output: OutputFormat,
}

/// The result of a diff: whether the inputs were identical, the rendered
/// output, and the number of changes (so the CLI can pick the exit code).
#[derive(Debug, Clone)]
pub struct DiffOutcome {
    pub identical: bool,
    pub rendered: String,
    pub change_count: usize,
}

/// Load both inputs, diff them, and render. Parsing or IO failures are errors;
/// finding differences is not (the caller maps it to exit 1).
pub fn run(req: &Request) -> Result<DiffOutcome, Error> {
    let left = load(&req.left.text, req.left.format, &req.left.label)?;
    let right = load(&req.right.text, req.right.format, &req.right.label)?;
    let opts = DiffOptions {
        array_key: req.array_key.clone(),
    };
    let changes = diff(&left, &right, &opts);
    Ok(DiffOutcome {
        identical: changes.is_empty(),
        rendered: render(&changes, req.output),
        change_count: changes.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(text: &str) -> Input {
        Input {
            text: text.to_string(),
            format: None,
            label: "x".to_string(),
        }
    }

    #[test]
    fn run_diffs_across_formats() {
        // Same data, different formats - must be identical.
        let req = Request {
            left: input(r#"{"a": 1, "b": 2}"#),
            right: input("a: 1\nb: 2\n"),
            array_key: None,
            output: OutputFormat::Json,
        };
        let out = run(&req).unwrap();
        assert!(out.identical, "yaml and json with same data are identical");
        assert_eq!(out.change_count, 0);
    }

    #[test]
    fn run_reports_changes_and_count() {
        let req = Request {
            left: input(r#"{"a": 1}"#),
            right: input(r#"{"a": 2}"#),
            array_key: None,
            output: OutputFormat::Json,
        };
        let out = run(&req).unwrap();
        assert!(!out.identical);
        assert_eq!(out.change_count, 1);
        assert!(out.rendered.contains("\"path\":\"a\""));
    }

    #[test]
    fn run_propagates_parse_errors() {
        let req = Request {
            left: Input {
                text: "{ broken".into(),
                format: Some(Format::Json),
                label: "left".into(),
            },
            right: input("{}"),
            array_key: None,
            output: OutputFormat::Json,
        };
        let e = run(&req).unwrap_err();
        assert_eq!(e.kind(), "parse");
        assert_eq!(e.exit_code(), 2);
    }
}
