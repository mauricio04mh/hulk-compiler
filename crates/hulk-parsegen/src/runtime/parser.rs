use crate::runtime::cst::CstNode;
use crate::runtime::error::ParseError;
use crate::runtime::token::ParseToken;
use crate::spec::grammar_spec::GrammarSpec;
use crate::spec::table::ParseTable;
use crate::symbol::Symbol;

pub struct RuntimeParser {
    grammar: GrammarSpec,
    table: ParseTable,
}

impl RuntimeParser {
    pub fn new(grammar: GrammarSpec, table: ParseTable) -> Self {
        Self { grammar, table }
    }

    pub fn parse(&self, tokens: &[ParseToken]) -> Result<CstNode, ParseError> {
        if tokens.is_empty() {
            return Err(ParseError {
                message: "Input token stream is empty; expected at least EOF".to_string(),
                line: 1,
                column: 1,
                found: None,
                expected: vec!["EOF".to_string()],
            });
        }

        let mut state = ParserState::new(tokens);
        let root = self.parse_non_terminal(&self.grammar.start, &mut state)?;

        if state.pos == tokens.len() {
            return Ok(root);
        }

        let current = state.current()?;
        if current.kind == "EOF" && state.pos + 1 == tokens.len() {
            return Ok(root);
        }

        if current.kind != "EOF" {
            return Err(ParseError {
                message: "Unexpected trailing input after parse completed".to_string(),
                line: current.line,
                column: current.column,
                found: Some(current.kind.clone()),
                expected: vec!["EOF".to_string()],
            });
        }

        Err(ParseError {
            message: "Unexpected tokens after EOF".to_string(),
            line: current.line,
            column: current.column,
            found: Some(current.kind.clone()),
            expected: vec![],
        })
    }

    fn parse_non_terminal(
        &self,
        non_terminal: &str,
        state: &mut ParserState<'_>,
    ) -> Result<CstNode, ParseError> {
        let lookahead = state.current()?;
        let key = (non_terminal.to_string(), lookahead.kind.clone());
        let Some(production) = self.table.get(&key) else {
            return Err(ParseError {
                message: format!(
                    "No production for non-terminal '{}' with lookahead '{}'",
                    non_terminal, lookahead.kind
                ),
                line: lookahead.line,
                column: lookahead.column,
                found: Some(lookahead.kind.clone()),
                expected: self.expected_lookaheads(non_terminal),
            });
        };

        let mut children = Vec::<CstNode>::new();
        for symbol in &production.rhs {
            match symbol {
                Symbol::Terminal(expected_kind) => {
                    let token = state.current()?;
                    if token.kind != *expected_kind {
                        return Err(ParseError {
                            message: format!(
                                "Expected terminal '{}', found '{}'",
                                expected_kind, token.kind
                            ),
                            line: token.line,
                            column: token.column,
                            found: Some(token.kind.clone()),
                            expected: vec![expected_kind.clone()],
                        });
                    }
                    children.push(CstNode::token(token));
                    state.advance();
                }
                Symbol::Eof => {
                    let token = state.current()?;
                    if token.kind != "EOF" {
                        return Err(ParseError {
                            message: format!("Expected EOF, found '{}'", token.kind),
                            line: token.line,
                            column: token.column,
                            found: Some(token.kind.clone()),
                            expected: vec!["EOF".to_string()],
                        });
                    }
                    children.push(CstNode::token(token));
                    state.advance();
                }
                Symbol::NonTerminal(name) => {
                    children.push(self.parse_non_terminal(name, state)?);
                }
                Symbol::Epsilon => {}
            }
        }

        Ok(CstNode::node(non_terminal, children))
    }

    fn expected_lookaheads(&self, non_terminal: &str) -> Vec<String> {
        let mut out = self
            .table
            .keys()
            .filter(|(lhs, _)| lhs == non_terminal)
            .map(|(_, terminal)| terminal.clone())
            .collect::<Vec<_>>();
        out.sort();
        out.dedup();
        out
    }
}

struct ParserState<'a> {
    tokens: &'a [ParseToken],
    pos: usize,
}

impl<'a> ParserState<'a> {
    fn new(tokens: &'a [ParseToken]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn current(&self) -> Result<&'a ParseToken, ParseError> {
        self.tokens.get(self.pos).ok_or_else(|| {
            let fallback = self
                .tokens
                .last()
                .cloned()
                .unwrap_or_else(|| ParseToken::new("EOF", "", 1, 1));
            ParseError {
                message: "Unexpected end of input".to_string(),
                line: fallback.line,
                column: fallback.column,
                found: None,
                expected: vec!["EOF".to_string()],
            }
        })
    }

    fn advance(&mut self) {
        self.pos += 1;
    }
}
