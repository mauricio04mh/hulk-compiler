use crate::runtime::error::ParseError;

pub fn terminal_display(kind: &str) -> &'static str {
    match kind {
        // keywords
        "LET" => "'let'",
        "IN" => "'in'",
        "FUNCTION" => "'function'",
        "TYPE" => "'type'",
        "PROTOCOL" => "'protocol'",
        "IF" => "'if'",
        "ELIF" => "'elif'",
        "ELSE" => "'else'",
        "WHILE" => "'while'",
        "FOR" => "'for'",
        "NEW" => "'new'",
        "SELF" => "'self'",
        "BASE" => "'base'",
        "IS" => "'is'",
        "AS" => "'as'",
        "TRUE" => "'true'",
        "FALSE" => "'false'",
        // operators
        "PLUS" => "'+'",
        "MINUS" => "'-'",
        "STAR" => "'*'",
        "SLASH" => "'/'",
        "MOD" => "'%'",
        "POW" => "'^'",
        "AT" => "'@'",
        "ATAT" => "'@@'",
        "EQ" => "'=='",
        "NEQ" => "'!='",
        "LT" => "'<'",
        "LE" => "'<='",
        "GT" => "'>'",
        "GE" => "'>='",
        "AND" => "'&'",
        "OR" => "'|'",
        "NOT" => "'!'",
        "ASSIGN" => "':='",
        "EQUAL" => "'='",
        // punctuation
        "LPAREN" => "'('",
        "RPAREN" => "')'",
        "LBRACE" => "'{'",
        "RBRACE" => "'}'",
        "LBRACKET" => "'['",
        "RBRACKET" => "']'",
        "COMMA" => "','",
        "SEMICOLON" => "';'",
        "COLON" => "':'",
        "DOT" => "'.'",
        "ARROW" => "'=>'",
        // literals
        "NUMBER" => "number",
        "STRING" => "string",
        "IDENT" => "identifier",
        // special
        "EOF" => "end of file",
        _ => "",
    }
}

/// Returns a human-readable display name for a token kind, falling back to the raw kind string.
fn display_or_raw(kind: &str) -> String {
    let mapped = terminal_display(kind);
    if mapped.is_empty() {
        kind.to_string()
    } else {
        mapped.to_string()
    }
}

impl ParseError {
    pub fn pretty(&self) -> String {
        let location = format!("[{}:{}]", self.line, self.column);
        match &self.found {
            None => format!("{} Unexpected end of input", location),
            Some(found_kind) => {
                let found_display = display_or_raw(found_kind);
                if self.expected.is_empty() {
                    format!("{} Unexpected {}", location, found_display)
                } else {
                    let expected_str = format_expected_list(&self.expected);
                    format!(
                        "{} Expected {}, found {}",
                        location, expected_str, found_display
                    )
                }
            }
        }
    }
}

fn format_expected_list(expected: &[String]) -> String {
    const MAX_DISPLAY: usize = 6;
    let mut items: Vec<String> = expected.iter().map(|k| display_or_raw(k)).collect();
    let has_more = items.len() > MAX_DISPLAY;
    items.truncate(MAX_DISPLAY);
    let suffix = if has_more { ", ..." } else { "" };
    match items.as_slice() {
        [] => String::new(),
        [single] => format!("{}{}", single, suffix),
        [first, second] => format!("{} or {}{}", first, second, suffix),
        _ => {
            let (last, rest) = items.split_last().unwrap();
            format!("{}, or {}{}", rest.join(", "), last, suffix)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_error(found: Option<&str>, expected: Vec<&str>, line: usize, col: usize) -> ParseError {
        ParseError {
            message: "test".to_string(),
            line,
            column: col,
            found: found.map(str::to_string),
            expected: expected.into_iter().map(str::to_string).collect(),
        }
    }

    #[test]
    fn pretty_formats_location() {
        let e = make_error(Some("SEMICOLON"), vec!["IDENT"], 3, 7);
        let msg = e.pretty();
        assert!(msg.starts_with("[3:7]"), "got: {}", msg);
    }

    #[test]
    fn pretty_uses_human_readable_token_names() {
        let e = make_error(Some("SEMICOLON"), vec!["IDENT"], 1, 1);
        let msg = e.pretty();
        assert!(msg.contains("';'"), "got: {}", msg);
        assert!(msg.contains("identifier"), "got: {}", msg);
    }

    #[test]
    fn pretty_handles_none_found() {
        let e = make_error(None, vec![], 2, 4);
        let msg = e.pretty();
        assert_eq!(msg, "[2:4] Unexpected end of input");
    }

    #[test]
    fn pretty_handles_multiple_expected() {
        let e = make_error(Some("RBRACE"), vec!["NUMBER", "IDENT", "LPAREN"], 1, 1);
        let msg = e.pretty();
        assert!(msg.contains("number"), "got: {}", msg);
        assert!(msg.contains("identifier"), "got: {}", msg);
        assert!(msg.contains("'('"), "got: {}", msg);
        assert!(msg.contains("'}'"), "got: {}", msg);
    }

    #[test]
    fn terminal_display_covers_keywords() {
        assert_eq!(terminal_display("LET"), "'let'");
        assert_eq!(terminal_display("IN"), "'in'");
        assert_eq!(terminal_display("FUNCTION"), "'function'");
        assert_eq!(terminal_display("EOF"), "end of file");
    }

    #[test]
    fn terminal_display_returns_empty_for_unknown() {
        assert_eq!(terminal_display("UNKNOWN_TOKEN"), "");
    }

    #[test]
    fn display_or_raw_falls_back_to_raw_kind() {
        assert_eq!(display_or_raw("UNKNOWN_TOKEN"), "UNKNOWN_TOKEN");
    }
}
