use crate::builtins::{builtin_constant_value, builtin_constants, builtin_functions};
use crate::checker::FunctionType;
use crate::context::TypeRegistry;
use crate::error::SemanticError;
use crate::hir::{
    DispatchKind, HirAssignTarget, HirAttributeDecl, HirCallee, HirDecl, HirExpr, HirExprKind,
    HirFunctionDecl, HirId, HirLetBinding, HirMethodDecl, HirParam, HirParent, HirProgram,
    HirProtocolDecl, HirProtocolMethod, HirTypeDecl, ResolvedMember, SemanticProgram, SymbolId,
};
use crate::types::Type;
use hulk_frontend::ast::{
    AttributeDecl, BinaryOp, Decl, Expr, FunctionDecl, LiteralPattern, MatchArm, MethodDecl,
    Param, Pattern, Program, ProtocolDecl, Span, TypeDecl, TypeMember, TypeParent, TypeRef,
    UnaryOp,
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub id: SymbolId,
    pub name: String,
    pub ty: Type,
    pub kind: SymbolKind,
}

struct PassResult {
    semantic: SemanticProgram,
    signature_changed: bool,
    functions: HashMap<String, FunctionType>,
    methods: HashMap<String, FunctionType>,
    types: HashMap<String, Vec<Type>>,
}

#[derive(Debug, Clone)]
pub enum SymbolKind {
    Local,
    Parameter,
    SelfValue,
    BuiltinConstant,
}

pub struct HirBuilder {
    registry: TypeRegistry,
    functions: HashMap<String, FunctionType>,
    /// AST bodies of `define` macro functions, keyed by function name.
    macro_decls: HashMap<String, FunctionDecl>,
    scopes: Vec<HashMap<String, SymbolInfo>>,
    errors: Vec<SemanticError>,
    current_type: Option<String>,
    current_type_parent: Option<String>,
    current_method: Option<String>,
    current_function: Option<String>,
    current_function_param_indices: HashMap<SymbolId, usize>,
    type_signatures: HashMap<String, Vec<Type>>,
    method_signatures: HashMap<String, FunctionType>,
    report_unresolved_parameter_errors: bool,
    signature_changed: bool,
    next_hir_id: u32,
    next_symbol_id: u32,
}

impl HirBuilder {
    pub fn new(registry: TypeRegistry) -> Self {
        Self {
            registry,
            functions: HashMap::new(),
            macro_decls: HashMap::new(),
            scopes: vec![HashMap::new()],
            errors: Vec::new(),
            current_type: None,
            current_type_parent: None,
            current_method: None,
            current_function: None,
            current_function_param_indices: HashMap::new(),
            type_signatures: HashMap::new(),
            method_signatures: HashMap::new(),
            report_unresolved_parameter_errors: false,
            signature_changed: false,
            next_hir_id: 0,
            next_symbol_id: 0,
        }
    }

    pub fn analyze_program(self, program: &Program) -> Result<SemanticProgram, Vec<SemanticError>> {
        let mut functions = HashMap::new();
        let mut types = HashMap::new();
        let mut methods = HashMap::new();

        loop {
            let mut pass_builder = HirBuilder::new(self.registry.clone());
            pass_builder.functions = functions.clone();
            pass_builder.type_signatures = types.clone();
            pass_builder.method_signatures = methods.clone();
            let result = pass_builder.run_pass(program, false)?;

            if result.signature_changed {
                functions = result.functions;
                types = result.types;
                methods = result.methods;
                continue;
            }

            let mut final_builder = HirBuilder::new(self.registry.clone());
            final_builder.functions = result.functions;
            final_builder.type_signatures = result.types;
            final_builder.method_signatures = result.methods;
            return final_builder
                .run_pass(program, true)
                .map(|result| result.semantic);
        }
    }

    fn run_pass(
        mut self,
        program: &Program,
        report_unresolved_parameter_errors: bool,
    ) -> Result<PassResult, Vec<SemanticError>> {
        self.report_unresolved_parameter_errors = report_unresolved_parameter_errors;
        self.signature_changed = false;

        self.register_builtin_constants();
        self.register_builtin_functions();

        for decl in &program.declarations {
            if let Decl::Function(func) = decl {
                self.register_function_signature(func);
                if func.is_macro {
                    self.macro_decls.insert(func.name.clone(), func.clone());
                }
            }
        }

        for decl in &program.declarations {
            if let Decl::Type(td) = decl {
                self.register_type_signatures(td);
            }
        }

        for decl in &program.declarations {
            if let Decl::Type(td) = decl {
                self.register_method_signatures(td);
            }
        }

        let mut declarations = Vec::new();
        for decl in &program.declarations {
            match decl {
                Decl::Function(func) => {
                    declarations.push(HirDecl::Function(self.analyze_function_decl(func)));
                }
                Decl::Type(td) => {
                    declarations.push(HirDecl::Type(self.analyze_type_decl(td)));
                }
                Decl::Protocol(pd) => {
                    declarations.push(HirDecl::Protocol(self.analyze_protocol_decl(pd)));
                }
            }
        }

        let entry = self.analyze_expr(&program.entry);

        if self.errors.is_empty() {
            Ok(PassResult {
                semantic: SemanticProgram {
                    hir: HirProgram {
                        declarations,
                        entry,
                    },
                    registry: self.registry,
                    functions: self.functions.clone(),
                },
                signature_changed: self.signature_changed,
                functions: self.functions,
                methods: self.method_signatures,
                types: self.type_signatures,
            })
        } else {
            Err(self.errors)
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

    fn define_symbol(&mut self, name: String, ty: Type, kind: SymbolKind) -> SymbolInfo {
        let symbol = SymbolInfo {
            id: self.new_symbol_id(),
            name: name.clone(),
            ty,
            kind,
        };
        self.scopes
            .last_mut()
            .expect("at least one scope")
            .insert(name, symbol.clone());
        symbol
    }

    fn resolve_symbol(&self, name: &str) -> Option<SymbolInfo> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn resolve_symbol_mut(&mut self, name: &str) -> Option<&mut SymbolInfo> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(symbol) = scope.get_mut(name) {
                return Some(symbol);
            }
        }
        None
    }

