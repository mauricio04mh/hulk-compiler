use hulk_frontend::parse_hulk_types_program;
use hulk_ir::{IrBinaryOp, IrDataValue, IrInstr, IrProgram};
use hulk_lower::{LowerError, lower_program};
use hulk_sema::analyze_program;

fn lower_source_ir(source: &str) -> Result<IrProgram, LowerError> {
    let ast = parse_hulk_types_program(source).expect("source should parse");
    let semantic = analyze_program(&ast).expect("source should pass semantic analysis");
    lower_program(&semantic)
}

fn lower_source(source: &str) -> Result<String, LowerError> {
    lower_source_ir(source).map(|ir| ir.to_string())
}

fn assert_golden(name: &str, source: &str, expected: &str) {
    let actual = lower_source(source).expect("lowering should pass");
    assert_eq!(actual, expected, "golden IR mismatch for {name}");
}

fn count_binary_op(program: &IrProgram, op: IrBinaryOp) -> usize {
    program
        .functions
        .iter()
        .flat_map(|function| &function.body)
        .filter(|instr| matches!(instr, IrInstr::Binary { op: actual, .. } if *actual == op))
        .count()
}

fn contains_string_data(program: &IrProgram, value: &str) -> bool {
    program
        .data
        .iter()
        .any(|data| matches!(&data.value, IrDataValue::String(actual) if actual == value))
}

#[test]
fn golden_01_number_literal() {
    assert_golden(
        "01_number_literal",
        include_str!("golden/01_number_literal.hulk"),
        include_str!("golden/01_number_literal.ir"),
    );
}

#[test]
fn golden_02_boolean_literal() {
    assert_golden(
        "02_boolean_literal",
        include_str!("golden/02_boolean_literal.hulk"),
        include_str!("golden/02_boolean_literal.ir"),
    );
}

#[test]
fn golden_03_string_literal() {
    assert_golden(
        "03_string_literal",
        include_str!("golden/03_string_literal.hulk"),
        include_str!("golden/03_string_literal.ir"),
    );
}

#[test]
fn golden_04_arithmetic_let() {
    assert_golden(
        "04_arithmetic_let",
        include_str!("golden/04_arithmetic_let.hulk"),
        include_str!("golden/04_arithmetic_let.ir"),
    );
}

#[test]
fn golden_05_string_concat() {
    assert_golden(
        "05_string_concat",
        include_str!("golden/05_string_concat.hulk"),
        include_str!("golden/05_string_concat.ir"),
    );
}

#[test]
fn golden_06_if_elif() {
    assert_golden(
        "06_if_elif",
        include_str!("golden/06_if_elif.hulk"),
        include_str!("golden/06_if_elif.ir"),
    );
}

#[test]
fn golden_07_assignment_while() {
    assert_golden(
        "07_assignment_while",
        include_str!("golden/07_assignment_while.hulk"),
        include_str!("golden/07_assignment_while.ir"),
    );
}

#[test]
fn golden_08_function_call() {
    assert_golden(
        "08_function_call",
        include_str!("golden/08_function_call.hulk"),
        include_str!("golden/08_function_call.ir"),
    );
}

#[test]
fn golden_inferred_function_param() {
    assert_golden(
        "inferred_function_param",
        include_str!("golden/inferred_function_param.hulk"),
        include_str!("golden/inferred_function_param.ir"),
    );
}

#[test]
fn golden_09_type_method() {
    assert_golden(
        "09_type_method",
        include_str!("golden/09_type_method.hulk"),
        include_str!("golden/09_type_method.ir"),
    );
}

#[test]
fn golden_inferred_method_param() {
    assert_golden(
        "inferred_method_param",
        include_str!("golden/inferred_method_param.hulk"),
        include_str!("golden/inferred_method_param.ir"),
    );
}

#[test]
fn golden_10_inheritance_base() {
    assert_golden(
        "10_inheritance_base",
        include_str!("golden/10_inheritance_base.hulk"),
        include_str!("golden/10_inheritance_base.ir"),
    );
}

#[test]
fn golden_inferred_constructor_param() {
    assert_golden(
        "inferred_constructor_param",
        include_str!("golden/inferred_constructor_param.hulk"),
        include_str!("golden/inferred_constructor_param.ir"),
    );
}

#[test]
fn golden_11_type_test_cast() {
    assert_golden(
        "11_type_test_cast",
        include_str!("golden/11_type_test_cast.hulk"),
        include_str!("golden/11_type_test_cast.ir"),
    );
}

#[test]
fn golden_12_vector_index() {
    assert_golden(
        "12_vector_index",
        include_str!("golden/12_vector_index.hulk"),
        include_str!("golden/12_vector_index.ir"),
    );
}

