use crate::builtins::{builtin_constants, builtin_functions};
use crate::error::SemanticError;
use crate::scope::ScopeStack;
use crate::symbols::{FunctionSymbol, Symbol, SymbolKind};
use hulk_frontend::ast::{Decl, Expr, FunctionDecl, Program, TypeDecl, TypeMember};
use std::collections::{HashMap, HashSet};

pub struct Resolver {
    globals: HashMap<String, FunctionSymbol>,
    builtins: HashMap<String, usize>,
    scopes: ScopeStack,
    type_names: HashSet<String>,
}

pub fn resolve_program(program: &Program) -> Result<(), SemanticError> {
    let mut resolver = Resolver::new();
    resolver.resolve_program(program)
}

impl Resolver {
    fn new() -> Self {
        let mut resolver = Self {
            globals: HashMap::new(),
            builtins: HashMap::new(),
            scopes: ScopeStack::new(),
            type_names: HashSet::new(),
        };
        resolver.register_builtins();
        resolver
    }

    fn register_builtins(&mut self) {
        for signature in builtin_functions() {
            self.builtins.insert(signature.name.to_string(), signature.params.len());
            self.scopes
                .define(Symbol {
                    name: signature.name.to_string(),
                    kind: SymbolKind::BuiltinFunction,
                })
                .expect("builtins should be unique");
        }

        for (name, _) in builtin_constants() {
            self.scopes
                .define(Symbol {
                    name: name.to_string(),
                    kind: SymbolKind::BuiltinConstant,
                })
                .expect("builtin constants should be unique");
        }
    }

    fn resolve_program(&mut self, program: &Program) -> Result<(), SemanticError> {
        // Pre-pass: collect all names so declarations can reference each other.
        for decl in &program.declarations {
            self.register_decl(decl)?;
        }

        for decl in &program.declarations {
            self.resolve_decl(decl)?;
        }

        self.resolve_expr(&program.entry)
    }

    fn register_decl(&mut self, decl: &Decl) -> Result<(), SemanticError> {
        match decl {
            Decl::Function(func) => {
                if self.globals.contains_key(&func.name) {
                    return Err(SemanticError::DuplicateFunction {
                        name: func.name.clone(),
                    });
                }
                self.globals.insert(
                    func.name.clone(),
                    FunctionSymbol {
                        name: func.name.clone(),
                        params_len: func.params.len(),
                        builtin: false,
                    },
                );
                Ok(())
            }
            Decl::Type(td) => {
                self.type_names.insert(td.name.clone());
                Ok(())
            }
            Decl::Protocol(pd) => {
                self.type_names.insert(pd.name.clone());
                Ok(())
            }
        }
    }

    fn resolve_decl(&mut self, decl: &Decl) -> Result<(), SemanticError> {
        match decl {
            Decl::Function(func) => self.resolve_function_decl(func),
            Decl::Type(td) => self.resolve_type_decl(td),
            Decl::Protocol(_) => Ok(()),
        }
    }

    fn resolve_function_decl(&mut self, func: &FunctionDecl) -> Result<(), SemanticError> {
        self.scopes.push();

        for param in &func.params {
            if self.scopes.resolve_current(&param.name).is_some() {
                self.scopes.pop();
                return Err(SemanticError::DuplicateParameter {
                    function: func.name.clone(),
                    parameter: param.name.clone(),
                });
            }

            self.scopes.define(Symbol {
                name: param.name.clone(),
                kind: SymbolKind::Parameter,
            })?;
        }

        let result = self.resolve_expr(&func.body);
        self.scopes.pop();
        result
    }

    fn resolve_type_decl(&mut self, td: &TypeDecl) -> Result<(), SemanticError> {
        // Constructor params are in scope for attribute initializers and method bodies.
        self.scopes.push();

        for param in &td.params {
            if self.scopes.resolve_current(&param.name).is_some() {
                self.scopes.pop();
                return Err(SemanticError::DuplicateParameter {
                    function: td.name.clone(),
                    parameter: param.name.clone(),
                });
            }
            self.scopes.define(Symbol {
                name: param.name.clone(),
                kind: SymbolKind::Parameter,
            })?;
        }

        for member in &td.members {
            match member {
                TypeMember::Attribute(attr) => {
                    self.resolve_expr(&attr.value)?;
                }
                TypeMember::Method(method) => {
                    self.scopes.push();
                    for param in &method.params {
                        if self.scopes.resolve_current(&param.name).is_some() {
                            self.scopes.pop();
                            self.scopes.pop();
                            return Err(SemanticError::DuplicateParameter {
                                function: method.name.clone(),
                                parameter: param.name.clone(),
                            });
                        }
                        self.scopes.define(Symbol {
                            name: param.name.clone(),
                            kind: SymbolKind::Parameter,
                        })?;
                    }
                    let result = self.resolve_expr(&method.body);
                    self.scopes.pop();
                    result?;
                }
            }
        }

        self.scopes.pop();
        Ok(())
    }

