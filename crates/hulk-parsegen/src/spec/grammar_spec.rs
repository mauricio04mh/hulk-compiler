use crate::production::Production;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct GrammarSpec {
    pub start: String,
    pub productions: Vec<Production>,
    pub non_terminals: HashSet<String>,
    pub terminals: HashSet<String>,
}

impl GrammarSpec {
    pub fn productions_for(&self, lhs: &str) -> Vec<&Production> {
        self.productions.iter().filter(|p| p.lhs == lhs).collect()
    }

    pub fn is_non_terminal(&self, name: &str) -> bool {
        self.non_terminals.contains(name)
    }

    pub fn is_terminal(&self, name: &str) -> bool {
        self.terminals.contains(name)
    }
}
