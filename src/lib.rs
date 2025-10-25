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

use std::path::PathBuf;

pub use error::{Error, Result};
use graph::InheritanceGraph;

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
        // Discover all Python files
        let python_files = discovery::discover_python_files(&root_dir)?;

        // Parse files in parallel and collect results
        let parse_results = parser::parse_files(&root_dir, &python_files)?;

        // Log any parse errors and collect successful parses
        let parsed_files: Vec<_> = parse_results
            .into_iter()
            .filter_map(|result| match result {
                Ok(parsed) => Some(parsed),
                Err(e) => {
                    eprintln!("Warning: {e}");
                    None
                }
            })
            .collect();

        // Build the inheritance graph
        let graph = graph::InheritanceGraph::build(&parsed_files);

        Ok(Self { graph })
    }

    /// Returns the total number of classes found in the codebase.
    pub fn class_count(&self) -> usize {
        self.graph.classes.values().map(|v| v.len()).sum()
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
        // Find all classes with the given name
        let mut candidates = Vec::new();
        for class_ids in self.graph.classes.values() {
            for class_id in class_ids {
                if class_id.name == class_name {
                    candidates.push(class_id);
                }
            }
        }

        // Filter by module path if provided
        let root_class = if let Some(module) = module_path {
            // Try exact match first
            let exact_match = candidates.iter().find(|c| c.module == module).copied();

            if let Some(class_id) = exact_match {
                class_id
            } else {
                // Check if the module re-exports the class
                // Look for imports in the specified module that import this class
                if let Some(imports) = self.graph.imports.get(module) {
                    let mut found = None;
                    for import in imports {
                        if let graph::ResolvedImport::ModuleMember {
                            module: source_module,
                            member,
                            ..
                        } = import
                        {
                            if member == class_name {
                                // Check if the class exists in the source module
                                found = candidates
                                    .iter()
                                    .find(|c| c.module == *source_module)
                                    .copied();
                                if found.is_some() {
                                    break;
                                }
                            }
                        }
                    }

                    if let Some(class_id) = found {
                        class_id
                    } else {
                        return Err(Error::ClassNotFound {
                            name: class_name.to_string(),
                            module_path: Some(module.to_string()),
                        });
                    }
                } else {
                    return Err(Error::ClassNotFound {
                        name: class_name.to_string(),
                        module_path: Some(module.to_string()),
                    });
                }
            }
        } else {
            // No module path specified
            if candidates.is_empty() {
                return Err(Error::ClassNotFound {
                    name: class_name.to_string(),
                    module_path: None,
                });
            } else if candidates.len() > 1 {
                let module_names: Vec<String> =
                    candidates.iter().map(|c| c.module.clone()).collect();
                return Err(Error::AmbiguousClassName {
                    name: class_name.to_string(),
                    candidates: module_names,
                });
            } else {
                candidates[0]
            }
        };

        // Find all subclasses using BFS
        let subclass_ids = self.graph.find_all_subclasses(root_class);

        // Convert ClassIds to ClassReferences
        let mut references = Vec::new();
        for class_id in subclass_ids {
            if let Some(metadata) = self.graph.modules.get(&class_id.module) {
                references.push(ClassReference {
                    class_name: class_id.name.clone(),
                    module_path: class_id.module.clone(),
                    file_path: metadata.file_path.clone(),
                });
            }
        }

        // Sort by module path for consistent output
        references.sort_by(|a, b| {
            a.module_path
                .cmp(&b.module_path)
                .then_with(|| a.class_name.cmp(&b.class_name))
        });

        Ok(references)
    }
}
