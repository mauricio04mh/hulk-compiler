use hulk_frontend::ast::{BinaryOp, Decl, Expr, Span, TypeMember, TypeRef};
use hulk_frontend::error::FrontendError;
use hulk_frontend::parse_hulk_types_program;

fn parse_ok(source: &str) -> hulk_frontend::ast::Program {
    parse_hulk_types_program(source).expect("source should parse into AST")
}

#[test]
fn case_a_empty_type() {
    let program = parse_ok("type A {\n}\n\nnew A();");
    assert_eq!(program.declarations.len(), 1);

    let Decl::Type(ty) = &program.declarations[0] else {
        panic!("expected type declaration");
    };
    assert_eq!(ty.name, "A");
    assert!(ty.members.is_empty());

    assert_eq!(
        program.entry,
        Expr::New {
            span: Span::default(),
            type_name: "A".to_string(),
            args: vec![],
        }
    );
}

#[test]
fn case_b_type_with_params_and_attributes() {
    let program = parse_ok(
        "type Point(x: Number, y: Number) {\n    x: Number = x;\n    y: Number = y;\n}\n\nnew Point(3, 4);",
    );

    let Decl::Type(ty) = &program.declarations[0] else {
        panic!("expected type declaration");
    };
    assert_eq!(ty.params.len(), 2);
    assert_eq!(ty.params[0].name, "x");
    assert_eq!(ty.params[1].name, "y");

    assert_eq!(ty.members.len(), 2);
    match &ty.members[0] {
        TypeMember::Attribute(attr) => {
            assert_eq!(attr.name, "x");
            assert_eq!(attr.ty, Some(TypeRef::Simple("Number".to_string())));
        }
        other => panic!("expected attribute, got {:?}", other),
    }

    assert_eq!(
        program.entry,
        Expr::New {
            span: Span::default(),
            type_name: "Point".to_string(),
            args: vec![Expr::Number(3.0), Expr::Number(4.0)],
        }
    );
}

#[test]
fn case_c_type_with_method() {
    let program = parse_ok(
        "type Point(x: Number) {\n    x: Number = x;\n    getX(): Number => self.x;\n}\n\nlet p = new Point(3) in p.getX();",
    );

    let Decl::Type(ty) = &program.declarations[0] else {
        panic!("expected type declaration");
    };

    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            TypeMember::Method(method) => Some(method),
            _ => None,
        })
        .expect("expected getX method");

    assert_eq!(method.name, "getX");
    assert_eq!(
        method.body,
        Expr::MemberAccess {
            span: Span::default(),
            object: Box::new(Expr::SelfRef),
            member: "x".to_string(),
        }
    );

    match &program.entry {
        Expr::Let { body, .. } => match body.as_ref() {
            Expr::MethodCall { object, method, .. } => {
                assert_eq!(method, "getX");
                assert_eq!(**object, Expr::Var("p".to_string(), Span::default()));
            }
            other => panic!("expected method call in let body, got {:?}", other),
        },
        other => panic!("expected let entry, got {:?}", other),
    }
}

#[test]
fn case_d_type_with_inheritance() {
    let program = parse_ok(
        "type PolarPoint(phi: Number, rho: Number) inherits Point(rho, phi) {\n    angle(): Number => phi;\n}\n\nnew PolarPoint(1, 2);",
    );

    let Decl::Type(ty) = &program.declarations[0] else {
        panic!("expected type declaration");
    };

    let parent = ty.parent.as_ref().expect("expected parent");
    assert_eq!(parent.name, "Point");
    assert_eq!(parent.args.as_deref().map(|a| a.len()), Some(2));
}

