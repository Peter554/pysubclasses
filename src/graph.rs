//! Inheritance graph construction and traversal.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::registry::{ClassId, ClassRegistry};

/// An inheritance graph mapping classes to their children.
pub struct InheritanceGraph {
    /// Map from parent ClassId to child ClassIds
    children: HashMap<ClassId, Vec<ClassId>>,
}

impl InheritanceGraph {
    /// Builds an inheritance graph from a class registry.
    ///
    /// # Arguments
    ///
    /// * `registry` - The class registry containing all classes
    ///
    /// # Returns
    ///
    /// An inheritance graph with parent-child relationships.
    pub fn build(registry: &ClassRegistry) -> Self {
        let mut children: HashMap<ClassId, Vec<ClassId>> = HashMap::new();

        // For each class, resolve its base classes and add edges
        for class_id in registry.all_class_ids() {
            if let Some(info) = registry.get(&class_id) {
                for base in &info.bases {
                    // Try to resolve the base class
                    if let Some(parent_id) = registry.resolve_base(base, &class_id.module_path) {
                        // Add edge from parent to child
                        children
                            .entry(parent_id)
                            .or_default()
                            .push(class_id.clone());
                    }
                }
            }
        }

        Self { children }
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
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Start with the root's immediate children
        if let Some(children) = self.children.get(root) {
            for child in children {
                queue.push_back(child.clone());
            }
        }

        // BFS traversal
        while let Some(class_id) = queue.pop_front() {
            // Skip if already visited (handles potential cycles)
            if visited.contains(&class_id) {
                continue;
            }

            visited.insert(class_id.clone());
            result.push(class_id.clone());

            // Add this class's children to the queue
            if let Some(children) = self.children.get(&class_id) {
                for child in children {
                    if !visited.contains(child) {
                        queue.push_back(child.clone());
                    }
                }
            }
        }

        result
    }

    /// Gets the immediate children of a class.
    pub fn get_children(&self, class_id: &ClassId) -> Option<&Vec<ClassId>> {
        self.children.get(class_id)
    }

    /// Returns true if the given class has any subclasses.
    pub fn has_subclasses(&self, class_id: &ClassId) -> bool {
        self.children.contains_key(class_id)
    }

