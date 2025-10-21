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

    for stmt in parsed.suite() {
        match stmt {
            Stmt::ClassDef(class_def) => {
                let bases = class_def
                    .bases()
                    .iter()
                    .filter_map(extract_base_class)
                    .collect();

                classes.push(ClassDefinition {
                    name: class_def.name.to_string(),
                    module_path: module_path.to_string(),
                    file_path: file_path.to_path_buf(),
                    bases,
                });
            }
            Stmt::Import(import_stmt) => {
                for alias in &import_stmt.names {
                    imports.push(Import::Module {
                        module: alias.name.to_string(),
                        alias: alias.asname.as_ref().map(|a| a.to_string()),
                    });
                }
            }
            Stmt::ImportFrom(import_from) => {
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
            _ => {}
        }
    }

    Ok(ParsedFile {
        module_path: module_path.to_string(),
        classes,
        imports,
    })
}

/// Extracts a base class reference from an expression.
fn extract_base_class(expr: &Expr) -> Option<BaseClass> {
    match expr {
        // Simple name: class Foo(Bar)
        Expr::Name(name) => Some(BaseClass::Simple(name.id.to_string())),

        // Attribute: class Foo(module.Bar) or class Foo(pkg.mod.Bar)
        Expr::Attribute(_attr) => {
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

/// Resolves an import name to a fully qualified module path.
///
/// Given a name used in the code and the imports in the file, determine
/// what module path it refers to.
///
/// # Arguments
///
/// * `name` - The name to resolve (e.g., "Animal")
/// * `imports` - The imports in the current file
/// * `current_module` - The current module path
///
/// # Returns
///
/// The fully qualified name, or None if it cannot be resolved.
pub fn resolve_import(
    name: &str,
    imports: &[Import],
    current_module: &str,
) -> Option<String> {
    // Check if the name was imported
    for import in imports {
        match import {
            Import::Module { module, alias } => {
                // import foo.bar as baz
                if let Some(a) = alias {
                    if a == name {
                        return Some(module.clone());
                    }
                }
                // import foo.bar (use as foo.bar.Something)
                if module == name {
                    return Some(module.clone());
                }
            }
            Import::From { module, names } => {
                // from foo import Bar
                // from foo import Bar as Baz
                for (n, alias) in names {
                    let imported_name = alias.as_ref().unwrap_or(n);
                    if imported_name == name {
                        return Some(format!("{module}.{n}"));
                    }
                }
            }
            Import::RelativeFrom {
                level,
                module: rel_module,
                names,
            } => {
                // from .relative import Bar
                // from ..parent import Bar
                for (n, alias) in names {
                    let imported_name = alias.as_ref().unwrap_or(n);
                    if imported_name == name {
                        // Resolve relative import
                        // In Python, level=1 always means "current package" (parent of current module)
                        // So we always need to go up one extra level to convert from module to package
                        let effective_level = level + 1;
                        let base = resolve_relative_import(current_module, effective_level)?;
                        return Some(if let Some(m) = rel_module {
                            if base.is_empty() {
                                format!("{m}.{n}")
                            } else {
                                format!("{base}.{m}.{n}")
                            }
                        } else {
                            if base.is_empty() {
                                n.clone()
                            } else {
                                format!("{base}.{n}")
                            }
                        });
                    }
                }
            }
        }
    }

    None
}

/// Resolves a relative import to an absolute module path.
///
/// # Arguments
///
/// * `current_module` - The current module path (e.g., "foo.bar.baz")
/// * `level` - Number of dots in the relative import (from Python AST)
///
/// # Returns
///
/// The base module path. In Python, level=1 means current package, level=2 means parent, etc.
/// So we go up `level - 1` levels.
pub fn resolve_relative_import(current_module: &str, level: usize) -> Option<String> {
    if level == 0 {
        return None; // Not a relative import
    }

    let parts: Vec<&str> = current_module.split('.').collect();
    let levels_to_go_up = level - 1;

    if levels_to_go_up > parts.len() {
        return None; // Invalid relative import
    }

    if levels_to_go_up == parts.len() {
        return Some(String::new()); // Top level
    }

    Some(parts[..parts.len() - levels_to_go_up].join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_relative_import() {
        // level=1 means current package (no going up)
        assert_eq!(
            resolve_relative_import("foo.bar.baz", 1),
            Some("foo.bar.baz".to_string())
        );
        // level=2 means parent package (go up 1)
        assert_eq!(
            resolve_relative_import("foo.bar.baz", 2),
            Some("foo.bar".to_string())
        );
        // level=3 means grandparent package (go up 2)
        assert_eq!(
            resolve_relative_import("foo.bar.baz", 3),
            Some("foo".to_string())
        );
        // level=4 means go up 3 levels (to top)
        assert_eq!(
            resolve_relative_import("foo.bar.baz", 4),
            Some(String::new())
        );
        // level=5 is invalid (can't go above top level)
        assert_eq!(resolve_relative_import("foo.bar.baz", 5), None);
    }

    #[test]
    fn test_resolve_import() {
        let imports = vec![
            Import::From {
                module: "animals".to_string(),
                names: vec![("Dog".to_string(), None)],
            },
            Import::From {
                module: "pets".to_string(),
                names: vec![("Cat".to_string(), Some("Kitty".to_string()))],
            },
            Import::Module {
                module: "zoo".to_string(),
                alias: None,
            },
        ];

        assert_eq!(
            resolve_import("Dog", &imports, "test.module"),
            Some("animals.Dog".to_string())
        );
        assert_eq!(
            resolve_import("Kitty", &imports, "test.module"),
            Some("pets.Cat".to_string())
        );
        assert_eq!(resolve_import("Cat", &imports, "test.module"), None);
        assert_eq!(
            resolve_import("zoo", &imports, "test.module"),
            Some("zoo".to_string())
        );

        // Test relative imports
        let rel_imports = vec![
            Import::RelativeFrom {
                level: 1,
                module: Some("base".to_string()),
                names: vec![("Animal".to_string(), None)],
            },
            Import::RelativeFrom {
                level: 1,
                module: None,
                names: vec![("Cat".to_string(), None)],
            },
        ];

        // from .base import Animal (in mypackage.dog)
        assert_eq!(
            resolve_import("Animal", &rel_imports, "mypackage.dog"),
            Some("mypackage.base.Animal".to_string())
        );

        // from . import Cat (in mypackage.dog - imports from parent package)
        assert_eq!(
            resolve_import("Cat", &rel_imports, "mypackage.dog"),
            Some("mypackage.Cat".to_string())
        );
    }
}
