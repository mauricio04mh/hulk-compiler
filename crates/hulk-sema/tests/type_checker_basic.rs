use hulk_frontend::{parse_hulk_control_program, parse_hulk_types_program};
use hulk_sema::check_program;
use hulk_sema::error::SemanticError;
use hulk_sema::types::Type;

fn check_ok(source: &str) {
    let program = parse_hulk_control_program(source).expect("source should parse");
    check_program(&program).expect("type check should pass");
}

fn check_ok_types(source: &str) {
    let program = parse_hulk_types_program(source).expect("source should parse");
    check_program(&program).expect("type check should pass");
}

fn check_err(source: &str) -> SemanticError {
    let program = parse_hulk_control_program(source).expect("source should parse");
    check_program(&program)
        .expect_err("type check should fail")
        .into_iter()
        .next()
        .expect("at least one error")
}

fn check_err_types(source: &str) -> SemanticError {
    let program = parse_hulk_types_program(source).expect("source should parse");
    check_program(&program)
        .expect_err("type check should fail")
        .into_iter()
        .next()
        .expect("at least one error")
}

#[test]
fn case_a_arithmetic() {
    check_ok("1 + 2 * 3;");
}

#[test]
fn case_b_invalid_arithmetic() {
    let err = check_err("\"hello\" + 1;");
    assert!(matches!(err, SemanticError::InvalidBinaryOperands { .. }));
}

#[test]
fn case_c_boolean_condition() {
    check_ok("if (1 < 2) 10 else 20;");
}

#[test]
fn case_d_invalid_if_condition() {
    let err = check_err("if (1) 10 else 20;");
    assert_eq!(
        err,
        SemanticError::InvalidConditionType {
            found: Type::Number
        }
    );
}

#[test]
fn case_e_invalid_branch_mismatch() {
    let err = check_err("if (true) 1 else \"one\";");
    assert!(matches!(err, SemanticError::TypeMismatch { .. }));
}

#[test]
fn case_f_let_inference() {
    check_ok("let x = 1, y = x + 2 in y;");
}

#[test]
fn case_g_invalid_assignment() {
    let err = check_err("let x = 1 in x := \"hello\";");
    assert!(matches!(err, SemanticError::TypeMismatch { .. }));
}

#[test]
fn case_h_valid_function() {
    check_ok("function square(x: Number): Number => x * x;\n\nsquare(5);");
}

#[test]
fn case_i_invalid_function_arg() {
    let err = check_err("function square(x: Number): Number => x * x;\n\nsquare(\"hello\");");
    assert!(matches!(err, SemanticError::InvalidArgumentType { .. }));
}

#[test]
fn case_j_invalid_return() {
    let err = check_err("function f(x: Number): Number => \"hello\";\n\nf(1);");
    assert!(matches!(err, SemanticError::InvalidReturnType { .. }));
}

#[test]
fn case_k_function_return_inference() {
    check_ok("function f(x: Number) => x + 1;\n\nf(2);");
}

#[test]
fn case_l_parameter_without_type() {
    check_ok("function f(x) => x + 1;\n\nf(1);");
}

#[test]
fn case_l_unconstrained_parameter_still_fails() {
    let err = check_err("function f(x) => x;\n\n1;");
    assert!(matches!(
        err,
        SemanticError::CannotInferParameterType { .. }
    ));
}

#[test]
fn case_m_while_valid() {
    check_ok("let x = 3 in while (x > 0) { x := x - 1; };");
}

#[test]
fn case_n_while_invalid_condition() {
    let err = check_err("while (1) print(1);");
    assert_eq!(
        err,
        SemanticError::InvalidConditionType {
            found: Type::Number
        }
    );
}

#[test]
fn builtin_log_takes_two_numbers() {
    check_ok("log(100, 10);");
}

#[test]
fn builtin_log_rejects_one_argument() {
    let err = check_err("log(100);");
    assert!(matches!(err, SemanticError::ArityMismatch { .. }));
}

#[test]
fn builtin_log_rejects_string_argument() {
    let err = check_err("log(\"100\", 10);");
    assert!(matches!(err, SemanticError::InvalidArgumentType { .. }));
}

#[test]
fn builtin_range_takes_two_numbers() {
    check_ok_types("for (x in range(0, 10)) x + 1;");
}

#[test]
fn builtin_constants_are_numbers() {
    check_ok("PI + E;");
}

#[test]
fn unannotated_method_parameter_reports_cannot_infer() {
    let err = check_err_types(
        "type A {
            f(x) => x;
        }

        1;",
    );

    assert!(matches!(
        err,
        SemanticError::CannotInferParameterType { function, parameter }
            if function == "f" && parameter == "x"
    ));
}

#[test]
fn unannotated_type_parameter_reports_cannot_infer() {
    let err = check_err_types(
        "type Box(x) {
            value: Object = 1;
        }

        1;",
    );

    assert!(matches!(
        err,
        SemanticError::CannotInferParameterType { function, parameter }
            if function == "Box" && parameter == "x"
    ));
}

#[test]
fn annotated_function_parameter_still_passes() {
    check_ok("function f(x: Number): Number => x + 1;\n\nf(1);");
}

#[test]
fn unannotated_let_still_passes() {
    check_ok("let x = 42 in x;");
}

#[test]
fn annotated_type_parameter_still_passes() {
    check_ok_types(
        "type Box(x: Number) {
            value: Number = x;
        }

        new Box(1);",
    );
}
