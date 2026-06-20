use hulk_frontend::parse_hulk_types_program;
use hulk_sema::error::SemanticError;
use hulk_sema::hir::{
    DispatchKind, HirCallee, HirDecl, HirExpr, HirExprKind, HirLetBinding, ResolvedMember,
};
use hulk_sema::types::Type;
use hulk_sema::{analyze_program, check_program};

#[test]
fn analyze_program_builds_semantic_program_for_number_literal() {
    let program = parse_hulk_types_program("42;").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert!(semantic.hir.declarations.is_empty());
    assert_eq!(semantic.hir.entry.ty, Type::Number);
    assert!(matches!(semantic.hir.entry.kind, HirExprKind::Number(42.0)));
    assert!(semantic.functions.contains_key("print"));
}

#[test]
fn analyze_program_builds_semantic_program_for_string_literal() {
    let program = parse_hulk_types_program("\"x\";").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::String);
    assert!(matches!(semantic.hir.entry.kind, HirExprKind::String(ref value) if value == "x"));
}

#[test]
fn analyze_program_builds_semantic_program_for_boolean_literal() {
    let program = parse_hulk_types_program("true;").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Boolean);
    assert!(matches!(semantic.hir.entry.kind, HirExprKind::Bool(true)));
}

#[test]
fn analyze_program_preserves_let_binding_symbol_in_var_use() {
    let program =
        parse_hulk_types_program("let x: Number = 5 in x + 1;").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Number);
    let HirExprKind::Let { bindings, body } = &semantic.hir.entry.kind else {
        panic!("entry should be a let expression");
    };
    let binding_symbol = bindings[0].symbol;
    let HirExprKind::Binary { left, .. } = &body.kind else {
        panic!("let body should be a binary expression");
    };
    let HirExprKind::Var { symbol, .. } = left.kind else {
        panic!("left side should be a var");
    };

    assert_eq!(binding_symbol, symbol);
}

#[test]
fn analyze_program_resolves_global_function_call() {
    let program = parse_hulk_types_program(
        "function inc(x: Number): Number => x + 1;
        inc(4);",
    )
    .expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Number);
    let HirExprKind::Call { callee, .. } = &semantic.hir.entry.kind else {
        panic!("entry should be a call");
    };
    assert!(matches!(callee, HirCallee::GlobalFunction { name, .. } if name == "inc"));
}

#[test]
fn analyze_program_resolves_builtin_function_call() {
    let program = parse_hulk_types_program("print(42);").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    let HirExprKind::Call { callee, .. } = &semantic.hir.entry.kind else {
        panic!("entry should be a call");
    };
    assert!(matches!(callee, HirCallee::Builtin { name, .. } if name == "print"));
}

#[test]
fn analyze_program_if_unifies_number_branches() {
    let program = parse_hulk_types_program("if (true) 1 else 2;").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Number);
    assert!(matches!(semantic.hir.entry.kind, HirExprKind::If { .. }));
}

#[test]
fn analyze_program_if_reports_primitive_branch_mismatch() {
    let program = parse_hulk_types_program("if (true) 1 else \"x\";").expect("source should parse");
    let errors = analyze_program(&program).expect_err("analysis should fail");

    assert!(
        errors
            .iter()
            .any(|error| matches!(error, SemanticError::TypeMismatch { .. }))
    );
}

#[test]
fn analyze_program_while_uses_body_type() {
    let program = parse_hulk_types_program("let x: Number = 3 in while (x > 0) x := x - 1;")
        .expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Number);
    let HirExprKind::Let { body, .. } = &semantic.hir.entry.kind else {
        panic!("entry should be a let expression");
    };
    assert!(matches!(body.kind, HirExprKind::While { .. }));
}

#[test]
fn analyze_program_for_over_vector_binds_element_type() {
    let program =
        parse_hulk_types_program("for (x in [1, 2, 3]) x + 1;").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Number);
    assert!(!contains_for(&semantic.hir.entry));
    let binding = find_binding(&semantic.hir.entry, "x").expect("loop binding should exist");
    assert_eq!(binding.ty, Type::Number);
}

