use crate::runtime::cst::CstNode;
use crate::runtime::error::ParseError;
use crate::runtime::token::ParseToken;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Associativity {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatorInfo {
    pub precedence: u8,
    pub associativity: Associativity,
}

#[derive(Debug, Clone)]
pub struct PrattConfig {
    pub binary_ops: HashMap<String, OperatorInfo>,
    pub unary_prefix_ops: HashSet<String>,
    pub primary_tokens: HashSet<String>,
    pub lparen: String,
    pub rparen: String,
}

pub struct PrattParser {
    config: PrattConfig,
}

impl PrattParser {
    pub fn new(config: PrattConfig) -> Self {
        Self { config }
    }

    // If the input ends with EOF, this parser consumes the expression and then expects EOF.
    // If EOF is not present, it accepts consuming the full token slice.
    pub fn parse_expression(&self, tokens: &[ParseToken]) -> Result<CstNode, ParseError> {
        if tokens.is_empty() {
            return Err(ParseError {
                message: "Input token stream is empty".to_string(),
                line: 1,
                column: 1,
                found: None,
                expected: vec![],
            });
        }

        let mut state = PrattState::new(tokens, &self.config);
        let expr = state.parse_expression_bp(0)?;

        if let Some(tok) = state.current() {
            if tok.kind == "EOF" {
                state.advance();
            } else {
                return Err(ParseError {
                    message: "Unexpected trailing tokens after expression".to_string(),
                    line: tok.line,
                    column: tok.column,
                    found: Some(tok.kind.clone()),
                    expected: vec!["EOF".to_string()],
                });
            }
        }

        if state.current().is_some() {
            let tok = state.current().expect("checked above");
            return Err(ParseError {
                message: "Unexpected tokens after EOF".to_string(),
                line: tok.line,
                column: tok.column,
                found: Some(tok.kind.clone()),
                expected: vec![],
            });
        }

        Ok(expr)
    }
}

struct PrattState<'a> {
    tokens: &'a [ParseToken],
    pos: usize,
    config: &'a PrattConfig,
}

impl<'a> PrattState<'a> {
    fn new(tokens: &'a [ParseToken], config: &'a PrattConfig) -> Self {
        Self {
            tokens,
            pos: 0,
            config,
        }
    }

    fn parse_expression_bp(&mut self, min_bp: u8) -> Result<CstNode, ParseError> {
        let mut left = self.parse_prefix()?;

        loop {
            let Some(op) = self.current() else {
                break;
            };

            if op.kind == "EOF" || op.kind == self.config.rparen {
                break;
            }

            let Some(op_info) = self.config.binary_ops.get(&op.kind) else {
                break;
            };

            if op_info.precedence < min_bp {
                break;
            }

            let op_token = op.clone();
            self.advance();

            let rhs_bp = match op_info.associativity {
                Associativity::Left => op_info.precedence + 1,
                Associativity::Right => op_info.precedence,
            };

            let right = self.parse_expression_bp(rhs_bp)?;
            left = CstNode::node("BinaryExpr", vec![CstNode::token(&op_token), left, right]);
        }

        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<CstNode, ParseError> {
        let tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Unexpected end of input while parsing expression".to_string(),
            line: 1,
            column: 1,
            found: None,
            expected: vec![],
        })?;

        if self.config.unary_prefix_ops.contains(&tok.kind) {
            self.advance();
            let expr = self.parse_expression_bp(9)?;
            return Ok(CstNode::node("UnaryExpr", vec![CstNode::token(&tok), expr]));
        }

        if tok.kind == self.config.lparen {
            self.advance();
            let expr = self.parse_expression_bp(0)?;
            let Some(close) = self.current() else {
                return Err(ParseError {
                    message: "Unclosed parenthesis in grouped expression".to_string(),
                    line: tok.line,
                    column: tok.column,
                    found: None,
                    expected: vec![self.config.rparen.clone()],
                });
            };

            if close.kind != self.config.rparen {
                return Err(ParseError {
                    message: "Expected closing parenthesis".to_string(),
                    line: close.line,
                    column: close.column,
                    found: Some(close.kind.clone()),
                    expected: vec![self.config.rparen.clone()],
                });
            }

            self.advance();
            return Ok(CstNode::node("GroupExpr", vec![expr]));
        }

        if self.config.primary_tokens.contains(&tok.kind) {
            self.advance();
            return Ok(CstNode::token(&tok));
        }

