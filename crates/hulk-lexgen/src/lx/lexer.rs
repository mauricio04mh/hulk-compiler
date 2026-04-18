use crate::lx::token::{Span, Token, TokenKind};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}",
            self.message, self.span.line, self.span.column
        )
    }
}

pub struct LxLexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl LxLexer {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn lex_all(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        loop {
            let token = self.next_token()?;
            let is_eof = matches!(token.kind, TokenKind::Eof);
            tokens.push(token);
            if is_eof {
                break;
            }
        }

        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        loop {
            self.skip_horizontal_whitespace();

            let start_pos = self.pos;
            let start_line = self.line;
            let start_col = self.column;

            let Some(ch) = self.peek() else {
                return Ok(Token {
                    kind: TokenKind::Eof,
                    span: self.make_span(start_pos, start_line, start_col),
                });
            };

            // .lx comments start with '#'
            if ch == '#' {
                self.skip_hash_comment();
                continue;
            }

            // Newlines are tokenized
            if ch == '\n' {
                self.bump();
                return Ok(Token {
                    kind: TokenKind::Newline,
                    span: self.make_span(start_pos, start_line, start_col),
                });
            }

            // String literal
            if ch == '"' {
                let value = self.lex_string()?;
                return Ok(Token {
                    kind: TokenKind::StringLit(value),
                    span: self.make_span(start_pos, start_line, start_col),
                });
            }

            // Escape atoms used in properties like: escapes=\"|\\|n|t
            if ch == '\\' {
                let value = self.lex_escape_atom()?;
                return Ok(Token {
                    kind: TokenKind::EscapeAtom(value),
                    span: self.make_span(start_pos, start_line, start_col),
                });
            }

            if ch == '_' && !matches!(self.peek_n(1), Some(next) if is_word_continue(next)) {
                self.bump();
                return Ok(Token {
                    kind: TokenKind::Underscore,
                    span: self.make_span(start_pos, start_line, start_col),
                });
            }

            // Identifiers / keywords of .lx
            if is_word_start(ch) {
                let word = self.lex_word();
                let kind = match word.as_str() {
                    "keyword" => TokenKind::KwKeyword,
                    "symbol" => TokenKind::KwSymbol,
                    "ident" => TokenKind::KwIdent,
                    "number" => TokenKind::KwNumber,
                    "string" => TokenKind::KwString,
                    "skip" => TokenKind::KwSkip,
                    "whitespace" => TokenKind::KwWhitespace,
                    "line_comment" => TokenKind::KwLineComment,
                    "start" => TokenKind::KwStart,
                    "rest" => TokenKind::KwRest,
                    _ => TokenKind::Ident(word),
                };

                return Ok(Token {
                    kind,
                    span: self.make_span(start_pos, start_line, start_col),
                });
            }

            // Standalone symbols of .lx
            match ch {
                '=' => {
                    self.bump();
                    return Ok(Token {
                        kind: TokenKind::Eq,
                        span: self.make_span(start_pos, start_line, start_col),
                    });
                }
                '|' => {
                    self.bump();
                    return Ok(Token {
                        kind: TokenKind::Pipe,
                        span: self.make_span(start_pos, start_line, start_col),
                    });
                }
                '_' => {
                    self.bump();
                    return Ok(Token {
                        kind: TokenKind::Underscore,
                        span: self.make_span(start_pos, start_line, start_col),
                    });
                }
                _ => {
                    return Err(LexError {
                        message: format!("Unexpected character '{}'", ch),
                        span: self.make_span(start_pos, start_line, start_col),
                    });
                }
            }
        }
    }

