use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct IrProgram {
    pub types: Vec<IrType>,
    pub data: Vec<IrData>,
    pub functions: Vec<IrFunction>,
    pub entry: FunctionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttrId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MethodSlot(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DataId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParamId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TempId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LabelId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrTypeRef {
    Number,
    String,
    Boolean,
    Object,
    User(String),
    Vector(Box<IrTypeRef>),
    Iterable(Box<IrTypeRef>),
    Functor {
        capture_types: Vec<IrTypeRef>,
        params: Vec<IrTypeRef>,
        ret: Box<IrTypeRef>,
    },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrType {
    pub id: TypeId,
    pub name: String,
    pub parent: Option<String>,
    pub attributes: Vec<IrAttribute>,
    pub methods: Vec<IrMethod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrAttribute {
    pub id: AttrId,
    pub name: String,
    pub ty: IrTypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrMethod {
    pub slot: MethodSlot,
    pub name: String,
    pub function: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrData {
    pub id: DataId,
    pub value: IrDataValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrDataValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrFunction {
    pub id: FunctionId,
    pub name: String,
    pub kind: IrFunctionKind,
    pub params: Vec<IrParam>,
    pub locals: Vec<IrLocal>,
    pub temps: Vec<IrTemp>,
    pub return_type: IrTypeRef,
    pub body: Vec<IrInstr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrFunctionKind {
    Entry,
    Function,
    Method {
        owner_type: String,
        method_name: String,
    },
    Lambda,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrParam {
    pub id: ParamId,
    pub name: String,
    pub ty: IrTypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrLocal {
    pub id: LocalId,
    pub name: String,
    pub ty: IrTypeRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrTemp {
    pub id: TempId,
    pub ty: IrTypeRef,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrValue {
    Temp(TempId),
    Local(LocalId),
    Param(ParamId),
    ConstNumber(f64),
    ConstBool(bool),
    DataRef(DataId),
    Null,
    Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrPlace {
    Temp(TempId),
    Local(LocalId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrUnaryOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrBinaryOp {
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

#[derive(Debug, Clone, PartialEq)]
pub enum IrInstr {
    Label(LabelId),
    Assign {
        dst: IrPlace,
        src: IrValue,
    },
    Unary {
        dst: IrPlace,
        op: IrUnaryOp,
        value: IrValue,
    },
    Binary {
        dst: IrPlace,
        op: IrBinaryOp,
        left: IrValue,
        right: IrValue,
    },
    Jump(LabelId),
    Branch {
        cond: IrValue,
        then_label: LabelId,
        else_label: LabelId,
    },
    Call {
        dst: Option<IrPlace>,
        function: String,
        args: Vec<IrValue>,
    },
    Allocate {
        dst: IrPlace,
        type_name: String,
    },
    GetAttr {
        dst: IrPlace,
        object: IrValue,
        attr: AttrId,
    },
    SetAttr {
        object: IrValue,
        attr: AttrId,
        value: IrValue,
    },
    VirtualCall {
        dst: Option<IrPlace>,
        receiver: IrValue,
        receiver_static_type: String,
        method: String,
        slot: MethodSlot,
        args: Vec<IrValue>,
    },
    StaticCall {
        dst: Option<IrPlace>,
        function: String,
        args: Vec<IrValue>,
    },
    BaseCall {
        dst: Option<IrPlace>,
        parent_type: String,
        method: String,
        args: Vec<IrValue>,
    },
    NewVector {
        dst: IrPlace,
        elements: Vec<IrValue>,
    },
    VectorLen {
        dst: IrPlace,
        vector: IrValue,
    },
    VectorPush {
        vector: IrValue,
        value: IrValue,
    },
    VectorGet {
        dst: IrPlace,
        vector: IrValue,
        index: IrValue,
    },
    VectorSet {
        vector: IrValue,
        index: IrValue,
        value: IrValue,
    },
    MakeClosure {
        dst: IrPlace,
        function: String,
        captures: Vec<IrValue>,
    },
    ClosureCall {
        dst: Option<IrPlace>,
        closure: IrValue,
        args: Vec<IrValue>,
    },
    GetCapture {
        dst: IrPlace,
        closure: IrValue,
        idx: usize,
        ty: IrTypeRef,
    },
    TypeTest {
        dst: IrPlace,
        value: IrValue,
        type_name: String,
    },
    TypeCast {
        dst: IrPlace,
        value: IrValue,
        type_name: String,
    },
    Return(Option<IrValue>),
}

impl fmt::Display for IrProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, ".TYPES")?;
        if self.types.is_empty() {
            writeln!(f, "  <empty>")?;
        } else {
            for ty in &self.types {
                write_indented(f, ty, 0)?;
            }
        }

        writeln!(f)?;
        writeln!(f, ".DATA")?;
        if self.data.is_empty() {
            writeln!(f, "  <empty>")?;
        } else {
            for data in &self.data {
                writeln!(f, "{}", data)?;
            }
        }

        writeln!(f)?;
        writeln!(f, ".CODE")?;
        writeln!(f, "entry #{}", self.entry.0)?;
        if self.functions.is_empty() {
            writeln!(f, "  <empty>")?;
        } else {
            for function in &self.functions {
                write_indented(f, function, 0)?;
            }
        }

        Ok(())
    }
}

fn write_indented<T: fmt::Display>(
    f: &mut fmt::Formatter<'_>,
    value: &T,
    indent: usize,
) -> fmt::Result {
    for line in value.to_string().lines() {
        for _ in 0..indent {
            write!(f, " ")?;
        }
        writeln!(f, "{line}")?;
    }
    Ok(())
}

impl fmt::Display for IrType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(parent) = &self.parent {
            writeln!(
                f,
                "type {} #{} inherits {} {{",
                self.name, self.id.0, parent
            )?;
        } else {
            writeln!(f, "type {} #{} {{", self.name, self.id.0)?;
        }

        for attr in &self.attributes {
            writeln!(f, "  attr #{} {}: {}", attr.id.0, attr.name, attr.ty)?;
        }
        for method in &self.methods {
            writeln!(
                f,
                "  method #{} {} : {}",
                method.slot.0, method.name, method.function
            )?;
        }

        write!(f, "}}")
    }
}

impl fmt::Display for IrData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "data @s{} = {}", self.id.0, self.value)
    }
}

impl fmt::Display for IrDataValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrDataValue::String(value) => write!(f, "\"{}\"", escape_string(value)),
            IrDataValue::Number(value) => write!(f, "{}", format_number(*value)),
            IrDataValue::Boolean(value) => write!(f, "{value}"),
        }
    }
}

impl fmt::Display for IrFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            IrFunctionKind::Entry => {
                writeln!(
                    f,
                    "function {} #{} -> {} {{",
                    self.name, self.id.0, self.return_type
                )?;
            }
            IrFunctionKind::Function => {
                writeln!(
                    f,
                    "function {} #{} -> {} {{",
                    self.name, self.id.0, self.return_type
                )?;
            }
            IrFunctionKind::Method {
                owner_type,
                method_name,
            } => {
                writeln!(
                    f,
                    "method {}.{} #{} -> {} {{",
                    owner_type, method_name, self.id.0, self.return_type
                )?;
            }
            IrFunctionKind::Lambda => {
                writeln!(
                    f,
                    "lambda {} #{} -> {} {{",
                    self.name, self.id.0, self.return_type
                )?;
            }
        }

        for param in &self.params {
            writeln!(
                f,
                "  param %p{}: {} name={}",
                param.id.0, param.ty, param.name
            )?;
        }
        for local in &self.locals {
            writeln!(
                f,
                "  local %l{}: {} name={}",
                local.id.0, local.ty, local.name
            )?;
        }
        for temp in &self.temps {
            writeln!(f, "  temp %t{}: {}", temp.id.0, temp.ty)?;
        }

        if (!self.params.is_empty() || !self.locals.is_empty() || !self.temps.is_empty())
            && !self.body.is_empty()
        {
            writeln!(f)?;
        }

        for instr in &self.body {
            writeln!(f, "  {instr}")?;
        }

        write!(f, "}}")
    }
}

impl fmt::Display for IrInstr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrInstr::Label(label) => write!(f, "label L{}", label.0),
            IrInstr::Assign { dst, src } => write!(f, "{dst} = {src}"),
            IrInstr::Unary { dst, op, value } => write!(f, "{dst} = {op}{value}"),
            IrInstr::Binary {
                dst,
                op,
                left,
                right,
            } => write!(f, "{dst} = {left} {op} {right}"),
            IrInstr::Jump(label) => write!(f, "jump L{}", label.0),
            IrInstr::Branch {
                cond,
                then_label,
                else_label,
            } => {
                write!(f, "branch {cond} ? L{} : L{}", then_label.0, else_label.0)
            }
            IrInstr::Call {
                dst,
                function,
                args,
            } => write_call(f, dst.as_ref(), "call", function, args),
            IrInstr::Allocate { dst, type_name } => write!(f, "{dst} = allocate {type_name}"),
            IrInstr::GetAttr { dst, object, attr } => {
                write!(f, "{dst} = getattr {object}, #{}", attr.0)
            }
            IrInstr::SetAttr {
                object,
                attr,
                value,
            } => write!(f, "setattr {object}, #{}, {value}", attr.0),
            IrInstr::VirtualCall {
                dst,
                receiver,
                receiver_static_type,
                method,
                slot,
                args,
            } => {
                let target = format!("{receiver}.{receiver_static_type}::{method}#{}", slot.0);
                write_call(f, dst.as_ref(), "vcall", &target, args)
            }
            IrInstr::StaticCall {
                dst,
                function,
                args,
            } => write_call(f, dst.as_ref(), "static_call", function, args),
            IrInstr::BaseCall {
                dst,
                parent_type,
                method,
                args,
            } => {
                let target = format!("{parent_type}::{method}");
                write_call(f, dst.as_ref(), "base_call", &target, args)
            }
            IrInstr::NewVector { dst, elements } => {
                write!(f, "{dst} = vector [{}]", display_list(elements))
            }
            IrInstr::VectorLen { dst, vector } => write!(f, "{dst} = vector_len {vector}"),
            IrInstr::VectorPush { vector, value } => write!(f, "vector_push {vector}, {value}"),
            IrInstr::VectorGet { dst, vector, index } => {
                write!(f, "{dst} = vector_get {vector}[{index}]")
            }
            IrInstr::VectorSet {
                vector,
                index,
                value,
            } => write!(f, "vector_set {vector}[{index}] = {value}"),
            IrInstr::MakeClosure {
                dst,
                function,
                captures,
            } => {
                write!(
                    f,
                    "{dst} = closure {function} captures [{}]",
                    display_list(captures)
                )
            }
            IrInstr::ClosureCall { dst, closure, args } => {
                if let Some(dst) = dst {
                    write!(f, "{dst} = ")?;
                }
                write!(f, "closure_call {closure}({})", display_list(args))
            }
            IrInstr::GetCapture { dst, closure, idx, ty } => {
                write!(f, "{dst} = get_capture {closure}[{idx}]: {ty}")
            }
            IrInstr::TypeTest {
                dst,
                value,
                type_name,
            } => {
                write!(f, "{dst} = type_test {value} is {type_name}")
            }
            IrInstr::TypeCast {
                dst,
                value,
                type_name,
            } => {
                write!(f, "{dst} = type_cast {value} as {type_name}")
            }
            IrInstr::Return(Some(value)) => write!(f, "return {value}"),
            IrInstr::Return(None) => write!(f, "return"),
        }
    }
}

