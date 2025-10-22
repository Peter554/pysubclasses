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
}

/// Information about where a class is defined.
#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub id: ClassId,
    pub file_path: PathBuf,
    pub bases: Vec<BaseClass>,
}

/// A registry of all classes found in a codebase.
#[derive(Default)]
pub struct ClassRegistry {
    /// Map from ClassId to ClassInfo
    classes: HashMap<ClassId, ClassInfo>,

    /// Map from simple class name to all ClassIds with that name
    /// (for ambiguity detection)
    name_index: HashMap<String, Vec<ClassId>>,

    /// Map from module path to imports in that module
    imports: HashMap<String, Vec<Import>>,

    /// Map from ClassId (re-exported location) to ClassId (original location)
    /// E.g., foo.Bar -> foo._internal.Bar
    re_exports: HashMap<ClassId, ClassId>,

    /// Set of module paths that are packages (__init__.py files)
    packages: std::collections::HashSet<String>,
}

impl ClassRegistry {
    /// Creates a new empty class registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a parsed file to the registry.
    pub fn add_file(&mut self, parsed: ParsedFile) {
        // Store imports for this module
        self.imports
            .insert(parsed.module_path.clone(), parsed.imports.clone());

        // Track if this is a package
        if parsed.is_package {
            self.packages.insert(parsed.module_path.clone());
        }

        // Add all classes
        for class in parsed.classes {
            self.add_class(class);
        }

        // Track re-exports: when we import a class, it may be re-exported
        // We'll do a second pass after all files are added
    }

    /// Second pass: build re-export mappings after all classes are registered.
    /// This should be called after all files have been added.
    ///
    /// This may need multiple iterations to resolve transitive re-exports
    /// (e.g., A re-exports from B, B re-exports from C).
    pub fn build_reexports(&mut self) {
        // Iterate until no new re-exports are added (fixed-point iteration)
        let mut changed = true;
        while changed {
            changed = false;
            let mut reexports_to_add = Vec::new();

            for (module_path, imports) in &self.imports {
                for import in imports {
                    match import {
                        Import::From { module, names } => {
                            for (name, alias) in names {
                                // The exported name is the alias if present, otherwise the original name
                                let exported_name = alias.as_ref().unwrap_or(name);

                                // Try to find the original class
                                let original_module = module.clone();
                                let original_id =
                                    ClassId::new(original_module.clone(), name.clone());

                                // Check if the class exists directly or as a re-export
                                if self.classes.contains_key(&original_id)
                                    || self.re_exports.contains_key(&original_id)
                                {
                                    // Register the re-export
                                    let reexport_id =
                                        ClassId::new(module_path.clone(), exported_name.clone());
                                    reexports_to_add.push((reexport_id, original_id));
                                }
                            }
                        }
                        Import::RelativeFrom {
                            level,
                            module: rel_module,
                            names,
                        } => {
                            // Resolve the relative import
                            let is_package = self.packages.contains(module_path);
                            if let Some(base) = crate::parser::resolve_relative_import_with_context(
                                module_path,
                                *level,
                                is_package,
                            ) {
                                for (name, alias) in names {
                                    let exported_name = alias.as_ref().unwrap_or(name);

                                    // Build the full module path
                                    let original_module = if let Some(m) = rel_module {
                                        if base.is_empty() {
                                            m.clone()
                                        } else {
                                            format!("{base}.{m}")
                                        }
                                    } else {
                                        base.clone()
                                    };

                                    let original_id = ClassId::new(original_module, name.clone());

                                    // Check if the class exists directly or as a re-export
                                    // (we'll resolve the chain later)
                                    if self.classes.contains_key(&original_id)
                                        || self.re_exports.contains_key(&original_id)
                                    {
                                        let reexport_id = ClassId::new(
                                            module_path.clone(),
                                            exported_name.clone(),
                                        );
                                        reexports_to_add.push((reexport_id, original_id));
                                    }
                                }
                            }
                        }
                        Import::Module { .. } => {
                            // Module imports don't re-export classes
                        }
                    }
                }
            }

            // Add all the re-exports
            for (reexport_id, original_id) in reexports_to_add {
                // Only add if it's new
                if let std::collections::hash_map::Entry::Vacant(e) =
                    self.re_exports.entry(reexport_id)
                {
                    e.insert(original_id);
                    changed = true;
                }
            }
        }
    }

