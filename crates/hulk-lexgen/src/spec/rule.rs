#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharClass {
    Letter,
    Digit,
    Underscore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberKind {
    Int,
    Float,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringEscape {
    Quote,
    Backslash,
    Newline,
    Tab,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipKind {
    Whitespace,
    LineComment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    Keyword {
        text: String,
        token: String,
    },
    Symbol {
        text: String,
        token: String,
    },

    Ident {
        token: String,
        start: Vec<CharClass>,
        rest: Vec<CharClass>,
    },

    Number {
        token: String,
        kinds: Vec<NumberKind>,
    },
    String {
        token: String,
        quote: char,
        escapes: Vec<StringEscape>,
        multiline: bool,
    },
    Skip {
        name: String,
        kind: SkipKind,
        prefix: Option<String>,
    },
}
