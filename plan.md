# Python Subclass Finder - Implementation Plan

## Problem Overview

Build a Rust CLI tool that finds all subclasses (transitive) of a given Python class within a codebase. The tool must handle:
- Class name ambiguity (same class name in multiple modules)
- Re-exported classes (class defined in one module, exported from another)
- Transitive subclass relationships (children, grandchildren, etc.)

## Program Flow

### High-Level Algorithm

```
1. Parse CLI arguments (class_name, optional module_path, optional root_dir)
2. Discover all Python files in the root directory
3. Build a comprehensive class registry:
   - Map class names to their definitions (module path + location)
   - Track where each class is defined
   - Track where each class is re-exported (imported and exposed)
4. Build inheritance graph:
   - Parse each file's AST
   - Extract class definitions and their base classes
   - Resolve base class references to actual class definitions
5. If module_path is provided:
   - Find the specific class definition matching (class_name, module_path)
   - Use this as the root for subclass search
6. If module_path is NOT provided:
   - Find all classes matching class_name
   - If multiple found, report ambiguity and suggest using module_path
   - If single found, use it as root
7. Traverse the inheritance graph to find all transitive subclasses
8. Output results (sorted, with module paths)
```

### Detailed Phases

#### Phase 1: File Discovery
- Walk the directory tree from root_dir using the `ignore` crate
- Collect all `.py` files
- Automatically respect `.gitignore` rules (skips `.venv`, `__pycache__`, etc.)
- Skip common non-relevant directories (`.git`, `.tox`, etc.)
- Handle symlinks appropriately (avoid infinite loops)
- The `ignore` crate provides efficient parallel traversal and respects VCS ignore files

#### Phase 2: Class Registry Construction
Build two key data structures:

**ClassDefinition**:
```rust
struct ClassDefinition {
    name: String,
    module_path: String,  // dotted path like "foo.bar.baz"
    file_path: PathBuf,
    bases: Vec<BaseClass>, // unresolved base class references
}

enum BaseClass {
    Simple(String),              // "Foo"
    Attribute(String, String),   // "module.Foo"
    // May need more complex variants for import resolution
}
```

**ClassRegistry**:
- Map from `(class_name, module_path)` -> `ClassDefinition`
- Map from `class_name` -> `Vec<(module_path, ClassDefinition)>` (for ambiguity detection)
- Map for re-exports: `(module_path, name)` -> `(original_module_path, original_name)`

#### Phase 3: Import Resolution
For each file, track:
- `import foo` -> `foo` resolves to module `foo`
- `from foo import Bar` -> `Bar` resolves to `foo.Bar`
- `from foo import Bar as Baz` -> `Baz` resolves to `foo.Bar`
- `from .relative import Bar` -> resolve relative imports

This allows resolving base class references like:
```python
from animals import Mammal

class Dog(Mammal):  # Need to resolve Mammal -> animals.Mammal
    pass
```

#### Phase 4: Inheritance Graph Construction
- Create a directed graph: `Map<ClassId, Vec<ClassId>>`
- `ClassId` = `(module_path, class_name)`
- For each class, resolve its base classes to ClassIds
- Build reverse map: `child_to_parents` and `parent_to_children`

#### Phase 5: Transitive Subclass Search
- Start from target class ClassId
- BFS/DFS to find all descendants in the graph
- Collect all reachable nodes
- Sort results by module path for consistent output

## Crate Architecture

### Library Crate (`src/lib.rs`)

```
src/
├── lib.rs              # Public API
├── discovery.rs        # File discovery logic
├── parser.rs           # AST parsing and class extraction
├── registry.rs         # Class registry and import resolution
├── graph.rs            # Inheritance graph construction and traversal
└── error.rs            # Error types
```

**Public API** (`lib.rs`):
```rust
pub struct SubclassFinder {
    root_dir: PathBuf,
}

impl SubclassFinder {
    pub fn new(root_dir: PathBuf) -> Result<Self, Error>;

    pub fn find_subclasses(
        &self,
        class_name: &str,
        module_path: Option<&str>,
    ) -> Result<Vec<ClassReference>, Error>;
}

pub struct ClassReference {
    pub class_name: String,
    pub module_path: String,
    pub file_path: PathBuf,
}

pub enum Error {
    AmbiguousClassName { name: String, candidates: Vec<String> },
    ClassNotFound { name: String, module_path: Option<String> },
    IoError(std::io::Error),
    ParseError { file: PathBuf, error: String },
}
```

### Binary Crate (`src/main.rs`)

Keep this thin - just CLI argument parsing and output formatting.

