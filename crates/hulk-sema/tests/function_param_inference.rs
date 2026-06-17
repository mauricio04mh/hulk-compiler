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
fn infer_multi_parameter_from_call_site() {
    let program =
        analyze("function add(x, y, z) => x + y;\n\nadd(1, 2, 3);").expect("should analyze");
    let func = function(&program, "add");
    assert_eq!(func.params[0].ty, Type::Number);
    assert_eq!(func.params[1].ty, Type::Number);
    assert_eq!(func.params[2].ty, Type::Number);
    assert_eq!(func.return_type, Type::Number);
}

#[test]
fn unconstrained_parameter_still_fails() {
    let err = analyze("function id(x) => x;\n\n1;")
        .expect_err("analysis should fail for unconstrained parameter");
    assert!(matches!(
        err.as_slice(),
        [SemanticError::CannotInferParameterType { .. }, ..]
    ));
}

#[test]
fn infer_pure_call_site_number() {
    let program = analyze("function id(x) => x;\n\nid(1);").expect("should analyze");
    let func = function(&program, "id");
    assert_eq!(func.params[0].ty, Type::Number);
    assert_eq!(func.return_type, Type::Number);
}

#[test]
fn infer_pure_call_site_string() {
    let program = analyze("function id(x) => x;\n\nid(\"hello\");").expect("should analyze");
    let func = function(&program, "id");
    assert_eq!(func.params[0].ty, Type::String);
    assert_eq!(func.return_type, Type::String);
}

#[test]
fn infer_across_one_global_function_call_number() {
    let program = analyze("function g(y) => y + 1;\nfunction f(x) => g(x);\n\nf(1);")
        .expect("should analyze");
    let g = function(&program, "g");
    let f = function(&program, "f");
    assert_eq!(g.params[0].ty, Type::Number);
    assert_eq!(g.return_type, Type::Number);
    assert_eq!(f.params[0].ty, Type::Number);
    assert_eq!(f.return_type, Type::Number);
}

#[test]
fn infer_identity_chain_string() {
    let program = analyze("function id(x) => x;\nfunction wrap(y) => id(y);\n\nwrap(\"hello\");")
        .expect("should analyze");
    let id = function(&program, "id");
    let wrap = function(&program, "wrap");
    assert_eq!(id.params[0].ty, Type::String);
    assert_eq!(id.return_type, Type::String);
    assert_eq!(wrap.params[0].ty, Type::String);
    assert_eq!(wrap.return_type, Type::String);
}

#[test]
fn infer_multiple_args_across_function_call() {
    let program =
        analyze("function add(a, b) => a + b;\nfunction f(x, y) => add(x, y);\n\nf(1, 2);")
            .expect("should analyze");
    let add = function(&program, "add");
    let f = function(&program, "f");
    assert_eq!(add.params[0].ty, Type::Number);
    assert_eq!(add.params[1].ty, Type::Number);
    assert_eq!(add.return_type, Type::Number);
    assert_eq!(f.params[0].ty, Type::Number);
    assert_eq!(f.params[1].ty, Type::Number);
    assert_eq!(f.return_type, Type::Number);
}

#[test]
fn infer_boolean_across_function_call() {
    let program = analyze("function negate(x) => !x;\nfunction f(y) => negate(y);\n\nf(true);")
        .expect("should analyze");
    let negate = function(&program, "negate");
    let f = function(&program, "f");
    assert_eq!(negate.params[0].ty, Type::Boolean);
    assert_eq!(negate.return_type, Type::Boolean);
    assert_eq!(f.params[0].ty, Type::Boolean);
    assert_eq!(f.return_type, Type::Boolean);
}

#[test]
fn unconstrained_function_chain_still_fails() {
    let err = analyze("function id(x) => x;\nfunction wrap(y) => id(y);\n\n1;")
        .expect_err("analysis should fail for unconstrained function chain");
    assert!(matches!(
        err.as_slice(),
        [SemanticError::CannotInferParameterType { .. }, ..]
    ));
}
