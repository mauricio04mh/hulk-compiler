use crate::lx::token::{Span, Token, TokenKind};
use crate::spec::rule::{CharClass, NumberKind, Rule, SkipKind, StringEscape};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}",
            self.message, self.span.line, self.span.column
        )
    }
}

pub struct LxParser {
    tokens: Vec<Token>,
    pos: usize,
}

impl LxParser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse_rules(&mut self) -> Result<Vec<Rule>, ParseError> {
        let mut rules = Vec::new();

        self.skip_newlines();

        while !self.is_eof() {
            let rule = self.parse_rule()?;
            rules.push(rule);
            self.skip_newlines();
        }

        Ok(rules)
    }

    fn parse_rule(&mut self) -> Result<Rule, ParseError> {
        match self.peek_kind() {
            Some(TokenKind::KwKeyword) => self.parse_keyword_rule(),
            Some(TokenKind::KwSymbol) => self.parse_symbol_rule(),
            Some(TokenKind::KwIdent) => self.parse_ident_rule(),
            Some(TokenKind::KwNumber) => self.parse_number_rule(),
            Some(TokenKind::KwString) => self.parse_string_rule(),
            Some(TokenKind::KwSkip) => self.parse_skip_rule(),
            Some(other) => Err(ParseError {
                message: format!("Unexpected token at start of rule: {:?}", other),
                span: self.peek().unwrap().span.clone(),
            }),
            None => Err(ParseError {
                message: "Unexpected end of input".to_string(),
                span: self.last_span_or_default(),
            }),
        }
    }

    fn parse_keyword_rule(&mut self) -> Result<Rule, ParseError> {
        self.expect_simple(TokenKind::KwKeyword)?;
        let text = self.expect_ident_value()?;
        let token = self.expect_ident_value()?;
        self.expect_line_end()?;

        Ok(Rule::Keyword { text, token })
    }

    fn parse_symbol_rule(&mut self) -> Result<Rule, ParseError> {
        self.expect_simple(TokenKind::KwSymbol)?;
        let text = self.expect_string_value()?;
        let token = self.expect_ident_value()?;
        self.expect_line_end()?;

        Ok(Rule::Symbol { text, token })
    }

    fn parse_ident_rule(&mut self) -> Result<Rule, ParseError> {
        self.expect_simple(TokenKind::KwIdent)?;
        let token = self.expect_ident_value()?;

        self.expect_simple(TokenKind::KwStart)?;
        self.expect_simple(TokenKind::Eq)?;
        let start = self.parse_char_class_list()?;

        self.expect_simple(TokenKind::KwRest)?;
        self.expect_simple(TokenKind::Eq)?;
        let rest = self.parse_char_class_list()?;

        self.expect_line_end()?;

        Ok(Rule::Ident { token, start, rest })
    }

    fn parse_number_rule(&mut self) -> Result<Rule, ParseError> {
        self.expect_simple(TokenKind::KwNumber)?;
        let token = self.expect_ident_value()?;
        let mut kinds = vec![NumberKind::Int, NumberKind::Float];

        while !self.is_line_end() {
            let key = self.expect_ident_value()?;
            self.expect_simple(TokenKind::Eq)?;

            match key.as_str() {
                "kind" => kinds = self.parse_number_kind_list()?,
                other => {
                    return Err(ParseError {
                        message: format!("Unknown number property '{}'", other),
                        span: self.current_span_or_last(),
                    });
                }
            }
        }

        self.expect_line_end()?;

        Ok(Rule::Number { token, kinds })
    }

    fn parse_string_rule(&mut self) -> Result<Rule, ParseError> {
        self.expect_simple(TokenKind::KwString)?;
        let token = self.expect_ident_value()?;
        let mut quote = '"';
        let mut escapes = vec![
            StringEscape::Quote,
            StringEscape::Backslash,
            StringEscape::Newline,
            StringEscape::Tab,
        ];
        let mut multiline = false;

        while !self.is_line_end() {
            let key = self.expect_ident_value()?;
            self.expect_simple(TokenKind::Eq)?;

            match key.as_str() {
                "quote" => quote = self.expect_quoted_char()?,
                "escapes" => escapes = self.parse_escape_list()?,
                "multiline" => multiline = self.parse_bool_value()?,
                other => {
                    return Err(ParseError {
                        message: format!("Unknown string property '{}'", other),
                        span: self.current_span_or_last(),
                    });
                }
            }
        }

        self.expect_line_end()?;

        Ok(Rule::String {
            token,
            quote,
            escapes,
            multiline,
        })
    }

    fn parse_skip_rule(&mut self) -> Result<Rule, ParseError> {
        self.expect_simple(TokenKind::KwSkip)?;

        match self.peek_kind() {
            Some(TokenKind::KwWhitespace) => {
                self.advance();
                self.expect_line_end()?;
                Ok(Rule::Skip {
                    name: "WHITESPACE".to_string(),
                    kind: SkipKind::Whitespace,
                    prefix: None,
                })
            }
            Some(TokenKind::KwLineComment) => {
                self.advance();
                let prefix = self.expect_string_value()?;
                self.expect_line_end()?;
                Ok(Rule::Skip {
                    name: "COMMENT".to_string(),
                    kind: SkipKind::LineComment,
                    prefix: Some(prefix),
                })
            }
            Some(TokenKind::Ident(_)) => self.parse_named_skip_rule(),
            Some(other) => Err(ParseError {
                message: format!(
                    "Expected skip name or legacy skip kind after 'skip', found {:?}",
                    other
                ),
                span: self.peek().unwrap().span.clone(),
            }),
            None => Err(ParseError {
                message: "Unexpected EOF after 'skip'".to_string(),
                span: self.last_span_or_default(),
            }),
        }
    }

    fn parse_named_skip_rule(&mut self) -> Result<Rule, ParseError> {
        let name = self.expect_ident_value()?;
        let mut kind = None;
        let mut prefix = None;

        while !self.is_line_end() {
            let key = self.expect_ident_value()?;
            self.expect_simple(TokenKind::Eq)?;

            match key.as_str() {
                "kind" => kind = Some(self.parse_skip_kind_value()?),
                "prefix" => prefix = Some(self.expect_string_value()?),
                other => {
                    return Err(ParseError {
                        message: format!("Unknown skip property '{}'", other),
                        span: self.current_span_or_last(),
                    });
                }
            }
        }

        self.expect_line_end()?;

        let Some(kind) = kind else {
            return Err(ParseError {
                message: "Skip rule requires a kind property".to_string(),
                span: self.last_span_or_default(),
            });
        };

        Ok(Rule::Skip { name, kind, prefix })
    }

    fn parse_char_class_list(&mut self) -> Result<Vec<CharClass>, ParseError> {
        let mut classes = vec![self.parse_char_class()?];

        while self.match_simple(TokenKind::Pipe) {
            classes.push(self.parse_char_class()?);
        }

        Ok(classes)
    }

    fn parse_char_class(&mut self) -> Result<CharClass, ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Ident(name),
                ..
            }) if name == "letter" => {
                self.advance();
                Ok(CharClass::Letter)
            }
            Some(Token {
                kind: TokenKind::Ident(name),
                ..
            }) if name == "digit" => {
                self.advance();
                Ok(CharClass::Digit)
            }
            Some(Token {
                kind: TokenKind::Underscore,
                ..
            }) => {
                self.advance();
                Ok(CharClass::Underscore)
            }
            Some(Token {
                kind: TokenKind::Ident(name),
                ..
            }) if name == "_" => {
                self.advance();
                Ok(CharClass::Underscore)
            }
            Some(tok) => Err(ParseError {
                message: format!(
                    "Expected character class ('letter', 'digit' or '_'), found {:?}",
                    tok.kind
                ),
                span: tok.span.clone(),
            }),
            None => Err(ParseError {
                message: "Unexpected EOF while parsing character class".to_string(),
                span: self.last_span_or_default(),
            }),
        }
    }

    fn parse_number_kind_list(&mut self) -> Result<Vec<NumberKind>, ParseError> {
        let mut kinds = vec![self.parse_number_kind()?];

        while self.match_simple(TokenKind::Pipe) {
            kinds.push(self.parse_number_kind()?);
        }

        Ok(kinds)
    }

    fn parse_number_kind(&mut self) -> Result<NumberKind, ParseError> {
        let value = self.expect_word_value()?;

        match value.as_str() {
            "int" => Ok(NumberKind::Int),
            "float" => Ok(NumberKind::Float),
            other => Err(ParseError {
                message: format!("Expected number kind 'int' or 'float', found '{}'", other),
                span: self.current_span_or_last(),
            }),
        }
    }

    fn parse_escape_list(&mut self) -> Result<Vec<StringEscape>, ParseError> {
        let mut escapes = vec![self.parse_escape_value()?];

        while self.match_simple(TokenKind::Pipe) {
            escapes.push(self.parse_escape_value()?);
        }

        Ok(escapes)
    }

    fn parse_escape_value(&mut self) -> Result<StringEscape, ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::EscapeAtom(value),
                ..
            }) => {
                let escape = match value.as_str() {
                    "\\\"" => StringEscape::Quote,
                    "\\\\" => StringEscape::Backslash,
                    "\\n" => StringEscape::Newline,
                    "\\t" => StringEscape::Tab,
                    other => {
                        return Err(ParseError {
                            message: format!("Unsupported escape atom '{}'", other),
                            span: self.current_span_or_last(),
                        });
                    }
                };
                self.advance();
                Ok(escape)
            }
            Some(Token {
                kind: TokenKind::Ident(value),
                ..
            }) => {
                let escape = match value.as_str() {
                    "n" => StringEscape::Newline,
                    "t" => StringEscape::Tab,
                    other => {
                        return Err(ParseError {
                            message: format!("Unsupported escape value '{}'", other),
                            span: self.current_span_or_last(),
                        });
                    }
                };
                self.advance();
                Ok(escape)
            }
            Some(tok) => Err(ParseError {
                message: format!("Expected escape value, found {:?}", tok.kind),
                span: tok.span.clone(),
            }),
            None => Err(ParseError {
                message: "Expected escape value, found EOF".to_string(),
                span: self.last_span_or_default(),
            }),
        }
    }

    fn parse_bool_value(&mut self) -> Result<bool, ParseError> {
        let value = self.expect_word_value()?;

        match value.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            other => Err(ParseError {
                message: format!("Expected boolean 'true' or 'false', found '{}'", other),
                span: self.current_span_or_last(),
            }),
        }
    }

    fn parse_skip_kind_value(&mut self) -> Result<SkipKind, ParseError> {
        let value = self.expect_word_value()?;

        match value.as_str() {
            "whitespace" => Ok(SkipKind::Whitespace),
            "line_comment" => Ok(SkipKind::LineComment),
            other => Err(ParseError {
                message: format!(
                    "Expected skip kind 'whitespace' or 'line_comment', found '{}'",
                    other
                ),
                span: self.current_span_or_last(),
            }),
        }
    }

    fn expect_ident_value(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Ident(value),
                ..
            }) => {
                let out = value.clone();
                self.advance();
                Ok(out)
            }
            Some(tok) => Err(ParseError {
                message: format!("Expected identifier, found {:?}", tok.kind),
                span: tok.span.clone(),
            }),
            None => Err(ParseError {
                message: "Expected identifier, found EOF".to_string(),
                span: self.last_span_or_default(),
            }),
        }
    }

    fn expect_string_value(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::StringLit(value),
                ..
            }) => {
                let out = value.clone();
                self.advance();
                Ok(out)
            }
            Some(tok) => Err(ParseError {
                message: format!("Expected string literal, found {:?}", tok.kind),
                span: tok.span.clone(),
            }),
            None => Err(ParseError {
                message: "Expected string literal, found EOF".to_string(),
                span: self.last_span_or_default(),
            }),
        }
    }

    fn expect_word_value(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Ident(value),
                ..
            }) => {
                let out = value.clone();
                self.advance();
                Ok(out)
            }
            Some(Token {
                kind: TokenKind::KwWhitespace,
                ..
            }) => {
                self.advance();
                Ok("whitespace".to_string())
            }
            Some(Token {
                kind: TokenKind::KwLineComment,
                ..
            }) => {
                self.advance();
                Ok("line_comment".to_string())
            }
            Some(tok) => Err(ParseError {
                message: format!("Expected word value, found {:?}", tok.kind),
                span: tok.span.clone(),
            }),
            None => Err(ParseError {
                message: "Expected word value, found EOF".to_string(),
                span: self.last_span_or_default(),
            }),
        }
    }

    fn expect_quoted_char(&mut self) -> Result<char, ParseError> {
        let value = self.expect_string_value()?;
        let mut chars = value.chars();
        let Some(ch) = chars.next() else {
            return Err(ParseError {
                message: "Expected quote string with exactly one character".to_string(),
                span: self.last_span_or_default(),
            });
        };

        if chars.next().is_some() {
            return Err(ParseError {
                message: "Expected quote string with exactly one character".to_string(),
                span: self.last_span_or_default(),
            });
        }

        Ok(ch)
    }

    fn expect_simple(&mut self, expected: TokenKind) -> Result<(), ParseError> {
        match self.peek() {
            Some(tok) if tok.kind == expected => {
                self.advance();
                Ok(())
            }
            Some(tok) => Err(ParseError {
                message: format!("Expected {:?}, found {:?}", expected, tok.kind),
                span: tok.span.clone(),
            }),
            None => Err(ParseError {
                message: format!("Expected {:?}, found EOF", expected),
                span: self.last_span_or_default(),
            }),
        }
    }

    fn match_simple(&mut self, expected: TokenKind) -> bool {
        if matches!(self.peek(), Some(tok) if tok.kind == expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect_line_end(&mut self) -> Result<(), ParseError> {
        match self.peek_kind() {
            Some(TokenKind::Newline) => {
                self.advance();
                Ok(())
            }
            Some(TokenKind::Eof) => Ok(()),
            Some(other) => Err(ParseError {
                message: format!("Expected end of line, found {:?}", other),
                span: self.peek().unwrap().span.clone(),
            }),
            None => Ok(()),
        }
    }

    fn is_line_end(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::Newline | TokenKind::Eof) | None
        )
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek_kind(), Some(TokenKind::Newline)) {
            self.advance();
        }
    }

    fn is_eof(&self) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::Eof))
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|t| &t.kind)
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn last_span_or_default(&self) -> Span {
        self.tokens.last().map(|t| t.span.clone()).unwrap_or(Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        })
    }

    fn current_span_or_last(&self) -> Span {
        self.peek()
            .map(|t| t.span.clone())
            .unwrap_or_else(|| self.last_span_or_default())
    }
}

