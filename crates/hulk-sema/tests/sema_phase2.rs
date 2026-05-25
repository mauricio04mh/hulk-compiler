use hulk_frontend::parse_hulk_types_program;
use hulk_sema::check_program;
use hulk_sema::context::TypeRegistry;
use hulk_sema::error::SemanticError;
use hulk_sema::types::Type;

fn parse(src: &str) -> hulk_frontend::ast::Program {
    parse_hulk_types_program(src).expect("source should parse")
}

fn check_ok(src: &str) {
    let program = parse(src);
    check_program(&program).expect("type check should pass");
}

fn check_err(src: &str) -> SemanticError {
    let program = parse(src);
    check_program(&program)
        .expect_err("type check should fail")
        .into_iter()
        .next()
        .expect("at least one error")
}

fn check_errors(src: &str) -> Vec<SemanticError> {
    let program = parse(src);
    check_program(&program).expect_err("type check should fail")
}

// ── Inheritance / subtyping ───────────────────────────────────────────────────

#[test]
fn subtype_assignable_to_parent() {
    // Dog inherits Animal — new Dog() should be valid where Animal is expected.
    check_ok(
        "type Animal {}\n\
         type Dog inherits Animal {}\n\
         let a: Animal = new Dog() in a;",
    );
}

#[test]
fn sibling_types_not_assignable() {
    let err = check_err(
        "type Animal {}\n\
         type Dog inherits Animal {}\n\
         type Cat inherits Animal {}\n\
         let d: Dog = new Cat() in d;",
    );
    assert!(matches!(err, SemanticError::TypeMismatch { .. }));
}

#[test]
fn subtype_as_function_arg() {
    check_ok(
        "type Shape {}\n\
         type Circle inherits Shape {}\n\
         function area(s: Shape): Shape => s;\n\
         area(new Circle());",
    );
}

#[test]
fn wrong_type_as_function_arg() {
    let err = check_err(
        "type Shape {}\n\
         type Color {}\n\
         function paint(s: Shape): Shape => s;\n\
         paint(new Color());",
    );
    assert!(matches!(err, SemanticError::InvalidArgumentType { .. }));
}

// ── Inheritance chain member/method lookup ────────────────────────────────────

#[test]
fn access_inherited_attribute() {
    // Per spec (A.7): attributes are always private, inaccessible from outside.
    let err = check_err(
        "type Animal(name: String) { name: String = name; }\n\
         type Dog inherits Animal(\"dog\") {}\n\
         let d = new Dog() in d.name;",
    );
    assert!(matches!(err, SemanticError::AttributeIsPrivate { .. }));
}

#[test]
fn call_inherited_method() {
    check_ok(
        "type Animal { speak(): String => \"...\"; }\n\
         type Dog inherits Animal() {}\n\
         let d = new Dog() in d.speak();",
    );
}

#[test]
fn overridden_method_uses_child_definition() {
    // Dog overrides speak() — checker should find the Dog version first.
    check_ok(
        "type Animal { speak(): String => \"generic\"; }\n\
         type Dog inherits Animal() { speak(): String => \"woof\"; }\n\
         new Dog().speak();",
    );
}

// ── if-branch unification (LCA) ───────────────────────────────────────────────

#[test]
fn if_branches_same_type() {
    check_ok("if (true) 1 else 2;");
}

#[test]
fn if_branches_sibling_types_unify_to_lca() {
    // Both branches produce subtypes — result should be the common ancestor (Animal).
    // The test just verifies no error is raised.
    check_ok(
        "type Animal {}\n\
         type Dog inherits Animal {}\n\
         type Cat inherits Animal {}\n\
         let x: Animal = if (true) new Dog() else new Cat() in x;",
    );
}

#[test]
fn if_branches_subtype_widens_to_parent() {
    check_ok(
        "type Animal {}\n\
         type Dog inherits Animal {}\n\
         let x: Animal = if (true) new Dog() else new Animal() in x;",
    );
}

// ── is_descendant_of ─────────────────────────────────────────────────────────

#[test]
fn registry_is_descendant_direct() {
    let program = parse("type A {}\ntype B inherits A {}\n42;");
    let registry = TypeRegistry::build(&program).unwrap();
    assert!(registry.is_descendant_of("B", "A"));
    assert!(registry.is_descendant_of("A", "A")); // reflexive
    assert!(!registry.is_descendant_of("A", "B")); // not reverse
}

#[test]
fn registry_is_descendant_transitive() {
    let program = parse("type A {}\ntype B inherits A {}\ntype C inherits B {}\n42;");
    let registry = TypeRegistry::build(&program).unwrap();
    assert!(registry.is_descendant_of("C", "A")); // transitive
    assert!(registry.is_descendant_of("C", "B"));
    assert!(!registry.is_descendant_of("A", "C"));
}

// ── lookup_attribute / lookup_method_info ─────────────────────────────────────

#[test]
fn registry_lookup_attribute_inherited() {
    let program = parse(
        "type Animal(name: String) { name: String = name; }\ntype Dog inherits Animal(\"x\") {}\n42;",
    );
    let registry = TypeRegistry::build(&program).unwrap();
    let ty = registry.lookup_attribute("Dog", "name");
    assert_eq!(ty, Some(Type::String));
}