```rust
// Use clap for argument parsing
struct Cli {
    class_name: String,
    module_path: Option<String>,
    root_dir: Option<PathBuf>,
    // Format flags: --json, --verbose, etc.
}

fn main() {
    // Parse args
    // Call library
    // Format and print results
    // Handle errors with user-friendly messages
}
```

## External Crates

### Core Dependencies

1. **AST Parsing**:
   ```toml
   ruff_python_parser = { git = "https://github.com/astral-sh/ruff.git", tag = "v0.4.10" }
   ruff_python_ast = { git = "https://github.com/astral-sh/ruff.git", tag = "v0.4.10" }
   ruff_source_file = { git = "https://github.com/astral-sh/ruff.git", tag = "v0.4.10" }
   ```

2. **CLI Parsing**:
   ```toml
   clap = { version = "4", features = ["derive"] }
   ```

3. **Error Handling**:
   ```toml
   anyhow = "1"          # For binary error handling
   thiserror = "1"        # For library error types
   ```

4. **File System Operations**:
   ```toml
   ignore = "0.4"          # Directory traversal with .gitignore support
   ```

5. **Data Structures**:
   ```toml
   petgraph = "0.6"       # For graph operations (optional, could use HashMap)
   ```

6. **Output Formatting** (optional):
   ```toml
   serde = { version = "1", features = ["derive"] }
   serde_json = "1"       # For --json output
   ```

### Development Dependencies

```toml
[dev-dependencies]
assert_cmd = "2"
assert_fs = "1"
predicates = "3"
```

## Testing Strategy

### Unit Tests

1. **Parser Tests** (`parser.rs`):
   - Test extraction of class definitions from various Python syntax
   - Test import statement parsing
   - Test relative import resolution
   - Test edge cases: decorators, nested classes, metaclasses

2. **Registry Tests** (`registry.rs`):
   - Test class lookup by name
   - Test ambiguity detection
   - Test re-export resolution

3. **Graph Tests** (`graph.rs`):
   - Test transitive subclass finding
   - Test circular inheritance detection
   - Test multiple inheritance

### Integration Tests

Create test fixtures in `tests/fixtures/`:

```
tests/
├── integration_test.rs
└── fixtures/
    ├── simple/              # Simple linear inheritance
    │   ├── base.py
    │   └── derived.py
    ├── ambiguous/           # Same class name in multiple modules
    │   ├── module_a.py
    │   └── module_b.py
    ├── reexport/            # Re-exported classes
    │   ├── _internal.py
    │   └── __init__.py
    ├── complex/             # Multiple inheritance, deep hierarchies
    │   └── ...
    └── relative_imports/    # Relative import resolution
        └── ...
```

**Integration test structure**:
```rust
use assert_cmd::Command;
use assert_fs::prelude::*;

#[test]
fn test_simple_inheritance() {
    let temp = assert_fs::TempDir::new().unwrap();
    // Create fixture files
    temp.child("base.py").write_str("class Animal: pass").unwrap();
    temp.child("derived.py").write_str(
        "from base import Animal\nclass Dog(Animal): pass"
    ).unwrap();

    // Run CLI
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
       .arg("--root")
       .arg(temp.path())
       .assert()
       .success()
       .stdout(predicates::str::contains("Dog"));
}
```

### Test Cases to Cover

1. **Simple inheritance**: A -> B -> C
2. **Multiple inheritance**: A, B -> C
3. **Diamond inheritance**: A -> B, C -> D
4. **Re-exports**: Class defined in `_internal.py`, imported in `__init__.py`
5. **Ambiguous names**: `Animal` in `zoo.py` and `farm.py`
6. **Relative imports**: `from ..parent import Base`
7. **No subclasses**: Class with no children
8. **Class not found**: Non-existent class name
9. **Complex package structures**: Nested packages with `__init__.py`
10. **Import aliases**: `from foo import Bar as Baz`

### Property-Based Testing (Future Enhancement)

Consider using `proptest` for:
- Generating random Python class hierarchies
- Testing graph traversal invariants
- Testing that all paths are found

## Documentation

### Code Documentation

1. **Module-level docs**: Each module should have `//!` docs explaining its purpose
2. **Public API docs**: All public items must have `///` docs with examples
3. **Examples in docs**: Use `cargo doc` doctests for key APIs

