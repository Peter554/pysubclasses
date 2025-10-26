use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::{
    error::Result,
    parser::{Import, ParsedFile},
};

pub type ModuleName = String;

#[derive(Debug, Clone)]
pub struct ModuleMetadata {
    pub file_path: PathBuf,
    pub is_package: bool,
}

#[derive(Debug, Clone)]
pub struct ClassMetadata {
    // Base classes (unresolved)
    pub bases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassId {
    pub module: ModuleName,
    pub name: String,
}

pub struct Registry {
    pub modules: HashMap<ModuleName, ModuleMetadata>,
    pub classes: HashMap<ClassId, ClassMetadata>,
    pub classes_by_module: HashMap<ModuleName, HashSet<ClassId>>,
    pub imports: HashMap<ModuleName, Vec<Import>>,
}

impl Registry {
    /// Builds a registry from parsed Python files.
    pub fn build(parsed_files: &[ParsedFile]) -> Result<Self> {
        let mut modules = HashMap::new();
        let mut classes = HashMap::new();
        let mut classes_by_module: HashMap<ModuleName, HashSet<ClassId>> = HashMap::new();
        let mut imports = HashMap::new();

        for parsed in parsed_files {
            // Add module metadata
            modules.insert(
                parsed.module_path.clone(),
                ModuleMetadata {
                    file_path: parsed.file_path.clone(),
                    is_package: parsed.is_package,
                },
            );

            // Add classes
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

            // Add imports
            imports.insert(parsed.module_path.clone(), parsed.imports.clone());
        }

        Ok(Self {
            modules,
            classes,
            classes_by_module,
            imports,
        })
    }

    pub fn resolve_class(&self, module: &str, name: &str) -> Option<ClassId> {
        // Resolve a class name in a given module.
        //
        // First, check whether a class with this name exists in the module. If so then we're done!
        let direct_id = ClassId {
            module: module.to_string(),
            name: name.to_string(),
        };
        if self.classes.contains_key(&direct_id) {
            return Some(direct_id);
        }

        // If not, we need to use the imports to resolve the class.
        let imports = self.imports.get(module)?;

        // First, resolve based on the imports in the module.
        // Look for a prefix based on `imported_as` and then substitute the related `imported_item`.
        let mut resolved_name = name.to_string();

        for import in imports {
            // Check if name starts with imported_as
            if name == import.imported_as {
                // Exact match: replace entire name
                resolved_name = import.imported_item.clone();
                break;
            } else if let Some(remainder) = name.strip_prefix(&format!("{}.", import.imported_as)) {
                // Prefix match: substitute prefix
                resolved_name = format!("{}.{}", import.imported_item, remainder);
                break;
            }
        }

        // Now we have to find which module resolved_name refers to.
        // Split the name into parts
        let parts: Vec<&str> = resolved_name.split('.').collect();

        // Try progressively shorter prefixes to find a matching module
        for i in (1..=parts.len()).rev() {
            let module_candidate = parts[..i].join(".");

            // Check if this module exists
            if self.modules.contains_key(&module_candidate) {
                if i == parts.len() {
                    // The entire name is a module - this shouldn't be a class
                    return None;
                }

                // We found the module, the rest is the class name within that module
                let remainder = parts[i..].join(".");
                // Recurse to resolve the class in that module
                return self.resolve_class(&module_candidate, &remainder);
            }
        }

        // Couldn't resolve
        None
    }
}
