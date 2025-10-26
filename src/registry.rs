//! Class registry for tracking class definitions and resolving references.
//!
//! The registry maintains a complete index of all Python modules, classes, and imports
//! found in a codebase. It provides functionality to resolve class references across
//! modules, handling imports and re-exports correctly.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::{
    error::Result,
    parser::{Import, ParsedFile},
};

/// Type alias for Python module names (e.g., "foo.bar.baz").
pub type ModuleName = String;

/// Metadata about a Python module.
#[derive(Debug, Clone)]
pub struct ModuleMetadata {
    /// The file system path to this module.
    pub file_path: PathBuf,
    /// Whether this is a package (`__init__.py` file).
    pub is_package: bool,
}

/// Metadata about a Python class definition.
#[derive(Debug, Clone)]
pub struct ClassMetadata {
    /// The base classes this class inherits from.
    ///
    /// These are stored as unresolved strings (e.g., "Foo" or "module.Foo")
    /// and must be resolved using the registry's import information.
    pub bases: Vec<String>,
}

/// A unique identifier for a class within the codebase.
///
/// Consists of the module path and class name. Note that nested classes
/// are represented with dot notation (e.g., "OuterClass.InnerClass").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassId {
    /// The module path (e.g., "foo.bar").
    pub module: ModuleName,
    /// The class name, potentially including nesting (e.g., "Outer.Inner").
    pub name: String,
}

/// A registry of all modules, classes, and imports in a Python codebase.
///
/// The registry is built from parsed files and provides methods to:
/// - Look up class definitions by name or module
/// - Resolve class references through imports and re-exports
/// - Track the inheritance relationships between classes
pub struct Registry {
    /// Metadata for each module in the codebase.
    pub modules: HashMap<ModuleName, ModuleMetadata>,
    /// Metadata for each class, indexed by ClassId.
    pub classes: HashMap<ClassId, ClassMetadata>,
    /// Index of classes organized by module for efficient lookup.
    pub classes_by_module: HashMap<ModuleName, HashSet<ClassId>>,
    /// Import statements for each module.
    pub imports: HashMap<ModuleName, Vec<Import>>,
}

impl Registry {
    /// Builds a registry from parsed Python files.
    ///
    /// This processes all parsed files to create indexes of modules, classes, and imports.
    ///
    /// # Arguments
    ///
    /// * `parsed_files` - The collection of parsed Python files
    ///
    /// # Returns
    ///
    /// A fully constructed registry ready for class resolution.
    pub fn build(parsed_files: &[ParsedFile]) -> Result<Self> {
        let mut modules = HashMap::new();
        let mut classes = HashMap::new();
        let mut classes_by_module: HashMap<ModuleName, HashSet<ClassId>> = HashMap::new();
        let mut imports = HashMap::new();

        for parsed in parsed_files {
            // Record module metadata
            modules.insert(
                parsed.module_path.clone(),
                ModuleMetadata {
                    file_path: parsed.file_path.clone(),
                    is_package: parsed.is_package,
                },
            );

            // Index all class definitions from this module
            for class in &parsed.classes {
                let class_id = ClassId {
                    module: parsed.module_path.clone(),
                    name: class.name.clone(),
                };

                classes.insert(
                    class_id.clone(),
                    ClassMetadata {
                        bases: class.bases.clone(),
                    },
                );

                classes_by_module
                    .entry(parsed.module_path.clone())
                    .or_default()
                    .insert(class_id);
            }

            // Store import statements for later resolution
            imports.insert(parsed.module_path.clone(), parsed.imports.clone());
        }

        Ok(Self {
            modules,
            classes,
            classes_by_module,
            imports,
        })
    }

    /// Resolves a class name within a given module's context.
    ///
    /// This method handles the complexity of Python's import system, including:
    /// - Direct class references within the same module
    /// - Classes imported from other modules
    /// - Classes re-exported through `__init__.py` files
    /// - Attribute-style class references (e.g., "module.Class")
    ///
    /// # Algorithm
    ///
    /// 1. Check if the class exists directly in the specified module
    /// 2. If not, consult the module's imports to resolve the name:
    ///    - Match against `imported_as` names from import statements
    ///    - Substitute with the actual `imported_item` path
    /// 3. Parse the resolved name to find the defining module:
    ///    - Try progressively shorter prefixes (e.g., "a.b.c" → "a.b" → "a")
    ///    - Stop when we find a module that exists
    /// 4. Recursively resolve the remaining name within that module
    ///
    /// # Arguments
    ///
    /// * `module` - The module path providing the context for resolution
    /// * `name` - The class name to resolve (may include dots for attribute access)
    ///
    /// # Returns
    ///
    /// The resolved `ClassId` if the class can be found, or `None` if resolution fails.
    ///
    /// # Examples
    ///
    /// ```text
    /// # In module "zoo":
    /// from animals import Dog
    /// class Puppy(Dog):  # Resolves "Dog" to ClassId { module: "animals", name: "Dog" }
    ///     pass
    /// ```
    pub fn resolve_class(&self, module: &str, name: &str) -> Option<ClassId> {
        // First, check for a direct class definition in this module
        let direct_id = ClassId {
            module: module.to_string(),
            name: name.to_string(),
        };
        if self.classes.contains_key(&direct_id) {
            return Some(direct_id);
        }

        // Not found directly - use imports to resolve the reference
        let imports = self.imports.get(module)?;

        // Substitute imported names with their actual module paths
        // Example: If "Dog" is imported as "from animals import Dog",
        // then "Dog" becomes "animals.Dog"
        let mut resolved_name = name.to_string();

        for import in imports {
            if name == import.imported_as {
                // Exact match: "Dog" → "animals.Dog"
                resolved_name = import.imported_item.clone();
                break;
            } else if let Some(remainder) = name.strip_prefix(&format!("{}.", import.imported_as)) {
                // Prefix match: "Dog.Puppy" → "animals.Dog.Puppy"
                resolved_name = format!("{}.{}", import.imported_item, remainder);
                break;
            }
        }

        // Parse the resolved name to find the defining module and class
        // Example: "animals.Dog" needs to be split into module "animals" and class "Dog"
        let parts: Vec<&str> = resolved_name.split('.').collect();

        // Try each possible split point from longest to shortest prefix
        // This handles cases like "a.b.c.Class" where "a.b.c" might be the module
        for i in (1..=parts.len()).rev() {
            let module_candidate = parts[..i].join(".");

            if self.modules.contains_key(&module_candidate) {
                if i == parts.len() {
                    // The entire resolved name is just a module, not a class
                    return None;
                }

                // Found the module! The remainder is the class name within it
                let remainder = parts[i..].join(".");

                // Recursively resolve in case the class itself is re-exported
                return self.resolve_class(&module_candidate, &remainder);
            }
        }

        // Unable to resolve this class reference
        None
    }
}
