//! Python AST parsing module for extracting class definitions and imports.

use rayon::prelude::*;
use ruff_python_ast::{Expr, Stmt};
use ruff_python_parser::parse_module;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Represents a base class reference in a class definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BaseClass {
    /// Simple name reference (e.g., `Animal`)
    Simple(String),
    /// Attribute reference (e.g., `module.Animal` or `package.module.Animal`)
    Attribute(Vec<String>),
}

/// Represents a Python class definition.
#[derive(Debug, Clone)]
pub struct ClassDefinition {
    /// The simple name of the class
    pub name: String,
    /// The module path where the class is defined (e.g., "foo.bar")
    pub module_path: String,
    /// The file path where the class is defined
    pub file_path: PathBuf,
    /// The base classes this class inherits from (unresolved)
    pub bases: Vec<BaseClass>,
}

/// Represents an import statement.
#[derive(Debug, Clone)]
pub enum Import {
    /// `import foo` or `import foo.bar`
    Module {
        module: String,
        alias: Option<String>,
    },
    /// `from foo import bar` or `from foo import bar as baz`
    From {
        module: String,
        names: Vec<(String, Option<String>)>, // (name, alias)
    },
    /// `from .relative import foo` (relative import)
    RelativeFrom {
        level: usize, // Number of dots
        module: Option<String>,
        names: Vec<(String, Option<String>)>,
    },
}

/// The result of parsing a Python file.
#[derive(Debug)]
pub struct ParsedFile {
    /// The path of this file
    pub file_path: PathBuf,
    /// The module path of this file
    pub module_path: String,
    /// Class definitions found in this file
    pub classes: Vec<ClassDefinition>,
    /// Import statements found in this file
    pub imports: Vec<Import>,
    /// Whether this is a package (__init__.py file)
    pub is_package: bool,
}

/// Parses multiple Python files in parallel.
///
/// This function processes a collection of Python files concurrently using Rayon's
/// parallel iterators. Each file's parse result is preserved in the returned vector,
/// allowing the caller to decide how to handle errors.
///
/// # Arguments
///
/// * `root_dir` - The root directory of the Python project, used to compute module paths
/// * `python_files` - Slice of paths to Python files to parse
///
/// # Returns
///
/// A vector containing the parse result for each file. Successful parses return
/// `Ok(ParsedFile)`, while failures return `Err` with details about the error.
///
/// # Errors
///
/// Returns an `Err` only if there's a fundamental issue with the parallel processing
/// infrastructure itself. Individual file parse errors are returned as `Err` variants
/// within the result vector.
pub fn parse_files(root_dir: &Path, python_files: &[PathBuf]) -> Result<Vec<Result<ParsedFile>>> {
    Ok(python_files
        .par_iter()
        .map(|file_path| {
            // Convert file path to module path
            let module_path =
                file_path_to_module_path(file_path, root_dir).ok_or_else(|| Error::ParseError {
                    file: file_path.to_path_buf(),
                    error: "Failed to convert file path to module path".to_string(),
                })?;
            parse_file(file_path, &module_path)
        })
        .collect())
}

/// Parses a Python file and extracts class definitions and imports.
///
/// # Arguments
///
/// * `file_path` - Path to the Python file
/// * `module_path` - The module path for this file (e.g., "foo.bar")
///
/// # Returns
///
/// A `ParsedFile` containing all classes and imports found in the file.
pub fn parse_file(file_path: &Path, module_path: &str) -> Result<ParsedFile> {
    let source = fs::read_to_string(file_path)?;

    let parsed = parse_module(&source).map_err(|e| Error::ParseError {
        file: file_path.to_path_buf(),
        error: format!("{e:?}"),
    })?;

    let mut classes = Vec::new();
    let mut imports = Vec::new();

    extract_from_statements(
        parsed.suite(),
        None,
        module_path,
        file_path,
        &mut classes,
        &mut imports,
    );

    // Check if this is a package (__init__.py file)
    let is_package = file_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name == "__init__.py")
        .unwrap_or(false);

    Ok(ParsedFile {
        file_path: file_path.to_path_buf(),
        module_path: module_path.to_string(),
        classes,
        imports,
        is_package,
    })
}