fn write_call(
    f: &mut fmt::Formatter<'_>,
    dst: Option<&IrPlace>,
    kind: &str,
    function: &str,
    args: &[IrValue],
) -> fmt::Result {
    if let Some(dst) = dst {
        write!(f, "{dst} = ")?;
    }
    write!(f, "{kind} {function}({})", display_list(args))
}

fn display_list<T: fmt::Display>(values: &[T]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

impl fmt::Display for IrTypeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrTypeRef::Number => write!(f, "Number"),
            IrTypeRef::String => write!(f, "String"),
            IrTypeRef::Boolean => write!(f, "Boolean"),
            IrTypeRef::Object => write!(f, "Object"),
            IrTypeRef::User(name) => write!(f, "{name}"),
            IrTypeRef::Vector(inner) => write!(f, "{}[]", inner),
            IrTypeRef::Iterable(inner) => write!(f, "{}*", inner),
            IrTypeRef::Functor { params, ret, .. } => {
                write!(f, "({}) -> {}", display_list(params), ret)
            }
            IrTypeRef::Unknown => write!(f, "Unknown"),
        }
    }
}

impl fmt::Display for IrValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrValue::Temp(id) => write!(f, "%t{}", id.0),
            IrValue::Local(id) => write!(f, "%l{}", id.0),
            IrValue::Param(id) => write!(f, "%p{}", id.0),
            IrValue::ConstNumber(value) => write!(f, "{}", format_number(*value)),
            IrValue::ConstBool(value) => write!(f, "{value}"),
            IrValue::DataRef(id) => write!(f, "@s{}", id.0),
            IrValue::Null => write!(f, "null"),
            IrValue::Unit => write!(f, "unit"),
        }
    }
}