    fn constrain_expr_type(&mut self, expr: &Expr, expected: &Type) {
        if *expected == Type::Unknown {
            return;
        }

        let Expr::Var(name, _) = expr else {
            return;
        };

        let Some(symbol) = self.resolve_symbol(name) else {
            return;
        };
        if !matches!(symbol.kind, SymbolKind::Parameter) {
            return;
        }

        if let Some(current_function) = self.current_function.clone() {
            let Some(&idx) = self.current_function_param_indices.get(&symbol.id) else {
                return;
            };

            let current_ty = self
                .functions
                .get(&current_function)
                .and_then(|signature| signature.params.get(idx))
                .cloned()
                .unwrap_or(Type::Unknown);

            if current_ty == Type::Unknown {
                self.refine_function_param_type(&current_function, idx, expected);
                if let Some(symbol_mut) = self.resolve_symbol_mut(name) {
                    symbol_mut.ty = expected.clone();
                }
                return;
            }

            if current_ty != *expected {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: current_ty,
                    found: expected.clone(),
                });
            }
            return;
        }

        if self.current_method.is_some() {
            let current_ty = symbol.ty.clone();
            if current_ty == Type::Unknown {
                if let Some(symbol_mut) = self.resolve_symbol_mut(name) {
                    symbol_mut.ty = expected.clone();
                }
                return;
            }

            if current_ty != *expected {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: current_ty,
                    found: expected.clone(),
                });
            }
            return;
        }

        if self.current_type.is_some() {
            let current_ty = symbol.ty.clone();
            if current_ty == Type::Unknown {
                if let Some(symbol_mut) = self.resolve_symbol_mut(name) {
                    symbol_mut.ty = expected.clone();
                }
                return;
            }

            if current_ty != *expected {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: current_ty,
                    found: expected.clone(),
                });
            }
        }
    }

    fn refine_function_param_type(&mut self, function: &str, idx: usize, expected: &Type) {
        if *expected == Type::Unknown {
            return;
        }

        let Some(signature) = self.functions.get_mut(function) else {
            return;
        };
        let Some(current) = signature.params.get(idx).cloned() else {
            return;
        };

        if current == Type::Unknown {
            signature.params[idx] = expected.clone();
            self.signature_changed = true;
            return;
        }

        if current != *expected {
            self.errors.push(SemanticError::TypeMismatch {
                expected: current,
                found: expected.clone(),
            });
        }
    }

    fn new_hir_id(&mut self) -> HirId {
        let id = HirId(self.next_hir_id);
        self.next_hir_id += 1;
        id
    }

    fn new_symbol_id(&mut self) -> SymbolId {
        let id = SymbolId(self.next_symbol_id);
        self.next_symbol_id += 1;
        id
    }

    fn make_expr(&mut self, span: Span, ty: Type, kind: HirExprKind) -> HirExpr {
        HirExpr {
            id: self.new_hir_id(),
            span,
            ty,
            kind,
        }
    }

    fn register_builtin_constants(&mut self) {
        for (name, ty) in builtin_constants() {
            self.define_symbol(name.to_string(), ty, SymbolKind::BuiltinConstant);
        }
    }

    fn register_builtin_functions(&mut self) {
        for builtin in builtin_functions() {
            self.functions.insert(
                builtin.name.to_string(),
                FunctionType {
                    params: builtin.params,
                    return_type: builtin.return_type,
                },
            );
        }
    }

    fn register_function_signature(&mut self, func: &FunctionDecl) {
        let existing = self.functions.get(&func.name).cloned();
        let params = func
            .params
            .iter()
            .enumerate()
            .map(|(idx, param)| {
                if let Some(ty_ref) = &param.ty {
                    let ty = Type::from_type_ref(ty_ref);
                    self.validate_user_type(&ty);
                    ty
                } else {
                    existing
                        .as_ref()
                        .and_then(|signature| signature.params.get(idx))
                        .cloned()
                        .unwrap_or(Type::Unknown)
                }
            })
            .collect();

        let return_type = if let Some(ret_ref) = &func.return_type {
            let ty = Type::from_type_ref(ret_ref);
            self.validate_user_type(&ty);
            ty
        } else {
            existing
                .map(|signature| signature.return_type)
                .unwrap_or(Type::Unknown)
        };

        self.functions.insert(
            func.name.clone(),
            FunctionType {
                params,
                return_type,
            },
        );
    }

    fn method_signature_key(owner_type: &str, method_name: &str) -> String {
        format!("{owner_type}::{method_name}")
    }

    fn type_signature_key(type_name: &str) -> String {
        type_name.to_string()
    }

    fn register_type_signatures(&mut self, td: &TypeDecl) {
        let key = Self::type_signature_key(&td.name);
        let existing = self.type_signatures.get(&key).cloned().unwrap_or_default();
        let params = td
            .params
            .iter()
            .enumerate()
            .map(|(idx, param)| {
                if let Some(ty_ref) = &param.ty {
                    let ty = Type::from_type_ref(ty_ref);
                    self.validate_user_type(&ty);
                    ty
                } else {
                    existing.get(idx).cloned().unwrap_or(Type::Unknown)
                }
            })
            .collect();

        self.type_signatures.insert(key, params);
    }

    fn register_method_signatures(&mut self, td: &TypeDecl) {
        for member in &td.members {
            let TypeMember::Method(method) = member else {
                continue;
            };

            let key = Self::method_signature_key(&td.name, &method.name);
            let existing = self.method_signatures.get(&key).cloned();
            let params = method
                .params
                .iter()
                .enumerate()
                .map(|(idx, param)| {
                    if let Some(ty_ref) = &param.ty {
                        let ty = Type::from_type_ref(ty_ref);
                        self.validate_user_type(&ty);
                        ty
                    } else {
                        existing
                            .as_ref()
                            .and_then(|signature| signature.params.get(idx))
                            .cloned()
                            .unwrap_or(Type::Unknown)
                    }
                })
                .collect();

            let return_type = if let Some(ret_ref) = &method.return_type {
                let ty = Type::from_type_ref(ret_ref);
                self.validate_user_type(&ty);
                ty
            } else {
                existing
                    .map(|signature| signature.return_type)
                    .unwrap_or(Type::Unknown)
            };

            self.method_signatures.insert(
                key,
                FunctionType {
                    params,
                    return_type,
                },
            );
        }
    }

    fn refine_type_param_type(&mut self, type_name: &str, idx: usize, expected: &Type) {
        if *expected == Type::Unknown {
            return;
        }

        let key = Self::type_signature_key(type_name);
        let Some(signature) = self.type_signatures.get_mut(&key) else {
            return;
        };
        let Some(current) = signature.get(idx).cloned() else {
            return;
        };

        if current == Type::Unknown {
            signature[idx] = expected.clone();
            self.signature_changed = true;
            return;
        }

        if current != *expected {
            self.errors.push(SemanticError::TypeMismatch {
                expected: current,
                found: expected.clone(),
            });
        }
    }

    fn refine_method_param_type(
        &mut self,
        owner_type: &str,
        method_name: &str,
        idx: usize,
        expected: &Type,
    ) {
        if *expected == Type::Unknown {
            return;
        }

        let key = Self::method_signature_key(owner_type, method_name);
        let Some(signature) = self.method_signatures.get_mut(&key) else {
            return;
        };
        let Some(current) = signature.params.get(idx).cloned() else {
            return;
        };

        if current == Type::Unknown {
            signature.params[idx] = expected.clone();
            self.signature_changed = true;
            return;
        }

        if current != *expected {
            self.errors.push(SemanticError::TypeMismatch {
                expected: current,
                found: expected.clone(),
            });
        }
    }

    fn analyze_function_decl(&mut self, func: &FunctionDecl) -> HirFunctionDecl {
        let signature = self
            .functions
            .get(&func.name)
            .cloned()
            .unwrap_or(FunctionType {
                params: Vec::new(),
                return_type: Type::Unknown,
            });

        self.current_function = Some(func.name.clone());
        self.current_function_param_indices.clear();
        self.push_scope();
        let mut params = Vec::new();
        for (idx, param) in func.params.iter().enumerate() {
            let ty = signature.params.get(idx).cloned().unwrap_or(Type::Unknown);
            let symbol = self.define_symbol(param.name.clone(), ty.clone(), SymbolKind::Parameter);
            self.current_function_param_indices.insert(symbol.id, idx);
            params.push(HirParam {
                name: param.name.clone(),
                ty,
                symbol: symbol.id,
                span: Span::default(),
            });
        }

        let body = self.analyze_expr(&func.body);
        for param in &mut params {
            if let Some(symbol) = self.resolve_symbol(&param.name) {
                param.ty = symbol.ty.clone();
            }
            if param.ty == Type::Unknown && self.report_unresolved_parameter_errors {
                self.errors.push(SemanticError::CannotInferParameterType {
                    function: func.name.clone(),
                    parameter: param.name.clone(),
                });
            }
        }
        self.pop_scope();
        self.current_function = None;
        self.current_function_param_indices.clear();

        let return_type = if let Some(ret_ref) = &func.return_type {
            let declared = Type::from_type_ref(ret_ref);
            if !self.is_assignable(&body.ty, &declared) {
                self.errors.push(SemanticError::InvalidReturnType {
                    function: func.name.clone(),
                    expected: declared.clone(),
                    found: body.ty.clone(),
                });
            }
            declared
        } else {
            if body.ty != Type::Unknown {
                let previous = self
                    .functions
                    .get(&func.name)
                    .map(|sig| sig.return_type.clone())
                    .unwrap_or(Type::Unknown);
                if previous != body.ty {
                    self.signature_changed = true;
                }
                if let Some(sig) = self.functions.get_mut(&func.name) {
                    sig.return_type = body.ty.clone();
                }
            }
            self.functions
                .get(&func.name)
                .map(|sig| sig.return_type.clone())
                .unwrap_or_else(|| body.ty.clone())
        };

        HirFunctionDecl {
            name: func.name.clone(),
            params,
            return_type,
            body,
            span: func.name_span,
        }
    }

    fn analyze_type_decl(&mut self, td: &TypeDecl) -> HirTypeDecl {
        self.current_type = Some(td.name.clone());
        self.current_type_parent = td.parent.as_ref().map(|parent| parent.name.clone());

        self.push_scope();
        let constructor_signature =
            self.type_signatures
                .get(&td.name)
                .cloned()
                .unwrap_or_else(|| {
                    self.registry
                        .get_type(&td.name)
                        .map(|info| {
                            info.constructor_params
                                .iter()
                                .map(|(_, ty)| ty.clone())
                                .collect()
                        })
                        .unwrap_or_default()
                });
        let mut params = Vec::new();
        for param in &td.params {
            let ty = if let Some(ty_ref) = &param.ty {
                let ty = Type::from_type_ref(ty_ref);
                self.validate_user_type(&ty);
                ty
            } else {
                constructor_signature
                    .get(params.len())
                    .cloned()
                    .unwrap_or(Type::Unknown)
            };
            let symbol = self.define_symbol(param.name.clone(), ty.clone(), SymbolKind::Parameter);
            params.push(HirParam {
                name: param.name.clone(),
                ty,
                symbol: symbol.id,
                span: Span::default(),
            });
        }

        let parent = td
            .parent
            .as_ref()
            .map(|parent| self.analyze_type_parent(parent));

        let mut attributes = Vec::new();
        let mut methods = Vec::new();
        for member in &td.members {
            match member {
                TypeMember::Attribute(attr) => attributes.push(self.analyze_attribute_decl(attr)),
                TypeMember::Method(method) => {
                    methods.push(self.analyze_method_decl(&td.name, method))
                }
            }
        }

        for param in &mut params {
            if let Some(symbol) = self.resolve_symbol(&param.name) {
                param.ty = symbol.ty.clone();
            }
            if param.ty == Type::Unknown && self.report_unresolved_parameter_errors {
                self.errors.push(SemanticError::CannotInferParameterType {
                    function: td.name.clone(),
                    parameter: param.name.clone(),
                });
            }
        }

        let updated_constructor_signature: Vec<Type> =
            params.iter().map(|param| param.ty.clone()).collect();
        if self
            .type_signatures
            .get(&td.name)
            .map(|signature| signature != &updated_constructor_signature)
            .unwrap_or(true)
        {
            self.signature_changed = true;
        }
        self.type_signatures
            .insert(td.name.clone(), updated_constructor_signature);

        self.pop_scope();
        self.current_type = None;
        self.current_type_parent = None;

        HirTypeDecl {
            name: td.name.clone(),
            params,
            parent,
            attributes,
            methods,
            span: td.name_span,
        }
    }

    fn analyze_type_parent(&mut self, parent: &TypeParent) -> HirParent {
        let args = parent.args.as_ref().map(|args| {
            let ctor_params = self
                .registry
                .get_type(&parent.name)
                .map(|info| info.constructor_params.clone())
                .unwrap_or_default();

            if ctor_params.len() != args.len() {
                self.errors.push(SemanticError::ArityMismatch {
                    function: parent.name.clone(),
                    expected: ctor_params.len(),
                    found: args.len(),
                });
            }

            args.iter()
                .enumerate()
                .map(|(idx, arg)| {
                    let hir_arg = self.analyze_expr(arg);
                    if let Some((_, expected)) = ctor_params.get(idx) {
                        if !self.is_assignable(&hir_arg.ty, expected) {
                            self.errors.push(SemanticError::InvalidArgumentType {
                                function: parent.name.clone(),
                                index: idx,
                                expected: expected.clone(),
                                found: hir_arg.ty.clone(),
                            });
                        }
                    }
                    hir_arg
                })
                .collect()
        });

        HirParent {
            name: parent.name.clone(),
            args,
        }
    }

    fn analyze_attribute_decl(&mut self, attr: &AttributeDecl) -> HirAttributeDecl {
        let value = self.analyze_expr(&attr.value);
        let ty = if let Some(ty_ref) = &attr.ty {
            let declared = Type::from_type_ref(ty_ref);
            self.validate_user_type(&declared);
            if !self.is_assignable(&value.ty, &declared) {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: declared.clone(),
                    found: value.ty.clone(),
                });
            }
            declared
        } else {
            value.ty.clone()
        };

        // Propagate inferred type back so that methods see the correct attribute type.
        if let Some(owner) = self.current_type.clone() {
            self.registry.update_attribute_type(&owner, &attr.name, ty.clone());
        }

        HirAttributeDecl {
            name: attr.name.clone(),
            ty,
            value,
            span: Span::default(),
        }
    }

    fn analyze_method_decl(&mut self, owner_type: &str, method: &MethodDecl) -> HirMethodDecl {
        self.current_method = Some(method.name.clone());
        self.push_scope();

        let key = Self::method_signature_key(owner_type, &method.name);
        let signature = self
            .method_signatures
            .get(&key)
            .cloned()
            .unwrap_or(FunctionType {
                params: Vec::new(),
                return_type: Type::Unknown,
            });

        self.define_symbol(
            "self".to_string(),
            Type::UserType(owner_type.to_string()),
            SymbolKind::SelfValue,
        );

        let mut params = Vec::new();
        for param in &method.params {
            let ty = if let Some(ty_ref) = &param.ty {
                let ty = Type::from_type_ref(ty_ref);
                self.validate_user_type(&ty);
                ty
            } else {
                signature
                    .params
                    .get(params.len())
                    .cloned()
                    .unwrap_or(Type::Unknown)
            };
            let symbol = self.define_symbol(param.name.clone(), ty.clone(), SymbolKind::Parameter);
            params.push(HirParam {
                name: param.name.clone(),
                ty,
                symbol: symbol.id,
                span: Span::default(),
            });
        }

        let body = self.analyze_expr(&method.body);
        for param in &mut params {
            if let Some(symbol) = self.resolve_symbol(&param.name) {
                param.ty = symbol.ty.clone();
            }
            if param.ty == Type::Unknown && self.report_unresolved_parameter_errors {
                self.errors.push(SemanticError::CannotInferParameterType {
                    function: method.name.clone(),
                    parameter: param.name.clone(),
                });
            }
        }
        self.pop_scope();
        self.current_method = None;

        let return_type = if let Some(ret_ref) = &method.return_type {
            let declared = Type::from_type_ref(ret_ref);
            self.validate_user_type(&declared);
            if !self.is_assignable(&body.ty, &declared) {
                self.errors.push(SemanticError::InvalidReturnType {
                    function: method.name.clone(),
                    expected: declared.clone(),
                    found: body.ty.clone(),
                });
            }
            declared
        } else {
            if body.ty != Type::Unknown {
                body.ty.clone()
            } else {
                signature.return_type.clone()
            }
        };

        // Propagate inferred return type back to the registry so that any later
        // method calls within the same type see the correct return type instead
        // of the initial Unknown placeholder set before bodies were analysed.
        self.registry.update_method_return_type(
            owner_type,
            &method.name,
            return_type.clone(),
        );

        let updated_signature = FunctionType {
            params: params.iter().map(|param| param.ty.clone()).collect(),
            return_type: return_type.clone(),
        };
        if self
            .method_signatures
            .get(&key)
            .map(|signature| {
                signature.params != updated_signature.params
                    || signature.return_type != updated_signature.return_type
            })
            .unwrap_or(true)
        {
            self.signature_changed = true;
        }
        self.method_signatures.insert(key, updated_signature);

        HirMethodDecl {
            owner_type: owner_type.to_string(),
            name: method.name.clone(),
            params,
            return_type,
            body,
            span: Span::default(),
        }
    }

    fn analyze_protocol_decl(&mut self, pd: &ProtocolDecl) -> HirProtocolDecl {
        let methods = pd
            .methods
            .iter()
            .map(|method| {
                let params = method
                    .params
                    .iter()
                    .map(|param| {
                        let ty = self.parameter_type(param, &method.name);
                        HirParam {
                            name: param.name.clone(),
                            ty,
                            symbol: self.new_symbol_id(),
                            span: Span::default(),
                        }
                    })
                    .collect();
                let return_type = method
                    .return_type
                    .as_ref()
                    .map(Type::from_type_ref)
                    .unwrap_or(Type::Unknown);
                self.validate_user_type(&return_type);
                HirProtocolMethod {
                    name: method.name.clone(),
                    params,
                    return_type,
                    span: Span::default(),
                }
            })
            .collect();

        HirProtocolDecl {
            name: pd.name.clone(),
            methods,
            parent: pd.parent.clone(),
            span: pd.name_span,
        }
    }

    fn parameter_type(&mut self, param: &Param, owner: &str) -> Type {
        if let Some(ty_ref) = &param.ty {
            let ty = Type::from_type_ref(ty_ref);
            self.validate_user_type(&ty);
            ty
        } else if self.current_function.as_deref() == Some(owner)
            || self.current_method.as_deref() == Some(owner)
            || self.current_type.as_deref() == Some(owner)
        {
            Type::Unknown
        } else {
            self.errors.push(SemanticError::CannotInferParameterType {
                function: owner.to_string(),
                parameter: param.name.clone(),
            });
            Type::Unknown
        }
    }

    fn validate_user_type(&mut self, ty: &Type) {
        match ty {
            Type::UserType(name) => {
                if let Err(error) = self.registry.validate_user_type(name) {
                    self.errors.push(error);
                }
            }
            Type::Vector(inner) | Type::Iterable(inner) => self.validate_user_type(inner),
            Type::Functor { params, ret } => {
                for param in params {
                    self.validate_user_type(param);
                }
                self.validate_user_type(ret);
            }
            Type::Number | Type::String | Type::Boolean | Type::Object | Type::Unknown => {}
        }
    }

    fn typeref_to_type(&self, ty_ref: &TypeRef) -> Type {
        Type::from_type_ref(ty_ref)
    }

    fn analyze_expr_with_span(&mut self, _span: Span, expr: &Expr) -> HirExpr {
        self.analyze_expr(expr)
    }

    fn analyze_expr(&mut self, expr: &Expr) -> HirExpr {
        match expr {
            Expr::Number(value) => {
                self.make_expr(Span::default(), Type::Number, HirExprKind::Number(*value))
            }
            Expr::String(value) => self.make_expr(
                Span::default(),
                Type::String,
                HirExprKind::String(value.clone()),
            ),
            Expr::Bool(value) => {
                self.make_expr(Span::default(), Type::Boolean, HirExprKind::Bool(*value))
            }
            Expr::Var(name, span) => self.analyze_var(name, *span),
            Expr::Unary { span, op, expr } => self.analyze_unary(*span, op, expr),
            Expr::Binary {
                span,
                left,
                op,
                right,
            } => self.analyze_binary(*span, left, op, right),
            Expr::Let {
                span,
                bindings,
                body,
            } => self.analyze_let(*span, bindings, body),
            Expr::Block(exprs) => self.analyze_block(exprs),
            Expr::Call { span, callee, args } => self.analyze_call(*span, callee, args),
            Expr::Assign {
                span,
                target,
                value,
            } => self.analyze_assign(*span, target, value),
            Expr::If {
                span,
                branches,
                else_branch,
            } => self.analyze_if(*span, branches, else_branch),
            Expr::While {
                span,
                condition,
                body,
            } => self.analyze_while(*span, condition, body),
            Expr::For {
                span,
                var,
                iterable,
                body,
            } => self.analyze_for(*span, var, iterable, body),
            Expr::New {
                span,
                type_name,
                args,
            } => self.analyze_new(*span, type_name, args),
            Expr::MemberAccess {
                span,
                object,
                member,
            } => self.analyze_member_access(*span, object, member),
            Expr::MethodCall {
                span,
                object,
                method,
                args,
            } => self.analyze_method_call(*span, object, method, args),
            Expr::SelfRef => self.analyze_self_ref(),
            Expr::BaseCall { span, args } => self.analyze_base_call(*span, args),
            Expr::TypeTest {
                span,
                expr,
                type_name,
            } => self.analyze_type_test(*span, expr, type_name),
            Expr::TypeCast {
                span,
                expr,
                type_name,
            } => self.analyze_type_cast(*span, expr, type_name),
            Expr::VectorLiteral(elements) => self.analyze_vector_literal(elements),
            Expr::NewVector {
                span,
                elem_type,
                size,
                init,
            } => self.analyze_new_vector(*span, elem_type, size, init.as_ref()),
            Expr::VectorGenerator {
                span,
                body,
                var,
                iterable,
            } => self.analyze_vector_generator(*span, body, var, iterable),
            Expr::VectorIndex {
                span,
                vector,
                index,
            } => self.analyze_vector_index(*span, vector, index),
            Expr::Lambda {
                span,
                params,
                return_type,
                body,
            } => self.analyze_lambda(*span, params, return_type.as_ref(), body),
            Expr::Match {
                span,
                scrutinee,
                arms,
            } => self.analyze_match(*span, scrutinee, arms),
        }
    }

    fn analyze_var(&mut self, name: &str, span: Span) -> HirExpr {
        match self.resolve_symbol(name) {
            Some(symbol) if matches!(symbol.kind, SymbolKind::BuiltinConstant) => {
                if let Some(value) = builtin_constant_value(&symbol.name) {
                    self.make_expr(span, symbol.ty.clone(), HirExprKind::Number(value))
                } else {
                    self.make_expr(
                        span,
                        symbol.ty.clone(),
                        HirExprKind::Var {
                            name: symbol.name.clone(),
                            symbol: symbol.id,
                        },
                    )
                }
            }
            Some(symbol) => self.make_expr(
                span,
                symbol.ty.clone(),
                HirExprKind::Var {
                    name: symbol.name.clone(),
                    symbol: symbol.id,
                },
            ),
            None => {
                self.errors.push(SemanticError::UndefinedVariable {
                    name: name.to_string(),
                    span,
                });
                let symbol = self.new_symbol_id();
                self.make_expr(
                    span,
                    Type::Unknown,
                    HirExprKind::Var {
                        name: name.to_string(),
                        symbol,
                    },
                )
            }
        }
    }

    fn analyze_unary(&mut self, span: Span, op: &UnaryOp, expr: &Expr) -> HirExpr {
        let hir_expr = self.analyze_expr(expr);
        let found = hir_expr.ty.clone();
        let ty = if found == Type::Unknown {
            match op {
                UnaryOp::Not => {
                    self.constrain_expr_type(expr, &Type::Boolean);
                    Type::Boolean
                }
                UnaryOp::Neg | UnaryOp::Pos => {
                    self.constrain_expr_type(expr, &Type::Number);
                    Type::Number
                }
            }
        } else {
            match op {
                UnaryOp::Not => {
                    if found == Type::Boolean {
                        Type::Boolean
                    } else {
                        self.errors.push(SemanticError::InvalidUnaryOperand {
                            op: op.clone(),
                            found,
                        });
                        Type::Unknown
                    }
                }
                UnaryOp::Neg | UnaryOp::Pos => {
                    if found == Type::Number {
                        Type::Number
                    } else {
                        self.errors.push(SemanticError::InvalidUnaryOperand {
                            op: op.clone(),
                            found,
                        });
                        Type::Unknown
                    }
                }
            }
        };

        self.make_expr(
            span,
            ty,
            HirExprKind::Unary {
                op: op.clone(),
                expr: Box::new(hir_expr),
            },
        )
    }

    fn analyze_binary(&mut self, span: Span, left: &Expr, op: &BinaryOp, right: &Expr) -> HirExpr {
        let left_hir = self.analyze_expr(left);
        let right_hir = self.analyze_expr(right);
        let left_ty = left_hir.ty.clone();
        let right_ty = right_hir.ty.clone();
        let ty = match op {
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::Pow => {
                let left_ok = self.constrain_numeric_operand(left, &left_ty);
                let right_ok = self.constrain_numeric_operand(right, &right_ty);
                if left_ok && right_ok {
                    Type::Number
                } else if left_ty == Type::Unknown || right_ty == Type::Unknown {
                    Type::Unknown
                } else {
                    self.errors.push(SemanticError::InvalidBinaryOperands {
                        op: op.clone(),
                        left: left_ty,
                        right: right_ty,
                    });
                    Type::Unknown
                }
            }
            BinaryOp::Concat | BinaryOp::ConcatSpace => {
                let left_ok = self.constrain_concat_operand(left, &left_ty);
                let right_ok = self.constrain_concat_operand(right, &right_ty);
                if left_ok && right_ok {
                    Type::String
                } else if left_ty == Type::Unknown || right_ty == Type::Unknown {
                    Type::Unknown
                } else {
                    self.errors.push(SemanticError::InvalidBinaryOperands {
                        op: op.clone(),
                        left: left_ty,
                        right: right_ty,
                    });
                    Type::Unknown
                }
            }
            BinaryOp::Eq | BinaryOp::Neq => {
                let compatible = left_ty == right_ty
                    || self.is_assignable(&left_ty, &right_ty)
                    || self.is_assignable(&right_ty, &left_ty);
                if !compatible && left_ty != Type::Unknown && right_ty != Type::Unknown {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: left_ty,
                        found: right_ty,
                    });
                }
                Type::Boolean
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                let left_ok = self.constrain_numeric_operand(left, &left_ty);
                let right_ok = self.constrain_numeric_operand(right, &right_ty);
                if (!left_ok || !right_ok) && left_ty != Type::Unknown && right_ty != Type::Unknown
                {
                    self.errors.push(SemanticError::InvalidBinaryOperands {
                        op: op.clone(),
                        left: left_ty,
                        right: right_ty,
                    });
                }
                Type::Boolean
            }
            BinaryOp::And | BinaryOp::Or => {
                let left_ok = self.constrain_boolean_operand(left, &left_ty);
                let right_ok = self.constrain_boolean_operand(right, &right_ty);
                if (!left_ok || !right_ok) && left_ty != Type::Unknown && right_ty != Type::Unknown
                {
                    self.errors.push(SemanticError::InvalidBinaryOperands {
                        op: op.clone(),
                        left: left_ty,
                        right: right_ty,
                    });
                }
                Type::Boolean
            }
        };

        self.make_expr(
            span,
            ty,
            HirExprKind::Binary {
                op: op.clone(),
                left: Box::new(left_hir),
                right: Box::new(right_hir),
            },
        )
    }

    fn analyze_assign(&mut self, span: Span, target: &Expr, value: &Expr) -> HirExpr {
        let value_hir = self.analyze_expr(value);

        let target = match target {
            Expr::Var(name, _) => match self.resolve_symbol(name) {
                Some(symbol) => {
                    if symbol.ty == Type::Unknown && value_hir.ty != Type::Unknown {
                        self.constrain_expr_type(target, &value_hir.ty);
                    }
                    if !self.is_assignable(&value_hir.ty, &symbol.ty) {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: symbol.ty.clone(),
                            found: value_hir.ty.clone(),
                        });
                    }
                    HirAssignTarget::Local {
                        name: symbol.name,
                        symbol: symbol.id,
                        ty: symbol.ty,
                    }
                }
                None => {
                    self.errors
                        .push(SemanticError::UndefinedVariable { name: name.clone(), span });
                    HirAssignTarget::Local {
                        name: name.clone(),
                        symbol: self.new_symbol_id(),
                        ty: Type::Unknown,
                    }
                }
            },
            Expr::MemberAccess { object, member, .. }
                if matches!(object.as_ref(), Expr::SelfRef) =>
            {
                let Some(owner_type) = self.current_type.clone() else {
                    self.errors.push(SemanticError::InvalidAssignmentTarget);
                    return self.make_expr(
                        span,
                        Type::Unknown,
                        HirExprKind::Block { exprs: Vec::new() },
                    );
                };

                let Some(attr_ty) = self.registry.lookup_attribute(&owner_type, member) else {
                    self.errors.push(SemanticError::AttributeIsPrivate {
                        type_name: owner_type.clone(),
                        attr_name: member.clone(),
                    });
                    return self.make_expr(
                        span,
                        Type::Unknown,
                        HirExprKind::Block { exprs: Vec::new() },
                    );
                };

                if !self.is_assignable(&value_hir.ty, &attr_ty) {
                    self.errors.push(SemanticError::TypeMismatch {
                        expected: attr_ty.clone(),
                        found: value_hir.ty.clone(),
                    });
                }

                HirAssignTarget::SelfAttribute {
                    owner_type,
                    attr_name: member.clone(),
                    ty: attr_ty,
                }
            }
            Expr::VectorIndex { vector, index, .. } => {
                let vec_hir = self.analyze_expr(vector);
                let elem_ty = match &vec_hir.ty {
                    Type::Vector(inner) => *inner.clone(),
                    _ => Type::Number,
                };
                let idx_hir = self.analyze_expr(index);
                HirAssignTarget::VectorIndex {
                    vector: Box::new(vec_hir),
                    index: Box::new(idx_hir),
                    elem_ty,
                }
            }
            _ => {
                self.errors.push(SemanticError::InvalidAssignmentTarget);
                return self.make_expr(
                    span,
                    Type::Unknown,
                    HirExprKind::Block { exprs: Vec::new() },
                );
            }
        };
        let ty = value_hir.ty.clone();

        self.make_expr(
            span,
            ty,
            HirExprKind::Assign {
                target,
                value: Box::new(value_hir),
            },
        )
    }

    fn analyze_if(&mut self, span: Span, branches: &[(Expr, Expr)], else_branch: &Expr) -> HirExpr {
        let mut hir_branches = Vec::new();
        let mut unified = Type::Unknown;

        for (condition, body) in branches {
            let condition_hir = self.analyze_expr(condition);
            if condition_hir.ty != Type::Boolean && condition_hir.ty != Type::Unknown {
                self.errors.push(SemanticError::InvalidConditionType {
                    found: condition_hir.ty.clone(),
                });
            }

            let body_hir = self.analyze_expr(body);
            if unified != Type::Unknown && !self.are_branch_types_compatible(&unified, &body_hir.ty)
            {
                self.errors.push(SemanticError::TypeMismatch {
                    expected: unified.clone(),
                    found: body_hir.ty.clone(),
                });
            }
            unified = self.unify_types(&unified, &body_hir.ty);
            hir_branches.push((condition_hir, body_hir));
        }

        let else_hir = self.analyze_expr(else_branch);
        if unified != Type::Unknown && !self.are_branch_types_compatible(&unified, &else_hir.ty) {
            self.errors.push(SemanticError::TypeMismatch {
                expected: unified.clone(),
                found: else_hir.ty.clone(),
            });
        }
        let ty = self.unify_types(&unified, &else_hir.ty);

        self.make_expr(
            span,
            ty,
            HirExprKind::If {
                branches: hir_branches,
                else_branch: Box::new(else_hir),
            },
        )
    }

    fn analyze_while(&mut self, span: Span, condition: &Expr, body: &Expr) -> HirExpr {
        let condition_hir = self.analyze_expr(condition);
        if condition_hir.ty != Type::Boolean && condition_hir.ty != Type::Unknown {
            self.errors.push(SemanticError::InvalidConditionType {
                found: condition_hir.ty.clone(),
            });
        }

        let body_hir = self.analyze_expr(body);
        let ty = body_hir.ty.clone();
        self.make_expr(
            span,
            ty,
            HirExprKind::While {
                condition: Box::new(condition_hir),
                body: Box::new(body_hir),
            },
        )
    }

    fn analyze_for(&mut self, span: Span, var: &str, iterable: &Expr, body: &Expr) -> HirExpr {
        let iterable_hir = self.analyze_expr(iterable);
        let iterable_ty = iterable_hir.ty.clone();
        let elem_ty = match &iterable_hir.ty {
            Type::Iterable(inner) | Type::Vector(inner) => *inner.clone(),
            Type::Unknown => Type::Unknown,
            Type::UserType(_) => {
                // User-defined generator: must have current() method
                if let Some((_, sig)) =
                    self.method_signature_for_call(&iterable_ty, "current")
                {
                    sig.return_type
                } else {
                    self.errors.push(SemanticError::InvalidIterableTarget {
                        found: iterable_hir.ty.clone(),
                    });
                    Type::Unknown
                }
            }
            other => {
                self.errors.push(SemanticError::InvalidIterableTarget {
                    found: other.clone(),
                });
                Type::Unknown
            }
        };

        // Normalize `for` before lowering: `let _iter$N = iterable in while (_iter$N.next()) ...`.
        // The `$` suffix keeps the generated name outside the user identifier space.
        self.push_scope();
        let iter_name = format!("_iter${}", self.next_symbol_id);
        let iter_symbol =
            self.define_symbol(iter_name.clone(), iterable_ty.clone(), SymbolKind::Local);
        let iter_binding = HirLetBinding {
            name: iter_name.clone(),
            symbol: iter_symbol.id,
            ty: iterable_ty.clone(),
            value: iterable_hir,
            span,
        };

        let next_signature = self
            .method_signature_for_call(&iterable_ty, "next")
            .map(|(_, signature)| signature)
            .unwrap_or(FunctionType {
                params: vec![],
                return_type: Type::Boolean,
            });
        let next_object = self.make_expr(
            span,
            iterable_ty.clone(),
            HirExprKind::Var {
                name: iter_name.clone(),
                symbol: iter_symbol.id,
            },
        );
        let condition = self.make_expr(
            span,
            Type::Boolean,
            HirExprKind::MethodCall {
                object: Box::new(next_object),
                method: "next".to_string(),
                args: Vec::new(),
                dispatch: DispatchKind::Virtual {
                    receiver_static_type: iterable_ty.clone(),
                    method_name: "next".to_string(),
                    signature: next_signature,
                },
            },
        );

        let current_signature = self
            .method_signature_for_call(&iterable_ty, "current")
            .map(|(_, signature)| signature)
            .unwrap_or(FunctionType {
                params: vec![],
                return_type: elem_ty.clone(),
            });
        let current_object = self.make_expr(
            span,
            iterable_ty.clone(),
            HirExprKind::Var {
                name: iter_name,
                symbol: iter_symbol.id,
            },
        );
        let current_value = self.make_expr(
            span,
            elem_ty.clone(),
            HirExprKind::MethodCall {
                object: Box::new(current_object),
                method: "current".to_string(),
                args: Vec::new(),
                dispatch: DispatchKind::Virtual {
                    receiver_static_type: iterable_ty.clone(),
                    method_name: "current".to_string(),
                    signature: current_signature,
                },
            },
        );

        self.push_scope();
        let loop_symbol = self.define_symbol(var.to_string(), elem_ty.clone(), SymbolKind::Local);
        let loop_binding = HirLetBinding {
            name: var.to_string(),
            symbol: loop_symbol.id,
            ty: elem_ty,
            value: current_value,
            span,
        };
        let body_hir = self.analyze_expr(body);
        let ty = body_hir.ty.clone();
        self.pop_scope();

        let loop_body = self.make_expr(
            span,
            ty.clone(),
            HirExprKind::Let {
                bindings: vec![loop_binding],
                body: Box::new(body_hir),
            },
        );
        let while_expr = self.make_expr(
            span,
            ty.clone(),
            HirExprKind::While {
                condition: Box::new(condition),
                body: Box::new(loop_body),
            },
        );
        self.pop_scope();

        self.make_expr(
            span,
            ty,
            HirExprKind::Let {
                bindings: vec![iter_binding],
                body: Box::new(while_expr),
            },
        )
    }

    fn analyze_new(&mut self, span: Span, type_name: &str, args: &[Expr]) -> HirExpr {
        let ctor_params = match self.registry.get_type(type_name) {
            Some(info) => self
                .type_signatures
                .get(type_name)
                .cloned()
                .unwrap_or_else(|| {
                    info.constructor_params
                        .iter()
                        .map(|(_, ty)| ty.clone())
                        .collect()
                }),
            None => {
                self.errors.push(SemanticError::UndefinedType {
                    name: type_name.to_string(),
                });
                let hir_args = args.iter().map(|arg| self.analyze_expr(arg)).collect();
                return self.make_expr(
                    span,
                    Type::Unknown,
                    HirExprKind::New {
                        type_name: type_name.to_string(),
                        args: hir_args,
                    },
                );
            }
        };

        if ctor_params.len() != args.len() {
            self.errors.push(SemanticError::ArityMismatch {
                function: type_name.to_string(),
                expected: ctor_params.len(),
                found: args.len(),
            });
        }

        let hir_args = args
            .iter()
            .enumerate()
            .map(|(idx, arg)| {
                let hir_arg = self.analyze_expr(arg);
                if let Some(expected) = ctor_params.get(idx) {
                    if *expected == Type::Unknown && hir_arg.ty != Type::Unknown {
                        self.refine_type_param_type(type_name, idx, &hir_arg.ty);
                    }
                    if !self.is_assignable(&hir_arg.ty, expected) {
                        self.errors.push(SemanticError::InvalidArgumentType {
                            function: type_name.to_string(),
                            index: idx,
                            expected: expected.clone(),
                            found: hir_arg.ty.clone(),
                        });
                    }
                }
                hir_arg
            })
            .collect();

        self.make_expr(
            span,
            Type::UserType(type_name.to_string()),
            HirExprKind::New {
                type_name: type_name.to_string(),
                args: hir_args,
            },
        )
    }

    fn analyze_member_access(&mut self, span: Span, object: &Expr, member: &str) -> HirExpr {
        let is_self = matches!(object, Expr::SelfRef);
        let object_hir = self.analyze_expr(object);

        if object_hir.ty == Type::Unknown {
            return self.make_expr(
                span,
                Type::Unknown,
                HirExprKind::MemberAccess {
                    object: Box::new(object_hir),
                    member: member.to_string(),
                    resolved: ResolvedMember::Attribute {
                        owner_type: String::new(),
                        attr_name: member.to_string(),
                        ty: Type::Unknown,
                    },
                },
            );
        }

        if is_self {
            let Type::UserType(owner_type) = object_hir.ty.clone() else {
                return self.make_expr(
                    span,
                    Type::Unknown,
                    HirExprKind::MemberAccess {
                        object: Box::new(object_hir),
                        member: member.to_string(),
                        resolved: ResolvedMember::Attribute {
                            owner_type: String::new(),
                            attr_name: member.to_string(),
                            ty: Type::Unknown,
                        },
                    },
                );
            };

            if let Some(attr_ty) = self.registry.lookup_attribute(&owner_type, member) {
                return self.make_expr(
                    span,
                    attr_ty.clone(),
                    HirExprKind::MemberAccess {
                        object: Box::new(object_hir),
                        member: member.to_string(),
                        resolved: ResolvedMember::Attribute {
                            owner_type,
                            attr_name: member.to_string(),
                            ty: attr_ty,
                        },
                    },
                );
            }

            self.errors.push(SemanticError::AttributeIsPrivate {
                type_name: owner_type.clone(),
                attr_name: member.to_string(),
            });
            return self.make_expr(
                span,
                Type::Unknown,
                HirExprKind::MemberAccess {
                    object: Box::new(object_hir),
                    member: member.to_string(),
                    resolved: ResolvedMember::Attribute {
                        owner_type,
                        attr_name: member.to_string(),
                        ty: Type::Unknown,
                    },
                },
            );
        }

        let type_name = if let Type::UserType(name) = &object_hir.ty {
            name.clone()
        } else {
            String::new()
        };

        // Allow access when the object's type equals the current class (same-class access).
        // e.g. `other.x` inside a method of the Vector class when `other: Vector`.
        let same_class = self
            .current_type
            .as_deref()
            .map(|ct| ct == type_name)
            .unwrap_or(false);
        if same_class {
            if let Some(attr_ty) = self.registry.lookup_attribute(&type_name, member) {
                return self.make_expr(
                    span,
                    attr_ty.clone(),
                    HirExprKind::MemberAccess {
                        object: Box::new(object_hir),
                        member: member.to_string(),
                        resolved: ResolvedMember::Attribute {
                            owner_type: type_name,
                            attr_name: member.to_string(),
                            ty: attr_ty,
                        },
                    },
                );
            }
        }

        self.errors.push(SemanticError::AttributeIsPrivate {
            type_name: type_name.clone(),
            attr_name: member.to_string(),
        });
        self.make_expr(
            span,
            Type::Unknown,
            HirExprKind::MemberAccess {
                object: Box::new(object_hir),
                member: member.to_string(),
                resolved: ResolvedMember::Attribute {
                    owner_type: type_name,
                    attr_name: member.to_string(),
                    ty: Type::Unknown,
                },
            },
        )
    }

    fn analyze_method_call(
        &mut self,
        span: Span,
        object: &Expr,
        method: &str,
        args: &[Expr],
    ) -> HirExpr {
        let object_hir = self.analyze_expr(object);

        let Some((owner_name, signature)) = self.method_signature_for_call(&object_hir.ty, method)
        else {
            self.errors.push(SemanticError::UndefinedMethod {
                type_name: method_receiver_type_name(&object_hir.ty),
                method_name: method.to_string(),
                span,
            });
            let hir_args = args.iter().map(|arg| self.analyze_expr(arg)).collect();
            let signature = FunctionType {
                params: Vec::new(),
                return_type: Type::Unknown,
            };
            return self.make_expr(
                span,
                Type::Unknown,
                HirExprKind::MethodCall {
                    object: Box::new(object_hir),
                    method: method.to_string(),
                    args: hir_args,
                    dispatch: DispatchKind::Virtual {
                        receiver_static_type: Type::Unknown,
                        method_name: method.to_string(),
                        signature,
                    },
                },
            );
        };

        let refine_target = self
            .registry
            .get_type(&owner_name)
            .map(|_| (owner_name.as_str(), method));
        let (hir_args, signature) = self.analyze_method_call_args(
            &format!("{owner_name}.{method}"),
            refine_target,
            args,
            &signature,
        );
        self.make_expr(
            span,
            signature.return_type.clone(),
            HirExprKind::MethodCall {
                object: Box::new(object_hir.clone()),
                method: method.to_string(),
                args: hir_args,
                dispatch: DispatchKind::Virtual {
                    receiver_static_type: object_hir.ty,
                    method_name: method.to_string(),
                    signature,
                },
            },
        )
    }

    fn analyze_self_ref(&mut self) -> HirExpr {
        let Some(type_name) = self.current_type.clone() else {
            self.errors.push(SemanticError::UnsupportedConstruct {
                message: "Cannot use 'self' outside of a type method".to_string(),
            });
            let symbol = self.new_symbol_id();
            return self.make_expr(
                Span::default(),
                Type::Unknown,
                HirExprKind::SelfRef {
                    symbol,
                    type_name: String::new(),
                },
            );
        };

        let Some(symbol) = self.resolve_symbol("self") else {
            self.errors.push(SemanticError::UnsupportedConstruct {
                message: "Cannot use 'self' outside of a type method".to_string(),
            });
            let symbol = self.new_symbol_id();
            return self.make_expr(
                Span::default(),
                Type::Unknown,
                HirExprKind::SelfRef { symbol, type_name },
            );
        };

        self.make_expr(
            Span::default(),
            symbol.ty,
            HirExprKind::SelfRef {
                symbol: symbol.id,
                type_name,
            },
        )
    }

    fn analyze_base_call(&mut self, span: Span, args: &[Expr]) -> HirExpr {
        let Some(method_name) = self.current_method.clone() else {
            self.errors.push(SemanticError::UnsupportedConstruct {
                message: "base() can only be called inside a method body".to_string(),
            });
            let hir_args = args.iter().map(|arg| self.analyze_expr(arg)).collect();
            return self.make_expr(
                span,
                Type::Unknown,
                HirExprKind::BaseCall {
                    parent_type: String::new(),
                    method_name: String::new(),
                    args: hir_args,
                },
            );
        };
        let Some(current_type) = self.current_type.clone() else {
            self.errors.push(SemanticError::UnsupportedConstruct {
                message: "base() can only be called inside a method body".to_string(),
            });
            let hir_args = args.iter().map(|arg| self.analyze_expr(arg)).collect();
            return self.make_expr(
                span,
                Type::Unknown,
                HirExprKind::BaseCall {
                    parent_type: String::new(),
                    method_name,
                    args: hir_args,
                },
            );
        };
        let Some(parent_type) = self.current_type_parent.clone() else {
            self.errors.push(SemanticError::UnsupportedConstruct {
                message: "Cannot use 'base' in a type without a parent".to_string(),
            });
            let hir_args = args.iter().map(|arg| self.analyze_expr(arg)).collect();
            return self.make_expr(
                span,
                Type::Unknown,
                HirExprKind::BaseCall {
                    parent_type: String::new(),
                    method_name,
                    args: hir_args,
                },
            );
        };

        let Some((base_method_name, info)) = self.registry.resolve_base_method_info(
            &current_type,
            &parent_type,
            &method_name,
            args.len(),
        ) else {
            self.errors.push(SemanticError::UnsupportedConstruct {
                message: format!("Parent type '{parent_type}' has no method '{method_name}'"),
            });
            let hir_args = args.iter().map(|arg| self.analyze_expr(arg)).collect();
            return self.make_expr(
                span,
                Type::Unknown,
                HirExprKind::BaseCall {
                    parent_type,
                    method_name,
                    args: hir_args,
                },
            );
        };

        let signature = FunctionType {
            params: info.params,
            return_type: info.return_type,
        };
        let (hir_args, signature) = self.analyze_method_call_args(
            &format!("base.{base_method_name}"),
            None,
            args,
            &signature,
        );
        self.make_expr(
            span,
            signature.return_type.clone(),
            HirExprKind::BaseCall {
                parent_type,
                method_name: base_method_name,
                args: hir_args,
            },
        )
    }

    fn analyze_let(
        &mut self,
        span: Span,
        bindings: &[hulk_frontend::ast::LetBinding],
        body: &Expr,
    ) -> HirExpr {
        self.push_scope();
        let mut hir_bindings = Vec::new();

        for binding in bindings {
            let value = self.analyze_expr(&binding.value);
            let ty = if let Some(ty_ref) = &binding.ty {
                let declared = Type::from_type_ref(ty_ref);
                self.validate_user_type(&declared);
                let is_proto_annotation = matches!(
                    &declared,
                    Type::UserType(name) if self.registry.get_protocol(name).is_some()
                );
                if is_proto_annotation {
                    if let (Type::UserType(proto_name), Type::UserType(concrete_name)) =
                        (&declared, &value.ty)
                    {
                        if !self
                            .registry
                            .implicitly_conforms_to_protocol(concrete_name, proto_name)
                        {
                            let method_names: Vec<String> = self
                                .registry
                                .get_protocol(proto_name)
                                .map(|protocol| protocol.methods.keys().cloned().collect())
                                .unwrap_or_default();
                            let mut missing = proto_name.clone();
                            for method in &method_names {
                                if self
                                    .registry
                                    .lookup_method_info(concrete_name, method)
                                    .is_none()
                                {
                                    missing = method.clone();
                                    break;
                                }
                            }
                            self.errors.push(SemanticError::MissingProtocolMethod {
                                type_name: concrete_name.clone(),
                                method_name: missing,
                            });
                        }
                    } else if !self.is_assignable(&value.ty, &declared) {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: declared.clone(),
                            found: value.ty.clone(),
                        });
                    }
                } else {
                    if !self.is_assignable(&value.ty, &declared) {
                        self.errors.push(SemanticError::TypeMismatch {
                            expected: declared.clone(),
                            found: value.ty.clone(),
                        });
                    }
                }
                declared
            } else {
                value.ty.clone()
            };
            let symbol = self.define_symbol(binding.name.clone(), ty.clone(), SymbolKind::Local);
            hir_bindings.push(HirLetBinding {
                name: binding.name.clone(),
                symbol: symbol.id,
                ty,
                value,
                span,
            });
        }

        let hir_body = self.analyze_expr(body);
        let ty = hir_body.ty.clone();
        self.pop_scope();

        self.make_expr(
            span,
            ty,
            HirExprKind::Let {
                bindings: hir_bindings,
                body: Box::new(hir_body),
            },
        )
    }

    fn analyze_block(&mut self, exprs: &[Expr]) -> HirExpr {
        self.push_scope();
        let mut hir_exprs = Vec::new();
        let mut ty = Type::Object;

        for expr in exprs {
            let hir_expr = self.analyze_expr(expr);
            ty = hir_expr.ty.clone();
            hir_exprs.push(hir_expr);
        }

        self.pop_scope();
        self.make_expr(Span::default(), ty, HirExprKind::Block { exprs: hir_exprs })
    }

    fn analyze_type_test(&mut self, span: Span, expr: &Expr, type_name: &str) -> HirExpr {
        let hir_expr = self.analyze_expr(expr);
        if let Err(error) = self.registry.validate_user_type(type_name) {
            self.errors.push(error);
        }

        self.make_expr(
            span,
            Type::Boolean,
            HirExprKind::TypeTest {
                expr: Box::new(hir_expr),
                type_name: type_name.to_string(),
            },
        )
    }

    fn analyze_type_cast(&mut self, span: Span, expr: &Expr, type_name: &str) -> HirExpr {
        let hir_expr = self.analyze_expr(expr);
        let ty = match self.registry.validate_user_type(type_name) {
            Ok(()) => Type::UserType(type_name.to_string()),
            Err(error) => {
                self.errors.push(error);
                Type::Unknown
            }
        };

        self.make_expr(
            span,
            ty,
            HirExprKind::TypeCast {
                expr: Box::new(hir_expr),
                type_name: type_name.to_string(),
            },
        )
    }

    fn analyze_vector_literal(&mut self, elements: &[Expr]) -> HirExpr {
        let mut hir_elements = Vec::new();
        let mut element_type: Option<Type> = None;

        for element in elements {
            let hir_element = self.analyze_expr(element);
            element_type = Some(match element_type {
                None => hir_element.ty.clone(),
                Some(prev) if prev == hir_element.ty => prev,
                Some(prev) => self.unify_types(&prev, &hir_element.ty),
            });
            hir_elements.push(hir_element);
        }

        let element_type = element_type.unwrap_or(Type::Object);
        self.make_expr(
            Span::default(),
            Type::Vector(Box::new(element_type.clone())),
            HirExprKind::VectorLiteral {
                elements: hir_elements,
                element_type,
            },
        )
    }

    fn analyze_new_vector(
        &mut self,
        span: Span,
        elem_type_ref: &TypeRef,
        size: &Expr,
        init: Option<&hulk_frontend::ast::NewVectorInit>,
    ) -> HirExpr {
        use crate::hir::HirVectorNewInit;
        let inner_ty = self.typeref_to_type(elem_type_ref);
        let size_hir = self.analyze_expr(size);
        let vector_ty = Type::Vector(Box::new(inner_ty.clone()));

        let init_hir = init.map(|init_info| {
            self.push_scope();
            let sym =
                self.define_symbol(init_info.var.clone(), inner_ty.clone(), SymbolKind::Local);
            let body_hir = self.analyze_expr(&init_info.body);
            self.pop_scope();
            HirVectorNewInit {
                var: init_info.var.clone(),
                symbol: sym.id,
                body: Box::new(body_hir),
            }
        });

        self.make_expr(
            span,
            vector_ty,
            HirExprKind::VectorNew {
                size: Box::new(size_hir),
                element_type: inner_ty,
                init: init_hir,
            },
        )
    }

    fn analyze_vector_generator(
        &mut self,
        span: Span,
        body: &Expr,
        var: &str,
        iterable: &Expr,
    ) -> HirExpr {
        let iterable_hir = self.analyze_expr(iterable);
        let iter_elem_ty = match &iterable_hir.ty {
            Type::Iterable(inner) | Type::Vector(inner) => *inner.clone(),
            Type::Unknown => Type::Unknown,
            other => {
                self.errors.push(SemanticError::InvalidIterableTarget {
                    found: other.clone(),
                });
                Type::Unknown
            }
        };

        self.push_scope();
        let symbol = self.define_symbol(var.to_string(), iter_elem_ty.clone(), SymbolKind::Local);
        let hir_var = HirParam {
            name: var.to_string(),
            ty: iter_elem_ty,
            symbol: symbol.id,
            span,
        };
        let body_hir = self.analyze_expr(body);
        let element_type = body_hir.ty.clone();
        self.pop_scope();

        self.make_expr(
            span,
            Type::Vector(Box::new(element_type.clone())),
            HirExprKind::VectorGenerator {
                body: Box::new(body_hir),
                var: hir_var,
                iterable: Box::new(iterable_hir),
                element_type,
            },
        )
    }

    fn analyze_vector_index(&mut self, span: Span, vector: &Expr, index: &Expr) -> HirExpr {
        let vector_hir = self.analyze_expr(vector);
        let index_hir = self.analyze_expr(index);

        if index_hir.ty != Type::Number && index_hir.ty != Type::Unknown {
            self.errors.push(SemanticError::TypeMismatch {
                expected: Type::Number,
                found: index_hir.ty.clone(),
            });
        }

        let element_type = match &vector_hir.ty {
            Type::Vector(inner) => *inner.clone(),
            Type::Unknown => Type::Unknown,
            other => {
                self.errors.push(SemanticError::InvalidIndexTarget {
                    found: other.clone(),
                });
                Type::Unknown
            }
        };

        self.make_expr(
            span,
            element_type.clone(),
            HirExprKind::VectorIndex {
                vector: Box::new(vector_hir),
                index: Box::new(index_hir),
                element_type,
            },
        )
    }

    fn analyze_lambda(
        &mut self,
        span: Span,
        params: &[Param],
        return_type: Option<&TypeRef>,
        body: &Expr,
    ) -> HirExpr {
        self.push_scope();
        let mut hir_params = Vec::new();
        let mut param_types = Vec::new();

        for param in params {
            let ty = param
                .ty
                .as_ref()
                .map(Type::from_type_ref)
                .unwrap_or(Type::Object);
            self.validate_user_type(&ty);
            let symbol = self.define_symbol(param.name.clone(), ty.clone(), SymbolKind::Parameter);
            param_types.push(ty.clone());
            hir_params.push(HirParam {
                name: param.name.clone(),
                ty,
                symbol: symbol.id,
                span,
            });
        }

        let body_hir = self.analyze_expr(body);
        self.pop_scope();

        let ret_ty = if let Some(ret_ref) = return_type {
            let declared = Type::from_type_ref(ret_ref);
            self.validate_user_type(&declared);
            declared
        } else {
            body_hir.ty.clone()
        };
        let ty = Type::Functor {
            params: param_types,
            ret: Box::new(ret_ty.clone()),
        };

        self.make_expr(
            span,
            ty,
            HirExprKind::Lambda {
                params: hir_params,
                return_type: ret_ty,
                body: Box::new(body_hir),
            },
        )
    }

    fn analyze_match(&mut self, span: Span, scrutinee: &Expr, arms: &[MatchArm]) -> HirExpr {
        let scr_hir = self.analyze_expr(scrutinee);
        let scr_ty = scr_hir.ty.clone();
        let scr_name = format!("_match_scr_{}", self.next_symbol_id);

        self.push_scope();
        let scr_symbol = self.define_symbol(scr_name.clone(), scr_ty.clone(), SymbolKind::Local);

        let scr_ref = self.make_expr(
            span,
            scr_ty.clone(),
            HirExprKind::Var {
                name: scr_name.clone(),
                symbol: scr_symbol.id,
            },
        );

        // Find the first catch-all arm (Wildcard or Binding).
        let catch_all_idx = arms
            .iter()
            .position(|a| matches!(a.pattern, Pattern::Wildcard | Pattern::Binding(_)));

        let (cond_arms, else_arm) = if let Some(idx) = catch_all_idx {
            (&arms[..idx], Some(&arms[idx]))
        } else {
            (arms, None)
        };

        // Build conditional branches.
        let mut branches: Vec<(HirExpr, HirExpr)> = Vec::new();
        let mut unified_ty = Type::Unknown;

        for arm in cond_arms.iter() {
            let (cond, body) = self.build_arm_cond_and_body(span, &scr_ref, arm);
            unified_ty = self.unify_types(&unified_ty, &body.ty);
            branches.push((cond, body));
        }

        // Build else branch.
        let else_hir = if let Some(arm) = else_arm {
            match &arm.pattern {
                Pattern::Wildcard => self.analyze_expr(&arm.body),
                Pattern::Binding(bind_name) => {
                    self.push_scope();
                    let bind_sym = self.define_symbol(
                        bind_name.clone(),
                        scr_ty.clone(),
                        SymbolKind::Local,
                    );
                    let body_hir = self.analyze_expr(&arm.body);
                    let body_ty = body_hir.ty.clone();
                    self.pop_scope();
                    let binding = HirLetBinding {
                        name: bind_name.clone(),
                        symbol: bind_sym.id,
                        ty: scr_ty.clone(),
                        value: scr_ref.clone(),
                        span,
                    };
                    self.make_expr(
                        span,
                        body_ty,
                        HirExprKind::Let {
                            bindings: vec![binding],
                            body: Box::new(body_hir),
                        },
                    )
                }
                _ => unreachable!(),
            }
        } else {
            // Non-exhaustive: emit a diagnostic and produce a dummy else.
            let ty_name = format!("{:?}", scr_ty);
            self.errors.push(SemanticError::NonExhaustiveMatch {
                scrutinee_type: ty_name,
            });
            self.make_expr(span, Type::Unknown, HirExprKind::Number(0.0))
        };

        unified_ty = self.unify_types(&unified_ty, &else_hir.ty);

        let if_hir = self.make_expr(
            span,
            unified_ty,
            HirExprKind::If {
                branches,
                else_branch: Box::new(else_hir),
            },
        );

        self.pop_scope();

        let scr_binding = HirLetBinding {
            name: scr_name,
            symbol: scr_symbol.id,
            ty: scr_ty,
            value: scr_hir,
            span,
        };
        let result_ty = if_hir.ty.clone();
        self.make_expr(
            span,
            result_ty,
            HirExprKind::Let {
                bindings: vec![scr_binding],
                body: Box::new(if_hir),
            },
        )
    }

    fn build_arm_cond_and_body(
        &mut self,
        span: Span,
        scr_ref: &HirExpr,
        arm: &MatchArm,
    ) -> (HirExpr, HirExpr) {
        match &arm.pattern {
            Pattern::TypePattern { type_name, bind } => {
                let cond = self.make_expr(
                    span,
                    Type::Boolean,
                    HirExprKind::TypeTest {
                        expr: Box::new(scr_ref.clone()),
                        type_name: type_name.clone(),
                    },
                );
                let body = if let Some(bind_name) = bind {
                    let bind_ty = Type::UserType(type_name.clone());
                    self.push_scope();
                    let cast_val = self.make_expr(
                        span,
                        bind_ty.clone(),
                        HirExprKind::TypeCast {
                            expr: Box::new(scr_ref.clone()),
                            type_name: type_name.clone(),
                        },
                    );
                    let bind_sym = self.define_symbol(
                        bind_name.clone(),
                        bind_ty.clone(),
                        SymbolKind::Local,
                    );
                    let body_hir = self.analyze_expr(&arm.body);
                    let body_ty = body_hir.ty.clone();
                    self.pop_scope();
                    let binding = HirLetBinding {
                        name: bind_name.clone(),
                        symbol: bind_sym.id,
                        ty: bind_ty,
                        value: cast_val,
                        span,
                    };
                    self.make_expr(
                        span,
                        body_ty,
                        HirExprKind::Let {
                            bindings: vec![binding],
                            body: Box::new(body_hir),
                        },
                    )
                } else {
                    self.analyze_expr(&arm.body)
                };
                (cond, body)
            }
            Pattern::Literal(lit) => {
                let lit_hir = match lit {
                    LiteralPattern::Number(v) => {
                        self.make_expr(span, Type::Number, HirExprKind::Number(*v))
                    }
                    LiteralPattern::String(s) => {
                        self.make_expr(span, Type::String, HirExprKind::String(s.clone()))
                    }
                    LiteralPattern::Bool(b) => {
                        self.make_expr(span, Type::Boolean, HirExprKind::Bool(*b))
                    }
                };
                let cond = self.make_expr(
                    span,
                    Type::Boolean,
                    HirExprKind::Binary {
                        op: BinaryOp::Eq,
                        left: Box::new(scr_ref.clone()),
                        right: Box::new(lit_hir),
                    },
                );
                let body = self.analyze_expr(&arm.body);
                (cond, body)
            }
            // Binding/Wildcard appearing before the last arm: treat as unconditional.
            Pattern::Binding(bind_name) => {
                let cond = self.make_expr(span, Type::Boolean, HirExprKind::Bool(true));
                self.push_scope();
                let bind_sym = self.define_symbol(
                    bind_name.clone(),
                    scr_ref.ty.clone(),
                    SymbolKind::Local,
                );
                let body_hir = self.analyze_expr(&arm.body);
                let body_ty = body_hir.ty.clone();
                self.pop_scope();
                let binding = HirLetBinding {
                    name: bind_name.clone(),
                    symbol: bind_sym.id,
                    ty: scr_ref.ty.clone(),
                    value: scr_ref.clone(),
                    span,
                };
                let wrapped = self.make_expr(
                    span,
                    body_ty,
                    HirExprKind::Let {
                        bindings: vec![binding],
                        body: Box::new(body_hir),
                    },
                );
                (cond, wrapped)
            }
            Pattern::Wildcard => {
                let cond = self.make_expr(span, Type::Boolean, HirExprKind::Bool(true));
                let body = self.analyze_expr(&arm.body);
                (cond, body)
            }
        }
    }

    /// Inline-expand a `define` macro call with call-by-name substitution.
    fn inline_macro(
        &mut self,
        span: Span,
        decl: &FunctionDecl,
        args: &[Expr],
    ) -> HirExpr {
        let subst: HashMap<String, Expr> = decl
            .params
            .iter()
            .zip(args.iter())
            .map(|(p, a)| (p.name.clone(), a.clone()))
            .collect();
        let body = substitute_expr(&decl.body, &subst);
        self.analyze_expr_with_span(span, &body)
    }

    fn analyze_call(&mut self, span: Span, callee: &Expr, args: &[Expr]) -> HirExpr {
        let Expr::Var(name, _) = callee else {
            self.errors.push(SemanticError::UnsupportedConstruct {
                message: "HIR lowering for non-variable callees is not implemented yet".to_string(),
            });
            self.analyze_expr(callee);
            let hir_args = args.iter().map(|arg| self.analyze_expr(arg)).collect();
            return self.make_expr(
                span,
                Type::Unknown,
                HirExprKind::Call {
                    callee: HirCallee::GlobalFunction {
                        name: "<unknown>".to_string(),
                        signature: FunctionType {
                            params: Vec::new(),
                            return_type: Type::Unknown,
                        },
                    },
                    args: hir_args,
                },
            );
        };

        // Inline `define` macros with call-by-name substitution.
        if let Some(decl) = self.macro_decls.get(name).cloned() {
            if args.len() == decl.params.len() {
                return self.inline_macro(span, &decl, args);
            }
        }

        if let Some(signature) = self.functions.get(name).cloned() {
            let (hir_args, signature) = self.analyze_call_args(name, args, &signature);
            let callee = if self.is_builtin_function(name) {
                HirCallee::Builtin {
                    name: name.clone(),
                    signature: signature.clone(),
                }
            } else {
                HirCallee::GlobalFunction {
                    name: name.clone(),
                    signature: signature.clone(),
                }
            };
            return self.make_expr(
                span,
                signature.return_type.clone(),
                HirExprKind::Call {
                    callee,
                    args: hir_args,
                },
            );
        }

        if let Some(symbol) = self.resolve_symbol(name) {
            if matches!(symbol.kind, SymbolKind::BuiltinConstant) {
                self.errors
                    .push(SemanticError::UndefinedFunction { name: name.clone(), span });
                let hir_args = args.iter().map(|arg| self.analyze_expr(arg)).collect();
                return self.make_expr(
                    span,
                    Type::Unknown,
                    HirExprKind::Call {
                        callee: HirCallee::GlobalFunction {
                            name: name.clone(),
                            signature: FunctionType {
                                params: Vec::new(),
                                return_type: Type::Unknown,
                            },
                        },
                        args: hir_args,
                    },
                );
            }

            if let Type::Functor { params, ret } = symbol.ty.clone() {
                let signature = FunctionType {
                    params,
                    return_type: *ret,
                };
                let (hir_args, signature) = self.analyze_call_args(name, args, &signature);
                return self.make_expr(
                    span,
                    signature.return_type.clone(),
                    HirExprKind::Call {
                        callee: HirCallee::LocalFunctor {
                            name: name.clone(),
                            symbol: symbol.id,
                            signature,
                        },
                        args: hir_args,
                    },
                );
            }
        }

        self.errors
            .push(SemanticError::UndefinedFunction { name: name.clone(), span });
        let hir_args = args.iter().map(|arg| self.analyze_expr(arg)).collect();
        self.make_expr(
            span,
            Type::Unknown,
            HirExprKind::Call {
                callee: HirCallee::GlobalFunction {
                    name: name.clone(),
                    signature: FunctionType {
                        params: Vec::new(),
                        return_type: Type::Unknown,
                    },
                },
                args: hir_args,
            },
        )
    }

    fn analyze_call_args(
        &mut self,
        name: &str,
        args: &[Expr],
        signature: &FunctionType,
    ) -> (Vec<HirExpr>, FunctionType) {
        let mut hir_args = Vec::new();
        let mut signature = signature.clone();

        if signature.params.len() != args.len() {
            self.errors.push(SemanticError::ArityMismatch {
                function: name.to_string(),
                expected: signature.params.len(),
                found: args.len(),
            });
            for arg in args {
                hir_args.push(self.analyze_expr(arg));
            }
            return (hir_args, signature);
        }

        for (idx, arg) in args.iter().enumerate() {
            let hir_arg = self.analyze_expr(arg);
            if signature.params[idx] == Type::Unknown && hir_arg.ty != Type::Unknown {
                self.refine_function_param_type(name, idx, &hir_arg.ty);
                signature.params[idx] = hir_arg.ty.clone();
            }
            if !self.is_assignable(&hir_arg.ty, &signature.params[idx]) {
                self.errors.push(SemanticError::InvalidArgumentType {
                    function: name.to_string(),
                    index: idx,
                    expected: signature.params[idx].clone(),
                    found: hir_arg.ty.clone(),
                });
            }
            hir_args.push(hir_arg);
        }

        (hir_args, signature)
    }

    fn analyze_method_call_args(
        &mut self,
        name: &str,
        refine_target: Option<(&str, &str)>,
        args: &[Expr],
        signature: &FunctionType,
    ) -> (Vec<HirExpr>, FunctionType) {
        let mut hir_args = Vec::new();
        let mut signature = signature.clone();

        if signature.params.len() != args.len() {
            self.errors.push(SemanticError::ArityMismatch {
                function: name.to_string(),
                expected: signature.params.len(),
                found: args.len(),
            });
            for arg in args {
                hir_args.push(self.analyze_expr(arg));
            }
            return (hir_args, signature);
        }

        for (idx, arg) in args.iter().enumerate() {
            let hir_arg = self.analyze_expr(arg);
            if let Some((owner_type, method_name)) = refine_target {
                if signature.params[idx] == Type::Unknown && hir_arg.ty != Type::Unknown {
                    self.refine_method_param_type(owner_type, method_name, idx, &hir_arg.ty);
                    signature.params[idx] = hir_arg.ty.clone();
                }
            }
            if !self.is_assignable(&hir_arg.ty, &signature.params[idx]) {
                self.errors.push(SemanticError::InvalidArgumentType {
                    function: name.to_string(),
                    index: idx,
                    expected: signature.params[idx].clone(),
                    found: hir_arg.ty.clone(),
                });
            }
            hir_args.push(hir_arg);
        }

        (hir_args, signature)
    }

    fn method_signature_for_call(
        &self,
        obj_ty: &Type,
        method: &str,
    ) -> Option<(String, FunctionType)> {
        match obj_ty {
            Type::UserType(type_name) => self
                .registry
                .lookup_method_owner_info(type_name, method)
                .map(|(owner, info)| {
                    let key = Self::method_signature_key(&owner, method);
                    let signature =
                        self.method_signatures
                            .get(&key)
                            .cloned()
                            .unwrap_or(FunctionType {
                                params: info.params,
                                return_type: info.return_type,
                            });
                    (owner, signature)
                }),
            Type::Vector(inner) => match method {
                "next" => Some((
                    "Vector".to_string(),
                    FunctionType {
                        params: vec![],
                        return_type: Type::Boolean,
                    },
                )),
                "size" => Some((
                    "Vector".to_string(),
                    FunctionType {
                        params: vec![],
                        return_type: Type::Number,
                    },
                )),
                "current" => Some((
                    "Vector".to_string(),
                    FunctionType {
                        params: vec![],
                        return_type: *inner.clone(),
                    },
                )),
                _ => None,
            },
            Type::Iterable(inner) => match method {
                "next" => Some((
                    "Iterable".to_string(),
                    FunctionType {
                        params: vec![],
                        return_type: Type::Boolean,
                    },
                )),
                "current" => Some((
                    "Iterable".to_string(),
                    FunctionType {
                        params: vec![],
                        return_type: *inner.clone(),
                    },
                )),
                "size" => Some((
                    "Iterable".to_string(),
                    FunctionType {
                        params: vec![],
                        return_type: Type::Number,
                    },
                )),
                _ => None,
            },
            Type::String => match method {
                "length" | "size" => Some((
                    "String".to_string(),
                    FunctionType {
                        params: vec![],
                        return_type: Type::Number,
                    },
                )),
                "substring" => Some((
                    "String".to_string(),
                    FunctionType {
                        params: vec![Type::Number, Type::Number],
                        return_type: Type::String,
                    },
                )),
                _ => None,
            },
            _ => None,
        }
    }

    fn is_builtin_function(&self, name: &str) -> bool {
        builtin_functions()
            .iter()
            .any(|builtin| builtin.name == name)
    }

    fn is_assignable(&self, sub: &Type, target: &Type) -> bool {
        if *sub == Type::Unknown || *target == Type::Unknown {
            return true;
        }
        if let Type::UserType(target_name) = target {
            if self.registry.get_protocol(target_name).is_some() {
                return match sub {
                    Type::UserType(concrete_name) => self
                        .registry
                        .implicitly_conforms_to_protocol(concrete_name, target_name),
                    Type::Iterable(_) if target_name == "Iterable" => true,
                    _ => false,
                };
            }
        }
        if sub == target || *target == Type::Object {
            return true;
        }
        match (sub, target) {
            (Type::UserType(sn), Type::UserType(tn)) => {
                // Inheritance check
                if self.registry.is_descendant_of(sn, tn) {
                    return true;
                }
                // Structural typing: sn implicitly conforms to protocol tn
                if self.registry.get_protocol(tn).is_some() {
                    return self.registry.implicitly_conforms_to_protocol(sn, tn);
                }
                false
            }
            (Type::UserType(_), Type::Object) => true,
            // Structural typing: UserType with next()/current() satisfies Iterable(elem).
            (Type::UserType(sn), Type::Iterable(elem)) => {
                self.registry.implements_iterable(sn, elem)
            }
            (Type::Vector(si), Type::Vector(ti)) => self.is_assignable(si, ti),
            (Type::Iterable(si), Type::Iterable(ti)) => self.is_assignable(si, ti),
            (Type::Iterable(_), Type::UserType(n)) if n == "Iterable" => true,
            _ => false,
        }
    }

    fn unify_types(&self, a: &Type, b: &Type) -> Type {
        if *a == Type::Unknown {
            return b.clone();
        }
        if *b == Type::Unknown {
            return a.clone();
        }
        if a == b {
            return a.clone();
        }
        if self.is_assignable(a, b) {
            return b.clone();
        }
        if self.is_assignable(b, a) {
            return a.clone();
        }
        match (a, b) {
            (Type::UserType(an), Type::UserType(bn)) => self.registry.least_common_ancestor(an, bn),
            _ => Type::Object,
        }
    }

    fn are_branch_types_compatible(&self, a: &Type, b: &Type) -> bool {
        if *a == Type::Unknown || *b == Type::Unknown {
            return true;
        }
        if a == b || *a == Type::Object || *b == Type::Object {
            return true;
        }
        if self.is_assignable(a, b) || self.is_assignable(b, a) {
            return true;
        }
        matches!((a, b), (Type::UserType(_), Type::UserType(_)))
    }

    fn constrain_numeric_operand(&mut self, expr: &Expr, actual: &Type) -> bool {
        if *actual == Type::Unknown {
            self.constrain_expr_type(expr, &Type::Number);
            matches!(expr, Expr::Var(_, _))
        } else {
            *actual == Type::Number
        }
    }

    fn constrain_boolean_operand(&mut self, expr: &Expr, actual: &Type) -> bool {
        if *actual == Type::Unknown {
            self.constrain_expr_type(expr, &Type::Boolean);
            matches!(expr, Expr::Var(_, _))
        } else {
            *actual == Type::Boolean
        }
    }

    fn constrain_concat_operand(&mut self, expr: &Expr, actual: &Type) -> bool {
        if *actual == Type::Unknown {
            self.constrain_expr_type(expr, &Type::String);
            matches!(expr, Expr::Var(_, _))
        } else {
            is_concat_compatible(actual)
        }
    }
}

