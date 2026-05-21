use crate::runtime::token::ParseToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CstNode {
    Node {
        name: String,
        children: Vec<CstNode>,
    },
    Token {
        kind: String,
        lexeme: String,
        line: usize,
        column: usize,
    },
    /// Placeholder inserted during error recovery when a sub-tree failed to parse.
    Error {
        message: String,
        line: usize,
        column: usize,
    },
}

impl CstNode {
    pub fn node(name: impl Into<String>, children: Vec<CstNode>) -> Self {
        Self::Node {
            name: name.into(),
            children,
        }
    }

    pub fn token(token: &ParseToken) -> Self {
        Self::Token {
            kind: token.kind.clone(),
            lexeme: token.lexeme.clone(),
            line: token.line,
            column: token.column,
        }
    }

    pub fn error(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self::Error {
            message: message.into(),
            line,
            column,
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }
}
