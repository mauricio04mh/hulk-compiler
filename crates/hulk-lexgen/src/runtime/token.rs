#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: String,
    pub lexeme: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}
