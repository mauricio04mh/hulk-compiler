use hulk_frontend::ast::{Decl, Expr, LetBinding, Param, ProtocolMethod, Span, TypeMember, TypeRef};
use hulk_frontend::parse_hulk_full_program;

fn parse_ok(source: &str) -> hulk_frontend::ast::Program {
    parse_hulk_full_program(source).expect("source should parse into AST")
}

fn parse_err(source: &str) {
    assert!(
        parse_hulk_full_program(source).is_err(),
        "expected parse error for: {}",
        source
    );
}

// ──────────────────────────────────────────────
// Task 1: let binding type annotations
// ──────────────────────────────────────────────

#[test]
fn let_binding_simple_type_annotation() {
    let program = parse_ok("let x: Number = 42 in x;");
    match program.entry {
        Expr::Let { bindings, .. } => {
            assert_eq!(bindings[0].name, "x");
            assert_eq!(bindings[0].ty, Some(TypeRef::Simple("Number".to_string())));
        }
        other => panic!("expected let, got {:?}", other),
    }
}

#[test]
fn let_binding_no_type_annotation() {
    let program = parse_ok("let x = 42 in x;");
    match program.entry {
        Expr::Let { bindings, .. } => {
            assert_eq!(bindings[0].ty, None);
        }
        other => panic!("expected let, got {:?}", other),
    }
}

#[test]
fn let_binding_multiple_with_types() {
    let program = parse_ok("let x: Number = 1, y: Boolean = true in x;");
    match program.entry {
        Expr::Let { bindings, .. } => {
            assert_eq!(bindings.len(), 2);
            assert_eq!(bindings[0].ty, Some(TypeRef::Simple("Number".to_string())));
            assert_eq!(bindings[1].ty, Some(TypeRef::Simple("Boolean".to_string())));
        }
        other => panic!("expected let, got {:?}", other),
    }
}

// ──────────────────────────────────────────────
// Task 2: for loop
// ──────────────────────────────────────────────

#[test]
fn for_loop_basic() {
    let program = parse_ok("for (x in range(0, 10)) print(x);");
    match program.entry {
        Expr::For { var, iterable, body, .. } => {
            assert_eq!(var, "x");
            match *iterable {
                Expr::Call { .. } => {}
                other => panic!("expected call as iterable, got {:?}", other),
            }
            match *body {
                Expr::Call { .. } => {}
                other => panic!("expected call as body, got {:?}", other),
            }
        }
        other => panic!("expected for, got {:?}", other),
    }
}

#[test]
fn for_loop_block_body() {
    let program = parse_ok("for (i in items) { print(i); };");
    match program.entry {
        Expr::For { var, body, .. } => {
            assert_eq!(var, "i");
            assert!(matches!(*body, Expr::Block(_)));
        }
        other => panic!("expected for, got {:?}", other),
    }
}

#[test]
fn for_loop_nested() {
    let program = parse_ok("for (x in xs) for (y in ys) print(x);");
    match program.entry {
        Expr::For { body, .. } => {
            assert!(matches!(*body, Expr::For { .. }));
        }
        other => panic!("expected for, got {:?}", other),
    }
}

// ──────────────────────────────────────────────
// Task 3: is / as operators
// ──────────────────────────────────────────────

#[test]
fn type_test_is_operator() {
    let program = parse_ok("x is Bird;");
    match program.entry {
        Expr::TypeTest { expr, type_name, .. } => {
            assert_eq!(*expr, Expr::Var("x".to_string(), Span::default()));
            assert_eq!(type_name, "Bird");
        }
        other => panic!("expected TypeTest, got {:?}", other),
    }
}

#[test]
fn type_cast_as_operator() {
    let program = parse_ok("x as Dog;");
    match program.entry {
        Expr::TypeCast { expr, type_name, .. } => {
            assert_eq!(*expr, Expr::Var("x".to_string(), Span::default()));
            assert_eq!(type_name, "Dog");
        }
        other => panic!("expected TypeCast, got {:?}", other),
    }
}

#[test]
fn is_lower_precedence_than_arithmetic() {
    // (a + b) is Number
    let program = parse_ok("a + b is Number;");
    match program.entry {
        Expr::TypeTest { expr, type_name, .. } => {
            assert!(matches!(*expr, Expr::Binary { .. }));
            assert_eq!(type_name, "Number");
        }
        other => panic!("expected TypeTest, got {:?}", other),
    }
}

