use crate::runtime::cst::CstNode;
use crate::runtime::error::ParseError;
use crate::runtime::pratt::PrattParser;
use crate::runtime::token::ParseToken;
use crate::spec::grammar_spec::GrammarSpec;
use crate::spec::table::ParseTable;
use crate::symbol::Symbol;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct PrattHook {
    pub parser: PrattParser,
    pub stop_tokens: HashSet<String>,
}

pub struct RuntimeParser {
    grammar: GrammarSpec,
    table: ParseTable,
    pratt_hooks: HashMap<String, PrattHook>,
}

impl RuntimeParser {
    pub fn new(grammar: GrammarSpec, table: ParseTable) -> Self {
        Self {
            grammar,
            table,
            pratt_hooks: HashMap::new(),
        }
    }

    pub fn with_pratt_hook(
        mut self,
        non_terminal: impl Into<String>,
        parser: PrattParser,
        stop_tokens: HashSet<String>,
    ) -> Self {
        self.pratt_hooks.insert(
            non_terminal.into(),
            PrattHook {
                parser,
                stop_tokens,
            },
        );
        self
    }

    /// Parse a token stream, returning the CST root or a list of all parse errors.
    ///
    /// Error recovery is performed at the declaration level: if a function/type/protocol
    /// declaration fails to parse, an `CstNode::Error` placeholder is inserted and parsing
    /// continues with the next declaration.  The collected errors are returned as `Err(errors)`
    /// so the caller receives every diagnostic rather than only the first one.
    pub fn parse(&self, tokens: &[ParseToken]) -> Result<CstNode, Vec<ParseError>> {
        if tokens.is_empty() {
            return Err(vec![ParseError {
                message: "Input token stream is empty; expected at least EOF".to_string(),
                line: 1,
                column: 1,
                found: None,
                expected: vec!["EOF".to_string()],
            }]);
        }

        let mut state = ParserState::new(tokens);
        let root = self.parse_non_terminal(&self.grammar.start, &mut state);

        match root {
            Ok(node) => {
                if state.errors.is_empty() {
                    Ok(node)
                } else {
                    Err(state.errors)
                }
            }
            Err(e) => {
                state.errors.push(e);
                Err(state.errors)
            }
        }
    }

    fn parse_non_terminal(
        &self,
        non_terminal: &str,
        state: &mut ParserState<'_>,
    ) -> Result<CstNode, ParseError> {
        if let Some(hook) = self.pratt_hooks.get(non_terminal) {
            let remaining = state.remaining();
            let (expr, consumed) = hook
                .parser
                .parse_expression_until(remaining, &hook.stop_tokens)?;
            if consumed == 0 {
                let tok = state.current()?;
                return Err(ParseError {
                    message: format!("Pratt hook for '{}' consumed zero tokens", non_terminal),
                    line: tok.line,
                    column: tok.column,
                    found: Some(tok.kind.clone()),
                    expected: vec![],
                });
            }
            state.advance_by(consumed);
            return Ok(CstNode::node(non_terminal, vec![expr]));
        }

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
                    if name == "Decl" {
                        // Recovery at declaration level: skip to the next decl keyword.
                        match self.parse_non_terminal(name, state) {
                            Ok(node) => children.push(node),
                            Err(e) => {
                                let err_line = e.line;
                                let err_col = e.column;
                                state.errors.push(e);
                                state.synchronize_decl();
                                children.push(CstNode::error(
                                    &state.errors.last().unwrap().message,
                                    err_line,
                                    err_col,
                                ));
                            }
                        }
                    } else if name == "Expr"
                        && matches!(non_terminal, "ExprList" | "ExprListTailAfterSemi")
                    {
                        // Recovery at statement level inside a block: skip to the next `;`
                        // or `}` so parsing can continue with the following statement.
                        match self.parse_non_terminal(name, state) {
                            Ok(node) => children.push(node),
                            Err(e) => {
                                let err_line = e.line;
                                let err_col = e.column;
                                state.errors.push(e);
                                state.synchronize_stmt();
                                children.push(CstNode::error(
                                    &state.errors.last().unwrap().message,
                                    err_line,
                                    err_col,
                                ));
                            }
                        }
                    } else if name == "Param"
                        && matches!(non_terminal, "ParamList" | "ParamListTail")
                    {
                        // Recovery inside a parameter list: skip to the next `,` or `)`.
                        match self.parse_non_terminal(name, state) {
                            Ok(node) => children.push(node),
                            Err(e) => {
                                let err_line = e.line;
                                let err_col = e.column;
                                state.errors.push(e);
                                state.synchronize_param();
                                children.push(CstNode::error(
                                    &state.errors.last().unwrap().message,
                                    err_line,
                                    err_col,
                                ));
                            }
                        }
                    } else if name == "TypeMember" && non_terminal == "TypeMemberList" {
                        // Recovery inside a type body: skip past the `;` of the bad member
                        // (or to `}`) so subsequent members can still be parsed.
                        match self.parse_non_terminal(name, state) {
                            Ok(node) => children.push(node),
                            Err(e) => {
                                let err_line = e.line;
                                let err_col = e.column;
                                state.errors.push(e);
                                state.synchronize_type_member();
                                children.push(CstNode::error(
                                    &state.errors.last().unwrap().message,
                                    err_line,
                                    err_col,
                                ));
                            }
                        }
                    } else if name == "LetBinding" && non_terminal == "LetBindingTail" {
                        // Recovery inside a multi-binding let: skip to the next `,` or `in`.
                        match self.parse_non_terminal(name, state) {
                            Ok(node) => children.push(node),
                            Err(e) => {
                                let err_line = e.line;
                                let err_col = e.column;
                                state.errors.push(e);
                                state.synchronize_let_binding();
                                children.push(CstNode::error(
                                    &state.errors.last().unwrap().message,
                                    err_line,
                                    err_col,
                                ));
                            }
                        }
                    } else {
                        children.push(self.parse_non_terminal(name, state)?);
                    }
                }
                Symbol::Epsilon => {}
            }
        }

        Ok(CstNode::node(non_terminal, children))
    }

    fn expected_lookaheads(&self, non_terminal: &str) -> Vec<String> {
        const MAX_EXPECTED: usize = 8;
        let mut out = self
            .table
            .keys()
            .filter(|(lhs, _)| lhs == non_terminal)
            .map(|(_, terminal)| terminal.clone())
            .collect::<Vec<_>>();
        out.sort();
        out.dedup();
        out.truncate(MAX_EXPECTED);
        out
    }
}

