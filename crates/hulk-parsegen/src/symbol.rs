#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Symbol {
    Terminal(String),
    NonTerminal(String),
    Epsilon,
    Eof,
}

#[cfg(test)]
mod tests {
    use super::Symbol;

    #[test]
    fn symbol_variants_are_comparable_and_hashable() {
        let a = Symbol::Terminal("NUMBER".to_string());
        let b = Symbol::Terminal("NUMBER".to_string());
        let c = Symbol::NonTerminal("Expr".to_string());

        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