        Err(ParseError {
            message: "Expected expression".to_string(),
            line: tok.line,
            column: tok.column,
            found: Some(tok.kind.clone()),
            expected: self.expected_prefix_kinds(),
        })
    }

    fn expected_prefix_kinds(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.extend(self.config.primary_tokens.iter().cloned());
        out.extend(self.config.unary_prefix_ops.iter().cloned());
        out.push(self.config.lparen.clone());
        out.sort();
        out.dedup();
        out
    }

    fn current(&self) -> Option<&'a ParseToken> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) {
        self.pos += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::token::ParseToken;

    fn tok(kind: &str, lexeme: &str) -> ParseToken {
        ParseToken::new(kind, lexeme, 1, 1)
    }

    fn hulk_like_pratt() -> PrattParser {
        let mut binary_ops = HashMap::new();
        binary_ops.insert(
            "OR".to_string(),
            OperatorInfo {
                precedence: 1,
                associativity: Associativity::Left,
            },
        );
        binary_ops.insert(
            "AND".to_string(),
            OperatorInfo {
                precedence: 2,
                associativity: Associativity::Left,
            },
        );
        binary_ops.insert(
            "EQ".to_string(),
            OperatorInfo {
                precedence: 3,
                associativity: Associativity::Left,
            },
        );
        binary_ops.insert(
            "NEQ".to_string(),
            OperatorInfo {
                precedence: 3,
                associativity: Associativity::Left,
            },
        );
        for op in ["LT", "LE", "GT", "GE"] {
            binary_ops.insert(
                op.to_string(),
                OperatorInfo {
                    precedence: 4,
                    associativity: Associativity::Left,
                },
            );
        }
        for op in ["AT", "ATAT"] {
            binary_ops.insert(
                op.to_string(),
                OperatorInfo {
                    precedence: 5,
                    associativity: Associativity::Left,
                },
            );
        }
        for op in ["PLUS", "MINUS"] {
            binary_ops.insert(
                op.to_string(),
                OperatorInfo {
                    precedence: 6,
                    associativity: Associativity::Left,
                },
            );
        }
        for op in ["STAR", "SLASH", "MOD"] {
            binary_ops.insert(
                op.to_string(),
                OperatorInfo {
                    precedence: 7,
                    associativity: Associativity::Left,
                },
            );
        }
        binary_ops.insert(
            "POW".to_string(),
            OperatorInfo {
                precedence: 8,
                associativity: Associativity::Right,
            },
        );

        let unary_prefix_ops = ["NOT", "MINUS", "PLUS"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let primary_tokens = ["NUMBER", "IDENT", "STRING", "TRUE", "FALSE"]
            .into_iter()
            .map(str::to_string)
            .collect();

        PrattParser::new(PrattConfig {
            binary_ops,
            unary_prefix_ops,
            primary_tokens,
            lparen: "LPAREN".to_string(),
            rparen: "RPAREN".to_string(),
        })
    }

    fn binary_op_kind(node: &CstNode) -> &str {
        let CstNode::Node { name, children } = node else {
            panic!("expected BinaryExpr node");
        };
        assert_eq!(name, "BinaryExpr");
        let CstNode::Token { kind, .. } = &children[0] else {
            panic!("expected operator token as first child");
        };
        kind
    }

    #[test]
    fn parses_single_number() {
        let parser = hulk_like_pratt();
        let cst = parser
            .parse_expression(&[tok("NUMBER", "1"), tok("EOF", "")])
            .unwrap();
        assert_eq!(cst, CstNode::token(&tok("NUMBER", "1")));
    }

    #[test]
    fn parses_identifier() {
        let parser = hulk_like_pratt();
        let cst = parser
            .parse_expression(&[tok("IDENT", "x"), tok("EOF", "")])
            .unwrap();
        assert_eq!(cst, CstNode::token(&tok("IDENT", "x")));
    }

    #[test]
    fn parses_simple_binary() {
        let parser = hulk_like_pratt();
        let cst = parser
            .parse_expression(&[
                tok("NUMBER", "1"),
                tok("PLUS", "+"),
                tok("NUMBER", "2"),
                tok("EOF", ""),
            ])
            .unwrap();
        assert_eq!(binary_op_kind(&cst), "PLUS");
    }

    #[test]
    fn respects_multiplication_precedence() {
        let parser = hulk_like_pratt();
        let cst = parser
            .parse_expression(&[
                tok("NUMBER", "1"),
                tok("PLUS", "+"),
                tok("NUMBER", "2"),
                tok("STAR", "*"),
                tok("NUMBER", "3"),
                tok("EOF", ""),
            ])
            .unwrap();

        assert_eq!(binary_op_kind(&cst), "PLUS");
        let CstNode::Node { children, .. } = cst else {
            unreachable!()
        };
        assert_eq!(binary_op_kind(&children[2]), "STAR");
    }

    #[test]
    fn respects_parentheses() {
        let parser = hulk_like_pratt();
        let cst = parser
            .parse_expression(&[
                tok("LPAREN", "("),
                tok("NUMBER", "1"),
                tok("PLUS", "+"),
                tok("NUMBER", "2"),
                tok("RPAREN", ")"),
                tok("STAR", "*"),
                tok("NUMBER", "3"),
                tok("EOF", ""),
            ])
            .unwrap();

        assert_eq!(binary_op_kind(&cst), "STAR");
        let CstNode::Node { children, .. } = cst else {
            unreachable!()
        };
        let CstNode::Node {
            name: left_name,
            children: left_children,
        } = &children[1]
        else {
            panic!("expected GroupExpr as left child");
        };
        assert_eq!(left_name, "GroupExpr");
        assert_eq!(binary_op_kind(&left_children[0]), "PLUS");
    }

    #[test]
    fn left_associative_subtraction() {
        let parser = hulk_like_pratt();
        let cst = parser
            .parse_expression(&[
                tok("IDENT", "a"),
                tok("MINUS", "-"),
                tok("IDENT", "b"),
                tok("MINUS", "-"),
                tok("IDENT", "c"),
                tok("EOF", ""),
            ])
            .unwrap();

        assert_eq!(binary_op_kind(&cst), "MINUS");
        let CstNode::Node { children, .. } = cst else {
            unreachable!()
        };
        assert_eq!(binary_op_kind(&children[1]), "MINUS");
    }

    #[test]
    fn right_associative_power() {
        let parser = hulk_like_pratt();
        let cst = parser
            .parse_expression(&[
                tok("IDENT", "a"),
                tok("POW", "^"),
                tok("IDENT", "b"),
                tok("POW", "^"),
                tok("IDENT", "c"),
                tok("EOF", ""),
            ])
            .unwrap();

        assert_eq!(binary_op_kind(&cst), "POW");
        let CstNode::Node { children, .. } = cst else {
            unreachable!()
        };
        assert_eq!(binary_op_kind(&children[2]), "POW");
    }

    #[test]
    fn parses_unary_not() {
        let parser = hulk_like_pratt();
        let cst = parser
            .parse_expression(&[tok("NOT", "!"), tok("IDENT", "flag"), tok("EOF", "")])
            .unwrap();
        let CstNode::Node { name, children } = cst else {
            panic!("expected UnaryExpr node");
        };
        assert_eq!(name, "UnaryExpr");
        let CstNode::Token { kind, .. } = &children[0] else {
            panic!("expected unary operator token");
        };
        assert_eq!(kind, "NOT");
    }

    #[test]
    fn parses_unary_minus_with_binary() {
        let parser = hulk_like_pratt();
        let cst = parser
            .parse_expression(&[
                tok("MINUS", "-"),
                tok("IDENT", "x"),
                tok("STAR", "*"),
                tok("NUMBER", "2"),
                tok("EOF", ""),
            ])
            .unwrap();
        assert_eq!(binary_op_kind(&cst), "STAR");
        let CstNode::Node { children, .. } = cst else {
            unreachable!()
        };
        let CstNode::Node { name, .. } = &children[1] else {
            panic!("expected unary node on left side");
        };
        assert_eq!(name, "UnaryExpr");
    }

    #[test]
    fn reports_error_for_missing_rhs() {
        let parser = hulk_like_pratt();
        let err = parser
            .parse_expression(&[tok("NUMBER", "1"), tok("PLUS", "+"), tok("EOF", "")])
            .unwrap_err();
        assert_eq!(err.found, Some("EOF".to_string()));
        assert!(!err.expected.is_empty());
    }

    #[test]
    fn reports_error_for_unclosed_parenthesis() {
        let parser = hulk_like_pratt();
        let err = parser
            .parse_expression(&[
                tok("LPAREN", "("),
                tok("NUMBER", "1"),
                tok("PLUS", "+"),
                tok("NUMBER", "2"),
                tok("EOF", ""),
            ])
            .unwrap_err();
        assert!(err.message.contains("parenthesis") || err.message.contains("closing"));
    }
}
