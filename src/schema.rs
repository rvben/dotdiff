//! The clispec v0.2 contract emitted by `dotdiff schema`.
//!
//! Conforms to <https://clispec.dev/schema/v0.2.json> (validated by a test
//! against the vendored copy in `schemas/clispec-v0.2.json`).

use serde_json::{Value, json};

/// The version of The CLI Spec this document conforms to.
pub const CLISPEC_VERSION: &str = "0.2";

/// Build the clispec contract as a JSON value.
pub fn contract() -> Value {
    json!({
        "clispec": CLISPEC_VERSION,
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "global_args": [
            {
                "name": "--output",
                "type": "string",
                "enum": ["auto", "json", "text"],
                "default": "auto",
                "description": "Output format. auto = text on a TTY, JSON when piped."
            }
        ],
        "commands": [
            {
                "name": "diff",
                "description": "Diff two structured documents (JSON/YAML/TOML/NDJSON) into a path-addressed change list. The default command, invoked as `dotdiff <a> <b>`. Read-only. Exit 0 = identical, 1 = differences found.",
                "mutating": false,
                "stability": "stable",
                "args": [
                    {"name": "a", "type": "path", "required": true, "description": "First (left) input; a file path, or `-` for stdin."},
                    {"name": "b", "type": "path", "required": true, "description": "Second (right) input; a file path, or `-` for stdin."},
                    {"name": "--array-key", "type": "string", "required": false, "description": "Match arrays of objects by this key field instead of by index (order-independent); locators read `items[id=42]`."},
                    {"name": "--format", "type": "string", "required": false, "enum": ["json", "yaml", "toml", "ndjson"], "description": "Force the input format for both sides (default: detect per file by extension/content)."}
                ],
                "output_fields": [
                    {"name": "identical", "type": "boolean", "description": "True when the inputs have no differences."},
                    {"name": "changes", "type": "object[]", "description": "Each change has `op` (added/removed/changed), `path` (a dotpath locator), and `old`/`new` values as applicable."}
                ],
                "example": {"args": ["-", "-"], "stdin": "{\"a\":1}"}
            },
            {
                "name": "schema",
                "description": "Print this clispec contract as JSON.",
                "mutating": false,
                "stability": "stable"
            },
            {
                "name": "completions",
                "description": "Generate a shell completion script.",
                "mutating": false,
                "stability": "stable",
                "args": [
                    {"name": "shell", "type": "string", "required": true, "enum": ["bash", "zsh", "fish", "powershell", "elvish"], "description": "Target shell."}
                ]
            }
        ],
        "outcomes": [
            {"code": 1, "name": "differences_found", "description": "The inputs differ; the change list is on stdout. Not an error - the diff/grep convention."}
        ],
        "errors": [
            {"kind": "usage", "exit_code": 3, "retryable": false, "description": "Invalid command-line arguments (e.g. fewer than two inputs)."},
            {"kind": "parse", "exit_code": 2, "retryable": false, "description": "An input could not be parsed in its detected or forced format."},
            {"kind": "io", "exit_code": 2, "retryable": false, "description": "An input file could not be read."}
        ]
    })
}

/// The contract as a pretty-printed JSON string.
pub fn contract_json() -> String {
    serde_json::to_string_pretty(&contract()).expect("contract serializes")
}
