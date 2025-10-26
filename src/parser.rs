//! Python AST parsing module for extracting class definitions and imports.

use rayon::prelude::*;
use ruff_python_ast::{Expr, Stmt};
use ruff_python_parser::parse_module;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Represents a Python class definition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClassDefinition {
    /// The simple name of the class
    pub name: String,
    /// The module path where the class is defined (e.g., "foo.bar")
    pub module_path: String,
    /// The file path where the class is defined
    pub file_path: PathBuf,
    /// The base classes this class inherits from e.g. "Foo" or "foo.Foo"
    pub bases: Vec<String>,
}

/// An import.
///
/// E.g.
/// `import a` => { imported_item=a, imported_as=a }
/// `import a.b` => { imported_item=a.b, imported_as=a.b }
/// `import a.b as c` => { imported_item=a.b, imported_as=c }
/// `from a import b` => { imported_item=a.b, imported_as=b }
/// `from a import b as c` => { imported_item=a.b, imported_as=c }
/// `from a.b import c` => { imported_item=a.b.c, imported_as=c }
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Import {
    pub imported_item: String,
    pub imported_as: String,
}

/// The result of parsing a Python file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParsedFile {
    /// The path of this file
    pub file_path: PathBuf,
    /// The module path of this file
    pub module_path: String,
    /// Class definitions found in this file
    pub classes: Vec<ClassDefinition>,
    /// Import statements found in this file (relative imports already resolved)
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
/// This function walks the AST and extracts:
/// - Class definitions (including nested classes)
/// - Import statements (both `import` and `from...import` forms)
///
/// Nested classes are represented with dot notation (e.g., "Outer.Inner").
///
/// # Arguments
///
/// * `stmts` - The AST statements to process
/// * `parent_class` - The parent class name if processing nested classes (e.g., `Some("Outer")`)
/// * `module_path` - The module path for this file (e.g., "foo.bar")
/// * `file_path` - The file path (used for resolving relative imports)
/// * `classes` - Mutable vector to accumulate discovered class definitions
/// * `imports` - Mutable vector to accumulate discovered imports (only at top level)
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
                // Build fully qualified class name (e.g., "Outer.Inner" for nested classes)
                let full_name = if let Some(parent) = parent_class {
                    format!("{}.{}", parent, class_def.name)
                } else {
                    class_def.name.to_string()
                };

                // Extract base classes, filtering out unresolvable references
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

                // Recursively process nested classes
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
                // Process `import foo` or `import foo as bar` statements
                // Format: { imported_item: "foo", imported_as: "bar" }
                for alias in &import_stmt.names {
                    let imported_item = alias.name.to_string();
                    let imported_as = alias
                        .asname
                        .as_ref()
                        .map(|a| a.to_string())
                        .unwrap_or_else(|| imported_item.clone());
                    imports.push(Import {
                        imported_item,
                        imported_as,
                    });
                }
            }
            Stmt::ImportFrom(import_from) => {
                // Process `from foo import bar` or `from .foo import bar` statements
                let level = import_from.level as usize;

                for alias in &import_from.names {
                    let name = alias.name.to_string();
                    let imported_as = alias
                        .asname
                        .as_ref()
                        .map(|a| a.to_string())
                        .unwrap_or_else(|| name.clone());

                    let imported_item = if level > 0 {
                        // Relative import: resolve dots to absolute module path
                        // e.g., `from ..pkg import Foo` → "parent.pkg.Foo"
                        let base_module = resolve_relative_module(module_path, level, file_path);
                        if let Some(from_module) = import_from.module.as_ref() {
                            format!("{base_module}.{from_module}.{name}")
                        } else {
                            format!("{base_module}.{name}")
                        }
                    } else {
                        // Absolute import: combine module and name
                        // e.g., `from foo import Bar` → "foo.Bar"
                        let from_module = import_from
                            .module
                            .as_ref()
                            .map(|m| m.to_string())
                            .unwrap_or_default();
                        if from_module.is_empty() {
                            name.clone()
                        } else {
                            format!("{from_module}.{name}")
                        }
                    };

                    imports.push(Import {
                        imported_item,
                        imported_as,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Resolves a relative import to an absolute module path.
///
/// Relative imports in Python use dots to indicate the starting point:
/// - `.foo` means "foo in the current package"
/// - `..foo` means "foo in the parent package"
/// - `...foo` means "foo in the grandparent package"
///
/// This function converts the relative path to an absolute module path based on
/// the current module's location.
///
/// # Arguments
///
/// * `current_module` - The module path of the file containing the import (e.g., "pkg.sub.module")
/// * `level` - The number of leading dots in the relative import (e.g., 2 for `..foo`)
/// * `file_path` - The file path (used to check if this is a `__init__.py` package file)
///
/// # Returns
///
/// The absolute module path that the relative import refers to.
///
/// # Examples
///
/// ```text
/// # In pkg/sub/module.py (module path "pkg.sub.module"):
/// from ..other import Foo  # level=2 → resolves to "pkg.other"
///
/// # In pkg/sub/__init__.py (module path "pkg.sub"):
/// from ..other import Foo  # level=2 → resolves to "pkg.other"
/// from .local import Bar   # level=1 → resolves to "pkg.sub.local"
/// ```
fn resolve_relative_module(current_module: &str, level: usize, file_path: &Path) -> String {
    let is_package = file_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name == "__init__.py")
        .unwrap_or(false);

    let parts: Vec<&str> = current_module.split('.').collect();

    // Packages start at themselves, modules start at their parent
    // e.g., In "pkg.sub" package: level 1 = "pkg.sub", level 2 = "pkg"
    //       In "pkg.sub.mod" module: level 1 = "pkg.sub", level 2 = "pkg"
    let base_level = if is_package { level - 1 } else { level };

    // Navigate up the package hierarchy
    if base_level >= parts.len() {
        // Trying to go above the root - return empty (error case)
        String::new()
    } else {
        parts[..parts.len() - base_level].join(".")
    }
}

/// Extracts a base class reference from an expression as a string.
/// Returns strings like "Foo" or "module.Foo" or "pkg.mod.Foo"
fn extract_base_class(expr: &Expr) -> Option<String> {
    match expr {
        // Simple name: class Foo(Bar)
        Expr::Name(name) => Some(name.id.to_string()),

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
            Some(parts.join("."))
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
        assert_eq!(nested_in_bar.bases[0], "Foo");

        // Verify that DoublyNested has Foo as a base
        let doubly_nested = parsed
            .classes
            .iter()
            .find(|c| c.name == "Bar.AnotherNested.DoublyNested")
            .unwrap();
        assert_eq!(doubly_nested.bases.len(), 1);
        assert_eq!(doubly_nested.bases[0], "Foo");
    }

    // Parametric tests for import parsing
    #[derive(Debug)]
    struct ImportCase {
        name: &'static str,
        python_code: &'static str,
        module_path: &'static str,
        is_package: bool,
        expected_imports: Vec<(&'static str, &'static str)>, // (imported_item, imported_as)
    }

    #[yare::parameterized(
        absolute_import_simple = { ImportCase {
            name: "absolute import simple",
            python_code: "import foo",
            module_path: "mymodule",
            is_package: false,
            expected_imports: vec![("foo", "foo")],
        } },
        absolute_import_dotted = { ImportCase {
            name: "absolute import dotted",
            python_code: "import foo.bar.baz",
            module_path: "mymodule",
            is_package: false,
            expected_imports: vec![("foo.bar.baz", "foo.bar.baz")],
        } },
        absolute_import_alias = { ImportCase {
            name: "absolute import with alias",
            python_code: "import foo as f",
            module_path: "mymodule",
            is_package: false,
            expected_imports: vec![("foo", "f")],
        } },
        absolute_import_dotted_alias = { ImportCase {
            name: "absolute import dotted with alias",
            python_code: "import foo.bar as fb",
            module_path: "mymodule",
            is_package: false,
            expected_imports: vec![("foo.bar", "fb")],
        } },
        from_import_simple = { ImportCase {
            name: "from import simple",
            python_code: "from foo import Bar",
            module_path: "mymodule",
            is_package: false,
            expected_imports: vec![("foo.Bar", "Bar")],
        } },
        from_import_dotted = { ImportCase {
            name: "from import dotted",
            python_code: "from foo.bar import Baz",
            module_path: "mymodule",
            is_package: false,
            expected_imports: vec![("foo.bar.Baz", "Baz")],
        } },
        from_import_alias = { ImportCase {
            name: "from import with alias",
            python_code: "from foo import Bar as B",
            module_path: "mymodule",
            is_package: false,
            expected_imports: vec![("foo.Bar", "B")],
        } },
        from_import_multiple = { ImportCase {
            name: "from import multiple",
            python_code: "from foo import Bar, Baz",
            module_path: "mymodule",
            is_package: false,
            expected_imports: vec![("foo.Bar", "Bar"), ("foo.Baz", "Baz")],
        } },
        from_import_multiple_alias = { ImportCase {
            name: "from import multiple with alias",
            python_code: "from foo import Bar as B, Baz as Z",
            module_path: "mymodule",
            is_package: false,
            expected_imports: vec![("foo.Bar", "B"), ("foo.Baz", "Z")],
        } },
        relative_import_one_level_module = { ImportCase {
            name: "relative import one level from module",
            python_code: "from .sibling import Foo",
            module_path: "pkg.mymodule",
            is_package: false,
            expected_imports: vec![("pkg.sibling.Foo", "Foo")],
        } },
        relative_import_one_level_package = { ImportCase {
            name: "relative import one level from package",
            python_code: "from .sibling import Foo",
            module_path: "pkg",
            is_package: true,
            expected_imports: vec![("pkg.sibling.Foo", "Foo")],
        } },
        relative_import_two_levels_module = { ImportCase {
            name: "relative import two levels from module",
            python_code: "from ..other import Foo",
            module_path: "pkg.sub.mymodule",
            is_package: false,
            expected_imports: vec![("pkg.other.Foo", "Foo")],
        } },
        relative_import_two_levels_package = { ImportCase {
            name: "relative import two levels from package",
            python_code: "from ..other import Foo",
            module_path: "pkg.sub",
            is_package: true,
            expected_imports: vec![("pkg.other.Foo", "Foo")],
        } },
        relative_import_no_module = { ImportCase {
            name: "relative import without from module",
            python_code: "from . import Foo",
            module_path: "pkg.mymodule",
            is_package: false,
            expected_imports: vec![("pkg.Foo", "Foo")],
        } },
        relative_import_alias = { ImportCase {
            name: "relative import with alias",
            python_code: "from .sibling import Foo as F",
            module_path: "pkg.mymodule",
            is_package: false,
            expected_imports: vec![("pkg.sibling.Foo", "F")],
        } },
        multiple_imports = { ImportCase {
            name: "multiple import statements",
            python_code: "import foo\nfrom bar import Baz",
            module_path: "mymodule",
            is_package: false,
            expected_imports: vec![("foo", "foo"), ("bar.Baz", "Baz")],
        } },
    )]
    fn test_import_parsing(case: ImportCase) {
        let temp_dir = std::env::temp_dir();
        // Create a proper directory structure for package tests
        let test_id = case.name.replace(' ', "_");
        let test_root = temp_dir.join(format!("test_imports_{test_id}"));
        std::fs::create_dir_all(&test_root).unwrap();

        let temp_file = if case.is_package {
            test_root.join("__init__.py")
        } else {
            test_root.join("test.py")
        };

        std::fs::write(&temp_file, case.python_code).unwrap();

        let parsed = parse_file(&temp_file, case.module_path).unwrap();

        // Clean up
        let _ = std::fs::remove_dir_all(&test_root);

        // Verify imports
        assert_eq!(
            parsed.imports.len(),
            case.expected_imports.len(),
            "Case '{}': expected {} imports, got {}",
            case.name,
            case.expected_imports.len(),
            parsed.imports.len()
        );

        for (i, (expected_item, expected_as)) in case.expected_imports.iter().enumerate() {
            assert_eq!(
                parsed.imports[i].imported_item, *expected_item,
                "Case '{}': import {} - expected imported_item '{}', got '{}'",
                case.name, i, expected_item, parsed.imports[i].imported_item
            );
            assert_eq!(
                parsed.imports[i].imported_as, *expected_as,
                "Case '{}': import {} - expected imported_as '{}', got '{}'",
                case.name, i, expected_as, parsed.imports[i].imported_as
            );
        }
    }

    // Parametric tests for base class extraction
    #[derive(Debug)]
    struct BaseClassCase {
        name: &'static str,
        python_code: &'static str,
        class_name: &'static str,
        expected_bases: Vec<&'static str>,
    }

    #[yare::parameterized(
        simple_base = { BaseClassCase {
            name: "simple base class",
            python_code: "class Foo(Bar): pass",
            class_name: "Foo",
            expected_bases: vec!["Bar"],
        } },
        attribute_base = { BaseClassCase {
            name: "attribute base class",
            python_code: "class Foo(module.Bar): pass",
            class_name: "Foo",
            expected_bases: vec!["module.Bar"],
        } },
        nested_attribute_base = { BaseClassCase {
            name: "nested attribute base class",
            python_code: "class Foo(pkg.module.Bar): pass",
            class_name: "Foo",
            expected_bases: vec!["pkg.module.Bar"],
        } },
        multiple_bases_simple = { BaseClassCase {
            name: "multiple simple bases",
            python_code: "class Foo(Bar, Baz): pass",
            class_name: "Foo",
            expected_bases: vec!["Bar", "Baz"],
        } },
        multiple_bases_mixed = { BaseClassCase {
            name: "multiple mixed bases",
            python_code: "class Foo(Bar, pkg.Baz): pass",
            class_name: "Foo",
            expected_bases: vec!["Bar", "pkg.Baz"],
        } },
        generic_base = { BaseClassCase {
            name: "generic base class",
            python_code: "class Foo(Generic[T]): pass",
            class_name: "Foo",
            expected_bases: vec!["Generic"],
        } },
        generic_with_multiple = { BaseClassCase {
            name: "generic with multiple type params",
            python_code: "class Foo(Dict[str, int]): pass",
            class_name: "Foo",
            expected_bases: vec!["Dict"],
        } },
        mixed_generic_and_simple = { BaseClassCase {
            name: "mixed generic and simple",
            python_code: "class Foo(Bar, Generic[T]): pass",
            class_name: "Foo",
            expected_bases: vec!["Bar", "Generic"],
        } },
        no_bases = { BaseClassCase {
            name: "no base classes",
            python_code: "class Foo: pass",
            class_name: "Foo",
            expected_bases: vec![],
        } },
        attribute_generic = { BaseClassCase {
            name: "attribute with generic",
            python_code: "class Foo(typing.Generic[T]): pass",
            class_name: "Foo",
            expected_bases: vec!["typing.Generic"],
        } },
    )]
    fn test_base_class_extraction(case: BaseClassCase) {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("test_bases_{}.py", case.name));

        std::fs::write(&temp_file, case.python_code).unwrap();

        let parsed = parse_file(&temp_file, "test_module").unwrap();

        // Clean up
        let _ = std::fs::remove_file(&temp_file);

        // Find the class
        let class = parsed
            .classes
            .iter()
            .find(|c| c.name == case.class_name)
            .unwrap_or_else(|| {
                panic!(
                    "Case '{}': class '{}' not found",
                    case.name, case.class_name
                )
            });

        // Verify bases
        assert_eq!(
            class.bases.len(),
            case.expected_bases.len(),
            "Case '{}': expected {} bases, got {}",
            case.name,
            case.expected_bases.len(),
            class.bases.len()
        );

        for (i, expected_base) in case.expected_bases.iter().enumerate() {
            assert_eq!(
                class.bases[i], *expected_base,
                "Case '{}': base {} - expected '{}', got '{}'",
                case.name, i, expected_base, class.bases[i]
            );
        }
    }
}
