use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use crate::parser::Import;

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
    pub class_children: HashMap<ClassId, Vec<ClassId>>,
}

impl Registry {
    pub fn resolve_class(&self, module: &str, name: &str) -> Option<ClassId> {
        // Resolve a class name in a given module.
        //
        // First, check whether a class with this name exists in the module. If so then we're done!
        // If not, we need to use the imports to resolve the class.
        //
        // Suppose we're trying to resolve the name a.b.c.d.
        //
        // First, resolve based on the imports in the module.
        // Look for a prefix based on `imported_as` and then substitute the related `imported_item`.
        // E.g. Suppose { imported_item=pkg.foo, imported_as=a.b }
        // Then the prefix a.b matches, so we substitute and the name becomes pkg.foo.c.d.
        //
        // Next, we have to find which module pkg.foo.c.d refers to. To do this consider in order:
        // - pkg.foo.c.d
        // - pkg.foo.c   (remainder: d)
        // - pkg.foo     (remainder: c.d)
        // - pkg         (remainder: foo.c.d)
        // Once we have a match, then we know we've found the module.
        // We then go to that module and resolve the remainder name (recurse into `resolve_class`).
        // E.g. Suppose pkg.foo matches a module.
        // Then we call `resolve_class` with module="pkg.foo" and remainder="c.d".
        todo!()
    }
}
