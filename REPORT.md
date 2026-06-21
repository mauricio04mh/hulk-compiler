# HULK Compiler — Technical Report

## Overview

This document describes the architecture, design decisions, implemented features, and known limitations of a complete compiler for the HULK programming language, implemented in Rust. The compiler takes a HULK source file and produces a native x86-64 Linux executable by lowering through several intermediate representations down to LLVM IR, which is then assembled and linked against a thin C runtime library.

The overall compilation pipeline is:

```
Source (.hulk)
  → Tokens        (hulk-lexgen)
  → CST           (hulk-parsegen)
  → AST           (hulk-frontend)
  → HIR + Types   (hulk-sema)
  → IR            (hulk-lower)
  → LLVM IR text  (hulk-codegen-llvm)
  → Binary        (clang)
```

The project is organized as a Cargo workspace with eight crates, each with a focused responsibility. This separation allows each phase to evolve independently and makes it easy to test phases in isolation.

---

## Crate Structure

| Crate | Role |
|---|---|
| `hulk-lexgen` | Lexer specification framework and runtime scanner |
| `hulk-parsegen` | LL(1) parser generator with Pratt parser hook |
| `hulk-frontend` | Integrates lexer/parser, builds typed AST |
| `hulk-sema` | Multi-pass semantic analysis, type inference, HIR |
| `hulk-ir` | Intermediate representation types and instructions |
| `hulk-lower` | HIR → IR lowering, vtable layout, protocol slots |
| `hulk-codegen-llvm` | LLVM IR text emission |
| `hulk-driver` | CLI entry point, error reporting, pipeline orchestration |

---

## Lexical Analysis

The lexer is implemented as a **generator framework** (`hulk-lexgen`). Lexer specifications are written in `.lx` files that define rules as regular expressions and token kinds. At startup the specification is parsed and normalized, and the resulting scanner processes source characters at runtime via `lex_hulk(source, spec)`, producing a `Vec<Token>`.

Each `Token` carries:
- **kind** — the token category (e.g. `NUMBER`, `IDENT`, `STRING`, `IF`, `LBRACE`)
- **lexeme** — the exact source substring
- **line** and **column** — 1-based position for error reporting

The full set of tokens covers all HULK constructs: numeric and string literals, identifiers, all arithmetic and logical operators (`+`, `-`, `*`, `/`, `%`, `**`), string concatenation operators (`@`, `@@`), comparison and equality operators, boolean literals (`true`, `false`), all keywords (`let`, `in`, `if`, `elif`, `else`, `while`, `for`, `function`, `define`, `type`, `inherits`, `protocol`, `interface`, `extends`, `new`, `self`, `base`, `is`, `as`), delimiters, and special arrows (`->` for function bodies, `=>` for functor types).

Lexical errors are detected and reported with precise position information. The exit code for lexical errors is `1`, and messages are formatted as `(line,col) LEXICAL: description` and written to stderr.

---

## Parsing

The parser (`hulk-parsegen` + `hulk-frontend`) uses a **hybrid LL(1) + Pratt** strategy. This design separates concerns cleanly: top-level structure (declarations, control flow, type annotations) is handled by a generated LL(1) table-driven parser, while expression-level operator precedence is handled by a Pratt (top-down operator precedence) parser.

### LL(1) Grammar

The grammar is defined in `.gx` files. The parser generator computes FIRST and FOLLOW sets for all non-terminals and builds an LL(1) parse table. At runtime a `RuntimeParser` drives the parse using this table.

The grammar covers:
- **Declarations**: `FunctionDecl`, `TypeDecl`, `ProtocolDecl`
- **Type annotations**: `TypeExpr` supports simple types (`Number`), iterable types (`Number*`), vector types (`Number[]`, `Number[][]`), and functor types (`(Number, Number) => Number`)
- **Statements and expressions**: `IfExpr`, `WhileExpr`, `ForExpr`, `LetExpr`, `BlockExpr`, and `OperatorExpr` (the Pratt entry point)

The `TypeExprSuffix` rule is right-recursive, enabling multi-dimensional array types like `Number[][]` in type annotations.

