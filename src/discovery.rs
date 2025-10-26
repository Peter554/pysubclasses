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
pub fn discover_python_files(root_dir: &Path) -> Result<Vec<PathBuf>> {
    discover_python_files_with_exclusions(root_dir, &[])
}

/// Discovers all Python files in a directory tree, excluding specified directories.
///
/// Uses the `ignore` crate to respect `.gitignore` files and other VCS ignore patterns.
/// Additionally excludes directories specified by the caller.
///
/// # Arguments
///
/// * `root_dir` - The root directory to start searching from
/// * `exclude_dirs` - Directories to exclude from the search (can be relative or absolute)
///
/// # Returns
///
/// A vector of paths to all `.py` files found in the directory tree.
pub fn discover_python_files_with_exclusions(
    root_dir: &Path,
    exclude_dirs: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let mut python_files = Vec::new();

    // Canonicalize exclusion paths relative to root_dir
    let canonical_excludes: Vec<PathBuf> = exclude_dirs
        .iter()
        .filter_map(|exclude_path| {
            // Try as absolute first, then relative to root_dir
            if exclude_path.is_absolute() {
                exclude_path.canonicalize().ok()
            } else {
                root_dir.join(exclude_path).canonicalize().ok()
            }
        })
        .collect();

    for entry in Walk::new(root_dir) {
        let entry = entry.map_err(std::io::Error::other)?;
        let path = entry.path();

        // Check if this path is under any excluded directory
        let is_excluded = canonical_excludes
            .iter()
            .any(|excluded| path.starts_with(excluded));

        if is_excluded {
            continue;
        }

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