#[test]
fn analyze_program_for_desugaring_preserves_body_semantics() {
    let program =
        parse_hulk_types_program("for (x in [1, 2, 3]) x + 1;").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Number);
    let HirExprKind::Let { bindings, body } = &semantic.hir.entry.kind else {
        panic!("for should desugar to an outer let");
    };
    assert!(bindings[0].name.starts_with("_iter$"));
    let HirExprKind::While { condition, body } = &body.kind else {
        panic!("outer let body should be a while");
    };
    assert!(matches!(
        condition.kind,
        HirExprKind::MethodCall { ref method, dispatch: DispatchKind::Virtual { .. }, .. }
            if method == "next"
    ));
    let HirExprKind::Let {
        bindings: loop_bindings,
        body: loop_body,
    } = &body.kind
    else {
        panic!("while body should bind the loop variable");
    };
    assert_eq!(loop_bindings[0].name, "x");
    assert_eq!(loop_bindings[0].ty, Type::Number);
    assert!(matches!(
        loop_bindings[0].value.kind,
        HirExprKind::MethodCall { ref method, dispatch: DispatchKind::Virtual { .. }, .. }
            if method == "current"
    ));
    assert!(matches!(loop_body.kind, HirExprKind::Binary { .. }));
}

#[test]
fn analyze_program_for_internal_iterator_does_not_collide_with_user_iter() {
    let program = parse_hulk_types_program("let iter = 10 in for (x in [1]) iter + x;")
        .expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Number);
    assert!(!contains_for(&semantic.hir.entry));
    let user_binding =
        find_binding(&semantic.hir.entry, "iter").expect("user binding should exist");
    let internal_binding =
        find_internal_iter_binding(&semantic.hir.entry).expect("internal iter should exist");
    assert_ne!(user_binding.symbol, internal_binding.symbol);
    assert_ne!(user_binding.name, internal_binding.name);
}

#[test]
fn analyze_program_for_over_number_reports_invalid_iterable_target() {
    let program = parse_hulk_types_program("for (x in 1) x;").expect("source should parse");
    let errors = analyze_program(&program).expect_err("analysis should fail");

    assert!(errors.iter().any(|error| {
        matches!(error, SemanticError::InvalidIterableTarget { found } if *found == Type::Number)
    }));
}

#[test]
fn analyze_program_resolves_self_member_access_and_virtual_method_call() {
    let program = parse_hulk_types_program(
        "type A {
            x: Number = 1;
            f(): Number => self.x;
        }
        new A().f();",
    )
    .expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    let HirDecl::Type(type_decl) = &semantic.hir.declarations[0] else {
        panic!("declaration should be a type");
    };
    assert_eq!(type_decl.methods[0].owner_type, "A");
    assert_eq!(type_decl.methods[0].body.ty, Type::Number);
    let HirExprKind::MemberAccess {
        object, resolved, ..
    } = &type_decl.methods[0].body.kind
    else {
        panic!("method body should be member access");
    };
    assert_eq!(object.ty, Type::UserType("A".to_string()));
    assert!(matches!(object.kind, HirExprKind::SelfRef { ref type_name, .. } if type_name == "A"));
    assert!(matches!(
        resolved,
        ResolvedMember::Attribute { owner_type, attr_name, ty }
            if owner_type == "A" && attr_name == "x" && *ty == Type::Number
    ));

    let HirExprKind::MethodCall { dispatch, .. } = &semantic.hir.entry.kind else {
        panic!("entry should be a method call");
    };
    assert!(matches!(
        dispatch,
        DispatchKind::Virtual { receiver_static_type, method_name, signature }
            if *receiver_static_type == Type::UserType("A".to_string())
                && method_name == "f"
                && signature.return_type == Type::Number
    ));
}

