use hulk_frontend::ast::{BinaryOp, Decl, Expr};
use hulk_frontend::parse_hulk_control_program;

fn parse_ok(source: &str) -> hulk_frontend::ast::Program {
    parse_hulk_control_program(source).expect("source should parse into AST")
}

#[test]
fn case_a_if_simple() {
    let program = parse_ok("if (x > 0) print(x) else print(0);");
    match program.entry {
        Expr::If {
            branches,
            else_branch,
            ..
        } => {
            assert_eq!(branches.len(), 1);
            match &branches[0].0 {
                Expr::Binary { op, .. } => assert_eq!(*op, BinaryOp::Gt),
                other => panic!("expected gt condition, got {:?}", other),
            }
            match *else_branch {
                Expr::Call { .. } => {}
                other => panic!("expected call in else, got {:?}", other),
            }
        }
        other => panic!("expected if entry, got {:?}", other),
    }
}

#[test]
fn case_b_if_with_elif() {
    let program = parse_ok("if (x == 0) \"zero\" elif (x == 1) \"one\" else \"many\";");
    match program.entry {
        Expr::If {
            branches,
            else_branch,
            ..
        } => {
            assert_eq!(branches.len(), 2);
            assert_eq!(*else_branch, Expr::String("many".to_string()));
        }
        other => panic!("expected if entry, got {:?}", other),
    }
}

#[test]
fn case_c_if_with_blocks() {
    let program = parse_ok("if (x > 0) { print(x); } else { print(0); }");
    match program.entry {
        Expr::If {
            branches,
            else_branch,
            ..
        } => {
            match &branches[0].1 {
                Expr::Block(_) => {}
                other => panic!("expected block then, got {:?}", other),
            }
            match *else_branch {
                Expr::Block(_) => {}
                other => panic!("expected block else, got {:?}", other),
            }
        }
        other => panic!("expected if entry, got {:?}", other),
    }
}

#[test]
fn case_d_while_simple() {
    let program = parse_ok("while (x > 0) { print(x); x := x - 1; }");
    match program.entry {
        Expr::While { condition, body, .. } => {
            match *condition {
                Expr::Binary { op, .. } => assert_eq!(op, BinaryOp::Gt),
                other => panic!("expected gt condition, got {:?}", other),
            }
            match *body {
                Expr::Block(exprs) => assert_eq!(exprs.len(), 2),
                other => panic!("expected while block body, got {:?}", other),
            }
        }
        other => panic!("expected while entry, got {:?}", other),
    }
}

#[test]
fn case_e_while_inside_function() {
    let program = parse_ok(
        "function countdown(x: Number) { while (x > 0) { print(x); x := x - 1; }; } countdown(3);",
    );
    assert_eq!(program.declarations.len(), 1);
    let Decl::Function(func) = &program.declarations[0] else {
        panic!("expected function declaration");
    };
    match &func.body {
        Expr::Block(exprs) => {
            assert!(!exprs.is_empty());
            let has_while = exprs.iter().any(|expr| matches!(expr, Expr::While { .. }));
            assert!(has_while);
        }
        other => panic!("expected function body block, got {:?}", other),
    }
}

#[test]
fn case_f_let_with_if() {
    let program = parse_ok("let x = 5 in if (x > 0) x else 0;");
    match program.entry {
        Expr::Let { body, .. } => match *body {
            Expr::If { .. } => {}
            other => panic!("expected let body if, got {:?}", other),
        },
        other => panic!("expected let entry, got {:?}", other),
    }
}

#[test]
fn case_g_invalid_if_without_else() {
    let err = parse_hulk_control_program("if (x > 0) print(x);");
    assert!(err.is_err());
}

#[test]
fn case_h_invalid_while_missing_rparen() {
    let err = parse_hulk_control_program("while (x > 0 { print(x); }");
    assert!(err.is_err());
}
