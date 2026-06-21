# HULK Compiler

A complete compiler for the [HULK](https://matcom.in/hulk/) programming language, written in Rust. Takes a `.hulk` source file and produces a native x86-64 Linux executable.

## Quick start

```bash
make build          # compile the compiler itself
./hulk program.hulk # compile a HULK program → ./output
./output            # run it
```

## Pipeline

```
Source (.hulk)
  → Tokens        (hulk-lexgen)
  → CST           (hulk-parsegen / Pratt)
  → AST           (hulk-frontend)
  → HIR + Types   (hulk-sema)
  → IR            (hulk-lower)
  → LLVM IR text  (hulk-codegen-llvm)
  → Binary        (clang)
```

## Language features

- **Types and literals**: `Number` (f64), `String`, `Boolean`
- **Operators**: arithmetic, comparison, logical, string concat (`@`, `@@`), assignment (`:=`)
- **Control flow**: `if`/`elif`/`else`, `while`, `for (x in iterable)`, `let`/`in`, blocks
- **Functions**: named functions, `define` macros (call-by-name), type annotations optional
- **OOP**: `type` declarations, single inheritance, `self`, `base()`, method overriding, `is`/`as`
- **Protocols**: structural conformance (`interface`/`protocol`), protocol inheritance
- **Vectors**: literals, indexing, comprehensions, `new T[N]`, 2D arrays
- **Lambdas**: anonymous functions, closures, higher-order functions
- **Pattern matching** *(additional feature)*: `match (expr) { pattern => body, ... }` — see below

## Pattern matching

Pattern matching is an additional language feature not in the original HULK spec. It provides a safe, readable alternative to chained `if`/`elif` type dispatch.

```hulk
type Shape() { area(): Number => 0; }
type Circle(r: Number) inherits Shape { area(): Number => 3 * r * r; }
type Rectangle(w: Number, h: Number) inherits Shape { area(): Number => w * h; }

let s: Shape = new Circle(5) in
print(match (s) {
    Circle as c    => "circle: " @ c.area(),
    Rectangle as r => "rect: "   @ r.area(),
    _              => "other",
});
// → "circle: 75"
```

Supported pattern kinds:

| Pattern | Example | Behaviour |
|---|---|---|
| Wildcard | `_` | matches anything; no binding |
| Literal | `42`, `"hi"`, `true` | matches exact value |
| Binding | `n` | matches anything; binds scrutinee to `n` |
| Type pattern | `Circle` | matches when `scrutinee is Circle` |
| Type pattern + narrow | `Circle as c` | matches + narrows `c: Circle` in the arm body |

The compiler enforces **exhaustiveness**: the last arm must be `_` or a binding, or a compile-time error is emitted.

## Running tests

```bash
cargo test --workspace   # unit and integration tests
make test                # end-to-end tests against reference outputs
```

## Technical report

See [REPORT.md](REPORT.md) for a full description of the architecture, every design decision, and the detailed write-up of the pattern matching implementation.
