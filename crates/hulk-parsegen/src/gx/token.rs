#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GxTokenKind {
    StartDirective,
    Ident(String),
    Arrow,
    Pipe,
    Semicolon,
    Epsilon,
    EndOfFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GxToken {
    pub kind: GxTokenKind,
    pub line: usize,
    pub column: usize,
}

impl GxToken {
    pub fn new(kind: GxTokenKind, line: usize, column: usize) -> Self {
        Self { kind, line, column }
    }
}
