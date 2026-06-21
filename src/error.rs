//! Error type, the stable error `kind` set, and the exit-code contract.
//!
//! Errors are reported as a clispec structured envelope on the last line of
//! stderr. Finding differences is NOT an error: it is the `differences_found`
//! outcome (exit 1), declared under `outcomes` in the schema.
//!
//! Exit codes:
//! - `2` an input could not be read (`io`) or parsed (`parse`)
//! - `3` usage error (bad arguments)

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    /// Invalid command-line arguments (also wraps clap errors).
    #[error("{message}")]
    Usage { message: String },

    /// An input could not be parsed in its (detected or forced) format.
    #[error("{label}: invalid {format}: {message}")]
    Parse {
        label: String,
        format: String,
        message: String,
    },

    /// An input file could not be read.
    #[error("{path}: {message}")]
    Io { path: String, message: String },
}

impl Error {
    /// Stable snake_case identifier consumers branch on (the schema `errors` set).
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Usage { .. } => "usage",
            Error::Parse { .. } => "parse",
            Error::Io { .. } => "io",
        }
    }

    /// Actionable remediation, when there is one.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Error::Usage { .. } => Some("see `dotdiff --help` or `dotdiff schema`"),
            Error::Parse { .. } => {
                Some("force the format with --format if detection guessed wrong")
            }
            Error::Io { .. } => None,
        }
    }

    /// The process exit code associated with this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Parse { .. } | Error::Io { .. } => 2,
            Error::Usage { .. } => 3,
        }
    }
}
