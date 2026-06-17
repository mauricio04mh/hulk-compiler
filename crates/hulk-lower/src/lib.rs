use hulk_frontend::ast::{BinaryOp, UnaryOp};
use hulk_ir::{
    AttrId, FunctionId, IrAttribute, IrBinaryOp, IrData, IrDataValue, IrFunction, IrFunctionKind,
    IrInstr, IrLocal, IrMethod, IrParam, IrPlace, IrProgram, IrTemp, IrType, IrTypeRef, IrUnaryOp,
    IrValue, LabelId, LocalId, MethodSlot, ParamId, TempId, TypeId,
};
use hulk_sema::hir::{
    DispatchKind, HirAssignTarget, HirCallee, HirDecl, HirExpr, HirExprKind, HirFunctionDecl,
    HirMethodDecl, HirParam, HirTypeDecl, ResolvedMember, SemanticProgram, SymbolId,
};
use hulk_sema::types::Type;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LowerError {
    #[error("unsupported HIR expression for lowering: {kind}")]
    UnsupportedExpression { kind: &'static str },

    #[error("unsupported HIR declaration for lowering: {kind}")]
    UnsupportedDeclaration { kind: &'static str },

    #[error("internal lowering invariant failed: {message}")]
    InternalInvariant { message: String },
}

pub fn lower_program(program: &SemanticProgram) -> Result<IrProgram, LowerError> {
    LoweringContext::new().lower_program(program)
}

#[derive(Debug, Clone)]
struct TypeLayout {
    id: TypeId,
    name: String,
    parent: Option<String>,
    attributes: Vec<IrAttribute>,
    methods: Vec<IrMethod>,
}

struct LoweringContext {
    next_function_id: u32,
    next_data_id: u32,
    next_type_id: u32,
    next_lambda_id: u32,
    data: Vec<IrData>,
    functions: Vec<IrFunction>,
    type_layouts: HashMap<String, TypeLayout>,
    attr_ids: HashMap<(String, String), AttrId>,
    method_slots: HashMap<(String, String), MethodSlot>,
    type_decls: HashMap<String, HirTypeDecl>,
}

impl LoweringContext {
    fn new() -> Self {
        Self {
            next_function_id: 0,
            next_data_id: 0,
            next_type_id: 0,
            next_lambda_id: 0,
            data: Vec::new(),
            functions: Vec::new(),
            type_layouts: HashMap::new(),
            attr_ids: HashMap::new(),
            method_slots: HashMap::new(),
            type_decls: HashMap::new(),
        }
    }

    fn lower_program(mut self, program: &SemanticProgram) -> Result<IrProgram, LowerError> {
        for decl in &program.hir.declarations {
            if let HirDecl::Type(td) = decl {
                self.type_decls.insert(td.name.clone(), td.clone());
            }
        }

        self.lower_type_layouts()?;

        let entry_id = self.new_function_id();
        let mut entry_builder = FunctionBuilder::new(
            entry_id,
            "entry".to_string(),
            IrFunctionKind::Entry,
            lower_type(&program.hir.entry.ty),
        );
        let entry_value = self.lower_expr(&program.hir.entry, &mut entry_builder)?;
        entry_builder.body.push(IrInstr::Return(Some(entry_value)));
        self.functions.push(entry_builder.finish());

        for decl in &program.hir.declarations {
            match decl {
                HirDecl::Function(function) => self.lower_function_decl(function)?,
                HirDecl::Type(ty) => {
                    self.lower_type_init(program, ty)?;
                    for method in &ty.methods {
                        self.lower_method_decl(method)?;
                    }
                }
                HirDecl::Protocol(_) => {}
            }
        }

        let mut types: Vec<IrType> = self
            .type_layouts
            .into_values()
            .map(|layout| IrType {
                id: layout.id,
                name: layout.name,
                parent: layout.parent,
                attributes: layout.attributes,
                methods: layout.methods,
            })
            .collect();
        types.sort_by_key(|ty| ty.id.0);

        Ok(IrProgram {
            types,
            data: self.data,
            functions: self.functions,
            entry: entry_id,
        })
    }

    fn lower_type_layouts(&mut self) -> Result<(), LowerError> {
        let names: Vec<String> = self.type_decls.keys().cloned().collect();
        for name in names {
            self.ensure_type_layout(&name, &mut HashSet::new())?;
        }
        Ok(())
    }

    fn ensure_type_layout(
        &mut self,
        name: &str,
        visiting: &mut HashSet<String>,
    ) -> Result<TypeLayout, LowerError> {
        if let Some(layout) = self.type_layouts.get(name) {
            return Ok(layout.clone());
        }
        if !visiting.insert(name.to_string()) {
            return Err(LowerError::InternalInvariant {
                message: format!("cycle while lowering type layout for '{name}'"),
            });
        }

        let decl =
            self.type_decls
                .get(name)
                .cloned()
                .ok_or_else(|| LowerError::InternalInvariant {
                    message: format!("missing HIR type declaration for '{name}'"),
                })?;

        let parent_name = decl.parent.as_ref().map(|parent| parent.name.clone());
        let mut attributes = Vec::new();
        let mut methods = Vec::new();
        if let Some(parent_name) = &parent_name {
            if self.type_decls.contains_key(parent_name) {
                let parent = self.ensure_type_layout(parent_name, visiting)?;
                attributes.extend(parent.attributes);
                methods.extend(parent.methods);
            }
        }

        for attr in &decl.attributes {
            let id = AttrId(attributes.len() as u32);
            self.attr_ids
                .insert((decl.name.clone(), attr.name.clone()), id);
            attributes.push(IrAttribute {
                id,
                name: attr.name.clone(),
                ty: lower_type(&attr.ty),
            });
        }

        for method in &decl.methods {
            let function = method_label(&method.owner_type, &method.name);
            let slot = if let Some(existing) = methods.iter_mut().find(|m| m.name == method.name) {
                existing.function = function.clone();
                existing.slot
            } else {
                let slot = MethodSlot(methods.len() as u32);
                methods.push(IrMethod {
                    slot,
                    name: method.name.clone(),
                    function: function.clone(),
                });
                slot
            };
            self.method_slots
                .insert((decl.name.clone(), method.name.clone()), slot);
        }

        let layout = TypeLayout {
            id: TypeId(self.next_type_id),
            name: decl.name.clone(),
            parent: parent_name,
            attributes,
            methods,
        };
        self.next_type_id += 1;
        visiting.remove(name);
        self.type_layouts.insert(name.to_string(), layout.clone());
        Ok(layout)
    }

    fn lower_function_decl(&mut self, function: &HirFunctionDecl) -> Result<(), LowerError> {
        let id = self.new_function_id();
        let mut builder = FunctionBuilder::new(
            id,
            function.name.clone(),
            IrFunctionKind::Function,
            lower_type(&function.return_type),
        );
        self.define_params(&mut builder, &function.params);
        let value = self.lower_expr(&function.body, &mut builder)?;
        builder.body.push(IrInstr::Return(Some(value)));
        self.functions.push(builder.finish());
        Ok(())
    }

    fn lower_type_init(
        &mut self,
        program: &SemanticProgram,
        ty: &HirTypeDecl,
    ) -> Result<(), LowerError> {
        let id = self.new_function_id();
        let function_name = init_label(&ty.name);
        let mut builder = FunctionBuilder::new(
            id,
            function_name,
            IrFunctionKind::Function,
            IrTypeRef::User(ty.name.clone()),
        );
        let self_value = builder.new_param(
            "self".to_string(),
            IrTypeRef::User(ty.name.clone()),
            Some(SymbolId(u32::MAX)),
        );
        builder.self_value = Some(self_value.clone());

        let ctor_params = program
            .registry
            .get_type(&ty.name)
            .map(|info| info.constructor_params.clone())
            .unwrap_or_default();
        for (idx, (name, ty_ref)) in ctor_params.iter().enumerate() {
            let symbol = ty
                .params
                .iter()
                .find(|param| param.name == *name)
                .map(|param| param.symbol);
            builder.new_param(name.clone(), lower_type(ty_ref), symbol);
            if idx >= ty.params.len() {
                // Passthrough parent params may not have HIR symbols in the child type.
                continue;
            }
        }

        if let Some(parent) = &ty.parent {
            let mut args = vec![self_value.clone()];
            if let Some(parent_args) = &parent.args {
                for arg in parent_args {
                    args.push(self.lower_expr(arg, &mut builder)?);
                }
            } else {
                for idx in 0..ctor_params.len() {
                    args.push(IrValue::Param(ParamId((idx + 1) as u32)));
                }
            }
            builder.body.push(IrInstr::StaticCall {
                dst: None,
                function: init_label(&parent.name),
                args,
            });
        }

        for attr in &ty.attributes {
            let value = self.lower_expr(&attr.value, &mut builder)?;
            let attr_id = self.attr_id(&ty.name, &attr.name)?;
            builder.body.push(IrInstr::SetAttr {
                object: self_value.clone(),
                attr: attr_id,
                value,
            });
        }

        builder.body.push(IrInstr::Return(Some(self_value)));
        self.functions.push(builder.finish());
        Ok(())
    }

    fn lower_method_decl(&mut self, method: &HirMethodDecl) -> Result<(), LowerError> {
        let id = self.new_function_id();
        let mut builder = FunctionBuilder::new(
            id,
            method_label(&method.owner_type, &method.name),
            IrFunctionKind::Method {
                owner_type: method.owner_type.clone(),
                method_name: method.name.clone(),
            },
            lower_type(&method.return_type),
        );
        let self_value = builder.new_param(
            "self".to_string(),
            IrTypeRef::User(method.owner_type.clone()),
            None,
        );
        builder.self_value = Some(self_value);
        self.define_params(&mut builder, &method.params);
        let value = self.lower_expr(&method.body, &mut builder)?;
        builder.body.push(IrInstr::Return(Some(value)));
        self.functions.push(builder.finish());
        Ok(())
    }

    fn define_params(&mut self, builder: &mut FunctionBuilder, params: &[HirParam]) {
        for param in params {
            builder.new_param(
                param.name.clone(),
                lower_type(&param.ty),
                Some(param.symbol),
            );
        }
    }

    fn lower_expr(
        &mut self,
        expr: &HirExpr,
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        match &expr.kind {
            HirExprKind::Number(value) => Ok(IrValue::ConstNumber(*value)),
            HirExprKind::Bool(value) => Ok(IrValue::ConstBool(*value)),
            HirExprKind::String(value) => Ok(IrValue::DataRef(self.intern_string(value.clone()))),
            HirExprKind::Var { name, symbol } => {
                builder
                    .resolve_symbol(*symbol)
                    .ok_or_else(|| LowerError::InternalInvariant {
                        message: format!("missing IR place for local symbol '{name}'"),
                    })
            }
            HirExprKind::SelfRef { symbol, .. } => builder
                .resolve_symbol(*symbol)
                .or_else(|| builder.self_value.clone())
                .ok_or_else(|| LowerError::InternalInvariant {
                    message: "missing IR value for self".to_string(),
                }),
            HirExprKind::Unary { op, expr: inner } => self.lower_unary(expr, op, inner, builder),
            HirExprKind::Binary { op, left, right } => {
                self.lower_binary(expr, op, left, right, builder)
            }
            HirExprKind::Let { bindings, body } => {
                for binding in bindings {
                    let value = self.lower_expr(&binding.value, builder)?;
                    let place = builder.new_local(binding.name.clone(), lower_type(&binding.ty));
                    builder.define_local_symbol(binding.symbol, place);
                    builder.body.push(IrInstr::Assign {
                        dst: place,
                        src: value,
                    });
                }
                self.lower_expr(body, builder)
            }
            HirExprKind::Block { exprs } => {
                let mut last_value = IrValue::Unit;
                for block_expr in exprs {
                    last_value = self.lower_expr(block_expr, builder)?;
                }
                Ok(last_value)
            }
            HirExprKind::Assign { target, value } => self.lower_assign(target, value, builder),
            HirExprKind::If {
                branches,
                else_branch,
            } => self.lower_if(expr, branches, else_branch, builder),
            HirExprKind::While { condition, body } => {
                self.lower_while(expr, condition, body, builder)
            }
            HirExprKind::Call { callee, args } => self.lower_call(expr, callee, args, builder),
            HirExprKind::New { type_name, args } => self.lower_new(expr, type_name, args, builder),
            HirExprKind::MemberAccess {
                object, resolved, ..
            } => self.lower_member_access(expr, object, resolved, builder),
            HirExprKind::MethodCall {
                object,
                method,
                args,
                dispatch,
            } => self.lower_method_call(expr, object, method, args, dispatch, builder),
            HirExprKind::BaseCall {
                parent_type,
                method_name,
                args,
            } => self.lower_base_call(expr, parent_type, method_name, args, builder),
            HirExprKind::TypeTest {
                expr: inner,
                type_name,
            } => {
                let value = self.lower_expr(inner, builder)?;
                let dst = builder.new_temp(lower_type(&expr.ty));
                builder.body.push(IrInstr::TypeTest {
                    dst,
                    value,
                    type_name: type_name.clone(),
                });
                Ok(dst.into_value())
            }
            HirExprKind::TypeCast {
                expr: inner,
                type_name,
            } => {
                let value = self.lower_expr(inner, builder)?;
                let dst = builder.new_temp(lower_type(&expr.ty));
                builder.body.push(IrInstr::TypeCast {
                    dst,
                    value,
                    type_name: type_name.clone(),
                });
                Ok(dst.into_value())
            }
            HirExprKind::VectorLiteral { elements, .. } => {
                let values = elements
                    .iter()
                    .map(|element| self.lower_expr(element, builder))
                    .collect::<Result<Vec<_>, _>>()?;
                let dst = builder.new_temp(lower_type(&expr.ty));
                builder.body.push(IrInstr::NewVector {
                    dst,
                    elements: values,
                });
                Ok(dst.into_value())
            }
            HirExprKind::VectorIndex { vector, index, .. } => {
                let vector = self.lower_expr(vector, builder)?;
                let index = self.lower_expr(index, builder)?;
                let dst = builder.new_temp(lower_type(&expr.ty));
                builder.body.push(IrInstr::VectorGet { dst, vector, index });
                Ok(dst.into_value())
            }
            HirExprKind::VectorGenerator {
                body,
                var,
                iterable,
                ..
            } => self.lower_vector_generator(expr, body, var, iterable, builder),
            HirExprKind::Lambda {
                params,
                return_type,
                body,
            } => self.lower_lambda(expr, params, return_type, body, builder),
            HirExprKind::For { .. } => Err(LowerError::UnsupportedExpression { kind: "For" }),
        }
    }

    fn lower_unary(
        &mut self,
        expr: &HirExpr,
        op: &UnaryOp,
        inner: &HirExpr,
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        let value = self.lower_expr(inner, builder)?;
        let Some(ir_op) = lower_unary_op(op) else {
            return Ok(value);
        };
        let dst = builder.new_temp(lower_type(&expr.ty));
        builder.body.push(IrInstr::Unary {
            dst,
            op: ir_op,
            value,
        });
        Ok(dst.into_value())
    }

    fn lower_binary(
        &mut self,
        expr: &HirExpr,
        op: &BinaryOp,
        left: &HirExpr,
        right: &HirExpr,
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        if matches!(op, BinaryOp::ConcatSpace) {
            let left = self.lower_expr(left, builder)?;
            let space = IrValue::DataRef(self.intern_string(" ".to_string()));
            let tmp = builder.new_temp(IrTypeRef::String);
            builder.body.push(IrInstr::Binary {
                dst: tmp,
                op: IrBinaryOp::Concat,
                left,
                right: space,
            });

            let right = self.lower_expr(right, builder)?;
            let dst = builder.new_temp(lower_type(&expr.ty));
            builder.body.push(IrInstr::Binary {
                dst,
                op: IrBinaryOp::Concat,
                left: tmp.into_value(),
                right,
            });
            return Ok(dst.into_value());
        }

        let left = self.lower_expr(left, builder)?;
        let right = self.lower_expr(right, builder)?;
        let dst = builder.new_temp(lower_type(&expr.ty));
        builder.body.push(IrInstr::Binary {
            dst,
            op: lower_binary_op(op),
            left,
            right,
        });
        Ok(dst.into_value())
    }

    fn lower_assign(
        &mut self,
        target: &HirAssignTarget,
        value: &HirExpr,
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        let value = self.lower_expr(value, builder)?;
        match target {
            HirAssignTarget::Local { name, symbol, .. } => {
                let place = builder.resolve_symbol_place(*symbol).ok_or_else(|| {
                    LowerError::InternalInvariant {
                        message: format!("missing IR place for assignment target '{name}'"),
                    }
                })?;
                builder.body.push(IrInstr::Assign {
                    dst: place,
                    src: value.clone(),
                });
                Ok(value)
            }
            HirAssignTarget::SelfAttribute {
                owner_type,
                attr_name,
                ..
            } => {
                let self_value =
                    builder
                        .self_value
                        .clone()
                        .ok_or_else(|| LowerError::InternalInvariant {
                            message: "self attribute assignment without self".to_string(),
                        })?;
                let attr = self.attr_id(owner_type, attr_name)?;
                builder.body.push(IrInstr::SetAttr {
                    object: self_value,
                    attr,
                    value: value.clone(),
                });
                Ok(value)
            }
        }
    }

    fn lower_if(
        &mut self,
        expr: &HirExpr,
        branches: &[(HirExpr, HirExpr)],
        else_branch: &HirExpr,
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        let result = builder.new_temp(lower_type(&expr.ty));
        let end_label = builder.new_label();
        for (condition, body) in branches {
            let then_label = builder.new_label();
            let next_label = builder.new_label();
            let condition = self.lower_expr(condition, builder)?;
            builder.body.push(IrInstr::Branch {
                cond: condition,
                then_label,
                else_label: next_label,
            });
            builder.body.push(IrInstr::Label(then_label));
            let body_value = self.lower_expr(body, builder)?;
            builder.body.push(IrInstr::Assign {
                dst: result,
                src: body_value,
            });
            builder.body.push(IrInstr::Jump(end_label));
            builder.body.push(IrInstr::Label(next_label));
        }
        let else_value = self.lower_expr(else_branch, builder)?;
        builder.body.push(IrInstr::Assign {
            dst: result,
            src: else_value,
        });
        builder.body.push(IrInstr::Label(end_label));
        Ok(result.into_value())
    }

    fn lower_while(
        &mut self,
        expr: &HirExpr,
        condition: &HirExpr,
        body: &HirExpr,
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        let result = builder.new_temp(lower_type(&expr.ty));
        let check_label = builder.new_label();
        let body_label = builder.new_label();
        let end_label = builder.new_label();
        builder.body.push(IrInstr::Assign {
            dst: result,
            src: IrValue::Unit,
        });
        builder.body.push(IrInstr::Label(check_label));
        let condition = self.lower_expr(condition, builder)?;
        builder.body.push(IrInstr::Branch {
            cond: condition,
            then_label: body_label,
            else_label: end_label,
        });
        builder.body.push(IrInstr::Label(body_label));
        let body_value = self.lower_expr(body, builder)?;
        builder.body.push(IrInstr::Assign {
            dst: result,
            src: body_value,
        });
        builder.body.push(IrInstr::Jump(check_label));
        builder.body.push(IrInstr::Label(end_label));
        Ok(result.into_value())
    }

    fn lower_call(
        &mut self,
        expr: &HirExpr,
        callee: &HirCallee,
        args: &[HirExpr],
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        let args = args
            .iter()
            .map(|arg| self.lower_expr(arg, builder))
            .collect::<Result<Vec<_>, _>>()?;
        let dst = builder.new_temp(lower_type(&expr.ty));
        match callee {
            HirCallee::Builtin { name, .. } | HirCallee::GlobalFunction { name, .. } => {
                builder.body.push(IrInstr::Call {
                    dst: Some(dst),
                    function: name.clone(),
                    args,
                });
            }
            HirCallee::LocalFunctor { name, symbol, .. } => {
                let closure = builder.resolve_symbol(*symbol).ok_or_else(|| {
                    LowerError::InternalInvariant {
                        message: format!("missing closure value for functor '{name}'"),
                    }
                })?;
                builder.body.push(IrInstr::ClosureCall {
                    dst: Some(dst),
                    closure,
                    args,
                });
            }
        }
        Ok(dst.into_value())
    }

    fn lower_new(
        &mut self,
        expr: &HirExpr,
        type_name: &str,
        args: &[HirExpr],
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        let arg_values = args
            .iter()
            .map(|arg| self.lower_expr(arg, builder))
            .collect::<Result<Vec<_>, _>>()?;
        let dst = builder.new_temp(lower_type(&expr.ty));
        builder.body.push(IrInstr::Allocate {
            dst,
            type_name: type_name.to_string(),
        });
        let mut init_args = vec![dst.into_value()];
        init_args.extend(arg_values);
        builder.body.push(IrInstr::StaticCall {
            dst: Some(dst),
            function: init_label(type_name),
            args: init_args,
        });
        Ok(dst.into_value())
    }

    fn lower_member_access(
        &mut self,
        expr: &HirExpr,
        object: &HirExpr,
        resolved: &ResolvedMember,
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        let object = self.lower_expr(object, builder)?;
        let ResolvedMember::Attribute {
            owner_type,
            attr_name,
            ..
        } = resolved;
        let attr = self.attr_id(owner_type, attr_name)?;
        let dst = builder.new_temp(lower_type(&expr.ty));
        builder.body.push(IrInstr::GetAttr { dst, object, attr });
        Ok(dst.into_value())
    }

    fn lower_method_call(
        &mut self,
        expr: &HirExpr,
        object: &HirExpr,
        method: &str,
        args: &[HirExpr],
        dispatch: &DispatchKind,
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        let object_value = self.lower_expr(object, builder)?;
        let arg_values = args
            .iter()
            .map(|arg| self.lower_expr(arg, builder))
            .collect::<Result<Vec<_>, _>>()?;
        let dst = builder.new_temp(lower_type(&expr.ty));
        match dispatch {
            DispatchKind::Virtual {
                receiver_static_type,
                method_name,
                ..
            } => {
                let receiver_type = type_name_for_dispatch(receiver_static_type);
                let slot = self
                    .method_slots
                    .get(&(receiver_type.clone(), method_name.clone()))
                    .copied()
                    .unwrap_or(MethodSlot(0));
                let mut args = vec![object_value.clone()];
                args.extend(arg_values);
                builder.body.push(IrInstr::VirtualCall {
                    dst: Some(dst),
                    receiver: object_value,
                    receiver_static_type: receiver_type,
                    method: method.to_string(),
                    slot,
                    args,
                });
            }
            DispatchKind::Static { function_label, .. } => {
                let mut args = vec![object_value];
                args.extend(arg_values);
                builder.body.push(IrInstr::StaticCall {
                    dst: Some(dst),
                    function: function_label.clone(),
                    args,
                });
            }
            DispatchKind::Base {
                parent_type,
                method_name,
                ..
            } => {
                let mut args = vec![object_value];
                args.extend(arg_values);
                builder.body.push(IrInstr::BaseCall {
                    dst: Some(dst),
                    parent_type: parent_type.clone(),
                    method: method_name.clone(),
                    args,
                });
            }
        }
        Ok(dst.into_value())
    }

    fn lower_base_call(
        &mut self,
        expr: &HirExpr,
        parent_type: &str,
        method_name: &str,
        args: &[HirExpr],
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        let self_value =
            builder
                .self_value
                .clone()
                .ok_or_else(|| LowerError::InternalInvariant {
                    message: "base call without self".to_string(),
                })?;
        let mut arg_values = vec![self_value];
        for arg in args {
            arg_values.push(self.lower_expr(arg, builder)?);
        }
        let dst = builder.new_temp(lower_type(&expr.ty));
        builder.body.push(IrInstr::BaseCall {
            dst: Some(dst),
            parent_type: parent_type.to_string(),
            method: method_name.to_string(),
            args: arg_values,
        });
        Ok(dst.into_value())
    }

    fn lower_vector_generator(
        &mut self,
        expr: &HirExpr,
        body: &HirExpr,
        var: &HirParam,
        iterable: &HirExpr,
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        let result = builder.new_temp(lower_type(&expr.ty));
        builder.body.push(IrInstr::NewVector {
            dst: result,
            elements: Vec::new(),
        });
        let iterable = self.lower_expr(iterable, builder)?;
        let iter_local = builder.new_local("_gen_iter".to_string(), lower_type(&Type::Object));
        builder.body.push(IrInstr::Assign {
            dst: iter_local,
            src: iterable,
        });

        let check_label = builder.new_label();
        let body_label = builder.new_label();
        let end_label = builder.new_label();
        builder.body.push(IrInstr::Label(check_label));
        let cond = builder.new_temp(IrTypeRef::Boolean);
        builder.body.push(IrInstr::VirtualCall {
            dst: Some(cond),
            receiver: iter_local.into_value(),
            receiver_static_type: "Iterable".to_string(),
            method: "next".to_string(),
            slot: MethodSlot(0),
            args: vec![iter_local.into_value()],
        });
        builder.body.push(IrInstr::Branch {
            cond: cond.into_value(),
            then_label: body_label,
            else_label: end_label,
        });
        builder.body.push(IrInstr::Label(body_label));

        let var_place = builder.new_local(var.name.clone(), lower_type(&var.ty));
        builder.define_local_symbol(var.symbol, var_place);
        builder.body.push(IrInstr::VirtualCall {
            dst: Some(var_place),
            receiver: iter_local.into_value(),
            receiver_static_type: "Iterable".to_string(),
            method: "current".to_string(),
            slot: MethodSlot(1),
            args: vec![iter_local.into_value()],
        });
        let body_value = self.lower_expr(body, builder)?;
        builder.body.push(IrInstr::VectorPush {
            vector: result.into_value(),
            value: body_value,
        });
        builder.body.push(IrInstr::Jump(check_label));
        builder.body.push(IrInstr::Label(end_label));
        Ok(result.into_value())
    }

    fn lower_lambda(
        &mut self,
        expr: &HirExpr,
        params: &[HirParam],
        return_type: &Type,
        body: &HirExpr,
        builder: &mut FunctionBuilder,
    ) -> Result<IrValue, LowerError> {
        let name = format!("lambda_{}", self.next_lambda_id);
        self.next_lambda_id += 1;
        let mut captures = Vec::new();
        collect_captures(
            body,
            &params.iter().map(|p| p.symbol).collect(),
            &mut captures,
        );
        captures.retain(|symbol| builder.resolve_symbol_place(*symbol).is_some());

        let id = self.new_function_id();
        let mut lambda_builder = FunctionBuilder::new(
            id,
            name.clone(),
            IrFunctionKind::Lambda,
            lower_type(return_type),
        );
        let mut capture_values = Vec::new();
        for (idx, symbol) in captures.iter().enumerate() {
            let value =
                builder
                    .resolve_symbol(*symbol)
                    .ok_or_else(|| LowerError::InternalInvariant {
                        message: "missing capture value".to_string(),
                    })?;
            capture_values.push(value);
            lambda_builder.new_param(format!("capture{idx}"), IrTypeRef::Object, Some(*symbol));
        }
        self.define_params(&mut lambda_builder, params);
        let value = self.lower_expr(body, &mut lambda_builder)?;
        lambda_builder.body.push(IrInstr::Return(Some(value)));
        self.functions.push(lambda_builder.finish());

        let dst = builder.new_temp(lower_type(&expr.ty));
        builder.body.push(IrInstr::MakeClosure {
            dst,
            function: name,
            captures: capture_values,
        });
        Ok(dst.into_value())
    }

    fn attr_id(&self, owner_type: &str, attr_name: &str) -> Result<AttrId, LowerError> {
        self.attr_ids
            .get(&(owner_type.to_string(), attr_name.to_string()))
            .copied()
            .or_else(|| {
                self.type_layouts.get(owner_type).and_then(|layout| {
                    layout
                        .attributes
                        .iter()
                        .find(|attr| attr.name == attr_name)
                        .map(|attr| attr.id)
                })
            })
            .ok_or_else(|| LowerError::InternalInvariant {
                message: format!("missing attr id for {owner_type}.{attr_name}"),
            })
    }

    fn intern_string(&mut self, value: String) -> hulk_ir::DataId {
        let id = hulk_ir::DataId(self.next_data_id);
        self.next_data_id += 1;
        self.data.push(IrData {
            id,
            value: IrDataValue::String(value),
        });
        id
    }

    fn new_function_id(&mut self) -> FunctionId {
        let id = FunctionId(self.next_function_id);
        self.next_function_id += 1;
        id
    }
}

struct FunctionBuilder {
    id: FunctionId,
    name: String,
    kind: IrFunctionKind,
    return_type: IrTypeRef,
    params: Vec<IrParam>,
    locals: Vec<IrLocal>,
    temps: Vec<IrTemp>,
    body: Vec<IrInstr>,
    symbol_values: HashMap<SymbolId, IrValue>,
    symbol_places: HashMap<SymbolId, IrPlace>,
    self_value: Option<IrValue>,
    next_param_id: u32,
    next_local_id: u32,
    next_temp_id: u32,
    next_label_id: u32,
}

impl FunctionBuilder {
    fn new(id: FunctionId, name: String, kind: IrFunctionKind, return_type: IrTypeRef) -> Self {
        Self {
            id,
            name,
            kind,
            return_type,
            params: Vec::new(),
            locals: Vec::new(),
            temps: Vec::new(),
            body: Vec::new(),
            symbol_values: HashMap::new(),
            symbol_places: HashMap::new(),
            self_value: None,
            next_param_id: 0,
            next_local_id: 0,
            next_temp_id: 0,
            next_label_id: 0,
        }
    }

    fn new_param(&mut self, name: String, ty: IrTypeRef, symbol: Option<SymbolId>) -> IrValue {
        let id = ParamId(self.next_param_id);
        self.next_param_id += 1;
        self.params.push(IrParam { id, name, ty });
        let value = IrValue::Param(id);
        if let Some(symbol) = symbol {
            self.symbol_values.insert(symbol, value.clone());
        }
        value
    }

    fn new_local(&mut self, name: String, ty: IrTypeRef) -> IrPlace {
        let id = LocalId(self.next_local_id);
        self.next_local_id += 1;
        self.locals.push(IrLocal { id, name, ty });
        IrPlace::Local(id)
    }

    fn new_temp(&mut self, ty: IrTypeRef) -> IrPlace {
        let id = TempId(self.next_temp_id);
        self.next_temp_id += 1;
        self.temps.push(IrTemp { id, ty });
        IrPlace::Temp(id)
    }

    fn new_label(&mut self) -> LabelId {
        let id = LabelId(self.next_label_id);
        self.next_label_id += 1;
        id
    }

    fn define_local_symbol(&mut self, symbol: SymbolId, place: IrPlace) {
        self.symbol_places.insert(symbol, place);
        self.symbol_values.insert(symbol, place.into_value());
    }

    fn resolve_symbol(&self, symbol: SymbolId) -> Option<IrValue> {
        self.symbol_values.get(&symbol).cloned()
    }

    fn resolve_symbol_place(&self, symbol: SymbolId) -> Option<IrPlace> {
        self.symbol_places.get(&symbol).copied()
    }

    fn finish(self) -> IrFunction {
        IrFunction {
            id: self.id,
            name: self.name,
            kind: self.kind,
            params: self.params,
            locals: self.locals,
            temps: self.temps,
            return_type: self.return_type,
            body: self.body,
        }
    }
}

trait PlaceExt {
    fn into_value(self) -> IrValue;
}

impl PlaceExt for IrPlace {
    fn into_value(self) -> IrValue {
        match self {
            IrPlace::Temp(id) if id.0 > u32::MAX / 2 => IrValue::Param(ParamId(u32::MAX - id.0)),
            IrPlace::Temp(id) => IrValue::Temp(id),
            IrPlace::Local(id) => IrValue::Local(id),
        }
    }
}

fn collect_captures(expr: &HirExpr, bound: &HashSet<SymbolId>, captures: &mut Vec<SymbolId>) {
    match &expr.kind {
        HirExprKind::Var { symbol, .. } | HirExprKind::SelfRef { symbol, .. } => {
            if !bound.contains(symbol) && !captures.contains(symbol) {
                captures.push(*symbol);
            }
        }
        HirExprKind::Unary { expr, .. } => collect_captures(expr, bound, captures),
        HirExprKind::Binary { left, right, .. } => {
            collect_captures(left, bound, captures);
            collect_captures(right, bound, captures);
        }
        HirExprKind::Assign { value, .. } => collect_captures(value, bound, captures),
        HirExprKind::Let { bindings, body } => {
            let mut next_bound = bound.clone();
            for binding in bindings {
                collect_captures(&binding.value, &next_bound, captures);
                next_bound.insert(binding.symbol);
            }
            collect_captures(body, &next_bound, captures);
        }
        HirExprKind::Block { exprs } => {
            for expr in exprs {
                collect_captures(expr, bound, captures);
            }
        }
        HirExprKind::If {
            branches,
            else_branch,
        } => {
            for (condition, body) in branches {
                collect_captures(condition, bound, captures);
                collect_captures(body, bound, captures);
            }
            collect_captures(else_branch, bound, captures);
        }
        HirExprKind::While { condition, body } => {
            collect_captures(condition, bound, captures);
            collect_captures(body, bound, captures);
        }
        HirExprKind::Call { args, .. } => {
            for arg in args {
                collect_captures(arg, bound, captures);
            }
        }
        HirExprKind::New { args, .. } | HirExprKind::BaseCall { args, .. } => {
            for arg in args {
                collect_captures(arg, bound, captures);
            }
        }
        HirExprKind::MemberAccess { object, .. } => collect_captures(object, bound, captures),
        HirExprKind::MethodCall { object, args, .. } => {
            collect_captures(object, bound, captures);
            for arg in args {
                collect_captures(arg, bound, captures);
            }
        }
        HirExprKind::TypeTest { expr, .. } | HirExprKind::TypeCast { expr, .. } => {
            collect_captures(expr, bound, captures);
        }
        HirExprKind::VectorLiteral { elements, .. } => {
            for element in elements {
                collect_captures(element, bound, captures);
            }
        }
        HirExprKind::VectorGenerator {
            body,
            var,
            iterable,
            ..
        } => {
            collect_captures(iterable, bound, captures);
            let mut next_bound = bound.clone();
            next_bound.insert(var.symbol);
            collect_captures(body, &next_bound, captures);
        }
        HirExprKind::VectorIndex { vector, index, .. } => {
            collect_captures(vector, bound, captures);
            collect_captures(index, bound, captures);
        }
        HirExprKind::Lambda { params, body, .. } => {
            let mut next_bound = bound.clone();
            for param in params {
                next_bound.insert(param.symbol);
            }
            collect_captures(body, &next_bound, captures);
        }
        HirExprKind::Number(_) | HirExprKind::String(_) | HirExprKind::Bool(_) => {}
        HirExprKind::For {
            iterable,
            body,
            var,
            ..
        } => {
            collect_captures(iterable, bound, captures);
            let mut next_bound = bound.clone();
            next_bound.insert(var.symbol);
            collect_captures(body, &next_bound, captures);
        }
    }
}

fn lower_unary_op(op: &UnaryOp) -> Option<IrUnaryOp> {
    match op {
        UnaryOp::Not => Some(IrUnaryOp::Not),
        UnaryOp::Neg => Some(IrUnaryOp::Neg),
        UnaryOp::Pos => None,
    }
}

fn lower_binary_op(op: &BinaryOp) -> IrBinaryOp {
    match op {
        BinaryOp::Add => IrBinaryOp::Add,
        BinaryOp::Sub => IrBinaryOp::Sub,
        BinaryOp::Mul => IrBinaryOp::Mul,
        BinaryOp::Div => IrBinaryOp::Div,
        BinaryOp::Mod => IrBinaryOp::Mod,
        BinaryOp::Pow => IrBinaryOp::Pow,
        BinaryOp::Concat => IrBinaryOp::Concat,
        BinaryOp::ConcatSpace => IrBinaryOp::ConcatSpace,
        BinaryOp::Eq => IrBinaryOp::Eq,
        BinaryOp::Neq => IrBinaryOp::Neq,
        BinaryOp::Lt => IrBinaryOp::Lt,
        BinaryOp::Le => IrBinaryOp::Le,
        BinaryOp::Gt => IrBinaryOp::Gt,
        BinaryOp::Ge => IrBinaryOp::Ge,
        BinaryOp::And => IrBinaryOp::And,
        BinaryOp::Or => IrBinaryOp::Or,
    }
}

fn lower_type(ty: &Type) -> IrTypeRef {
    match ty {
        Type::Number => IrTypeRef::Number,
        Type::String => IrTypeRef::String,
        Type::Boolean => IrTypeRef::Boolean,
        Type::Object => IrTypeRef::Object,
        Type::UserType(name) => IrTypeRef::User(name.clone()),
        Type::Vector(inner) => IrTypeRef::Vector(Box::new(lower_type(inner))),
        Type::Iterable(inner) => IrTypeRef::Iterable(Box::new(lower_type(inner))),
        Type::Functor { params, ret } => IrTypeRef::Functor {
            params: params.iter().map(lower_type).collect(),
            ret: Box::new(lower_type(ret)),
        },
        Type::Unknown => IrTypeRef::Unknown,
    }
}

fn type_name_for_dispatch(ty: &Type) -> String {
    match ty {
        Type::UserType(name) => name.clone(),
        Type::Vector(_) => "Vector".to_string(),
        Type::Iterable(_) => "Iterable".to_string(),
        Type::Number => "Number".to_string(),
        Type::String => "String".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::Object | Type::Functor { .. } | Type::Unknown => "Object".to_string(),
    }
}

fn init_label(type_name: &str) -> String {
    format!("{type_name}_init")
}

fn method_label(owner_type: &str, method_name: &str) -> String {
    format!("{owner_type}_{method_name}")
}
