use hulk_frontend::parse_hulk_types_program;
use hulk_sema::check_program;
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

// ── Wildcard arm ─────────────────────────────────────────────────────────────

#[test]
fn match_wildcard_arm_compiles() {
    check_ok("match (42) { _ => 0 };");
}

#[test]
fn match_wildcard_returns_body_value() {
    check_ok("let x = match (1) { _ => 99 } in x;");
}

// ── Literal arms ─────────────────────────────────────────────────────────────

#[test]
fn match_number_literal_arm() {
    check_ok("match (1) { 1 => true, _ => false };");
}

#[test]
fn match_bool_literal_arm() {
    check_ok("match (true) { true => 1, false => 2, _ => 0 };");
}

#[test]
fn match_string_literal_arm() {
    check_ok(r#"match ("hi") { "hi" => 1, _ => 0 };"#);
}

// ── Binding arm ──────────────────────────────────────────────────────────────

#[test]
fn match_binding_arm_introduces_variable() {
    check_ok("match (42) { n => n };");
}

#[test]
fn match_binding_is_last_arm() {
    check_ok("match (1) { 1 => 10, n => n };");
}

// ── Type-pattern arms ────────────────────────────────────────────────────────

#[test]
fn match_type_pattern_without_bind() {
    check_ok(
        "type Animal() {}\n\
         type Dog() inherits Animal {}\n\
         let a: Animal = new Dog() in \
         match (a) { Dog => 1, _ => 0 };",
    );
}

#[test]
fn match_type_pattern_with_bind() {
    check_ok(
        "type Shape() { area(): Number => 0; }\n\
         type Circle() inherits Shape { area(): Number => 3; }\n\
         let s: Shape = new Circle() in \
         match (s) { Circle as c => c.area(), _ => 0 };",
    );
}

// ── Multiple arms ─────────────────────────────────────────────────────────────

#[test]
fn match_multiple_type_patterns() {
    check_ok(
        "type A() {}\n\
         type B() inherits A {}\n\
         type C() inherits A {}\n\
         let a: A = new B() in \
         match (a) { B => 1, C => 2, _ => 3 };",
    );
}

#[test]
fn match_mixed_pattern_types() {
    check_ok("match (1) { 1 => \"one\", 2 => \"two\", _ => \"other\" };");
}

// ── Non-exhaustive match ──────────────────────────────────────────────────────

#[test]
fn match_non_exhaustive_emits_error() {
    let err = check_err("match (1) { 1 => 10 };");
    assert!(
        matches!(err, SemanticError::NonExhaustiveMatch { .. }),
        "expected NonExhaustiveMatch, got {:?}",
        err
    );
}