#[test]
fn analyze_program_resolves_base_call_to_parent_method() {
    let program = parse_hulk_types_program(
        "type A {
            f(): Number => 1;
        }
        type B inherits A {
            f(): Number => base() + 1;
        }
        new B().f();",
    )
    .expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    let HirDecl::Type(type_decl) = &semantic.hir.declarations[1] else {
        panic!("second declaration should be a type");
    };
    let base_call = find_base_call(&type_decl.methods[0].body).expect("base call should exist");
    assert_eq!(base_call.ty, Type::Number);
    assert!(matches!(
        &base_call.kind,
        HirExprKind::BaseCall { parent_type, method_name, .. }
            if parent_type == "A" && method_name == "f"
    ));
}

#[test]
fn analyze_program_external_attribute_access_reports_private() {
    let program = parse_hulk_types_program(
        "type A {
            x: Number = 1;
        }
        let a = new A() in a.x;",
    )
    .expect("source should parse");
    let errors = analyze_program(&program).expect_err("analysis should fail");

    assert!(errors.iter().any(|error| {
        matches!(error, SemanticError::AttributeIsPrivate { type_name, attr_name }
            if type_name == "A" && attr_name == "x")
    }));
}

#[test]
fn analyze_program_new_wrong_arity_reports_error() {
    let program =
        parse_hulk_types_program("type A(x: Number) {}\nnew A();").expect("source should parse");
    let errors = analyze_program(&program).expect_err("analysis should fail");

    assert!(errors.iter().any(|error| {
        matches!(error, SemanticError::ArityMismatch { function, expected, found }
            if function == "A" && *expected == 1 && *found == 0)
    }));
}

#[test]
fn analyze_program_unknown_method_reports_error() {
    let program =
        parse_hulk_types_program("type A {}\nnew A().missing();").expect("source should parse");
    let errors = analyze_program(&program).expect_err("analysis should fail");

    assert!(errors.iter().any(|error| {
        matches!(error, SemanticError::UndefinedMethod { type_name, method_name }
            if type_name == "A" && method_name == "missing")
    }));
}

#[test]
fn analyze_program_vector_literal_numbers_has_number_element_type() {
    let program = parse_hulk_types_program("[1, 2, 3];").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Vector(Box::new(Type::Number)));
    assert!(matches!(
        semantic.hir.entry.kind,
        HirExprKind::VectorLiteral {
            element_type: Type::Number,
            ..
        }
    ));
}

#[test]
fn analyze_program_vector_literal_mixed_unifies_to_object() {
    let program = parse_hulk_types_program("[1, \"x\"];").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Vector(Box::new(Type::Object)));
    assert!(matches!(
        semantic.hir.entry.kind,
        HirExprKind::VectorLiteral {
            element_type: Type::Object,
            ..
        }
    ));
}

#[test]
fn analyze_program_vector_index_returns_element_type() {
    let program = parse_hulk_types_program("let v = [1, 2] in v[0];").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Number);
    let HirExprKind::Let { body, .. } = &semantic.hir.entry.kind else {
        panic!("entry should be a let expression");
    };
    assert!(matches!(
        body.kind,
        HirExprKind::VectorIndex {
            element_type: Type::Number,
            ..
        }
    ));
}

#[test]
fn analyze_program_vector_index_non_number_reports_type_mismatch() {
    let program =
        parse_hulk_types_program("let v = [1, 2] in v[\"x\"];").expect("source should parse");
    let errors = analyze_program(&program).expect_err("analysis should fail");

    assert!(errors.iter().any(|error| {
        matches!(error, SemanticError::TypeMismatch { expected, found }
            if *expected == Type::Number && *found == Type::String)
    }));
}

#[test]
fn analyze_program_lambda_has_functor_type() {
    let program =
        parse_hulk_types_program("(x: Number): Number => x + 1;").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(
        semantic.hir.entry.ty,
        Type::Functor {
            params: vec![Type::Number],
            ret: Box::new(Type::Number),
        }
    );
    assert!(matches!(
        semantic.hir.entry.kind,
        HirExprKind::Lambda {
            return_type: Type::Number,
            ..
        }
    ));
}

