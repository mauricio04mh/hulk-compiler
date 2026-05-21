use crate::production::Production;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrammarError {
    EmptyGrammar,
    UndefinedStartSymbol(String),
    UndefinedNonTerminal(String),
    InvalidEpsilonUsage {
        lhs: String,
    },
    Ll1Conflict {
        non_terminal: String,
        terminal: String,
        existing: Production,
        incoming: Production,
    },
}

impl fmt::Display for GrammarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GrammarError::EmptyGrammar => write!(f, "Grammar cannot be empty"),
            GrammarError::UndefinedStartSymbol(start) => {
                write!(f, "Undefined start symbol '{}'", start)
            }
            GrammarError::UndefinedNonTerminal(non_terminal) => {
                write!(f, "Undefined non-terminal '{}'", non_terminal)
            }
            GrammarError::InvalidEpsilonUsage { lhs } => {
                write!(f, "Invalid epsilon usage in production '{}'", lhs)
            }
            GrammarError::Ll1Conflict {
                non_terminal,
                terminal,
                ..
            } => write!(
                f,
                "LL(1) conflict for non-terminal '{}' with lookahead '{}'",
                non_terminal, terminal
            ),
        }
    }
}
