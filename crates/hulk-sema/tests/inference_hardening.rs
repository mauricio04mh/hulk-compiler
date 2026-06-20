use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use hulk_frontend::parse_hulk_types_program;
use hulk_sema::analyze_program;
use hulk_sema::error::SemanticError;
use hulk_sema::hir::{HirDecl, HirFunctionDecl, HirMethodDecl, HirTypeDecl, SemanticProgram};
use hulk_sema::types::Type;

const ANALYSIS_TIMEOUT: Duration = Duration::from_secs(5);

fn analyze(source: &str) -> Result<SemanticProgram, Vec<SemanticError>> {
    let program = parse_hulk_types_program(source).expect("source should parse");
    analyze_program(&program)
}

fn analyze_with_timeout(
    source: &'static str,
) -> Result<Result<SemanticProgram, Vec<SemanticError>>, String> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let _ = tx.send(analyze(source));
    });

    match rx.recv_timeout(ANALYSIS_TIMEOUT) {
        Ok(result) => Ok(result),
        Err(RecvTimeoutError::Timeout) => Err("analysis timed out".to_string()),
        Err(RecvTimeoutError::Disconnected) => Err("analysis thread disconnected".to_string()),
    }
}

fn expect_failure(source: &'static str) -> Vec<SemanticError> {
    analyze_with_timeout(source)
        .expect("analysis should terminate")
        .expect_err("analysis should fail")
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

fn type_decl<'a>(program: &'a SemanticProgram, name: &str) -> &'a HirTypeDecl {
    program
        .hir
        .declarations
        .iter()
        .find_map(|decl| match decl {
            HirDecl::Type(type_decl) if type_decl.name == name => Some(type_decl),
            _ => None,
        })
        .expect("type should exist")
}

fn method<'a>(
    program: &'a SemanticProgram,
    type_name: &str,
    method_name: &str,
) -> &'a HirMethodDecl {
    type_decl(program, type_name)
        .methods
        .iter()
        .find(|method| method.name == method_name)
        .expect("method should exist")
}

fn assert_conflicting_error(errors: &[SemanticError]) {
    assert!(
        errors.iter().any(|error| matches!(
            error,
            SemanticError::TypeMismatch { .. } | SemanticError::InvalidArgumentType { .. }
        )),
        "expected a type-related conflict error, got {errors:?}"
    );
}

fn assert_cannot_infer_error(errors: &[SemanticError], function: &str, parameter: &str) {
    assert!(
        errors.iter().any(|error| matches!(
            error,
            SemanticError::CannotInferParameterType { function: f, parameter: p }
                if f == function && p == parameter
        )),
        "expected CannotInferParameterType for {function}.{parameter}, got {errors:?}"
    );
}