#[test]
fn analyze_program_type_test_returns_boolean() {
    let program = parse_hulk_types_program("type A {}\nlet x: Object = new A() in x is A;")
        .expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Boolean);
    let HirExprKind::Let { body, .. } = &semantic.hir.entry.kind else {
        panic!("entry should be a let expression");
    };
    assert!(matches!(body.kind, HirExprKind::TypeTest { ref type_name, .. } if type_name == "A"));
}

#[test]
fn analyze_program_type_cast_returns_target_user_type() {
    let program = parse_hulk_types_program("type A {}\nlet x: Object = new A() in x as A;")
        .expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::UserType("A".to_string()));
    let HirExprKind::Let { body, .. } = &semantic.hir.entry.kind else {
        panic!("entry should be a let expression");
    };
    assert!(matches!(body.kind, HirExprKind::TypeCast { ref type_name, .. } if type_name == "A"));
}

#[test]
fn analyze_program_vector_generator_produces_vector_of_body_type() {
    let program = parse_hulk_types_program("[x + 1 | x in [1, 2]];").expect("source should parse");
    let semantic = analyze_program(&program).expect("analysis should pass");

    assert_eq!(semantic.hir.entry.ty, Type::Vector(Box::new(Type::Number)));
    assert!(matches!(
        semantic.hir.entry.kind,
        HirExprKind::VectorGenerator { element_type: Type::Number, ref var, .. } if var.ty == Type::Number
    ));
}

#[test]
fn check_program_and_analyze_program_both_accept_valid_program() {
    let program =
        parse_hulk_types_program("let x: Number = 1 in x + 1;").expect("source should parse");

    analyze_program(&program).expect("analysis should pass");
    check_program(&program).expect("check should pass");
}

#[test]
fn migrated_check_program_reports_undefined_variable() {
    assert_analysis_and_check_error(
        "x;",
        |error| matches!(error, SemanticError::UndefinedVariable { name } if name == "x"),
    );
}

#[test]
fn migrated_check_program_reports_undefined_function() {
    assert_analysis_and_check_error(
        "missing();",
        |error| matches!(error, SemanticError::UndefinedFunction { name } if name == "missing"),
    );
}

#[test]
fn migrated_check_program_reports_arity_mismatch() {
    assert_analysis_and_check_error("print();", |error| {
        matches!(error, SemanticError::ArityMismatch { function, expected, found }
            if function == "print" && *expected == 1 && *found == 0)
    });
}

#[test]
fn migrated_check_program_reports_type_mismatch() {
    assert_analysis_and_check_error("let x: Number = \"x\" in x;", |error| {
        matches!(error, SemanticError::TypeMismatch { expected, found }
            if *expected == Type::Number && *found == Type::String)
    });
}

#[test]
fn migrated_check_program_reports_invalid_condition_type() {
    assert_analysis_and_check_error(
        "if (1) 2 else 3;",
        |error| matches!(error, SemanticError::InvalidConditionType { found } if *found == Type::Number),
    );
}

#[test]
fn migrated_check_program_reports_cannot_infer_parameter_type() {
    assert_analysis_and_check_error("function f(x) => x;\n1;", |error| {
        matches!(error, SemanticError::CannotInferParameterType { function, parameter }
            if function == "f" && parameter == "x")
    });
}

#[test]
fn migrated_check_program_reports_undefined_type() {
    assert_analysis_and_check_error(
        "new Ghost();",
        |error| matches!(error, SemanticError::UndefinedType { name } if name == "Ghost"),
    );
}

#[test]
fn migrated_check_program_reports_attribute_is_private() {
    assert_analysis_and_check_error(
        "type A { x: Number = 1; }\nlet a = new A() in a.x;",
        |error| {
            matches!(error, SemanticError::AttributeIsPrivate { type_name, attr_name }
                if type_name == "A" && attr_name == "x")
        },
    );
}

#[test]
fn migrated_check_program_reports_undefined_method() {
    assert_analysis_and_check_error("type A {}\nnew A().missing();", |error| {
        matches!(error, SemanticError::UndefinedMethod { type_name, method_name }
            if type_name == "A" && method_name == "missing")
    });
}