#[test]
fn registry_lookup_method_inherited() {
    let program =
        parse("type Animal { speak(): String => \"...\"; }\ntype Dog inherits Animal() {}\n42;");
    let registry = TypeRegistry::build(&program).unwrap();
    let mi = registry.lookup_method_info("Dog", "speak");
    assert!(mi.is_some());
    assert_eq!(mi.unwrap().return_type, Type::String);
}

// ── least_common_ancestor ─────────────────────────────────────────────────────

#[test]
fn registry_lca_siblings() {
    let program =
        parse("type Animal {}\ntype Dog inherits Animal {}\ntype Cat inherits Animal {}\n42;");
    let registry = TypeRegistry::build(&program).unwrap();
    let lca = registry.least_common_ancestor("Dog", "Cat");
    assert_eq!(lca, Type::UserType("Animal".to_string()));
}

#[test]
fn registry_lca_same_type() {
    let program = parse("type Dog {}\n42;");
    let registry = TypeRegistry::build(&program).unwrap();
    let lca = registry.least_common_ancestor("Dog", "Dog");
    assert_eq!(lca, Type::UserType("Dog".to_string()));
}

#[test]
fn registry_lca_no_common_ancestor_returns_object() {
    let program = parse("type A {}\ntype B {}\n42;");
    let registry = TypeRegistry::build(&program).unwrap();
    let lca = registry.least_common_ancestor("A", "B");
    assert_eq!(lca, Type::Object);
}

// ── Protocol conformance ──────────────────────────────────────────────────────

#[test]
fn protocol_conformance_valid() {
    check_ok(
        "protocol Printable { toString(): String; }\n\
         type Dog inherits Printable { toString(): String => \"dog\"; }\n\
         42;",
    );
}

#[test]
fn protocol_missing_method_error() {
    let program = parse(
        "protocol Printable { toString(): String; }\n\
         type Dog inherits Printable {}\n\
         42;",
    );
    let err = TypeRegistry::build(&program).expect_err("should fail");
    assert!(matches!(err, SemanticError::MissingProtocolMethod { .. }));
}

#[test]
fn protocol_wrong_arity_error() {
    let program = parse(
        "protocol Printable { format(x: Number): String; }\n\
         type Dog inherits Printable { format(): String => \"dog\"; }\n\
         42;",
    );
    let err = TypeRegistry::build(&program).expect_err("should fail");
    assert!(matches!(
        err,
        SemanticError::ProtocolMethodSignatureMismatch { .. }
    ));
}

// ── base() call validation ────────────────────────────────────────────────────

#[test]
fn base_call_valid_in_child_type() {
    check_ok(
        "type Animal(name: String) { name: String = name; }\n\
         type Dog inherits Animal(\"dog\") {}\n\
         new Dog();",
    );
}

#[test]
fn base_call_arity_mismatch() {
    // base() called with wrong number of args relative to parent ctor.
    let err = check_err(
        "type Animal(name: String) {}\n\
         type Dog inherits Animal(\"x\", \"extra\") {}\n\
         new Dog();",
    );
    assert!(matches!(err, SemanticError::ArityMismatch { .. }));
}

// ── Self type resolution ──────────────────────────────────────────────────────

#[test]
fn self_returns_current_type() {
    // self inside a method should have the enclosing type.
    check_ok(
        "type Counter(n: Number) {\n\
             value: Number = n;\n\
             get(): Counter => self;\n\
         }\n\
         new Counter(0).get();",
    );
}

// ── Multi-error accumulation ──────────────────────────────────────────────────

#[test]
fn multiple_errors_reported() {
    // Two independent type errors — both should be reported.
    let errors = check_errors(
        "function bad1(x: Number): Number => \"wrong\";\n\
         function bad2(y: Number): Number => true;\n\
         bad1(0);",
    );
    assert!(
        errors.len() >= 2,
        "expected at least 2 errors, got {}",
        errors.len()
    );
}

#[test]
fn multiple_type_member_errors() {
    let errors = check_errors(
        "type Broken(n: Number) {\n\
             a: Number = \"bad\";\n\
             b: Number = true;\n\
         }\n\
         42;",
    );
    assert!(
        errors.len() >= 2,
        "expected at least 2 errors, got {}",
        errors.len()
    );
}

#[test]
fn unknown_propagates_without_cascading() {
    // An undefined variable should produce exactly one error, not many cascading errors.
    let errors = check_errors("undefined_var + 1;");
    assert_eq!(
        errors.len(),
        1,
        "expected exactly 1 error, got {:?}",
        errors
    );
}

// ── Vector element type propagation ──────────────────────────────────────────

#[test]
fn vector_of_subtypes_is_valid() {
    check_ok(
        "type Animal {}\n\
         type Dog inherits Animal {}\n\
         let v = [new Dog(), new Dog()] in v;",
    );
}

#[test]
fn for_loop_over_range_gives_number() {
    check_ok("let sum = 0 in for (i in range(0, 5)) sum := sum + i;");
}

// ── TypeCast return type ──────────────────────────────────────────────────────

#[test]
fn typecast_returns_target_user_type() {
    check_ok(
        "type Animal {}\n\
         type Dog inherits Animal {}\n\
         let a: Object = new Dog() in\n\
         let d: Dog = (a as Dog) in d;",
    );
}
