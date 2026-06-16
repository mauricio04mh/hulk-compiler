use hulk_frontend::ast::{BinaryOp, Decl, Expr, Program, Span, TypeRef};
use hulk_frontend::error::FrontendError;
use hulk_frontend::parse_hulk_functions_program;

fn parse_ok(source: &str) -> Program {
    parse_hulk_functions_program(source).expect("source should parse into AST")
}

#[test]
fn case_a_function_inline() {
    let program = parse_ok("function square(x: Number): Number => x * x;\n\nsquare(5);");
    assert_eq!(program.declarations.len(), 1);

    let Decl::Function(func) = &program.declarations[0] else {
        panic!("expected function declaration");
    };
    assert_eq!(func.name, "square");
    assert_eq!(func.params.len(), 1);
    assert_eq!(func.params[0].name, "x");
    assert_eq!(
        func.params[0].ty,
        Some(TypeRef::Simple("Number".to_string()))
    );
    assert_eq!(
        func.return_type,
        Some(TypeRef::Simple("Number".to_string()))
    );
    assert_eq!(
        func.body,
        Expr::Binary {
            span: Span::default(),
            left: Box::new(Expr::Var("x".to_string(), Span::default())),
            op: BinaryOp::Mul,
            right: Box::new(Expr::Var("x".to_string(), Span::default())),
        }
    );

    assert_eq!(
        program.entry,
        Expr::Call {
            span: Span::default(),
            callee: Box::new(Expr::Var("square".to_string(), Span::default())),
            args: vec![Expr::Number(5.0)],
        }
    );
}

#[test]
fn case_b_function_without_types() {
    let program = parse_ok("function id(x) => x;\n\nid(42);");
    let Decl::Function(func) = &program.declarations[0] else {
        panic!("expected function declaration");
    };
    assert_eq!(func.params.len(), 1);
    assert_eq!(func.params[0].ty, None);
    assert_eq!(func.return_type, None);
}

#[test]
fn case_c_function_multiple_params() {
    let program =
        parse_ok("function add(x: Number, y: Number): Number => x + y;\n\nprint(add(1, 2));");
    let Decl::Function(func) = &program.declarations[0] else {
        panic!("expected function declaration");
    };
    assert_eq!(func.params.len(), 2);
    assert_eq!(func.params[0].name, "x");
    assert_eq!(func.params[1].name, "y");
}

#[test]
fn case_d_function_block_body() {
    let program = parse_ok("function demo() {\n    print(1);\n    print(2);\n}\n\ndemo();");
    let Decl::Function(func) = &program.declarations[0] else {
        panic!("expected function declaration");
    };

    match &func.body {
        Expr::Block(exprs) => {
            assert_eq!(exprs.len(), 2);
        }
        other => panic!("expected block body, got {:?}", other),
    }
}

#[test]
fn case_e_entry_is_block() {
    let program = parse_ok("{\n    print(1);\n    print(2);\n}");
    assert_eq!(program.declarations.len(), 0);
    match program.entry {
        Expr::Block(exprs) => assert_eq!(exprs.len(), 2),
        other => panic!("expected block entry, got {:?}", other),
    }
}

#[test]
fn case_f_block_inside_let() {
    let program = parse_ok("let x = 1 in {\n    print(x);\n    x := x + 1;\n};");
    match program.entry {
        Expr::Let { body, .. } => match *body {
            Expr::Block(exprs) => assert_eq!(exprs.len(), 2),
            other => panic!("expected let body block, got {:?}", other),
        },
        other => panic!("expected let entry, got {:?}", other),
    }
}

#[test]
fn case_g_invalid_function_signature() {
    let err = parse_hulk_functions_program("function bad(x: Number => x;");
    assert!(err.is_err());
}

#[test]
fn case_h_invalid_missing_rbrace() {
    let err = parse_hulk_functions_program("function f() { print(1);");
    assert!(err.is_err());
}

// ──────────────────────────────────────────────
// T2.1: ParamList / ParamListTail recovery
// ──────────────────────────────────────────────

#[test]
fn recovery_param_bad_type_annotation_reports_one_error() {
    // `x: 42` — number where a type IDENT is expected; `y` should still parse.
    let err =
        parse_hulk_functions_program("function f(x: 42, y: Number): Number => y;\n\nf(1, 2);")
            .expect_err("should fail due to invalid type annotation");
    let FrontendError::ParseErrors(list) = err else {
        panic!("expected ParseErrors");
    };
    assert_eq!(
        list.errors().len(),
        1,
        "expected exactly one error, got {:?}",
        list.errors()
    );
}

#[test]
fn recovery_two_bad_params_report_two_errors() {
    // Both `x: 42` and `y: 99` have invalid type annotations.
    let err = parse_hulk_functions_program("function f(x: 42, y: 99): Number => 0;\n\nf(1, 2);")
        .expect_err("should fail");
    let FrontendError::ParseErrors(list) = err else {
        panic!("expected ParseErrors");
    };
    assert_eq!(
        list.errors().len(),
        2,
        "expected two errors, got {:?}",
        list.errors()
    );
}
