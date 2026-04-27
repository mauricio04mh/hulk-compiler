#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    // Reserved words for the .lx language
    KwKeyword,
    KwSymbol,
    KwIdent,
    KwNumber,
    KwString,
    KwSkip,
    KwWhitespace,
    KwLineComment,
    KwStart,
    KwRest,

    // Data
    Ident(String),
    StringLit(String),
    EscapeAtom(String),

    // Structural symbols of .lx
    Eq,
    Pipe,
    Underscore,

    // Line control / end of file
    Newline,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