/// Recursively extracts classes and imports from a list of statements.
///
/// This function handles both top-level and nested class definitions.
///
/// # Arguments
///
/// * `stmts` - The statements to process
/// * `parent_class` - The parent class name for nested classes (e.g., Some("Bar"))
/// * `module_path` - The module path for this file
/// * `file_path` - The file path
/// * `classes` - Mutable vector to accumulate class definitions
/// * `imports` - Mutable vector to accumulate imports (only at top level)
fn extract_from_statements(
    stmts: &[Stmt],
    parent_class: Option<&str>,
    module_path: &str,
    file_path: &Path,
    classes: &mut Vec<ClassDefinition>,
    imports: &mut Vec<Import>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::ClassDef(class_def) => {
                // Build the full class name (with parent prefix if nested)
                let full_name = if let Some(parent) = parent_class {
                    format!("{}.{}", parent, class_def.name)
                } else {
                    class_def.name.to_string()
                };

                let bases = class_def
                    .bases()
                    .iter()
                    .filter_map(extract_base_class)
                    .collect();

                classes.push(ClassDefinition {
                    name: full_name.clone(),
                    module_path: module_path.to_string(),
                    file_path: file_path.to_path_buf(),
                    bases,
                });

                // Recursively extract nested classes
                extract_from_statements(
                    class_def.body.as_slice(),
                    Some(&full_name),
                    module_path,
                    file_path,
                    classes,
                    imports,
                );
            }
            Stmt::Import(import_stmt) => {
                // Only process imports at the top level (not inside classes)
                if parent_class.is_none() {
                    for alias in &import_stmt.names {
                        imports.push(Import::Module {
                            module: alias.name.to_string(),
                            alias: alias.asname.as_ref().map(|a| a.to_string()),
                        });
                    }
                }
            }
            Stmt::ImportFrom(import_from) => {
                // Only process imports at the top level (not inside classes)
                if parent_class.is_none() {
                    let level = import_from.level as usize;
                    let names: Vec<(String, Option<String>)> = import_from
                        .names
                        .iter()
                        .map(|alias| {
                            (
                                alias.name.to_string(),
                                alias.asname.as_ref().map(|a| a.to_string()),
                            )
                        })
                        .collect();

                    if level > 0 {
                        imports.push(Import::RelativeFrom {
                            level,
                            module: import_from.module.as_ref().map(|m| m.to_string()),
                            names,
                        });
                    } else {
                        imports.push(Import::From {
                            module: import_from
                                .module
                                .as_ref()
                                .map(|m| m.to_string())
                                .unwrap_or_default(),
                            names,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

/// Extracts a base class reference from an expression.
fn extract_base_class(expr: &Expr) -> Option<BaseClass> {
    match expr {
        // Simple name: class Foo(Bar)
        Expr::Name(name) => Some(BaseClass::Simple(name.id.to_string())),

        // Attribute: class Foo(module.Bar) or class Foo(pkg.mod.Bar)
        Expr::Attribute(_) => {
            let mut parts = Vec::new();
            let mut current = expr;

            // Walk the attribute chain from right to left
            loop {
                match current {
                    Expr::Attribute(attr) => {
                        parts.push(attr.attr.to_string());
                        current = &attr.value;
                    }
                    Expr::Name(name) => {
                        parts.push(name.id.to_string());
                        break;
                    }
                    _ => return None,
                }
            }

            parts.reverse();
            Some(BaseClass::Attribute(parts))
        }

        // Subscript: class Foo(Generic[T]) - extract the base without the subscript
        Expr::Subscript(subscript) => extract_base_class(&subscript.value),

        // Ignore other forms (call expressions, etc.)
        _ => None,
    }
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
fn file_path_to_module_path(file_path: &Path, root_dir: &Path) -> Option<String> {
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
    fn test_nested_class_extraction() {
        // Create a temporary Python file with nested classes
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_nested_classes.py");

        std::fs::write(
            &temp_file,
            r#"
class Foo:
    pass

class Bar:
    class NestedInBar(Foo):
        pass

    class AnotherNested:
        class DoublyNested(Foo):
            pass

class TopLevel(Foo):
    pass
"#,
        )
        .unwrap();

        let parsed = parse_file(&temp_file, "test_module").unwrap();

        // Clean up
        let _ = std::fs::remove_file(&temp_file);

        // Check that we found all classes with correct names
        let class_names: Vec<&str> = parsed.classes.iter().map(|c| c.name.as_str()).collect();

        assert_eq!(class_names.len(), 6);
        assert!(class_names.contains(&"Foo"));
        assert!(class_names.contains(&"Bar"));
        assert!(class_names.contains(&"Bar.NestedInBar"));
        assert!(class_names.contains(&"Bar.AnotherNested"));
        assert!(class_names.contains(&"Bar.AnotherNested.DoublyNested"));
        assert!(class_names.contains(&"TopLevel"));

        // Verify that NestedInBar has Foo as a base
        let nested_in_bar = parsed
            .classes
            .iter()
            .find(|c| c.name == "Bar.NestedInBar")
            .unwrap();
        assert_eq!(nested_in_bar.bases.len(), 1);
        assert_eq!(nested_in_bar.bases[0], BaseClass::Simple("Foo".to_string()));

        // Verify that DoublyNested has Foo as a base
        let doubly_nested = parsed
            .classes
            .iter()
            .find(|c| c.name == "Bar.AnotherNested.DoublyNested")
            .unwrap();
        assert_eq!(doubly_nested.bases.len(), 1);
        assert_eq!(doubly_nested.bases[0], BaseClass::Simple("Foo".to_string()));
    }
}