### Pratt Parser

When the LL(1) parser reaches an `OperatorExpr`, control transfers to the Pratt parser. Operators are assigned binding powers:

| Power | Operators |
|---|---|
| 0 (right) | `:=` assignment |
| 1 | `or` |
| 2 | `and` |
| 3 | `==`, `!=` |
| 4 | `<`, `<=`, `>`, `>=` |
| 5 | `@`, `@@` (string concat) |
| 6 | `+`, `-` |
| 7 | `*`, `/`, `%` |
| 8 (right) | `**` (power) |

Infix position handles member access (`.`), indexing (`[i]`), and function calls (`(args)`). Prefix handles unary `not`, `-`, `+`. Special primary parsers handle `new`, `self`, `base`, `if`, `while`, `for`, `let`, `function` (lambda), block `{...}`, and vector literals `[...]`.

The `new T[N]` and `new T[][N]` syntax for allocating sized arrays is handled in the Pratt parser's `new` expression path. When an empty `[]` is detected after the type identifier, the element type is promoted to a `TypeVector` node before parsing the actual size expression from the following `[N]`.

### Concrete Syntax Tree → AST

The frontend translates the Concrete Syntax Tree (CST) produced by the parsegen into a typed `Ast` via the `Builder` module. The resulting `Program` contains a list of `Decl` values (functions, types, protocols) and a single entry expression.

Syntactic errors are reported with exit code `2` as `(line,col) SYNTACTIC: description`.

---

## Semantic Analysis

The semantic analysis phase (`hulk-sema`) is the most complex part of the compiler. It performs multi-pass type inference, scope resolution, protocol conformance checking, and produces a typed High-level IR (HIR).

### Type System

The `Type` enum models the HULK type hierarchy:

- **Primitive types**: `Number` (IEEE 754 double), `String`, `Boolean`
- **Object**: top of the user-defined type hierarchy
- **UserType(name)**: a user-defined class or protocol type
- **Vector(inner)**: a resizable array of `inner` elements
- **Iterable(inner)**: any type that supports `next()` and `current()` iteration
- **Functor { params, ret }**: a first-class function (lambda) type
- **Unknown**: placeholder for unresolved types during inference

Subtyping is structural: `Unknown` is assignable from anything, `Object` is assignable from any `UserType`, and inheritance is modeled by walking the parent chain in the `TypeRegistry`.

### Type Registry

Before analysis begins, `TypeRegistry::build()` performs a declaration-collection pass. It reads all type and protocol declarations and records:

- Constructor parameter types for each type
- Attribute declarations and their types
- Method signatures (parameter types and return type) for each type and protocol
- Inheritance relationships and protocol hierarchy

A special case handles **passthrough inheritance**: when a child type declares `inherits Parent` without explicit constructor arguments, the parent's constructor parameters flow into the child automatically. This allows deep inheritance chains to work without repeating constructor parameters at every level.

Circular inheritance is detected and reported as a semantic error.

### Multi-Pass Type Inference

The `HirBuilder` runs in a fixpoint loop, repeating analysis until no signatures change. This enables **interprocedural type inference** across several dimensions:

1. **Call-site inference**: When a function `f` is called with a concrete argument (e.g., `f(42)`), the type `Number` propagates into `f`'s parameter even if `f` has no type annotation.

2. **Method parameter inference**: Method parameters without annotations are inferred from how the method is called at each concrete call site in the program.

3. **Body constraint inference**: The types of expressions within a method or function body constrain the parameter types. For example, if a parameter `x` is used as `x + 1`, it must be `Number`.

4. **Return type inference**: If no return type is declared, the return type is inferred from the last expression in the body.

The loop terminates when `signature_changed` remains false for a full pass, meaning all types have stabilized.

### Scope and Symbol Resolution

The scope system uses a `ScopeStack` that models lexical nesting. Each scope level maps names to `Symbol` values. `SymbolKind` distinguishes builtin functions/types/constants, user-defined functions, types, protocols, local variables, parameters, and the special `self` and `base` bindings.

A pre-pass collects all declaration names before resolving bodies, allowing forward references between functions and types.

