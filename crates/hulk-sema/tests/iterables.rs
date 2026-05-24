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
fn for_over_range_ok() {
    check_ok("for (x in range(0, 10)) x + 1;");
}

#[test]
fn for_over_vector_ok() {
    check_ok("for (x in [1, 2, 3]) x + 1;");
}

#[test]
fn for_over_vector_variable_ok() {
    check_ok("let xs = [1, 2, 3] in for (x in xs) x + 1;");
}

#[test]
fn for_over_range_binds_number_ok() {
    check_ok("for (x in range(0, 10)) x * 2;");
}

#[test]
fn for_over_vector_binds_number_ok() {
    check_ok("for (x in [1, 2, 3]) x * 2;");
}

#[test]
fn for_rejects_number_iterable() {
    let err = check_err("for (x in 42) x;");
    assert!(matches!(err, SemanticError::InvalidIterableTarget { .. }));
}

#[test]
fn for_rejects_string_iterable() {
    let err = check_err("for (x in \"hello\") x;");
    assert!(matches!(err, SemanticError::InvalidIterableTarget { .. }));
}

#[test]
fn for_rejects_boolean_iterable() {
    let err = check_err("for (x in true) x;");
    assert!(matches!(err, SemanticError::InvalidIterableTarget { .. }));
}

#[test]
fn vector_generator_over_range_ok() {
    check_ok("[x + 1 | x in range(0, 10)];");
}

#[test]
fn vector_generator_over_vector_ok() {
    check_ok("[x + 1 | x in [1, 2, 3]];");
}

#[test]
fn vector_generator_rejects_number_iterable() {
    let err = check_err("[x | x in 42];");
    assert!(matches!(err, SemanticError::InvalidIterableTarget { .. }));
}

#[test]
fn vector_generator_rejects_string_iterable() {
    let err = check_err("[x | x in \"hello\"];");
    assert!(matches!(err, SemanticError::InvalidIterableTarget { .. }));
}
