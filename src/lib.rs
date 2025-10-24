//! A library for finding all subclasses of a Python class in a codebase.
//!
//! This library parses Python files, builds an inheritance graph, and finds all
//! transitive subclasses of a given class. It handles imports, re-exports, and
//! ambiguous class names.
//!
//! # Examples
//!
//! ```no_run
//! use pysubclasses::SubclassFinder;
//! use std::path::PathBuf;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let finder = SubclassFinder::new(PathBuf::from("./src"))?;
//! let subclasses = finder.find_subclasses("BaseClass", None)?;
//!
//! for class_ref in subclasses {
//!     println!("{} ({})", class_ref.class_name, class_ref.module_path);
//! }
//! # Ok(())
//! # }
//! ```

pub mod discovery;
pub mod error;
pub mod graph;
pub mod parser;
pub mod registry;
pub mod utils;

use std::path::PathBuf;

use rayon::prelude::*;

pub use error::{Error, Result};
use registry::{ClassId, ClassRegistry};

/// A reference to a Python class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassReference {
    /// The simple name of the class
    pub class_name: String,
    /// The module path where the class is defined (e.g., "foo.bar")
    pub module_path: String,
    /// The file path where the class is defined
    pub file_path: PathBuf,
}

impl ClassReference {
    /// Returns the fully qualified name of the class.
    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.module_path, self.class_name)
    }
}

/// The main entry point for finding Python subclasses.
///
/// # Examples
///
/// ```no_run
/// use pysubclasses::SubclassFinder;
/// use std::path::PathBuf;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a finder for the current directory
/// let finder = SubclassFinder::new(PathBuf::from("."))?;
///
/// // Find all subclasses of "Animal"
/// let subclasses = finder.find_subclasses("Animal", None)?;
///
/// // Find subclasses of "Animal" from a specific module
/// let subclasses = finder.find_subclasses("Animal", Some("zoo.animals"))?;
/// # Ok(())
/// # }
/// ```
pub struct SubclassFinder {
    root_dir: PathBuf,
    registry: ClassRegistry,
}

impl SubclassFinder {
    /// Creates a new SubclassFinder for the given root directory.
    ///
    /// This will discover and parse all Python files in the directory tree.
    ///
    /// # Arguments
    ///
    /// * `root_dir` - The root directory to search for Python files
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The directory cannot be read
    /// - Any Python files cannot be parsed
    pub fn new(root_dir: PathBuf) -> Result<Self> {
        // Discover all Python files
        let python_files = discovery::discover_python_files(&root_dir)?;

        // Parse files in parallel and collect results
        let parsed_files: Vec<_> = python_files
            .par_iter()
            .filter_map(|file_path| {
                // Convert file path to module path
                let module_path = utils::file_path_to_module_path(file_path, &root_dir)?;
                match parser::parse_file(file_path, &module_path) {
                    Ok(parsed) => Some(parsed),
                    Err(e) => {
                        // Log parse errors but continue
                        eprintln!("Warning: {e}");
                        None
                    }
                }
            })
            .collect();

        // Build registry from parsed files
        let registry = ClassRegistry::new(parsed_files);

        Ok(Self { root_dir, registry })
    }

    /// Finds all transitive subclasses of a given class.
    ///
    /// # Arguments
    ///
    /// * `class_name` - The simple name of the class to find subclasses for
    /// * `module_path` - Optional module path to disambiguate the class if the name
    ///   appears multiple times in the codebase
    ///
    /// # Returns
    ///
    /// A sorted vector of all transitive subclasses. The results are sorted by
    /// module path for consistent output.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The class is not found
    /// - The class name is ambiguous and no module path is provided
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pysubclasses::SubclassFinder;
    /// use std::path::PathBuf;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let finder = SubclassFinder::new(PathBuf::from("."))?;
    ///
    /// // Find with just the class name
    /// let subclasses = finder.find_subclasses("Animal", None)?;
    ///
    /// // Find with module path to disambiguate
    /// let subclasses = finder.find_subclasses("Animal", Some("zoo.animals"))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn find_subclasses(
        &self,
        class_name: &str,
        module_path: Option<&str>,
    ) -> Result<Vec<ClassReference>> {
        // Find the target class
        let target_id = self.find_target_class(class_name, module_path)?;

        // Build the inheritance graph
        let graph = graph::InheritanceGraph::build(&self.registry);

        // Find all subclasses
        let subclass_ids = graph.find_all_subclasses(&target_id);

        // Convert to ClassReferences
        let mut results: Vec<ClassReference> = subclass_ids
            .into_iter()
            .filter_map(|id| {
                self.registry.get(&id).map(|info| ClassReference {
                    class_name: id.class_name.clone(),
                    module_path: id.module_path.clone(),
                    file_path: info.file_path.clone(),
                })
            })
            .collect();

        // Sort by module path for consistent output
        results.sort_by(|a, b| {
            a.module_path
                .cmp(&b.module_path)
                .then(a.class_name.cmp(&b.class_name))
        });

        Ok(results)
    }

    /// Finds the ClassId for the target class.
    fn find_target_class(&self, class_name: &str, module_path: Option<&str>) -> Result<ClassId> {
        // If module path provided, look for exact match or re-export
        if let Some(module) = module_path {
            let id = ClassId::new(module.to_string(), class_name.to_string());

            // Try direct lookup first
            if self.registry.get(&id).is_some() {
                return Ok(id);
            }

            // Try resolving through re-exports
            if let Some(resolved) = self.registry.resolve_class_through_reexports(&id) {
                return Ok(resolved);
            }

            return Err(Error::ClassNotFound {
                name: class_name.to_string(),
                module_path: Some(module.to_string()),
            });
        }

        // Otherwise find by name
        let matches = self
            .registry
            .find_by_name(class_name)
            .filter(|ids| !ids.is_empty())
            .ok_or_else(|| Error::ClassNotFound {
                name: class_name.to_string(),
                module_path: None,
            })?;

        match matches.len() {
            1 => Ok(matches[0].clone()),
            _ => {
                let candidates = matches.iter().map(|id| id.module_path.clone()).collect();
                Err(Error::AmbiguousClassName {
                    name: class_name.to_string(),
                    candidates,
                })
            }
        }
    }

    /// Returns the number of classes found in the codebase.
    pub fn class_count(&self) -> usize {
        self.registry.len()
    }

    /// Returns the root directory being searched.
    pub fn root_dir(&self) -> &PathBuf {
        &self.root_dir
    }
}

#[cfg(test)]
mod tests {

    // Integration tests will be in tests/ directory
}
