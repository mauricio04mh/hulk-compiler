mod common;

use common::{lex_input, render_tokens};

#[test]
fn distinguishes_keywords_from_identifiers() {
    let tokens = lex_input("basic.lx", "let if else in letter iffy elsex in2 x x0 x_0").unwrap();

    assert_eq!(
        render_tokens(&tokens),
        [
            "LET",
            "IF",
            "ELSE",
            "IN",
            "IDENT(letter)",
            "IDENT(iffy)",
            "IDENT(elsex)",
            "IDENT(in2)",
            "IDENT(x)",
            "IDENT(x0)",
            "IDENT(x_0)",
            "EOF",
        ]
        .join("\n")
    );
}

#[test]
fn applies_longest_match_for_conflicting_symbols() {
    let tokens = lex_input("longest_match.lx", "a == b; a := b; name @@ last; value @ other; label: value;").unwrap();

    assert_eq!(
        render_tokens(&tokens),
        [
            "IDENT(a)",
            "EQEQ",
            "IDENT(b)",
            "SEMICOLON",
            "IDENT(a)",
            "ASSIGN",
            "IDENT(b)",
            "SEMICOLON",
            "IDENT(name)",
            "CONCAT_WS",
            "IDENT(last)",
            "SEMICOLON",
            "IDENT(value)",
            "CONCAT",
            "IDENT(other)",
            "SEMICOLON",
            "IDENT(label)",
            "COLON",
            "IDENT(value)",
            "SEMICOLON",
            "EOF",
        ]
        .join("\n")
    );
}

#[test]
fn lexes_valid_strings_and_reports_unterminated_ones() {
    let tokens = lex_input("basic.lx", "\"hola\" \"linea\\notra\"").unwrap();

    assert_eq!(
        render_tokens(&tokens),
        ["STRING(\"hola\")", "STRING(\"linea\\notra\")", "EOF"].join("\n")
    );

    let error = lex_input("basic.lx", "\"sin cerrar").unwrap_err();
    assert_eq!(error.message, "Unterminated string literal");
}

#[test]
fn skips_comments_and_whitespace() {
    let tokens =
        lex_input("basic.lx", " \tlet x = 1; // comentario\n\n  in letter;\n").unwrap();

    assert_eq!(
        render_tokens(&tokens),
        [
            "LET",
            "IDENT(x)",
            "EQ",
            "NUMBER(1)",
            "SEMICOLON",
            "IN",
            "IDENT(letter)",
            "SEMICOLON",
            "EOF",
        ]
        .join("\n")
    );
}
