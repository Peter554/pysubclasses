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
        .stderr(predicate::str::contains("multiple modules"))
        .stderr(predicate::str::contains(
            "Please specify --module to disambiguate",
        ));

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
        .env("RUST_LOG", "debug")
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
        .write_str(
            "class Node:
    pass
",
        )
        .unwrap();

    // Re-export at package level
    temp.child("pkg/__init__.py")
        .write_str(
            "from ._internal.base import Node
",
        )
        .unwrap();

    // Subclass using the re-exported name
    temp.child("pkg/custom.py")
        .write_str(
            "from . import Node

class CustomNode(Node):
    pass
",
        )
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

#[test]
fn test_generic_classes() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Create base class that uses Generic
    temp.child("base.py")
        .write_str(
            r#"
from typing import Generic, TypeVar

T = TypeVar('T')

class Container(Generic[T]):
    pass
"#,
        )
        .unwrap();

    // Create derived class that inherits from Generic base
    temp.child("derived.py")
        .write_str(
            r#"
from base import Container

class StringContainer(Container[str]):
    pass

class IntContainer(Container[int]):
    pass
"#,
        )
        .unwrap();

    // Should find both StringContainer and IntContainer as subclasses of Container
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Container")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("StringContainer"))
        .stdout(predicate::str::contains("IntContainer"))
        .stdout(predicate::str::contains("derived"));

    temp.close().unwrap();
}

#[test]
fn test_generic_classes_python312_syntax() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Create base class using Python 3.12+ generic syntax
    temp.child("base.py")
        .write_str(
            r#"
class Container[T]:
    pass
"#,
        )
        .unwrap();

    // Create derived classes that inherit from the generic base
    temp.child("derived.py")
        .write_str(
            r#"
from base import Container

class StringContainer(Container[str]):
    pass

class IntContainer(Container[int]):
    pass
"#,
        )
        .unwrap();

    // Should find both StringContainer and IntContainer as subclasses of Container
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Container")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("StringContainer"))
        .stdout(predicate::str::contains("IntContainer"))
        .stdout(predicate::str::contains("derived"));

    temp.close().unwrap();
}

#[test]
fn test_nested_classes() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Create base class and nested subclass
    temp.child("classes.py")
        .write_str(
            r#"
class Foo:
    pass

class Bar:
    class SomeFoo(Foo):
        pass

class Baz:
    class AnotherFoo(Foo):
        pass
"#,
        )
        .unwrap();

    // Should find both nested subclasses
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Foo")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Bar.SomeFoo"))
        .stdout(predicate::str::contains("Baz.AnotherFoo"))
        .stdout(predicate::str::contains("classes"));

    temp.close().unwrap();
}

#[test]
fn test_reexport_of_module_import_from() {
    let temp = assert_fs::TempDir::new().unwrap();

    temp.child("testpkg/a.py")
        .write_str(
            r#"
class A: ...
"#,
        )
        .unwrap();

    temp.child("testpkg/b.py")
        .write_str(
            r#"
from testpkg import a

class B(a.A): ...
"#,
        )
        .unwrap();

    temp.child("testpkg/c.py")
        .write_str(
            r#"
from testpkg import b

class C(b.a.A): ...
"#,
        )
        .unwrap();

    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("A")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 2 subclass(es) of 'A'"))
        .stdout(predicate::str::contains("B"))
        .stdout(predicate::str::contains("C"));

    temp.close().unwrap();
}

#[test]
fn test_reexport_of_module_import() {
    let temp = assert_fs::TempDir::new().unwrap();

    temp.child("testpkg/a.py")
        .write_str(
            r#"
class A: ...
"#,
        )
        .unwrap();

    temp.child("testpkg/b.py")
        .write_str(
            r#"
import testpkg.a

class B(testpkg.a.A): ...
"#,
        )
        .unwrap();

    temp.child("testpkg/c.py")
        .write_str(
            r#"
import testpkg.b

class C(testpkg.b.testpkg.a.A): ...
"#,
        )
        .unwrap();

    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("A")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 2 subclass(es) of 'A'"))
        .stdout(predicate::str::contains("B"))
        .stdout(predicate::str::contains("C"));

    temp.close().unwrap();
}

#[test]
fn test_reexport_not_via_init() {
    let temp = assert_fs::TempDir::new().unwrap();

    temp.child("testpkg/a.py")
        .write_str(
            r#"
class A: ...
"#,
        )
        .unwrap();

    temp.child("testpkg/b.py")
        .write_str(
            r#"
from testpkg.a import A

class B(A): ...
"#,
        )
        .unwrap();

    temp.child("testpkg/c.py")
        .write_str(
            r#"
from testpkg.b import A

class C(A): ...
"#,
        )
        .unwrap();

    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("A")
        .arg("--directory")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 2 subclass(es) of 'A'"))
        .stdout(predicate::str::contains("B"))
        .stdout(predicate::str::contains("C"));

    temp.close().unwrap();
}