// ===== Unit tests start here =====
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lx::lexer::LxLexer;

    fn parse_rules(input: &str) -> Result<Vec<Rule>, ParseError> {
        let mut lexer = LxLexer::new(input);
        let tokens = lexer.lex_all().unwrap();
        let mut parser = LxParser::new(tokens);
        parser.parse_rules()
    }

    #[test]
    fn parses_keyword_rules() {
        // Tests keyword rules map a source word to its token name.
        let rules = parse_rules("keyword let LET\n").unwrap();

        assert_eq!(
            rules,
            vec![Rule::Keyword {
                text: "let".to_string(),
                token: "LET".to_string(),
            }]
        );
    }

    #[test]
    fn parses_symbol_rules() {
        // Tests symbol rules parse quoted symbols and token names.
        let rules = parse_rules("symbol \":=\" ASSIGN\n").unwrap();

        assert_eq!(
            rules,
            vec![Rule::Symbol {
                text: ":=".to_string(),
                token: "ASSIGN".to_string(),
            }]
        );
    }

    #[test]
    fn parses_ident_rules_with_start_and_rest_classes() {
        // Tests ident rules parse start and rest character classes.
        let rules = parse_rules("ident IDENT start=letter rest=letter|digit|_\n").unwrap();

        assert_eq!(
            rules,
            vec![Rule::Ident {
                token: "IDENT".to_string(),
                start: vec![CharClass::Letter],
                rest: vec![CharClass::Letter, CharClass::Digit, CharClass::Underscore,],
            }]
        );
    }

    #[test]
    fn reports_parse_errors_for_incomplete_rules() {
        // Tests incomplete rules produce a parser error.
        let error = parse_rules("keyword let\n").unwrap_err();

        assert_eq!(error.message, "Expected identifier, found Newline");
        assert_eq!(error.span.line, 1);
    }
}
