use crate::runtime::cst::CstNode;
use crate::runtime::error::ParseError;
use crate::runtime::token::ParseToken;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

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
    pub comma: Option<String>,
    pub new_kw: Option<String>,
    pub self_kw: Option<String>,
    pub base_kw: Option<String>,
    pub dot: Option<String>,
    pub is_kw: Option<String>,
    pub as_kw: Option<String>,
    pub lbracket: Option<String>,
    pub rbracket: Option<String>,
    pub arrow: Option<String>,
    pub funcarrow: Option<String>,
    pub if_kw: Option<String>,
    pub elif_kw: Option<String>,
    pub else_kw: Option<String>,
    pub while_kw: Option<String>,
    pub for_kw: Option<String>,
    pub in_kw: Option<String>,
    pub lbrace: Option<String>,
    pub rbrace: Option<String>,
    pub semicolon: Option<String>,
    pub function_kw: Option<String>,
    pub let_kw: Option<String>,
    pub match_kw: Option<String>,
    pub wildcard: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PrattParser {
    config: Arc<PrattConfig>,
}

impl PrattParser {
    pub fn new(config: PrattConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    fn from_arc(config: Arc<PrattConfig>) -> Self {
        Self { config }
    }

    pub fn parse_expression(&self, tokens: &[ParseToken]) -> Result<CstNode, ParseError> {
        let mut stop_tokens = HashSet::new();
        stop_tokens.insert("EOF".to_string());
        let (expr, consumed) = self.parse_expression_until(tokens, &stop_tokens)?;

        if let Some(tok) = tokens.get(consumed)
            && tok.kind == "EOF"
        {
            if consumed + 1 != tokens.len() {
                let extra = &tokens[consumed + 1];
                return Err(ParseError {
                    message: "Unexpected tokens after EOF".to_string(),
                    line: extra.line,
                    column: extra.column,
                    found: Some(extra.kind.clone()),
                    expected: vec![],
                });
            }
            return Ok(expr);
        }

        if consumed != tokens.len() {
            let trailing = &tokens[consumed];
            return Err(ParseError {
                message: "Unexpected trailing tokens after expression".to_string(),
                line: trailing.line,
                column: trailing.column,
                found: Some(trailing.kind.clone()),
                expected: vec!["EOF".to_string()],
            });
        }

        Ok(expr)
    }

    pub fn parse_expression_until(
        &self,
        tokens: &[ParseToken],
        stop_tokens: &HashSet<String>,
    ) -> Result<(CstNode, usize), ParseError> {
        if tokens.is_empty() {
            return Err(ParseError {
                message: "Input token stream is empty".to_string(),
                line: 1,
                column: 1,
                found: None,
                expected: vec![],
            });
        }

        if let Some(first) = tokens.first()
            && stop_tokens.contains(&first.kind)
        {
            return Err(ParseError {
                message: "Expected expression before stop token".to_string(),
                line: first.line,
                column: first.column,
                found: Some(first.kind.clone()),
                expected: self.expected_prefix_kinds(),
            });
        }

        let mut state = PrattState::new(tokens, Arc::clone(&self.config), stop_tokens);
        let expr = state.parse_expression_bp(0)?;

        if state.pos == 0 {
            return Err(ParseError {
                message: "Expected expression".to_string(),
                line: 1,
                column: 1,
                found: None,
                expected: self.expected_prefix_kinds(),
            });
        }

        Ok((expr, state.pos))
    }

    fn expected_prefix_kinds(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.extend(self.config.primary_tokens.iter().cloned());
        out.extend(self.config.unary_prefix_ops.iter().cloned());
        out.push(self.config.lparen.clone());
        if let Some(new_kw) = &self.config.new_kw {
            out.push(new_kw.clone());
        }
        if let Some(self_kw) = &self.config.self_kw {
            out.push(self_kw.clone());
        }
        if let Some(base_kw) = &self.config.base_kw {
            out.push(base_kw.clone());
        }
        if let Some(lb) = &self.config.lbracket {
            out.push(lb.clone());
        }
        out.sort();
        out.dedup();
        out
    }
}

const MAX_PARSE_DEPTH: usize = 512;

struct PrattState<'a> {
    tokens: &'a [ParseToken],
    pos: usize,
    depth: usize,
    config: Arc<PrattConfig>,
    stop_tokens: &'a HashSet<String>,
}

impl<'a> PrattState<'a> {
    fn new(
        tokens: &'a [ParseToken],
        config: Arc<PrattConfig>,
        stop_tokens: &'a HashSet<String>,
    ) -> Self {
        Self {
            tokens,
            pos: 0,
            depth: 0,
            config,
            stop_tokens,
        }
    }

    fn parse_expression_bp(&mut self, min_bp: u8) -> Result<CstNode, ParseError> {
        if self.depth >= MAX_PARSE_DEPTH {
            let (line, col, found) = self
                .current()
                .map(|t| (t.line, t.column, Some(t.kind.clone())))
                .unwrap_or((1, 1, None));
            return Err(ParseError {
                message: format!(
                    "Parser recursion limit ({}) exceeded; expression is too deeply nested",
                    MAX_PARSE_DEPTH
                ),
                line,
                column: col,
                found,
                expected: vec![],
            });
        }
        self.depth += 1;
        let result = self.parse_expression_bp_impl(min_bp);
        self.depth -= 1;
        result
    }