Example:
```rust
/// Finds all subclasses of a given Python class within a codebase.
///
/// # Examples
///
/// ```
/// use pysubclasses::SubclassFinder;
///
/// let finder = SubclassFinder::new("./src".into())?;
/// let subclasses = finder.find_subclasses("BaseClass", None)?;
/// for class_ref in subclasses {
///     println!("{}", class_ref.module_path);
/// }
/// # Ok::<(), pysubclasses::Error>(())
/// ```
pub struct SubclassFinder { /* ... */ }
```

### CLI Documentation

1. **Help text**: Use clap's built-in help with good descriptions
2. **README.md**: Include:
   - Installation instructions
   - Usage examples
   - Common use cases
   - Limitations and known issues

Example help text:
```
pysubclasses - Find all subclasses of a Python class

USAGE:
    pysubclasses [OPTIONS] <CLASS_NAME>

ARGS:
    <CLASS_NAME>    Name of the class to find subclasses for

OPTIONS:
    -m, --module <MODULE>    Dotted module path where the class is defined
                             (use to disambiguate when class name appears
                             multiple times)
    -d, --directory <DIR>    Root directory to search [default: .]
    -f, --format <FORMAT>    Output format: text, json [default: text]
    -v, --verbose            Show additional information
    -h, --help               Print help information
    -V, --version            Print version information

EXAMPLES:
    # Find all subclasses of Animal
    pysubclasses Animal

    # Find subclasses of Animal defined in zoo.animals module
    pysubclasses Animal --module zoo.animals

    # Search in specific directory with JSON output
    pysubclasses Animal --directory ./src --format json
```

### User Documentation

Create a `docs/` folder with:
- `usage.md`: Detailed usage guide
- `architecture.md`: High-level architecture overview
- `limitations.md`: Known limitations and edge cases
- `contributing.md`: How to contribute

## Edge Cases and Considerations

### Import Resolution Challenges

1. **Star imports**: `from foo import *`
   - Need to parse `__all__` in target module
   - May need to import all public names
   - Consider flagging as "incomplete resolution" in verbose mode

2. **Dynamic imports**: `importlib.import_module()`
   - Cannot be statically resolved
   - Document as limitation

3. **Conditional imports**:
   ```python
   if TYPE_CHECKING:
       from typing import Protocol
   ```
   - Parse both branches of conditionals

### Class Definition Challenges

1. **Metaclasses**: `class Foo(metaclass=Meta)`
   - Track as potential base class

2. **Nested classes**:
   ```python
   class Outer:
       class Inner:
           pass
   ```
   - Use qualified names: `Outer.Inner`

3. **Generic classes**: `class Foo(Generic[T])`
   - Parse subscripted bases carefully

4. **String-based inheritance** (Python 3.7+):
   ```python
   class Foo("BaseClass"):  # Forward reference
       pass
   ```
   - Resolve string literals to class names

### Module Path Resolution

- Convert file paths to module paths correctly:
  - `src/foo/bar.py` -> `foo.bar`
  - `src/foo/__init__.py` -> `foo`
- Handle namespace packages (no `__init__.py`)
- Handle different project structures (flat vs. src-layout)

### Performance Considerations

1. **Lazy parsing**: Only parse files as needed for large codebases
2. **Caching**: Cache parsed ASTs for repeated queries
3. **Parallel parsing**: Use rayon for parallel file parsing
4. **Progress indication**: Show progress for large codebases

## Implementation Phases

### Phase 1: Core Infrastructure
- Set up library/binary crate structure
- Implement file discovery
- Integrate ruff parser
- Extract class definitions (ignore inheritance)

### Phase 2: Import Resolution
- Parse import statements
- Build import resolution maps
- Handle relative imports
- Test with simple cases

### Phase 3: Inheritance Graph
- Resolve base classes using import maps
- Build parent-child relationships
- Implement transitive search
- Test with linear inheritance

### Phase 4: Advanced Features
- Handle re-exports
- Ambiguity detection and resolution
- Multiple inheritance
- Error handling and reporting

### Phase 5: CLI and Output
- Implement CLI with clap
- Add output formatting (text, JSON)
- Add verbose mode
- User-friendly error messages

### Phase 6: Testing and Documentation
- Write comprehensive tests
- Add integration tests with assert_cmd/assert_fs
- Write documentation
- Create examples

## Success Criteria

The tool should be able to:
1. ✓ Find all direct and transitive subclasses
2. ✓ Handle re-exported classes correctly
3. ✓ Disambiguate classes with same name using module paths
4. ✓ Provide clear error messages for ambiguous cases
5. ✓ Work with various project structures and import patterns
6. ✓ Be well-documented and tested
7. ✓ Have a clean, ergonomic CLI interface
8. ✓ Handle reasonably sized codebases (1000s of Python files) efficiently
