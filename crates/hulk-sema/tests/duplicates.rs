use hulk_frontend::parse_hulk_types_program;
use hulk_sema::check_program;
use hulk_sema::context::TypeRegistry;
use hulk_sema::error::SemanticError;

fn parse(src: &str) -> hulk_frontend::ast::Program {
    parse_hulk_types_program(src).expect("source should parse")
}

fn registry_err(src: &str) -> SemanticError {
    let program = parse(src);
    TypeRegistry::build(&program).expect_err("registry build should fail")
}

fn check_ok(src: &str) {
    let program = parse(src);
    check_program(&program).expect("type check should pass");
}

#[test]
fn duplicate_attribute_in_type_fails() {
    let err = registry_err(
        "type A {
            x: Number = 1;
            x: String = \"bad\";
         }
         42;",
    );
    assert!(matches!(err, SemanticError::DuplicateAttribute { .. }));
}

#[test]
fn duplicate_method_in_type_fails() {
    let err = registry_err(
        "type A {
            f(): Number => 1;
            f(): String => \"bad\";
         }
         42;",
    );
    assert!(matches!(err, SemanticError::DuplicateMethod { .. }));
}

#[test]
fn attribute_and_method_same_name_fails() {
    let err = registry_err(
        "type A {
            f: Number = 1;
            f(): Number => 2;
         }
         42;",
    );
    assert!(matches!(
        err,
        SemanticError::DuplicateAttribute { .. }
            | SemanticError::DuplicateMethod { .. }
            | SemanticError::DuplicateSymbol { .. }
    ));
}

#[test]
fn method_and_attribute_same_name_fails() {
    let err = registry_err(
        "type A {
            f(): Number => 2;
            f: Number = 1;
         }
         42;",
    );
    assert!(matches!(
        err,
        SemanticError::DuplicateAttribute { .. }
            | SemanticError::DuplicateMethod { .. }
            | SemanticError::DuplicateSymbol { .. }
    ));
}

#[test]
fn duplicate_protocol_method_fails() {
    let err = registry_err(
        "protocol P {
            f(): Number;
            f(): String;
         }
         42;",
    );
    assert!(matches!(err, SemanticError::DuplicateProtocolMethod { .. }));
}

#[test]
fn type_and_protocol_same_name_fails() {
    let err = registry_err(
        "type A {}
         protocol A {}
         42;",
    );
    assert!(matches!(err, SemanticError::DuplicateType { .. }));
}

#[test]
fn protocol_and_type_same_name_fails() {
    let err = registry_err(
        "protocol A {}
         type A {}
         42;",
    );
    assert!(matches!(err, SemanticError::DuplicateType { .. }));
}

#[test]
fn same_attribute_name_in_different_types_ok() {
    check_ok(
        "type A { x: Number = 1; }
         type B { x: Number = 2; }
         42;",
    );
}

#[test]
fn same_method_name_in_type_and_protocol_ok() {
    check_ok(
        "protocol P { f(): Number; }
         type A { f(): Number => 1; }
         42;",
    );
}