    fn resolve_expr(&mut self, expr: &Expr) -> Result<(), SemanticError> {
        match expr {
            Expr::Number(_) | Expr::String(_) | Expr::Bool(_) => Ok(()),
            Expr::Var(name, _) => self.resolve_variable(name),
            Expr::Unary { expr, .. } => self.resolve_expr(expr),
            Expr::Binary { left, right, .. } => {
                self.resolve_expr(left)?;
                self.resolve_expr(right)
            }
            Expr::Assign { target, value, .. } => {
                match target.as_ref() {
                    Expr::Var(name, _) => {
                        self.resolve_variable(name)?;
                    }
                    _ => return Err(SemanticError::InvalidAssignmentTarget),
                }
                self.resolve_expr(value)
            }
            Expr::Let { bindings, body, .. } => {
                self.scopes.push();
                for binding in bindings {
                    self.resolve_expr(&binding.value)?;
                    self.scopes.define(Symbol {
                        name: binding.name.clone(),
                        kind: SymbolKind::Variable,
                    })?;
                }
                let result = self.resolve_expr(body);
                self.scopes.pop();
                result
            }
            Expr::Call { callee, args, .. } => {
                if let Expr::Var(name, _) = callee.as_ref() {
                    self.resolve_function_call(name, args.len())?;
                } else {
                    self.resolve_expr(callee)?;
                }

                for arg in args {
                    self.resolve_expr(arg)?;
                }
                Ok(())
            }
            Expr::Block(exprs) => {
                self.scopes.push();
                for expr in exprs {
                    self.resolve_expr(expr)?;
                }
                self.scopes.pop();
                Ok(())
            }
            Expr::If {
                branches,
                else_branch,
                ..
            } => {
                for (cond, body) in branches {
                    self.resolve_expr(cond)?;
                    self.resolve_expr(body)?;
                }
                self.resolve_expr(else_branch)
            }
            Expr::While { condition, body, .. } => {
                self.resolve_expr(condition)?;
                self.resolve_expr(body)
            }
            Expr::For { var, iterable, body, .. } => {
                self.resolve_expr(iterable)?;
                self.scopes.push();
                self.scopes.define(Symbol {
                    name: var.clone(),
                    kind: SymbolKind::Variable,
                })?;
                let result = self.resolve_expr(body);
                self.scopes.pop();
                result
            }
            Expr::TypeTest { expr, .. } | Expr::TypeCast { expr, .. } => self.resolve_expr(expr),
            Expr::VectorLiteral(elements) => {
                for el in elements {
                    self.resolve_expr(el)?;
                }
                Ok(())
            }
            Expr::VectorGenerator { body, var, iterable, .. } => {
                self.resolve_expr(iterable)?;
                self.scopes.push();
                self.scopes.define(Symbol {
                    name: var.clone(),
                    kind: SymbolKind::Variable,
                })?;
                let result = self.resolve_expr(body);
                self.scopes.pop();
                result
            }
            Expr::VectorIndex { vector, index, .. } => {
                self.resolve_expr(vector)?;
                self.resolve_expr(index)
            }
            Expr::Lambda { params, body, .. } => {
                self.scopes.push();
                for param in params {
                    self.scopes.define(Symbol {
                        name: param.name.clone(),
                        kind: SymbolKind::Parameter,
                    })?;
                }
                let result = self.resolve_expr(body);
                self.scopes.pop();
                result
            }
            Expr::New { args, .. } => {
                for arg in args {
                    self.resolve_expr(arg)?;
                }
                Ok(())
            }
            Expr::MemberAccess { object, .. } => self.resolve_expr(object),
            Expr::MethodCall { object, args, .. } => {
                self.resolve_expr(object)?;
                for arg in args {
                    self.resolve_expr(arg)?;
                }
                Ok(())
            }
            Expr::SelfRef => Ok(()),
            Expr::BaseCall { args, .. } => {
                for arg in args {
                    self.resolve_expr(arg)?;
                }
                Ok(())
            }
        }
    }

    fn resolve_variable(&self, name: &str) -> Result<(), SemanticError> {
        let Some(symbol) = self.scopes.resolve(name) else {
            return Err(SemanticError::UndefinedVariable {
                name: name.to_string(),
            });
        };

        match symbol.kind {
            SymbolKind::Variable | SymbolKind::Parameter | SymbolKind::BuiltinConstant => Ok(()),
            _ => Err(SemanticError::UndefinedVariable {
                name: name.to_string(),
            }),
        }
    }

    fn resolve_function_call(&self, name: &str, found_arity: usize) -> Result<(), SemanticError> {
        if let Some(expected_arity) = self.builtins.get(name) {
            if *expected_arity != found_arity {
                return Err(SemanticError::ArityMismatch {
                    function: name.to_string(),
                    expected: *expected_arity,
                    found: found_arity,
                });
            }
            return Ok(());
        }

        if let Some(func) = self.globals.get(name) {
            if func.params_len != found_arity {
                return Err(SemanticError::ArityMismatch {
                    function: name.to_string(),
                    expected: func.params_len,
                    found: found_arity,
                });
            }
            return Ok(());
        }

        // May be a variable of Functor type — arity validated in checker.
        if self.scopes.resolve(name).is_some() {
            return Ok(());
        }

        Err(SemanticError::UndefinedFunction {
            name: name.to_string(),
        })
    }
}
