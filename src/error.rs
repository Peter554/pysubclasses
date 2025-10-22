//! Error types for the pysubclasses library.

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for pysubclasses operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when finding Python subclasses.
#[derive(Error, Debug)]
pub enum Error {
    /// The specified class name appears in multiple modules.
    #[error("Class '{name}' found in multiple modules: {}", .candidates.join(", "))]
    AmbiguousClassName {
        name: String,
        candidates: Vec<String>,
    },

    /// The specified class was not found.
    #[error("Class '{name}' not found{}", .module_path.as_ref().map(|m| format!(" in module '{m}'")).unwrap_or_default())]
    ClassNotFound {
        name: String,
        module_path: Option<String>,
    },

    /// IO error occurred while reading files.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Failed to parse a Python file.
    #[error("Failed to parse {}: {error}", .file.display())]
    ParseError { file: PathBuf, error: String },
}
