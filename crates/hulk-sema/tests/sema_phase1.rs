use hulk_frontend::parse_hulk_types_program;
use hulk_sema::check_program;
use hulk_sema::context::TypeRegistry;
use hulk_sema::error::SemanticError;
use hulk_sema::types::Type;

fn parse(src: &str) -> hulk_frontend::ast::Program {
    parse_hulk_types_program(src).expect("source should parse")
}

fn check_ok(src: &str) {
    let program = parse(src);
    check_program(&program).expect("type check should pass");
}

fn check_err(src: &str) -> SemanticError {
    let program = parse(src);
    check_program(&program)
        .expect_err("type check should fail")
        .into_iter()
        .next()
        .expect("at least one error")
}


// ── TypeRegistry ──────────────────────────────────────────────────────────────

#[test]
fn registry_empty_program() {
    let program = parse("42;");
    let registry = TypeRegistry::build(&program).expect("should build");
    assert!(!registry.type_exists("Anything"));
}

#[test]
fn registry_registers_type() {
    // Type declarations don't end with a semicolon in HULK.
    let program = parse("type Point(x: Number, y: Number) {}\nnew Point(1, 2);");
    let registry = TypeRegistry::build(&program).expect("should build");
    assert!(registry.type_exists("Point"));
    let info = registry.get_type("Point").unwrap();
    assert_eq!(info.constructor_params.len(), 2);
    assert_eq!(info.constructor_params[0].0, "x");
    assert_eq!(info.constructor_params[0].1, Type::Number);
}

#[test]
fn registry_registers_protocol() {
    let program = parse("protocol Printable { toString(): String; }\n42;");
    let registry = TypeRegistry::build(&program).expect("should build");
    assert!(registry.type_exists("Printable"));
    let info = registry.get_protocol("Printable").unwrap();
    assert!(info.methods.contains_key("toString"));
}

#[test]
fn registry_duplicate_type_error() {
    let program = parse("type A {}\ntype A {}\n42;");
    let err = TypeRegistry::build(&program).expect_err("should fail");
    assert_eq!(err, SemanticError::DuplicateType { name: "A".to_string() });
}

#[test]
fn registry_circular_inheritance_error() {
    let program = parse("type A inherits B {}\ntype B inherits A {}\n42;");
    let err = TypeRegistry::build(&program).expect_err("should fail on circular");
    assert!(matches!(err, SemanticError::CircularInheritance { .. }));
}

#[test]
fn registry_undefined_parent_error() {
    let program = parse("type A inherits Ghost {}\n42;");
    let err = TypeRegistry::build(&program).expect_err("should fail");
    assert_eq!(err, SemanticError::UndefinedType { name: "Ghost".to_string() });
}

// ── Type enum ─────────────────────────────────────────────────────────────────

#[test]
fn type_user_type_assignable_to_object() {
    let dog = Type::UserType("Dog".to_string());
    assert!(dog.is_assignable_to(&Type::Object));
}

#[test]
fn type_number_not_assignable_to_string() {
    assert!(!Type::Number.is_assignable_to(&Type::String));
}

#[test]
fn type_from_type_ref_vector() {
    use hulk_frontend::ast::TypeRef;
    let tr = TypeRef::Vector(Box::new(TypeRef::simple("Number")));
    let ty = Type::from_type_ref(&tr);
    assert_eq!(ty, Type::Vector(Box::new(Type::Number)));
}

#[test]
fn type_from_type_ref_functor() {
    use hulk_frontend::ast::TypeRef;
    let tr = TypeRef::Functor {
        params: vec![TypeRef::simple("Number")],
        ret: Box::new(TypeRef::simple("Boolean")),
    };
    let ty = Type::from_type_ref(&tr);
    assert_eq!(
        ty,
        Type::Functor {
            params: vec![Type::Number],
            ret: Box::new(Type::Boolean)
        }
    );
}

#[test]
fn type_from_type_ref_unknown_becomes_user_type() {
    use hulk_frontend::ast::TypeRef;
    let tr = TypeRef::simple("Animal");
    let ty = Type::from_type_ref(&tr);
    assert_eq!(ty, Type::UserType("Animal".to_string()));
}

