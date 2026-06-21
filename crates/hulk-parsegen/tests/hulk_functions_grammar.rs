use hulk_parsegen::gx::parser::parse_gx;
use hulk_parsegen::runtime::parser::RuntimeParser;
use hulk_parsegen::runtime::pratt::{Associativity, OperatorInfo, PrattConfig, PrattParser};
use hulk_parsegen::runtime::token::ParseToken;
use hulk_parsegen::spec::first::compute_first_sets;
use hulk_parsegen::spec::follow::compute_follow_sets;
use hulk_parsegen::spec::normalize::normalize_grammar;
use hulk_parsegen::spec::table::build_ll1_table;
use std::collections::{HashMap, HashSet};

fn tok(kind: &str, lexeme: &str) -> ParseToken {
    ParseToken::new(kind, lexeme, 1, 1)
}

fn tok_at(kind: &str, lexeme: &str, line: usize, col: usize) -> ParseToken {
    ParseToken::new(kind, lexeme, line, col)
}

fn read_fixture(relative_path: &str) -> String {
    let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), relative_path);
    std::fs::read_to_string(path).expect("fixture file must exist")
}

fn minimal_pratt() -> PrattParser {
    let mut binary_ops = HashMap::new();
    for op in ["PLUS", "MINUS"] {
        binary_ops.insert(
            op.to_string(),
            OperatorInfo {
                precedence: 6,
                associativity: Associativity::Left,
            },
        );
    }
    for op in ["STAR", "SLASH"] {
        binary_ops.insert(
            op.to_string(),
            OperatorInfo {
                precedence: 7,
                associativity: Associativity::Left,
            },
        );
    }

    let unary_prefix_ops = ["NOT", "MINUS"].into_iter().map(str::to_string).collect();
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

fn functions_stop_tokens() -> HashSet<String> {
    ["SEMICOLON", "COMMA", "RPAREN", "IN", "RBRACE", "EOF"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn build_functions_parser() -> RuntimeParser {
    let gx_source = read_fixture("testdata/grammars/hulk_functions.gx");
    let grammar = parse_gx(&gx_source).expect("gx grammar should parse");
    let spec = normalize_grammar(grammar).expect("grammar should normalize");
    let first = compute_first_sets(&spec);
    let follow = compute_follow_sets(&spec, &first);
    let table = build_ll1_table(&spec, &first, &follow).expect("grammar must be LL(1)");
    RuntimeParser::new(spec, table).with_pratt_hook(
        "OperatorExpr",
        minimal_pratt(),
        functions_stop_tokens(),
    )
}

// --- Happy path ---

#[test]
fn parses_block_with_single_statement() {
    let parser = build_functions_parser();
    // { 42 }
    let tokens = vec![
        tok("LBRACE", "{"),
        tok("NUMBER", "42"),
        tok("RBRACE", "}"),
        tok("EOF", ""),
    ];
    let result = parser.parse(&tokens);
    assert!(result.is_ok(), "expected ok, got: {:?}", result.err());
}

#[test]
fn parses_block_with_two_statements() {
    let parser = build_functions_parser();
    // { 1; 2 }
    let tokens = vec![
        tok("LBRACE", "{"),
        tok("NUMBER", "1"),
        tok("SEMICOLON", ";"),
        tok("NUMBER", "2"),
        tok("RBRACE", "}"),
        tok("EOF", ""),
    ];
    let result = parser.parse(&tokens);
    assert!(result.is_ok(), "expected ok, got: {:?}", result.err());
}

#[test]
fn parses_function_with_block_body() {
    let parser = build_functions_parser();
    // function f() { 42 }
    let tokens = vec![
        tok("FUNCTION", "function"),
        tok("IDENT", "f"),
        tok("LPAREN", "("),
        tok("RPAREN", ")"),
        tok("LBRACE", "{"),
        tok("NUMBER", "42"),
        tok("RBRACE", "}"),
        tok("NUMBER", "0"),
        tok("EOF", ""),
    ];
    let result = parser.parse(&tokens);
    assert!(result.is_ok(), "expected ok, got: {:?}", result.err());
}

// --- Statement-level error recovery ---

#[test]
fn recovers_from_single_invalid_statement_in_block() {
    let parser = build_functions_parser();
    // { + ; 42 }
    // PLUS is not a valid expression start in the test's Pratt config, so the Pratt parser
    // fails at PLUS. Recovery skips to SEMICOLON and continues with `42`.
    let tokens = vec![
        tok("LBRACE", "{"),
        tok("PLUS", "+"),
        tok("SEMICOLON", ";"),
        tok("NUMBER", "42"),
        tok("RBRACE", "}"),
        tok("EOF", ""),
    ];
    let errors = parser.parse(&tokens).expect_err("should have parse errors");
    assert_eq!(
        errors.len(),
        1,
        "expected exactly one error, got: {:?}",
        errors
    );
    assert_eq!(errors[0].found, Some("PLUS".to_string()));
}

#[test]
fn recovers_from_two_invalid_statements_reports_two_errors() {
    let parser = build_functions_parser();
    // { + ; - ; 42 }
    // Both `+` and `-` fail as unary ops without operands, then `42` parses fine.
    let tokens = vec![
        tok("LBRACE", "{"),
        tok("PLUS", "+"),
        tok("SEMICOLON", ";"),
        tok("MINUS", "-"),
        tok("SEMICOLON", ";"),
        tok("NUMBER", "42"),
        tok("RBRACE", "}"),
        tok("EOF", ""),
    ];
    let errors = parser.parse(&tokens).expect_err("should have parse errors");
    assert_eq!(
        errors.len(),
        2,
        "expected two errors, one per bad statement, got: {:?}",
        errors
    );
}

#[test]
fn recovered_errors_carry_correct_position() {
    let parser = build_functions_parser();
    // { + ; 42 } with specific line/col
    let tokens = vec![
        tok_at("LBRACE", "{", 1, 1),
        tok_at("PLUS", "+", 2, 3),
        tok_at("SEMICOLON", ";", 2, 4),
        tok_at("NUMBER", "42", 3, 3),
        tok_at("RBRACE", "}", 4, 1),
        tok_at("EOF", "", 4, 2),
    ];
    let errors = parser.parse(&tokens).expect_err("should have parse errors");
    assert_eq!(errors.len(), 1);
    // Error should point at the bad token (SEMICOLON where operand was expected)
    assert_eq!(errors[0].line, 2);
}

// --- Pretty error messages ---

#[test]
fn pretty_message_includes_line_and_column() {
    use hulk_parsegen::runtime::error::ParseError;
    let e = ParseError {
        message: "raw".to_string(),
        line: 5,
        column: 12,
        found: Some("SEMICOLON".to_string()),
        expected: vec!["IDENT".to_string()],
    };
    let msg = e.pretty();
    assert!(msg.contains("[5:12]"), "got: {}", msg);
}

#[test]
fn pretty_message_translates_token_kinds() {
    use hulk_parsegen::runtime::error::ParseError;
    let e = ParseError {
        message: "raw".to_string(),
        line: 1,
        column: 1,
        found: Some("SEMICOLON".to_string()),
        expected: vec!["NUMBER".to_string(), "IDENT".to_string()],
    };
    let msg = e.pretty();
    assert!(
        msg.contains("';'"),
        "semicolon not translated, got: {}",
        msg
    );
    assert!(
        msg.contains("number"),
        "number not translated, got: {}",
        msg
    );
    assert!(
        msg.contains("identifier"),
        "ident not translated, got: {}",
        msg
    );
}

#[test]
fn recovery_error_pretty_message_is_human_readable() {
    let parser = build_functions_parser();
    let tokens = vec![
        tok_at("LBRACE", "{", 1, 1),
        tok_at("PLUS", "+", 1, 3),
        tok_at("SEMICOLON", ";", 1, 4),
        tok_at("NUMBER", "42", 1, 6),
        tok_at("RBRACE", "}", 1, 8),
        tok_at("EOF", "", 1, 9),
    ];
    let errors = parser.parse(&tokens).expect_err("should have parse errors");
    let msg = errors[0].pretty();
    // Message should have a location prefix and use human-readable token names (no raw ALL_CAPS).
    assert!(
        msg.starts_with("["),
        "should start with location, got: {}",
        msg
    );
    assert!(
        msg.contains("'+'"),
        "should translate PLUS to '+', got: {}",
        msg
    );
    assert!(
        !msg.contains("PLUS"),
        "should not contain raw PLUS, got: {}",
        msg
    );
}
