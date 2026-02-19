use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Parse error at {path}:{line}: {message}")]
    Parse {
        path: PathBuf,
        line: usize,
        message: String,
    },

    #[error("Variable '{0}' is not set")]
    MissingVariable(String),

    #[error("Required variables missing: {}", .0.join(", "))]
    RequiredMissing(Vec<String>),

    #[error("Cannot parse value for '{0}': {1}")]
    ParseValue(String, String),

    #[error("Circular interpolation detected: {0}")]
    CircularInterpolation(String),

    #[error("Root path does not exist or is not a directory: {0}")]
    InvalidRoot(PathBuf),
}

pub type Result<T> = std::result::Result<T, Error>;
