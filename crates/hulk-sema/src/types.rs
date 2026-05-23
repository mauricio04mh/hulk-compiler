use hulk_frontend::ast::TypeRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Number,
    String,
    Boolean,
    Object,
    UserType(String),
    Vector(Box<Type>),
    Iterable(Box<Type>),
    Functor { params: Vec<Type>, ret: Box<Type> },
    Unknown,
}

impl Type {
    pub fn from_type_ref(type_ref: &TypeRef) -> Type {
        match type_ref {
            TypeRef::Simple(name) => match name.as_str() {
                "Number" => Type::Number,
                "String" => Type::String,
                "Boolean" => Type::Boolean,
                "Object" => Type::Object,
                other => Type::UserType(other.to_string()),
            },
            TypeRef::Iterable(inner) => Type::Iterable(Box::new(Type::from_type_ref(inner))),
            TypeRef::Vector(inner) => Type::Vector(Box::new(Type::from_type_ref(inner))),
            TypeRef::Functor { params, ret } => Type::Functor {
                params: params.iter().map(Type::from_type_ref).collect(),
                ret: Box::new(Type::from_type_ref(ret)),
            },
        }
    }

    /// Returns true if a value of this type can be used where `target` is expected.
    /// Phase 1: structural equality + Object/Unknown as universal supertypes.
    pub fn is_assignable_to(&self, target: &Type) -> bool {
        self == target || *target == Type::Object || *target == Type::Unknown
    }
}
