use crate::checker::FunctionType;
use crate::context::TypeRegistry;
use crate::types::Type;
use hulk_frontend::ast::{BinaryOp, Span, UnaryOp};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SemanticProgram {
    pub hir: HirProgram,
    pub registry: TypeRegistry,
    pub functions: HashMap<String, FunctionType>,
}

#[derive(Debug, Clone)]
pub struct HirProgram {
    pub declarations: Vec<HirDecl>,
    pub entry: HirExpr,
}

#[derive(Debug, Clone)]
pub enum HirDecl {
    Function(HirFunctionDecl),
    Type(HirTypeDecl),
    Protocol(HirProtocolDecl),
}

#[derive(Debug, Clone)]
pub struct HirFunctionDecl {
    pub name: String,
    pub params: Vec<HirParam>,
    pub return_type: Type,
    pub body: HirExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirTypeDecl {
    pub name: String,
    pub params: Vec<HirParam>,
    pub parent: Option<HirParent>,
    pub attributes: Vec<HirAttributeDecl>,
    pub methods: Vec<HirMethodDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirProtocolDecl {
    pub name: String,
    pub methods: Vec<HirProtocolMethod>,
    pub parent: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirParent {
    pub name: String,
    pub args: Option<Vec<HirExpr>>,
}

#[derive(Debug, Clone)]
pub struct HirAttributeDecl {
    pub name: String,
    pub ty: Type,
    pub value: HirExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirMethodDecl {
    pub owner_type: String,
    pub name: String,
    pub params: Vec<HirParam>,
    pub return_type: Type,
    pub body: HirExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirProtocolMethod {
    pub name: String,
    pub params: Vec<HirParam>,
    pub return_type: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirExpr {
    pub id: HirId,
    pub span: Span,
    pub ty: Type,
    pub kind: HirExprKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HirId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone)]
pub enum HirExprKind {
    Number(f64),
    String(String),
    Bool(bool),
    Var {
        name: String,
        symbol: SymbolId,
    },
    Unary {
        op: UnaryOp,
        expr: Box<HirExpr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<HirExpr>,
        right: Box<HirExpr>,
    },
    Assign {
        target: HirAssignTarget,
        value: Box<HirExpr>,
    },
    Let {
        bindings: Vec<HirLetBinding>,
        body: Box<HirExpr>,
    },
    Block {
        exprs: Vec<HirExpr>,
    },
    If {
        branches: Vec<(HirExpr, HirExpr)>,
        else_branch: Box<HirExpr>,
    },
    While {
        condition: Box<HirExpr>,
        body: Box<HirExpr>,
    },
    For {
        var: HirParam,
        iterable: Box<HirExpr>,
        body: Box<HirExpr>,
    },
    Call {
        callee: HirCallee,
        args: Vec<HirExpr>,
    },
    New {
        type_name: String,
        args: Vec<HirExpr>,
    },
    MemberAccess {
        object: Box<HirExpr>,
        member: String,
        resolved: ResolvedMember,
    },
    MethodCall {
        object: Box<HirExpr>,
        method: String,
        args: Vec<HirExpr>,
        dispatch: DispatchKind,
    },
    SelfRef {
        symbol: SymbolId,
        type_name: String,
    },
    BaseCall {
        parent_type: String,
        method_name: String,
        args: Vec<HirExpr>,
    },
    TypeTest {
        expr: Box<HirExpr>,
        type_name: String,
    },
    TypeCast {
        expr: Box<HirExpr>,
        type_name: String,
    },
    VectorLiteral {
        elements: Vec<HirExpr>,
        element_type: Type,
    },
    VectorGenerator {
        body: Box<HirExpr>,
        var: HirParam,
        iterable: Box<HirExpr>,
        element_type: Type,
    },
    VectorNew {
        size: Box<HirExpr>,
        element_type: Type,
        init: Option<HirVectorNewInit>,
    },
    VectorIndex {
        vector: Box<HirExpr>,
        index: Box<HirExpr>,
        element_type: Type,
    },
    Lambda {
        params: Vec<HirParam>,
        return_type: Type,
        body: Box<HirExpr>,
    },
}

#[derive(Debug, Clone)]
pub struct HirParam {
    pub name: String,
    pub ty: Type,
    pub symbol: SymbolId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct HirVectorNewInit {
    pub var: String,
    pub symbol: SymbolId,
    pub body: Box<HirExpr>,
}

#[derive(Debug, Clone)]
pub struct HirLetBinding {
    pub name: String,
    pub symbol: SymbolId,
    pub ty: Type,
    pub value: HirExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HirAssignTarget {
    Local {
        name: String,
        symbol: SymbolId,
        ty: Type,
    },
    SelfAttribute {
        owner_type: String,
        attr_name: String,
        ty: Type,
    },
    VectorIndex {
        vector: Box<HirExpr>,
        index: Box<HirExpr>,
        elem_ty: Type,
    },
}

#[derive(Debug, Clone)]
pub enum HirCallee {
    Builtin {
        name: String,
        signature: FunctionType,
    },
    GlobalFunction {
        name: String,
        signature: FunctionType,
    },
    LocalFunctor {
        name: String,
        symbol: SymbolId,
        signature: FunctionType,
    },
}

#[derive(Debug, Clone)]
pub enum DispatchKind {
    Virtual {
        receiver_static_type: Type,
        method_name: String,
        signature: FunctionType,
    },
    Static {
        function_label: String,
        signature: FunctionType,
    },
    Base {
        parent_type: String,
        method_name: String,
        signature: FunctionType,
    },
}

#[derive(Debug, Clone)]
pub enum ResolvedMember {
    Attribute {
        owner_type: String,
        attr_name: String,
        ty: Type,
    },
}