Detected semantic errors include: undeclared identifiers, duplicate declarations, wrong argument count, invalid assignment targets, type mismatches in return types and assignments, use of `self` outside a class context, circular inheritance, inheriting from undefined types, and using non-iterable values in `for` loops.

The three most user-visible error variants — `UndefinedVariable`, `UndefinedFunction`, and `UndefinedMethod` — carry a `Span { line, col }` sourced from the AST node that triggered the error. When a non-zero span is available the error message appends `at line L:C`, e.g.:

```
SEMANTIC error: Undefined variable 'x' at line 3:9
SEMANTIC error: Undefined function 'foo' at line 7:1
SEMANTIC error: Undefined method 'bar' for type 'MyType' at line 12:5
```

Semantic errors are reported with exit code `3` as `SEMANTIC error: description`.

### Protocol Conformance

Protocols (also declared with the `interface` keyword) define method signatures. Types conform to a protocol implicitly if they declare all required methods with compatible signatures — no explicit `implements` declaration is needed.

Protocol hierarchy is supported (`protocol B extends A { ... }`), and protocol types can be used as parameter/variable types to enable polymorphism. When a method call is made on a protocol-typed receiver, the call is emitted as a virtual dispatch.

### High-Level IR (HIR)

The HIR is a fully typed, resolved representation of the program. Every `HirExpr` carries a `Type`, a source `Span`, and a `HirId`. The `HirExprKind` variants cover all language constructs with dispatch decisions already resolved:

