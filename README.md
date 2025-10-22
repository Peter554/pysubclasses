# pysubclasses

[![CI](https://github.com/Peter554/pysubclasses/actions/workflows/ci.yml/badge.svg)](https://github.com/Peter554/pysubclasses/actions/workflows/ci.yml)

A Rust CLI tool and library for finding all subclasses (direct and transitive) of a Python class within a codebase.

## Features

- **Transitive subclass search**: Finds all descendants, not just direct children
- **Import resolution**: Handles various import styles including relative imports
- **Re-export support**: Tracks classes defined in one module but exported from another
- **Ambiguity detection**: Detects when a class name appears in multiple modules and provides clear guidance
- **Gitignore support**: Automatically respects `.gitignore` files using the `ignore` crate
- **Multiple output formats**: Text and JSON output formats
- **Fast and efficient**: Written in Rust with parallel file traversal

## Installation

### From Source

```bash
git clone <repository-url>
cd pysubclasses
cargo build --release
```

The binary will be available at `target/release/pysubclasses`.

To install globally:

```bash
cargo install --path .
```

## Usage

### Basic Usage

Find all subclasses of a class:

```bash
pysubclasses Animal
```

### With Module Disambiguation

When the same class name appears in multiple modules, specify the module path:

```bash
pysubclasses Animal --module zoo.animals
```

The `--module` flag also works with re-exporting modules. For example, if `Node` is defined in `pkg._internal` but re-exported through `pkg/__init__.py`, both of these will work:

```bash
pysubclasses Node --module pkg._internal
pysubclasses Node --module pkg  # Shorter, using the re-export
```

### Search in Specific Directory

Search in a different directory than the current one:

```bash
pysubclasses Animal --directory /path/to/project
```

### JSON Output

Get results in JSON format for scripting:

```bash
pysubclasses Animal --format json
```

### Verbose Mode

Show additional information about the search process:

```bash
pysubclasses Animal --verbose
```

## Examples

### Example 1: Simple Inheritance

```python
# animals.py
class Animal:
    pass

class Mammal(Animal):
    pass

class Dog(Mammal):
    pass
```

```bash
$ pysubclasses Animal
Found 2 subclass(es) of 'Animal':

  Mammal (animals)
  Dog (animals)
```

### Example 2: Ambiguous Class Names

```python
# zoo.py
class Animal:
    pass

# farm.py
class Animal:
    pass
```

```bash
$ pysubclasses Animal
Error: Class 'Animal' found in multiple modules. Please specify --module to disambiguate:
  - zoo
  - farm

$ pysubclasses Animal --module zoo
Found 0 subclass(es) of 'Animal'
```

### Example 3: Imports and Re-exports

```python
# animals/_internal.py
class Animal:
    pass

# animals/__init__.py
from animals._internal import Animal

# zoo.py
from animals import Animal

class Dog(Animal):
    pass
```

```bash
$ pysubclasses Animal
Found 1 subclass(es) of 'Animal':

  Dog (zoo)
```

### Example 4: JSON Output

```bash
$ pysubclasses Animal --format json
{
  "class_name": "Animal",
  "module_path": null,
  "subclasses": [
    {
      "class_name": "Dog",
      "module_path": "animals",
      "file_path": "/path/to/animals.py"
    },
    {
      "class_name": "Cat",
      "module_path": "animals",
      "file_path": "/path/to/animals.py"
    }
  ]
}
```

## Library Usage

You can also use `pysubclasses` as a library in your Rust projects:

```rust
use pysubclasses::SubclassFinder;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let finder = SubclassFinder::new(PathBuf::from("./src"))?;
    let subclasses = finder.find_subclasses("BaseClass", None)?;

    for class_ref in subclasses {
        println!("{} ({})", class_ref.class_name, class_ref.module_path);
    }

    Ok(())
}
```

## How It Works

1. **File Discovery**: Walks the directory tree and finds all `.py` files, respecting `.gitignore`
2. **Parsing**: Parses each Python file using the ruff parser to extract class definitions and imports
3. **Registry Construction**: Builds a registry of all classes with their base classes
4. **Import Resolution**: Resolves base class references to actual class definitions using import statements
5. **Graph Building**: Constructs an inheritance graph mapping parents to children
6. **Traversal**: Performs a breadth-first search to find all transitive subclasses

## Supported Python Features

- ✅ Simple inheritance: `class Dog(Animal)`
- ✅ Multiple inheritance: `class Dog(Animal, Pet)`
- ✅ Attribute-based inheritance: `class Dog(module.Animal)`
- ✅ Import statements: `import foo`, `from foo import Bar`
- ✅ Import aliases: `from foo import Bar as Baz`
- ✅ Relative imports: `from .module import Class`
- ✅ Re-exports via `__init__.py`
- ✅ Generic classes: `class Foo(Generic[T])`

## Limitations

- ❌ Dynamic imports: `importlib.import_module()` cannot be statically resolved
- ❌ Star imports: `from foo import *` requires parsing `__all__`
- ❌ Conditional imports: Only parses first branch of conditionals
- ❌ Runtime-generated classes: Cannot detect classes created at runtime
- ❌ Forward references in strings: `class Foo("BaseClass")` not fully supported

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test integration_test
```

### Project Structure

```
src/
├── lib.rs          # Public API
├── main.rs         # CLI implementation
├── discovery.rs    # File discovery
├── parser.rs       # AST parsing
├── registry.rs     # Class registry
├── graph.rs        # Inheritance graph
└── error.rs        # Error types

tests/
└── integration_test.rs  # Integration tests
```

## Contributing

Contributions are welcome! Please ensure all tests pass before submitting a PR:

```bash
cargo test
cargo clippy
cargo fmt
```

## License

[Add your license here]

## Acknowledgments

- Uses the [ruff](https://github.com/astral-sh/ruff) Python parser
- Uses the [ignore](https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore) crate for efficient file traversal
