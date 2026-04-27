# `hulk-lexgen`

`hulk-lexgen` is the lexer generation module of the project.

Its job is to read a `.lx` specification, transform it into a normalized internal representation, and use that representation to tokenize HULK source code.

This crate is intentionally split into three internal areas:

- `lx/`: reads the `.lx` specification language
- `spec/`: validates and normalizes parsed rules
- `runtime/`: tokenizes HULK source code using the normalized specification

---

## What This Crate Does

Current responsibilities:

1. tokenize `.lx` files
2. parse `.lx` files into `Rule` values
3. normalize those rules into `LexerSpec`
4. use `LexerSpec` to lex HULK source code

Current pipeline:

```text
.lx source
  -> lx::lexer
  -> lx::parser
  -> Vec<Rule>
  -> spec::normalize::normalize_spec
  -> LexerSpec
  -> runtime::lexer::lex_hulk
  -> Vec<Token>
```

---

## Internal Structure

```text
src/
  lib.rs
  lx/
    mod.rs
    token.rs
    lexer.rs
    parser.rs
  spec/
    mod.rs
    rule.rs
    lexer_spec.rs
    normalize.rs
  runtime/
    mod.rs
    token.rs
    lexer.rs
```

### `lx/`

This is the frontend for the `.lx` DSL.

#### `lx/token.rs`
Defines token kinds, `Span`, and `Token` for the `.lx` language.

#### `lx/lexer.rs`
Turns raw `.lx` text into `.lx` tokens.

#### `lx/parser.rs`
Turns `.lx` tokens into `Vec<Rule>`.

---

### `spec/`

This area holds the semantic representation of the lexer specification.

#### `spec/rule.rs`
Defines the parsed rule model.
Examples:

- `Rule::Keyword`
- `Rule::Symbol`
- `Rule::Ident`
- `Rule::Number`
- `Rule::String`
- `Rule::SkipWhitespace`
- `Rule::SkipLineComment`

#### `spec/lexer_spec.rs`
Defines the normalized representation used by the runtime.
This is the executable specification of the lexer.

#### `spec/normalize.rs`
Validates and converts `Vec<Rule>` into `LexerSpec`.
This is where duplicate checks and exact-rule ordering happen.

---

### `runtime/`

This area uses `LexerSpec` to lex real HULK source code.

#### `runtime/token.rs`
Defines final runtime tokens produced for HULK source.

#### `runtime/lexer.rs`
Implements the runtime lexer:

- skip ignored input
- match exact rules
- lex strings
- lex numbers
- lex identifiers
- resolve keywords
- emit final tokens

---

## Design Notes

### Why `.lx` is a DSL instead of regex-based input

The current design uses a small DSL instead of general regular expressions because the project is focused on the HULK language and on clarity of implementation.

This keeps the system:

- easier to parse
- easier to debug
- easier to evolve incrementally

### Why `Rule` and `LexerSpec` are separate

`Rule` represents the parsed meaning of the `.lx` file.

`LexerSpec` represents the normalized executable form of that specification.

This separation is useful because:

- parsing concerns stay isolated from runtime concerns
- validation and normalization have a clear place to live
- the runtime does not depend on syntax-level details of the DSL

### Why exact rules are sorted

The runtime must support longest match for symbols.

That means rules such as:

- `==` before `=`
- `:=` before `:`
- `@@` before `@`

This ordering is handled during normalization.

### Why keywords are resolved after identifier scanning

A word such as `let` has the shape of an identifier.
The runtime first scans an identifier candidate, then checks whether it matches a declared keyword.

This prevents incorrect partial matches inside longer identifiers.

---

## Current Development Status

Current supported concepts:

- exact keywords
- exact symbols
- identifier shapes through `start` and `rest`
- simple numbers
- simple strings
- whitespace skipping
- line comment skipping

Current limitations:

- no regex input language
- no Unicode-oriented classes yet

---

## How to Run Tests for This Crate

From the workspace root:

```bash
cargo test -p hulk-lexgen
```

Run a specific integration test:

```bash
cargo test -p hulk-lexgen --test runtime_lex
```

Run a specific test by name:

```bash
cargo test -p hulk-lexgen parse_ident_rule
```

---

## Testing Strategy

This crate uses three levels of testing.

### 1. Unit tests

Unit tests live in the same `.rs` file as the code they test.

Typical examples:

- `.lx` token recognition
- `.lx` parser rules
- normalization checks
- runtime matching behavior

### 2. Integration tests

Integration tests live in:

```text
tests/
```

These test the public API of the crate.
Typical integration tests cover:

- `.lx -> Rule[]`
- `Rule[] -> LexerSpec`
- `LexerSpec + HULK source -> runtime tokens`
- end-to-end crate behavior

### 3. Test data

Fixtures live in:

```text
testdata/
```

Typical structure:

```text
testdata/
  specs/
  hulk/
  expected/
```

Use `testdata/` for:

- `.lx` fixtures
- HULK input samples
- expected token output files
