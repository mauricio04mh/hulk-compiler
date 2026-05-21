use hulk_parsegen::gx::lexer::lex_gx;
use hulk_parsegen::gx::parser::parse_gx;
use hulk_parsegen::gx::token::GxTokenKind;
use hulk_parsegen::runtime::parser::RuntimeParser;
use hulk_parsegen::runtime::token::ParseToken;
use hulk_parsegen::spec::first::compute_first_sets;
use hulk_parsegen::spec::follow::compute_follow_sets;
use hulk_parsegen::spec::normalize::normalize_grammar;
use hulk_parsegen::spec::table::build_ll1_table;
use hulk_parsegen::symbol::Symbol;

#[test]
fn gx_lexer_recognizes_start_ident_arrow_pipe_semicolon() {
    let tokens = lex_gx("%start Program\nExpr -> NUMBER | IDENT ;").unwrap();
    let kinds = tokens.into_iter().map(|t| t.kind).collect::<Vec<_>>();

    assert_eq!(
        kinds,
        vec![
            GxTokenKind::StartDirective,
            GxTokenKind::Ident("Program".to_string()),
            GxTokenKind::Ident("Expr".to_string()),
            GxTokenKind::Arrow,
            GxTokenKind::Ident("NUMBER".to_string()),
            GxTokenKind::Pipe,
            GxTokenKind::Ident("IDENT".to_string()),
            GxTokenKind::Semicolon,
            GxTokenKind::EndOfFile,
        ]
    );
}

#[test]
fn gx_parser_parses_minimal_grammar() {
    let grammar = parse_gx(
        "%start Program
Program -> Expr EOF ;
Expr -> NUMBER ;
",
    )
    .unwrap();

    assert_eq!(grammar.start, "Program");
    assert_eq!(grammar.productions.len(), 2);
}

#[test]
fn gx_parser_expands_alternatives() {
    let grammar = parse_gx(
        "%start Expr
Expr -> NUMBER | IDENT ;
",
    )
    .unwrap();

    assert_eq!(grammar.productions.len(), 2);
    assert_eq!(grammar.productions[0].lhs, "Expr");
    assert_eq!(grammar.productions[1].lhs, "Expr");
}

#[test]
fn gx_parser_supports_epsilon() {
    let grammar = parse_gx(
        "%start ArgList
ArgList -> epsilon ;
",
    )
    .unwrap();

    assert_eq!(grammar.productions.len(), 1);
    assert_eq!(grammar.productions[0].rhs, vec![Symbol::Epsilon]);
}

#[test]
fn gx_pipeline_integrates_with_spec_and_runtime() {
    let grammar = parse_gx(
        "%start Program
Program -> Expr EOF ;
Expr -> NUMBER ;
",
    )
    .unwrap();

    let spec = normalize_grammar(grammar).unwrap();
    let first = compute_first_sets(&spec);
    let follow = compute_follow_sets(&spec, &first);
    let table = build_ll1_table(&spec, &first, &follow).unwrap();
    let parser = RuntimeParser::new(spec, table);

    let tokens = vec![
        ParseToken::new("NUMBER", "42", 1, 1),
        ParseToken::new("EOF", "", 1, 3),
    ];

    let cst = parser.parse(&tokens);
    assert!(cst.is_ok());
}