    fn parse_expression_bp_impl(&mut self, min_bp: u8) -> Result<CstNode, ParseError> {
        let mut left = self.parse_prefix()?;

        loop {
            let Some(next) = self.current() else {
                break;
            };

            if self.stop_tokens.contains(&next.kind)
                || next.kind == "EOF"
                || next.kind == self.config.rparen
            {
                break;
            }

            // Call suffix: f(args)
            if next.kind == self.config.lparen {
                left = self.parse_call_suffix(left)?;
                continue;
            }

            // Dot suffix: obj.member or obj.method(args)
            if let Some(dot) = &self.config.dot
                && next.kind == *dot
            {
                left = self.parse_dot_suffix(left)?;
                continue;
            }

            // Vector index suffix: v[i]
            if let Some(lb) = &self.config.lbracket
                && next.kind == *lb
            {
                left = self.parse_index_suffix(left)?;
                continue;
            }

            // is / as — type-test / type-cast operators (precedence 4, left-assoc)
            if let Some(is_kw) = &self.config.is_kw
                && next.kind == *is_kw
            {
                if 4 < min_bp {
                    break;
                }
                let op_tok = next.clone();
                self.advance();
                let type_name_tok = self.expect_ident_after(&op_tok, "is")?;
                left = CstNode::node("TypeTestExpr", vec![left, CstNode::token(&type_name_tok)]);
                continue;
            }

            if let Some(as_kw) = &self.config.as_kw
                && next.kind == *as_kw
            {
                if 4 < min_bp {
                    break;
                }
                let op_tok = next.clone();
                self.advance();
                let type_name_tok = self.expect_ident_after(&op_tok, "as")?;
                left = CstNode::node("TypeCastExpr", vec![left, CstNode::token(&type_name_tok)]);
                continue;
            }

            let Some(op_info) = self.config.binary_ops.get(&next.kind).copied() else {
                break;
            };

            if op_info.precedence < min_bp {
                break;
            }

            let op_token = next.clone();
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

    fn expect_ident_after(
        &mut self,
        op_tok: &ParseToken,
        op_name: &str,
    ) -> Result<ParseToken, ParseError> {
        let tok = self.current().cloned().ok_or_else(|| ParseError {
            message: format!("Expected type name after '{}'", op_name),
            line: op_tok.line,
            column: op_tok.column,
            found: None,
            expected: vec!["IDENT".to_string()],
        })?;
        if tok.kind != "IDENT" {
            return Err(ParseError {
                message: format!("Expected type name after '{}'", op_name),
                line: tok.line,
                column: tok.column,
                found: Some(tok.kind.clone()),
                expected: vec!["IDENT".to_string()],
            });
        }
        self.advance();
        Ok(tok)
    }

    fn parse_index_suffix(&mut self, vector: CstNode) -> Result<CstNode, ParseError> {
        let open_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '[' to start index expression".to_string(),
            line: 1,
            column: 1,
            found: None,
            expected: vec!["LBRACKET".to_string()],
        })?;
        self.advance(); // consume [

        let rb_kind = self
            .config
            .rbracket
            .as_deref()
            .unwrap_or("RBRACKET")
            .to_string();

        let start_pos = self.pos;
        let remaining = &self.tokens[start_pos..];
        let mut index_stops = HashSet::new();
        index_stops.insert(rb_kind.clone());

        let (index, consumed) = PrattParser::from_arc(Arc::clone(&self.config))
            .parse_expression_until(remaining, &index_stops)
            .map_err(|e| ParseError {
                message: format!("Error parsing index expression: {}", e.message),
                line: open_tok.line,
                column: open_tok.column,
                found: e.found,
                expected: e.expected,
            })?;
        self.pos += consumed;

        // consume ]
        let close = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected ']' after index expression".to_string(),
            line: open_tok.line,
            column: open_tok.column,
            found: None,
            expected: vec![rb_kind.clone()],
        })?;
        if close.kind != rb_kind {
            return Err(ParseError {
                message: "Expected ']' after index expression".to_string(),
                line: close.line,
                column: close.column,
                found: Some(close.kind.clone()),
                expected: vec![rb_kind],
            });
        }
        self.advance();

        Ok(CstNode::node("VectorIndexExpr", vec![vector, index]))
    }

    fn parse_dot_suffix(&mut self, object: CstNode) -> Result<CstNode, ParseError> {
        let dot_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '.' for member access".to_string(),
            line: 1,
            column: 1,
            found: None,
            expected: vec!["DOT".to_string()],
        })?;
        self.advance();

        let ident = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected member name after '.'".to_string(),
            line: dot_tok.line,
            column: dot_tok.column,
            found: None,
            expected: vec!["IDENT".to_string()],
        })?;

        if !self.is_identifier_kind(&ident.kind) {
            return Err(ParseError {
                message: "Expected identifier after '.'".to_string(),
                line: ident.line,
                column: ident.column,
                found: Some(ident.kind.clone()),
                expected: vec!["IDENT".to_string()],
            });
        }
        // Normalize BASE → IDENT so the builder finds it
        let ident = if ident.kind != "IDENT" {
            ParseToken::new("IDENT", &ident.lexeme, ident.line, ident.column)
        } else {
            ident
        };

        self.advance();

        if let Some(next) = self.current()
            && next.kind == self.config.lparen
        {
            self.advance();
            let args = self.parse_args_until_rparen(next)?;
            let mut children = vec![object, CstNode::token(&ident)];
            children.extend(args);
            return Ok(CstNode::node("MethodCallExpr", children));
        }

        Ok(CstNode::node(
            "MemberAccessExpr",
            vec![object, CstNode::token(&ident)],
        ))
    }

    fn parse_call_suffix(&mut self, callee: CstNode) -> Result<CstNode, ParseError> {
        let open = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '(' to start function call".to_string(),
            line: 1,
            column: 1,
            found: None,
            expected: vec!["LPAREN".to_string()],
        })?;
        self.advance();
        let args = self.parse_args_until_rparen(&open)?;
        let mut children = vec![callee];
        children.extend(args);
        Ok(CstNode::node("CallExpr", children))
    }

    fn parse_args_until_rparen(&mut self, open: &ParseToken) -> Result<Vec<CstNode>, ParseError> {
        let mut args = Vec::<CstNode>::new();

        if let Some(tok) = self.current()
            && tok.kind != self.config.rparen
        {
            loop {
                let mut arg_stops = HashSet::new();
                arg_stops.insert(self.config.rparen.clone());
                if let Some(comma) = &self.config.comma {
                    arg_stops.insert(comma.clone());
                }

                let start_pos = self.pos;
                let remaining = &self.tokens[start_pos..];
                let (arg, consumed) = PrattParser::from_arc(Arc::clone(&self.config))
                    .parse_expression_until(remaining, &arg_stops)?;
                if consumed == 0 {
                    let fallback = self.current().cloned().unwrap_or(open.clone());
                    return Err(ParseError {
                        message: "Expected argument expression".to_string(),
                        line: fallback.line,
                        column: fallback.column,
                        found: Some(fallback.kind),
                        expected: vec![],
                    });
                }
                self.pos += consumed;
                args.push(arg);

                if let Some(comma) = &self.config.comma
                    && let Some(tok) = self.current()
                    && tok.kind == *comma
                {
                    self.advance();
                    continue;
                }
                break;
            }
        }

        let Some(close) = self.current() else {
            return Err(ParseError {
                message: "Unclosed call expression".to_string(),
                line: open.line,
                column: open.column,
                found: None,
                expected: vec![self.config.rparen.clone()],
            });
        };

        if close.kind != self.config.rparen {
            return Err(ParseError {
                message: "Expected closing parenthesis for call".to_string(),
                line: close.line,
                column: close.column,
                found: Some(close.kind.clone()),
                expected: vec![self.config.rparen.clone()],
            });
        }
        self.advance();

        Ok(args)
    }

    fn parse_prefix(&mut self) -> Result<CstNode, ParseError> {
        let tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Unexpected end of input while parsing expression".to_string(),
            line: 1,
            column: 1,
            found: None,
            expected: vec![],
        })?;

        if self.stop_tokens.contains(&tok.kind) {
            return Err(ParseError {
                message: "Expected expression".to_string(),
                line: tok.line,
                column: tok.column,
                found: Some(tok.kind.clone()),
                expected: self.expected_prefix_kinds(),
            });
        }

        if let Some(self_kw) = &self.config.self_kw
            && tok.kind == *self_kw
        {
            self.advance();
            return Ok(CstNode::node("SelfExpr", vec![]));
        }

        if let Some(base_kw) = &self.config.base_kw
            && tok.kind == *base_kw
        {
            // `base` is contextual: only a base call when followed by `(`
            // otherwise treat it as a plain identifier
            self.advance(); // consume BASE token
            let next_is_lparen = self
                .current()
                .map(|t| t.kind == self.config.lparen)
                .unwrap_or(false);

            if !next_is_lparen {
                // Treat as identifier
                return Ok(CstNode::Token {
                    kind: "IDENT".to_string(),
                    lexeme: tok.lexeme.clone(),
                    line: tok.line,
                    column: tok.column,
                });
            }

            let Some(open) = self.current().cloned() else {
                return Err(ParseError {
                    message: "Expected '(' after base".to_string(),
                    line: tok.line,
                    column: tok.column,
                    found: None,
                    expected: vec![self.config.lparen.clone()],
                });
            };

            if open.kind != self.config.lparen {
                return Err(ParseError {
                    message: "Expected '(' after base".to_string(),
                    line: open.line,
                    column: open.column,
                    found: Some(open.kind.clone()),
                    expected: vec![self.config.lparen.clone()],
                });
            }
            self.advance();
            let args = self.parse_args_until_rparen(&open)?;
            return Ok(CstNode::node("BaseCallExpr", args));
        }

        if let Some(new_kw) = &self.config.new_kw
            && tok.kind == *new_kw
        {
            self.advance();

            let ident = self.current().cloned().ok_or_else(|| ParseError {
                message: "Expected type identifier after new".to_string(),
                line: tok.line,
                column: tok.column,
                found: None,
                expected: vec!["IDENT".to_string()],
            })?;

            if ident.kind != "IDENT" {
                return Err(ParseError {
                    message: "Expected type identifier after new".to_string(),
                    line: ident.line,
                    column: ident.column,
                    found: Some(ident.kind.clone()),
                    expected: vec!["IDENT".to_string()],
                });
            }
            self.advance();

            // Check if array allocation: new T[N], new T[N]{ i -> body }, or new T[][N]
            if let Some(lb) = self.config.lbracket.clone() {
                if let Some(curr) = self.current()
                    && curr.kind == lb
                {
                    self.advance(); // consume [

                    let rb_kind = self
                        .config
                        .rbracket
                        .clone()
                        .unwrap_or_else(|| "RBRACKET".to_string());

                    // Detect `new T[][N]`: empty [] means element type is T[]
                    let elem_type_node = if let Some(curr) = self.current()
                        && curr.kind == rb_kind
                    {
                        // Empty brackets: this is `new T[][...]`
                        self.advance(); // consume ]
                        // Must be followed by another [
                        if let Some(curr) = self.current()
                            && curr.kind == lb
                        {
                            self.advance(); // consume [
                        } else {
                            return Err(ParseError {
                                message: "Expected '[' after '[]' in new T[][N] expression"
                                    .to_string(),
                                line: tok.line,
                                column: tok.column,
                                found: self.current().map(|c| c.kind.clone()),
                                expected: vec![lb.clone()],
                            });
                        }
                        // Element type is T[] (a vector of T)
                        CstNode::node("TypeVector", vec![CstNode::token(&ident)])
                    } else {
                        // Normal case: element type is just T
                        CstNode::token(&ident)
                    };

                    // Parse size expression stopping at ]
                    let start_pos = self.pos;
                    let remaining = &self.tokens[start_pos..];
                    let mut size_stops = HashSet::new();
                    size_stops.insert(rb_kind.clone());
                    let (size_expr, consumed) =
                        PrattParser::from_arc(Arc::clone(&self.config))
                            .parse_expression_until(remaining, &size_stops)
                            .map_err(|e| ParseError {
                                message: format!(
                                    "Error in array size expression: {}",
                                    e.message
                                ),
                                line: tok.line,
                                column: tok.column,
                                found: e.found,
                                expected: e.expected,
                            })?;
                    self.pos += consumed;

                    // consume ]
                    self.expect_rbracket(&tok, &rb_kind)?;

                    // Optional initializer: { var -> body }
                    let lbrace_kind = self
                        .config
                        .lbrace
                        .clone()
                        .unwrap_or_else(|| "LBRACE".to_string());
                    let rbrace_kind = self
                        .config
                        .rbrace
                        .clone()
                        .unwrap_or_else(|| "RBRACE".to_string());
                    let funcarrow_kind = self
                        .config
                        .funcarrow
                        .clone()
                        .unwrap_or_else(|| "FUNCARROW".to_string());

                    if let Some(curr) = self.current()
                        && curr.kind == lbrace_kind
                    {
                        self.advance(); // consume {

                        // Parse init var name
                        let init_var = self.current().cloned().ok_or_else(|| ParseError {
                            message: "Expected variable name in array initializer".to_string(),
                            line: tok.line,
                            column: tok.column,
                            found: None,
                            expected: vec!["IDENT".to_string()],
                        })?;
                        if init_var.kind != "IDENT" {
                            return Err(ParseError {
                                message: "Expected variable name in array initializer"
                                    .to_string(),
                                line: init_var.line,
                                column: init_var.column,
                                found: Some(init_var.kind.clone()),
                                expected: vec!["IDENT".to_string()],
                            });
                        }
                        self.advance(); // consume IDENT

                        // consume ->
                        let arrow_tok = self.current().cloned().ok_or_else(|| ParseError {
                            message: "Expected '->' in array initializer".to_string(),
                            line: tok.line,
                            column: tok.column,
                            found: None,
                            expected: vec![funcarrow_kind.clone()],
                        })?;
                        if arrow_tok.kind != funcarrow_kind {
                            return Err(ParseError {
                                message: "Expected '->' in array initializer".to_string(),
                                line: arrow_tok.line,
                                column: arrow_tok.column,
                                found: Some(arrow_tok.kind.clone()),
                                expected: vec![funcarrow_kind.clone()],
                            });
                        }
                        self.advance(); // consume ->

                        // Parse initializer body expression (stops at })
                        let start_pos = self.pos;
                        let remaining = &self.tokens[start_pos..];
                        let mut body_stops = HashSet::new();
                        body_stops.insert(rbrace_kind.clone());
                        let (init_body, consumed) =
                            PrattParser::from_arc(Arc::clone(&self.config))
                                .parse_expression_until(remaining, &body_stops)
                                .map_err(|e| ParseError {
                                    message: format!(
                                        "Error in array initializer: {}",
                                        e.message
                                    ),
                                    line: tok.line,
                                    column: tok.column,
                                    found: e.found,
                                    expected: e.expected,
                                })?;
                        self.pos += consumed;

                        // consume }
                        let close = self.current().cloned().ok_or_else(|| ParseError {
                            message: "Expected '}' to close array initializer".to_string(),
                            line: tok.line,
                            column: tok.column,
                            found: None,
                            expected: vec![rbrace_kind.clone()],
                        })?;
                        if close.kind != rbrace_kind {
                            return Err(ParseError {
                                message: "Expected '}' to close array initializer".to_string(),
                                line: close.line,
                                column: close.column,
                                found: Some(close.kind.clone()),
                                expected: vec![rbrace_kind.clone()],
                            });
                        }
                        self.advance(); // consume }

                        let mut children =
                            vec![elem_type_node, size_expr, CstNode::token(&init_var)];
                        children.push(CstNode::node("Expr", vec![init_body]));
                        return Ok(CstNode::node("NewArrayExpr", children));
                    }

                    // No initializer: new T[N] or new T[][N]
                    return Ok(CstNode::node(
                        "NewArrayExpr",
                        vec![elem_type_node, size_expr],
                    ));
                }
            }

            // Constructor call: new T(args)
            let Some(open) = self.current().cloned() else {
                return Err(ParseError {
                    message: "Expected '(' after type name in new".to_string(),
                    line: ident.line,
                    column: ident.column,
                    found: None,
                    expected: vec![self.config.lparen.clone()],
                });
            };

            if open.kind != self.config.lparen {
                return Err(ParseError {
                    message: "Expected '(' after type name in new".to_string(),
                    line: open.line,
                    column: open.column,
                    found: Some(open.kind.clone()),
                    expected: vec![self.config.lparen.clone()],
                });
            }
            self.advance();
            let args = self.parse_args_until_rparen(&open)?;
            let mut children = vec![CstNode::token(&ident)];
            children.extend(args);
            return Ok(CstNode::node("NewExpr", children));
        }

        if self.config.unary_prefix_ops.contains(&tok.kind) {
            self.advance();
            let expr = self.parse_expression_bp(9)?;
            return Ok(CstNode::node("UnaryExpr", vec![CstNode::token(&tok), expr]));
        }

        // Vector literal or generator: [ ... ]
        if let Some(lb) = &self.config.lbracket
            && tok.kind == *lb
        {
            return self.parse_vector_primary(tok);
        }

        // Grouped expression or lambda: ( ... )
        if tok.kind == self.config.lparen {
            if self.is_lambda_start() {
                return self.parse_lambda();
            }
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

        // Control-flow expressions used as sub-expressions in binary ops
        if let Some(if_kw) = &self.config.if_kw
            && tok.kind == *if_kw
        {
            return self.parse_if_subexpr(tok);
        }

        if let Some(while_kw) = &self.config.while_kw
            && tok.kind == *while_kw
        {
            return self.parse_while_subexpr(tok);
        }

        if let Some(for_kw) = &self.config.for_kw
            && tok.kind == *for_kw
        {
            return self.parse_for_subexpr(tok);
        }

        if let Some(lbrace) = &self.config.lbrace
            && tok.kind == *lbrace
        {
            return self.parse_block_subexpr(tok);
        }

        if let Some(fn_kw) = self.config.function_kw.clone()
            && tok.kind == fn_kw
        {
            return self.parse_function_lambda(tok);
        }

        if let Some(lk) = self.config.let_kw.clone()
            && tok.kind == lk
        {
            return self.parse_let_subexpr(tok);
        }

        if let Some(mk) = self.config.match_kw.clone()
            && tok.kind == mk
        {
            return self.parse_match_subexpr(tok);
        }

        Err(ParseError {
            message: "Expected expression".to_string(),
            line: tok.line,
            column: tok.column,
            found: Some(tok.kind.clone()),
            expected: self.expected_prefix_kinds(),
        })
    }

    // --- Control-flow sub-expression parsers ---
    // These allow if/while/for/block to appear as operands in binary expressions.

    fn parse_if_subexpr(&mut self, if_tok: ParseToken) -> Result<CstNode, ParseError> {
        self.advance(); // consume IF

        let lparen_kind = self.config.lparen.clone();
        let rparen_kind = self.config.rparen.clone();

        // Consume opening (
        let open = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '(' after 'if'".to_string(),
            line: if_tok.line,
            column: if_tok.column,
            found: None,
            expected: vec![lparen_kind.clone()],
        })?;
        if open.kind != lparen_kind {
            return Err(ParseError {
                message: "Expected '(' after 'if'".to_string(),
                line: open.line,
                column: open.column,
                found: Some(open.kind.clone()),
                expected: vec![lparen_kind.clone()],
            });
        }
        self.advance(); // consume (

        // Parse condition — stops at ) naturally (RPAREN in stop_tokens)
        let cond = self.parse_expression_bp(0)?;

        // Consume )
        let close = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected ')' after if condition".to_string(),
            line: if_tok.line,
            column: if_tok.column,
            found: None,
            expected: vec![rparen_kind.clone()],
        })?;
        if close.kind != rparen_kind {
            return Err(ParseError {
                message: "Expected ')' after if condition".to_string(),
                line: close.line,
                column: close.column,
                found: Some(close.kind.clone()),
                expected: vec![rparen_kind.clone()],
            });
        }
        self.advance(); // consume )

        // Parse then-branch — ELIF/ELSE are in stop_tokens, so it stops naturally
        let then_expr = self.parse_expression_bp(0)?;

        // Collect elif chains
        let mut elif_branches: Vec<(CstNode, CstNode)> = Vec::new();
        loop {
            let is_elif = self
                .current()
                .and_then(|t| self.config.elif_kw.as_deref().map(|k| t.kind == k))
                .unwrap_or(false);
            if !is_elif {
                break;
            }
            self.advance(); // consume ELIF

            let elif_open = self.current().cloned().ok_or_else(|| ParseError {
                message: "Expected '(' after 'elif'".to_string(),
                line: if_tok.line,
                column: if_tok.column,
                found: None,
                expected: vec![lparen_kind.clone()],
            })?;
            if elif_open.kind != lparen_kind {
                return Err(ParseError {
                    message: "Expected '(' after 'elif'".to_string(),
                    line: elif_open.line,
                    column: elif_open.column,
                    found: Some(elif_open.kind.clone()),
                    expected: vec![lparen_kind.clone()],
                });
            }
            self.advance(); // consume (

            let elif_cond = self.parse_expression_bp(0)?;

            let elif_close = self.current().cloned().ok_or_else(|| ParseError {
                message: "Expected ')' after elif condition".to_string(),
                line: if_tok.line,
                column: if_tok.column,
                found: None,
                expected: vec![rparen_kind.clone()],
            })?;
            if elif_close.kind != rparen_kind {
                return Err(ParseError {
                    message: "Expected ')' after elif condition".to_string(),
                    line: elif_close.line,
                    column: elif_close.column,
                    found: Some(elif_close.kind.clone()),
                    expected: vec![rparen_kind.clone()],
                });
            }
            self.advance(); // consume )

            let elif_body = self.parse_expression_bp(0)?;
            elif_branches.push((elif_cond, elif_body));
        }

        // Expect ELSE
        let else_kw = self.config.else_kw.clone().unwrap_or_else(|| "ELSE".to_string());
        let else_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected 'else' in if expression".to_string(),
            line: if_tok.line,
            column: if_tok.column,
            found: None,
            expected: vec![else_kw.clone()],
        })?;
        if else_tok.kind != else_kw {
            return Err(ParseError {
                message: "Expected 'else' in if expression".to_string(),
                line: else_tok.line,
                column: else_tok.column,
                found: Some(else_tok.kind.clone()),
                expected: vec![else_kw],
            });
        }
        self.advance(); // consume ELSE

        // Parse else-branch
        let else_expr = self.parse_expression_bp(0)?;

        // Build ElifList recursively (innermost first)
        let mut elif_list = CstNode::node("ElifList", vec![]);
        for (ec, eb) in elif_branches.into_iter().rev() {
            elif_list = CstNode::node(
                "ElifList",
                vec![
                    CstNode::node("Expr", vec![ec]),
                    CstNode::node("Expr", vec![eb]),
                    elif_list,
                ],
            );
        }

        Ok(CstNode::node(
            "IfExpr",
            vec![
                CstNode::node("Expr", vec![cond]),
                CstNode::node("Expr", vec![then_expr]),
                elif_list,
                CstNode::node("Expr", vec![else_expr]),
            ],
        ))
    }

    fn parse_while_subexpr(&mut self, while_tok: ParseToken) -> Result<CstNode, ParseError> {
        self.advance(); // consume WHILE

        let lparen_kind = self.config.lparen.clone();
        let rparen_kind = self.config.rparen.clone();

        let open = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '(' after 'while'".to_string(),
            line: while_tok.line,
            column: while_tok.column,
            found: None,
            expected: vec![lparen_kind.clone()],
        })?;
        if open.kind != lparen_kind {
            return Err(ParseError {
                message: "Expected '(' after 'while'".to_string(),
                line: open.line,
                column: open.column,
                found: Some(open.kind.clone()),
                expected: vec![lparen_kind.clone()],
            });
        }
        self.advance(); // consume (

        let cond = self.parse_expression_bp(0)?;

        let close = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected ')' after while condition".to_string(),
            line: while_tok.line,
            column: while_tok.column,
            found: None,
            expected: vec![rparen_kind.clone()],
        })?;
        if close.kind != rparen_kind {
            return Err(ParseError {
                message: "Expected ')' after while condition".to_string(),
                line: close.line,
                column: close.column,
                found: Some(close.kind.clone()),
                expected: vec![rparen_kind.clone()],
            });
        }
        self.advance(); // consume )

        let body = self.parse_expression_bp(0)?;

        Ok(CstNode::node(
            "WhileExpr",
            vec![
                CstNode::node("Expr", vec![cond]),
                CstNode::node("Expr", vec![body]),
            ],
        ))
    }

    fn parse_for_subexpr(&mut self, for_tok: ParseToken) -> Result<CstNode, ParseError> {
        self.advance(); // consume FOR

        let lparen_kind = self.config.lparen.clone();
        let rparen_kind = self.config.rparen.clone();

        let open = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '(' after 'for'".to_string(),
            line: for_tok.line,
            column: for_tok.column,
            found: None,
            expected: vec![lparen_kind.clone()],
        })?;
        if open.kind != lparen_kind {
            return Err(ParseError {
                message: "Expected '(' after 'for'".to_string(),
                line: open.line,
                column: open.column,
                found: Some(open.kind.clone()),
                expected: vec![lparen_kind.clone()],
            });
        }
        self.advance(); // consume (

        // IDENT
        let var_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected variable name in 'for'".to_string(),
            line: for_tok.line,
            column: for_tok.column,
            found: None,
            expected: vec!["IDENT".to_string()],
        })?;
        if var_tok.kind != "IDENT" {
            return Err(ParseError {
                message: "Expected variable name in 'for'".to_string(),
                line: var_tok.line,
                column: var_tok.column,
                found: Some(var_tok.kind.clone()),
                expected: vec!["IDENT".to_string()],
            });
        }
        self.advance(); // consume IDENT

        // IN keyword
        let in_kw = self.config.in_kw.clone().unwrap_or_else(|| "IN".to_string());
        let in_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected 'in' in for expression".to_string(),
            line: for_tok.line,
            column: for_tok.column,
            found: None,
            expected: vec![in_kw.clone()],
        })?;
        if in_tok.kind != in_kw {
            return Err(ParseError {
                message: "Expected 'in' in for expression".to_string(),
                line: in_tok.line,
                column: in_tok.column,
                found: Some(in_tok.kind.clone()),
                expected: vec![in_kw],
            });
        }
        self.advance(); // consume IN

        // Parse iterable expression — stops at RPAREN
        let iter = self.parse_expression_bp(0)?;

        let close = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected ')' after for iterable".to_string(),
            line: for_tok.line,
            column: for_tok.column,
            found: None,
            expected: vec![rparen_kind.clone()],
        })?;
        if close.kind != rparen_kind {
            return Err(ParseError {
                message: "Expected ')' after for iterable".to_string(),
                line: close.line,
                column: close.column,
                found: Some(close.kind.clone()),
                expected: vec![rparen_kind.clone()],
            });
        }
        self.advance(); // consume )

        let body = self.parse_expression_bp(0)?;

        Ok(CstNode::node(
            "ForExpr",
            vec![
                CstNode::token(&var_tok),
                CstNode::node("Expr", vec![iter]),
                CstNode::node("Expr", vec![body]),
            ],
        ))
    }

    fn parse_block_subexpr(&mut self, brace_tok: ParseToken) -> Result<CstNode, ParseError> {
        self.advance(); // consume {

        let rbrace_kind = self
            .config
            .rbrace
            .clone()
            .unwrap_or_else(|| "RBRACE".to_string());
        let semi_kind = self
            .config
            .semicolon
            .clone()
            .unwrap_or_else(|| "SEMICOLON".to_string());

        // Empty block: {}
        if let Some(curr) = self.current()
            && curr.kind == rbrace_kind
        {
            self.advance();
            return Ok(CstNode::node("BlockExpr", vec![]));
        }

        // Parse first expression
        let first_expr = self.parse_expression_bp(0)?;

        // Check next token to determine if this is array literal (comma) or block (semicolon/rbrace)
        if let Some(curr) = self.current() {
            if let Some(ref ck) = self.config.comma.clone()
                && curr.kind == *ck
            {
                // Array literal: {first, second, ...}
                self.advance(); // consume comma
                let mut elements = vec![first_expr];

                loop {
                    if let Some(c) = self.current()
                        && c.kind == rbrace_kind
                    {
                        break;
                    }
                    if self.current().is_none() {
                        break;
                    }
                    let elem = self.parse_expression_bp(0)?;
                    elements.push(elem);

                    if let Some(c) = self.current()
                        && let Some(ref ck2) = self.config.comma.clone()
                        && c.kind == *ck2
                    {
                        self.advance(); // consume comma
                    } else {
                        break;
                    }
                }

                let close = self.current().cloned().ok_or_else(|| ParseError {
                    message: "Expected '}' to close array literal".to_string(),
                    line: brace_tok.line,
                    column: brace_tok.column,
                    found: None,
                    expected: vec![rbrace_kind.clone()],
                })?;
                if close.kind != rbrace_kind {
                    return Err(ParseError {
                        message: "Expected '}' to close array literal".to_string(),
                        line: close.line,
                        column: close.column,
                        found: Some(close.kind.clone()),
                        expected: vec![rbrace_kind.clone()],
                    });
                }
                self.advance(); // consume }
                return Ok(CstNode::node("VectorLiteralExpr", elements));
            }
        }

        // Block expression: { first; second; ... }
        let mut stmts = vec![CstNode::node("Expr", vec![first_expr])];

        loop {
            let Some(curr) = self.current() else { break };
            if curr.kind == semi_kind {
                self.advance(); // consume ;
                // Trailing semicolon or end of block
                match self.current() {
                    None => break,
                    Some(c) if c.kind == rbrace_kind => break,
                    _ => {}
                }
                let stmt = self.parse_expression_bp(0)?;
                stmts.push(CstNode::node("Expr", vec![stmt]));
            } else if curr.kind == rbrace_kind {
                break;
            } else {
                break;
            }
        }

        let close = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '}' to close block".to_string(),
            line: brace_tok.line,
            column: brace_tok.column,
            found: None,
            expected: vec![rbrace_kind.clone()],
        })?;
        if close.kind != rbrace_kind {
            return Err(ParseError {
                message: "Expected '}' to close block".to_string(),
                line: close.line,
                column: close.column,
                found: Some(close.kind.clone()),
                expected: vec![rbrace_kind.clone()],
            });
        }
        self.advance(); // consume }
        Ok(CstNode::node("BlockExpr", stmts))
    }

    fn parse_function_lambda(&mut self, fn_tok: ParseToken) -> Result<CstNode, ParseError> {
        self.advance(); // consume FUNCTION

        let open_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '(' after 'function' in lambda".to_string(),
            line: fn_tok.line,
            column: fn_tok.column,
            found: None,
            expected: vec![self.config.lparen.clone()],
        })?;
        if open_tok.kind != self.config.lparen {
            return Err(ParseError {
                message: "Expected '(' after 'function' in lambda".to_string(),
                line: open_tok.line,
                column: open_tok.column,
                found: Some(open_tok.kind.clone()),
                expected: vec![self.config.lparen.clone()],
            });
        }
        self.advance(); // consume (

        let mut param_nodes = Vec::new();

        while let Some(curr) = self.current()
            && curr.kind != self.config.rparen
        {
            let ident_tok = curr.clone();
            if ident_tok.kind != "IDENT" {
                return Err(ParseError {
                    message: "Expected parameter name in function lambda".to_string(),
                    line: ident_tok.line,
                    column: ident_tok.column,
                    found: Some(ident_tok.kind.clone()),
                    expected: vec!["IDENT".to_string()],
                });
            }
            self.advance();

            let mut param_children = vec![CstNode::token(&ident_tok)];
            if let Some(curr) = self.current()
                && curr.kind == "COLON"
            {
                self.advance();
                let type_node = self.parse_type_tokens()?;
                param_children.push(type_node);
            }
            param_nodes.push(CstNode::node("LambdaParam", param_children));

            if let Some(curr) = self.current()
                && let Some(ref comma) = self.config.comma.clone()
                && curr.kind == *comma
            {
                self.advance();
            }
        }

        let close = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected ')' to close function lambda parameters".to_string(),
            line: fn_tok.line,
            column: fn_tok.column,
            found: None,
            expected: vec![self.config.rparen.clone()],
        })?;
        if close.kind != self.config.rparen {
            return Err(ParseError {
                message: "Expected ')' to close function lambda parameters".to_string(),
                line: close.line,
                column: close.column,
                found: Some(close.kind.clone()),
                expected: vec![self.config.rparen.clone()],
            });
        }
        self.advance();

        let mut return_type_node = None;
        if let Some(curr) = self.current()
            && curr.kind == "COLON"
        {
            self.advance();
            let type_node = self.parse_type_tokens()?;
            return_type_node = Some(CstNode::node("LambdaReturnType", vec![type_node]));
        }

        // consume -> (FUNCARROW)
        let funcarrow = self
            .config
            .funcarrow
            .clone()
            .unwrap_or_else(|| "FUNCARROW".to_string());
        let arrow_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '->' after function lambda parameters".to_string(),
            line: fn_tok.line,
            column: fn_tok.column,
            found: None,
            expected: vec![funcarrow.clone()],
        })?;
        if arrow_tok.kind != funcarrow {
            return Err(ParseError {
                message: "Expected '->' after function lambda parameters".to_string(),
                line: arrow_tok.line,
                column: arrow_tok.column,
                found: Some(arrow_tok.kind.clone()),
                expected: vec![funcarrow],
            });
        }
        self.advance();

        let body = self.parse_expression_bp(0)?;

        let mut children = param_nodes;
        if let Some(ret) = return_type_node {
            children.push(ret);
        }
        children.push(body);
        Ok(CstNode::node("LambdaExpr", children))
    }

    fn parse_let_subexpr(&mut self, let_tok: ParseToken) -> Result<CstNode, ParseError> {
        self.advance(); // consume LET

        let in_kw = self
            .config
            .in_kw
            .clone()
            .unwrap_or_else(|| "IN".to_string());

        let first_binding = self.parse_one_let_binding()?;

        let mut more_bindings = Vec::new();
        while let Some(curr) = self.current()
            && let Some(ref ck) = self.config.comma.clone()
            && curr.kind == *ck
        {
            self.advance(); // consume comma
            more_bindings.push(self.parse_one_let_binding()?);
        }

        let in_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected 'in' in let expression".to_string(),
            line: let_tok.line,
            column: let_tok.column,
            found: None,
            expected: vec![in_kw.clone()],
        })?;
        if in_tok.kind != in_kw {
            return Err(ParseError {
                message: "Expected 'in' in let expression".to_string(),
                line: in_tok.line,
                column: in_tok.column,
                found: Some(in_tok.kind.clone()),
                expected: vec![in_kw],
            });
        }
        self.advance(); // consume IN

        let body = self.parse_expression_bp(0)?;

        // Build LetBindingTail chain (reversed so first extra binding is outermost)
        let mut tail = CstNode::node("LetBindingTail", vec![]);
        for b in more_bindings.into_iter().rev() {
            tail = CstNode::node("LetBindingTail", vec![b, tail]);
        }

        Ok(CstNode::node(
            "LetExpr",
            vec![first_binding, tail, CstNode::node("Expr", vec![body])],
        ))
    }

    fn parse_one_let_binding(&mut self) -> Result<CstNode, ParseError> {
        let ident_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected identifier in let binding".to_string(),
            line: 1,
            column: 1,
            found: None,
            expected: vec!["IDENT".to_string()],
        })?;
        if !self.is_identifier_kind(&ident_tok.kind) {
            return Err(ParseError {
                message: "Expected identifier in let binding".to_string(),
                line: ident_tok.line,
                column: ident_tok.column,
                found: Some(ident_tok.kind.clone()),
                expected: vec!["IDENT".to_string()],
            });
        }
        self.advance();
        // Normalize contextual keywords to IDENT so the frontend builder finds them.
        let ident_tok = if ident_tok.kind != "IDENT" {
            ParseToken::new("IDENT", &ident_tok.lexeme, ident_tok.line, ident_tok.column)
        } else {
            ident_tok
        };

        let type_ann = if let Some(curr) = self.current()
            && curr.kind == "COLON"
        {
            self.advance();
            let type_node = self.parse_type_tokens()?;
            Some(CstNode::node("TypeAnnotation", vec![type_node]))
        } else {
            None
        };

        // consume =
        let eq_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '=' in let binding".to_string(),
            line: ident_tok.line,
            column: ident_tok.column,
            found: None,
            expected: vec!["EQUAL".to_string()],
        })?;
        if eq_tok.kind != "EQUAL" {
            return Err(ParseError {
                message: "Expected '=' in let binding".to_string(),
                line: eq_tok.line,
                column: eq_tok.column,
                found: Some(eq_tok.kind.clone()),
                expected: vec!["EQUAL".to_string()],
            });
        }
        self.advance();

        // Value expression stops at stop_tokens (which includes IN and COMMA)
        let val_expr = self.parse_expression_bp(0)?;

        let mut children = vec![CstNode::token(&ident_tok)];
        if let Some(ann) = type_ann {
            children.push(ann);
        }
        children.push(CstNode::node("Expr", vec![val_expr]));
        Ok(CstNode::node("LetBinding", children))
    }

    // --- Match parsing ---

    fn parse_match_pattern(&mut self, match_tok: &ParseToken) -> Result<CstNode, ParseError> {
        let curr = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected pattern in match arm".to_string(),
            line: match_tok.line,
            column: match_tok.column,
            found: None,
            expected: vec!["WILDCARD".to_string(), "IDENT".to_string()],
        })?;

        // Wildcard: _
        if let Some(ref wk) = self.config.wildcard.clone()
            && curr.kind == *wk
        {
            self.advance();
            return Ok(CstNode::node("PatternWildcard", vec![]));
        }

        // Literal: number, string, true, false
        if self.config.primary_tokens.contains(&curr.kind)
            && curr.kind != "IDENT"
        {
            self.advance();
            return Ok(CstNode::node("PatternLiteral", vec![CstNode::token(&curr)]));
        }

        // Identifier: TypePattern (uppercase first) or Binding (lowercase first)
        if self.is_identifier_kind(&curr.kind) || curr.kind == "IDENT" {
            let name_tok = ParseToken::new("IDENT", &curr.lexeme, curr.line, curr.column);
            self.advance();

            let first_char = curr.lexeme.chars().next().unwrap_or('a');
            if first_char.is_uppercase() {
                // TypePattern — optional "as varname" binding
                if let Some(as_kw) = self.config.as_kw.clone()
                    && let Some(next) = self.current()
                    && next.kind == as_kw
                {
                    self.advance(); // consume 'as'
                    let bind_tok = self.current().cloned().ok_or_else(|| ParseError {
                        message: "Expected identifier after 'as' in pattern".to_string(),
                        line: curr.line,
                        column: curr.column,
                        found: None,
                        expected: vec!["IDENT".to_string()],
                    })?;
                    if !self.is_identifier_kind(&bind_tok.kind) {
                        return Err(ParseError {
                            message: "Expected identifier after 'as' in pattern".to_string(),
                            line: bind_tok.line,
                            column: bind_tok.column,
                            found: Some(bind_tok.kind.clone()),
                            expected: vec!["IDENT".to_string()],
                        });
                    }
                    let bind_ident =
                        ParseToken::new("IDENT", &bind_tok.lexeme, bind_tok.line, bind_tok.column);
                    self.advance(); // consume bind name
                    return Ok(CstNode::node(
                        "PatternType",
                        vec![
                            CstNode::token(&name_tok),
                            CstNode::node("PatternBind", vec![CstNode::token(&bind_ident)]),
                        ],
                    ));
                }
                return Ok(CstNode::node("PatternType", vec![CstNode::token(&name_tok)]));
            } else {
                // Binding pattern
                return Ok(CstNode::node(
                    "PatternBinding",
                    vec![CstNode::token(&name_tok)],
                ));
            }
        }

        Err(ParseError {
            message: format!("Unexpected token '{}' in match pattern", curr.lexeme),
            line: curr.line,
            column: curr.column,
            found: Some(curr.kind.clone()),
            expected: vec!["WILDCARD".to_string(), "IDENT".to_string(), "NUMBER".to_string()],
        })
    }

    fn parse_match_subexpr(&mut self, match_tok: ParseToken) -> Result<CstNode, ParseError> {
        self.advance(); // consume MATCH

        let lparen_kind = self.config.lparen.clone();
        let rparen_kind = self.config.rparen.clone();
        let lbrace_kind = self
            .config
            .lbrace
            .clone()
            .unwrap_or_else(|| "LBRACE".to_string());
        let rbrace_kind = self
            .config
            .rbrace
            .clone()
            .unwrap_or_else(|| "RBRACE".to_string());
        let arrow_kind = self
            .config
            .arrow
            .clone()
            .unwrap_or_else(|| "ARROW".to_string());

        // Parse scrutinee in parens: (expr)
        let open_paren = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '(' after 'match'".to_string(),
            line: match_tok.line,
            column: match_tok.column,
            found: None,
            expected: vec![lparen_kind.clone()],
        })?;
        if open_paren.kind != lparen_kind {
            return Err(ParseError {
                message: "Expected '(' after 'match'".to_string(),
                line: open_paren.line,
                column: open_paren.column,
                found: Some(open_paren.kind.clone()),
                expected: vec![lparen_kind.clone()],
            });
        }
        self.advance(); // consume (

        let scrutinee = self.parse_expression_bp(0)?;

        let close_paren = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected ')' after match scrutinee".to_string(),
            line: match_tok.line,
            column: match_tok.column,
            found: None,
            expected: vec![rparen_kind.clone()],
        })?;
        if close_paren.kind != rparen_kind {
            return Err(ParseError {
                message: "Expected ')' after match scrutinee".to_string(),
                line: close_paren.line,
                column: close_paren.column,
                found: Some(close_paren.kind.clone()),
                expected: vec![rparen_kind.clone()],
            });
        }
        self.advance(); // consume )

        // Consume opening {
        let open_brace = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '{' after match scrutinee".to_string(),
            line: match_tok.line,
            column: match_tok.column,
            found: None,
            expected: vec![lbrace_kind.clone()],
        })?;
        if open_brace.kind != lbrace_kind {
            return Err(ParseError {
                message: "Expected '{' after match scrutinee".to_string(),
                line: open_brace.line,
                column: open_brace.column,
                found: Some(open_brace.kind.clone()),
                expected: vec![lbrace_kind.clone()],
            });
        }
        self.advance(); // consume {

        // Parse arms: pattern => body ,?
        let mut children = vec![CstNode::node("Expr", vec![scrutinee])];

        loop {
            // Check for closing brace (no more arms)
            if let Some(curr) = self.current()
                && curr.kind == rbrace_kind
            {
                break;
            }
            if self.current().is_none() {
                break;
            }

            let pattern = self.parse_match_pattern(&match_tok)?;

            // Consume =>
            let arrow_tok = self.current().cloned().ok_or_else(|| ParseError {
                message: "Expected '=>' after match pattern".to_string(),
                line: match_tok.line,
                column: match_tok.column,
                found: None,
                expected: vec![arrow_kind.clone()],
            })?;
            if arrow_tok.kind != arrow_kind {
                return Err(ParseError {
                    message: "Expected '=>' after match pattern".to_string(),
                    line: arrow_tok.line,
                    column: arrow_tok.column,
                    found: Some(arrow_tok.kind.clone()),
                    expected: vec![arrow_kind.clone()],
                });
            }
            self.advance(); // consume =>

            let body = self.parse_expression_bp(0)?;

            children.push(CstNode::node(
                "MatchArm",
                vec![pattern, CstNode::node("Expr", vec![body])],
            ));

            // Optional trailing comma
            if let Some(curr) = self.current()
                && let Some(ref ck) = self.config.comma.clone()
                && curr.kind == *ck
            {
                self.advance(); // consume comma
            }
        }

        // Consume closing }
        let close_brace = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '}' to close match expression".to_string(),
            line: match_tok.line,
            column: match_tok.column,
            found: None,
            expected: vec![rbrace_kind.clone()],
        })?;
        if close_brace.kind != rbrace_kind {
            return Err(ParseError {
                message: "Expected '}' to close match expression".to_string(),
                line: close_brace.line,
                column: close_brace.column,
                found: Some(close_brace.kind.clone()),
                expected: vec![rbrace_kind.clone()],
            });
        }
        self.advance(); // consume }

        Ok(CstNode::node("MatchExpr", children))
    }

    // --- Vector parsing ---

    fn parse_vector_primary(&mut self, open_tok: ParseToken) -> Result<CstNode, ParseError> {
        self.advance(); // consume [

        let rb_kind = self
            .config
            .rbracket
            .as_deref()
            .unwrap_or("RBRACKET")
            .to_string();

        // Empty vector
        if let Some(curr) = self.current()
            && curr.kind == rb_kind
        {
            self.advance();
            return Ok(CstNode::node("VectorLiteralExpr", vec![]));
        }

        // Determine if this is a generator by scanning ahead for `| IDENT IN` at depth 0
        let is_gen = self.scan_vector_is_generator();

        if is_gen {
            return self.parse_vector_generator(open_tok, rb_kind);
        }

        // Vector literal: collect comma-separated exprs
        let mut elements = Vec::new();
        loop {
            let start_pos = self.pos;
            let remaining = &self.tokens[start_pos..];
            let mut el_stops = HashSet::new();
            el_stops.insert(rb_kind.clone());
            if let Some(comma) = &self.config.comma {
                el_stops.insert(comma.clone());
            }

            let (el, consumed) = PrattParser::from_arc(Arc::clone(&self.config))
                .parse_expression_until(remaining, &el_stops)
                .map_err(|e| ParseError {
                    message: format!("Error in vector element: {}", e.message),
                    line: open_tok.line,
                    column: open_tok.column,
                    found: e.found,
                    expected: e.expected,
                })?;
            self.pos += consumed;
            elements.push(el);

            if let Some(curr) = self.current() {
                if curr.kind == rb_kind {
                    break;
                }
                if let Some(comma) = &self.config.comma
                    && curr.kind == *comma
                {
                    self.advance(); // consume ,
                    continue;
                }
            }
            break;
        }

        // consume ]
        self.expect_rbracket(&open_tok, &rb_kind)?;
        Ok(CstNode::node("VectorLiteralExpr", elements))
    }

    fn parse_vector_generator(
        &mut self,
        open_tok: ParseToken,
        rb_kind: String,
    ) -> Result<CstNode, ParseError> {
        // Parse body expression stopping at OR (|)
        let start_pos = self.pos;
        let remaining = &self.tokens[start_pos..];
        let mut body_stops = HashSet::new();
        body_stops.insert("OR".to_string()); // | separator
        body_stops.insert(rb_kind.clone());

        let (body, consumed) = PrattParser::from_arc(Arc::clone(&self.config))
            .parse_expression_until(remaining, &body_stops)
            .map_err(|e| ParseError {
                message: format!("Error in vector generator body: {}", e.message),
                line: open_tok.line,
                column: open_tok.column,
                found: e.found,
                expected: e.expected,
            })?;
        self.pos += consumed;

        // consume | (OR token)
        let pipe = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '|' in vector generator".to_string(),
            line: open_tok.line,
            column: open_tok.column,
            found: None,
            expected: vec!["OR".to_string()],
        })?;
        if pipe.kind != "OR" {
            return Err(ParseError {
                message: "Expected '|' in vector generator".to_string(),
                line: pipe.line,
                column: pipe.column,
                found: Some(pipe.kind.clone()),
                expected: vec!["OR".to_string()],
            });
        }
        self.advance();

        // consume IDENT (loop variable)
        let var_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected variable name in vector generator".to_string(),
            line: pipe.line,
            column: pipe.column,
            found: None,
            expected: vec!["IDENT".to_string()],
        })?;
        if var_tok.kind != "IDENT" {
            return Err(ParseError {
                message: "Expected variable name in vector generator".to_string(),
                line: var_tok.line,
                column: var_tok.column,
                found: Some(var_tok.kind.clone()),
                expected: vec!["IDENT".to_string()],
            });
        }
        self.advance();

        // consume IN
        let in_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected 'in' in vector generator".to_string(),
            line: var_tok.line,
            column: var_tok.column,
            found: None,
            expected: vec!["IN".to_string()],
        })?;
        if in_tok.kind != "IN" {
            return Err(ParseError {
                message: "Expected 'in' in vector generator".to_string(),
                line: in_tok.line,
                column: in_tok.column,
                found: Some(in_tok.kind.clone()),
                expected: vec!["IN".to_string()],
            });
        }
        self.advance();

        // Parse iterable expression stopping at ]
        let start_pos = self.pos;
        let remaining = &self.tokens[start_pos..];
        let mut iter_stops = HashSet::new();
        iter_stops.insert(rb_kind.clone());

        let (iterable, consumed) = PrattParser::from_arc(Arc::clone(&self.config))
            .parse_expression_until(remaining, &iter_stops)
            .map_err(|e| ParseError {
                message: format!("Error in vector generator iterable: {}", e.message),
                line: open_tok.line,
                column: open_tok.column,
                found: e.found,
                expected: e.expected,
            })?;
        self.pos += consumed;

        // consume ]
        self.expect_rbracket(&open_tok, &rb_kind)?;

        Ok(CstNode::node(
            "VectorGeneratorExpr",
            vec![body, CstNode::token(&var_tok), iterable],
        ))
    }

    /// Scan ahead from current position (inside a `[`) to detect generator pattern:
    /// `| IDENT IN` at bracket+paren depth 0.
    fn scan_vector_is_generator(&self) -> bool {
        let mut bracket_depth: i32 = 0;
        let mut paren_depth: i32 = 0;
        let lb = self.config.lbracket.as_deref().unwrap_or("LBRACKET");
        let rb = self.config.rbracket.as_deref().unwrap_or("RBRACKET");

        let mut i = self.pos;
        while i < self.tokens.len() {
            let tok = &self.tokens[i];
            match tok.kind.as_str() {
                k if k == lb => bracket_depth += 1,
                k if k == rb => {
                    if bracket_depth == 0 {
                        return false; // reached outer ]
                    }
                    bracket_depth -= 1;
                }
                k if k == self.config.lparen => paren_depth += 1,
                k if k == self.config.rparen => {
                    if paren_depth > 0 {
                        paren_depth -= 1;
                    }
                }
                "OR" if bracket_depth == 0 && paren_depth == 0 => {
                    // Check next two tokens: IDENT IN
                    let next1 = self.tokens.get(i + 1);
                    let next2 = self.tokens.get(i + 2);
                    let is_gen = next1.map(|t| t.kind == "IDENT").unwrap_or(false)
                        && next2.map(|t| t.kind == "IN").unwrap_or(false);
                    return is_gen;
                }
                "EOF" | "SEMICOLON" => return false,
                _ => {}
            }
            i += 1;
        }
        false
    }

    fn expect_rbracket(&mut self, open_tok: &ParseToken, rb_kind: &str) -> Result<(), ParseError> {
        let close = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected ']'".to_string(),
            line: open_tok.line,
            column: open_tok.column,
            found: None,
            expected: vec![rb_kind.to_string()],
        })?;
        if close.kind != rb_kind {
            return Err(ParseError {
                message: "Expected ']'".to_string(),
                line: close.line,
                column: close.column,
                found: Some(close.kind.clone()),
                expected: vec![rb_kind.to_string()],
            });
        }
        self.advance();
        Ok(())
    }

    // --- Lambda parsing ---

    /// Check (without consuming) if the current `(` starts a lambda expression.
    fn is_lambda_start(&self) -> bool {
        let mut depth: i32 = 0;
        let mut i = self.pos;
        while i < self.tokens.len() {
            let tok = &self.tokens[i];
            if tok.kind == self.config.lparen {
                depth += 1;
            } else if tok.kind == self.config.rparen {
                depth -= 1;
                if depth == 0 {
                    // Found matching RPAREN at index i. Check what follows.
                    let after = i + 1;
                    if let Some(next) = self.tokens.get(after) {
                        // Case 1: () => body  or  (params) => body
                        if let Some(arrow) = &self.config.arrow
                            && next.kind == *arrow
                        {
                            return true;
                        }
                        // Case 2: (params): ReturnType => body
                        if next.kind == "COLON" {
                            let mut j = after + 1;
                            while j < self.tokens.len() {
                                let t = &self.tokens[j];
                                if let Some(arrow) = &self.config.arrow
                                    && t.kind == *arrow
                                {
                                    return true;
                                }
                                // Only scan through type-like tokens
                                if matches!(
                                    t.kind.as_str(),
                                    "IDENT"
                                        | "STAR"
                                        | "LBRACKET"
                                        | "RBRACKET"
                                        | "FUNCARROW"
                                        | "LPAREN"
                                        | "RPAREN"
                                        | "COMMA"
                                ) {
                                    j += 1;
                                } else {
                                    return false;
                                }
                            }
                        }
                    }
                    return false;
                }
            } else if tok.kind == "EOF" {
                return false;
            }
            i += 1;
        }
        false
    }

    fn parse_lambda(&mut self) -> Result<CstNode, ParseError> {
        let open_tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected '(' to start lambda expression".to_string(),
            line: 1,
            column: 1,
            found: None,
            expected: vec!["LPAREN".to_string()],
        })?;
        self.advance(); // consume (

        let mut param_nodes = Vec::new();

        // Parse params until RPAREN
        while let Some(curr) = self.current()
            && curr.kind != self.config.rparen
        {
            let ident_tok = curr.clone();
            if ident_tok.kind != "IDENT" {
                return Err(ParseError {
                    message: "Expected parameter name in lambda".to_string(),
                    line: ident_tok.line,
                    column: ident_tok.column,
                    found: Some(ident_tok.kind.clone()),
                    expected: vec!["IDENT".to_string()],
                });
            }
            self.advance(); // consume IDENT

            let mut param_children = vec![CstNode::token(&ident_tok)];

            // Optional type annotation: COLON TypeTokens
            if let Some(curr) = self.current()
                && curr.kind == "COLON"
            {
                self.advance(); // consume COLON
                let type_node = self.parse_type_tokens()?;
                param_children.push(type_node);
            }

            param_nodes.push(CstNode::node("LambdaParam", param_children));

            // Consume optional comma
            if let Some(curr) = self.current()
                && let Some(comma) = &self.config.comma
                && curr.kind == *comma
            {
                self.advance();
            }
        }

        // consume )
        let close = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected ')' to close lambda parameters".to_string(),
            line: open_tok.line,
            column: open_tok.column,
            found: None,
            expected: vec![self.config.rparen.clone()],
        })?;
        if close.kind != self.config.rparen {
            return Err(ParseError {
                message: "Expected ')' to close lambda parameters".to_string(),
                line: close.line,
                column: close.column,
                found: Some(close.kind.clone()),
                expected: vec![self.config.rparen.clone()],
            });
        }
        self.advance();

        // Optional return type: COLON TypeTokens
        let mut return_type_node = None;
        if let Some(curr) = self.current()
            && curr.kind == "COLON"
        {
            self.advance(); // consume COLON
            let type_node = self.parse_type_tokens()?;
            return_type_node = Some(CstNode::node("LambdaReturnType", vec![type_node]));
        }

        // consume => (ARROW)
        if let Some(arrow_kind) = self.config.arrow.clone() {
            let arrow_tok = self.current().cloned().ok_or_else(|| ParseError {
                message: "Expected '=>' after lambda parameters".to_string(),
                line: open_tok.line,
                column: open_tok.column,
                found: None,
                expected: vec![arrow_kind.clone()],
            })?;
            if arrow_tok.kind != arrow_kind {
                return Err(ParseError {
                    message: "Expected '=>' after lambda parameters".to_string(),
                    line: arrow_tok.line,
                    column: arrow_tok.column,
                    found: Some(arrow_tok.kind.clone()),
                    expected: vec![arrow_kind],
                });
            }
            self.advance();
        }

        // Parse body expression
        let body = self.parse_expression_bp(0)?;

        let mut children = param_nodes;
        if let Some(ret) = return_type_node {
            children.push(ret);
        }
        children.push(body);

        Ok(CstNode::node("LambdaExpr", children))
    }

    /// Parse a type token sequence: IDENT, IDENT STAR, IDENT LBRACKET RBRACKET,
    /// or a functor type (T1, T2) -> T3 when `funcarrow` is configured.
    fn parse_type_tokens(&mut self) -> Result<CstNode, ParseError> {
        let tok = self.current().cloned().ok_or_else(|| ParseError {
            message: "Expected type".to_string(),
            line: 1,
            column: 1,
            found: None,
            expected: vec!["IDENT".to_string()],
        })?;

        // Functor type: (T1, T2) -> ReturnType
        if tok.kind == self.config.lparen {
            if let Some(funcarrow) = self.config.funcarrow.clone() {
                self.advance(); // consume (
                let mut param_nodes = Vec::new();

                while let Some(curr) = self.current()
                    && curr.kind != self.config.rparen
                {
                    param_nodes.push(self.parse_type_tokens()?);
                    if let Some(curr) = self.current()
                        && self.config.comma.as_deref() == Some(curr.kind.as_str())
                    {
                        self.advance(); // consume ,
                    } else {
                        break;
                    }
                }

                let close = self.current().cloned().ok_or_else(|| ParseError {
                    message: "Expected ')' to close functor type parameters".to_string(),
                    line: tok.line,
                    column: tok.column,
                    found: None,
                    expected: vec![self.config.rparen.clone()],
                })?;
                if close.kind != self.config.rparen {
                    return Err(ParseError {
                        message: "Expected ')' to close functor type parameters".to_string(),
                        line: close.line,
                        column: close.column,
                        found: Some(close.kind.clone()),
                        expected: vec![self.config.rparen.clone()],
                    });
                }
                self.advance(); // consume )

                let arrow_tok = self.current().cloned().ok_or_else(|| ParseError {
                    message: "Expected '->' in functor type".to_string(),
                    line: tok.line,
                    column: tok.column,
                    found: None,
                    expected: vec![funcarrow.clone()],
                })?;
                if arrow_tok.kind != funcarrow {
                    return Err(ParseError {
                        message: "Expected '->' in functor type".to_string(),
                        line: arrow_tok.line,
                        column: arrow_tok.column,
                        found: Some(arrow_tok.kind.clone()),
                        expected: vec![funcarrow],
                    });
                }
                self.advance(); // consume ->

                let return_type = self.parse_type_tokens()?;
                let mut children = param_nodes;
                children.push(CstNode::node("TypeFunctorReturn", vec![return_type]));
                return Ok(CstNode::node("TypeFunctor", children));
            }
        }

        if tok.kind != "IDENT" {
            return Err(ParseError {
                message: "Expected type identifier".to_string(),
                line: tok.line,
                column: tok.column,
                found: Some(tok.kind.clone()),
                expected: vec!["IDENT".to_string()],
            });
        }
        self.advance();

        // Check for [] suffix → vector (supports multiple [] for 2D+ arrays)
        if let Some(lb) = self.config.lbracket.clone() {
            let rb = self
                .config
                .rbracket
                .clone()
                .unwrap_or_else(|| "RBRACKET".to_string());
            if let Some(curr) = self.current()
                && curr.kind == lb
            {
                self.advance(); // consume [
                if let Some(curr) = self.current()
                    && curr.kind == rb
                {
                    self.advance(); // consume ]
                    let mut node = CstNode::node("TypeVector", vec![CstNode::token(&tok)]);
                    // Keep wrapping for additional [] dimensions (e.g. Number[][])
                    loop {
                        match self.current() {
                            Some(c) if c.kind == lb => {
                                self.advance(); // consume [
                                match self.current() {
                                    Some(c2) if c2.kind == rb => {
                                        self.advance(); // consume ]
                                        node = CstNode::node("TypeVector", vec![node]);
                                    }
                                    _ => break,
                                }
                            }
                            _ => break,
                        }
                    }
                    return Ok(node);
                }
            }
        }

        // Check for * suffix → iterable
        if let Some(curr) = self.current()
            && curr.kind == "STAR"
        {
            self.advance();
            return Ok(CstNode::node("TypeIterable", vec![CstNode::token(&tok)]));
        }

        // Simple type
        Ok(CstNode::token(&tok))
    }

    fn expected_prefix_kinds(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.extend(self.config.primary_tokens.iter().cloned());
        out.extend(self.config.unary_prefix_ops.iter().cloned());
        out.push(self.config.lparen.clone());
        if let Some(new_kw) = &self.config.new_kw {
            out.push(new_kw.clone());
        }
        if let Some(self_kw) = &self.config.self_kw {
            out.push(self_kw.clone());
        }
        if let Some(base_kw) = &self.config.base_kw {
            out.push(base_kw.clone());
        }
        if let Some(lb) = &self.config.lbracket {
            out.push(lb.clone());
        }
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

    /// Returns true if `kind` is usable as an identifier in binding positions.
    /// `base`, `self`, and similar contextual keywords can appear as variable names.
    fn is_identifier_kind(&self, kind: &str) -> bool {
        if kind == "IDENT" {
            return true;
        }
        if let Some(base_kw) = &self.config.base_kw {
            if kind == base_kw {
                return true;
            }
        }
        false
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
            comma: Some("COMMA".to_string()),
            new_kw: None,
            self_kw: None,
            base_kw: None,
            dot: None,
            is_kw: None,
            as_kw: None,
            lbracket: None,
            rbracket: None,
            arrow: None,
            funcarrow: None,
            if_kw: None,
            elif_kw: None,
            else_kw: None,
            while_kw: None,
            for_kw: None,
            in_kw: None,
            lbrace: None,
            rbrace: None,
            semicolon: None,
            function_kw: None,
            let_kw: None,
            match_kw: None,
            wildcard: None,
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
