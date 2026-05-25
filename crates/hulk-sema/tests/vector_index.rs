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

fn check_errors(src: &str) -> Vec<SemanticError> {
    let program = parse_hulk_types_program(src).expect("source should parse");
    check_program(&program).expect_err("type check should fail")
}

#[test]
fn vector_index_number_vector_ok() {
    check_ok("let v = [1, 2, 3] in v[0];");
}

#[test]
fn vector_index_string_vector_ok() {
    check_ok("let v = [\"a\", \"b\"] in v[1];");
}

#[test]
fn vector_index_nested_vector_ok() {
    check_ok("let matrix = [[1, 2], [3, 4]] in matrix[0][1];");
}

#[test]
fn vector_index_rejects_number_target() {
    let err = check_err("let x = 42 in x[0];");
    assert!(matches!(err, SemanticError::InvalidIndexTarget { .. }));
}

#[test]
fn vector_index_rejects_string_target() {
    let err = check_err("\"hello\"[0];");
    assert!(matches!(err, SemanticError::InvalidIndexTarget { .. }));
}

#[test]
fn vector_index_rejects_boolean_target() {
    let err = check_err("true[0];");
    assert!(matches!(err, SemanticError::InvalidIndexTarget { .. }));
}

#[test]
fn vector_index_rejects_string_index() {
    let err = check_err("let v = [1, 2, 3] in v[\"bad\"];");
    assert!(matches!(err, SemanticError::TypeMismatch { .. }));
}

#[test]
fn vector_index_reports_target_and_index_errors_when_both_invalid() {
    let errors = check_errors("42[\"bad\"];");
    assert!(
        errors.iter().any(|e| matches!(e, SemanticError::InvalidIndexTarget { .. })),
        "expected InvalidIndexTarget, got {:?}",
        errors
    );
    assert!(
        errors.iter().any(|e| matches!(e, SemanticError::TypeMismatch { .. })),
        "expected TypeMismatch for index, got {:?}",
        errors
    );
}