struct ParserState<'a> {
    tokens: &'a [ParseToken],
    pos: usize,
    errors: Vec<ParseError>,
}

impl<'a> ParserState<'a> {
    fn new(tokens: &'a [ParseToken]) -> Self {
        Self {
            tokens,
            pos: 0,
            errors: Vec::new(),
        }
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

    fn remaining(&self) -> &'a [ParseToken] {
        &self.tokens[self.pos..]
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn advance_by(&mut self, count: usize) {
        self.pos += count;
    }

    /// Skip tokens until the next declaration keyword or EOF.
    /// The synchronization token is NOT consumed so the grammar can match it.
    fn synchronize_decl(&mut self) {
        while self.pos < self.tokens.len() {
            let kind = self.tokens[self.pos].kind.as_str();
            if matches!(kind, "FUNCTION" | "TYPE" | "PROTOCOL" | "EOF") {
                break;
            }
            self.pos += 1;
        }
    }

    /// Skip tokens until `;`, `}`, or EOF.
    /// The synchronization token is NOT consumed — `ExprListTail -> SEMICOLON …`
    /// needs to consume the `;` itself on the next production step.
    fn synchronize_stmt(&mut self) {
        while self.pos < self.tokens.len() {
            let kind = self.tokens[self.pos].kind.as_str();
            if matches!(kind, "SEMICOLON" | "RBRACE" | "EOF") {
                break;
            }
            self.pos += 1;
        }
    }

    /// Skip tokens until `,`, `)`, or EOF.
    /// The synchronization token is NOT consumed so the outer `ParamListTail`
    /// production can consume the `,` or the caller can close the `)`.
    fn synchronize_param(&mut self) {
        while self.pos < self.tokens.len() {
            let kind = self.tokens[self.pos].kind.as_str();
            if matches!(kind, "COMMA" | "RPAREN" | "EOF") {
                break;
            }
            self.pos += 1;
        }
    }

    /// Skip tokens until after `;` (consumed) or until `}` / EOF (not consumed).
    /// Used to recover from a bad type member so the next member can be parsed.
    fn synchronize_type_member(&mut self) {
        while self.pos < self.tokens.len() {
            let kind = self.tokens[self.pos].kind.as_str();
            if kind == "SEMICOLON" {
                self.pos += 1; // consume the `;` — next member starts fresh
                break;
            }
            if matches!(kind, "RBRACE" | "EOF") {
                break; // leave `}` for the outer block to close
            }
            self.pos += 1;
        }
    }

    /// Skip tokens until `,`, `in`, or EOF.
    /// The synchronization token is NOT consumed so `LetBindingTail -> COMMA …`
    /// can pick up at the `,`, or `LetExpr -> … IN Expr` can consume the `in`.
    fn synchronize_let_binding(&mut self) {
        while self.pos < self.tokens.len() {
            let kind = self.tokens[self.pos].kind.as_str();
            if matches!(kind, "COMMA" | "IN" | "EOF") {
                break;
            }
            self.pos += 1;
        }
    }
}
