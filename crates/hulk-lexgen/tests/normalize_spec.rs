mod common;

use common::{normalize_fixture, parse_rules_fixture};
use hulk_lexgen::spec::lexer_spec::{IdentifierRule, LineCommentRule, NumberRule, StringRule};
use hulk_lexgen::spec::normalize::normalize_spec;
use hulk_lexgen::spec::rule::CharClass;

#[test]
fn rejects_duplicate_symbol_texts() {
    let rules = parse_rules_fixture("duplicate_symbol.lx").unwrap();
    let error = normalize_spec(&rules).unwrap_err();

    assert_eq!(error.message, "Duplicate exact rule text: '='");
}

#[test]
fn sorts_conflicting_symbols_for_longest_match() {
    let spec = normalize_fixture("longest_match.lx").unwrap();
    let ordered = spec
        .exact_rules
        .iter()
        .map(|rule| rule.text.as_str())
        .collect::<Vec<_>>();

    assert_eq!(ordered, vec!["==", ":=", "@@", "=", ":", "@", ";"]);
}

#[test]
fn builds_expected_lexer_spec_from_basic_fixture() {
    let spec = normalize_fixture("basic.lx").unwrap();

    assert_eq!(
        spec.identifier,
        Some(IdentifierRule {
            token: "IDENT".to_string(),
            start: vec![CharClass::Letter],
            rest: vec![CharClass::Letter, CharClass::Digit, CharClass::Underscore],
        })
    );
    assert_eq!(
        spec.number,
        Some(NumberRule {
            token: "NUMBER".to_string(),
            allow_int: true,
            allow_float: true,
        })
    );
    assert_eq!(
        spec.string,
        Some(StringRule {
            token: "STRING".to_string(),
            quote: '"',
            allow_quote_escape: true,
            allow_backslash_escape: true,
            allow_newline_escape: true,
            allow_tab_escape: true,
            multiline: false,
        })
    );
    assert!(spec.skip_whitespace);
    assert_eq!(
        spec.line_comment,
        Some(LineCommentRule {
            prefix: "//".to_string(),
        })
    );
    assert!(spec.exact_rules.iter().any(|rule| {
        rule.text == "let" && rule.token == "LET" && rule.is_keyword
    }));
    assert!(spec.exact_rules.iter().any(|rule| {
        rule.text == ":=" && rule.token == "ASSIGN" && !rule.is_keyword
    }));
}