#[test]
fn test_reexport_not_via_init_pass_reexport_module() {
    let temp = assert_fs::TempDir::new().unwrap();

    temp.child("testpkg/a.py")
        .write_str(
            r#"
class A: ...
"#,
        )
        .unwrap();

    temp.child("testpkg/b.py")
        .write_str(
            r#"
from testpkg.a import A

class B(A): ...
"#,
        )
        .unwrap();

    temp.child("testpkg/c.py")
        .write_str(
            r#"
from testpkg.b import A

class C(A): ...
"#,
        )
        .unwrap();

    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("A")
        .arg("--directory")
        .arg(temp.path())
        .arg("--module")
        .arg("testpkg.b")
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 2 subclass(es) of 'A'"))
        .stdout(predicate::str::contains("B"))
        .stdout(predicate::str::contains("C"));

    temp.close().unwrap();
}

#[test]
fn test_direct_mode() {
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

class Cat(Mammal):
    pass
"#,
        )
        .unwrap();

    // Test direct mode - should only get Mammal
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--directory")
        .arg(temp.path())
        .arg("--mode")
        .arg("direct")
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 1 subclass(es) of 'Animal'"))
        .stdout(predicate::str::contains("Mammal"))
        .stdout(predicate::str::contains("Dog").not())
        .stdout(predicate::str::contains("Cat").not());

    // Test all mode (default) - should get Mammal, Dog, and Cat
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Animal")
        .arg("--directory")
        .arg(temp.path())
        .arg("--mode")
        .arg("all")
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 3 subclass(es) of 'Animal'"))
        .stdout(predicate::str::contains("Mammal"))
        .stdout(predicate::str::contains("Dog"))
        .stdout(predicate::str::contains("Cat"));

    // Test direct mode on Mammal - should get Dog and Cat
    let mut cmd = Command::cargo_bin("pysubclasses").unwrap();
    cmd.arg("Mammal")
        .arg("--directory")
        .arg(temp.path())
        .arg("--mode")
        .arg("direct")
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 2 subclass(es) of 'Mammal'"))
        .stdout(predicate::str::contains("Dog"))
        .stdout(predicate::str::contains("Cat"));

    temp.close().unwrap();
}

#[test]
fn test_find_parent_classes_direct() {
    use pysubclasses::{SearchMode, SubclassFinder};

    let temp = assert_fs::TempDir::new().unwrap();

    // Create inheritance hierarchy: Animal -> Mammal -> Dog
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

    let finder = SubclassFinder::new(temp.path().to_path_buf()).unwrap();

    // Dog's direct parent should be Mammal
    let parents = finder
        .find_parent_classes("Dog", Some("animals"), SearchMode::Direct)
        .unwrap();
    assert_eq!(parents.len(), 1);
    assert_eq!(parents[0].class_name, "Mammal");
    assert_eq!(parents[0].module_path, "animals");

    // Mammal's direct parent should be Animal
    let parents = finder
        .find_parent_classes("Mammal", Some("animals"), SearchMode::Direct)
        .unwrap();
    assert_eq!(parents.len(), 1);
    assert_eq!(parents[0].class_name, "Animal");
    assert_eq!(parents[0].module_path, "animals");

    // Animal should have no parents
    let parents = finder
        .find_parent_classes("Animal", Some("animals"), SearchMode::Direct)
        .unwrap();
    assert_eq!(parents.len(), 0);

    temp.close().unwrap();
}

#[test]
fn test_find_parent_classes_all() {
    use pysubclasses::{SearchMode, SubclassFinder};

    let temp = assert_fs::TempDir::new().unwrap();

    // Create inheritance hierarchy: Animal -> Mammal -> Dog
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

    let finder = SubclassFinder::new(temp.path().to_path_buf()).unwrap();

    // Dog's all parents should be Mammal and Animal
    let parents = finder
        .find_parent_classes("Dog", Some("animals"), SearchMode::All)
        .unwrap();
    assert_eq!(parents.len(), 2);

    let parent_names: Vec<_> = parents.iter().map(|p| p.class_name.as_str()).collect();
    assert!(parent_names.contains(&"Mammal"));
    assert!(parent_names.contains(&"Animal"));
    assert!(parents.iter().all(|p| p.module_path == "animals"));

    // Mammal's all parents should be just Animal
    let parents = finder
        .find_parent_classes("Mammal", Some("animals"), SearchMode::All)
        .unwrap();
    assert_eq!(parents.len(), 1);
    assert_eq!(parents[0].class_name, "Animal");
    assert_eq!(parents[0].module_path, "animals");

    temp.close().unwrap();
}

#[test]
fn test_find_parent_classes_multiple_inheritance() {
    use pysubclasses::{SearchMode, SubclassFinder};

    let temp = assert_fs::TempDir::new().unwrap();

    // Create multiple inheritance
    temp.child("animals.py")
        .write_str(
            r#"
class Animal:
    pass

class Walker:
    pass

class Dog(Animal, Walker):
    pass
"#,
        )
        .unwrap();

    let finder = SubclassFinder::new(temp.path().to_path_buf()).unwrap();

    // Dog should have both Animal and Walker as direct parents
    let parents = finder
        .find_parent_classes("Dog", Some("animals"), SearchMode::Direct)
        .unwrap();
    assert_eq!(parents.len(), 2);

    let parent_names: Vec<_> = parents.iter().map(|p| p.class_name.as_str()).collect();
    assert!(parent_names.contains(&"Animal"));
    assert!(parent_names.contains(&"Walker"));

    temp.close().unwrap();
}
