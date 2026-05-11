use hulk_lexgen::lx::lexer::LxLexer;
use hulk_lexgen::lx::parser::LxParser;
use hulk_lexgen::runtime::lexer::lex_hulk;
use hulk_lexgen::runtime::token::Token as LexToken;
use hulk_lexgen::spec::normalize::normalize_spec;
use hulk_parsegen::gx::parser::parse_gx;
use hulk_parsegen::runtime::cst::CstNode;
use hulk_parsegen::runtime::error::ParseError;
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

fn read_fixture(relative_path: &str) -> String {
    let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), relative_path);
    std::fs::read_to_string(path).expect("fixture file must exist")
}

fn load_hulk_minimal_gx() -> String {
    read_fixture("testdata/grammars/hulk_minimal.gx")
}

fn load_hulk_minimal_lx() -> String {
    read_fixture("testdata/specs/hulk_minimal.lx")
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

fn build_lexer_spec_and_lex(source: &str) -> Vec<LexToken> {
    let lx_source = load_hulk_minimal_lx();
    let lx_tokens = LxLexer::new(&lx_source)
        .lex_all()
        .expect("lx spec should lex successfully");
    let rules = LxParser::new(lx_tokens)
        .parse_rules()
        .expect("lx spec should parse successfully");
    let spec = normalize_spec(&rules).expect("lx spec should normalize");
    lex_hulk(source, &spec).expect("hulk source should lex successfully")
}

fn adapt_tokens(lex_tokens: &[LexToken]) -> Vec<ParseToken> {
    lex_tokens
        .iter()
        .map(|token| ParseToken {
            kind: token.kind.clone(),
            lexeme: token.lexeme.clone(),
            line: token.line,
            column: token.column,
        })
        .collect()
}

fn parse_source_with_pipeline(source: &str) -> Result<CstNode, ParseError> {
    let parser = build_runtime_parser();
    let lex_tokens = build_lexer_spec_and_lex(source);
    let parse_tokens = adapt_tokens(&lex_tokens);
    parser.parse(&parse_tokens)
}

fn assert_root_is_program(cst: &CstNode) {
    match cst {
        CstNode::Node { name, .. } => assert_eq!(name, "Program"),
        _ => panic!("root CST node must be Program node"),
    }
}

#[test]
fn integration_grammar_is_ll1_without_conflicts() {
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
fn integration_parses_number_source() {
    let cst = parse_source_with_pipeline("42;").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn integration_parses_ident_source() {
    let cst = parse_source_with_pipeline("x;").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn integration_parses_call_source() {
    let cst = parse_source_with_pipeline("print(42);").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn integration_parses_let_single_binding_source() {
    let cst = parse_source_with_pipeline("let x = 42 in print(x);").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn integration_parses_let_multiple_bindings_source() {
    let cst =
        parse_source_with_pipeline("let x = 42, y = x in print(y);").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn integration_parses_parenthesized_source() {
    let cst = parse_source_with_pipeline("(42);").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn integration_parses_true_source() {
    let cst = parse_source_with_pipeline("true;").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn integration_parses_false_source() {
    let cst = parse_source_with_pipeline("false;").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn integration_reports_error_for_invalid_let_source() {
    let err = parse_source_with_pipeline("let x = in x;").expect_err("parse should fail");
    assert_eq!(err.found, Some("IN".to_string()));
    assert!(!err.expected.is_empty());
}

#[test]
fn integration_reports_error_for_unfinished_call_source() {
    let err = parse_source_with_pipeline("print(;").expect_err("parse should fail");
    assert_eq!(err.found, Some("SEMICOLON".to_string()));
    assert!(!err.expected.is_empty());
}

#[test]
fn integration_adapter_maps_lexgen_token_to_parse_token() {
    let mapped = adapt_tokens(&[LexToken {
        kind: "NUMBER".to_string(),
        lexeme: "42".to_string(),
        start: 0,
        end: 2,
        line: 1,
        column: 1,
    }]);

    assert_eq!(mapped, vec![tok("NUMBER", "42")]);
}
