/// Source position. PartialEq is always true so spans are transparent to AST equality checks.
#[derive(Debug, Clone, Copy, Default)]
pub struct Span {
    pub line: u32,
    pub col: u32,
}

impl Span {
    pub fn new(line: usize, col: usize) -> Self {
        Span {
            line: line as u32,
            col: col as u32,
        }
    }
}

impl PartialEq for Span {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Span {}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub declarations: Vec<Decl>,
    pub entry: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Function(FunctionDecl),
    Type(TypeDecl),
    Protocol(ProtocolDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub return_type: Option<TypeRef>,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeDecl {
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub parent: Option<TypeParent>,
    pub members: Vec<TypeMember>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeParent {
    pub name: String,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeMember {
    Attribute(AttributeDecl),
    Method(MethodDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributeDecl {
    pub name: String,
    pub ty: Option<TypeRef>,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeRef>,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolDecl {
    pub name: String,
    pub name_span: Span,
    pub parent: Option<String>,
    pub methods: Vec<ProtocolMethod>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<TypeRef>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Option<TypeRef>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeRef {
    Simple(String),
    Iterable(Box<TypeRef>),
    Vector(Box<TypeRef>),
    Functor {
        params: Vec<TypeRef>,
        ret: Box<TypeRef>,
    },
}

impl TypeRef {
    pub fn simple(name: impl Into<String>) -> Self {
        TypeRef::Simple(name.into())
    }

    pub fn name(&self) -> Option<&str> {
        if let TypeRef::Simple(n) = self {
            Some(n.as_str())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    String(String),
    Bool(bool),
    Var(String, Span),

    Unary {
        span: Span,
        op: UnaryOp,
        expr: Box<Expr>,
    },

    Binary {
        span: Span,
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },

    Assign {
        span: Span,
        target: Box<Expr>,
        value: Box<Expr>,
    },

    Let {
        span: Span,
        bindings: Vec<LetBinding>,
        body: Box<Expr>,
    },

    Call {
        span: Span,
        callee: Box<Expr>,
        args: Vec<Expr>,
    },

    Block(Vec<Expr>),

    If {
        span: Span,
        branches: Vec<(Expr, Expr)>,
        else_branch: Box<Expr>,
    },

    While {
        span: Span,
        condition: Box<Expr>,
        body: Box<Expr>,
    },

    For {
        span: Span,
        var: String,
        iterable: Box<Expr>,
        body: Box<Expr>,
    },

    New {
        span: Span,
        type_name: String,
        args: Vec<Expr>,
    },

    MemberAccess {
        span: Span,
        object: Box<Expr>,
        member: String,
    },

    MethodCall {
        span: Span,
        object: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },

    SelfRef,

    BaseCall {
        span: Span,
        args: Vec<Expr>,
    },

    TypeTest {
        span: Span,
        expr: Box<Expr>,
        type_name: String,
    },

    TypeCast {
        span: Span,
        expr: Box<Expr>,
        type_name: String,
    },

    VectorLiteral(Vec<Expr>),

    VectorGenerator {
        span: Span,
        body: Box<Expr>,
        var: String,
        iterable: Box<Expr>,
    },

    VectorIndex {
        span: Span,
        vector: Box<Expr>,
        index: Box<Expr>,
    },

    Lambda {
        span: Span,
        params: Vec<Param>,
        return_type: Option<TypeRef>,
        body: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LetBinding {
    pub name: String,
    pub ty: Option<TypeRef>,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,
    Neg,
    Pos,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Concat,
    ConcatSpace,
    Eq,
    Neq,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}