- **DispatchKind**: `Static` (concrete type, call function label directly), `Virtual` (protocol receiver, look up slot at runtime), `Base` (call parent's implementation)
- **HirAssignTarget**: `Local` (variable), `SelfAttribute` (instance field), `VectorIndex` (array element)
- **HirCallee**: `Builtin`, `GlobalFunction`, `LocalFunctor` (lambda stored in a variable)

---

## IR and Lowering

### Intermediate Representation

The `hulk-ir` crate defines a flat, three-address IR suitable for code generation. An `IrProgram` contains:

- **`types`**: User-defined types with their attributes (`AttrId`-indexed) and methods (`MethodSlot`-indexed with function labels)
- **`data`**: Static constants (strings, numbers, booleans)
- **`functions`**: All functions — entry point, user functions, type methods, and lambda functions

Each `IrFunction` has explicit lists of parameters, locals, and temporaries, plus a flat instruction list. Control flow is explicit: `Label`, `Jump`, and `Branch` instructions.

### HIR → IR Lowering

The `hulk-lower` crate converts HIR to IR with several important transformations:

**Type layout construction**: `ensure_type_layout()` computes the attribute layout for each type respecting inheritance (parent attributes first, then child-specific attributes). Methods are assigned `MethodSlot` indices contiguously.

**Protocol slot assignment**: The `populate_protocol_slots()` function pre-populates method slot indices from protocol declarations before type layouts are finalized. A protocol with `N` methods gets slots `0..N-1`. A child protocol `C extends P` gets slots `0..N-1` from `P` plus new slots `N..` for its own methods. This guarantees that the slot indices used for virtual dispatch on protocol-typed receivers match the actual vtable layout of any conforming concrete type.

**Synthetic initializers**: For each user type, a synthetic `init_<TypeName>(self, constructor_params)` function is generated. It calls the parent's initializer with appropriate arguments, then sets the type's attribute values from their initializer expressions.

**Lambda lowering**: Lambda expressions are lifted to synthetic global functions. Free variables (captured from the enclosing scope) are passed as extra parameters and stored in a `HulkClosure` struct. `MakeClosure` and `ClosureCall` IR instructions handle allocation and indirect invocation.

**Vector generators**: `[expr | var in iterable]` comprehensions are lowered to a loop that allocates a vector, iterates the source via `next()`/`current()`, and pushes each computed result.

---

## LLVM Code Generation

The `hulk-codegen-llvm` crate emits LLVM IR as text, which is then compiled and linked by `clang`.

### Object Model

All runtime objects — user type instances, vectors, ranges — share a common layout: a `HulkVTable*` pointer as the first field, followed by `int64_t` slots for attributes. This uniform layout allows `hulk_object_method(obj, slot)` to work on any object without knowing its concrete type.

Typed attribute accessors (`hulk_object_get_number`, `hulk_object_set_string`, etc.) use `memcpy` to union-cast between `double`/`int8_t`/pointer and `int64_t`, avoiding undefined behavior.

### Vtable Layout

For each user-defined type, the codegen emits a global constant `@vtable_<TypeName>` of type `%HulkVTable = type { i64, ptr, i64, ptr }` (type_id, parent vtable pointer, method count, methods array). The methods array is a global array of function pointers sorted by slot index.

Vtables are linked by inheritance: the `parent` field of a child's vtable points to the parent's vtable. This chain enables `hulk_object_is(obj, target_type_id)` to walk up the hierarchy at runtime.

### Virtual Dispatch

When emitting a `VirtualCall` instruction, the codegen:
1. Calls `hulk_object_method(receiver, slot)` to retrieve the function pointer from the vtable
2. Infers the concrete function signature from the receiver's static type and method name
3. Emits an indirect call through the retrieved pointer

For protocol-typed receivers (where the static type is not in the concrete type map), a generic dispatch path infers argument and return types directly from the IR values and emits a call with those types.

For iterable receivers (ranges, vectors, user generators), a dedicated dispatch path handles the known slot 0 (`next`) and slot 1 (`current`) signatures.

### Closures

`MakeClosure` emits:
1. A call to `hulk_closure_alloc(fn_ptr, num_captures)` to allocate the closure struct
2. A sequence of `hulk_closure_set_capture` calls to store each captured value

`ClosureCall` emits a load of the function pointer from the closure, then calls it with captured values prepended to the argument list.

### String and Math Operations

String operations delegate to runtime functions (`hulk_string_concat`, `hulk_string_equals`, `hulk_string_from_number`, `hulk_string_from_bool`). The `print` builtin dispatches based on argument type: `hulk_print_number`, `hulk_print_bool`, `hulk_print_string`.

Math builtins wrap C standard library or runtime implementations:

| HULK builtin | Arity | Runtime function |
|---|---|---|
| `sqrt` | 1 | `hulk_sqrt` |
| `sin` | 1 | `hulk_sin` |
| `cos` | 1 | `hulk_cos` |
| `exp` | 1 | `hulk_exp` |
| `log` | 2 | `hulk_log` |
| `pow` | 2 | `hulk_pow` |
| `rand` | 0 | `hulk_rand` |
| `abs` | 1 | `hulk_abs` |
| `floor` | 1 | `hulk_floor` |
| `ceil` | 1 | `hulk_ceil` |
| `round` | 1 | `hulk_round` |
| `min` | 2 | `hulk_min` |
| `max` | 2 | `hulk_max` |

String method calls on a `String`-typed receiver are routed to dedicated runtime functions: `length()`/`size()` call `hulk_string_len` (result is converted from `i64` to `double`); `substring(start, end)` converts its `Number` arguments to `i64` and calls `hulk_string_sub`.

---

## Runtime Library

The C runtime (`runtime/hulk_runtime.c`) provides the low-level implementations of all runtime operations declared as external functions in the LLVM IR.

Key design decisions:

- **Uniform pointer layout**: All heap objects start with `HulkVTable*`, enabling type-agnostic vtable lookup.
- **Attribute storage as `int64_t[]`**: Avoids struct-level type information at runtime; typed accessors handle the cast.
- **Vectors as iterables**: `HulkVector` has a vtable with `next` at slot 0 and `current` at slot 1, so vectors can be used in any `for` loop accepting an iterable.
- **Ranges as iterables**: `HulkRange` follows the same pattern, representing a floating-point sequence.
- **Closures with flexible array member**: `HulkClosure` uses a C99 flexible array member for the capture array to avoid a double-indirection.
- **Arena allocator**: All heap allocations go through a 64 MB bump-pointer arena (`hulk_arena_alloc`), eliminating fragmentation and `free` overhead for compiler test programs.

### Runtime Safety Checks

The runtime performs the following safety checks at runtime and aborts with a diagnostic message on violation:

- **Division and modulo by zero**: The codegen emits an `fcmp oeq double divisor, 0.0` guard before every `/` and `%` operation. A branch to an error block calls `hulk_abort("division by zero")` before the division executes.
- **Null object method call**: `hulk_object_method` checks that the receiver pointer is non-null before loading the vtable. A null receiver prints `"hulk runtime error: method call on null object"` and exits.
- **VTable slot bounds**: `hulk_object_method` also validates that the requested slot is within `[0, method_count)`. An out-of-range slot reports the slot number and method count.
- **Null dereference on attribute access**: All eight typed attribute accessors (`hulk_object_get_number`, `hulk_object_set_string`, etc.) call `check_obj_not_null` before the field offset arithmetic.
- **Type cast failure**: `hulk_object_as` verifies the object is not null before calling `hulk_object_is`; an invalid cast returns null rather than silently misinterpreting memory.
- **Vector index out of bounds**: `hulk_vector_get` and `hulk_vector_set` reject negative indices and indices ≥ `len`, printing the index and vector length before aborting.

All fatal errors are reported to stderr via `hulk_abort(const char* msg)` and exit with code 1.

---

## Implemented Language Features

The compiler fully supports the following HULK language features:

**Core types and literals**: `Number` (double), `String`, `Boolean`, `true`, `false`

**Operators**: arithmetic (`+`, `-`, `*`, `/`, `%`, `**`), comparison (`==`, `!=`, `<`, `<=`, `>`, `>=`), logical (`and`, `or`, `not`), string concatenation (`@`, `@@`), assignment (`:=`)

**Control flow**: `if`/`elif`/`else` expressions, `while` loops, `for (var in iterable)` loops, `let`/`in` bindings, block expressions `{ ... }`

**Functions**: named function declarations with optional type annotations, `define` macros (call-by-name), multi-pass interprocedural type inference for unannotated parameters

**Object-oriented programming**: type declarations with constructor parameters, single inheritance (`inherits`), attribute declarations, instance methods, `self` references, `base()` calls to parent constructors and methods, method overriding, type tests (`is`), type casts (`as`)

**Protocols and interfaces**: structural conformance (implicit), protocol inheritance (`extends`), virtual dispatch on protocol-typed receivers, protocol method slots derived from declaration order

**Arrays and vectors**: vector type annotations (`Number[]`, `Number[][]`), vector literals `[e1, e2, ...]`, indexed access `v[i]` (read and write), 2D arrays `new Number[][N]`, sized allocation `new T[N]`, initialized allocation `new T[N] { init }`, vector comprehensions `[expr | x in iterable]`, `size` method

**Iterables**: `Iterable(T)` type, ranges via `range(start, end)`, user-defined iterables by implementing `next(): Boolean` and `current(): T` methods

**Lambdas and closures**: anonymous function expressions, first-class functor values, closure capture of free variables, higher-order functions

**String operations**: concatenation (`@`, `@@`), equality, automatic conversion of numbers and booleans to strings for printing, `length()`/`size()` (returns string length as `Number`), `substring(start, end)` (returns a new string slice)

**Builtins**: `print`, `sqrt`, `sin`, `cos`, `exp`, `log`, `pow`, `rand`, `range`, `PI`, `E`, `abs`, `floor`, `ceil`, `round`, `min`, `max`

**Pattern matching** *(additional feature)*: `match (expr) { pattern => body, ... }` with wildcard (`_`), literal (`42`, `"s"`, `true`/`false`), binding (`n`), type-pattern (`TypeName`), and type-pattern-with-narrowing (`TypeName as x`) arms. Exhaustiveness checking at compile time ensures every `match` has a catch-all arm. See the dedicated section above for full details.

---

## Additional Feature: Pattern Matching

Pattern matching was implemented as the compiler's additional language feature — a construct not present in the HULK specification but common in modern functional and hybrid languages (Rust, Swift, Kotlin, Scala, OCaml, Haskell). It extends HULK with a `match` expression that dispatches on the *shape* or *identity* of a value, making polymorphic code considerably more readable and safer than nested `if`/`elif`/`else` chains.

### Why pattern matching fits HULK

HULK already has a rich type hierarchy with single inheritance, protocols, `is` type tests, and `as` type casts. In practice, code that works with heterogeneous object graphs — a Shape that might be a Circle, Rectangle, or Triangle; an AST node that might be a Literal, BinaryExpr, or LetExpr — needs to dispatch on the concrete type at runtime. Before pattern matching, the idiomatic way to write this in HULK was:

```hulk
if (s is Circle) {
    let c = s as Circle in c.area()
} elif (s is Rectangle) {
    let r = s as Rectangle in r.area()
} else {
    0
};
```

This pattern is verbose, error-prone (the `as` cast must be repeated manually after the `is` test), and offers no compiler feedback if a case is forgotten. Pattern matching addresses all three problems.

### Syntax

```
match (scrutinee) {
    PatternA            => body_a,
    PatternB as x       => body_b,
    42                  => body_c,
    _                   => body_default,
}
```

The `match` expression evaluates `scrutinee` exactly once and tests each arm from top to bottom. The first arm whose pattern matches determines the result. The overall type of the `match` expression is the least common ancestor of all arm body types.

### Pattern kinds

| Pattern | Syntax | Matches when |
|---|---|---|
| **Wildcard** | `_` | always; no binding |
| **Literal** | `42`, `"hello"`, `true` | scrutinee equals the literal value |
| **Binding** | `n` (lowercase identifier) | always; binds the scrutinee to `n` in the arm body |
| **Type pattern** | `Circle` (uppercase identifier) | `scrutinee is Circle` at runtime |
| **Type pattern with bind** | `Circle as c` | same as above; additionally narrows `c: Circle` in the arm body, giving access to `Circle`-specific methods without an explicit cast |

### Exhaustiveness checking

The compiler requires that the last arm of every match is either a wildcard (`_`) or a binding pattern. If it is not, a semantic error `NonExhaustiveMatch` is emitted at compile time:

```
SEMANTIC error: Non-exhaustive match: missing a wildcard or binding arm for scrutinee of type 'Number'
```

This guarantees that every match expression has a defined result for all possible inputs, preventing silent `undefined`-style bugs that unguarded type dispatch can introduce.

### Concrete example

```hulk
type Shape() {
    area(): Number => 0;
}
type Circle(r: Number) inherits Shape {
    area(): Number => 3 * r * r;
}
type Rectangle(w: Number, h: Number) inherits Shape {
    area(): Number => w * h;
}

function describe(s: Shape): String =>
    match (s) {
        Circle as c   => "circle, area=" @ c.area(),
        Rectangle as r => "rect, area=" @ r.area(),
        _             => "unknown shape",
    };
```

The compiler automatically narrows `c` to `Circle` and `r` to `Rectangle` inside their respective arms, so `c.area()` resolves to `Circle.area` at the type level and can be dispatched statically.

### Implementation design

**Desugaring strategy.** Rather than introducing new HIR, IR, lowering, or codegen nodes, `Expr::Match` is desugared directly to existing HIR constructs inside the HIR builder:

1. The scrutinee is stored in a fresh synthetic variable `_match_scr_N` (impossible to name in user code, since HULK identifiers cannot start with `_`).
2. Each conditional arm (TypePattern or Literal) becomes a `(condition, body)` pair in a `HirExprKind::If` branch vector.
3. A TypePattern condition is `HirExprKind::TypeTest { expr: _scr, type_name }`.
4. A TypePattern-with-bind body is wrapped in a `HirExprKind::Let` that casts the scrutinee: `let x: TypeName = _scr as TypeName in body`.
5. A Literal condition is `HirExprKind::Binary { op: Eq, left: _scr, right: literal }`.
6. The last wildcard or binding arm becomes the `else_branch` of the `HirExprKind::If`.

Because `match` fully resolves to `Let` + `If` + `TypeTest` + `TypeCast` + `Binary` nodes — all of which the lowering and codegen already handle — the entire pass pipeline (`hulk-lower`, `hulk-ir`, `hulk-codegen-llvm`) required **zero changes**.

**Parser integration.** The keyword `match` and the wildcard symbol `_` are added to all three lexer specs (`.lx` files). The grammar files (`.gx`) expose `OperatorExpr -> MATCH ;`, which causes the LL(1) table-driven parser to invoke the Pratt hook whenever it sees `MATCH` in expression position. The Pratt parser's `parse_match_subexpr` function then consumes `(scrutinee) { arms }` entirely.

Pattern disambiguation inside `parse_match_pattern` follows a simple rule derived from HULK's own naming conventions: a token whose first character is uppercase is a TypePattern (since HULK type names are `TitleCase`); a lowercase-first identifier is a Binding; `_` (lexed as a distinct `WILDCARD` token) is a wildcard; and numeric, string, or boolean literals are literal patterns.

**Scope hygiene.** Each arm body gets its own scope. Binding variables and type-pattern binds are defined in that scope only and do not leak to adjacent arms or the surrounding expression.

**Interaction with type inference.** The result type of the whole match is computed by unifying all arm body types using the same `unify_types` (least common ancestor) function used for `if` branches. This means a match over type patterns whose arms return objects of different sibling types automatically produces the common parent type.

### Files changed

| File | Change |
|---|---|
| `crates/hulk-frontend/src/ast.rs` | New types: `LiteralPattern`, `Pattern`, `MatchArm`; new `Expr::Match` variant |
| `crates/hulk-parsegen/testdata/specs/hulk_{control,types,full}.lx` | `keyword match MATCH`, `symbol "_" WILDCARD` |
| `crates/hulk-parsegen/testdata/grammars/hulk_{control,types,full}.gx` | `OperatorExpr -> MATCH ;` |
| `crates/hulk-parsegen/src/runtime/pratt.rs` | `match_kw`, `wildcard` fields in `PrattConfig`; `parse_match_subexpr`, `parse_match_pattern` |
| `crates/hulk-frontend/src/lib.rs` | Wire `match_kw`, `wildcard` in `hulk_pratt_parser()` |
| `crates/hulk-frontend/src/builder.rs` | `build_match_expr`, `build_match_arm`, `build_pattern` |
| `crates/hulk-sema/src/resolver.rs` | `Expr::Match` arm; scope push/pop for arm bindings |
| `crates/hulk-sema/src/checker.rs` | `Expr::Match` arm in `infer_expr`; scope management for arm bindings |
| `crates/hulk-sema/src/hir_builder.rs` | `analyze_match`, `build_arm_cond_and_body`; `Expr::Match` in `substitute_expr` |
| `crates/hulk-sema/src/error.rs` | `SemanticError::NonExhaustiveMatch` |
| `crates/hulk-sema/tests/match_patterns.rs` | 12 unit tests covering all pattern kinds and exhaustiveness |

---

## Known Limitations

**No generic type parameters**: The `Vector` and `Iterable` types carry element types in the compiler's type system, but user-defined types cannot be parameterized (no `type List<T>`).

**Single inheritance**: Multiple inheritance is not supported. Protocol conformance provides interface polymorphism instead.

**Immutable `let` bindings**: Variables bound with `let` cannot be reassigned. Only assignment targets (declared with `:=`) are mutable.

**No exception handling**: There is no `try`/`catch` mechanism. Runtime errors (bounds checks, type cast failures) call `exit(1)` through the C runtime.

**No module system**: All declarations share a single global namespace. There are no imports or namespaces.

**Closure capture by value**: Captured variables are copied at closure creation time. Modifications inside the lambda do not affect the outer scope.

**Limited compile-time evaluation**: `define` macros provide syntactic inline expansion but no compile-time computation or hygienic variable capture.

---

## Conclusion

This compiler implements a complete, working compilation pipeline for HULK, from source text to native x86-64 binaries. The design prioritizes clear phase separation, allowing each stage to be developed and tested independently. The multi-pass semantic inference engine enables expressive, lightly-annotated programs while maintaining strong type safety. The vtable-based object model, implemented with a uniform `HulkVTable*`-first layout shared by user types, vectors, and ranges, provides efficient runtime polymorphism without a separate class descriptor or boxed type hierarchy.
