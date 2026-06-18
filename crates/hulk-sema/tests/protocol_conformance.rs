use hulk_frontend::parse_hulk_types_program;
use hulk_sema::analyze_program;
use hulk_sema::error::SemanticError;
use hulk_sema::hir::{HirDecl, HirFunctionDecl, SemanticProgram};
use hulk_sema::types::Type;

fn analyze(source: &str) -> Result<SemanticProgram, Vec<SemanticError>> {
    let program = parse_hulk_types_program(source).expect("source should parse");
    analyze_program(&program)
}

fn function<'a>(program: &'a SemanticProgram, name: &str) -> &'a HirFunctionDecl {
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

fn expect_protocol_error(errors: &[SemanticError]) {
    assert!(
        errors.iter().any(|error| matches!(
            error,
            SemanticError::CannotInferParameterType { .. }
                | SemanticError::UnsupportedConstruct { .. }
                | SemanticError::MissingProtocolMethod { .. }
                | SemanticError::ProtocolMethodSignatureMismatch { .. }
                | SemanticError::ProtocolReturnTypeMismatch { .. }
                | SemanticError::ProtocolParamTypeMismatch { .. }
                | SemanticError::TypeMismatch { .. }
                | SemanticError::InvalidArgumentType { .. }
        )),
        "expected a protocol-related semantic error, got {errors:?}"
    );
}

#[test]
fn protocol_method_must_have_typed_parameter() {
    let err = analyze(
        "protocol P {
            f(x): Number;
        }

        1;",
    )
    .expect_err("analysis should fail");
    expect_protocol_error(&err);
}

#[test]
fn protocol_method_must_have_typed_return() {
    let err = analyze(
        "protocol P {
            f(x: Number);
        }

        1;",
    )
    .expect_err("analysis should fail");
    expect_protocol_error(&err);
}

#[test]
fn type_conforms_to_protocol_with_exact_signature() {
    let program = analyze(
        "protocol Hashable {
            hash(): Number;
        }

        type Person {
            hash(): Number => 1;
        }

        function use_hashable(x: Hashable): Number => x.hash();

        use_hashable(new Person());",
    )
    .expect("analysis should succeed");

    let func = function(&program, "use_hashable");
    assert_eq!(func.return_type, Type::Number);
}

#[test]
fn type_missing_protocol_method_fails() {
    let err = analyze(
        "protocol Hashable {
            hash(): Number;
        }

        type Person {
            name(): String => \"Bob\";
        }

        function use_hashable(x: Hashable): Number => x.hash();

        use_hashable(new Person());",
    )
    .expect_err("analysis should fail");
    expect_protocol_error(&err);
}

#[test]
fn protocol_return_covariance_accepts_more_specific_return() {
    let program = analyze(
        "protocol Named {
            name(): Object;
        }

        type Person {
            name(): String => \"Bob\";
        }

        function get_name(x: Named): Object => x.name();

        get_name(new Person());",
    )
    .expect("analysis should succeed");

    let func = function(&program, "get_name");
    assert_eq!(func.return_type, Type::Object);
}

#[test]
fn protocol_return_covariance_rejects_less_specific_return() {
    let err = analyze(
        "protocol Named {
            name(): String;
        }

        type Person {
            name(): Object => \"Bob\";
        }

        function get_name(x: Named): String => x.name();

        get_name(new Person());",
    )
    .expect_err("analysis should fail");
    expect_protocol_error(&err);
}

#[test]
fn protocol_parameter_contravariance_accepts_more_general_parameter() {
    let program = analyze(
        "protocol Printer {
            print_value(x: String): Object;
        }

        type AnyPrinter {
            print_value(x: Object): Object => x;
        }

        function call_printer(p: Printer): Object => p.print_value(\"hello\");

        call_printer(new AnyPrinter());",
    )
    .expect("analysis should succeed");

    let func = function(&program, "call_printer");
    assert_eq!(func.return_type, Type::Object);
}

#[test]
fn protocol_parameter_contravariance_rejects_more_specific_parameter() {
    let err = analyze(
        "protocol Printer {
            print_value(x: Object): Object;
        }

        type StringPrinter {
            print_value(x: String): Object => x;
        }

        function call_printer(p: Printer): Object => p.print_value(\"hello\");

        call_printer(new StringPrinter());",
    )
    .expect_err("analysis should fail");
    expect_protocol_error(&err);
}

#[test]
fn protocol_extension_requires_parent_methods() {
    let err = analyze(
        "protocol Hashable {
            hash(): Number;
        }

        protocol Equatable extends Hashable {
            equals(other: Object): Boolean;
        }

        type Person {
            equals(other: Object): Boolean => true;
        }

        function check(x: Equatable): Boolean => x.equals(x);

        check(new Person());",
    )
    .expect_err("analysis should fail");
    expect_protocol_error(&err);
}

#[test]
fn protocol_extension_accepts_type_with_all_methods() {
    let program = analyze(
        "protocol Hashable {
            hash(): Number;
        }

        protocol Equatable extends Hashable {
            equals(other: Object): Boolean;
        }

        type Person {
            hash(): Number => 1;
            equals(other: Object): Boolean => true;
        }

        function check(x: Equatable): Boolean => x.equals(x);

        check(new Person());",
    )
    .expect("analysis should succeed");

    let func = function(&program, "check");
    assert_eq!(func.return_type, Type::Boolean);
}
