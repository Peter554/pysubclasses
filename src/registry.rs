//! Class registry for tracking class definitions and resolving references.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::parser::{BaseClass, ClassDefinition, Import, ParsedFile};

/// A unique identifier for a class.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassId {
    pub module_path: String,
    pub class_name: String,
}

impl ClassId {
    pub fn new(module_path: String, class_name: String) -> Self {
        Self {
            module_path,
            class_name,
        }
    }

    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.module_path, self.class_name)
    }
}

/// Information about where a class is defined.
#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub id: ClassId,
    pub file_path: PathBuf,
    pub bases: Vec<BaseClass>,
}

/// A registry of all classes found in a codebase.
pub struct ClassRegistry {
    /// Map from ClassId to ClassInfo
    classes: HashMap<ClassId, ClassInfo>,

    /// Map from simple class name to all ClassIds with that name
    /// (for ambiguity detection)
    name_index: HashMap<String, Vec<ClassId>>,

    /// Map from module path to imports in that module
    imports: HashMap<String, Vec<Import>>,
}

impl ClassRegistry {
    /// Creates a new empty class registry.
    pub fn new() -> Self {
        Self {
            classes: HashMap::new(),
            name_index: HashMap::new(),
            imports: HashMap::new(),
        }
    }

    /// Adds a parsed file to the registry.
    pub fn add_file(&mut self, parsed: ParsedFile) {
        // Store imports for this module
        self.imports
            .insert(parsed.module_path.clone(), parsed.imports);

        // Add all classes
        for class in parsed.classes {
            self.add_class(class);
        }
    }

    /// Adds a class definition to the registry.
    fn add_class(&mut self, class: ClassDefinition) {
        let id = ClassId::new(class.module_path.clone(), class.name.clone());

        let info = ClassInfo {
            id: id.clone(),
            file_path: class.file_path,
            bases: class.bases,
        };

        // Add to main registry
        self.classes.insert(id.clone(), info);

        // Add to name index
        self.name_index
            .entry(class.name)
            .or_default()
            .push(id);
    }

    /// Finds all classes with a given name.
    pub fn find_by_name(&self, name: &str) -> Option<&Vec<ClassId>> {
        self.name_index.get(name)
    }

    /// Finds a specific class by name and optional module path.
    pub fn find_class(&self, name: &str, module_path: Option<&str>) -> Option<&ClassInfo> {
        if let Some(module) = module_path {
            let id = ClassId::new(module.to_string(), name.to_string());
            self.classes.get(&id)
        } else {
            // No module specified, find by name alone
            let matches = self.find_by_name(name)?;
            if matches.len() == 1 {
                self.classes.get(&matches[0])
            } else {
                // Ambiguous
                None
            }
        }
    }

    /// Gets class info by ClassId.
    pub fn get(&self, id: &ClassId) -> Option<&ClassInfo> {
        self.classes.get(id)
    }

    /// Returns all class IDs in the registry.
    pub fn all_class_ids(&self) -> Vec<ClassId> {
        self.classes.keys().cloned().collect()
    }

    /// Resolves a base class reference to a ClassId.
    ///
    /// Given a base class reference in a class definition, determine which
    /// actual class it refers to based on imports and available classes.
    pub fn resolve_base(&self, base: &BaseClass, context_module: &str) -> Option<ClassId> {
        match base {
            BaseClass::Simple(name) => {
                // Look up the name in imports for this module
                if let Some(imports) = self.imports.get(context_module) {
                    if let Some(qualified) =
                        crate::parser::resolve_import(name, imports, context_module)
                    {
                        // The import tells us it's from a specific module
                        // Try to find a class with this name in that module
                        return self.find_class_by_qualified_name(&qualified);
                    }
                }

                // Not imported - might be in the same module
                let id = ClassId::new(context_module.to_string(), name.clone());
                if self.classes.contains_key(&id) {
                    return Some(id);
                }

                // Try to find by name alone (if unambiguous)
                let matches = self.find_by_name(name)?;
                if matches.len() == 1 {
                    return Some(matches[0].clone());
                }

                None
            }
            BaseClass::Attribute(parts) => {
                // For attribute references like `module.Class`, we need to figure out
                // what `module` refers to based on imports.

                if parts.len() < 2 {
                    return None;
                }

                // The last part is the class name, everything before is the module/package
                let class_name = parts.last().unwrap();

                // Check if this is a fully qualified reference
                // Try progressively shorter module paths
                for i in (0..parts.len() - 1).rev() {
                    let module_path = parts[..=i].join(".");
                    let _remaining_parts = &parts[i + 1..];

                    // Check if this module path matches an import
                    if let Some(imports) = self.imports.get(context_module) {
                        if let Some(resolved) =
                            crate::parser::resolve_import(&parts[0], imports, context_module)
                        {
                            // Build the full path
                            let full_module = if parts.len() > 2 {
                                format!("{}.{}", resolved, parts[1..parts.len() - 1].join("."))
                            } else {
                                resolved
                            };

                            let id = ClassId::new(full_module, class_name.to_string());
                            if self.classes.contains_key(&id) {
                                return Some(id);
                            }
                        }
                    }

                    // Try as a direct module path
                    let id = ClassId::new(module_path, class_name.to_string());
                    if self.classes.contains_key(&id) {
                        return Some(id);
                    }
                }

                None
            }
        }
    }

    /// Finds a class by its qualified name (e.g., "foo.bar.ClassName").
    fn find_class_by_qualified_name(&self, qualified: &str) -> Option<ClassId> {
        // Split into module path and class name
        let parts: Vec<&str> = qualified.split('.').collect();
        if parts.is_empty() {
            return None;
        }

        let class_name = parts.last().unwrap();

        // Try progressively shorter module paths
        for i in (0..parts.len() - 1).rev() {
            let module_path = parts[..=i].join(".");
            let id = ClassId::new(module_path, class_name.to_string());
            if self.classes.contains_key(&id) {
                return Some(id);
            }
        }

        None
    }

    /// Returns the number of classes in the registry.
    pub fn len(&self) -> usize {
        self.classes.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }
}

impl Default for ClassRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ClassDefinition;

    #[test]
    fn test_add_and_find_class() {
        let mut registry = ClassRegistry::new();

        let class = ClassDefinition {
            name: "Dog".to_string(),
            module_path: "animals".to_string(),
            file_path: PathBuf::from("animals.py"),
            bases: vec![],
        };

        registry.add_class(class);

        // Find by name only
        let matches = registry.find_by_name("Dog").unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].class_name, "Dog");

        // Find by name and module
        let info = registry.find_class("Dog", Some("animals")).unwrap();
        assert_eq!(info.id.class_name, "Dog");
    }

    #[test]
    fn test_ambiguous_class_name() {
        let mut registry = ClassRegistry::new();

        registry.add_class(ClassDefinition {
            name: "Animal".to_string(),
            module_path: "zoo".to_string(),
            file_path: PathBuf::from("zoo.py"),
            bases: vec![],
        });

        registry.add_class(ClassDefinition {
            name: "Animal".to_string(),
            module_path: "farm".to_string(),
            file_path: PathBuf::from("farm.py"),
            bases: vec![],
        });

        // Should find both
        let matches = registry.find_by_name("Animal").unwrap();
        assert_eq!(matches.len(), 2);

        // Without module path, find_class returns None for ambiguous names
        assert!(registry.find_class("Animal", None).is_none());

        // With module path, should find the right one
        let info = registry.find_class("Animal", Some("zoo")).unwrap();
        assert_eq!(info.id.module_path, "zoo");
    }
}