    /// Returns the total number of parent-child relationships in the graph.
    pub fn edge_count(&self) -> usize {
        self.children.values().map(|v| v.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{BaseClass, ClassDefinition};
    use crate::registry::ClassRegistry;
    use std::path::PathBuf;

    #[test]
    fn test_simple_inheritance() {
        let mut registry = ClassRegistry::new();

        // Animal (base class)
        registry.add_file(crate::parser::ParsedFile {
            module_path: "animals".to_string(),
            classes: vec![ClassDefinition {
                name: "Animal".to_string(),
                module_path: "animals".to_string(),
                file_path: PathBuf::from("animals.py"),
                bases: vec![],
            }],
            imports: vec![],
        });

        // Dog(Animal)
        registry.add_file(crate::parser::ParsedFile {
            module_path: "animals".to_string(),
            classes: vec![ClassDefinition {
                name: "Dog".to_string(),
                module_path: "animals".to_string(),
                file_path: PathBuf::from("animals.py"),
                bases: vec![BaseClass::Simple("Animal".to_string())],
            }],
            imports: vec![],
        });

        let graph = InheritanceGraph::build(&registry);

        let animal_id = ClassId::new("animals".to_string(), "Animal".to_string());
        let subclasses = graph.find_all_subclasses(&animal_id);

        assert_eq!(subclasses.len(), 1);
        assert_eq!(subclasses[0].class_name, "Dog");
    }

    #[test]
    fn test_transitive_inheritance() {
        let mut registry = ClassRegistry::new();

        // Animal
        registry.add_file(crate::parser::ParsedFile {
            module_path: "base".to_string(),
            classes: vec![ClassDefinition {
                name: "Animal".to_string(),
                module_path: "base".to_string(),
                file_path: PathBuf::from("base.py"),
                bases: vec![],
            }],
            imports: vec![],
        });

        // Mammal(Animal)
        registry.add_file(crate::parser::ParsedFile {
            module_path: "base".to_string(),
            classes: vec![ClassDefinition {
                name: "Mammal".to_string(),
                module_path: "base".to_string(),
                file_path: PathBuf::from("base.py"),
                bases: vec![BaseClass::Simple("Animal".to_string())],
            }],
            imports: vec![],
        });

        // Dog(Mammal)
        registry.add_file(crate::parser::ParsedFile {
            module_path: "base".to_string(),
            classes: vec![ClassDefinition {
                name: "Dog".to_string(),
                module_path: "base".to_string(),
                file_path: PathBuf::from("base.py"),
                bases: vec![BaseClass::Simple("Mammal".to_string())],
            }],
            imports: vec![],
        });

        let graph = InheritanceGraph::build(&registry);

        let animal_id = ClassId::new("base".to_string(), "Animal".to_string());
        let subclasses = graph.find_all_subclasses(&animal_id);

        // Should find both Mammal and Dog
        assert_eq!(subclasses.len(), 2);

        let names: HashSet<_> = subclasses.iter().map(|c| c.class_name.as_str()).collect();
        assert!(names.contains("Mammal"));
        assert!(names.contains("Dog"));
    }

    #[test]
    fn test_multiple_inheritance() {
        let mut registry = ClassRegistry::new();

        // Animal
        registry.add_file(crate::parser::ParsedFile {
            module_path: "base".to_string(),
            classes: vec![ClassDefinition {
                name: "Animal".to_string(),
                module_path: "base".to_string(),
                file_path: PathBuf::from("base.py"),
                bases: vec![],
            }],
            imports: vec![],
        });

        // Pet
        registry.add_file(crate::parser::ParsedFile {
            module_path: "base".to_string(),
            classes: vec![ClassDefinition {
                name: "Pet".to_string(),
                module_path: "base".to_string(),
                file_path: PathBuf::from("base.py"),
                bases: vec![],
            }],
            imports: vec![],
        });

        // Dog(Animal, Pet)
        registry.add_file(crate::parser::ParsedFile {
            module_path: "base".to_string(),
            classes: vec![ClassDefinition {
                name: "Dog".to_string(),
                module_path: "base".to_string(),
                file_path: PathBuf::from("base.py"),
                bases: vec![
                    BaseClass::Simple("Animal".to_string()),
                    BaseClass::Simple("Pet".to_string()),
                ],
            }],
            imports: vec![],
        });

        let graph = InheritanceGraph::build(&registry);

        // Dog should be a subclass of both Animal and Pet
        let animal_id = ClassId::new("base".to_string(), "Animal".to_string());
        let animal_subclasses = graph.find_all_subclasses(&animal_id);
        assert_eq!(animal_subclasses.len(), 1);
        assert_eq!(animal_subclasses[0].class_name, "Dog");

        let pet_id = ClassId::new("base".to_string(), "Pet".to_string());
        let pet_subclasses = graph.find_all_subclasses(&pet_id);
        assert_eq!(pet_subclasses.len(), 1);
        assert_eq!(pet_subclasses[0].class_name, "Dog");
    }

    #[test]
    fn test_no_subclasses() {
        let mut registry = ClassRegistry::new();

        registry.add_file(crate::parser::ParsedFile {
            module_path: "base".to_string(),
            classes: vec![ClassDefinition {
                name: "Animal".to_string(),
                module_path: "base".to_string(),
                file_path: PathBuf::from("base.py"),
                bases: vec![],
            }],
            imports: vec![],
        });

        let graph = InheritanceGraph::build(&registry);

        let animal_id = ClassId::new("base".to_string(), "Animal".to_string());
        let subclasses = graph.find_all_subclasses(&animal_id);

        assert_eq!(subclasses.len(), 0);
    }
}
