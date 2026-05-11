use crate::gx::error::GxError;
use crate::gx::token::{GxToken, GxTokenKind};

pub fn lex_gx(source: &str) -> Result<Vec<GxToken>, GxError> {
    let mut lexer = GxLexer::new(source);
    lexer.lex_all()
}

struct GxLexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl GxLexer {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn lex_all(&mut self) -> Result<Vec<GxToken>, GxError> {
        let mut tokens = Vec::new();

        loop {
            self.skip_ignored();
            let start_line = self.line;
            let start_col = self.column;

            let Some(ch) = self.peek() else {
                tokens.push(GxToken::new(GxTokenKind::EndOfFile, start_line, start_col));
                break;
            };

            if ch == '%' {
                if self.starts_with("%start") {
                    self.advance_text("%start");
                    tokens.push(GxToken::new(
                        GxTokenKind::StartDirective,
                        start_line,
                        start_col,
                    ));
                    continue;
                }

                return Err(GxError {
                    message: "Unknown directive".to_string(),
                    line: start_line,
                    column: start_col,
                });
            }

            if ch == '-' && self.peek_n(1) == Some('>') {
                self.bump();
                self.bump();
                tokens.push(GxToken::new(GxTokenKind::Arrow, start_line, start_col));
                continue;
            }

            if ch == '|' {
                self.bump();
                tokens.push(GxToken::new(GxTokenKind::Pipe, start_line, start_col));
                continue;
            }

            if ch == ';' {
                self.bump();
                tokens.push(GxToken::new(GxTokenKind::Semicolon, start_line, start_col));
                continue;
            }

            if ch == 'ε' {
                self.bump();
                tokens.push(GxToken::new(GxTokenKind::Epsilon, start_line, start_col));
                continue;
            }

            if is_ident_start(ch) {
                let ident = self.lex_ident();
                if ident == "epsilon" {
                    tokens.push(GxToken::new(GxTokenKind::Epsilon, start_line, start_col));
                } else {
                    tokens.push(GxToken::new(
                        GxTokenKind::Ident(ident),
                        start_line,
                        start_col,
                    ));
                }
                continue;
            }

            return Err(GxError {
                message: format!("Unexpected character '{}'", ch),
                line: start_line,
                column: start_col,
            });
        }

        Ok(tokens)
    }

    fn skip_ignored(&mut self) {
        loop {
            let mut progressed = false;

            while matches!(self.peek(), Some(ch) if ch.is_whitespace()) {
                self.bump();
                progressed = true;
            }

            if self.starts_with("//") {
                progressed = true;
                while let Some(ch) = self.peek() {
                    self.bump();
                    if ch == '\n' {
                        break;
                    }
                }
            }

            if !progressed {
                break;
            }
        }
    }

    fn lex_ident(&mut self) -> String {
        let mut out = String::new();
        while let Some(ch) = self.peek() {
            if is_ident_continue(ch) {
                out.push(ch);
                self.bump();
            } else {
                break;
            }
        }
        out
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_n(&self, n: usize) -> Option<char> {
        self.chars.get(self.pos + n).copied()
    }

    fn starts_with(&self, text: &str) -> bool {
        for (offset, expected) in text.chars().enumerate() {
            if self.peek_n(offset) != Some(expected) {
                return false;
            }
        }
        true
    }

    fn advance_text(&mut self, text: &str) {
        for _ in text.chars() {
            self.bump();
        }
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_ident_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexer_recognizes_core_tokens() {
        let source = "%start Program\nExpr -> NUMBER | IDENT ;";
        let tokens = lex_gx(source).unwrap();
        let kinds = tokens.into_iter().map(|t| t.kind).collect::<Vec<_>>();

        assert_eq!(
            kinds,
            vec![
                GxTokenKind::StartDirective,
                GxTokenKind::Ident("Program".to_string()),
                GxTokenKind::Ident("Expr".to_string()),
                GxTokenKind::Arrow,
                GxTokenKind::Ident("NUMBER".to_string()),
                GxTokenKind::Pipe,
                GxTokenKind::Ident("IDENT".to_string()),
                GxTokenKind::Semicolon,
                GxTokenKind::EndOfFile,
            ]
        );
    }
}
