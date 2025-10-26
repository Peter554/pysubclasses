//! Inheritance graph construction and traversal.

use std::collections::{HashMap, HashSet};

use crate::registry::{ClassId, Registry};

pub type ModuleName = String;

/// An inheritance graph mapping classes to their children.
pub struct InheritanceGraph {
    pub children: HashMap<ClassId, HashSet<ClassId>>,
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
    pub fn build(registry: Registry) -> Self {
        // TODO Use `registry.resolve_class`.
        todo!()
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
