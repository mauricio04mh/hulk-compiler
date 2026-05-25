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

        if ident.kind != "IDENT" {
            return Err(ParseError {
                message: "Expected identifier after '.'".to_string(),
                line: ident.line,
                column: ident.column,
                found: Some(ident.kind.clone()),
                expected: vec!["IDENT".to_string()],
            });
        }

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
            self.advance();
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

        Err(ParseError {
            message: "Expected expression".to_string(),
            line: tok.line,
            column: tok.column,
            found: Some(tok.kind.clone()),
            expected: self.expected_prefix_kinds(),
        })
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

        // Check for [] suffix → vector
        if let Some(lb) = self.config.lbracket.clone() {
            if let Some(curr) = self.current()
                && curr.kind == lb
            {
                let rb = self
                    .config
                    .rbracket
                    .clone()
                    .unwrap_or_else(|| "RBRACKET".to_string());
                self.advance(); // consume [
                if let Some(curr) = self.current()
                    && curr.kind == rb
                {
                    self.advance(); // consume ]
                    return Ok(CstNode::node("TypeVector", vec![CstNode::token(&tok)]));
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
