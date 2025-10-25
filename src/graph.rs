//! Inheritance graph construction and traversal.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::parser::ParsedFile;

pub type ModuleName = String;

#[derive(Debug)]
pub struct ModuleMetadata {
    pub file_path: PathBuf,
    pub is_package: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassId {
    pub module: ModuleName,
    pub name: String,
}

/// An inheritance graph mapping classes to their children.
pub struct InheritanceGraph {
    pub modules: HashMap<ModuleName, ModuleMetadata>,
    pub classes: HashSet<ClassId>,
    pub imports: HashMap<ModuleName, Vec<ResolvedImport>>,
    pub class_children: HashMap<ClassId, Vec<ClassId>>,
}

/// An enum representing a resolved import.
///
/// `import X` is always an imported module.
///
/// `from X import Y` can be either a module import, or a module member import.
/// This can be determined by first seeing if the module X.Y exists. If so then this is a module import of module X.Y.
/// If not we check if the module X exists. If so then this is an import of the member Y from the module X.
pub enum ResolvedImport {
    Module {
        module: ModuleName,
        imported_as: String,
    },
    ModuleMember {
        module: ModuleName,
        member: String,
        imported_as: String,
    },
}

impl InheritanceGraph {
    /// Builds an inheritance graph from parsed files.
    ///
    /// # Arguments
    ///
    /// * `parsed_files` - The parsed python files.
    ///
    /// # Returns
    ///
    /// An inheritance graph with parent-child relationships.
    pub fn build(parsed_files: &[ParsedFile]) -> Self {
        let mut modules = HashMap::new();
        let mut classes: HashSet<ClassId> = HashSet::new();
        let mut imports: HashMap<ModuleName, Vec<ResolvedImport>> = HashMap::new();
        let mut class_children: HashMap<ClassId, Vec<ClassId>> = HashMap::new();

        // First pass: collect all modules and classes
        for file in parsed_files {
            // Register module metadata
            modules.insert(
                file.module_path.clone(),
                ModuleMetadata {
                    file_path: file.file_path.clone(),
                    is_package: file.is_package,
                },
            );

            // Register classes in this module
            for class in &file.classes {
                classes.insert(ClassId {
                    module: file.module_path.clone(),
                    name: class.name.clone(),
                });
            }
        }

        // Second pass: resolve imports
        for file in parsed_files {
            let mut resolved_imports = Vec::new();

            for import in &file.imports {
                match import {
                    crate::parser::Import::Module { module, alias } => {
                        let imported_as = if let Some(alias) = alias {
                            alias.clone()
                        } else {
                            module.clone()
                        };
                        resolved_imports.push(ResolvedImport::Module {
                            module: module.clone(),
                            imported_as,
                        });
                    }
                    crate::parser::Import::From { module, names } => {
                        for (name, alias) in names {
                            // Check if this is a module import or member import
                            let full_module = format!("{module}.{name}");
                            if modules.contains_key(&full_module) {
                                // This is a module import
                                resolved_imports.push(ResolvedImport::Module {
                                    module: full_module,
                                    imported_as: alias.clone().unwrap_or_else(|| name.clone()),
                                });
                            } else {
                                // This is a member import
                                resolved_imports.push(ResolvedImport::ModuleMember {
                                    module: module.clone(),
                                    member: name.clone(),
                                    imported_as: alias.clone().unwrap_or_else(|| name.clone()),
                                });
                            }
                        }
                    }
                    crate::parser::Import::RelativeFrom {
                        level,
                        module,
                        names,
                    } => {
                        // Resolve relative import to absolute module path
                        if let Some(abs_module) = resolve_relative_import(
                            &file.module_path,
                            *level,
                            module.as_deref(),
                            file.is_package,
                        ) {
                            for (name, alias) in names {
                                // Check if this is a module import or member import
                                let full_module = format!("{abs_module}.{name}");
                                if modules.contains_key(&full_module) {
                                    // This is a module import
                                    resolved_imports.push(ResolvedImport::Module {
                                        module: full_module,
                                        imported_as: alias.clone().unwrap_or_else(|| name.clone()),
                                    });
                                } else {
                                    // This is a member import
                                    resolved_imports.push(ResolvedImport::ModuleMember {
                                        module: abs_module.clone(),
                                        member: name.clone(),
                                        imported_as: alias.clone().unwrap_or_else(|| name.clone()),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            imports.insert(file.module_path.clone(), resolved_imports);
        }

        // Third pass: build parent-child relationships
        for file in parsed_files {
            for class in &file.classes {
                for base in &class.bases {
                    // Resolve the base class to a ClassId
                    // TODO
                }
            }
        }

        Self {
            modules,
            classes,
            imports,
            class_children,
        }
    }

    /// Finds all transitive subclasses of a given class.
    ///
    /// Uses BFS to traverse the inheritance graph and collect all descendants.
    ///
    /// # Arguments
    ///
    /// * `root` - The root class to find subclasses of
    ///
    /// # Returns
    ///
    /// A vector of all transitive subclasses (not including the root class itself).
    pub fn find_all_subclasses(&self, root: &ClassId) -> Vec<ClassId> {
        use std::collections::VecDeque;

        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Start BFS from the root
        queue.push_back(root.clone());
        visited.insert((root.module.clone(), root.name.clone()));

        while let Some(current) = queue.pop_front() {
            // Find all direct children
            if let Some(children) = self.class_children.get(&current) {
                for child in children {
                    let key = (child.module.clone(), child.name.clone());
                    if !visited.contains(&key) {
                        visited.insert(key);
                        result.push(child.clone());
                        queue.push_back(child.clone());
                    }
                }
            }
        }

        result
    }
}

/// Resolves a relative import to an absolute module path.
///
/// # Arguments
///
/// * `current_module` - The module path where the import occurs
/// * `level` - Number of dots in the relative import
/// * `relative_module` - Optional module name after the dots
/// * `is_package` - Whether the current module is a package (__init__.py)
///
/// # Returns
///
/// The absolute module path, or None if the import cannot be resolved
fn resolve_relative_import(
    current_module: &str,
    level: usize,
    relative_module: Option<&str>,
    is_package: bool,
) -> Option<String> {
    let parts: Vec<&str> = current_module.split('.').collect();

    // level determines how many parent levels to go up
    // level=1: from . import x (current package)
    // level=2: from .. import x (parent package)
    //
    // Key insight: "current package" means different things:
    // - For pkg/__init__.py (is_package=true), current package is "pkg"
    // - For pkg/module.py (is_package=false), current package is also "pkg"
    //
    // So for a package, level=1 should not remove anything
    // For a module, level=1 should remove the last component
    if level == 0 || level > parts.len() {
        return None;
    }

    // Calculate how many components to remove
    // For packages: level=1 removes 0, level=2 removes 1, etc.
    // For modules: level=1 removes 1, level=2 removes 2, etc.
    let levels_to_remove = if is_package {
        level.saturating_sub(1)
    } else {
        level
    };

    if levels_to_remove > parts.len() {
        return None;
    }

    let base_parts = &parts[..parts.len() - levels_to_remove];

    let base = if base_parts.is_empty() {
        None
    } else {
        Some(base_parts.join("."))
    };

    match (base, relative_module) {
        (Some(base_str), Some(module)) => Some(format!("{base_str}.{module}")),
        (Some(base_str), None) => Some(base_str),
        (None, Some(module)) => Some(module.to_string()),
        (None, None) => None,
    }
}
