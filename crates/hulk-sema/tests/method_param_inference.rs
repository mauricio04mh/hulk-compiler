use hulk_frontend::parse_hulk_types_program;
use hulk_sema::analyze_program;
use hulk_sema::hir::{HirDecl, HirMethodDecl};
use hulk_sema::types::Type;

fn analyze(source: &str) -> hulk_sema::hir::SemanticProgram {
    let program = parse_hulk_types_program(source).expect("source should parse");
    analyze_program(&program).expect("analysis should pass")
}

fn method<'a>(
    program: &'a hulk_sema::hir::SemanticProgram,
    type_name: &str,
    method_name: &str,
) -> &'a HirMethodDecl {
    let type_decl = program
        .hir
        .declarations
        .iter()
        .find_map(|decl| match decl {
            HirDecl::Type(type_decl) if type_decl.name == type_name => Some(type_decl),
            _ => None,
        })
        .expect("type should exist");

    type_decl
        .methods
        .iter()
        .find(|method| method.name == method_name)
        .expect("method should exist")
}

#[test]
fn infer_method_param_from_arithmetic_body() {
    let program = analyze(
        "type A {
            f(x) => x + 1;
        }

        new A().f(1);",
    );

    let method = method(&program, "A", "f");
    assert_eq!(method.params[0].ty, Type::Number);
    assert_eq!(method.return_type, Type::Number);
}

#[test]
fn infer_method_param_from_boolean_body() {
    let program = analyze(
        "type A {
            f(x) => !x;
        }

        new A().f(true);",
    );

    let method = method(&program, "A", "f");
    assert_eq!(method.params[0].ty, Type::Boolean);
    assert_eq!(method.return_type, Type::Boolean);
}

#[test]
fn infer_method_param_from_string_concat_body() {
    let program = analyze(
        "type A {
            f(x) => x @ \" world\";
        }

        new A().f(\"hello\");",
    );

    let method = method(&program, "A", "f");
    assert_eq!(method.params[0].ty, Type::String);
    assert_eq!(method.return_type, Type::String);
}

#[test]
fn infer_multiple_method_params_from_body() {
    let program = analyze(
        "type A {
            f(x, y) => x + y;
        }

        new A().f(1, 2);",
    );

    let method = method(&program, "A", "f");
    assert_eq!(method.params[0].ty, Type::Number);
    assert_eq!(method.params[1].ty, Type::Number);
    assert_eq!(method.return_type, Type::Number);
}

#[test]
fn unconstrained_method_param_still_fails() {
    let program = parse_hulk_types_program(
        "type A {
            f(x) => x;
        }

        1;",
    )
    .expect("source should parse");

    let err = analyze_program(&program).expect_err("analysis should fail");
    assert!(err.iter().any(
        |error| matches!(error, hulk_sema::error::SemanticError::CannotInferParameterType { function, parameter } if function == "f" && parameter == "x")
    ));
}
