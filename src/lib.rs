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
    root_dir: PathBuf,
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

        Ok(Self { root_dir, graph })
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
    }
}
