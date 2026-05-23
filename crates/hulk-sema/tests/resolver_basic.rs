use hulk_frontend::parse_hulk_control_program;
use hulk_sema::error::SemanticError;
use hulk_sema::resolve_program;

fn resolve_ok(source: &str) {
    let program = parse_hulk_control_program(source).expect("source should parse");
    resolve_program(&program).expect("semantic resolution should pass");
}

fn resolve_err(source: &str) -> SemanticError {
    let program = parse_hulk_control_program(source).expect("source should parse");
    resolve_program(&program).expect_err("semantic resolution should fail")
}

#[test]
fn case_a_valid_let() {
    resolve_ok("let x = 42 in x;");
}

#[test]
fn case_b_undefined_variable() {
    let err = resolve_err("x;");
    assert_eq!(
        err,
        SemanticError::UndefinedVariable {
            name: "x".to_string()
        }
    );
}

#[test]
fn case_c_valid_builtin() {
    resolve_ok("print(42);");
}

#[test]
fn case_d_function_declaration_and_call() {
    resolve_ok("function square(x: Number): Number => x * x;\n\nsquare(5);");
}

#[test]
fn case_e_function_can_call_later_function() {
    resolve_ok("function f() => g();\nfunction g() => 42;\n\nf();");
}

#[test]
fn case_f_duplicate_function() {
    let err = resolve_err("function f() => 1;\nfunction f() => 2;\n\nf();");
    assert_eq!(
        err,
        SemanticError::DuplicateFunction {
            name: "f".to_string()
        }
    );
}

#[test]
fn case_g_duplicate_parameter() {
    let err = resolve_err("function f(x, x) => x;\n\nf(1);");
    assert_eq!(
        err,
        SemanticError::DuplicateParameter {
            function: "f".to_string(),
            parameter: "x".to_string()
        }
    );
}

#[test]
fn case_h_let_sequential_binding() {
    resolve_ok("let x = 1, y = x + 1 in y;");
}

#[test]
fn case_i_undefined_variable_in_let_binding() {
    let err = resolve_err("let y = x + 1 in y;");
    assert_eq!(
        err,
        SemanticError::UndefinedVariable {
            name: "x".to_string()
        }
    );
}

#[test]
fn case_j_assignment_valid() {
    resolve_ok("let x = 1 in x := x + 1;");
}

#[test]
fn case_k_assignment_undefined() {
    let err = resolve_err("x := 1;");
    assert_eq!(
        err,
        SemanticError::UndefinedVariable {
            name: "x".to_string()
        }
    );
}

#[test]
fn case_l_invalid_assignment_target() {
    let err = resolve_err("let x = 1 in (x + 1) := 2;");
    assert_eq!(err, SemanticError::InvalidAssignmentTarget);
}

#[test]
fn case_m_if_resolves_variables() {
    resolve_ok("let x = 1 in if (x > 0) x else 0;");
}

#[test]
fn case_n_while_resolves_variables() {
    resolve_ok("let x = 3 in while (x > 0) { x := x - 1; };");
}
