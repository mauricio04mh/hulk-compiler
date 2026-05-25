use crate::types::Type;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinSignature {
    pub name: &'static str,
    pub params: Vec<Type>,
    pub return_type: Type,
}

pub fn builtin_functions() -> Vec<BuiltinSignature> {
    vec![
        BuiltinSignature {
            name: "print",
            params: vec![Type::Object],
            return_type: Type::Object,
        },
        BuiltinSignature {
            name: "sqrt",
            params: vec![Type::Number],
            return_type: Type::Number,
        },
        BuiltinSignature {
            name: "sin",
            params: vec![Type::Number],
            return_type: Type::Number,
        },
        BuiltinSignature {
            name: "cos",
            params: vec![Type::Number],
            return_type: Type::Number,
        },
        BuiltinSignature {
            name: "exp",
            params: vec![Type::Number],
            return_type: Type::Number,
        },
        BuiltinSignature {
            name: "log",
            params: vec![Type::Number, Type::Number],
            return_type: Type::Number,
        },
        BuiltinSignature {
            name: "rand",
            params: vec![],
            return_type: Type::Number,
        },
        BuiltinSignature {
            name: "range",
            params: vec![Type::Number, Type::Number],
            return_type: Type::Iterable(Box::new(Type::Number)),
        },
    ]
}

pub fn builtin_constants() -> Vec<(&'static str, Type)> {
    vec![("PI", Type::Number), ("E", Type::Number)]
}

pub fn builtin_constant_value(name: &str) -> Option<f64> {
    match name {
        "PI" => Some(std::f64::consts::PI),
        "E" => Some(std::f64::consts::E),
        _ => None,
    }
}
