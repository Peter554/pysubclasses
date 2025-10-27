//! Inheritance graph construction and traversal.
//!
//! This module provides functionality to build and query a class inheritance graph.
//! The graph maps parent classes to their direct and transitive children, enabling
//! efficient subclass discovery.

use std::collections::{HashMap, HashSet};

use crate::registry::{ClassId, Registry};

/// An inheritance graph representing parent-child relationships between classes.
///
/// This structure maps each class to the set of classes that directly inherit from it,
/// and also maintains the reverse mapping for efficient parent lookup.
/// The graph can be used to efficiently find all descendants or ancestors of a given
/// class using breadth-first search.
pub struct InheritanceGraph {
    /// Maps parent classes to their direct children.
    pub children: HashMap<ClassId, HashSet<ClassId>>,
    /// Maps child classes to their direct parents.
    parents: HashMap<ClassId, HashSet<ClassId>>,
}

impl InheritanceGraph {
    /// Builds an inheritance graph from a registry.
    ///
    /// This resolves all base class references and constructs parent-to-child
    /// relationships. The registry's `resolve_class` method is used to handle
    /// imports and re-exports correctly.
    ///
    /// # Arguments
    ///
    /// * `registry` - The registry containing all classes and their base class references
    ///
    /// # Returns
    ///
    /// An inheritance graph ready for traversal.
    ///
    /// # Algorithm
    ///
    /// For each class in the registry:
    /// 1. Resolve each of its base class names to a `ClassId`
    /// 2. Add this class to the parent's children set
    /// 3. Skip any base classes that cannot be resolved (e.g., external dependencies)
    pub fn build(registry: &Registry) -> Self {
        let mut children: HashMap<ClassId, HashSet<ClassId>> = HashMap::new();
        let mut parents: HashMap<ClassId, HashSet<ClassId>> = HashMap::new();

        // Build parent → children and child → parents edges by examining each class's bases
        for (child_id, metadata) in &registry.classes {
            for base_name in &metadata.bases {
                // Resolve the base class reference in this class's module context
                if let Some(parent_id) = registry.resolve_class(&child_id.module, base_name) {
                    // Add this class as a child of its parent
                    children
                        .entry(parent_id.clone())
                        .or_default()
                        .insert(child_id.clone());

                    // Add the parent as a parent of this class
                    parents
                        .entry(child_id.clone())
                        .or_default()
                        .insert(parent_id);
                }
            }
        }

        Self { children, parents }
    }

    /// Finds only the direct subclasses of a given class.
    ///
    /// Returns classes that directly inherit from the specified root class,
    /// without traversing further down the inheritance hierarchy.
    ///
    /// # Arguments
    ///
    /// * `root` - The class to find direct subclasses for
    ///
    /// # Returns
    ///
    /// A vector containing only the direct subclasses. The root class itself is not
    /// included in the result.
    ///
    /// # Examples
    ///
    /// ```text
    /// Given:
    ///   class Animal: pass
    ///   class Mammal(Animal): pass
    ///   class Dog(Mammal): pass
    ///   class Cat(Mammal): pass
    ///
    /// find_direct_subclasses(Animal) → [Mammal]
    /// find_direct_subclasses(Mammal) → [Dog, Cat]
    /// ```
    pub fn find_direct_subclasses(&self, root: &ClassId) -> Vec<ClassId> {
        self.children
            .get(root)
            .map(|children| children.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Finds all transitive subclasses of a given class.
    ///
    /// Performs a breadth-first search to discover all classes that directly or
    /// indirectly inherit from the specified root class. This includes:
    /// - Direct children (one level of inheritance)
    /// - Grandchildren (two levels)
    /// - Great-grandchildren, etc. (any depth)
    ///
    /// # Arguments
    ///
    /// * `root` - The class to find subclasses for
    ///
    /// # Returns
    ///
    /// A vector containing all transitive subclasses. The root class itself is not
    /// included in the result. The order of classes in the vector is determined by
    /// the BFS traversal order.
    ///
    /// # Examples
    ///
    /// ```text
    /// Given:
    ///   class Animal: pass
    ///   class Mammal(Animal): pass
    ///   class Dog(Mammal): pass
    ///   class Cat(Mammal): pass
    ///
    /// find_all_subclasses(Animal) → [Mammal, Dog, Cat]
    /// ```
    pub fn find_all_subclasses(&self, root: &ClassId) -> Vec<ClassId> {
        use std::collections::VecDeque;

        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Initialize BFS with the root class
        queue.push_back(root.clone());
        visited.insert((root.module.clone(), root.name.clone()));

        // BFS traversal
        while let Some(current) = queue.pop_front() {
            // Examine all direct children of the current class
            if let Some(children) = self.children.get(&current) {
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

    /// Finds only the direct parent classes of a given class.
    ///
    /// Returns classes that the specified class directly inherits from,
    /// without traversing further up the inheritance hierarchy.
    ///
    /// # Arguments
    ///
    /// * `root` - The class to find direct parent classes for
    ///
    /// # Returns
    ///
    /// A vector containing only the direct parent classes.
    ///
    /// # Examples
    ///
    /// ```text
    /// Given:
    ///   class Animal: pass
    ///   class Mammal(Animal): pass
    ///   class Dog(Mammal): pass
    ///
    /// find_direct_parent_classes(Dog) → [Mammal]
    /// find_direct_parent_classes(Mammal) → [Animal]
    /// ```
    pub fn find_direct_parent_classes(&self, root: &ClassId) -> Vec<ClassId> {
        self.parents
            .get(root)
            .map(|parents| parents.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Finds all transitive parent classes of a given class.
    ///
    /// Performs a breadth-first search to discover all classes that the specified
    /// class directly or indirectly inherits from. This includes:
    /// - Direct parents (one level of inheritance)
    /// - Grandparents (two levels)
    /// - Great-grandparents, etc. (any depth)
    ///
    /// # Arguments
    ///
    /// * `root` - The class to find parent classes for
    ///
    /// # Returns
    ///
    /// A vector containing all transitive parent classes. The root class itself is not
    /// included in the result. The order of classes in the vector is determined by
    /// the BFS traversal order.
    ///
    /// # Examples
    ///
    /// ```text
    /// Given:
    ///   class Animal: pass
    ///   class Mammal(Animal): pass
    ///   class Dog(Mammal): pass
    ///
    /// find_all_parent_classes(Dog) → [Mammal, Animal]
    /// ```
    pub fn find_all_parent_classes(&self, root: &ClassId) -> Vec<ClassId> {
        use std::collections::VecDeque;

        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Initialize BFS with the root class
        queue.push_back(root.clone());
        visited.insert((root.module.clone(), root.name.clone()));

        // BFS traversal
        while let Some(current) = queue.pop_front() {
            // Examine all direct parents of the current class
            if let Some(parents) = self.parents.get(&current) {
                for parent in parents {
                    let key = (parent.module.clone(), parent.name.clone());
                    if !visited.contains(&key) {
                        visited.insert(key);
                        result.push(parent.clone());
                        queue.push_back(parent.clone());
                    }
                }
            }
        }

        result
    }
}
