use crate::error::SemanticError;
use crate::symbols::Symbol;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ScopeStack {
    scopes: Vec<HashMap<String, Symbol>>,
}

impl ScopeStack {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn define(&mut self, symbol: Symbol) -> Result<(), SemanticError> {
        let current = self.scopes.last_mut().expect("scope stack never empty");
        if current.contains_key(&symbol.name) {
            return Err(SemanticError::DuplicateSymbol { name: symbol.name });
        }
        current.insert(symbol.name.clone(), symbol);
        Ok(())
    }

    pub fn resolve(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(symbol) = scope.get(name) {
                return Some(symbol);
            }
        }
        None
    }

    pub fn resolve_current(&self, name: &str) -> Option<&Symbol> {
        self.scopes.last().and_then(|scope| scope.get(name))
    }
}
