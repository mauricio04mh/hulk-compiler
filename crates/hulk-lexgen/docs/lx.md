# `.lx` DSL Reference

## Purpose

`.lx` is the small domain-specific language used by `hulk-lexgen` to describe how the HULK lexer should behave.

The goal of `.lx` is to keep lexer specifications simple, explicit, and easy to parse without requiring a full regular-expression language.

At the moment, `.lx` is used to define:

- exact keywords
- exact symbols
- identifier rules
- number rules
- string rules
- skip rules

---

## Where `.lx` Fits in the Pipeline

The `.lx` file is not HULK source code.
It is an input to the lexer generator module.

Current flow:

```text
.lx file
  -> lx lexer
  -> lx parser
  -> Vec<Rule>
  -> normalize_spec()
  -> LexerSpec
  -> runtime lexer
  -> HULK tokens
```

This means:

- the `lx` frontend reads the specification
- `spec` turns it into a normalized executable representation
- `runtime` uses that representation to tokenize HULK source code

---

## Current Rule Forms

### 1. Keyword rules

Use a `keyword` rule to declare a reserved word.

```lx
keyword let LET
keyword if IF
keyword else ELSE
```

Meaning:

- the exact text `let` becomes token `LET`
- the exact text `if` becomes token `IF`
- the exact text `else` becomes token `ELSE`

---

### 2. Symbol rules

Use a `symbol` rule for exact non-keyword tokens.

```lx
symbol ":=" ASSIGN
symbol "==" EQEQ
symbol "=" EQ
symbol "(" LPAREN
symbol ")" RPAREN
```

Meaning:

- the exact text `:=` becomes token `ASSIGN`
- the exact text `==` becomes token `EQEQ`
- the exact text `=` becomes token `EQ`

Symbols are written as string literals to make parsing unambiguous.

---

### 3. Identifier rules

Use an `ident` rule to describe how identifiers are recognized.

```lx
ident IDENT start=letter rest=letter|digit|_
```

Meaning:

- produced token kind: `IDENT`
- first character must match `letter`
- remaining characters may match `letter`, `digit`, or `_`

Currently supported character classes are:

- `letter`
- `digit`
- `_`

This rule is intentionally explicit because it defines the valid shape of identifiers instead of leaving it hardcoded in the runtime.

---

### 4. Number rules

Use a `number` rule to configure which numeric literal shapes are recognized.

```lx
number NUMBER
number NUMBER kind=int
number NUMBER kind=float
number NUMBER kind=int|float
```

Meaning:

- the runtime produces token `NUMBER` when a configured numeric literal is recognized
- `kind` controls which numeric forms are accepted
- if `kind` is omitted, the default is `int|float`

Currently supported number kinds are:

- `int`
- `float`

Current runtime behavior recognizes:

- integers as ASCII digits only
- floats as `digits '.' digits`

Example accepted literals:

```text
42
3.14
0
007
```

Current constraints:

- no sign prefix such as `-1` or `+1`
- no exponent form such as `1e9`
- no leading-dot float such as `.5`
- no trailing-dot float such as `1.`

Examples:

```lx
number NUMBER kind=int
number DECIMAL kind=float
number NUM kind=int|float
```

---

### 5. String rules

Use a `string` rule to configure string literal behavior.

```lx
string STRING
string STRING quote="'" escapes=\n|\t multiline=false
string STRING quote="\"" escapes=\"|\\|\n|\t multiline=true
```

Meaning:

- the runtime produces token `STRING` when a configured string literal is recognized
- `quote` selects the delimiter character
- `escapes` selects which escape sequences are allowed
- `multiline` controls whether raw newline characters are allowed inside the string
- if these properties are omitted, defaults are:
  - `quote="\""`
  - `escapes=\"|\\|\n|\t`
  - `multiline=false`

Current string properties:

- `quote="<char>"`
- `escapes=<escape>|<escape>|...`
- `multiline=true|false`

Currently supported escape values are:

- `\"` enables escaping the configured quote character
- `\\` enables escaping backslash
- `\n` enables newline escape
- `\t` enables tab escape

Notes:

- `quote` must be a string literal containing exactly one character
- when `quote` is `'`, the rule still uses backslash escapes and can recognize `\'` if quote escaping is enabled
- raw newlines are rejected unless `multiline=true`
- unsupported escapes raise a lexer error

Examples:

```lx
string STRING
string STRING quote="'" escapes=\n|\t multiline=false
string STRING quote="\"" escapes=\"|\\|\n|\t multiline=true
```

---

### 6. Skip rules

Use `skip` rules to define input that should be ignored by the runtime lexer.

There are two supported forms:

- legacy shorthand
- named/property form

#### Skip whitespace

```lx
skip whitespace
```

Meaning:

- all characters matched by Rust `char::is_whitespace()` are ignored

Named equivalent:

```lx
skip WHITESPACE kind=whitespace
```

#### Skip line comments

```lx
skip line_comment "//"
skip COMMENT kind=line_comment prefix="//"
```

Meaning:

- line comments starting with `//` are ignored until end of line

Named form notes:

- `kind` is required in the named form
- `prefix` is required for `line_comment`
- `prefix` must be non-empty
- `prefix` is not allowed for `whitespace`

Currently supported skip kinds are:

- `whitespace`
- `line_comment`

---

## Example Full Specification

```lx
keyword let LET
keyword in IN
keyword if IF
keyword else ELSE

symbol ":=" ASSIGN
symbol "==" EQEQ
symbol "=" EQ
symbol "(" LPAREN
symbol ")" RPAREN
symbol "{" LBRACE
symbol "}" RBRACE
symbol ";" SEMICOLON

ident IDENT start=letter rest=letter|digit|_
number NUMBER kind=int|float
string STRING quote="\"" escapes=\"|\\|\n|\t multiline=false

skip whitespace
skip line_comment "//"
```

---

## Notes on Matching Behavior

### Exact rules

Keyword and symbol rules are normalized into exact-match rules.

The normalizer sorts exact rules so the runtime can apply longest match correctly.
For example:

- `==` must be tried before `=`
- `:=` must be tried before `:`
- `@@` must be tried before `@`

### Keywords vs identifiers

Keywords are not matched as symbols directly inside longer identifiers.
The runtime first reads an identifier candidate, then checks whether the lexeme is one of the declared keywords.

That is why:

- `let` becomes `LET`
- `letter` becomes `IDENT`

---

## Current Limitations

At the moment, `.lx` is intentionally small and focused.

Current limitations include:

- no general regex support
- number rules only support `int` and `float`
- floats are limited to `digits '.' digits`
- string escape support is limited to `\"`, `\\`, `\n`, and `\t`
- skip rules only support `whitespace` and `line_comment`
- no block comment rules yet
- no Unicode-oriented character classes yet

These are acceptable limitations for the current stage of the project.

---

## When to Extend the DSL

You should extend `.lx` only when:

- you need a new category of token
- the current syntax cannot describe a valid HULK feature

Before extending `.lx`, make sure you update:

1. `lx/token.rs`
2. `lx/lexer.rs`
3. `lx/parser.rs`
4. `spec/rule.rs`
5. `spec/lexer_spec.rs`
6. `spec/normalize.rs`
7. `runtime/lexer.rs`
8. tests and `testdata/`
9. this document
