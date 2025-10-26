//! Inheritance graph construction and traversal.
//!
//! This module provides functionality to build and query a class inheritance graph.
//! The graph maps parent classes to their direct and transitive children, enabling
//! efficient subclass discovery.

use std::collections::{HashMap, HashSet};

use crate::registry::{ClassId, Registry};

/// An inheritance graph representing parent-child relationships between classes.
///
/// This structure maps each class to the set of classes that directly inherit from it.
/// The graph can be used to efficiently find all descendants of a given class using
/// breadth-first search.
pub struct InheritanceGraph {
    /// Maps parent classes to their direct children.
    pub children: HashMap<ClassId, HashSet<ClassId>>,
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

        // Build parent → children edges by examining each class's bases
        for (child_id, metadata) in &registry.classes {
            for base_name in &metadata.bases {
                // Resolve the base class reference in this class's module context
                if let Some(parent_id) = registry.resolve_class(&child_id.module, base_name) {
                    // Add this class as a child of its parent
                    children
                        .entry(parent_id)
                        .or_default()
                        .insert(child_id.clone());
                }
            }
        }

        Self { children }
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
}
