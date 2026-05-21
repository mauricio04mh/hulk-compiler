use crate::grammar::Grammar;
use crate::gx::error::GxError;
use crate::gx::lexer::lex_gx;
use crate::gx::token::{GxToken, GxTokenKind};
use crate::production::Production;
use crate::symbol::Symbol;

pub fn parse_gx(source: &str) -> Result<Grammar, GxError> {
    let tokens = lex_gx(source)?;
    let mut parser = GxParser::new(tokens);
    parser.parse_grammar()
}

struct GxParser {
    tokens: Vec<GxToken>,
    pos: usize,
}

impl GxParser {
    fn new(tokens: Vec<GxToken>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse_grammar(&mut self) -> Result<Grammar, GxError> {
        self.expect_simple(GxTokenKind::StartDirective)?;
        let start = self.expect_ident()?;
        let mut productions = Vec::new();

        while !self.is_eof() {
            let lhs = self.expect_ident()?;
            self.expect_simple(GxTokenKind::Arrow)?;

            loop {
                let rhs = self.parse_rhs_alternative()?;
                productions.push(Production {
                    lhs: lhs.clone(),
                    rhs,
                });

                if self.match_simple(GxTokenKind::Pipe) {
                    continue;
                }

                self.expect_simple(GxTokenKind::Semicolon)?;
                break;
            }
        }

        Ok(Grammar { start, productions })
    }

    fn parse_rhs_alternative(&mut self) -> Result<Vec<Symbol>, GxError> {
        let mut rhs = Vec::new();

        if self.match_simple(GxTokenKind::Epsilon) {
            rhs.push(Symbol::Epsilon);
            return Ok(rhs);
        }

        while !self.check_simple(&GxTokenKind::Pipe)
            && !self.check_simple(&GxTokenKind::Semicolon)
            && !self.check_simple(&GxTokenKind::EndOfFile)
        {
            let ident = self.expect_ident()?;
            rhs.push(classify_symbol(&ident));
        }

        if rhs.is_empty() {
            let tok = self.current();
            return Err(GxError {
                message: "Expected symbols in production RHS".to_string(),
                line: tok.line,
                column: tok.column,
            });
        }

        Ok(rhs)
    }

    fn expect_ident(&mut self) -> Result<String, GxError> {
        let token = self.current().clone();
        match token.kind {
            GxTokenKind::Ident(value) => {
                self.pos += 1;
                Ok(value)
            }
            _ => Err(GxError {
                message: "Expected identifier".to_string(),
                line: token.line,
                column: token.column,
            }),
        }
    }

    fn expect_simple(&mut self, expected: GxTokenKind) -> Result<(), GxError> {
        let token = self.current().clone();
        if token.kind == expected {
            self.pos += 1;
            Ok(())
        } else {
            Err(GxError {
                message: format!("Expected {:?}", expected),
                line: token.line,
                column: token.column,
            })
        }
    }

    fn match_simple(&mut self, expected: GxTokenKind) -> bool {
        if self.check_simple(&expected) {
            self.pos += 1;
            return true;
        }
        false
    }

    fn check_simple(&self, expected: &GxTokenKind) -> bool {
        self.current().kind == *expected
    }

    fn is_eof(&self) -> bool {
        self.check_simple(&GxTokenKind::EndOfFile)
    }

    fn current(&self) -> &GxToken {
        if self.pos < self.tokens.len() {
            &self.tokens[self.pos]
        } else {
            self.tokens
                .last()
                .expect("lexer should always emit EndOfFile")
        }
    }
}

fn classify_symbol(name: &str) -> Symbol {
    if name == "EOF" {
        Symbol::Eof
    } else if is_all_uppercase(name) {
        Symbol::Terminal(name.to_string())
    } else {
        Symbol::NonTerminal(name.to_string())
    }
}

fn is_all_uppercase(name: &str) -> bool {
    name.chars().all(|ch| !ch.is_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_parses_minimal_grammar() {
        let source = "%start Program
Program -> Expr EOF ;
Expr -> NUMBER ;
";

        let grammar = parse_gx(source).unwrap();
        assert_eq!(grammar.start, "Program");
        assert_eq!(grammar.productions.len(), 2);
    }

    #[test]
    fn parser_expands_alternatives() {
        let source = "%start Expr
Expr -> NUMBER | IDENT ;
";

        let grammar = parse_gx(source).unwrap();
        assert_eq!(grammar.productions.len(), 2);
        assert_eq!(grammar.productions[0].lhs, "Expr");
        assert_eq!(grammar.productions[1].lhs, "Expr");
    }

    #[test]
    fn parser_supports_epsilon_rhs() {
        let source = "%start ArgList
ArgList -> epsilon ;
";
        let grammar = parse_gx(source).unwrap();
        assert_eq!(grammar.productions.len(), 1);
        assert_eq!(grammar.productions[0].rhs, vec![Symbol::Epsilon]);
    }
}
