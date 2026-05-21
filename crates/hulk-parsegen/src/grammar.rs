use crate::production::Production;
use crate::symbol::Symbol;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Grammar {
    pub start: String,
    pub productions: Vec<Production>,
}

impl Grammar {
    pub fn productions_for(&self, non_terminal: &str) -> Vec<&Production> {
        self.productions
            .iter()
            .filter(|p| p.lhs == non_terminal)
            .collect()
    }

    pub fn non_terminals(&self) -> HashSet<String> {
        self.productions.iter().map(|p| p.lhs.clone()).collect()
    }

    pub fn terminals(&self) -> HashSet<String> {
        let mut terminals = HashSet::new();

        for production in &self.productions {
            for symbol in &production.rhs {
                if let Symbol::Terminal(name) = symbol {
                    terminals.insert(name.clone());
                }
            }
        }

        terminals
    }

    pub fn is_non_terminal(&self, name: &str) -> bool {
        self.productions.iter().any(|p| p.lhs == name)
    }

    pub fn is_terminal(&self, name: &str) -> bool {
        self.terminals().contains(name)
    }
}

#[cfg(test)]
mod tests {
    use super::Grammar;
    use crate::production::Production;
    use crate::symbol::Symbol;

    #[test]
    fn can_create_basic_grammar() {
        let grammar = Grammar {
            start: "Program".to_string(),
            productions: vec![
                Production {
                    lhs: "Program".to_string(),
                    rhs: vec![Symbol::NonTerminal("Expr".to_string()), Symbol::Eof],
                },
                Production {
                    lhs: "Expr".to_string(),
                    rhs: vec![Symbol::Terminal("NUMBER".to_string())],
                },
            ],
        };

        assert_eq!(grammar.start, "Program");
        assert_eq!(grammar.productions.len(), 2);
    }

    #[test]
    fn returns_productions_for_non_terminal() {
        let grammar = Grammar {
            start: "Program".to_string(),
            productions: vec![
                Production {
                    lhs: "Program".to_string(),
                    rhs: vec![Symbol::NonTerminal("Expr".to_string()), Symbol::Eof],
                },
                Production {
                    lhs: "Expr".to_string(),
                    rhs: vec![Symbol::Terminal("NUMBER".to_string())],
                },
                Production {
                    lhs: "Expr".to_string(),
                    rhs: vec![Symbol::Terminal("IDENT".to_string())],
                },
            ],
        };

        let expr_rules = grammar.productions_for("Expr");

        assert_eq!(expr_rules.len(), 2);
        assert!(expr_rules.iter().all(|p| p.lhs == "Expr"));
    }

    #[test]
    fn returns_non_terminals_without_duplicates() {
        let grammar = Grammar {
            start: "Program".to_string(),
            productions: vec![
                Production {
                    lhs: "Program".to_string(),
                    rhs: vec![Symbol::NonTerminal("Expr".to_string()), Symbol::Eof],
                },
                Production {
                    lhs: "Expr".to_string(),
                    rhs: vec![Symbol::Terminal("NUMBER".to_string())],
                },
                Production {
                    lhs: "Expr".to_string(),
                    rhs: vec![Symbol::Terminal("IDENT".to_string())],
                },
            ],
        };

        let non_terminals = grammar.non_terminals();

        assert!(non_terminals.contains("Program"));
        assert!(non_terminals.contains("Expr"));
        assert_eq!(non_terminals.len(), 2);
    }

    #[test]
    fn computes_terminals_from_rhs_symbols() {
        let grammar = Grammar {
            start: "Program".to_string(),
            productions: vec![
                Production {
                    lhs: "Program".to_string(),
                    rhs: vec![Symbol::NonTerminal("Expr".to_string()), Symbol::Eof],
                },
                Production {
                    lhs: "Expr".to_string(),
                    rhs: vec![Symbol::Terminal("NUMBER".to_string())],
                },
            ],
        };

        let terminals = grammar.terminals();
        assert!(terminals.contains("NUMBER"));
        assert_eq!(terminals.len(), 1);
    }

    #[test]
    fn identifies_terminal_and_non_terminal_names() {
        let grammar = Grammar {
            start: "Program".to_string(),
            productions: vec![
                Production {
                    lhs: "Program".to_string(),
                    rhs: vec![Symbol::NonTerminal("Expr".to_string()), Symbol::Eof],
                },
                Production {
                    lhs: "Expr".to_string(),
                    rhs: vec![Symbol::Terminal("NUMBER".to_string())],
                },
            ],
        };

        assert!(grammar.is_non_terminal("Program"));
        assert!(grammar.is_non_terminal("Expr"));
        assert!(!grammar.is_non_terminal("NUMBER"));

        assert!(grammar.is_terminal("NUMBER"));
        assert!(!grammar.is_terminal("Expr"));
    }
}
