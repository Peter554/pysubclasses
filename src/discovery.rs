//! File discovery module for finding Python files in a directory tree.

use ignore::Walk;
use std::path::{Path, PathBuf};

use crate::error::Result;

/// Discovers all Python files in a directory tree.
///
/// Uses the `ignore` crate to respect `.gitignore` files and other VCS ignore patterns.
/// This automatically skips common directories like `__pycache__`, `.venv`, etc. if they
/// are gitignored.
///
/// # Arguments
///
/// * `root_dir` - The root directory to start searching from
///
/// # Returns
///
/// A vector of paths to all `.py` files found in the directory tree.
///
/// # Examples
///
/// ```no_run
/// use pysubclasses::discovery::discover_python_files;
/// use std::path::Path;
///
/// let files = discover_python_files(Path::new("./src")).unwrap();
/// for file in files {
///     println!("{}", file.display());
/// }
/// ```
pub fn discover_python_files(root_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut python_files = Vec::new();

    for entry in Walk::new(root_dir) {
        let entry = entry.map_err(std::io::Error::other)?;
        let path = entry.path();

        // Only include files (not directories) with .py extension
        if entry.file_type().is_some_and(|ft| ft.is_file())
            && path
                .extension()
                .is_some_and(|s| s.to_str().is_some_and(|s| s == "py"))
        {
            python_files.push(path.to_path_buf());
        }
    }

    Ok(python_files)
}
