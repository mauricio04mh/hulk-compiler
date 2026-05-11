use crate::symbol::Symbol;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Production {
    pub lhs: String,
    pub rhs: Vec<Symbol>,
}

#[cfg(test)]
mod tests {
    use super::Production;
    use crate::symbol::Symbol;

    #[test]
    fn can_create_production() {
        let production = Production {
            lhs: "Expr".to_string(),
            rhs: vec![Symbol::Terminal("NUMBER".to_string())],
        };

        assert_eq!(production.lhs, "Expr");
        assert_eq!(production.rhs.len(), 1);
    }
}
