use hulk_parsegen::grammar::Grammar;
use hulk_parsegen::production::Production;
use hulk_parsegen::runtime::cst::CstNode;
use hulk_parsegen::runtime::parser::RuntimeParser;
use hulk_parsegen::runtime::token::ParseToken;
use hulk_parsegen::spec::error::GrammarError;
use hulk_parsegen::spec::first::compute_first_sets;
use hulk_parsegen::spec::follow::compute_follow_sets;
use hulk_parsegen::spec::normalize::normalize_grammar;
use hulk_parsegen::spec::table::build_ll1_table;
use hulk_parsegen::symbol::Symbol;

fn minimal_program_grammar() -> Grammar {
    Grammar {
        start: "Program".to_string(),
        productions: vec![
            Production {
                lhs: "Program".to_string(),
                rhs: vec![Symbol::NonTerminal("Expr".to_string()), Symbol::Eof],
            },
            Production {
                lhs: "Expr".to_string(),
                rhs: vec![Symbol::Terminal("NUMBER".to_string())],
            },
        ],
    }
}

#[test]
fn normalize_accepts_basic_grammar() {
    let grammar = minimal_program_grammar();
    let spec = normalize_grammar(grammar).unwrap();

    assert_eq!(spec.start, "Program");
    assert!(spec.non_terminals.contains("Program"));
    assert!(spec.non_terminals.contains("Expr"));
    assert!(spec.terminals.contains("NUMBER"));
}

#[test]
fn normalize_rejects_empty_grammar() {
    let grammar = Grammar {
        start: "Program".to_string(),
        productions: vec![],
    };

    let err = normalize_grammar(grammar).unwrap_err();
    assert_eq!(err, GrammarError::EmptyGrammar);
}

#[test]
fn normalize_rejects_undefined_start_symbol() {
    let grammar = Grammar {
        start: "Program".to_string(),
        productions: vec![Production {
            lhs: "Expr".to_string(),
            rhs: vec![Symbol::Terminal("NUMBER".to_string())],
        }],
    };

    let err = normalize_grammar(grammar).unwrap_err();
    assert_eq!(
        err,
        GrammarError::UndefinedStartSymbol("Program".to_string())
    );
}

#[test]
fn normalize_rejects_undefined_non_terminal() {
    let grammar = Grammar {
        start: "Program".to_string(),
        productions: vec![Production {
            lhs: "Program".to_string(),
            rhs: vec![Symbol::NonTerminal("Expr".to_string()), Symbol::Eof],
        }],
    };

    let err = normalize_grammar(grammar).unwrap_err();
    assert_eq!(err, GrammarError::UndefinedNonTerminal("Expr".to_string()));
}

#[test]
fn normalize_rejects_invalid_epsilon_usage() {
    let grammar = Grammar {
        start: "A".to_string(),
        productions: vec![
            Production {
                lhs: "A".to_string(),
                rhs: vec![Symbol::Epsilon, Symbol::NonTerminal("B".to_string())],
            },
            Production {
                lhs: "B".to_string(),
                rhs: vec![Symbol::Terminal("NUMBER".to_string())],
            },
        ],
    };

    let err = normalize_grammar(grammar).unwrap_err();
    assert_eq!(
        err,
        GrammarError::InvalidEpsilonUsage {
            lhs: "A".to_string()
        }
    );
}

#[test]
fn first_for_simple_alternatives() {
    let grammar = Grammar {
        start: "Expr".to_string(),
        productions: vec![
            Production {
                lhs: "Expr".to_string(),
                rhs: vec![Symbol::Terminal("NUMBER".to_string())],
            },
            Production {
                lhs: "Expr".to_string(),
                rhs: vec![Symbol::Terminal("IDENT".to_string())],
            },
        ],
    };

    let spec = normalize_grammar(grammar).unwrap();
    let first = compute_first_sets(&spec);
    let expr_first = first.get("Expr").unwrap();

    assert!(expr_first.contains(&Symbol::Terminal("NUMBER".to_string())));
    assert!(expr_first.contains(&Symbol::Terminal("IDENT".to_string())));
}

