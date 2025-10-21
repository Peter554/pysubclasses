//! File discovery module for finding Python files in a directory tree.

use ignore::WalkBuilder;
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

    let walker = WalkBuilder::new(root_dir)
        .hidden(false) // Include hidden files (some projects have .hidden dirs)
        .git_ignore(true) // Respect .gitignore
        .git_global(true) // Respect global gitignore
        .git_exclude(true) // Respect .git/info/exclude
        .ignore(true) // Respect .ignore files
        .follow_links(false) // Don't follow symlinks to avoid infinite loops
        .build();

    for entry in walker {
        let entry = entry.map_err(std::io::Error::other)?;
        let path = entry.path();

        // Only include files (not directories) with .py extension
        if entry.file_type().is_some_and(|ft| ft.is_file())
            && path.extension().and_then(|s| s.to_str()) == Some("py")
        {
            python_files.push(path.to_path_buf());
        }
    }

    Ok(python_files)
}

/// Converts a file path to a Python module path.
///
/// # Arguments
///
/// * `file_path` - The absolute or relative path to a Python file
/// * `root_dir` - The root directory of the Python project
///
/// # Returns
///
/// The dotted module path (e.g., `foo.bar.baz` for `root/foo/bar/baz.py`).
/// Returns `None` if the file path cannot be converted to a module path.
///
/// # Examples
///
/// ```
/// use pysubclasses::discovery::file_path_to_module_path;
/// use std::path::Path;
///
/// let module = file_path_to_module_path(
///     Path::new("/project/src/foo/bar.py"),
///     Path::new("/project/src")
/// );
/// assert_eq!(module, Some("foo.bar".to_string()));
///
/// // __init__.py files become the parent package
/// let module = file_path_to_module_path(
///     Path::new("/project/src/foo/__init__.py"),
///     Path::new("/project/src")
/// );
/// assert_eq!(module, Some("foo".to_string()));
/// ```
pub fn file_path_to_module_path(file_path: &Path, root_dir: &Path) -> Option<String> {
    // Get the relative path from root_dir to file_path
    let rel_path = file_path.strip_prefix(root_dir).ok()?;

    // Remove the .py extension
    let without_ext = rel_path.with_extension("");

    // Convert path components to module path
    let components: Vec<&str> = without_ext
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    if components.is_empty() {
        return None;
    }

    // Handle __init__.py - it represents the parent package
    let module_parts: Vec<&str> = if components.last() == Some(&"__init__") {
        components[..components.len() - 1].to_vec()
    } else {
        components
    };

    if module_parts.is_empty() {
        return None;
    }

    Some(module_parts.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_path_to_module_path() {
        // Regular module
        let path = Path::new("/project/src/foo/bar/baz.py");
        let root = Path::new("/project/src");
        assert_eq!(
            file_path_to_module_path(path, root),
            Some("foo.bar.baz".to_string())
        );

        // __init__.py
        let path = Path::new("/project/src/foo/bar/__init__.py");
        let root = Path::new("/project/src");
        assert_eq!(
            file_path_to_module_path(path, root),
            Some("foo.bar".to_string())
        );

        // Top-level module
        let path = Path::new("/project/src/module.py");
        let root = Path::new("/project/src");
        assert_eq!(
            file_path_to_module_path(path, root),
            Some("module".to_string())
        );

        // Top-level __init__.py (edge case)
        let path = Path::new("/project/src/__init__.py");
        let root = Path::new("/project/src");
        assert_eq!(file_path_to_module_path(path, root), None);
    }

    #[test]
    fn test_file_path_to_module_path_relative() {
        // Test with matching prefixes
        let path = Path::new("./foo/bar.py");
        let root = Path::new(".");
        assert_eq!(
            file_path_to_module_path(path, root),
            Some("foo.bar".to_string())
        );
    }
}