// ── New expression ────────────────────────────────────────────────────────────

#[test]
fn new_valid_construction() {
    check_ok("type Point(x: Number, y: Number) {}\nlet p = new Point(1, 2) in p;");
}

#[test]
fn new_arity_mismatch() {
    let err = check_err("type Point(x: Number, y: Number) {}\nnew Point(1);");
    assert!(matches!(err, SemanticError::ArityMismatch { .. }));
}

#[test]
fn new_undefined_type() {
    let err = check_err("new Ghost(1);");
    assert_eq!(err, SemanticError::UndefinedType { name: "Ghost".to_string() });
}

#[test]
fn new_returns_user_type() {
    // Assign result of new to a variable and use it — should not error.
    check_ok("type Box(v: Number) {}\nlet b = new Box(42) in b;");
}

// ── TypeTest / TypeCast ───────────────────────────────────────────────────────

#[test]
fn type_test_valid() {
    check_ok("type Animal {}\nlet a: Object = new Animal() in a is Animal;");
}

#[test]
fn type_test_undefined_type() {
    let err = check_err("42 is Ghost;");
    assert_eq!(err, SemanticError::UndefinedType { name: "Ghost".to_string() });
}

#[test]
fn type_cast_valid() {
    check_ok("type Cat {}\nlet c: Object = new Cat() in (c as Cat);");
}

#[test]
fn type_cast_undefined_type() {
    let err = check_err("42 as Ghost;");
    assert_eq!(err, SemanticError::UndefinedType { name: "Ghost".to_string() });
}

// ── Vector / Iterable ─────────────────────────────────────────────────────────

#[test]
fn vector_literal_homogeneous() {
    check_ok("[1, 2, 3];");
}

#[test]
fn vector_index_valid() {
    check_ok("let v = [1, 2, 3] in v[0];");
}

#[test]
fn vector_index_non_number_index() {
    let err = check_err("let v = [1, 2, 3] in v[\"x\"];");
    assert!(matches!(err, SemanticError::TypeMismatch { .. }));
}

#[test]
fn for_loop_range_binds_number() {
    // range returns Iterable(Number) — loop variable should be Number.
    check_ok("for (x in range(0, 10)) x * 2;");
}

// ── Lambda / Functor ──────────────────────────────────────────────────────────

#[test]
fn lambda_infers_functor_type() {
    // Lambda stored in variable and immediately called.
    check_ok("let f = (x: Number) => x * 2 in f(3);");
}

#[test]
fn lambda_arity_mismatch_via_variable() {
    let err = check_err("let f = (x: Number) => x in f(1, 2);");
    assert!(matches!(err, SemanticError::ArityMismatch { .. }));
}

// ── OOP member access ─────────────────────────────────────────────────────────

#[test]
fn member_access_known_attribute_type() {
    // External attribute access is forbidden per spec (A.7): attributes are private.
    let err = check_err(
        "type Counter(n: Number) { value: Number = n; }\nlet c = new Counter(0) in c.value;",
    );
    assert!(matches!(err, SemanticError::AttributeIsPrivate { .. }));
}

#[test]
fn method_call_resolves() {
    // Note: 'base' is a reserved keyword — use 'n' instead.
    check_ok(
        "type Adder(start: Number) { add(n: Number): Number => start + n; }\nlet a = new Adder(10) in a.add(5);",
    );
}

// ── Type member body checking ─────────────────────────────────────────────────

#[test]
fn type_method_body_type_error() {
    let err = check_err(
        "type Broken(n: Number) { bad(): Number => \"not a number\"; }\n42;",
    );
    assert!(matches!(err, SemanticError::InvalidReturnType { .. }));
}

#[test]
fn type_attribute_type_mismatch() {
    let err = check_err(
        "type Wrong(n: Number) { x: Number = \"hello\"; }\n42;",
    );
    assert!(matches!(err, SemanticError::TypeMismatch { .. }));
}
