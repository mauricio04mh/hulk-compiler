use crate::spec::rule::CharClass;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactRule {
    pub text: String,
    pub token: String,
    pub is_keyword: bool,
    pub priority: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifierRule {
    pub token: String,
    pub start: Vec<CharClass>,
    pub rest: Vec<CharClass>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberRule {
    pub token: String,
    pub allow_int: bool,
    pub allow_float: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringRule {
    pub token: String,
    pub quote: char,
    pub allow_quote_escape: bool,
    pub allow_backslash_escape: bool,
    pub allow_newline_escape: bool,
    pub allow_tab_escape: bool,
    pub multiline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineCommentRule {
    pub prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LexerSpec {
    pub exact_rules: Vec<ExactRule>,
    pub identifier: Option<IdentifierRule>,
    pub number: Option<NumberRule>,
    pub string: Option<StringRule>,
    pub skip_whitespace: bool,
    pub line_comment: Option<LineCommentRule>,
}