impl fmt::Display for IrPlace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrPlace::Temp(id) => write!(f, "%t{}", id.0),
            IrPlace::Local(id) => write!(f, "%l{}", id.0),
        }
    }
}

impl fmt::Display for IrUnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrUnaryOp::Not => write!(f, "!"),
            IrUnaryOp::Neg => write!(f, "-"),
        }
    }
}

impl fmt::Display for IrBinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let op = match self {
            IrBinaryOp::Add => "+",
            IrBinaryOp::Sub => "-",
            IrBinaryOp::Mul => "*",
            IrBinaryOp::Div => "/",
            IrBinaryOp::Mod => "%",
            IrBinaryOp::Pow => "^",
            IrBinaryOp::Concat => "@",
            IrBinaryOp::ConcatSpace => "@@",
            IrBinaryOp::Eq => "==",
            IrBinaryOp::Neq => "!=",
            IrBinaryOp::Lt => "<",
            IrBinaryOp::Le => "<=",
            IrBinaryOp::Gt => ">",
            IrBinaryOp::Ge => ">=",
            IrBinaryOp::And => "&",
            IrBinaryOp::Or => "|",
        };
        write!(f, "{op}")
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn escape_string(value: &str) -> String {
    value
        .chars()
        .flat_map(char::escape_default)
        .collect::<String>()
}