#[test]
fn case_e_self_access_and_chained_method_call() {
    let program = parse_ok("type A { x: Number = 1; get(): Number => self.x; }\n\nnew A().get();");

    let Decl::Type(ty) = &program.declarations[0] else {
        panic!("expected type declaration");
    };
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            TypeMember::Method(method) => Some(method),
            _ => None,
        })
        .expect("expected get method");

    assert_eq!(
        method.body,
        Expr::MemberAccess {
            span: Span::default(),
            object: Box::new(Expr::SelfRef),
            member: "x".to_string(),
        }
    );

    match &program.entry {
        Expr::MethodCall {
            object,
            method,
            args,
            ..
        } => {
            assert_eq!(method, "get");
            assert!(args.is_empty());
            match object.as_ref() {
                Expr::New {
                    type_name, args, ..
                } => {
                    assert_eq!(type_name, "A");
                    assert!(args.is_empty());
                }
                other => panic!("expected new A() as method object, got {:?}", other),
            }
        }
        other => panic!("expected method call entry, got {:?}", other),
    }
}

#[test]
fn case_f_base_call() {
    let program = parse_ok("type B inherits A { get(): Number => base(); }\n\nnew B().get();");

    let Decl::Type(ty) = &program.declarations[0] else {
        panic!("expected type declaration");
    };
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            TypeMember::Method(method) => Some(method),
            _ => None,
        })
        .expect("expected get method");

    assert_eq!(
        method.body,
        Expr::BaseCall {
            span: Span::default(),
            args: vec![]
        }
    );
}

#[test]
fn case_g_method_full_form_block() {
    let program = parse_ok("type A { run() { print(1); print(2); } }\n\nnew A().run();");

    let Decl::Type(ty) = &program.declarations[0] else {
        panic!("expected type declaration");
    };

    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            TypeMember::Method(method) => Some(method),
            _ => None,
        })
        .expect("expected run method");

    match &method.body {
        Expr::Block(exprs) => assert_eq!(exprs.len(), 2),
        other => panic!("expected block method body, got {:?}", other),
    }
}

#[test]
fn case_h_invalid_attribute_syntax() {
    let err = parse_hulk_types_program("type A { x Number = 1; }");
    assert!(err.is_err());
}

#[test]
fn parses_member_and_method_from_pratt() {
    let program = parse_ok("let p = new A() in p.x @ p.getX();");

    match program.entry {
        Expr::Let { body, .. } => match *body {
            Expr::Binary {
                op: BinaryOp::Concat,
                ..
            } => {}
            other => panic!("expected concat expression, got {:?}", other),
        },
        other => panic!("expected let entry, got {:?}", other),
    }
}

// ──────────────────────────────────────────────
// T2.2: TypeMemberList recovery
// ──────────────────────────────────────────────

#[test]
fn recovery_bad_type_member_reports_one_error() {
    // `x: 42 = 0;` — `42` is not a valid type identifier.
    // The bad member is skipped; `y` should parse cleanly.
    let err = parse_hulk_types_program("type A {\n    x: 42 = 0;\n    y: Number = 1;\n}\nnew A();")
        .expect_err("should fail due to bad type member");
    let FrontendError::ParseErrors(list) = err else {
        panic!("expected ParseErrors, got: {:?}", err);
    };
    assert_eq!(
        list.errors().len(),
        1,
        "expected exactly one error, got {:?}",
        list.errors()
    );
}

#[test]
fn recovery_two_bad_type_members_report_two_errors() {
    let err = parse_hulk_types_program(
        "type A {\n    x: 42 = 0;\n    y: 99 = 1;\n    z: Number = 2;\n}\nnew A();",
    )
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

// ──────────────────────────────────────────────
// T2.3: LetBindingTail recovery
// ──────────────────────────────────────────────

#[test]
fn recovery_bad_let_binding_reports_one_error() {
    // In `let x = 1, 42 = 2 in x`, the second binding `42 = 2` starts with a
    // number where an identifier is expected.  Recovery should skip to `in` and
    // allow the body to parse.
    let err = parse_hulk_types_program("let x = 1, 42 = 2 in x;")
        .expect_err("should fail due to bad let binding");
    let FrontendError::ParseErrors(list) = err else {
        panic!("expected ParseErrors, got: {:?}", err);
    };
    assert_eq!(
        list.errors().len(),
        1,
        "expected exactly one error, got {:?}",
        list.errors()
    );
}