#[test]
fn follow_puts_eof_in_start_symbol() {
    let spec = normalize_grammar(minimal_program_grammar()).unwrap();
    let first = compute_first_sets(&spec);
    let follow = compute_follow_sets(&spec, &first);

    let start_follow = follow.get("Program").unwrap();
    assert!(start_follow.contains(&Symbol::Eof));
}

#[test]
fn ll1_table_maps_basic_entries() {
    let grammar = Grammar {
        start: "Expr".to_string(),
        productions: vec![
            Production {
                lhs: "Expr".to_string(),
                rhs: vec![Symbol::Terminal("NUMBER".to_string())],
            },
            Production {
                lhs: "Expr".to_string(),
                rhs: vec![Symbol::Terminal("IDENT".to_string())],
            },
        ],
    };
    let spec = normalize_grammar(grammar).unwrap();
    let first = compute_first_sets(&spec);
    let follow = compute_follow_sets(&spec, &first);
    let table = build_ll1_table(&spec, &first, &follow).unwrap();

    assert!(table.contains_key(&("Expr".to_string(), "NUMBER".to_string())));
    assert!(table.contains_key(&("Expr".to_string(), "IDENT".to_string())));
}

#[test]
fn ll1_conflict_detection() {
    let grammar = Grammar {
        start: "Stmt".to_string(),
        productions: vec![
            Production {
                lhs: "Stmt".to_string(),
                rhs: vec![
                    Symbol::Terminal("IDENT".to_string()),
                    Symbol::Terminal("EQUAL".to_string()),
                    Symbol::Terminal("EXPR".to_string()),
                ],
            },
            Production {
                lhs: "Stmt".to_string(),
                rhs: vec![
                    Symbol::Terminal("IDENT".to_string()),
                    Symbol::Terminal("LPAREN".to_string()),
                    Symbol::Terminal("RPAREN".to_string()),
                ],
            },
        ],
    };
    let spec = normalize_grammar(grammar).unwrap();
    let first = compute_first_sets(&spec);
    let follow = compute_follow_sets(&spec, &first);
    let err = build_ll1_table(&spec, &first, &follow).unwrap_err();

    match err {
        GrammarError::Ll1Conflict {
            non_terminal,
            terminal,
            ..
        } => {
            assert_eq!(non_terminal, "Stmt");
            assert_eq!(terminal, "IDENT");
        }
        other => panic!("expected Ll1Conflict, got {:?}", other),
    }
}

#[test]
fn runtime_parser_parses_minimal_program() {
    let spec = normalize_grammar(minimal_program_grammar()).unwrap();
    let first = compute_first_sets(&spec);
    let follow = compute_follow_sets(&spec, &first);
    let table = build_ll1_table(&spec, &first, &follow).unwrap();
    let parser = RuntimeParser::new(spec, table);

    let tokens = vec![
        ParseToken::new("NUMBER", "42", 1, 1),
        ParseToken::new("EOF", "", 1, 3),
    ];

    let cst = parser.parse(&tokens).unwrap();
    assert_eq!(
        cst,
        CstNode::node(
            "Program",
            vec![
                CstNode::node(
                    "Expr",
                    vec![CstNode::Token {
                        kind: "NUMBER".to_string(),
                        lexeme: "42".to_string(),
                        line: 1,
                        column: 1,
                    }]
                ),
                CstNode::Token {
                    kind: "EOF".to_string(),
                    lexeme: "".to_string(),
                    line: 1,
                    column: 3,
                },
            ]
        )
    );
}

#[test]
fn runtime_parser_reports_expected_tokens() {
    let spec = normalize_grammar(minimal_program_grammar()).unwrap();
    let first = compute_first_sets(&spec);
    let follow = compute_follow_sets(&spec, &first);
    let table = build_ll1_table(&spec, &first, &follow).unwrap();
    let parser = RuntimeParser::new(spec, table);

    let tokens = vec![
        ParseToken::new("IDENT", "x", 1, 1),
        ParseToken::new("EOF", "", 1, 2),
    ];

    let errors = parser.parse(&tokens).unwrap_err();
    let err = errors.first().expect("at least one error");
    assert!(err.expected.contains(&"NUMBER".to_string()));
    assert_eq!(err.found, Some("IDENT".to_string()));
}
