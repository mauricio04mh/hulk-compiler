use hulk_frontend::parse_hulk_types_program;
use hulk_sema::analyze_program;
use hulk_sema::error::SemanticError;
use hulk_sema::hir::{HirDecl, HirMethodDecl, HirTypeDecl};
use hulk_sema::types::Type;

fn analyze(source: &str) -> hulk_sema::hir::SemanticProgram {
    let program = parse_hulk_types_program(source).expect("source should parse");
    analyze_program(&program).expect("analysis should pass")
}

fn type_decl<'a>(program: &'a hulk_sema::hir::SemanticProgram, name: &str) -> &'a HirTypeDecl {
    program
        .hir
        .declarations
        .iter()
        .find_map(|decl| match decl {
            HirDecl::Type(type_decl) if type_decl.name == name => Some(type_decl),
            _ => None,
        })
        .expect("type should exist")
}

fn method<'a>(
    program: &'a hulk_sema::hir::SemanticProgram,
    type_name: &str,
    method_name: &str,
) -> &'a HirMethodDecl {
    type_decl(program, type_name)
        .methods
        .iter()
        .find(|method| method.name == method_name)
        .expect("method should exist")
}

#[test]
fn infer_type_constructor_param_from_number_call_site() {
    let program = analyze(
        "type Box(value) {
            get() => value;
        }

        new Box(1).get();",
    );

    let type_decl = type_decl(&program, "Box");
    assert_eq!(type_decl.params[0].ty, Type::Number);
    assert_eq!(method(&program, "Box", "get").return_type, Type::Number);
}

#[test]
fn infer_type_constructor_param_from_string_call_site() {
    let program = analyze(
        "type Box(value) {
            get() => value;
        }

        new Box(\"hello\").get();",
    );

    let type_decl = type_decl(&program, "Box");
    assert_eq!(type_decl.params[0].ty, Type::String);
    assert_eq!(method(&program, "Box", "get").return_type, Type::String);
}

#[test]
fn infer_type_constructor_param_from_boolean_call_site() {
    let program = analyze(
        "type Box(value) {
            get() => value;
        }

        new Box(true).get();",
    );

    let type_decl = type_decl(&program, "Box");
    assert_eq!(type_decl.params[0].ty, Type::Boolean);
    assert_eq!(method(&program, "Box", "get").return_type, Type::Boolean);
}

#[test]
fn infer_multiple_type_constructor_params_from_call_site() {
    let program = analyze(
        "type Pair(x, y) {
            first() => x;
            second() => y;
        }

        new Pair(1, \"hello\").first();",
    );

    let type_decl = type_decl(&program, "Pair");
    assert_eq!(type_decl.params[0].ty, Type::Number);
    assert_eq!(type_decl.params[1].ty, Type::String);
    assert_eq!(method(&program, "Pair", "first").return_type, Type::Number);
    assert_eq!(method(&program, "Pair", "second").return_type, Type::String);
}

#[test]
fn infer_constructor_param_used_by_method_constraint() {
    let program = analyze(
        "type Counter(value) {
            inc() => value + 1;
        }

        new Counter(1).inc();",
    );

    let type_decl = type_decl(&program, "Counter");
    assert_eq!(type_decl.params[0].ty, Type::Number);
    assert_eq!(method(&program, "Counter", "inc").return_type, Type::Number);
}

#[test]
fn conflicting_type_constructor_call_site_types_fail() {
    let program = parse_hulk_types_program(
        "type Box(value) {
            get() => value;
        }

        {
            new Box(1).get();
            new Box(\"hello\").get();
        }",
    )
    .expect("source should parse");

    let err = analyze_program(&program).expect_err("analysis should fail");
    assert!(err.iter().any(|error| matches!(
        error,
        SemanticError::TypeMismatch { .. } | SemanticError::InvalidArgumentType { .. }
    )));
}

#[test]
fn unconstrained_type_constructor_param_still_fails() {
    let program = parse_hulk_types_program(
        "type Box(x) {
            get() => x;
        }

        1;",
    )
    .expect("source should parse");

    let err = analyze_program(&program).expect_err("analysis should fail");
    assert!(err.iter().any(
        |error| matches!(error, SemanticError::CannotInferParameterType { function, parameter } if function == "Box" && parameter == "x")
    ));
}
