//! Python AST parsing module for extracting class definitions and imports.

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

impl BaseClass {
    /// Get the simple name of the base class (rightmost component).
    pub fn name(&self) -> &str {
        match self {
            BaseClass::Simple(name) => name,
            BaseClass::Attribute(parts) => parts.last().unwrap(),
        }
    }
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
    /// The module path of this file
    pub module_path: String,
    /// Class definitions found in this file
    pub classes: Vec<ClassDefinition>,
    /// Import statements found in this file
    pub imports: Vec<Import>,
    /// Whether this is a package (__init__.py file)
    pub is_package: bool,
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
