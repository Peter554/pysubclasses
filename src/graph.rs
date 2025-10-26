//! Inheritance graph construction and traversal.

use std::collections::{HashMap, HashSet};

use crate::registry::{ClassId, Registry};

/// An inheritance graph mapping classes to their children.
pub struct InheritanceGraph {
    pub children: HashMap<ClassId, HashSet<ClassId>>,
}

impl InheritanceGraph {
    pub fn build(registry: &Registry) -> Self {
        let mut children: HashMap<ClassId, HashSet<ClassId>> = HashMap::new();

        // Iterate through all classes and resolve their base classes
        for (child_id, metadata) in &registry.classes {
            for base_name in &metadata.bases {
                // Try to resolve the base class using the registry
                if let Some(parent_id) = registry.resolve_class(&child_id.module, base_name) {
                    // Add child to parent's children set
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
