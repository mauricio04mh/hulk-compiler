use crate::runtime::token::Token;
use crate::spec::lexer_spec::{ExactRule, IdentifierRule, LexerSpec};
use crate::spec::rule::CharClass;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub start: usize,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}",
            self.message, self.line, self.column
        )
    }
}

pub fn lex_hulk(input: &str, spec: &LexerSpec) -> Result<Vec<Token>, LexError> {
    let mut lexer = RuntimeLexer::new(input, spec);
    lexer.lex_all()
}

struct RuntimeLexer<'a> {
    chars: Vec<char>,
    spec: &'a LexerSpec,
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> RuntimeLexer<'a> {
    fn new(input: &str, spec: &'a LexerSpec) -> Self {
        Self {
            chars: input.chars().collect(),
            spec,
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn lex_all(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        loop {
            self.skip_ignored();

            if self.is_eof() {
                tokens.push(Token {
                    kind: "EOF".into(),
                    lexeme: String::new(),
                    start: self.pos,
                    end: self.pos,
                    line: self.line,
                    column: self.column,
                });
                break;
            }

            let start = self.pos;
            let line = self.line;
            let column = self.column;

            if let Some(token) = self.try_match_symbol(start, line, column) {
                tokens.push(token);
                continue;
            }

            if let Some(token) = self.try_match_string(start, line, column)? {
                tokens.push(token);
                continue;
            }

            if let Some(token) = self.try_match_number(start, line, column) {
                tokens.push(token);
                continue;
            }

            if let Some(token) = self.try_match_identifier_or_keyword(start, line, column) {
                tokens.push(token);
                continue;
            }

            let ch = self.peek().unwrap();
            return Err(LexError {
                message: format!("Unexpected character '{}'", ch),
                start,
                line,
                column,
            });
        }

        Ok(tokens)
    }

    fn skip_ignored(&mut self) {
        loop {
            let mut progressed = false;

            if self.spec.skip_whitespace {
                while let Some(ch) = self.peek() {
                    if ch.is_whitespace() {
                        self.bump();
                        progressed = true;
                    } else {
                        break;
                    }
                }
            }

            if let Some(comment_rule) = &self.spec.line_comment {
                if self.starts_with_text(&comment_rule.prefix) {
                    while let Some(ch) = self.peek() {
                        self.bump();
                        progressed = true;
                        if ch == '\n' {
                            break;
                        }
                    }
                }
            }

            if !progressed {
                break;
            }
        }
    }

    fn try_match_symbol(&mut self, start: usize, line: usize, column: usize) -> Option<Token> {
        for rule in self.spec.exact_rules.iter().filter(|r| !r.is_keyword) {
            if self.starts_with_text(&rule.text) {
                self.advance_text(&rule.text);

                return Some(Token {
                    kind: rule.token.clone(),
                    lexeme: rule.text.clone(),
                    start,
                    end: self.pos,
                    line,
                    column,
                });
            }
        }

        None
    }

    fn try_match_string(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Result<Option<Token>, LexError> {
        let Some(rule) = &self.spec.string else {
            return Ok(None);
        };

        if self.peek() != Some(rule.quote) {
            return Ok(None);
        }

        self.bump();
        let mut value = String::new();

        while let Some(ch) = self.peek() {
            match ch {
                ch if ch == rule.quote => {
                    self.bump();
                    return Ok(Some(Token {
                        kind: rule.token.clone(),
                        lexeme: value,
                        start,
                        end: self.pos,
                        line,
                        column,
                    }));
                }
                '\\' => {
                    self.bump();
                    let escaped = match self.peek() {
                        Some(next) if next == rule.quote && rule.allow_quote_escape => {
                            self.bump();
                            rule.quote
                        }
                        Some('\\') if rule.allow_backslash_escape => {
                            self.bump();
                            '\\'
                        }
                        Some('n') if rule.allow_newline_escape => {
                            self.bump();
                            '\n'
                        }
                        Some('t') if rule.allow_tab_escape => {
                            self.bump();
                            '\t'
                        }
                        Some(other) => {
                            return Err(LexError {
                                message: format!("Invalid escape sequence \\{}", other),
                                start,
                                line,
                                column,
                            });
                        }
                        None => {
                            return Err(LexError {
                                message: "Unterminated escape sequence".to_string(),
                                start,
                                line,
                                column,
                            });
                        }
                    };
                    value.push(escaped);
                }
                '\n' if !rule.multiline => {
                    return Err(LexError {
                        message: "Unterminated string literal".to_string(),
                        start,
                        line,
                        column,
                    });
                }
                other => {
                    value.push(other);
                    self.bump();
                }
            }
        }

        Err(LexError {
            message: "Unterminated string literal".to_string(),
            start,
            line,
            column,
        })
    }

    fn try_match_number(&mut self, start: usize, line: usize, column: usize) -> Option<Token> {
        let Some(rule) = &self.spec.number else {
            return None;
        };

        if !matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
            return None;
        }

        let checkpoint = (self.pos, self.line, self.column);
        let mut lexeme = String::new();

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                lexeme.push(ch);
                self.bump();
            } else {
                break;
            }
        }

        let mut has_fraction = false;
        if self.peek() == Some('.') && matches!(self.peek_n(1), Some(ch) if ch.is_ascii_digit()) {
            has_fraction = true;
            lexeme.push('.');
            self.bump();

            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() {
                    lexeme.push(ch);
                    self.bump();
                } else {
                    break;
                }
            }
        }

        let accepted = if has_fraction {
            rule.allow_float
        } else {
            rule.allow_int
        };

        if !accepted {
            (self.pos, self.line, self.column) = checkpoint;
            return None;
        }

        Some(Token {
            kind: rule.token.clone(),
            lexeme,
            start,
            end: self.pos,
            line,
            column,
        })
    }

    fn try_match_identifier_or_keyword(
        &mut self,
        start: usize,
        line: usize,
        column: usize,
    ) -> Option<Token> {
        let rule = self.spec.identifier.as_ref()?;

        let lexeme = self.read_identifier(rule)?;
        let kind = self
            .lookup_keyword(&lexeme)
            .unwrap_or_else(|| rule.token.clone());

        Some(Token {
            kind,
            lexeme,
            start,
            end: self.pos,
            line,
            column,
        })
    }

    fn read_identifier(&mut self, rule: &IdentifierRule) -> Option<String> {
        if self.is_eof() {
            return None;
        }

        let first = self.peek()?;
        if !matches_classes(first, &rule.start) {
            return None;
        }

        let mut out = String::new();
        out.push(first);
        self.bump();

        while let Some(ch) = self.peek() {
            if matches_classes(ch, &rule.rest) {
                out.push(ch);
                self.bump();
            } else {
                break;
            }
        }

        Some(out)
    }

    fn lookup_keyword(&self, lexeme: &str) -> Option<String> {
        self.spec
            .exact_rules
            .iter()
            .find(|r| r.is_keyword && r.text == lexeme)
            .map(|r| r.token.clone())
    }

    fn starts_with_text(&self, text: &str) -> bool {
        for (i, expected) in text.chars().enumerate() {
            if self.peek_n(i) != Some(expected) {
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

    fn is_eof(&self) -> bool {
        self.pos >= self.chars.len()
    }
}

fn matches_classes(ch: char, classes: &[CharClass]) -> bool {
    classes.iter().any(|class| match class {
        CharClass::Letter => ch.is_ascii_alphabetic(),
        CharClass::Digit => ch.is_ascii_digit(),
        CharClass::Underscore => ch == '_',
    })
}

#[allow(dead_code)]
fn _debug_exact_rules(rules: &[ExactRule]) {
    for rule in rules {
        println!("{:?}", rule);
    }
}

// ===== Unit tests start here =====
#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::normalize::normalize_spec;
    use crate::spec::rule::{NumberKind, Rule, SkipKind, StringEscape};

    fn build_spec(rules: Vec<Rule>) -> LexerSpec {
        normalize_spec(&rules).unwrap()
    }

    fn default_spec() -> LexerSpec {
        build_spec(vec![
            Rule::Keyword {
                text: "let".to_string(),
                token: "LET".to_string(),
            },
            Rule::Ident {
                token: "IDENT".to_string(),
                start: vec![CharClass::Letter],
                rest: vec![CharClass::Letter, CharClass::Digit, CharClass::Underscore],
            },
            Rule::Number {
                token: "NUMBER".to_string(),
                kinds: vec![NumberKind::Int, NumberKind::Float],
            },
            Rule::String {
                token: "STRING".to_string(),
                quote: '"',
                escapes: vec![
                    StringEscape::Quote,
                    StringEscape::Backslash,
                    StringEscape::Newline,
                    StringEscape::Tab,
                ],
                multiline: false,
            },
            Rule::Skip {
                name: "WS".to_string(),
                kind: SkipKind::Whitespace,
                prefix: None,
            },
            Rule::Skip {
                name: "COMMENT".to_string(),
                kind: SkipKind::LineComment,
                prefix: Some("#".to_string()),
            },
        ])
    }

    #[test]
    fn recognizes_keywords_from_exact_rules() {
        // Tests configured keywords are emitted with their keyword token kind.
        let spec = default_spec();
        let tokens = lex_hulk("let", &spec).unwrap();

        assert_eq!(tokens[0].kind, "LET");
        assert_eq!(tokens[0].lexeme, "let");
        assert_eq!(tokens[1].kind, "EOF");
    }

    #[test]
    fn distinguishes_keywords_from_identifiers() {
        // Tests identifiers only become keywords on an exact text match.
        let spec = default_spec();
        let tokens = lex_hulk("let let_name", &spec).unwrap();

        assert_eq!(tokens[0].kind, "LET");
        assert_eq!(tokens[1].kind, "IDENT");
        assert_eq!(tokens[1].lexeme, "let_name");
    }

    #[test]
    fn recognizes_integer_and_float_numbers() {
        // Tests numeric rules match both integer and float lexemes.
        let spec = default_spec();
        let tokens = lex_hulk("42 3.14", &spec).unwrap();

        assert_eq!(tokens[0].kind, "NUMBER");
        assert_eq!(tokens[0].lexeme, "42");
        assert_eq!(tokens[1].kind, "NUMBER");
        assert_eq!(tokens[1].lexeme, "3.14");
        assert_eq!(tokens[2].kind, "EOF");
    }

    #[test]
    fn ignores_line_comments() {
        // Tests configured line comments are skipped by the runtime lexer.
        let spec = default_spec();
        let tokens = lex_hulk("let # ignored\nname", &spec).unwrap();

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, "LET");
        assert_eq!(tokens[1].kind, "IDENT");
        assert_eq!(tokens[1].lexeme, "name");
        assert_eq!(tokens[2].kind, "EOF");
    }

    #[test]
    fn reports_runtime_lexical_errors() {
        // Tests unexpected runtime input produces a lexical error.
        let spec = default_spec();
        let error = lex_hulk("@", &spec).unwrap_err();

        assert_eq!(error.message, "Unexpected character '@'");
        assert_eq!(error.line, 1);
        assert_eq!(error.column, 1);
    }
}
