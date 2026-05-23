# Parser Frontend: LL(1) + Pratt · Error Recovery · Typed AST

> Implements the complete parse pipeline for HULK — from grammar specification
> to a typed AST — with multi-error recovery, human-readable diagnostics, and
> a cached pipeline per grammar variant.

---

## Overview

```
Source code
    └─► Lexer (hulk-lexgen)
            └─► LL(1) parser  ──── Pratt sub-parser (expressions)
                    └─► CST
                            └─► AST Builder
                                    └─► Program (typed AST)
```

---

## What's in this PR

### Grammar & lexer spec variants

Five `.gx` + `.lx` pairs covering progressive language subsets:

| Variant | Declarations | Control flow | Types & protocols |
|---|:---:|:---:|:---:|
| `hulk_expr` | — | — | — |
| `hulk_functions` | `function` | — | — |
| `hulk_control` | `function` | `if` / `while` / `for` | — |
| `hulk_types` | `function` | `if` / `while` / `for` | `type` / `protocol` |
| `hulk_full` | `function` | `if` / `while` / `for` | `type` / `protocol` |

---

### Error recovery — `CstNode::Error`

A new `Error` variant on the CST lets the parser insert a placeholder when a
sub-tree fails, so all remaining errors are collected before returning.

Five independent recovery points with distinct sync strategies:

| Point | Sync to (not consumed unless noted) |
|---|---|
| Declaration | `function` / `type` / `protocol` / EOF |
| Statement in block | `;` / `}` / EOF |
| Parameter | `,` / `)` / EOF |
| Type member | `;` **(consumed)** / `}` / EOF |
| Let binding | `,` / `in` / EOF |

---

### Human-readable diagnostics — `diagnostics.rs`

```
[3:12] Expected identifier or '(', found ';'
[5:1]  Expected '}', found end of file
```

- `ParseError::pretty()` formats `[line:col] Expected X, found Y`
- `terminal_display()` maps all token kinds to readable symbols
- Expected list capped at **6 items** with `", ..."` overflow

---

### Extended Pratt expression parser — `pratt.rs`

Full HULK expression support on top of the existing binary-op engine:

| Feature | Syntax |
|---|---|
| Lambda | `(x: T, y: T) => body` |
| Call / method chain | `f(a)` · `obj.method(a)` |
| OOP primitives | `new T(args)` · `self` · `base` |
| Vector literal | `[a, b, c]` |
| Vector generator | `[expr \|\| x in iterable]` |
| Index | `v[i]` |
| Type test / cast | `x is T` · `x as T` |
| Functor type annotation | `(T1, T2) -> T3` |
| Unary prefix | `!x` · `-x` |

**Hardening applied:**
- All `.expect()` panics → `Err(ParseError { … })`
- Recursion depth guard: `MAX_PARSE_DEPTH = 512`
- `Arc<PrattConfig>` eliminates per-call config clones

---

### `hulk-frontend` crate

| Module | Responsibility |
|---|---|
| `ast.rs` | Full typed AST for all HULK declarations and expressions |
| `builder.rs` | CST → AST lowering, handles grammar-based and Pratt-generated type nodes |
| `error.rs` | `FrontendError` + `ParseErrorList` — preserves all parse errors through `thiserror` |
| `lib.rs` | One `OnceLock<CachedPipeline>` per grammar variant — built once, reused forever |

---

## Test plan

- [ ] `cargo test --workspace` passes with 0 failures
- [ ] Recovery: single and multiple errors in blocks, params, type members, and let bindings report the correct count and position
- [ ] Functor types: `(T) -> T` in lambda params and return types produce `TypeRef::Functor` in the AST
- [ ] Diagnostics: `pretty()` output contains `[line:col]` and no raw `ALL_CAPS` token kinds

---

🤖 Generated with [Claude Code](https://claude.com/claude-code)
