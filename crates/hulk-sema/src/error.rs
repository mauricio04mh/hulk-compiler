use crate::types::Type;
use hulk_frontend::ast::{BinaryOp, UnaryOp};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum SemanticError {
    #[error("Duplicate function '{name}'")]
    DuplicateFunction { name: String },

    #[error("Duplicate parameter '{parameter}' in function '{function}'")]
    DuplicateParameter { function: String, parameter: String },

    #[error("Duplicate symbol '{name}'")]
    DuplicateSymbol { name: String },

    #[error("Duplicate attribute '{attr_name}' in type '{type_name}'")]
    DuplicateAttribute { type_name: String, attr_name: String },

    #[error("Duplicate method '{method_name}' in type '{type_name}'")]
    DuplicateMethod { type_name: String, method_name: String },

    #[error("Duplicate method '{method_name}' in protocol '{protocol_name}'")]
    DuplicateProtocolMethod { protocol_name: String, method_name: String },

    #[error("Undefined variable '{name}'")]
    UndefinedVariable { name: String },

    #[error("Undefined function '{name}'")]
    UndefinedFunction { name: String },

    #[error("Undefined method '{method_name}' for type '{type_name}'")]
    UndefinedMethod { type_name: String, method_name: String },

    #[error("Invalid assignment target")]
    InvalidAssignmentTarget,

    #[error("Arity mismatch for function '{function}': expected {expected}, found {found}")]
    ArityMismatch {
        function: String,
        expected: usize,
        found: usize,
    },

    #[error("Unknown type '{name}'")]
    UnknownType { name: String },

    #[error("Cannot infer parameter type for '{parameter}' in function '{function}'")]
    CannotInferParameterType { function: String, parameter: String },

    #[error("Type mismatch: expected {expected:?}, found {found:?}")]
    TypeMismatch { expected: Type, found: Type },

    #[error("Invalid index target: expected vector, found {found:?}")]
    InvalidIndexTarget { found: Type },

    #[error("Invalid iterable target: expected iterable or vector, found {found:?}")]
    InvalidIterableTarget { found: Type },

    #[error("Invalid unary operand for {op:?}: found {found:?}")]
    InvalidUnaryOperand { op: UnaryOp, found: Type },

    #[error("Invalid binary operands for {op:?}: left {left:?}, right {right:?}")]
    InvalidBinaryOperands {
        op: BinaryOp,
        left: Type,
        right: Type,
    },

    #[error("Invalid condition type: found {found:?}")]
    InvalidConditionType { found: Type },

    #[error("Invalid return type in function '{function}': expected {expected:?}, found {found:?}")]
    InvalidReturnType {
        function: String,
        expected: Type,
        found: Type,
    },

    #[error(
        "Invalid argument type in '{function}' at index {index}: expected {expected:?}, found {found:?}"
    )]
    InvalidArgumentType {
        function: String,
        index: usize,
        expected: Type,
        found: Type,
    },

    #[error("Cannot infer return type for function '{function}'")]
    CannotInferFunctionReturnType { function: String },

    #[error("Unsupported construct: {message}")]
    UnsupportedConstruct { message: String },

    #[error("Duplicate type declaration '{name}'")]
    DuplicateType { name: String },

    #[error("Undefined type '{name}'")]
    UndefinedType { name: String },

    #[error("Circular inheritance involving type '{name}'")]
    CircularInheritance { name: String },

    #[error("Type '{type_name}' is missing method '{method_name}' required by protocol")]
    MissingProtocolMethod { type_name: String, method_name: String },

    #[error(
        "Method '{method_name}' in '{type_name}' has wrong arity for protocol: expected {expected} params, found {found}"
    )]
    ProtocolMethodSignatureMismatch {
        type_name: String,
        method_name: String,
        expected: usize,
        found: usize,
    },

    #[error("Attribute '{attr_name}' of type '{type_name}' is private and cannot be accessed externally")]
    AttributeIsPrivate { type_name: String, attr_name: String },

    #[error("Type '{child}' cannot inherit from primitive type '{parent}'")]
    CannotInheritFromPrimitive { child: String, parent: String },

    #[error("Override of method '{method_name}' in '{type_name}' does not match the parent signature")]
    MethodOverrideSignatureMismatch { type_name: String, method_name: String },

    #[error(
        "Method '{method_name}' in '{type_name}' has wrong return type for protocol: expected {expected:?}, found {found:?}"
    )]
    ProtocolReturnTypeMismatch {
        type_name: String,
        method_name: String,
        expected: Type,
        found: Type,
    },

    #[error(
        "Method '{method_name}' in '{type_name}' has wrong param type at index {param_idx} for protocol: expected {expected:?}, found {found:?}"
    )]
    ProtocolParamTypeMismatch {
        type_name: String,
        method_name: String,
        param_idx: usize,
        expected: Type,
        found: Type,
    },
}