#[test]
fn as_in_let_binding() {
    let program = parse_ok("let b: Dog = animal as Dog in b;");
    match program.entry {
        Expr::Let { bindings, .. } => {
            assert_eq!(bindings[0].ty, Some(TypeRef::Simple("Dog".to_string())));
            assert!(matches!(bindings[0].value, Expr::TypeCast { .. }));
        }
        other => panic!("expected let, got {:?}", other),
    }
}

// ──────────────────────────────────────────────
// Task 4: Extended TypeRef
// ──────────────────────────────────────────────

#[test]
fn typeref_iterable_star() {
    let program = parse_ok("function f(xs: Number*): Number => 0;  0;");
    let Decl::Function(func) = &program.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(
        func.params[0].ty,
        Some(TypeRef::Iterable(Box::new(TypeRef::Simple("Number".to_string()))))
    );
}

#[test]
fn typeref_vector_brackets() {
    let program = parse_ok("function f(v: Number[]): Number => 0;  0;");
    let Decl::Function(func) = &program.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(
        func.params[0].ty,
        Some(TypeRef::Vector(Box::new(TypeRef::Simple("Number".to_string()))))
    );
}

#[test]
fn typeref_functor_type() {
    let program = parse_ok("function apply(f: (Number) -> Boolean): Boolean => f(0);  apply((x) => x > 0);");
    let Decl::Function(func) = &program.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(
        func.params[0].ty,
        Some(TypeRef::Functor {
            params: vec![TypeRef::Simple("Number".to_string())],
            ret: Box::new(TypeRef::Simple("Boolean".to_string())),
        })
    );
}

#[test]
fn return_type_vector() {
    let program = parse_ok("function squares(): Number[] => [1, 4, 9];  squares();");
    let Decl::Function(func) = &program.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(
        func.return_type,
        Some(TypeRef::Vector(Box::new(TypeRef::Simple("Number".to_string()))))
    );
}

// ──────────────────────────────────────────────
// Task 5: Vectors
// ──────────────────────────────────────────────

#[test]
fn vector_literal_empty() {
    let program = parse_ok("[];");
    assert_eq!(program.entry, Expr::VectorLiteral(vec![]));
}

#[test]
fn vector_literal_elements() {
    let program = parse_ok("[1, 2, 3];");
    match program.entry {
        Expr::VectorLiteral(elements) => {
            assert_eq!(elements.len(), 3);
            assert_eq!(elements[0], Expr::Number(1.0));
            assert_eq!(elements[1], Expr::Number(2.0));
            assert_eq!(elements[2], Expr::Number(3.0));
        }
        other => panic!("expected VectorLiteral, got {:?}", other),
    }
}

#[test]
fn vector_generator_basic() {
    let program = parse_ok("[x | x in range(0, 5)];");
    match program.entry {
        Expr::VectorGenerator { body, var, iterable, .. } => {
            assert_eq!(var, "x");
            assert_eq!(*body, Expr::Var("x".to_string(), Span::default()));
            assert!(matches!(*iterable, Expr::Call { .. }));
        }
        other => panic!("expected VectorGenerator, got {:?}", other),
    }
}

#[test]
fn vector_generator_with_expression_body() {
    let program = parse_ok("[x * x | x in range(1, 10)];");
    match program.entry {
        Expr::VectorGenerator { body, var, .. } => {
            assert_eq!(var, "x");
            assert!(matches!(*body, Expr::Binary { .. }));
        }
        other => panic!("expected VectorGenerator, got {:?}", other),
    }
}

#[test]
fn vector_index_basic() {
    let program = parse_ok("v[0];");
    match program.entry {
        Expr::VectorIndex { vector, index, .. } => {
            assert_eq!(*vector, Expr::Var("v".to_string(), Span::default()));
            assert_eq!(*index, Expr::Number(0.0));
        }
        other => panic!("expected VectorIndex, got {:?}", other),
    }
}

#[test]
fn vector_index_chained() {
    let program = parse_ok("matrix[i][j];");
    match program.entry {
        Expr::VectorIndex { vector, index, .. } => {
            assert_eq!(*index, Expr::Var("j".to_string(), Span::default()));
            assert!(matches!(*vector, Expr::VectorIndex { .. }));
        }
        other => panic!("expected VectorIndex, got {:?}", other),
    }
}

