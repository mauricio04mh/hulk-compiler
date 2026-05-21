use hulk_parsegen::runtime::error::ParseError as ParsegenError;
use std::fmt;
use thiserror::Error;

/// Wraps a list of parse errors and implements Display so thiserror can use it.
#[derive(Debug, Clone)]
pub struct ParseErrorList(pub Vec<ParsegenError>);

impl fmt::Display for ParseErrorList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{}", e.pretty())?;
        }
        Ok(())
    }
}

impl ParseErrorList {
    pub fn errors(&self) -> &[ParsegenError] {
        &self.0
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum AstError {
    #[error("Unexpected node '{name}'{location}")]
    UnexpectedNode { name: String, location: String },

    #[error("Unexpected token '{kind}'{location}")]
    UnexpectedToken { kind: String, location: String },

    #[error("Missing child in node '{node}'{location}")]
    MissingChild { node: String, location: String },

    #[error("Invalid number literal '{literal}'{location}")]
    InvalidNumberLiteral { literal: String, location: String },

    #[error("Unsupported construct: {message}{location}")]
    UnsupportedConstruct { message: String, location: String },
}

impl AstError {
    pub fn at(line: usize, column: usize) -> String {
        format!(" at line {}, column {}", line, column)
    }

    pub fn no_location() -> String {
        String::new()
    }
}

#[derive(Debug, Error)]
pub enum FrontendError {
    #[error("I/O error: {0}")]
    Io(String),

    #[error("gx parse error: {0}")]
    GxParse(String),

    #[error("grammar normalize error: {0}")]
    GrammarNormalize(String),

    #[error("ll1 table error: {0}")]
    Ll1Table(String),

    #[error("lx lex error: {0}")]
    LxLex(String),

    #[error("lx parse error: {0}")]
    LxParse(String),

    #[error("lx normalize error: {0}")]
    LxNormalize(String),

    #[error("source lex error: {0}")]
    SourceLex(String),

    #[error("parse errors:\n{0}")]
    ParseErrors(ParseErrorList),

    #[error(transparent)]
    Ast(#[from] AstError),
}
