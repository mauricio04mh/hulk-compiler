#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseToken {
    pub kind: String,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

impl ParseToken {
    pub fn new(
        kind: impl Into<String>,
        lexeme: impl Into<String>,
        line: usize,
        column: usize,
    ) -> Self {
        Self {
            kind: kind.into(),
            lexeme: lexeme.into(),
            line,
            column,
        }
    }
}
