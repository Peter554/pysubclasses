use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn test_simple_inheritance() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Create base class
    temp.child("base.py")
        .write_str("class Animal:\n    pass\n")
        .unwrap();

    // Create derived class
    temp.child("derived.py")
        .write_str("from base import Animal\n\nclass Dog(Animal):\n    pass\n")
        .unwrap();

    // Run the CLI
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Dog"))
        .stdout(predicate::str::contains("derived"));

    temp.close().unwrap();
}

#[test]
fn test_transitive_inheritance() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Animal -> Mammal -> Dog
    temp.child("animals.py")
        .write_str(
            r#"
class Animal:
    pass

class Mammal(Animal):
    pass

class Dog(Mammal):
    pass
"#,
        )
        .unwrap();

    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Mammal"))
        .stdout(predicate::str::contains("Dog"));

    temp.close().unwrap();
}

#[test]
fn test_no_subclasses() {
    let temp = assert_fs::TempDir::new().unwrap();

    temp.child("base.py")
        .write_str("class Animal:\n    pass\n")
        .unwrap();

    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No subclasses found"));

    temp.close().unwrap();
}

#[test]
fn test_class_not_found() {
    let temp = assert_fs::TempDir::new().unwrap();

    temp.child("base.py")
        .write_str("class Animal:\n    pass\n")
        .unwrap();

    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("NonExistent")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));

    temp.close().unwrap();
}

#[test]
fn test_ambiguous_class_name() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Create two files with the same class name
    temp.child("zoo.py")
        .write_str("class Animal:\n    pass\n")
        .unwrap();

    temp.child("farm.py")
        .write_str("class Animal:\n    pass\n")
        .unwrap();

    // Without module path, should error
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("multiple modules"));

    // With module path, should succeed
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--module")
        .arg("zoo")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success();

    temp.close().unwrap();
}

#[test]
fn test_json_output() {
    let temp = assert_fs::TempDir::new().unwrap();

    temp.child("base.py")
        .write_str("class Animal:\n    pass\n")
        .unwrap();

    temp.child("derived.py")
        .write_str("from base import Animal\n\nclass Dog(Animal):\n    pass\n")
        .unwrap();

    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--directory")
        .arg(temp.path())
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"class_name\": \"Dog\""))
        .stdout(predicate::str::contains("\"module_path\": \"derived\""));

    temp.close().unwrap();
}

#[test]
fn test_verbose_output() {
    let temp = assert_fs::TempDir::new().unwrap();

    temp.child("base.py")
        .write_str("class Animal:\n    pass\n")
        .unwrap();

    temp.child("derived.py")
        .write_str("from base import Animal\n\nclass Dog(Animal):\n    pass\n")
        .unwrap();

    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--directory")
        .arg(temp.path())
        .arg("--verbose")
        .assert()
        .success()
        .stderr(predicate::str::contains("Searching for Python files"))
        .stderr(predicate::str::contains("Found"))
        .stderr(predicate::str::contains("classes in codebase"));

    temp.close().unwrap();
}

#[test]
fn test_multiple_inheritance() {
    let temp = assert_fs::TempDir::new().unwrap();

    temp.child("base.py")
        .write_str(
            r#"
class Animal:
    pass

class Pet:
    pass

class Dog(Animal, Pet):
    pass
"#,
        )
        .unwrap();

    // Dog should be a subclass of Animal
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Dog"));

    // Dog should also be a subclass of Pet
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Pet")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Dog"));

    temp.close().unwrap();
}

#[test]
fn test_package_with_init() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Create package structure
    temp.child("animals/__init__.py")
        .write_str("from animals.base import Animal\n")
        .unwrap();

    temp.child("animals/base.py")
        .write_str("class Animal:\n    pass\n")
        .unwrap();

    temp.child("animals/dog.py")
        .write_str("from animals.base import Animal\n\nclass Dog(Animal):\n    pass\n")
        .unwrap();

    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--module")
        .arg("animals.base")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Dog"));

    temp.close().unwrap();
}

#[test]
fn test_relative_imports() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Create package with relative imports
    temp.child("pkg/__init__.py").write_str("").unwrap();

    temp.child("pkg/base.py")
        .write_str("class Animal:\n    pass\n")
        .unwrap();

    temp.child("pkg/derived.py")
        .write_str("from .base import Animal\n\nclass Dog(Animal):\n    pass\n")
        .unwrap();

    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Dog"));

    temp.close().unwrap();
}

#[test]
fn test_help_message() {
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("finds all subclasses"))
        .stdout(predicate::str::contains("--module"))
        .stdout(predicate::str::contains("--directory"));
}

#[test]
fn test_reexport_single_level() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Create package structure with re-export
    temp.child("mypackage/base.py")
        .write_str("class Animal:\n    pass\n")
        .unwrap();

    temp.child("mypackage/__init__.py")
        .write_str("from .base import Animal\n")
        .unwrap();

    temp.child("mypackage/dog.py")
        .write_str("from . import Animal\n\nclass Dog(Animal):\n    pass\n")
        .unwrap();

    // Should find Dog as subclass of Animal
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Dog"));

    temp.close().unwrap();
}

#[test]
fn test_reexport_multi_level() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Create nested package structure with transitive re-exports
    temp.child("pkg/_nodes/_base.py")
        .write_str("class Node:\n    pass\n")
        .unwrap();

    temp.child("pkg/_nodes/__init__.py")
        .write_str("from ._base import Node\n")
        .unwrap();

    temp.child("pkg/__init__.py")
        .write_str("from ._nodes import Node\n")
        .unwrap();

    temp.child("pkg/custom.py")
        .write_str("from . import Node\n\nclass CustomNode(Node):\n    pass\n")
        .unwrap();

    // Should find CustomNode as subclass of Node through multi-level re-exports
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Node")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("CustomNode"));

    temp.close().unwrap();
}

#[test]
fn test_complex_relative_imports() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Base class deep in package
    temp.child("company/domain/core/base.py")
        .write_str("class Entity:\n    pass\n")
        .unwrap();

    // Re-export at core level
    temp.child("company/domain/core/__init__.py")
        .write_str("from .base import Entity\n")
        .unwrap();

    // Subclass using ../../ relative import
    temp.child("company/app/models/user.py")
        .write_str("from ...domain.core import Entity\n\nclass User(Entity):\n    pass\n")
        .unwrap();

    // Should find User as subclass of Entity
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Entity")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("User"));

    temp.close().unwrap();
}

#[test]
fn test_reexport_module_path() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Base class in internal module
    temp.child("pkg/_internal/base.py")
        .write_str("class Node:
    pass
")
        .unwrap();

    // Re-export at package level
    temp.child("pkg/__init__.py")
        .write_str("from ._internal.base import Node
")
        .unwrap();

    // Subclass using the re-exported name
    temp.child("pkg/custom.py")
        .write_str("from . import Node

class CustomNode(Node):
    pass
")
        .unwrap();

    // Should work with the actual module path
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Node")
        .arg("--module")
        .arg("pkg._internal.base")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("CustomNode"));

    // Should also work with the re-exporting module path
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Node")
        .arg("--module")
        .arg("pkg")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("CustomNode"));

    temp.close().unwrap();
}
