mod common;

use common::assert_golden;

#[test]
fn lexes_hello_program_from_basic_spec() {
    assert_golden("basic.lx", "hello.hulk", "hello.tokens");
}

#[test]
fn lexes_keywords_vs_identifiers_program_from_basic_spec() {
    assert_golden(
        "basic.lx",
        "keywords_vs_identifiers.hulk",
        "keywords_vs_identifiers.tokens",
    );
}

#[test]
fn lexes_symbol_conflicts_from_longest_match_spec() {
    assert_golden("longest_match.lx", "symbols.hulk", "symbols.tokens");
}

#[test]
fn lexes_strings_and_escapes_from_basic_spec() {
    assert_golden("basic.lx", "strings.hulk", "strings.tokens");
}

#[test]
fn lexes_comments_and_whitespace_from_basic_spec() {
    assert_golden("basic.lx", "comments.hulk", "comments.tokens");
}