#[test]
fn migrated_check_program_reports_invalid_assignment_target() {
    assert_analysis_and_check_error("1 := 2;", |error| {
        matches!(error, SemanticError::InvalidAssignmentTarget)
    });
}

#[test]
fn analyze_program_accumulates_multiple_errors_when_possible() {
    let program = parse_hulk_types_program("let a: Number = \"x\", b: Boolean = 1 in a;")
        .expect("source should parse");
    let errors = analyze_program(&program).expect_err("analysis should fail");

    assert!(errors.len() >= 2);
    assert!(errors.iter().any(|error| {
        matches!(error, SemanticError::TypeMismatch { expected, found }
            if *expected == Type::Number && *found == Type::String)
    }));
    assert!(errors.iter().any(|error| {
        matches!(error, SemanticError::TypeMismatch { expected, found }
            if *expected == Type::Boolean && *found == Type::Number)
    }));
    check_program(&program).expect_err("check should fail when analysis fails");
}

fn find_base_call(expr: &HirExpr) -> Option<&HirExpr> {
    match &expr.kind {
        HirExprKind::BaseCall { .. } => Some(expr),
        HirExprKind::Binary { left, right, .. } => {
            find_base_call(left).or_else(|| find_base_call(right))
        }
        HirExprKind::Unary { expr, .. } => find_base_call(expr),
        HirExprKind::Let { bindings, body } => bindings
            .iter()
            .find_map(|binding| find_base_call(&binding.value))
            .or_else(|| find_base_call(body)),
        HirExprKind::Block { exprs } => exprs.iter().find_map(find_base_call),
        HirExprKind::If {
            branches,
            else_branch,
        } => branches
            .iter()
            .find_map(|(condition, body)| {
                find_base_call(condition).or_else(|| find_base_call(body))
            })
            .or_else(|| find_base_call(else_branch)),
        HirExprKind::While { condition, body } => {
            find_base_call(condition).or_else(|| find_base_call(body))
        }
        HirExprKind::For { iterable, body, .. } => {
            find_base_call(iterable).or_else(|| find_base_call(body))
        }
        HirExprKind::Call { args, .. } | HirExprKind::New { args, .. } => {
            args.iter().find_map(find_base_call)
        }
        HirExprKind::MemberAccess { object, .. } => find_base_call(object),
        HirExprKind::MethodCall { object, args, .. } => {
            find_base_call(object).or_else(|| args.iter().find_map(find_base_call))
        }
        HirExprKind::TypeTest { expr, .. } | HirExprKind::TypeCast { expr, .. } => {
            find_base_call(expr)
        }
        HirExprKind::VectorLiteral { elements, .. } => elements.iter().find_map(find_base_call),
        HirExprKind::VectorGenerator { body, iterable, .. } => {
            find_base_call(body).or_else(|| find_base_call(iterable))
        }
        HirExprKind::VectorNew { size, init, .. } => {
            find_base_call(size).or_else(|| init.as_ref().and_then(|i| find_base_call(&i.body)))
        }
        HirExprKind::VectorIndex { vector, index, .. } => {
            find_base_call(vector).or_else(|| find_base_call(index))
        }
        HirExprKind::Lambda { body, .. } => find_base_call(body),
        HirExprKind::Assign { value, .. } => find_base_call(value),
        HirExprKind::Number(_)
        | HirExprKind::String(_)
        | HirExprKind::Bool(_)
        | HirExprKind::Var { .. }
        | HirExprKind::SelfRef { .. } => None,
    }
}

