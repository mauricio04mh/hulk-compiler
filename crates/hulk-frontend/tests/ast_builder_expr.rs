use hulk_frontend::ast::{BinaryOp, Expr, Program, Span, UnaryOp};
use hulk_frontend::parse_hulk_expr_program;

fn parse_ok(source: &str) -> Program {
    parse_hulk_expr_program(source).expect("source should parse into AST")
}

#[test]
fn case_1_number_program() {
    let program = parse_ok("42;");
    assert_eq!(
        program,
        Program {
            declarations: vec![],
            entry: Expr::Number(42.0)
        }
    );
}

#[test]
fn case_2_var_program() {
    let program = parse_ok("x;");
    assert_eq!(
        program,
        Program {
            declarations: vec![],
            entry: Expr::Var("x".to_string(), Span::default())
        }
    );
}

#[test]
fn case_3_precedence_add_mul() {
    let program = parse_ok("1 + 2 * 3;");
    assert_eq!(
        program.entry,
        Expr::Binary {
            span: Span::default(),
            left: Box::new(Expr::Number(1.0)),
            op: BinaryOp::Add,
            right: Box::new(Expr::Binary {
                span: Span::default(),
                left: Box::new(Expr::Number(2.0)),
                op: BinaryOp::Mul,
                right: Box::new(Expr::Number(3.0)),
            }),
        }
    );
}

#[test]
fn case_4_parenthesized_grouping() {
    let program = parse_ok("(1 + 2) * 3;");
    assert_eq!(
        program.entry,
        Expr::Binary {
            span: Span::default(),
            left: Box::new(Expr::Binary {
                span: Span::default(),
                left: Box::new(Expr::Number(1.0)),
                op: BinaryOp::Add,
                right: Box::new(Expr::Number(2.0)),
            }),
            op: BinaryOp::Mul,
            right: Box::new(Expr::Number(3.0)),
        }
    );
}

#[test]
fn case_5_left_assoc_sub() {
    let program = parse_ok("a - b - c;");
    assert_eq!(
        program.entry,
        Expr::Binary {
            span: Span::default(),
            left: Box::new(Expr::Binary {
                span: Span::default(),
                left: Box::new(Expr::Var("a".to_string(), Span::default())),
                op: BinaryOp::Sub,
                right: Box::new(Expr::Var("b".to_string(), Span::default())),
            }),
            op: BinaryOp::Sub,
            right: Box::new(Expr::Var("c".to_string(), Span::default())),
        }
    );
}

#[test]
fn case_6_right_assoc_pow() {
    let program = parse_ok("a ^ b ^ c;");
    assert_eq!(
        program.entry,
        Expr::Binary {
            span: Span::default(),
            left: Box::new(Expr::Var("a".to_string(), Span::default())),
            op: BinaryOp::Pow,
            right: Box::new(Expr::Binary {
                span: Span::default(),
                left: Box::new(Expr::Var("b".to_string(), Span::default())),
                op: BinaryOp::Pow,
                right: Box::new(Expr::Var("c".to_string(), Span::default())),
            }),
        }
    );
}

#[test]
fn case_7_unary_and_or_gt() {
    let program = parse_ok("!flag | x > 0;");
    assert_eq!(
        program.entry,
        Expr::Binary {
            span: Span::default(),
            left: Box::new(Expr::Unary {
                span: Span::default(),
                op: UnaryOp::Not,
                expr: Box::new(Expr::Var("flag".to_string(), Span::default())),
            }),
            op: BinaryOp::Or,
            right: Box::new(Expr::Binary {
                span: Span::default(),
                left: Box::new(Expr::Var("x".to_string(), Span::default())),
                op: BinaryOp::Gt,
                right: Box::new(Expr::Number(0.0)),
            }),
        }
    );
}

#[test]
fn case_8_concat() {
    let program = parse_ok("\"hello\" @ name;");
    assert_eq!(
        program.entry,
        Expr::Binary {
            span: Span::default(),
            left: Box::new(Expr::String("hello".to_string())),
            op: BinaryOp::Concat,
            right: Box::new(Expr::Var("name".to_string(), Span::default())),
        }
    );
}

#[test]
fn case_9_call() {
    let program = parse_ok("print(1 + 2);");
    assert_eq!(
        program.entry,
        Expr::Call {
            span: Span::default(),
            callee: Box::new(Expr::Var("print".to_string(), Span::default())),
            args: vec![Expr::Binary {
                span: Span::default(),
                left: Box::new(Expr::Number(1.0)),
                op: BinaryOp::Add,
                right: Box::new(Expr::Number(2.0)),
            }],
        }
    );
}

#[test]
fn case_10_let_with_binding_and_call_body() {
    let program = parse_ok("let x = 5 + 2 * 3 in print(x);");
    match program.entry {
        Expr::Let { bindings, body, .. } => {
            assert_eq!(bindings.len(), 1);
            assert_eq!(bindings[0].name, "x");
            assert_eq!(
                bindings[0].value,
                Expr::Binary {
                    span: Span::default(),
                    left: Box::new(Expr::Number(5.0)),
                    op: BinaryOp::Add,
                    right: Box::new(Expr::Binary {
                        span: Span::default(),
                        left: Box::new(Expr::Number(2.0)),
                        op: BinaryOp::Mul,
                        right: Box::new(Expr::Number(3.0)),
                    }),
                }
            );
            assert_eq!(
                *body,
                Expr::Call {
                    span: Span::default(),
                    callee: Box::new(Expr::Var("print".to_string(), Span::default())),
                    args: vec![Expr::Var("x".to_string(), Span::default())],
                }
            );
        }
        other => panic!("expected Let, got {:?}", other),
    }
}

#[test]
fn case_11_let_with_assign() {
    let program = parse_ok("let x = 10 in x := x - 1;");
    match program.entry {
        Expr::Let { bindings, body, .. } => {
            assert_eq!(bindings.len(), 1);
            assert_eq!(bindings[0].name, "x");
            assert_eq!(bindings[0].value, Expr::Number(10.0));
            assert_eq!(
                *body,
                Expr::Assign {
                    span: Span::default(),
                    target: Box::new(Expr::Var("x".to_string(), Span::default())),
                    value: Box::new(Expr::Binary {
                        span: Span::default(),
                        left: Box::new(Expr::Var("x".to_string(), Span::default())),
                        op: BinaryOp::Sub,
                        right: Box::new(Expr::Number(1.0)),
                    }),
                }
            );
        }
        other => panic!("expected Let, got {:?}", other),
    }
}
