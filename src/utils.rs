//! Utility functions for module path and import resolution.

use std::path::Path;

use crate::parser::Import;

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
/// use pysubclasses::utils::file_path_to_module_path;
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
    let module_parts = if components.last() == Some(&"__init__") {
        &components[..components.len() - 1]
    } else {
        &components[..]
    };

    if module_parts.is_empty() {
        return None;
    }

    Some(module_parts.join("."))
}

/// Resolves a name to a fully qualified module path.
///
/// Given a name used in the code and the imports in the file, determine
/// what module path it refers to.
///
/// # Arguments
///
/// * `name` - The name to resolve (e.g., "Animal")
/// * `imports` - The imports in the current file
/// * `current_module` - The current module path
/// * `is_package` - Whether the current module is a package (__init__.py)
///
/// # Returns
///
/// The fully qualified name, or None if it cannot be resolved.
pub fn resolve_name(
    name: &str,
    imports: &[Import],
    current_module: &str,
    is_package: bool,
) -> Option<String> {
    for import in imports {
        match import {
            Import::Module { module, alias } => {
                // import foo.bar as baz OR import foo.bar
                if alias.as_ref().unwrap_or(module) == name {
                    return Some(module.clone());
                }
            }
            Import::From { module, names } => {
                // from foo import Bar [as Baz]
                if let Some((n, _)) = names
                    .iter()
                    .find(|(n, alias)| alias.as_ref().unwrap_or(n) == name)
                {
                    return Some(format!("{module}.{n}"));
                }
            }
            Import::RelativeFrom {
                level,
                module: rel_module,
                names,
            } => {
                // from .relative import Bar
                if let Some((n, _)) = names
                    .iter()
                    .find(|(n, alias)| alias.as_ref().unwrap_or(n) == name)
                {
                    let base = resolve_relative_import_base(current_module, *level, is_package)?;
                    return Some(match (base.is_empty(), rel_module) {
                        (true, None) => n.to_string(),
                        (true, Some(m)) => format!("{m}.{n}"),
                        (false, None) => format!("{base}.{n}"),
                        (false, Some(m)) => format!("{base}.{m}.{n}"),
                    });
                }
            }
        }
    }
    None
}