#[test]
fn vector_index_higher_precedence_than_add() {
    // v[i] + 1 should parse as (v[i]) + 1
    let program = parse_ok("v[i] + 1;");
    match program.entry {
        Expr::Binary { left, .. } => {
            assert!(matches!(*left, Expr::VectorIndex { .. }));
        }
        other => panic!("expected Binary, got {:?}", other),
    }
}

// ──────────────────────────────────────────────
// Task 6: Lambda expressions
// ──────────────────────────────────────────────

#[test]
fn lambda_no_params() {
    let program = parse_ok("() => 42;");
    match program.entry {
        Expr::Lambda { params, body, .. } => {
            assert!(params.is_empty());
            assert_eq!(*body, Expr::Number(42.0));
        }
        other => panic!("expected Lambda, got {:?}", other),
    }
}

#[test]
fn lambda_single_param_no_type() {
    let program = parse_ok("(x) => x + 1;");
    match program.entry {
        Expr::Lambda { params, body, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[0].ty, None);
            assert!(matches!(*body, Expr::Binary { .. }));
        }
        other => panic!("expected Lambda, got {:?}", other),
    }
}

#[test]
fn lambda_single_param_with_type() {
    let program = parse_ok("(x: Number) => x * 2;");
    match program.entry {
        Expr::Lambda { params, .. } => {
            assert_eq!(params[0].ty, Some(TypeRef::Simple("Number".to_string())));
        }
        other => panic!("expected Lambda, got {:?}", other),
    }
}

#[test]
fn lambda_multiple_params() {
    let program = parse_ok("(x: Number, y: Number) => x + y;");
    match program.entry {
        Expr::Lambda { params, .. } => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[1].name, "y");
        }
        other => panic!("expected Lambda, got {:?}", other),
    }
}

#[test]
fn lambda_with_return_type() {
    let program = parse_ok("(x: Number): Boolean => x > 0;");
    match program.entry {
        Expr::Lambda { return_type, .. } => {
            assert_eq!(return_type, Some(TypeRef::Simple("Boolean".to_string())));
        }
        other => panic!("expected Lambda, got {:?}", other),
    }
}

#[test]
fn lambda_as_function_argument() {
    let program = parse_ok("filter(items, (x) => x > 0);");
    match program.entry {
        Expr::Call { args, .. } => {
            assert_eq!(args.len(), 2);
            assert!(matches!(&args[1], Expr::Lambda { .. }));
        }
        other => panic!("expected Call, got {:?}", other),
    }
}

#[test]
fn grouped_expr_not_mistaken_for_lambda() {
    let program = parse_ok("(1 + 2) * 3;");
    match program.entry {
        Expr::Binary { .. } => {}
        other => panic!("expected Binary, got {:?}", other),
    }
}

// ──────────────────────────────────────────────
// Task 7: Protocol declarations
// ──────────────────────────────────────────────

#[test]
fn protocol_empty() {
    let program = parse_ok("protocol Printable { }  0;");
    assert_eq!(program.declarations.len(), 1);
    let Decl::Protocol(proto) = &program.declarations[0] else {
        panic!("expected Protocol declaration");
    };
    assert_eq!(proto.name, "Printable");
    assert!(proto.parent.is_none());
    assert!(proto.methods.is_empty());
}

#[test]
fn protocol_with_methods() {
    let program = parse_ok(
        "protocol Hashable { hash(): Number; equals(other: Object): Boolean; }  0;",
    );
    let Decl::Protocol(proto) = &program.declarations[0] else {
        panic!("expected Protocol declaration");
    };
    assert_eq!(proto.name, "Hashable");
    assert_eq!(proto.methods.len(), 2);

    assert_eq!(proto.methods[0].name, "hash");
    assert!(proto.methods[0].params.is_empty());
    assert_eq!(
        proto.methods[0].return_type,
        Some(TypeRef::Simple("Number".to_string()))
    );

    assert_eq!(proto.methods[1].name, "equals");
    assert_eq!(proto.methods[1].params.len(), 1);
    assert_eq!(proto.methods[1].params[0].name, "other");
}

