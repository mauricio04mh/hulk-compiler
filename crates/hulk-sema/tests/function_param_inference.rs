use hulk_frontend::parse_hulk_control_program;
use hulk_sema::analyze_program;
use hulk_sema::error::SemanticError;
use hulk_sema::hir::{HirDecl, HirFunctionDecl};
use hulk_sema::types::Type;

fn analyze(source: &str) -> Result<hulk_sema::hir::SemanticProgram, Vec<SemanticError>> {
    let program = parse_hulk_control_program(source).expect("source should parse");
    analyze_program(&program)
}

fn function<'a>(program: &'a hulk_sema::hir::SemanticProgram, name: &str) -> &'a HirFunctionDecl {
    program
        .hir
        .declarations
        .iter()
        .find_map(|decl| match decl {
            HirDecl::Function(func) if func.name == name => Some(func),
            _ => None,
        })
        .expect("function should exist")
}

#[test]
fn infer_number_from_arithmetic() {
    let program = analyze("function double(x) => x * 2;\n\ndouble(4);").expect("should analyze");
    let func = function(&program, "double");
    assert_eq!(func.params[0].ty, Type::Number);
    assert_eq!(func.return_type, Type::Number);
}

#[test]
fn infer_boolean_from_unary_not() {
    let program = analyze("function negate(x) => !x;\n\nnegate(true);").expect("should analyze");
    let func = function(&program, "negate");
    assert_eq!(func.params[0].ty, Type::Boolean);
    assert_eq!(func.return_type, Type::Boolean);
}

#[test]
fn infer_string_from_concat() {
    let program = analyze("function concat(x) => x @ \" world\";\n\nconcat(\"hello\");")
        .expect("should analyze");
    let func = function(&program, "concat");
    assert_eq!(func.params[0].ty, Type::String);
    assert_eq!(func.return_type, Type::String);
}

#[test]
fn infer_two_numbers_from_binary_operation() {
    let program = analyze("function add(x, y) => x + y;\n\nadd(1, 2);").expect("should analyze");
    let func = function(&program, "add");
    assert_eq!(func.params[0].ty, Type::Number);
    assert_eq!(func.params[1].ty, Type::Number);
    assert_eq!(func.return_type, Type::Number);
}

#[test]
fn unconstrained_parameter_still_fails() {
    let err = analyze("function id(x) => x;\n\nid(1);")
        .expect_err("analysis should fail for unconstrained parameter");
    assert!(matches!(
        err.as_slice(),
        [SemanticError::CannotInferParameterType { .. }, ..]
    ));
}
