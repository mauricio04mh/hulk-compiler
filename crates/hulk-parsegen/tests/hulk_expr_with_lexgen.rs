use hulk_lexgen::lx::lexer::LxLexer;
use hulk_lexgen::lx::parser::LxParser;
use hulk_lexgen::runtime::lexer::lex_hulk;
use hulk_lexgen::runtime::token::Token as LexToken;
use hulk_lexgen::spec::normalize::normalize_spec;
use hulk_parsegen::gx::parser::parse_gx;
use hulk_parsegen::runtime::cst::CstNode;
use hulk_parsegen::runtime::error::ParseError;
use hulk_parsegen::runtime::parser::RuntimeParser;
use hulk_parsegen::runtime::pratt::{Associativity, OperatorInfo, PrattConfig, PrattParser};
use hulk_parsegen::runtime::token::ParseToken;
use hulk_parsegen::spec::error::GrammarError;
use hulk_parsegen::spec::first::compute_first_sets;
use hulk_parsegen::spec::follow::compute_follow_sets;
use hulk_parsegen::spec::normalize::normalize_grammar;
use hulk_parsegen::spec::table::build_ll1_table;
use std::collections::{HashMap, HashSet};

fn read_fixture(relative_path: &str) -> String {
    let path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), relative_path);
    std::fs::read_to_string(path).expect("fixture file must exist")
}

fn load_hulk_expr_gx() -> String {
    read_fixture("testdata/grammars/hulk_expr.gx")
}

fn load_hulk_expr_lx() -> String {
    read_fixture("testdata/specs/hulk_expr.lx")
}

fn hulk_pratt_parser() -> PrattParser {
    let mut binary_ops = HashMap::new();
    binary_ops.insert(
        "ASSIGN".to_string(),
        OperatorInfo {
            precedence: 0,
            associativity: Associativity::Right,
        },
    );
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
    for op in ["EQ", "NEQ"] {
        binary_ops.insert(
            op.to_string(),
            OperatorInfo {
                precedence: 3,
                associativity: Associativity::Left,
            },
        );
    }
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
    })
}

fn hulk_pratt_stop_tokens() -> HashSet<String> {
    ["SEMICOLON", "COMMA", "RPAREN", "IN", "EOF"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn build_runtime_parser() -> RuntimeParser {
    let gx_source = load_hulk_expr_gx();
    let grammar = parse_gx(&gx_source).expect("gx grammar should parse");
    let spec = normalize_grammar(grammar).expect("grammar should normalize");
    let first = compute_first_sets(&spec);
    let follow = compute_follow_sets(&spec, &first);
    let table = build_ll1_table(&spec, &first, &follow).expect("grammar must be LL(1)");

    RuntimeParser::new(spec, table).with_pratt_hook(
        "OperatorExpr",
        hulk_pratt_parser(),
        hulk_pratt_stop_tokens(),
    )
}

fn build_lexer_spec_and_lex(source: &str) -> Vec<LexToken> {
    let lx_source = load_hulk_expr_lx();
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

fn parse_source_with_pipeline(source: &str) -> Result<CstNode, Vec<ParseError>> {
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
fn hulk_expr_grammar_is_ll1() {
    let gx_source = load_hulk_expr_gx();
    let grammar = parse_gx(&gx_source).unwrap();
    let spec = normalize_grammar(grammar).unwrap();
    let first = compute_first_sets(&spec);
    let follow = compute_follow_sets(&spec, &first);
    let table_result = build_ll1_table(&spec, &first, &follow);

    match table_result {
        Ok(_) => {}
        Err(GrammarError::Ll1Conflict { .. }) => {
            panic!("hulk_expr.gx should not produce LL(1) conflicts")
        }
        Err(other) => panic!("unexpected grammar error: {:?}", other),
    }
}

#[test]
fn parses_binary_precedence_expression() {
    let cst = parse_source_with_pipeline("1 + 2 * 3;").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_parenthesized_binary_expression() {
    let cst = parse_source_with_pipeline("(1 + 2) * 3;").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_left_associative_subtraction() {
    let cst = parse_source_with_pipeline("a - b - c;").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_right_associative_power() {
    let cst = parse_source_with_pipeline("a ^ b ^ c;").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_unary_and_comparison_expression() {
    let cst = parse_source_with_pipeline("!flag | x > 0;").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_concat_expression() {
    let cst = parse_source_with_pipeline("\"hello\" @ name;").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_call_expression_with_operator_argument() {
    let cst = parse_source_with_pipeline("print(1 + 2);").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_let_with_operator_expression() {
    let cst =
        parse_source_with_pipeline("let x = 5 + 2 * 3 in print(x);").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn parses_let_with_assign_expression() {
    let cst =
        parse_source_with_pipeline("let x = 10 in x := x - 1;").expect("parse should succeed");
    assert_root_is_program(&cst);
}

#[test]
fn reports_error_for_missing_rhs() {
    let errors = parse_source_with_pipeline("1 + ;").expect_err("parse should fail");
    let err = errors.first().expect("at least one error");
    assert_eq!(err.found, Some("SEMICOLON".to_string()));
}

#[test]
fn reports_error_for_unclosed_parenthesis() {
    let errors = parse_source_with_pipeline("(1 + 2;").expect_err("parse should fail");
    let err = errors.first().expect("at least one error");
    assert!(err.message.contains("parenthesis") || err.message.contains("closing"));
}

#[test]
fn reports_error_for_invalid_let_expr() {
    let errors = parse_source_with_pipeline("let x = 5 + in x;").expect_err("parse should fail");
    let err = errors.first().expect("at least one error");
    assert_eq!(err.found, Some("IN".to_string()));
}