#[test]
fn protocol_with_parent() {
    let program = parse_ok("protocol Equatable extends Hashable { equals(other: Object): Boolean; }  0;");
    let Decl::Protocol(proto) = &program.declarations[0] else {
        panic!("expected Protocol declaration");
    };
    assert_eq!(proto.name, "Equatable");
    assert_eq!(proto.parent, Some("Hashable".to_string()));
    assert_eq!(proto.methods.len(), 1);
}

#[test]
fn multiple_protocols() {
    let program = parse_ok(
        "protocol A { f(): Number; }  protocol B extends A { g(): Boolean; }  0;",
    );
    assert_eq!(program.declarations.len(), 2);
    assert!(matches!(&program.declarations[0], Decl::Protocol(_)));
    assert!(matches!(&program.declarations[1], Decl::Protocol(_)));
}

// ──────────────────────────────────────────────
// Combined features
// ──────────────────────────────────────────────

#[test]
fn for_loop_over_vector_literal() {
    let program = parse_ok("for (x in [1, 2, 3]) print(x);");
    match program.entry {
        Expr::For { iterable, .. } => {
            assert!(matches!(*iterable, Expr::VectorLiteral(_)));
        }
        other => panic!("expected for, got {:?}", other),
    }
}

#[test]
fn vector_generator_with_type_test() {
    let program = parse_ok("[x | x in animals];");
    match program.entry {
        Expr::VectorGenerator { var, .. } => {
            assert_eq!(var, "x");
        }
        other => panic!("expected VectorGenerator, got {:?}", other),
    }
}

#[test]
fn lambda_with_vector_return_type() {
    let program = parse_ok("(n: Number): Number[] => [n, n * 2];");
    match program.entry {
        Expr::Lambda { return_type, body, .. } => {
            assert_eq!(
                return_type,
                Some(TypeRef::Vector(Box::new(TypeRef::Simple("Number".to_string()))))
            );
            assert!(matches!(*body, Expr::VectorLiteral(_)));
        }
        other => panic!("expected Lambda, got {:?}", other),
    }
}

#[test]
fn lambda_param_with_nullary_functor_type() {
    // (f: () -> Number) => f()
    let program = parse_ok("(f: () -> Number) => f();");
    match program.entry {
        Expr::Lambda { params, .. } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "f");
            assert_eq!(
                params[0].ty,
                Some(TypeRef::Functor {
                    params: vec![],
                    ret: Box::new(TypeRef::Simple("Number".to_string())),
                })
            );
        }
        other => panic!("expected Lambda, got {:?}", other),
    }
}

#[test]
fn lambda_param_with_unary_functor_type() {
    // (f: (Number) -> Boolean) => f(42)
    let program = parse_ok("(f: (Number) -> Boolean) => f(42);");
    match program.entry {
        Expr::Lambda { params, .. } => {
            assert_eq!(params[0].ty, Some(TypeRef::Functor {
                params: vec![TypeRef::Simple("Number".to_string())],
                ret: Box::new(TypeRef::Simple("Boolean".to_string())),
            }));
        }
        other => panic!("expected Lambda, got {:?}", other),
    }
}

#[test]
fn lambda_param_with_binary_functor_type() {
    // (f: (Number, Number) -> Number) => f(1, 2)
    let program = parse_ok("(f: (Number, Number) -> Number) => f(1, 2);");
    match program.entry {
        Expr::Lambda { params, .. } => {
            assert_eq!(params[0].ty, Some(TypeRef::Functor {
                params: vec![
                    TypeRef::Simple("Number".to_string()),
                    TypeRef::Simple("Number".to_string()),
                ],
                ret: Box::new(TypeRef::Simple("Number".to_string())),
            }));
        }
        other => panic!("expected Lambda, got {:?}", other),
    }
}

#[test]
fn lambda_return_type_is_functor() {
    // (): (Number) -> Number => (x: Number) => x
    let program = parse_ok("(): (Number) -> Number => (x: Number) => x;");
    match program.entry {
        Expr::Lambda { return_type, .. } => {
            assert_eq!(return_type, Some(TypeRef::Functor {
                params: vec![TypeRef::Simple("Number".to_string())],
                ret: Box::new(TypeRef::Simple("Number".to_string())),
            }));
        }
        other => panic!("expected Lambda, got {:?}", other),
    }
}