fn contains_for(expr: &HirExpr) -> bool {
    match &expr.kind {
        HirExprKind::For { .. } => true,
        HirExprKind::Binary { left, right, .. } => contains_for(left) || contains_for(right),
        HirExprKind::Unary { expr, .. } => contains_for(expr),
        HirExprKind::Let { bindings, body } => {
            bindings.iter().any(|binding| contains_for(&binding.value)) || contains_for(body)
        }
        HirExprKind::Block { exprs } => exprs.iter().any(contains_for),
        HirExprKind::If {
            branches,
            else_branch,
        } => {
            branches
                .iter()
                .any(|(condition, body)| contains_for(condition) || contains_for(body))
                || contains_for(else_branch)
        }
        HirExprKind::While { condition, body } => contains_for(condition) || contains_for(body),
        HirExprKind::Call { args, .. } | HirExprKind::New { args, .. } => {
            args.iter().any(contains_for)
        }
        HirExprKind::MemberAccess { object, .. } => contains_for(object),
        HirExprKind::MethodCall { object, args, .. } => {
            contains_for(object) || args.iter().any(contains_for)
        }
        HirExprKind::TypeTest { expr, .. } | HirExprKind::TypeCast { expr, .. } => {
            contains_for(expr)
        }
        HirExprKind::VectorLiteral { elements, .. } => elements.iter().any(contains_for),
        HirExprKind::VectorGenerator { body, iterable, .. } => {
            contains_for(body) || contains_for(iterable)
        }
        HirExprKind::VectorNew { size, init, .. } => {
            contains_for(size) || init.as_ref().map_or(false, |i| contains_for(&i.body))
        }
        HirExprKind::VectorIndex { vector, index, .. } => {
            contains_for(vector) || contains_for(index)
        }
        HirExprKind::Lambda { body, .. } => contains_for(body),
        HirExprKind::Assign { value, .. } => contains_for(value),
        HirExprKind::Number(_)
        | HirExprKind::String(_)
        | HirExprKind::Bool(_)
        | HirExprKind::Var { .. }
        | HirExprKind::SelfRef { .. }
        | HirExprKind::BaseCall { .. } => false,
    }
}

fn find_binding<'a>(expr: &'a HirExpr, name: &str) -> Option<&'a HirLetBinding> {
    match &expr.kind {
        HirExprKind::Let { bindings, body } => {
            for binding in bindings {
                if binding.name == name {
                    return Some(binding);
                }
                if let Some(found) = find_binding(&binding.value, name) {
                    return Some(found);
                }
            }
            find_binding(body, name)
        }
        HirExprKind::Binary { left, right, .. } => {
            find_binding(left, name).or_else(|| find_binding(right, name))
        }
        HirExprKind::Unary { expr, .. } => find_binding(expr, name),
        HirExprKind::Block { exprs } => exprs.iter().find_map(|expr| find_binding(expr, name)),
        HirExprKind::If {
            branches,
            else_branch,
        } => branches
            .iter()
            .find_map(|(condition, body)| {
                find_binding(condition, name).or_else(|| find_binding(body, name))
            })
            .or_else(|| find_binding(else_branch, name)),
        HirExprKind::While { condition, body } => {
            find_binding(condition, name).or_else(|| find_binding(body, name))
        }
        HirExprKind::For { iterable, body, .. } => {
            find_binding(iterable, name).or_else(|| find_binding(body, name))
        }
        HirExprKind::Call { args, .. } | HirExprKind::New { args, .. } => {
            args.iter().find_map(|arg| find_binding(arg, name))
        }
        HirExprKind::MemberAccess { object, .. } => find_binding(object, name),
        HirExprKind::MethodCall { object, args, .. } => find_binding(object, name)
            .or_else(|| args.iter().find_map(|arg| find_binding(arg, name))),
        HirExprKind::TypeTest { expr, .. } | HirExprKind::TypeCast { expr, .. } => {
            find_binding(expr, name)
        }
        HirExprKind::VectorLiteral { elements, .. } => elements
            .iter()
            .find_map(|element| find_binding(element, name)),
        HirExprKind::VectorGenerator { body, iterable, .. } => {
            find_binding(body, name).or_else(|| find_binding(iterable, name))
        }
        HirExprKind::VectorNew { size, init, .. } => {
            find_binding(size, name)
                .or_else(|| init.as_ref().and_then(|i| find_binding(&i.body, name)))
        }
        HirExprKind::VectorIndex { vector, index, .. } => {
            find_binding(vector, name).or_else(|| find_binding(index, name))
        }
        HirExprKind::Lambda { body, .. } => find_binding(body, name),
        HirExprKind::Assign { value, .. } => find_binding(value, name),
        HirExprKind::Number(_)
        | HirExprKind::String(_)
        | HirExprKind::Bool(_)
        | HirExprKind::Var { .. }
        | HirExprKind::SelfRef { .. }
        | HirExprKind::BaseCall { .. } => None,
    }
}

