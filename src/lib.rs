//! A library for finding all subclasses of a Python class in a codebase.
//!
//! This library parses Python files, builds an inheritance graph, and finds all
//! transitive subclasses of a given class. It handles imports, re-exports, and
//! ambiguous class names.
//!
//! # Examples
//!
//! ```no_run
//! use pysubclasses::{SubclassFinder, SearchMode};
//! use std::path::PathBuf;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let finder = SubclassFinder::new(PathBuf::from("./src"))?;
//! let subclasses = finder.find_subclasses("BaseClass", None, SearchMode::All)?;
//!
//! for class_ref in subclasses {
//!     println!("{} ({})", class_ref.class_name, class_ref.module_path);
//! }
//! # Ok(())
//! # }
//! ```

pub mod cache;
pub mod discovery;
pub mod error;
pub mod graph;
pub mod parser;
pub mod registry;

use std::path::PathBuf;

pub use error::{Error, Result};
use graph::InheritanceGraph;

use crate::registry::Registry;

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

/// Mode for searching the inheritance graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Find only direct relationships (one level of inheritance)
    Direct,
    /// Find all transitive relationships (any depth)
    All,
}

/// The main entry point for finding Python subclasses.
///
/// # Examples
///
/// ```no_run
/// use pysubclasses::{SubclassFinder, SearchMode};
/// use std::path::PathBuf;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a finder for the current directory
/// let finder = SubclassFinder::new(PathBuf::from("."))?;
///
/// // Find all subclasses of "Animal"
/// let subclasses = finder.find_subclasses("Animal", None, SearchMode::All)?;
///
/// // Find direct subclasses of "Animal" from a specific module
/// let subclasses = finder.find_subclasses("Animal", Some("zoo.animals"), SearchMode::Direct)?;
/// # Ok(())
/// # }
/// ```
pub struct SubclassFinder {
    registry: Registry,
    graph: InheritanceGraph,
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
        Self::with_options(root_dir, Vec::new(), true)
    }

    /// Creates a new SubclassFinder with custom options.
    ///
    /// # Arguments
    ///
    /// * `root_dir` - The root directory to search for Python files
    /// * `exclude_dirs` - Directories to exclude from the search
    /// * `use_cache` - Whether to use the cache for faster repeated runs
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The directory cannot be read
    /// - Any Python files cannot be parsed
    pub fn with_options(
        root_dir: PathBuf,
        exclude_dirs: Vec<PathBuf>,
        use_cache: bool,
    ) -> Result<Self> {
        let root_dir = root_dir.canonicalize()?;

        // Discover all Python files
        let python_files =
            discovery::discover_python_files_with_exclusions(&root_dir, &exclude_dirs)?;

        // Parse files in parallel (with optional caching)
        let parse_results = if use_cache {
            cache::parse_with_cache(&root_dir, &python_files)?
        } else {
            parser::parse_files(&root_dir, &python_files)?
        };

        // Log any parse errors and collect successful parses
        let parsed_files: Vec<_> = parse_results
            .into_iter()
            .filter_map(|result| match result {
                Ok(parsed) => Some(parsed),
                Err(e) => {
                    log::warn!("{e}");
                    None
                }
            })
            .collect();

        let registry = Registry::build(&parsed_files)?;

        // Build the inheritance graph
        let graph = InheritanceGraph::build(&registry);

        Ok(Self { registry, graph })
    }

    /// Finds subclasses of a given class with a specified mode.
    ///
    /// # Arguments
    ///
    /// * `class_name` - The simple name of the class to find subclasses for
    /// * `module_path` - Optional module path to disambiguate the class if the name
    ///   appears multiple times in the codebase
    /// * `mode` - Whether to find only direct subclasses or all transitive subclasses
    ///
    /// # Returns
    ///
    /// A sorted vector of subclasses. The results are sorted by module path for
    /// consistent output.
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
    /// use pysubclasses::{SubclassFinder, SearchMode};
    /// use std::path::PathBuf;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let finder = SubclassFinder::new(PathBuf::from("."))?;
    ///
    /// // Find all transitive subclasses
    /// let subclasses = finder.find_subclasses("Animal", None, SearchMode::All)?;
    ///
    /// // Find only direct subclasses
    /// let direct = finder.find_subclasses("Animal", None, SearchMode::Direct)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn find_subclasses(
        &self,
        class_name: &str,
        module_path: Option<&str>,
        mode: SearchMode,
    ) -> Result<Vec<ClassReference>> {
        // Find the target class
        let target_id = self.resolve_target_class(class_name, module_path)?;

        // Find subclasses using the graph based on the mode
        let subclass_ids = match mode {
            SearchMode::Direct => self.graph.find_direct_subclasses(&target_id),
            SearchMode::All => self.graph.find_all_subclasses(&target_id),
        };

        // Convert to ClassReference
        let mut results: Vec<ClassReference> = subclass_ids
            .into_iter()
            .filter_map(|id| {
                self.registry
                    .modules
                    .get(&id.module)
                    .map(|metadata| ClassReference {
                        class_name: id.name.clone(),
                        module_path: id.module.clone(),
                        file_path: metadata.file_path.clone(),
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

    /// Finds parent classes of a given class with a specified mode.
    ///
    /// # Arguments
    ///
    /// * `class_name` - The simple name of the class to find parent classes for
    /// * `module_path` - Optional module path to disambiguate the class if the name
    ///   appears multiple times in the codebase
    /// * `mode` - Whether to find only direct parents or all transitive parents
    ///
    /// # Returns
    ///
    /// A sorted vector of parent classes. The results are sorted by module path for
    /// consistent output.
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
    /// use pysubclasses::{SubclassFinder, SearchMode};
    /// use std::path::PathBuf;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let finder = SubclassFinder::new(PathBuf::from("."))?;
    ///
    /// // Find all transitive parent classes
    /// let parents = finder.find_parent_classes("Dog", Some("animals"), SearchMode::All)?;
    ///
    /// // Find only direct parent classes
    /// let direct = finder.find_parent_classes("Dog", Some("animals"), SearchMode::Direct)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn find_parent_classes(
        &self,
        class_name: &str,
        module_path: Option<&str>,
        mode: SearchMode,
    ) -> Result<Vec<ClassReference>> {
        // Find the target class
        let class_id = self.resolve_target_class(class_name, module_path)?;

        // Find parent classes using the graph based on the mode
        let parent_ids = match mode {
            SearchMode::Direct => self.graph.find_direct_parent_classes(&class_id),
            SearchMode::All => self.graph.find_all_parent_classes(&class_id),
        };

        // Convert to ClassReference
        let mut results: Vec<ClassReference> = parent_ids
            .into_iter()
            .filter_map(|id| {
                self.registry
                    .modules
                    .get(&id.module)
                    .map(|metadata| ClassReference {
                        class_name: id.name.clone(),
                        module_path: id.module.clone(),
                        file_path: metadata.file_path.clone(),
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

    /// Returns the number of classes found in the codebase.
    pub fn class_count(&self) -> usize {
        self.registry.classes.len()
    }

    /// Resolves a target class by name and optional module path.
    ///
    /// This helper method encapsulates the logic for finding a class given its name
    /// and optional module path. It handles:
    /// - Module-qualified lookups (with re-export resolution)
    /// - Unqualified lookups by class name
    /// - Ambiguity detection when multiple classes have the same name
    ///
    /// # Arguments
    ///
    /// * `class_name` - The simple name of the class to find
    /// * `module_path` - Optional module path to disambiguate the class
    ///
    /// # Returns
    ///
    /// The ClassId of the resolved class.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The class is not found
    /// - The class name is ambiguous and no module path is provided
    fn resolve_target_class(
        &self,
        class_name: &str,
        module_path: Option<&str>,
    ) -> Result<registry::ClassId> {
        if let Some(module) = module_path {
            // Module specified - look for class in that module (including re-exports)
            if let Some(resolved_id) = self.registry.resolve_class(module, class_name) {
                Ok(resolved_id)
            } else {
                Err(Error::ClassNotFound {
                    name: class_name.to_string(),
                    module_path: Some(module.to_string()),
                })
            }
        } else {
            // No module specified - search for class by name
            let mut matches = Vec::new();
            for class_id in self.registry.classes.keys() {
                if class_id.name == class_name {
                    matches.push(class_id.clone());
                }
            }

            match matches.len() {
                0 => Err(Error::ClassNotFound {
                    name: class_name.to_string(),
                    module_path: None,
                }),
                1 => Ok(matches.into_iter().next().unwrap()),
                _ => {
                    let candidates: Vec<String> =
                        matches.iter().map(|id| id.module.clone()).collect();
                    Err(Error::AmbiguousClassName {
                        name: class_name.to_string(),
                        candidates,
                    })
                }
            }
        }
    }

    /// Resolves a class reference by name and optional module.
    pub fn resolve_class_reference(
        &self,
        class_name: &str,
        module_path: Option<&str>,
    ) -> Option<ClassReference> {
        // Use resolve_target_class for the core logic
        let target_id = self.resolve_target_class(class_name, module_path).ok()?;

        // Convert to ClassReference
        let metadata = self.registry.modules.get(&target_id.module)?;
        Some(ClassReference {
            class_name: target_id.name.clone(),
            module_path: target_id.module.clone(),
            file_path: metadata.file_path.clone(),
        })
    }
}
