use crate::builtins::{builtin_constants, builtin_functions};
use crate::context::TypeRegistry;
use crate::error::SemanticError;
use crate::resolver::resolve_program;
use crate::types::Type;
use hulk_frontend::ast::{BinaryOp, Decl, Expr, FunctionDecl, MethodDecl, Param, Program, TypeDecl, TypeMember, UnaryOp};
use std::collections::HashMap;

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FunctionType {
    pub params: Vec<Type>,
    pub return_type: Type,
}

// ── TypeEnv ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TypeEnv {
    scopes: Vec<HashMap<String, Type>>,
    pub functions: HashMap<String, FunctionType>,
    pub registry: TypeRegistry,
    /// Name of the type whose members are currently being checked (for `self`).
    pub current_type: Option<String>,
    /// Parent type of `current_type`, if any (for `base` validation).
    pub current_type_parent: Option<String>,
    /// Name of the method currently being type-checked (for `base()` call resolution).
    pub current_method: Option<String>,
    errors: Vec<SemanticError>,
}

impl TypeEnv {
    fn new(registry: TypeRegistry) -> Self {
        Self {
            scopes: vec![HashMap::new()],
            functions: HashMap::new(),
            registry,
            current_type: None,
            current_type_parent: None,
            current_method: None,
            errors: Vec::new(),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn define_var(&mut self, name: String, ty: Type) {
        self.scopes.last_mut().expect("at least one scope").insert(name, ty);
    }

    fn resolve_var(&self, name: &str) -> Option<Type> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }

    fn define_function(&mut self, name: String, ty: FunctionType) {
        self.functions.insert(name, ty);
    }

    fn resolve_function(&self, name: &str) -> Option<FunctionType> {
        self.functions.get(name).cloned()
    }

    /// Push a semantic error; checking continues with `Type::Unknown` as a fallback.
    pub fn record_error(&mut self, err: SemanticError) {
        self.errors.push(err);
    }

    /// Validate a user-type name against the registry, recording an error on failure.
    fn check_user_type(&mut self, name: &str) -> bool {
        let result = self.registry.validate_user_type(name);
        if let Err(e) = result {
            self.errors.push(e);
            return false;
        }
        true
    }

    fn take_errors(&mut self) -> Vec<SemanticError> {
        std::mem::take(&mut self.errors)
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Type-checks a program.  Returns `Ok(())` when there are no errors, or
/// `Err(errors)` with every error found (not just the first).
pub fn check_program(program: &Program) -> Result<(), Vec<SemanticError>> {
    resolve_program(program).map_err(|e| vec![e])?;

    let registry = TypeRegistry::build(program).map_err(|e| vec![e])?;

    let mut env = TypeEnv::new(registry);
    register_builtin_constants(&mut env);
    register_builtin_functions(&mut env);

    // Pass 1 — register all function signatures.
    for decl in &program.declarations {
        if let Decl::Function(func) = decl {
            register_function_signature(func, &mut env);
        }
    }

    // Pass 2 — check function bodies.
    for decl in &program.declarations {
        if let Decl::Function(func) = decl {
            check_function_body(func, &mut env);
        }
    }

    // Pass 3 — check type member bodies.
    for decl in &program.declarations {
        if let Decl::Type(td) = decl {
            check_type_decl(td, &mut env);
        }
    }

    // Check the program's entry expression.
    infer_expr(&program.entry, &mut env);

    let errors = env.take_errors();
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

// ── Builtins ──────────────────────────────────────────────────────────────────

fn register_builtin_constants(env: &mut TypeEnv) {
    for (name, ty) in builtin_constants() {
        env.define_var(name.to_string(), ty);
    }
}

fn register_builtin_functions(env: &mut TypeEnv) {
    for builtin in builtin_functions() {
        env.define_function(
            builtin.name.to_string(),
            FunctionType { params: builtin.params, return_type: builtin.return_type },
        );
    }
}

// ── Pass 1 helper ─────────────────────────────────────────────────────────────

fn register_function_signature(func: &FunctionDecl, env: &mut TypeEnv) {
    let mut params = Vec::new();
    for param in &func.params {
        params.push(check_parameter_type(param, &func.name, env));
    }

    let ret_ty = if let Some(ret_ref) = &func.return_type {
        let ty = Type::from_type_ref(ret_ref);
        if let Type::UserType(ref name) = ty {
            env.check_user_type(name);
        }
        ty
    } else {
        Type::Unknown
    };

    env.define_function(func.name.clone(), FunctionType { params, return_type: ret_ty });
}

fn check_parameter_type(param: &Param, owner: &str, env: &mut TypeEnv) -> Type {
    if let Some(ty_ref) = &param.ty {
        let ty = Type::from_type_ref(ty_ref);
        if let Type::UserType(ref name) = ty {
            env.check_user_type(name);
        }
        ty
    } else {
        env.record_error(SemanticError::CannotInferParameterType {
            function: owner.to_string(),
            parameter: param.name.clone(),
        });
        Type::Unknown
    }
}

// ── Pass 2 helper ─────────────────────────────────────────────────────────────

fn check_function_body(func: &FunctionDecl, env: &mut TypeEnv) {
    let signature = env
        .resolve_function(&func.name)
        .expect("function should be pre-registered");

    env.push_scope();
    for (idx, param) in func.params.iter().enumerate() {
        env.define_var(param.name.clone(), signature.params[idx].clone());
    }

    let body_ty = infer_expr(&func.body, env);
    env.pop_scope();

    if let Some(ret_ref) = &func.return_type {
        let declared = Type::from_type_ref(ret_ref);
        if !is_assignable(&body_ty, &declared, &env.registry) {
            env.record_error(SemanticError::InvalidReturnType {
                function: func.name.clone(),
                expected: declared,
                found: body_ty,
            });
        }
    } else if let Some(sig) = env.functions.get_mut(&func.name) {
        sig.return_type = body_ty;
    }
}

// ── Pass 3 helpers ────────────────────────────────────────────────────────────

fn check_type_decl(td: &TypeDecl, env: &mut TypeEnv) {
    env.current_type = Some(td.name.clone());
    env.current_type_parent = td.parent.as_ref().map(|p| p.name.clone());

    // Validate parent constructor arguments (from the `inherits Parent(args)` clause).
    // When `parent.args` is None (no `(...)` clause), constructor params are passed through.
    if let Some(parent) = &td.parent {
        if let Some(explicit_args) = &parent.args {
            let ctor_params: Vec<(String, Type)> = env
                .registry
                .get_type(&parent.name)
                .map(|ti| ti.constructor_params.clone())
                .unwrap_or_default();

            if ctor_params.len() != explicit_args.len() {
                env.record_error(SemanticError::ArityMismatch {
                    function: parent.name.clone(),
                    expected: ctor_params.len(),
                    found: explicit_args.len(),
                });
            } else {
                for (idx, arg) in explicit_args.iter().enumerate() {
                    let found = infer_expr(arg, env);
                    let expected = &ctor_params[idx].1;
                    if !is_assignable(&found, expected, &env.registry) {
                        env.record_error(SemanticError::InvalidArgumentType {
                            function: parent.name.clone(),
                            index: idx,
                            expected: expected.clone(),
                            found,
                        });
                    }
                }
            }
        }
        // else: passthrough — no explicit args, parent's constructor params already
        // propagated into this type's constructor_params by context::TypeRegistry::build.
    }

    // Constructor params are visible in attribute initializers and method bodies.
    env.push_scope();
    for param in &td.params {
        let ty = check_parameter_type(param, &td.name, env);
        env.define_var(param.name.clone(), ty);
    }

    for member in &td.members {
        match member {
            TypeMember::Attribute(attr) => {
                let val_ty = infer_expr(&attr.value, env);
                if let Some(ty_ref) = &attr.ty {
                    let declared_ty = Type::from_type_ref(ty_ref);
                    if let Type::UserType(ref name) = declared_ty {
                        env.check_user_type(name);
                    }
                    let ok = is_assignable(&val_ty, &declared_ty, &env.registry);
                    if !ok {
                        env.record_error(SemanticError::TypeMismatch {
                            expected: declared_ty,
                            found: val_ty,
                        });
                    }
                }
            }
            TypeMember::Method(method) => {
                check_method_decl(method, env);
            }
        }
    }

    env.pop_scope();
    env.current_type = None;
    env.current_type_parent = None;
}

fn check_method_decl(method: &MethodDecl, env: &mut TypeEnv) {
    env.current_method = Some(method.name.clone()); // W3a
    env.push_scope();

    for param in &method.params {
        let ty = check_parameter_type(param, &method.name, env);
        env.define_var(param.name.clone(), ty);
    }

    let body_ty = infer_expr(&method.body, env);
    env.pop_scope();
    env.current_method = None; // W3a

    if let Some(ret_ref) = &method.return_type {
        let declared = Type::from_type_ref(ret_ref);
        if let Type::UserType(ref name) = declared {
            if !env.check_user_type(name) {
                return;
            }
        }
        let ok = is_assignable(&body_ty, &declared, &env.registry);
        if !ok {
            env.record_error(SemanticError::InvalidReturnType {
                function: method.name.clone(),
                expected: declared,
                found: body_ty,
            });
        }
    }
}

// ── Core type inference ───────────────────────────────────────────────────────

/// Infer the type of `expr`, recording any errors into `env`.
/// On error the expression still produces a type (`Type::Unknown` when unknown,
/// `Type::Boolean` for relational operators, etc.) so that checking continues.
fn infer_expr(expr: &Expr, env: &mut TypeEnv) -> Type {
    match expr {
        Expr::Number(_) => Type::Number,
        Expr::String(_) => Type::String,
        Expr::Bool(_)   => Type::Boolean,

        Expr::Var(name, _) => {
            match env.resolve_var(name) {
                Some(ty) => ty,
                None => {
                    env.record_error(SemanticError::UndefinedVariable { name: name.clone() });
                    Type::Unknown
                }
            }
        }

        Expr::Unary { op, expr, .. } => {
            let found = infer_expr(expr, env);
            if found == Type::Unknown { return Type::Unknown; }
            match op {
                UnaryOp::Not => {
                    if found == Type::Boolean { Type::Boolean }
                    else {
                        env.record_error(SemanticError::InvalidUnaryOperand { op: op.clone(), found });
                        Type::Unknown
                    }
                }
                UnaryOp::Neg | UnaryOp::Pos => {
                    if found == Type::Number { Type::Number }
                    else {
                        env.record_error(SemanticError::InvalidUnaryOperand { op: op.clone(), found });
                        Type::Unknown
                    }
                }
            }
        }

        Expr::Binary { left, op, right, .. } => {
            let left_ty  = infer_expr(left, env);
            let right_ty = infer_expr(right, env);

            // Propagate Unknown silently — an error was already recorded upstream.
            if left_ty == Type::Unknown || right_ty == Type::Unknown {
                return match op {
                    BinaryOp::Eq | BinaryOp::Neq
                    | BinaryOp::Lt | BinaryOp::Le
                    | BinaryOp::Gt | BinaryOp::Ge
                    | BinaryOp::And | BinaryOp::Or => Type::Boolean,
                    _ => Type::Unknown,
                };
            }

            match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul
                | BinaryOp::Div | BinaryOp::Mod | BinaryOp::Pow => {
                    if left_ty == Type::Number && right_ty == Type::Number {
                        Type::Number
                    } else {
                        env.record_error(SemanticError::InvalidBinaryOperands {
                            op: op.clone(), left: left_ty, right: right_ty,
                        });
                        Type::Unknown
                    }
                }
                BinaryOp::Concat | BinaryOp::ConcatSpace => {
                    if is_concat_compatible(&left_ty) && is_concat_compatible(&right_ty) {
                        Type::String
                    } else {
                        env.record_error(SemanticError::InvalidBinaryOperands {
                            op: op.clone(), left: left_ty, right: right_ty,
                        });
                        Type::Unknown
                    }
                }
                BinaryOp::Eq | BinaryOp::Neq => {
                    let compatible = left_ty == right_ty
                        || is_assignable(&left_ty, &right_ty, &env.registry)
                        || is_assignable(&right_ty, &left_ty, &env.registry);
                    if !compatible {
                        env.record_error(SemanticError::TypeMismatch {
                            expected: left_ty, found: right_ty,
                        });
                    }
                    Type::Boolean
                }
                BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                    if left_ty == Type::Number && right_ty == Type::Number {
                        Type::Boolean
                    } else {
                        env.record_error(SemanticError::InvalidBinaryOperands {
                            op: op.clone(), left: left_ty, right: right_ty,
                        });
                        Type::Boolean
                    }
                }
                BinaryOp::And | BinaryOp::Or => {
                    if left_ty == Type::Boolean && right_ty == Type::Boolean {
                        Type::Boolean
                    } else {
                        env.record_error(SemanticError::InvalidBinaryOperands {
                            op: op.clone(), left: left_ty, right: right_ty,
                        });
                        Type::Boolean
                    }
                }
            }
        }

        Expr::Assign { target, value, .. } => {
            let Expr::Var(name, _) = target.as_ref() else {
                env.record_error(SemanticError::InvalidAssignmentTarget);
                infer_expr(value, env);
                return Type::Unknown;
            };

            let target_ty = match env.resolve_var(name) {
                Some(ty) => ty,
                None => {
                    env.record_error(SemanticError::UndefinedVariable { name: name.clone() });
                    infer_expr(value, env);
                    return Type::Unknown;
                }
            };

            let value_ty = infer_expr(value, env);
            let ok = is_assignable(&value_ty, &target_ty, &env.registry);
            if !ok {
                env.record_error(SemanticError::TypeMismatch {
                    expected: target_ty,
                    found: value_ty.clone(),
                });
            }
            value_ty
        }

        Expr::Let { bindings, body, .. } => {
            env.push_scope();
            for binding in bindings {
                let value_ty = infer_expr(&binding.value, env);
                let bind_ty = if let Some(ty_ref) = &binding.ty {
                    let declared = Type::from_type_ref(ty_ref);
                    if let Type::UserType(ref n) = declared {
                        env.check_user_type(n);
                    }
                    // W3d: When the annotation is a protocol, check structural conformance
                    // instead of (nominal) is_assignable, to enable implicit conformance.
                    let is_proto_annotation = matches!(
                        &declared,
                        Type::UserType(n) if env.registry.get_protocol(n).is_some()
                    );
                    if is_proto_annotation {
                        if let (Type::UserType(proto_name), Type::UserType(concrete_name)) =
                            (&declared, &value_ty)
                        {
                            let proto_name = proto_name.clone();
                            let concrete_name = concrete_name.clone();
                            if !env.registry.implicitly_conforms_to_protocol(
                                &concrete_name,
                                &proto_name,
                            ) {
                                // Find first missing method for a precise error.
                                let method_names: Vec<String> = env
                                    .registry
                                    .get_protocol(&proto_name)
                                    .map(|p| p.methods.keys().cloned().collect())
                                    .unwrap_or_default();
                                let mut missing = proto_name.clone();
                                for m in &method_names {
                                    if env.registry.lookup_method_info(&concrete_name, m).is_none() {
                                        missing = m.clone();
                                        break;
                                    }
                                }
                                env.record_error(SemanticError::MissingProtocolMethod {
                                    type_name: concrete_name,
                                    method_name: missing,
                                });
                            }
                        } else if !is_assignable(&value_ty, &declared, &env.registry) {
                            // Non-UserType value with protocol annotation (e.g. Iterable literal).
                            env.record_error(SemanticError::TypeMismatch {
                                expected: declared.clone(),
                                found: value_ty,
                            });
                        }
                        declared
                    } else {
                        let ok = is_assignable(&value_ty, &declared, &env.registry);
                        if !ok {
                            env.record_error(SemanticError::TypeMismatch {
                                expected: declared.clone(),
                                found: value_ty,
                            });
                        }
                        declared
                    }
                } else {
                    value_ty
                };
                env.define_var(binding.name.clone(), bind_ty);
            }
            let body_ty = infer_expr(body, env);
            env.pop_scope();
            body_ty
        }

        Expr::Call { callee, args, .. } => {
            if let Expr::Var(name, _) = callee.as_ref() {
                // Direct named call — try functions table first.
                if let Some(func) = env.resolve_function(name) {
                    let param_types = func.params.clone();
                    let ret = func.return_type.clone();
                    if param_types.len() != args.len() {
                        env.record_error(SemanticError::ArityMismatch {
                            function: name.clone(),
                            expected: param_types.len(),
                            found: args.len(),
                        });
                        for arg in args { infer_expr(arg, env); }
                        return ret;
                    }
                    for (idx, arg) in args.iter().enumerate() {
                        let found = infer_expr(arg, env);
                        if name == "print" { continue; }
                        let ok = is_assignable(&found, &param_types[idx], &env.registry);
                        if !ok {
                            env.record_error(SemanticError::InvalidArgumentType {
                                function: name.clone(),
                                index: idx,
                                expected: param_types[idx].clone(),
                                found,
                            });
                        }
                    }
                    return ret;
                }

                // Lambda/closure stored in a variable of Functor type.
                if let Some(callee_ty) = env.resolve_var(name) {
                    if let Type::Functor { params, ret } = callee_ty {
                        if params.len() != args.len() {
                            env.record_error(SemanticError::ArityMismatch {
                                function: name.clone(),
                                expected: params.len(),
                                found: args.len(),
                            });
                            for arg in args { infer_expr(arg, env); }
                            return *ret;
                        }
                        for (idx, arg) in args.iter().enumerate() {
                            let found = infer_expr(arg, env);
                            let ok = is_assignable(&found, &params[idx], &env.registry);
                            if !ok {
                                env.record_error(SemanticError::InvalidArgumentType {
                                    function: name.clone(),
                                    index: idx,
                                    expected: params[idx].clone(),
                                    found,
                                });
                            }
                        }
                        return *ret;
                    }
                }

                env.record_error(SemanticError::UndefinedFunction { name: name.clone() });
                for arg in args { infer_expr(arg, env); }
                return Type::Unknown;
            }

            // Arbitrary callee expression (e.g., higher-order result).
            let callee_ty = infer_expr(callee, env);
            for arg in args { infer_expr(arg, env); }
            match callee_ty {
                Type::Functor { ret, .. } => *ret,
                _ => Type::Object,
            }
        }

        Expr::Block(exprs) => {
            env.push_scope();
            let mut ty = Type::Object;
            for expr in exprs {
                ty = infer_expr(expr, env);
            }
            env.pop_scope();
            ty
        }

        Expr::If { branches, else_branch, .. } => {
            let mut unified = Type::Unknown;
            for (cond, body) in branches {
                let cond_ty = infer_expr(cond, env);
                if cond_ty != Type::Boolean && cond_ty != Type::Unknown {
                    env.record_error(SemanticError::InvalidConditionType { found: cond_ty });
                }
                let body_ty = infer_expr(body, env);
                if unified != Type::Unknown
                    && !are_branch_types_compatible(&unified, &body_ty, &env.registry)
                {
                    env.record_error(SemanticError::TypeMismatch {
                        expected: unified.clone(),
                        found: body_ty.clone(),
                    });
                }
                unified = unify_types(&unified, &body_ty, &env.registry);
            }
            let else_ty = infer_expr(else_branch, env);
            if unified != Type::Unknown
                && !are_branch_types_compatible(&unified, &else_ty, &env.registry)
            {
                env.record_error(SemanticError::TypeMismatch {
                    expected: unified.clone(),
                    found: else_ty.clone(),
                });
            }
            unify_types(&unified, &else_ty, &env.registry)
        }

        Expr::While { condition, body, .. } => {
            let cond_ty = infer_expr(condition, env);
            if cond_ty != Type::Boolean && cond_ty != Type::Unknown {
                env.record_error(SemanticError::InvalidConditionType { found: cond_ty });
            }
            infer_expr(body, env)
        }

        Expr::For { iterable, body, var, .. } => {
            let iter_ty = infer_expr(iterable, env);
            let elem_ty = match iter_ty {
                Type::Iterable(inner) | Type::Vector(inner) => *inner,
                Type::Unknown => Type::Unknown,
                other => {
                    env.record_error(SemanticError::InvalidIterableTarget { found: other });
                    Type::Unknown
                }
            };
            env.push_scope();
            env.define_var(var.clone(), elem_ty);
            let body_ty = infer_expr(body, env);
            env.pop_scope();
            body_ty
        }

        Expr::TypeTest { expr, type_name, .. } => {
            infer_expr(expr, env);
            env.check_user_type(type_name);
            Type::Boolean
        }

        Expr::TypeCast { expr, type_name, .. } => {
            infer_expr(expr, env);
            if env.check_user_type(type_name) {
                Type::UserType(type_name.clone())
            } else {
                Type::Unknown
            }
        }

        Expr::VectorLiteral(elements) => {
            let mut elem_ty: Option<Type> = None;
            for el in elements {
                let ty = infer_expr(el, env);
                elem_ty = Some(match elem_ty {
                    None => ty,
                    Some(prev) if prev == ty => prev,
                    Some(prev) => unify_types(&prev, &ty, &env.registry),
                });
            }
            Type::Vector(Box::new(elem_ty.unwrap_or(Type::Object)))
        }

        Expr::VectorGenerator { body, var, iterable, .. } => {
            let iter_ty = infer_expr(iterable, env);
            let elem_ty = match iter_ty {
                Type::Iterable(inner) | Type::Vector(inner) => *inner,
                Type::Unknown => Type::Unknown,
                other => {
                    env.record_error(SemanticError::InvalidIterableTarget { found: other });
                    Type::Unknown
                }
            };
            env.push_scope();
            env.define_var(var.clone(), elem_ty);
            let body_ty = infer_expr(body, env);
            env.pop_scope();
            Type::Vector(Box::new(body_ty))
        }

        Expr::VectorIndex { vector, index, .. } => {
            let vec_ty = infer_expr(vector, env);
            let idx_ty = infer_expr(index, env);
            if idx_ty != Type::Number && idx_ty != Type::Unknown {
                env.record_error(SemanticError::TypeMismatch {
                    expected: Type::Number,
                    found: idx_ty,
                });
            }
            match vec_ty {
                Type::Vector(inner) => *inner,
                Type::Unknown => Type::Unknown,
                other => {
                    env.record_error(SemanticError::InvalidIndexTarget { found: other });
                    Type::Unknown
                }
            }
        }

        Expr::Lambda { params, body, return_type, .. } => {
            env.push_scope();
            let mut param_types = Vec::new();
            for param in params {
                let ty = param.ty.as_ref().map(Type::from_type_ref).unwrap_or(Type::Object);
                param_types.push(ty.clone());
                env.define_var(param.name.clone(), ty);
            }
            let body_ty = infer_expr(body, env);
            env.pop_scope();

            let ret_ty = if let Some(ret_ref) = return_type {
                Type::from_type_ref(ret_ref)
            } else {
                body_ty
            };

            Type::Functor { params: param_types, ret: Box::new(ret_ty) }
        }

        Expr::New { type_name, args, .. } => {
            let ctor_params: Vec<(String, Type)> = match env.registry.get_type(type_name) {
                Some(ti) => ti.constructor_params.clone(),
                None => {
                    env.record_error(SemanticError::UndefinedType { name: type_name.clone() });
                    for arg in args { infer_expr(arg, env); }
                    return Type::Unknown;
                }
            };

            if ctor_params.len() != args.len() {
                env.record_error(SemanticError::ArityMismatch {
                    function: type_name.clone(),
                    expected: ctor_params.len(),
                    found: args.len(),
                });
                for arg in args { infer_expr(arg, env); }
                return Type::UserType(type_name.clone());
            }

            for (idx, arg) in args.iter().enumerate() {
                let found = infer_expr(arg, env);
                let expected = &ctor_params[idx].1;
                let ok = is_assignable(&found, expected, &env.registry);
                if !ok {
                    env.record_error(SemanticError::InvalidArgumentType {
                        function: type_name.clone(),
                        index: idx,
                        expected: expected.clone(),
                        found,
                    });
                }
            }

            Type::UserType(type_name.clone())
        }

        Expr::MemberAccess { object, member, .. } => {
            // W3b: Attributes are private per spec. Only self.attr inside the type's own
            // methods is allowed, and only for attributes the type itself declares.
            let is_self = matches!(object.as_ref(), Expr::SelfRef);
            let obj_ty = infer_expr(object, env);
            if obj_ty == Type::Unknown {
                return Type::Unknown;
            }
            if is_self {
                // self.attr — look only in the current type's OWN attributes (not inherited).
                let Type::UserType(ref tname) = obj_ty else { return Type::Object; };
                let tname = tname.clone();
                if let Some(ti) = env.registry.get_type(&tname) {
                    if let Some(ty) = ti.attributes.get(member.as_str()) {
                        return ty.clone();
                    }
                }
                env.record_error(SemanticError::AttributeIsPrivate {
                    type_name: tname,
                    attr_name: member.clone(),
                });
                Type::Unknown
            } else {
                // Any external attribute access is forbidden.
                let type_name = if let Type::UserType(ref n) = obj_ty { n.clone() } else { String::new() };
                env.record_error(SemanticError::AttributeIsPrivate {
                    type_name,
                    attr_name: member.clone(),
                });
                Type::Unknown
            }
        }

        Expr::MethodCall { object, method, args, .. } => {
            let obj_ty = infer_expr(object, env);

            if obj_ty == Type::Unknown {
                for arg in args { infer_expr(arg, env); }
                return Type::Unknown;
            }

            let Some((type_name, param_types, return_type)) =
                method_signature_for_call(&obj_ty, method, env)
            else {
                env.record_error(SemanticError::UndefinedMethod {
                    type_name: method_receiver_type_name(&obj_ty),
                    method_name: method.clone(),
                });
                for arg in args { infer_expr(arg, env); }
                return Type::Unknown;
            };

            validate_method_call(type_name, method, param_types, return_type, args, env)
        }

        Expr::SelfRef => match &env.current_type {
            Some(tname) => Type::UserType(tname.clone()),
            None => {
                env.record_error(SemanticError::UnsupportedConstruct {
                    message: "Cannot use 'self' outside of a type method".to_string(),
                });
                Type::Unknown
            }
        },

        Expr::BaseCall { args, .. } => {
            // W3c: base(args) in a method body calls the parent's implementation of the
            // CURRENT method, not the parent constructor.
            let current_method = env.current_method.clone();
            let parent_name = env.current_type_parent.clone();

            let Some(method_name) = current_method else {
                env.record_error(SemanticError::UnsupportedConstruct {
                    message: "base() can only be called inside a method body".to_string(),
                });
                for arg in args { infer_expr(arg, env); }
                return Type::Unknown;
            };
            let Some(pname) = parent_name else {
                env.record_error(SemanticError::UnsupportedConstruct {
                    message: "Cannot use 'base' in a type without a parent".to_string(),
                });
                for arg in args { infer_expr(arg, env); }
                return Type::Unknown;
            };

            // Look up the parent's version of the current method.
            let parent_mi = env.registry.lookup_method_info(&pname, &method_name);
            let Some(mi) = parent_mi else {
                env.record_error(SemanticError::UnsupportedConstruct {
                    message: format!("Parent type '{pname}' has no method '{method_name}'"),
                });
                for arg in args { infer_expr(arg, env); }
                return Type::Unknown;
            };
            let param_types = mi.params.clone();
            let ret_type = mi.return_type.clone();

            if param_types.len() != args.len() {
                env.record_error(SemanticError::ArityMismatch {
                    function: format!("base.{method_name}"),
                    expected: param_types.len(),
                    found: args.len(),
                });
                for arg in args { infer_expr(arg, env); }
                return ret_type;
            }
            for (idx, arg) in args.iter().enumerate() {
                let found = infer_expr(arg, env);
                let ok = is_assignable(&found, &param_types[idx], &env.registry);
                if !ok {
                    env.record_error(SemanticError::InvalidArgumentType {
                        function: format!("base.{method_name}"),
                        index: idx,
                        expected: param_types[idx].clone(),
                        found,
                    });
                }
            }
            ret_type
        }
    }
}

fn method_signature_for_call(
    obj_ty: &Type,
    method: &str,
    env: &TypeEnv,
) -> Option<(String, Vec<Type>, Type)> {
    match obj_ty {
        Type::UserType(tname) => env
            .registry
            .lookup_method_info(tname, method)
            .map(|mi| (tname.clone(), mi.params, mi.return_type)),
        Type::Vector(inner) => match method {
            "size" => Some(("Vector".to_string(), vec![], Type::Number)),
            "current" => Some(("Vector".to_string(), vec![], *inner.clone())),
            _ => None,
        },
        Type::Iterable(inner) => match method {
            "next" => Some(("Iterable".to_string(), vec![], Type::Boolean)),
            "current" => Some(("Iterable".to_string(), vec![], *inner.clone())),
            "size" => Some(("Iterable".to_string(), vec![], Type::Number)),
            _ => None,
        },
        _ => None,
    }
}

fn validate_method_call(
    type_name: String,
    method: &str,
    param_types: Vec<Type>,
    return_type: Type,
    args: &[Expr],
    env: &mut TypeEnv,
) -> Type {
    let function = format!("{type_name}.{method}");

    if param_types.len() != args.len() {
        env.record_error(SemanticError::ArityMismatch {
            function,
            expected: param_types.len(),
            found: args.len(),
        });
        for arg in args { infer_expr(arg, env); }
        return return_type;
    }

    for (idx, arg) in args.iter().enumerate() {
        let found = infer_expr(arg, env);
        let expected = &param_types[idx];
        if !is_assignable(&found, expected, &env.registry) {
            env.record_error(SemanticError::InvalidArgumentType {
                function: function.clone(),
                index: idx,
                expected: expected.clone(),
                found,
            });
        }
    }

    return_type
}

fn method_receiver_type_name(ty: &Type) -> String {
    match ty {
        Type::Number => "Number".to_string(),
        Type::String => "String".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::Object => "Object".to_string(),
        Type::UserType(name) => name.clone(),
        Type::Vector(_) => "Vector".to_string(),
        Type::Iterable(_) => "Iterable".to_string(),
        Type::Functor { .. } => "Functor".to_string(),
        Type::Unknown => "Unknown".to_string(),
    }
}

// ── Type helpers ──────────────────────────────────────────────────────────────

/// Registry-aware assignability check.
/// `Unknown` on either side is treated as compatible to avoid cascading errors.
fn is_assignable(sub: &Type, target: &Type, registry: &TypeRegistry) -> bool {
    if *sub == Type::Unknown || *target == Type::Unknown { return true; }
    if sub == target || *target == Type::Object { return true; }
    match (sub, target) {
        (Type::UserType(sn), Type::UserType(tn)) => registry.is_descendant_of(sn, tn),
        (Type::UserType(_), Type::Object) => true,
        (Type::Vector(si), Type::Vector(ti)) => is_assignable(si, ti, registry),
        (Type::Iterable(si), Type::Iterable(ti)) => is_assignable(si, ti, registry),
        // A concrete Iterable value satisfies an `Iterable` protocol annotation.
        (Type::Iterable(_), Type::UserType(n)) if n == "Iterable" => true,
        _ => false,
    }
}

/// Compute the join (least upper bound) of two types.
/// Used to unify the types of all branches of an `if` expression.
fn unify_types(a: &Type, b: &Type, registry: &TypeRegistry) -> Type {
    if *a == Type::Unknown { return b.clone(); }
    if *b == Type::Unknown { return a.clone(); }
    if a == b { return a.clone(); }
    if is_assignable(a, b, registry) { return b.clone(); } // a <: b  →  b is wider
    if is_assignable(b, a, registry) { return a.clone(); } // b <: a  →  a is wider
    match (a, b) {
        (Type::UserType(an), Type::UserType(bn)) => registry.least_common_ancestor(an, bn),
        _ => Type::Object,
    }
}

/// Returns true if two branch types can be unified without a type error.
/// UserType pairs are always compatible (unified via LCA); primitive mismatches are not.
fn are_branch_types_compatible(a: &Type, b: &Type, registry: &TypeRegistry) -> bool {
    if *a == Type::Unknown || *b == Type::Unknown { return true; }
    if a == b || *a == Type::Object || *b == Type::Object { return true; }
    if is_assignable(a, b, registry) || is_assignable(b, a, registry) { return true; }
    matches!((a, b), (Type::UserType(_), Type::UserType(_)))
}

fn is_concat_compatible(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Number | Type::String | Type::Boolean | Type::Object | Type::UserType(_) | Type::Unknown
    )
}