#[test]
fn let_with_annotated_type_and_cast() {
    let program = parse_ok("let b: Bird = animal as Bird in b;");
    match &program.entry {
        Expr::Let { bindings, .. } => {
            assert_eq!(bindings[0].ty, Some(TypeRef::Simple("Bird".to_string())));
            assert!(matches!(&bindings[0].value, Expr::TypeCast { .. }));
        }
        other => panic!("expected let, got {:?}", other),
    }
}

// ──────────────────────────────────────────────
// Task 8 (OOP): types, new, self, methods via parse_hulk_full_program
// ──────────────────────────────────────────────

#[test]
fn oop_new_no_args() {
    let program = parse_ok("type A {}  new A();");
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
fn oop_new_with_args() {
    let program = parse_ok("type Point(x: Number, y: Number) { x: Number = x; y: Number = y; }  new Point(3, 4);");
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
fn oop_member_access() {
    let program = parse_ok("type A { v: Number = 1; }  let a = new A() in a.v;");
    match program.entry {
        Expr::Let { body, .. } => {
            assert!(matches!(*body, Expr::MemberAccess { .. }));
        }
        other => panic!("expected let, got {:?}", other),
    }
}

#[test]
fn oop_method_call() {
    let program = parse_ok("type A { get(): Number => 1; }  let a = new A() in a.get();");
    match program.entry {
        Expr::Let { body, .. } => match *body {
            Expr::MethodCall { ref method, .. } => assert_eq!(method, "get"),
            other => panic!("expected method call, got {:?}", other),
        },
        other => panic!("expected let, got {:?}", other),
    }
}

#[test]
fn oop_chained_new_method() {
    let program = parse_ok("type A { get(): Number => 1; }  new A().get();");
    match program.entry {
        Expr::MethodCall { object, method, args, .. } => {
            assert_eq!(method, "get");
            assert!(args.is_empty());
            assert!(matches!(*object, Expr::New { .. }));
        }
        other => panic!("expected method call, got {:?}", other),
    }
}

#[test]
fn oop_self_member_access_in_method() {
    let program = parse_ok("type A { x: Number = 1; getX(): Number => self.x; }  new A();");
    let Decl::Type(ty) = &program.declarations[0] else {
        panic!("expected type declaration");
    };
    let method = ty.members.iter().find_map(|m| {
        if let TypeMember::Method(method) = m { Some(method) } else { None }
    }).expect("expected getX method");
    assert_eq!(
        method.body,
        Expr::MemberAccess {
            span: Span::default(),
            object: Box::new(Expr::SelfRef),
            member: "x".to_string(),
        }
    );
}

#[test]
fn oop_base_call_in_method() {
    let program = parse_ok("type B inherits A { get(): Number => base(); }  new B();");
    let Decl::Type(ty) = &program.declarations[0] else {
        panic!("expected type declaration");
    };
    let method = ty.members.iter().find_map(|m| {
        if let TypeMember::Method(method) = m { Some(method) } else { None }
    }).expect("expected get method");
    assert_eq!(method.body, Expr::BaseCall { span: Span::default(), args: vec![] });
}

#[test]
fn oop_type_with_inheritance() {
    let program = parse_ok("type B inherits A { }  new B();");
    let Decl::Type(ty) = &program.declarations[0] else {
        panic!("expected type declaration");
    };
    assert_eq!(ty.name, "B");
    let parent = ty.parent.as_ref().expect("expected parent");
    assert_eq!(parent.name, "A");
}

#[test]
fn oop_type_declaration_carries_name_span() {
    let program = parse_ok("type MyType {}  new MyType();");
    let Decl::Type(ty) = &program.declarations[0] else {
        panic!("expected type declaration");
    };
    assert_eq!(ty.name, "MyType");
    assert!(ty.name_span.line > 0);
}

#[test]
fn oop_function_carries_name_span() {
    let program = parse_ok("function greet(): Number => 42;  greet();");
    let Decl::Function(func) = &program.declarations[0] else {
        panic!("expected function declaration");
    };
    assert_eq!(func.name, "greet");
    assert!(func.name_span.line > 0);
}

#[test]
fn oop_protocol_carries_name_span() {
    let program = parse_ok("protocol Walkable { walk(): Number; }  0;");
    let Decl::Protocol(proto) = &program.declarations[0] else {
        panic!("expected protocol declaration");
    };
    assert_eq!(proto.name, "Walkable");
    assert!(proto.name_span.line > 0);
}
