mod common;

use common::parse_rules_fixture;
use hulk_lexgen::spec::rule::{CharClass, NumberKind, Rule, SkipKind, StringEscape};

#[test]
fn parses_basic_spec_into_expected_rules() {
    let rules = parse_rules_fixture("basic.lx").unwrap();

    assert_eq!(
        rules,
        vec![
            Rule::Keyword {
                text: "let".to_string(),
                token: "LET".to_string(),
            },
            Rule::Keyword {
                text: "if".to_string(),
                token: "IF".to_string(),
            },
            Rule::Keyword {
                text: "else".to_string(),
                token: "ELSE".to_string(),
            },
            Rule::Keyword {
                text: "in".to_string(),
                token: "IN".to_string(),
            },
            Rule::Symbol {
                text: ":=".to_string(),
                token: "ASSIGN".to_string(),
            },
            Rule::Symbol {
                text: "==".to_string(),
                token: "EQEQ".to_string(),
            },
            Rule::Symbol {
                text: "=".to_string(),
                token: "EQ".to_string(),
            },
            Rule::Symbol {
                text: "(".to_string(),
                token: "LPAREN".to_string(),
            },
            Rule::Symbol {
                text: ")".to_string(),
                token: "RPAREN".to_string(),
            },
            Rule::Symbol {
                text: ";".to_string(),
                token: "SEMICOLON".to_string(),
            },
            Rule::Ident {
                token: "IDENT".to_string(),
                start: vec![CharClass::Letter],
                rest: vec![CharClass::Letter, CharClass::Digit, CharClass::Underscore],
            },
            Rule::Number {
                token: "NUMBER".to_string(),
                kinds: vec![NumberKind::Int, NumberKind::Float],
            },
            Rule::String {
                token: "STRING".to_string(),
                quote: '"',
                escapes: vec![
                    StringEscape::Quote,
                    StringEscape::Backslash,
                    StringEscape::Newline,
                    StringEscape::Tab,
                ],
                multiline: false,
            },
            Rule::Skip {
                name: "WHITESPACE".to_string(),
                kind: SkipKind::Whitespace,
                prefix: None,
            },
            Rule::Skip {
                name: "COMMENT".to_string(),
                kind: SkipKind::LineComment,
                prefix: Some("//".to_string()),
            },
        ]
    );
}

#[test]
fn rejects_ident_rule_without_rest_classes() {
    let error = parse_rules_fixture("invalid_ident_rule.lx").unwrap_err();

    assert!(error.message.contains("Expected KwRest"));
    assert_eq!(error.span.line, 1);
}
