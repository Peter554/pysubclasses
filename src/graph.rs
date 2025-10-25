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

#[derive(Debug)]
pub struct ClassId {
    pub module: ModuleName,
    pub name: String,
}

/// An inheritance graph mapping classes to their children.
pub struct InheritanceGraph {
    pub modules: HashMap<ModuleName, ModuleMetadata>,
    pub classes: HashMap<ModuleName, Vec<ClassId>>,
    pub children: HashMap<ClassId, Vec<ClassId>>,
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
    pub fn build(parsed_files: &ParsedFile) -> Self {}

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
    pub fn find_all_subclasses(&self, root: &ClassId) -> Vec<ClassId> {}
}
