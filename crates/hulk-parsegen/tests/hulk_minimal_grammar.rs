use hulk_parsegen::gx::parser::parse_gx;
use hulk_parsegen::runtime::cst::CstNode;
use hulk_parsegen::runtime::parser::RuntimeParser;
use hulk_parsegen::runtime::token::ParseToken;
use hulk_parsegen::spec::error::GrammarError;
use hulk_parsegen::spec::first::compute_first_sets;
use hulk_parsegen::spec::follow::compute_follow_sets;
use hulk_parsegen::spec::normalize::normalize_grammar;
use hulk_parsegen::spec::table::build_ll1_table;

fn tok(kind: &str, lexeme: &str) -> ParseToken {
    ParseToken::new(kind, lexeme, 1, 1)
}

fn load_hulk_minimal_gx() -> String {
    let path = format!(
        "{}/testdata/grammars/hulk_minimal.gx",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).expect("hulk_minimal.gx fixture must exist")
}

fn build_runtime_parser() -> RuntimeParser {
    let gx_source = load_hulk_minimal_gx();
    let grammar = parse_gx(&gx_source).expect("gx grammar should parse");
    let spec = normalize_grammar(grammar).expect("grammar should normalize");
    let first = compute_first_sets(&spec);
    let follow = compute_follow_sets(&spec, &first);
    let table = build_ll1_table(&spec, &first, &follow).expect("grammar must be LL(1)");
    RuntimeParser::new(spec, table)
}

fn assert_root_is_program(cst: &CstNode) {
    match cst {
        CstNode::Node { name, .. } => assert_eq!(name, "Program"),
        _ => panic!("root CST node must be Program node"),
    }
}

#[test]
fn hulk_minimal_grammar_is_ll1() {
    let gx_source = load_hulk_minimal_gx();
    let grammar = parse_gx(&gx_source).unwrap();
    let spec = normalize_grammar(grammar).unwrap();
    let first = compute_first_sets(&spec);
    let follow = compute_follow_sets(&spec, &first);
    let table_result = build_ll1_table(&spec, &first, &follow);

    match table_result {
        Ok(_) => {}
        Err(GrammarError::Ll1Conflict { .. }) => {
            panic!("hulk_minimal.gx should not produce LL(1) conflicts")
        }
        Err(other) => panic!("unexpected grammar error: {:?}", other),
    }
}

#[test]
fn parses_number_expression_with_semicolon() {
    let parser = build_runtime_parser();
    let tokens = vec![tok("NUMBER", "42"), tok("SEMICOLON", ";"), tok("EOF", "")];
    let cst = parser.parse(&tokens).expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_identifier_expression_with_semicolon() {
    let parser = build_runtime_parser();
    let tokens = vec![tok("IDENT", "x"), tok("SEMICOLON", ";"), tok("EOF", "")];
    let cst = parser.parse(&tokens).expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_call_expression_with_semicolon() {
    let parser = build_runtime_parser();
    let tokens = vec![
        tok("IDENT", "print"),
        tok("LPAREN", "("),
        tok("NUMBER", "42"),
        tok("RPAREN", ")"),
        tok("SEMICOLON", ";"),
        tok("EOF", ""),
    ];
    let cst = parser.parse(&tokens).expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_let_expression_with_single_binding() {
    let parser = build_runtime_parser();
    let tokens = vec![
        tok("LET", "let"),
        tok("IDENT", "x"),
        tok("EQUAL", "="),
        tok("NUMBER", "42"),
        tok("IN", "in"),
        tok("IDENT", "print"),
        tok("LPAREN", "("),
        tok("IDENT", "x"),
        tok("RPAREN", ")"),
        tok("SEMICOLON", ";"),
        tok("EOF", ""),
    ];
    let cst = parser.parse(&tokens).expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_let_expression_with_multiple_bindings() {
    let parser = build_runtime_parser();
    let tokens = vec![
        tok("LET", "let"),
        tok("IDENT", "x"),
        tok("EQUAL", "="),
        tok("NUMBER", "42"),
        tok("COMMA", ","),
        tok("IDENT", "y"),
        tok("EQUAL", "="),
        tok("IDENT", "x"),
        tok("IN", "in"),
        tok("IDENT", "print"),
        tok("LPAREN", "("),
        tok("IDENT", "y"),
        tok("RPAREN", ")"),
        tok("SEMICOLON", ";"),
        tok("EOF", ""),
    ];
    let cst = parser.parse(&tokens).expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_parenthesized_expression_with_semicolon() {
    let parser = build_runtime_parser();
    let tokens = vec![
        tok("LPAREN", "("),
        tok("NUMBER", "42"),
        tok("RPAREN", ")"),
        tok("SEMICOLON", ";"),
        tok("EOF", ""),
    ];
    let cst = parser.parse(&tokens).expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn reports_error_for_missing_expr_in_let_binding() {
    let parser = build_runtime_parser();
    let tokens = vec![
        tok("LET", "let"),
        tok("IDENT", "x"),
        tok("EQUAL", "="),
        tok("IN", "in"),
        tok("IDENT", "x"),
        tok("SEMICOLON", ";"),
        tok("EOF", ""),
    ];

    let errors = parser.parse(&tokens).expect_err("parse should fail");
    let err = errors.first().expect("at least one error");
    assert_eq!(err.found, Some("IN".to_string()));
    assert!(!err.expected.is_empty());
}

#[test]
fn reports_error_for_unfinished_call_expression() {
    let parser = build_runtime_parser();
    let tokens = vec![
        tok("IDENT", "print"),
        tok("LPAREN", "("),
        tok("SEMICOLON", ";"),
        tok("EOF", ""),
    ];

    let errors = parser.parse(&tokens).expect_err("parse should fail");
    let err = errors.first().expect("at least one error");
    assert_eq!(err.found, Some("SEMICOLON".to_string()));
    assert!(!err.expected.is_empty());
}
