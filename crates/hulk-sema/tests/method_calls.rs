use hulk_frontend::parse_hulk_types_program;
use hulk_sema::check_program;
use hulk_sema::error::SemanticError;

fn check_ok(src: &str) {
    let program = parse_hulk_types_program(src).expect("source should parse");
    check_program(&program).expect("type check should pass");
}

fn check_err(src: &str) -> SemanticError {
    let program = parse_hulk_types_program(src).expect("source should parse");
    check_program(&program)
        .expect_err("type check should fail")
        .into_iter()
        .next()
        .expect("at least one error")
}

#[test]
fn method_call_valid_argument_type() {
    check_ok(
        "type A { f(x: Number): Number => x; }\n\
         new A().f(1);",
    );
}

#[test]
fn method_call_undefined_method_on_user_type() {
    let err = check_err(
        "type A {}\n\
         new A().f();",
    );
    assert!(matches!(err, SemanticError::UndefinedMethod { .. }));
}

#[test]
fn method_call_too_few_args() {
    let err = check_err(
        "type A { f(x: Number): Number => x; }\n\
         new A().f();",
    );
    assert!(matches!(err, SemanticError::ArityMismatch { .. }));
}

#[test]
fn method_call_too_many_args() {
    let err = check_err(
        "type A { f(x: Number): Number => x; }\n\
         new A().f(1, 2);",
    );
    assert!(matches!(err, SemanticError::ArityMismatch { .. }));
}

#[test]
fn method_call_invalid_argument_type() {
    let err = check_err(
        "type A { f(x: Number): Number => x; }\n\
         new A().f(\"bad\");",
    );
    assert!(matches!(err, SemanticError::InvalidArgumentType { .. }));
}

#[test]
fn inherited_method_call_validates_args() {
    check_ok(
        "type Animal { speak(n: Number): String => \"hi\"; }\n\
         type Dog inherits Animal() {}\n\
         new Dog().speak(1);",
    );
}

#[test]
fn inherited_method_call_invalid_arg_type() {
    let err = check_err(
        "type Animal { speak(n: Number): String => \"hi\"; }\n\
         type Dog inherits Animal() {}\n\
         new Dog().speak(\"bad\");",
    );
    assert!(matches!(err, SemanticError::InvalidArgumentType { .. }));
}

#[test]
fn vector_size_method_valid() {
    check_ok("let v = [1, 2, 3] in v.size();");
}

#[test]
fn vector_size_rejects_args() {
    let err = check_err("let v = [1, 2, 3] in v.size(1);");
    assert!(matches!(err, SemanticError::ArityMismatch { .. }));
}

#[test]
fn vector_unknown_method_fails() {
    let err = check_err("let v = [1, 2, 3] in v.foo();");
    assert!(matches!(err, SemanticError::UndefinedMethod { .. }));
}

#[test]
fn iterable_next_method_valid() {
    check_ok("let r = range(0, 10) in r.next();");
}

#[test]
fn iterable_unknown_method_fails() {
    let err = check_err("let r = range(0, 10) in r.foo();");
    assert!(matches!(err, SemanticError::UndefinedMethod { .. }));
}

#[test]
fn method_call_on_number_fails() {
    let err = check_err("42.foo();");
    assert!(matches!(err, SemanticError::UndefinedMethod { .. }));
}