fn find_internal_iter_binding(expr: &HirExpr) -> Option<&HirLetBinding> {
    match &expr.kind {
        HirExprKind::Let { bindings, body } => {
            for binding in bindings {
                if binding.name.starts_with("_iter$") {
                    return Some(binding);
                }
                if let Some(found) = find_internal_iter_binding(&binding.value) {
                    return Some(found);
                }
            }
            find_internal_iter_binding(body)
        }
        HirExprKind::Binary { left, right, .. } => {
            find_internal_iter_binding(left).or_else(|| find_internal_iter_binding(right))
        }
        HirExprKind::Unary { expr, .. } => find_internal_iter_binding(expr),
        HirExprKind::Block { exprs } => exprs.iter().find_map(find_internal_iter_binding),
        HirExprKind::If {
            branches,
            else_branch,
        } => branches
            .iter()
            .find_map(|(condition, body)| {
                find_internal_iter_binding(condition).or_else(|| find_internal_iter_binding(body))
            })
            .or_else(|| find_internal_iter_binding(else_branch)),
        HirExprKind::While { condition, body } => {
            find_internal_iter_binding(condition).or_else(|| find_internal_iter_binding(body))
        }
        HirExprKind::For { iterable, body, .. } => {
            find_internal_iter_binding(iterable).or_else(|| find_internal_iter_binding(body))
        }
        HirExprKind::Call { args, .. } | HirExprKind::New { args, .. } => {
            args.iter().find_map(find_internal_iter_binding)
        }
        HirExprKind::MemberAccess { object, .. } => find_internal_iter_binding(object),
        HirExprKind::MethodCall { object, args, .. } => find_internal_iter_binding(object)
            .or_else(|| args.iter().find_map(find_internal_iter_binding)),
        HirExprKind::TypeTest { expr, .. } | HirExprKind::TypeCast { expr, .. } => {
            find_internal_iter_binding(expr)
        }
        HirExprKind::VectorLiteral { elements, .. } => {
            elements.iter().find_map(find_internal_iter_binding)
        }
        HirExprKind::VectorGenerator { body, iterable, .. } => {
            find_internal_iter_binding(body).or_else(|| find_internal_iter_binding(iterable))
        }
        HirExprKind::VectorNew { size, init, .. } => find_internal_iter_binding(size)
            .or_else(|| init.as_ref().and_then(|i| find_internal_iter_binding(&i.body))),
        HirExprKind::VectorIndex { vector, index, .. } => {
            find_internal_iter_binding(vector).or_else(|| find_internal_iter_binding(index))
        }
        HirExprKind::Lambda { body, .. } => find_internal_iter_binding(body),
        HirExprKind::Assign { value, .. } => find_internal_iter_binding(value),
        HirExprKind::Number(_)
        | HirExprKind::String(_)
        | HirExprKind::Bool(_)
        | HirExprKind::Var { .. }
        | HirExprKind::SelfRef { .. }
        | HirExprKind::BaseCall { .. } => None,
    }
}

fn assert_analysis_and_check_error(source: &str, expected: impl Fn(&SemanticError) -> bool) {
    let program = parse_hulk_types_program(source).expect("source should parse");
    let analysis_errors = analyze_program(&program).expect_err("analysis should fail");
    let check_errors = check_program(&program).expect_err("check should fail");

    assert!(analysis_errors.iter().any(&expected));
    assert!(check_errors.iter().any(expected));
}