fn is_concat_compatible(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Number
            | Type::String
            | Type::Boolean
            | Type::Object
            | Type::UserType(_)
            | Type::Unknown
    )
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

/// Recursively substitute variable references in an AST expression.
/// For each parameter name in `subst`, replace `Expr::Var(name)` with the
/// corresponding argument expression. This implements call-by-name for `define`.
fn substitute_expr(expr: &Expr, subst: &HashMap<String, Expr>) -> Expr {
    match expr {
        Expr::Var(name, span) => {
            if let Some(replacement) = subst.get(name.as_str()) {
                replacement.clone()
            } else {
                Expr::Var(name.clone(), *span)
            }
        }
        Expr::Number(_) | Expr::String(_) | Expr::Bool(_) | Expr::SelfRef => expr.clone(),
        Expr::Unary { op, expr: inner, span } => Expr::Unary {
            op: op.clone(),
            expr: Box::new(substitute_expr(inner, subst)),
            span: *span,
        },
        Expr::Binary { op, left, right, span } => Expr::Binary {
            op: op.clone(),
            left: Box::new(substitute_expr(left, subst)),
            right: Box::new(substitute_expr(right, subst)),
            span: *span,
        },
        Expr::Let { bindings, body, span } => {
            let mut inner = subst.clone();
            let new_bindings: Vec<_> = bindings.iter().map(|b| {
                let new_val = substitute_expr(&b.value, &inner);
                inner.remove(&b.name);
                hulk_frontend::ast::LetBinding { name: b.name.clone(), ty: b.ty.clone(), value: new_val }
            }).collect();
            Expr::Let {
                bindings: new_bindings,
                body: Box::new(substitute_expr(body, &inner)),
                span: *span,
            }
        }
        Expr::Assign { target, value, span } => Expr::Assign {
            target: Box::new(substitute_expr(target, subst)),
            value: Box::new(substitute_expr(value, subst)),
            span: *span,
        },
        Expr::Block(exprs) => Expr::Block(
            exprs.iter().map(|e| substitute_expr(e, subst)).collect(),
        ),
        Expr::If { span, branches, else_branch } => Expr::If {
            span: *span,
            branches: branches.iter().map(|(c, b)| (substitute_expr(c, subst), substitute_expr(b, subst))).collect(),
            else_branch: Box::new(substitute_expr(else_branch, subst)),
        },
        Expr::While { span, condition, body } => Expr::While {
            span: *span,
            condition: Box::new(substitute_expr(condition, subst)),
            body: Box::new(substitute_expr(body, subst)),
        },
        Expr::For { span, var, iterable, body } => Expr::For {
            span: *span,
            var: var.clone(),
            iterable: Box::new(substitute_expr(iterable, subst)),
            body: Box::new(substitute_expr(body, subst)),
        },
        Expr::Call { callee, args, span } => Expr::Call {
            callee: Box::new(substitute_expr(callee, subst)),
            args: args.iter().map(|a| substitute_expr(a, subst)).collect(),
            span: *span,
        },
        Expr::MethodCall { span, object, method, args } => Expr::MethodCall {
            span: *span,
            object: Box::new(substitute_expr(object, subst)),
            method: method.clone(),
            args: args.iter().map(|a| substitute_expr(a, subst)).collect(),
        },
        Expr::MemberAccess { span, object, member } => Expr::MemberAccess {
            span: *span,
            object: Box::new(substitute_expr(object, subst)),
            member: member.clone(),
        },
        Expr::New { span, type_name, args } => Expr::New {
            span: *span,
            type_name: type_name.clone(),
            args: args.iter().map(|a| substitute_expr(a, subst)).collect(),
        },
        Expr::BaseCall { span, args } => Expr::BaseCall {
            span: *span,
            args: args.iter().map(|a| substitute_expr(a, subst)).collect(),
        },
        Expr::TypeTest { span, expr: inner, type_name } => Expr::TypeTest {
            span: *span,
            expr: Box::new(substitute_expr(inner, subst)),
            type_name: type_name.clone(),
        },
        Expr::TypeCast { span, expr: inner, type_name } => Expr::TypeCast {
            span: *span,
            expr: Box::new(substitute_expr(inner, subst)),
            type_name: type_name.clone(),
        },
        Expr::VectorLiteral(elements) => Expr::VectorLiteral(
            elements.iter().map(|e| substitute_expr(e, subst)).collect(),
        ),
        Expr::NewVector { span, elem_type, size, init } => Expr::NewVector {
            span: *span,
            elem_type: elem_type.clone(),
            size: Box::new(substitute_expr(size, subst)),
            init: init.as_ref().map(|i| hulk_frontend::ast::NewVectorInit {
                var: i.var.clone(),
                body: Box::new(substitute_expr(&i.body, subst)),
            }),
        },
        Expr::VectorGenerator { span, body, var, iterable } => Expr::VectorGenerator {
            span: *span,
            body: Box::new(substitute_expr(body, subst)),
            var: var.clone(),
            iterable: Box::new(substitute_expr(iterable, subst)),
        },
        Expr::VectorIndex { span, vector, index } => Expr::VectorIndex {
            span: *span,
            vector: Box::new(substitute_expr(vector, subst)),
            index: Box::new(substitute_expr(index, subst)),
        },
        Expr::Lambda { span, params, return_type, body } => Expr::Lambda {
            span: *span,
            params: params.clone(),
            return_type: return_type.clone(),
            body: Box::new(substitute_expr(body, subst)),
        },
        Expr::Match { span, scrutinee, arms } => Expr::Match {
            span: *span,
            scrutinee: Box::new(substitute_expr(scrutinee, subst)),
            arms: arms
                .iter()
                .map(|arm| {
                    let mut inner = subst.clone();
                    // Binding patterns shadow the substitution variable inside the arm body.
                    match &arm.pattern {
                        Pattern::Binding(name) => {
                            inner.remove(name.as_str());
                        }
                        Pattern::TypePattern { bind: Some(name), .. } => {
                            inner.remove(name.as_str());
                        }
                        _ => {}
                    }
                    hulk_frontend::ast::MatchArm {
                        pattern: arm.pattern.clone(),
                        body: substitute_expr(&arm.body, &inner),
                        span: arm.span,
                    }
                })
                .collect(),
        },
    }
}
