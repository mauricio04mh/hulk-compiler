#![allow(dead_code)]

use hulk_lexgen::lx::lexer::LxLexer;
use hulk_lexgen::lx::parser::{LxParser, ParseError};
use hulk_lexgen::runtime::lexer::{LexError as RuntimeLexError, lex_hulk};
use hulk_lexgen::runtime::token::Token;
use hulk_lexgen::spec::lexer_spec::LexerSpec;
use hulk_lexgen::spec::normalize::{SpecError, normalize_spec};
use hulk_lexgen::spec::rule::Rule;
use std::fs;
use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_fixture(group: &str, name: &str) -> String {
    fs::read_to_string(crate_root().join("testdata").join(group).join(name))
        .unwrap_or_else(|error| panic!("failed to read test fixture {group}/{name}: {error}"))
}

pub fn load_spec(name: &str) -> String {
    read_fixture("specs", name)
}

pub fn load_hulk(name: &str) -> String {
    read_fixture("hulk", name)
}

pub fn load_expected(name: &str) -> String {
    read_fixture("expected", name)
}

pub fn parse_rules_from_str(input: &str) -> Result<Vec<Rule>, ParseError> {
    let tokens = LxLexer::new(input)
        .lex_all()
        .expect("fixture .lx input should lex successfully");

    LxParser::new(tokens).parse_rules()
}

pub fn parse_rules_fixture(name: &str) -> Result<Vec<Rule>, ParseError> {
    parse_rules_from_str(&load_spec(name))
}

pub fn normalize_fixture(name: &str) -> Result<LexerSpec, SpecError> {
    let rules = parse_rules_fixture(name).expect("fixture .lx input should parse successfully");
    normalize_spec(&rules)
}

pub fn lex_input(spec_name: &str, input: &str) -> Result<Vec<Token>, RuntimeLexError> {
    let spec = normalize_fixture(spec_name).expect("fixture .lx input should normalize");
    lex_hulk(input, &spec)
}

pub fn lex_fixture(spec_name: &str, source_name: &str) -> Result<Vec<Token>, RuntimeLexError> {
    lex_input(spec_name, &load_hulk(source_name))
}

pub fn render_tokens(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(render_token)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn assert_golden(spec_name: &str, source_name: &str, expected_name: &str) {
    let tokens = lex_fixture(spec_name, source_name).expect("fixture .hulk input should lex");
    let actual = render_tokens(&tokens);
    let expected = load_expected(expected_name);
    assert_eq!(actual.trim_end(), expected.trim_end());
}

fn render_token(token: &Token) -> String {
    match token.kind.as_str() {
        "IDENT" => format!("IDENT({})", token.lexeme),
        "NUMBER" => format!("NUMBER({})", token.lexeme),
        "STRING" => format!("STRING(\"{}\")", escape_text(&token.lexeme)),
        "EOF" => "EOF".to_string(),
        _ => token.kind.clone(),
    }
}

fn escape_text(text: &str) -> String {
    text.chars().flat_map(char::escape_default).collect()
}
