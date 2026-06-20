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

Semantic errors are reported with exit code `3` as `(line,col) SEMANTIC: description`.

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

Math builtins (`sqrt`, `sin`, `cos`, `exp`, `log`, `pow`, `rand`) wrap the corresponding C standard library functions.

---

## Runtime Library

The C runtime (`runtime/hulk_runtime.c`) provides the low-level implementations of all runtime operations declared as external functions in the LLVM IR.

Key design decisions:

- **Uniform pointer layout**: All heap objects start with `HulkVTable*`, enabling type-agnostic vtable lookup.
- **Attribute storage as `int64_t[]`**: Avoids struct-level type information at runtime; typed accessors handle the cast.
- **Vectors as iterables**: `HulkVector` has a vtable with `next` at slot 0 and `current` at slot 1, so vectors can be used in any `for` loop accepting an iterable.
- **Ranges as iterables**: `HulkRange` follows the same pattern, representing a floating-point sequence.
- **Closures with flexible array member**: `HulkClosure` uses a C99 flexible array member for the capture array to avoid a double-indirection.

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

**String operations**: concatenation, equality, automatic conversion of numbers and booleans to strings for printing

**Builtins**: `print`, `sqrt`, `sin`, `cos`, `exp`, `log`, `pow`, `rand`, `range`, `PI`, `E`

---

## Known Limitations

**No generic type parameters**: The `Vector` and `Iterable` types carry element types in the compiler's type system, but user-defined types cannot be parameterized (no `type List<T>`).

**Single inheritance**: Multiple inheritance is not supported. Protocol conformance provides interface polymorphism instead.

**Immutable `let` bindings**: Variables bound with `let` cannot be reassigned. Only assignment targets (declared with `:=`) are mutable.

**No exception handling**: There is no `try`/`catch` mechanism. Runtime errors (bounds checks, type cast failures) call `exit(1)` through the C runtime.

**No module system**: All declarations share a single global namespace. There are no imports or namespaces.

**No pattern matching**: Destructuring and match expressions are not supported.

**Closure capture by value**: Captured variables are copied at closure creation time. Modifications inside the lambda do not affect the outer scope.

**Limited compile-time evaluation**: `define` macros provide syntactic inline expansion but no compile-time computation or hygienic variable capture.

---

## Conclusion

This compiler implements a complete, working compilation pipeline for HULK, from source text to native x86-64 binaries. The design prioritizes clear phase separation, allowing each stage to be developed and tested independently. The multi-pass semantic inference engine enables expressive, lightly-annotated programs while maintaining strong type safety. The vtable-based object model, implemented with a uniform `HulkVTable*`-first layout shared by user types, vectors, and ranges, provides efficient runtime polymorphism without a separate class descriptor or boxed type hierarchy.