/// Resolves a relative import to an absolute module path.
///
/// This follows Python's relative import semantics as described in PEP 328.
///
/// # Arguments
///
/// * `current_module` - The current module path (e.g., "foo.bar.baz")
/// * `level` - Number of dots in the relative import (from Python AST)
/// * `is_package` - Whether the current module is a package (__init__.py file)
///
/// # Returns
///
/// The base module path to which the relative import is resolved.
pub fn resolve_relative_import_base(
    current_module: &str,
    level: usize,
    is_package: bool,
) -> Option<String> {
    if level == 0 {
        return None; // Not a relative import
    }

    let parts: Vec<&str> = current_module.split('.').collect();

    let base = if is_package {
        // For packages (__init__.py files)
        if level == 1 {
            // Single dot means "this package"
            current_module.to_string()
        } else {
            // Multiple dots: go up (level - 1) parent packages
            let components_to_keep = parts.len().saturating_sub(level - 1);
            if components_to_keep == 0 {
                String::new()
            } else {
                parts[..components_to_keep].join(".")
            }
        }
    } else {
        // For regular modules
        let components_to_keep = parts.len().saturating_sub(level);
        if components_to_keep == 0 {
            String::new()
        } else {
            parts[..components_to_keep].join(".")
        }
    };

    Some(base)
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

    #[test]
    fn test_resolve_relative_import_base() {
        // Package with level=1: stay in current package
        // "foo.bar.baz" package (__init__.py) with level=1 stays at "foo.bar.baz"
        assert_eq!(
            resolve_relative_import_base("foo.bar.baz", 1, true),
            Some("foo.bar.baz".to_string())
        );
        // Package with level=2: go up 1 parent
        assert_eq!(
            resolve_relative_import_base("foo.bar.baz", 2, true),
            Some("foo.bar".to_string())
        );
        // Package with level=3: go up 2 parents
        assert_eq!(
            resolve_relative_import_base("foo.bar.baz", 3, true),
            Some("foo".to_string())
        );

        // Regular module with level=1: go to containing package
        // "foo.bar.baz" module with level=1 goes to "foo.bar"
        assert_eq!(
            resolve_relative_import_base("foo.bar.baz", 1, false),
            Some("foo.bar".to_string())
        );

        // Single-component package with level=1: stay in package
        assert_eq!(
            resolve_relative_import_base("mypackage", 1, true),
            Some("mypackage".to_string())
        );
        // Single-component module with level=1: go to empty (top level)
        assert_eq!(
            resolve_relative_import_base("mypackage", 1, false),
            Some(String::new())
        );
    }

    /// Test case for resolve_name function
    struct Case {
        name: &'static str,
        imports: Vec<Import>,
        current_module: &'static str,
        is_package: bool,
        expected: Option<&'static str>,
    }

    #[yare::parameterized(
        from_import_direct = {
            Case {
                name: "Dog",
                imports: vec![Import::From {
                    module: "animals".to_string(),
                    names: vec![("Dog".to_string(), None)],
                }],
                current_module: "test.module",
                is_package: false,
                expected: Some("animals.Dog"),
            }
        },
        from_import_with_alias = {
            Case {
                name: "Kitty",
                imports: vec![Import::From {
                    module: "pets".to_string(),
                    names: vec![("Cat".to_string(), Some("Kitty".to_string()))],
                }],
                current_module: "test.module",
                is_package: false,
                expected: Some("pets.Cat"),
            }
        },
        from_import_alias_no_match = {
            Case {
                name: "Cat",
                imports: vec![Import::From {
                    module: "pets".to_string(),
                    names: vec![("Cat".to_string(), Some("Kitty".to_string()))],
                }],
                current_module: "test.module",
                is_package: false,
                expected: None,
            }
        },
        module_import = {
            Case {
                name: "zoo",
                imports: vec![Import::Module {
                    module: "zoo".to_string(),
                    alias: None,
                }],
                current_module: "test.module",
                is_package: false,
                expected: Some("zoo"),
            }
        },
        relative_from_module_import = {
            Case {
                name: "Animal",
                imports: vec![Import::RelativeFrom {
                    level: 1,
                    module: Some("base".to_string()),
                    names: vec![("Animal".to_string(), None)],
                }],
                current_module: "mypackage.dog",
                is_package: false,
                expected: Some("mypackage.base.Animal"),
            }
        },
        relative_from_current_package = {
            Case {
                name: "Cat",
                imports: vec![Import::RelativeFrom {
                    level: 1,
                    module: None,
                    names: vec![("Cat".to_string(), None)],
                }],
                current_module: "mypackage.dog",
                is_package: false,
                expected: Some("mypackage.Cat"),
            }
        },
        relative_two_levels_up = {
            Case {
                name: "Helper",
                imports: vec![Import::RelativeFrom {
                    level: 2,
                    module: Some("utils".to_string()),
                    names: vec![("Helper".to_string(), None)],
                }],
                current_module: "pkg.sub.module",
                is_package: false,
                expected: Some("pkg.utils.Helper"),
            }
        },
        relative_three_levels_up = {
            Case {
                name: "Config",
                imports: vec![Import::RelativeFrom {
                    level: 3,
                    module: None,
                    names: vec![("Config".to_string(), None)],
                }],
                current_module: "pkg.sub.module",
                is_package: false,
                expected: Some("Config"),
            }
        },
        relative_from_init = {
            Case {
                name: "Node",
                imports: vec![Import::RelativeFrom {
                    level: 1,
                    module: Some("_core".to_string()),
                    names: vec![("Node".to_string(), None)],
                }],
                current_module: "mypackage",
                is_package: true,
                expected: Some("mypackage._core.Node"),
            }
        },
        relative_from_toplevel = {
            Case {
                name: "Foo",
                imports: vec![Import::RelativeFrom {
                    level: 1,
                    module: None,
                    names: vec![("Foo".to_string(), None)],
                }],
                current_module: "toplevel",
                is_package: false,
                expected: Some("Foo"),
            }
        },
    )]
    fn test_resolve_name(case: Case) {
        assert_eq!(
            resolve_name(
                case.name,
                &case.imports,
                case.current_module,
                case.is_package
            ),
            case.expected.map(|s| s.to_string())
        );
    }
}
