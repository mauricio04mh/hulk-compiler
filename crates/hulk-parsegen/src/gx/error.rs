use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GxError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for GxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}",
            self.message, self.line, self.column
        )
    }
}
