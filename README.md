# pysubclasses

[![CI](https://github.com/Peter554/pysubclasses/actions/workflows/ci.yml/badge.svg)](https://github.com/Peter554/pysubclasses/actions/workflows/ci.yml)

A Rust CLI tool and library for finding all subclasses (direct and transitive) of a Python class within a codebase.

## Features

- **Transitive subclass search**: Finds all descendants, not just direct children
- **Import resolution**: Handles various import styles including relative imports
- **Re-export support**: Tracks classes defined in one module but exported from another
- **Ambiguity detection**: Detects when a class name appears in multiple modules and provides clear guidance
- **Gitignore support**: Automatically respects `.gitignore` files using the `ignore` crate
- **Multiple output formats**: Text, JSON, and Graphviz dot formats for visualization
- **Fast and efficient**: Written in Rust with parallel file traversal
- **Smart caching**: Caches parsed files with gzip compression for 2.5x speedup on repeated runs
- **Configurable logging**: Uses `env_logger` for flexible logging control

## Installation

### Via Homebrew

```bash
brew install peter554/tap/pysubclasses
```

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

### Graphviz Dot Format

Generate a graph visualization in Graphviz dot format:

```bash
# Output dot format
pysubclasses Animal --format dot

# Pipe to dot to generate an image
pysubclasses Animal --format dot | dot -Tpng > graph.png
pysubclasses Animal --format dot | dot -Tsvg > graph.svg
```

### Search Mode

Control whether to find only direct subclasses or all transitive subclasses:

```bash
# Find all transitive subclasses (default)
pysubclasses Animal --mode all

# Find only direct subclasses
pysubclasses Animal --mode direct
```

For example, given:
```python
class Animal: pass
class Mammal(Animal): pass
class Dog(Mammal): pass
```

- `--mode all` (default) would find: Mammal, Dog
- `--mode direct` would find: Mammal (only)

### Logging

Control logging verbosity using the `RUST_LOG` environment variable:

```bash
# No logging (default) - only shows output
pysubclasses Animal

# Show info level logs
RUST_LOG=info pysubclasses Animal

# Show debug level logs
RUST_LOG=debug pysubclasses Animal

# Show only pysubclasses logs (filter out dependencies)
RUST_LOG=pysubclasses=debug pysubclasses Animal
```

### Exclude Directories

Exclude specific directories from analysis:

```bash
pysubclasses Animal --exclude ./tests
```

### Disable Cache

Force re-parsing of all files:

```bash
pysubclasses Animal --no-cache
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
- ❌ Runtime-generated classes: Cannot detect classes created at runtime
- ❌ Forward references in strings: `class Foo("BaseClass")` not fully supported
