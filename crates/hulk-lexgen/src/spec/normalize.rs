use crate::spec::lexer_spec::{
    ExactRule, IdentifierRule, LexerSpec, LineCommentRule, NumberRule, StringRule,
};
use crate::spec::rule::{NumberKind, Rule, SkipKind, StringEscape};
use std::collections::HashSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecError {
    pub message: String,
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

pub fn normalize_spec(rules: &[Rule]) -> Result<LexerSpec, SpecError> {
    let mut spec = LexerSpec::default();

    let mut seen_exact_texts = HashSet::<String>::new();

    for (priority, rule) in rules.iter().enumerate() {
        match rule {
            Rule::Keyword { text, token } => {
                if text.is_empty() {
                    return Err(SpecError {
                        message: "Keyword text cannot be empty".to_string(),
                    });
                }

                if !seen_exact_texts.insert(text.clone()) {
                    return Err(SpecError {
                        message: format!("Duplicate exact rule text: '{}'", text),
                    });
                }

                spec.exact_rules.push(ExactRule {
                    text: text.clone(),
                    token: token.clone(),
                    is_keyword: true,
                    priority,
                });
            }

            Rule::Symbol { text, token } => {
                if text.is_empty() {
                    return Err(SpecError {
                        message: "Symbol text cannot be empty".to_string(),
                    });
                }

                if !seen_exact_texts.insert(text.clone()) {
                    return Err(SpecError {
                        message: format!("Duplicate exact rule text: '{}'", text),
                    });
                }

                spec.exact_rules.push(ExactRule {
                    text: text.clone(),
                    token: token.clone(),
                    is_keyword: false,
                    priority,
                });
            }

            Rule::Ident { token, start, rest } => {
                if spec.identifier.is_some() {
                    return Err(SpecError {
                        message: "Only one ident rule is allowed".to_string(),
                    });
                }

                spec.identifier = Some(IdentifierRule {
                    token: token.clone(),
                    start: start.clone(),
                    rest: rest.clone(),
                });
            }

            Rule::Number { token, kinds } => {
                if spec.number.is_some() {
                    return Err(SpecError {
                        message: "Only one number rule is allowed".to_string(),
                    });
                }

                if kinds.is_empty() {
                    return Err(SpecError {
                        message: "Number rule must declare at least one kind".to_string(),
                    });
                }

                spec.number = Some(NumberRule {
                    token: token.clone(),
                    allow_int: kinds.contains(&NumberKind::Int),
                    allow_float: kinds.contains(&NumberKind::Float),
                });
            }

            Rule::String {
                token,
                quote,
                escapes,
                multiline,
            } => {
                if spec.string.is_some() {
                    return Err(SpecError {
                        message: "Only one string rule is allowed".to_string(),
                    });
                }

                spec.string = Some(StringRule {
                    token: token.clone(),
                    quote: *quote,
                    allow_quote_escape: escapes.contains(&StringEscape::Quote),
                    allow_backslash_escape: escapes.contains(&StringEscape::Backslash),
                    allow_newline_escape: escapes.contains(&StringEscape::Newline),
                    allow_tab_escape: escapes.contains(&StringEscape::Tab),
                    multiline: *multiline,
                });
            }

            Rule::Skip { kind, prefix, .. } => match kind {
                SkipKind::Whitespace => {
                    if prefix.is_some() {
                        return Err(SpecError {
                            message: "Whitespace skip rule cannot define a prefix".to_string(),
                        });
                    }

                    if spec.skip_whitespace {
                        return Err(SpecError {
                            message: "Duplicate skip whitespace rule".to_string(),
                        });
                    }

                    spec.skip_whitespace = true;
                }
                SkipKind::LineComment => {
                    if spec.line_comment.is_some() {
                        return Err(SpecError {
                            message: "Only one line_comment skip rule is allowed".to_string(),
                        });
                    }

                    let Some(prefix) = prefix else {
                        return Err(SpecError {
                            message: "Line comment skip rule requires a prefix".to_string(),
                        });
                    };

                    if prefix.is_empty() {
                        return Err(SpecError {
                            message: "Line comment prefix cannot be empty".to_string(),
                        });
                    }

                    spec.line_comment = Some(LineCommentRule {
                        prefix: prefix.clone(),
                    });
                }
            },
        }
    }

    // Sort exact rules for "longest match":
    // 1. longer first
    // 2. if tied, lower priority first (appears earlier in the .lx file)
    spec.exact_rules.sort_by(|a, b| {
        b.text
            .len()
            .cmp(&a.text.len())
            .then(a.priority.cmp(&b.priority))
    });

    Ok(spec)
}

// ===== Unit tests start here =====
#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::rule::{CharClass, NumberKind, StringEscape};

    #[test]
    fn rejects_duplicate_exact_rule_texts() {
        // Tests duplicate exact texts are rejected during normalization.
        let rules = vec![
            Rule::Keyword {
                text: "let".to_string(),
                token: "LET".to_string(),
            },
            Rule::Symbol {
                text: "let".to_string(),
                token: "ALSO_LET".to_string(),
            },
        ];

        let error = normalize_spec(&rules).unwrap_err();

        assert_eq!(error.message, "Duplicate exact rule text: 'let'");
    }

    #[test]
    fn sorts_exact_rules_by_length_then_priority() {
        // Tests exact rules keep longest-match order and stable priority ties.
        let rules = vec![
            Rule::Keyword {
                text: "let".to_string(),
                token: "LET".to_string(),
            },
            Rule::Symbol {
                text: ":=".to_string(),
                token: "ASSIGN".to_string(),
            },
            Rule::Symbol {
                text: "->".to_string(),
                token: "ARROW".to_string(),
            },
            Rule::Symbol {
                text: "=".to_string(),
                token: "EQ".to_string(),
            },
        ];

        let spec = normalize_spec(&rules).unwrap();
        let ordered = spec
            .exact_rules
            .iter()
            .map(|rule| (rule.text.clone(), rule.priority))
            .collect::<Vec<_>>();

        assert_eq!(
            ordered,
            vec![
                ("let".to_string(), 0),
                (":=".to_string(), 1),
                ("->".to_string(), 2),
                ("=".to_string(), 3),
            ]
        );
    }

    #[test]
    fn builds_a_complete_lexer_spec() {
        // Tests normalization builds the expected lexer specification fields.
        let rules = vec![
            Rule::Keyword {
                text: "let".to_string(),
                token: "LET".to_string(),
            },
            Rule::Symbol {
                text: ":=".to_string(),
                token: "ASSIGN".to_string(),
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
                ],
                multiline: false,
            },
            Rule::Skip {
                name: "WS".to_string(),
                kind: SkipKind::Whitespace,
                prefix: None,
            },
            Rule::Skip {
                name: "COMMENT".to_string(),
                kind: SkipKind::LineComment,
                prefix: Some("#".to_string()),
            },
        ];

        let spec = normalize_spec(&rules).unwrap();

        assert_eq!(spec.exact_rules.len(), 2);
        assert_eq!(
            spec.identifier,
            Some(IdentifierRule {
                token: "IDENT".to_string(),
                start: vec![CharClass::Letter],
                rest: vec![CharClass::Letter, CharClass::Digit, CharClass::Underscore,],
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
                allow_tab_escape: false,
                multiline: false,
            })
        );
        assert!(spec.skip_whitespace);
        assert_eq!(
            spec.line_comment,
            Some(LineCommentRule {
                prefix: "#".to_string(),
            })
        );
    }
}