#[test]
fn conflicting_global_function_call_sites_fail() {
    let errors = expect_failure(
        "function id(x) => x;

        {
            id(1);
            id(\"hello\");
        }",
    );
    assert_conflicting_error(&errors);
}

#[test]
fn conflicting_global_function_body_and_call_site_fail() {
    let errors = expect_failure(
        "function f(x) => x + 1;

        f(\"hello\");",
    );
    assert_conflicting_error(&errors);
}

#[test]
fn recursive_unconstrained_functions_still_fail() {
    let errors = expect_failure(
        "function f(x) => g(x);
        function g(y) => f(y);

        1;",
    );
    assert_cannot_infer_error(&errors, "f", "x");
    assert_cannot_infer_error(&errors, "g", "y");
}

#[test]
fn recursive_functions_with_concrete_call_site_terminate() {
    match analyze_with_timeout(
        "function f(x) => g(x);
        function g(y) => f(y);

        f(1);",
    )
    .expect("analysis should terminate")
    {
        Ok(program) => {
            let f = function(&program, "f");
            let g = function(&program, "g");
            assert_eq!(f.params[0].ty, Type::Number);
            assert_eq!(g.params[0].ty, Type::Number);
            assert!(matches!(f.return_type, Type::Number | Type::Unknown));
            assert!(matches!(g.return_type, Type::Number | Type::Unknown));
        }
        Err(errors) => {
            assert_conflicting_error(&errors);
            assert!(
                errors
                    .iter()
                    .any(|error| matches!(error, SemanticError::CannotInferParameterType { .. })),
                "expected a stable semantic error for recursive inference, got {errors:?}"
            );
        }
    }
}

#[test]
fn conflicting_method_body_and_call_site_fail() {
    let errors = expect_failure(
        "type A {
            f(x) => x + 1;
        }

        new A().f(\"hello\");",
    );
    assert_conflicting_error(&errors);
}

#[test]
fn method_cycle_without_concrete_call_site_still_fails() {
    let errors = expect_failure(
        "type A {
            f(x) => self.g(x);
            g(y) => self.f(y);
        }

        1;",
    );
    assert_cannot_infer_error(&errors, "f", "x");
    assert_cannot_infer_error(&errors, "g", "y");
}

#[test]
fn method_cycle_with_concrete_call_site_terminates() {
    match analyze_with_timeout(
        "type A {
            f(x) => self.g(x);
            g(y) => self.f(y);
        }

        new A().f(1);",
    )
    .expect("analysis should terminate")
    {
        Ok(program) => {
            let f = method(&program, "A", "f");
            let g = method(&program, "A", "g");
            assert_eq!(f.params[0].ty, Type::Number);
            assert_eq!(g.params[0].ty, Type::Number);
            assert!(matches!(f.return_type, Type::Number | Type::Unknown));
            assert!(matches!(g.return_type, Type::Number | Type::Unknown));
        }
        Err(errors) => {
            assert!(
                errors.iter().any(|error| matches!(
                    error,
                    SemanticError::TypeMismatch { .. }
                        | SemanticError::InvalidArgumentType { .. }
                        | SemanticError::CannotInferParameterType { .. }
                )),
                "expected a stable semantic error for recursive method inference, got {errors:?}"
            );
        }
    }
}

#[test]
fn conflicting_constructor_body_and_call_site_fail() {
    let errors = expect_failure(
        "type Counter(value) {
            inc() => value + 1;
        }

        new Counter(\"hello\").inc();",
    );
    assert_conflicting_error(&errors);
}

#[test]
#[ignore = "nested constructor-to-method propagation is not supported by the current object typing model"]
fn constructor_method_chain_infers_type_param() {
    match analyze_with_timeout(
        "type Box(value) {
            get() => value;
        }

        type UseBox(box) {
            read() => box.get();
        }

        new UseBox(new Box(1)).read();",
    )
    .expect("analysis should terminate")
    {
        Ok(program) => {
            let box_type = type_decl(&program, "Box");
            let use_box = type_decl(&program, "UseBox");
            let get = method(&program, "Box", "get");
            let read = method(&program, "UseBox", "read");

            assert_eq!(box_type.params[0].ty, Type::Number);
            assert_eq!(get.return_type, Type::Number);
            assert_eq!(read.return_type, Type::Number);
            assert!(
                matches!(&use_box.params[0].ty, Type::UserType(name) if name == "Box")
                    || matches!(use_box.params[0].ty, Type::Object)
            );
        }
        Err(errors) => {
            assert!(
                errors.iter().any(|error| matches!(
                    error,
                    SemanticError::TypeMismatch { .. }
                        | SemanticError::InvalidArgumentType { .. }
                        | SemanticError::CannotInferParameterType { .. }
                )),
                "expected either successful inference or a stable semantic error, got {errors:?}"
            );
        }
    }
}

#[test]
fn function_to_constructor_to_method_inference() {
    match analyze_with_timeout(
        "type Box(value) {
            get() => value;
        }

        function make(x) => new Box(x);

        make(1).get();",
    )
    .expect("analysis should terminate")
    {
        Ok(program) => {
            let make = function(&program, "make");
            let box_type = type_decl(&program, "Box");
            let get = method(&program, "Box", "get");

            assert_eq!(make.params[0].ty, Type::Number);
            assert_eq!(make.return_type, Type::UserType("Box".to_string()));
            assert_eq!(box_type.params[0].ty, Type::Number);
            assert_eq!(get.return_type, Type::Number);
        }
        Err(errors) => {
            assert!(
                errors.iter().any(|error| matches!(
                    error,
                    SemanticError::TypeMismatch { .. }
                        | SemanticError::InvalidArgumentType { .. }
                        | SemanticError::CannotInferParameterType { .. }
                )),
                "expected either successful inference or a stable semantic error, got {errors:?}"
            );
        }
    }
}