    /// Adds a class definition to the registry.
    fn add_class(&mut self, class: ClassDefinition) {
        let id = ClassId::new(class.module_path, class.name.clone());

        let info = ClassInfo {
            id: id.clone(),
            file_path: class.file_path,
            bases: class.bases,
        };

        // Add to name index
        self.name_index
            .entry(class.name)
            .or_default()
            .push(id.clone());

        // Add to main registry
        self.classes.insert(id, info);
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

    /// Resolves a ClassId through re-exports to find the canonical ClassId.
    /// If the given ClassId is a re-export, follows the chain to find the original.
    /// Otherwise returns the input ClassId if it exists in the registry.
    fn resolve_through_reexports(&self, id: &ClassId) -> Option<ClassId> {
        let mut current = id.clone();
        let mut visited = std::collections::HashSet::new();

        // Follow the re-export chain
        loop {
            // Prevent infinite loops
            if visited.contains(&current) {
                break;
            }
            visited.insert(current.clone());

            // Check if this is a re-export
            if let Some(original) = self.re_exports.get(&current) {
                current = original.clone();
            } else {
                // No more re-exports, check if this exists
                if self.classes.contains_key(&current) {
                    return Some(current);
                }
                break;
            }
        }

        None
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
                    let is_package = self.packages.contains(context_module);
                    if let Some(qualified) =
                        crate::parser::resolve_import(name, imports, context_module, is_package)
                    {
                        // The import tells us it's from a specific module
                        // Try to find a class with this name in that module
                        return self.find_class_by_qualified_name(&qualified);
                    }
                }

                // Not imported - might be in the same module
                let id = ClassId::new(context_module.to_string(), name.clone());
                if let Some(resolved) = self.resolve_through_reexports(&id) {
                    return Some(resolved);
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
                        let is_package = self.packages.contains(context_module);
                        if let Some(resolved) = crate::parser::resolve_import(
                            &parts[0],
                            imports,
                            context_module,
                            is_package,
                        ) {
                            // Build the full path
                            let full_module = if parts.len() > 2 {
                                format!("{}.{}", resolved, parts[1..parts.len() - 1].join("."))
                            } else {
                                resolved
                            };

                            let id = ClassId::new(full_module, class_name.to_string());
                            if let Some(resolved) = self.resolve_through_reexports(&id) {
                                return Some(resolved);
                            }
                        }
                    }

                    // Try as a direct module path
                    let id = ClassId::new(module_path, class_name.to_string());
                    if let Some(resolved) = self.resolve_through_reexports(&id) {
                        return Some(resolved);
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
            if let Some(resolved) = self.resolve_through_reexports(&id) {
                return Some(resolved);
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

    #[test]
    fn test_simple_reexport() {
        use crate::parser::{Import, ParsedFile};

        let mut registry = ClassRegistry::new();

        // Define a class in base module
        registry.add_file(ParsedFile {
            module_path: "mypackage.base".to_string(),
            classes: vec![ClassDefinition {
                name: "Animal".to_string(),
                module_path: "mypackage.base".to_string(),
                file_path: PathBuf::from("mypackage/base.py"),
                bases: vec![],
            }],
            imports: vec![],
            is_package: false,
        });

        // Re-export it from __init__.py
        registry.add_file(ParsedFile {
            module_path: "mypackage".to_string(),
            classes: vec![],
            imports: vec![Import::RelativeFrom {
                level: 1,
                module: Some("base".to_string()),
                names: vec![("Animal".to_string(), None)],
            }],
            is_package: true,
        });

        // Build re-exports
        registry.build_reexports();

        // Should be able to find it via the re-exported path
        let id = ClassId::new("mypackage".to_string(), "Animal".to_string());
        let resolved = registry.resolve_through_reexports(&id);
        assert!(resolved.is_some());
        let resolved = resolved.unwrap();
        assert_eq!(resolved.module_path, "mypackage.base");
        assert_eq!(resolved.class_name, "Animal");
    }

    #[test]
    fn test_transitive_reexport() {
        use crate::parser::{Import, ParsedFile};

        let mut registry = ClassRegistry::new();

        // Define a class in _base module
        registry.add_file(ParsedFile {
            module_path: "pkg._nodes._base".to_string(),
            classes: vec![ClassDefinition {
                name: "Node".to_string(),
                module_path: "pkg._nodes._base".to_string(),
                file_path: PathBuf::from("pkg/_nodes/_base.py"),
                bases: vec![],
            }],
            imports: vec![],
            is_package: false,
        });

        // Re-export from _nodes/__init__.py
        registry.add_file(ParsedFile {
            module_path: "pkg._nodes".to_string(),
            classes: vec![],
            imports: vec![Import::RelativeFrom {
                level: 1,
                module: Some("_base".to_string()),
                names: vec![("Node".to_string(), None)],
            }],
            is_package: true,
        });

        // Re-export from pkg/__init__.py
        registry.add_file(ParsedFile {
            module_path: "pkg".to_string(),
            classes: vec![],
            imports: vec![Import::RelativeFrom {
                level: 1,
                module: Some("_nodes".to_string()),
                names: vec![("Node".to_string(), None)],
            }],
            is_package: true,
        });

        // Build re-exports
        registry.build_reexports();

        // Should be able to find it via the top-level re-exported path
        let id = ClassId::new("pkg".to_string(), "Node".to_string());
        let resolved = registry.resolve_through_reexports(&id);
        assert!(resolved.is_some());
        let resolved = resolved.unwrap();
        assert_eq!(resolved.module_path, "pkg._nodes._base");
        assert_eq!(resolved.class_name, "Node");

        // Should also work via the intermediate path
        let id = ClassId::new("pkg._nodes".to_string(), "Node".to_string());
        let resolved = registry.resolve_through_reexports(&id);
        assert!(resolved.is_some());
        let resolved = resolved.unwrap();
        assert_eq!(resolved.module_path, "pkg._nodes._base");
    }

    #[test]
    fn test_reexport_with_inheritance() {
        use crate::parser::{BaseClass, Import, ParsedFile};

        let mut registry = ClassRegistry::new();

        // Define Animal in base module
        registry.add_file(ParsedFile {
            module_path: "animals.base".to_string(),
            classes: vec![ClassDefinition {
                name: "Animal".to_string(),
                module_path: "animals.base".to_string(),
                file_path: PathBuf::from("animals/base.py"),
                bases: vec![],
            }],
            imports: vec![],
            is_package: false,
        });

        // Re-export Animal from animals/__init__.py
        registry.add_file(ParsedFile {
            module_path: "animals".to_string(),
            classes: vec![],
            imports: vec![Import::RelativeFrom {
                level: 1,
                module: Some("base".to_string()),
                names: vec![("Animal".to_string(), None)],
            }],
            is_package: true,
        });

        // Define Dog that inherits from re-exported Animal
        registry.add_file(ParsedFile {
            module_path: "pets".to_string(),
            classes: vec![ClassDefinition {
                name: "Dog".to_string(),
                module_path: "pets".to_string(),
                file_path: PathBuf::from("pets.py"),
                bases: vec![BaseClass::Simple("Animal".to_string())],
            }],
            imports: vec![Import::From {
                module: "animals".to_string(),
                names: vec![("Animal".to_string(), None)],
            }],
            is_package: false,
        });

        // Build re-exports
        registry.build_reexports();

        // Resolve Dog's base class
        let dog_info = registry
            .get(&ClassId::new("pets".to_string(), "Dog".to_string()))
            .unwrap();
        let base = &dog_info.bases[0];
        let resolved_base = registry.resolve_base(base, "pets");

        assert!(resolved_base.is_some());
        let resolved_base = resolved_base.unwrap();
        assert_eq!(resolved_base.module_path, "animals.base");
        assert_eq!(resolved_base.class_name, "Animal");
    }
}
