use hulk_frontend::parse_hulk_types_program;
use hulk_sema::check_program;
use hulk_sema::context::TypeRegistry;
use hulk_sema::error::SemanticError;

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

// ── Attribute privacy (G1) ────────────────────────────────────────────────────

#[test]
fn attr_access_from_outside_fails() {
    let err = check_err(
        "type Counter(n: Number) { value: Number = n; }\n\
         let c = new Counter(0) in c.value;",
    );
    assert!(matches!(err, SemanticError::AttributeIsPrivate { .. }));
}

#[test]
fn attr_access_via_self_ok() {
    check_ok(
        "type Counter(n: Number) {\n\
             value: Number = n;\n\
             get(): Number => self.value;\n\
         }\n\
         new Counter(0).get();",
    );
}

#[test]
fn attr_access_in_child_via_self_parent_attr_fails() {
    // Per spec (A.7): attributes are private to the declaring type — even self cannot
    // reach a parent's attribute.
    let err = check_err(
        "type Animal(name: String) { name: String = name; }\n\
         type Dog inherits Animal(\"dog\") {\n\
             getName(): String => self.name;\n\
         }\n\
         42;",
    );
    assert!(matches!(err, SemanticError::AttributeIsPrivate { .. }));
}

// ── Primitive inheritance guard (G2) ──────────────────────────────────────────

#[test]
fn cannot_inherit_from_number() {
    let program = parse("type MyNum inherits Number {}\n42;");
    let err = TypeRegistry::build(&program).expect_err("should fail");
    assert!(matches!(err, SemanticError::CannotInheritFromPrimitive { .. }));
}

#[test]
fn cannot_inherit_from_string() {
    let program = parse("type MyStr inherits String {}\n42;");
    let err = TypeRegistry::build(&program).expect_err("should fail");
    assert!(matches!(err, SemanticError::CannotInheritFromPrimitive { .. }));
}

#[test]
fn cannot_inherit_from_boolean() {
    let program = parse("type MyBool inherits Boolean {}\n42;");
    let err = TypeRegistry::build(&program).expect_err("should fail");
    assert!(matches!(err, SemanticError::CannotInheritFromPrimitive { .. }));
}

// ── Protocol variance checks (G3) ─────────────────────────────────────────────

#[test]
fn protocol_return_type_mismatch() {
    let program = parse(
        "protocol Printable { toString(): String; }\n\
         type Dog inherits Printable { toString(): Number => 42; }\n\
         42;",
    );
    let err = TypeRegistry::build(&program).expect_err("should fail");
    assert!(matches!(err, SemanticError::ProtocolReturnTypeMismatch { .. }));
}

#[test]
fn protocol_param_type_mismatch() {
    let program = parse(
        "protocol Formatter { format(x: Number): String; }\n\
         type Dog inherits Formatter { format(x: String): String => x; }\n\
         42;",
    );
    let err = TypeRegistry::build(&program).expect_err("should fail");
    assert!(matches!(err, SemanticError::ProtocolParamTypeMismatch { .. }));
}

#[test]
fn protocol_full_signature_valid() {
    // Covariant return: Number <= Object is fine.
    check_ok(
        "protocol Sizeable { getSize(): Object; }\n\
         type Box inherits Sizeable { getSize(): Number => 42; }\n\
         42;",
    );
}

// ── Method override signature (G4) ────────────────────────────────────────────

#[test]
fn method_override_arity_mismatch() {
    let program = parse(
        "type Animal { speak(): String => \"...\"; }\n\
         type Dog inherits Animal() { speak(x: Number): String => \"woof\"; }\n\
         42;",
    );
    let err = TypeRegistry::build(&program).expect_err("should fail");
    assert!(matches!(err, SemanticError::MethodOverrideSignatureMismatch { .. }));
}

#[test]
fn method_override_return_type_mismatch() {
    let program = parse(
        "type Animal { speak(): String => \"...\"; }\n\
         type Dog inherits Animal() { speak(): Number => 42; }\n\
         42;",
    );
    let err = TypeRegistry::build(&program).expect_err("should fail");
    assert!(matches!(err, SemanticError::MethodOverrideSignatureMismatch { .. }));
}

#[test]
fn method_override_valid() {
    check_ok(
        "type Animal { speak(): String => \"generic\"; }\n\
         type Dog inherits Animal() { speak(): String => \"woof\"; }\n\
         42;",
    );
}

// ── base() calls parent method (G5) ───────────────────────────────────────────

#[test]
fn base_in_method_calls_parent_method() {
    check_ok(
        "type Animal { speak(): String => \"...\"; }\n\
         type Dog inherits Animal() { speak(): String => base(); }\n\
         new Dog().speak();",
    );
}

#[test]
fn base_in_method_arity_mismatch() {
    let err = check_err(
        "type Animal { speak(): String => \"...\"; }\n\
         type Dog inherits Animal() { speak(): String => base(\"extra\"); }\n\
         new Dog().speak();",
    );
    assert!(matches!(err, SemanticError::ArityMismatch { .. }));
}

// ── Implicit protocol conformance (G6) ────────────────────────────────────────

#[test]
fn implicit_protocol_conformance_valid() {
    check_ok("let x: Iterable = range(0, 10) in x;");
}

#[test]
fn implicit_protocol_non_conformance() {
    let err = check_err(
        "protocol Walkable { walk(): Boolean; }\n\
         type Rock {}\n\
         let r: Walkable = new Rock() in r;",
    );
    assert!(matches!(err, SemanticError::MissingProtocolMethod { .. }));
}