    fn skip_horizontal_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.bump();
            } else {
                break;
            }
        }
    }

    fn skip_hash_comment(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                break;
            }
            self.bump();
        }
    }

    fn lex_word(&mut self) -> String {
        let mut out = String::new();

        while let Some(ch) = self.peek() {
            if is_word_continue(ch) {
                out.push(ch);
                self.bump();
            } else {
                break;
            }
        }

        out
    }

    fn lex_escape_atom(&mut self) -> Result<String, LexError> {
        let start_pos = self.pos;
        let start_line = self.line;
        let start_col = self.column;

        self.bump();

        let Some(ch) = self.peek() else {
            return Err(LexError {
                message: "Unterminated escape atom".to_string(),
                span: self.make_span(start_pos, start_line, start_col),
            });
        };

        match ch {
            '"' | '\\' | 'n' | 't' => {
                self.bump();
                Ok(format!("\\{}", ch))
            }
            other => Err(LexError {
                message: format!("Invalid escape atom \\{}", other),
                span: self.make_span(start_pos, start_line, start_col),
            }),
        }
    }

    fn lex_string(&mut self) -> Result<String, LexError> {
        let start_pos = self.pos;
        let start_line = self.line;
        let start_col = self.column;

        // Consume the opening quote
        self.bump();

        let mut out = String::new();

        while let Some(ch) = self.peek() {
            match ch {
                '"' => {
                    self.bump();
                    return Ok(out);
                }
                '\\' => {
                    self.bump();
                    let escaped = match self.peek() {
                        Some('"') => {
                            self.bump();
                            '"'
                        }
                        Some('\\') => {
                            self.bump();
                            '\\'
                        }
                        Some('n') => {
                            self.bump();
                            '\n'
                        }
                        Some('t') => {
                            self.bump();
                            '\t'
                        }
                        Some(other) => {
                            return Err(LexError {
                                message: format!("Invalid escape sequence \\{}", other),
                                span: self.make_span(start_pos, start_line, start_col),
                            });
                        }
                        None => {
                            return Err(LexError {
                                message: "Unterminated escape sequence".to_string(),
                                span: self.make_span(start_pos, start_line, start_col),
                            });
                        }
                    };
                    out.push(escaped);
                }
                '\n' => {
                    return Err(LexError {
                        message: "Unterminated string literal".to_string(),
                        span: self.make_span(start_pos, start_line, start_col),
                    });
                }
                other => {
                    out.push(other);
                    self.bump();
                }
            }
        }

        Err(LexError {
            message: "Unterminated string literal".to_string(),
            span: self.make_span(start_pos, start_line, start_col),
        })
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_n(&self, n: usize) -> Option<char> {
        self.chars.get(self.pos + n).copied()
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

    fn make_span(&self, start: usize, line: usize, column: usize) -> Span {
        Span {
            start,
            end: self.pos,
            line,
            column,
        }
    }
}

fn is_word_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_word_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

// ===== Unit tests start here =====
#[cfg(test)]
mod tests {
    use super::*;

    fn lex(input: &str) -> Result<Vec<Token>, LexError> {
        let mut lexer = LxLexer::new(input);
        lexer.lex_all()
    }

    #[test]
    fn lexes_lx_keywords() {
        // Tests .lx reserved words are tokenized with dedicated keyword kinds.
        let tokens =
            lex("keyword symbol ident number string skip whitespace line_comment start rest")
                .unwrap();

        let kinds = tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>();

        assert_eq!(
            kinds,
            vec![
                TokenKind::KwKeyword,
                TokenKind::KwSymbol,
                TokenKind::KwIdent,
                TokenKind::KwNumber,
                TokenKind::KwString,
                TokenKind::KwSkip,
                TokenKind::KwWhitespace,
                TokenKind::KwLineComment,
                TokenKind::KwStart,
                TokenKind::KwRest,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lexes_string_literals_with_supported_escapes() {
        // Tests string literals decode the supported escape sequences.
        let tokens = lex(r#""hi\n\t\"\\end""#).unwrap();

        assert_eq!(
            tokens[0].kind,
            TokenKind::StringLit("hi\n\t\"\\end".to_string())
        );
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_eq_pipe_and_underscore_symbols() {
        // Tests standalone .lx symbols are emitted as their dedicated tokens.
        let tokens = lex("=|_").unwrap();
        let kinds = tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Eq,
                TokenKind::Pipe,
                TokenKind::Underscore,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn skips_hash_comments_and_keeps_newlines() {
        // Tests hash comments are ignored while newlines remain tokenized.
        let tokens = lex("keyword foo BAR # ignored comment\n=\n").unwrap();
        let kinds = tokens
            .into_iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>();

        assert_eq!(
            kinds,
            vec![
                TokenKind::KwKeyword,
                TokenKind::Ident("foo".to_string()),
                TokenKind::Ident("BAR".to_string()),
                TokenKind::Newline,
                TokenKind::Eq,
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn reports_unexpected_character_errors() {
        // Tests invalid characters produce a lexical error.
        let error = lex("@").unwrap_err();

        assert_eq!(error.message, "Unexpected character '@'");
        assert_eq!(error.span.line, 1);
        assert_eq!(error.span.column, 1);
    }

    #[test]
    fn reports_invalid_string_escape_errors() {
        // Tests invalid string escapes produce a lexical error.
        let error = lex(r#""bad\q""#).unwrap_err();

        assert_eq!(error.message, "Invalid escape sequence \\q");
        assert_eq!(error.span.line, 1);
        assert_eq!(error.span.column, 1);
    }
}