#[test]
fn golden_13_vector_generator() {
    assert_golden(
        "13_vector_generator",
        include_str!("golden/13_vector_generator.hulk"),
        include_str!("golden/13_vector_generator.ir"),
    );
}

#[test]
fn golden_14_lambda_closure() {
    assert_golden(
        "14_lambda_closure",
        include_str!("golden/14_lambda_closure.hulk"),
        include_str!("golden/14_lambda_closure.ir"),
    );
}

#[test]
fn golden_15_for_loop() {
    assert_golden(
        "15_for_loop",
        include_str!("golden/15_for_loop.hulk"),
        include_str!("golden/15_for_loop.ir"),
    );
}

#[test]
fn golden_scope_shadowing() {
    assert_golden(
        "scope_shadowing",
        include_str!("golden/scope_shadowing.hulk"),
        include_str!("golden/scope_shadowing.ir"),
    );
}

#[test]
fn golden_outer_scope_assignment() {
    assert_golden(
        "outer_scope_assignment",
        include_str!("golden/outer_scope_assignment.hulk"),
        include_str!("golden/outer_scope_assignment.ir"),
    );
}

#[test]
fn golden_attribute_initializer_expression() {
    assert_golden(
        "attribute_initializer_expression",
        include_str!("golden/attribute_initializer_expression.hulk"),
        include_str!("golden/attribute_initializer_expression.ir"),
    );
}

#[test]
fn golden_dynamic_dispatch_static_base() {
    assert_golden(
        "dynamic_dispatch_static_base",
        include_str!("golden/dynamic_dispatch_static_base.hulk"),
        include_str!("golden/dynamic_dispatch_static_base.ir"),
    );
}

#[test]
fn golden_base_call_value() {
    assert_golden(
        "base_call_value",
        include_str!("golden/base_call_value.hulk"),
        include_str!("golden/base_call_value.ir"),
    );
}

#[test]
fn golden_operators_full() {
    assert_golden(
        "operators_full",
        include_str!("golden/operators_full.hulk"),
        include_str!("golden/operators_full.ir"),
    );
}

#[test]
fn golden_string_data_no_interning() {
    assert_golden(
        "string_data_no_interning",
        include_str!("golden/string_data_no_interning.hulk"),
        include_str!("golden/string_data_no_interning.ir"),
    );
}

#[test]
fn golden_math_builtins() {
    assert_golden(
        "math_builtins",
        include_str!("golden/math_builtins.hulk"),
        include_str!("golden/math_builtins.ir"),
    );
}

#[test]
fn golden_recursive_function() {
    assert_golden(
        "recursive_function",
        include_str!("golden/recursive_function.hulk"),
        include_str!("golden/recursive_function.ir"),
    );
}

#[test]
fn golden_big_ir_smoke() {
    assert_golden(
        "big_ir_smoke",
        include_str!("golden/big_ir_smoke.hulk"),
        include_str!("golden/big_ir_smoke.expected.ir"),
    );
}

#[test]
fn concat_space_literal_lowers_to_two_concats() {
    let program = lower_source_ir("\"Hello\" @@ \"World\";").expect("lowering should pass");

    assert_eq!(count_binary_op(&program, IrBinaryOp::ConcatSpace), 0);
    assert_eq!(count_binary_op(&program, IrBinaryOp::Concat), 2);
    assert!(contains_string_data(&program, " "));
}

#[test]
fn concat_space_bindings_lowers_to_two_concats_and_keeps_locals() {
    let program =
        lower_source_ir("let a: String = \"Hello\" in let b: String = \"World\" in a @@ b;")
            .expect("lowering should pass");

    assert_eq!(count_binary_op(&program, IrBinaryOp::ConcatSpace), 0);
    assert_eq!(count_binary_op(&program, IrBinaryOp::Concat), 2);

    let entry = program
        .functions
        .iter()
        .find(|function| function.id == program.entry)
        .expect("entry function should exist");
    assert!(entry.locals.iter().any(|local| local.name == "a"));
    assert!(entry.locals.iter().any(|local| local.name == "b"));
}

#[test]
fn concat_does_not_insert_space_data() {
    let program = lower_source_ir("\"Hello\" @ \"World\";").expect("lowering should pass");

    assert_eq!(count_binary_op(&program, IrBinaryOp::ConcatSpace), 0);
    assert_eq!(count_binary_op(&program, IrBinaryOp::Concat), 1);
    assert!(!contains_string_data(&program, " "));
}

#[test]
fn nested_concat_space_lowers_without_concat_space_op() {
    let program = lower_source_ir("(\"A\" @@ \"B\") @@ \"C\";").expect("lowering should pass");

    assert_eq!(count_binary_op(&program, IrBinaryOp::ConcatSpace), 0);
    assert_eq!(count_binary_op(&program, IrBinaryOp::Concat), 4);
    assert!(contains_string_data(&program, " "));
}
