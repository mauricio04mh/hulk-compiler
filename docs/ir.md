# HULK IR

This document defines the backend-independent intermediate representation used
after semantic analysis. The Rust data model lives in `crates/hulk-ir`; the text
format is a stable debug and golden-test format, not the source of truth.

The IR is intentionally higher level than LLVM IR. It should describe typed
HULK operations in a lowered, control-flow-explicit form without committing to a
native ABI, pointer layout, register allocation strategy, or runtime memory
model.

## Pipeline Position

```text
HULK source
  -> tokens
  -> CST
  -> AST
  -> SemanticProgram/HIR
  -> Hulk IR
  -> LLVM IR / other backend IR
  -> executable or JIT
```

`hulk-ir` is the contract between `hulk-lower` and execution-oriented compiler
stages. Backends should consume `IrProgram`; they should not depend on AST or
HIR internals.

## Responsibilities

The IR is responsible for:

- representing a whole lowered program as types, static data, functions, and an
  entry point;
- making control flow explicit with labels, jumps, branches, and returns;
- making intermediate values explicit with parameters, locals, and temporaries;
- preserving enough typed information for backend code generation;
- representing object, vector, closure, cast, and dispatch operations at a
  runtime-abstract level;
- providing deterministic textual output for golden tests.

The IR is not responsible for:

- parsing, name resolution, or type checking;
- deciding source-level overloads, member visibility, or protocol conformance;
- choosing LLVM structs, native pointer layouts, calling conventions, or ABI;
- inserting garbage collection or reference-counting code;
- performing target-specific optimization.

## Invariants

Lowering into `IrProgram` assumes semantic analysis has already guaranteed:

- all names are resolved;
- all expressions have types;
- function and method arities are valid;
- function and method argument types are valid;
- inheritance and protocol declarations are valid;
- assignment targets are valid;
- attribute privacy has already been checked;
- `self`, `base`, attributes, and methods are resolved;
- invalid HULK programs do not reach IR lowering.

Backends may assume:

- every `IrFunction` listed in `.CODE` has a stable `FunctionId`;
- `IrProgram.entry` points to the entry function id;
- every local, param, and temp used by a function is declared in that function;
- `IrPlace` is a write target and is always either a local or a temp;
- `IrValue` is a read operand;
- params are not assignment destinations;
- labels are local to their function;
- type, attribute, method slot, data, function, param, local, temp, and label ids
  are deterministic within one lowered program;
- the textual format produced by `Display` is stable enough for golden tests.

Current code does not have a dedicated IR verifier yet. Until one is added,
these invariants are enforced by `hulk-lower` tests.

## Program Shape

An `IrProgram` has three printed sections:

```text
.TYPES
  Runtime-independent type layout metadata.

.DATA
  Static constants.

.CODE
  Entry point, functions, methods, lambdas, and instruction streams.
```

Rust shape:

```rust
IrProgram {
    types: Vec<IrType>,
    data: Vec<IrData>,
    functions: Vec<IrFunction>,
    entry: FunctionId,
}
```

## Types

`IrTypeRef` is a typed contract for backend lowering:

```text
Number
String
Boolean
Object
User(name)
Vector(inner)
Iterable(inner)
Functor(params -> ret)
Unknown
```

`Unknown` should only appear when earlier phases intentionally preserve an
unknown type to avoid cascading errors. A production backend should either reject
remaining `Unknown` values or map them through a well-defined dynamic value
strategy.

## Values and Places

The IR is three-address style. Complex HULK expressions are lowered into simple
instructions using temporary values.

Read operands are `IrValue`:

```text
%t0        temporary
%l0        local
%p0        parameter
42         number literal
true       boolean literal
@s0        static data reference
null       null reference
unit       no meaningful value
```

Write targets are `IrPlace`:

```text
%t0        temporary
%l0        local
```

Parameters are read-only in this IR. If a source-level transformation needs a
mutable copy of a parameter, lowering should introduce a local and assign the
parameter into that local.

## Type Layouts

The `.TYPES` section stores logical object metadata:

```text
type Point #0 {
  attr #0 x: Number
  attr #1 y: Number
  method #0 move : Point_move
}

type PolarPoint #1 inherits Point {
  attr #2 rho: Number
  method #0 move : PolarPoint_move
}
```

Attributes use stable `AttrId` values. Method entries use stable `MethodSlot`
values so a backend can lower virtual dispatch without re-running source-level
method lookup.

This section is not an LLVM layout. A later runtime document must define how
logical attributes, method slots, and inheritance map to actual memory.

## Static Data

The `.DATA` section stores compile-time constants:

```text
data @s0 = "hello"
data @s1 = 42
data @s2 = true
```

String escaping is handled by the IR display implementation. The lowering stage
currently emits static data entries as needed; the IR does not require string
interning.

## Functions

Functions are linear instruction streams with declarations for params, locals,
and temps:

```text
function entry #0 -> Number {
  local %l0: Number name=x
  temp %t0: Number

  %l0 = 1
  %t0 = %l0 + 2
  return %t0
}
```

Function kinds:

- `Entry`: synthetic program entry function.
- `Function`: source-level global function.
- `Method`: source-level method with owner type and method name metadata.
- `Lambda`: lowered lambda body.

Method functions receive `self` as an explicit parameter when lowering produces
the method body.

## Instruction Semantics

Instruction semantics are target-independent. They describe what a backend must
implement, not how it must implement it.

### Data Movement

```text
%dst = %src
```

Copies an `IrValue` into an `IrPlace`.

### Unary and Binary Operations

```text
%dst = -%value
%dst = !%value
%dst = %left + %right
%dst = %left <= %right
```

Supported unary ops:

```text
Not, Neg
```

Supported binary ops:

```text
Add, Sub, Mul, Div, Mod, Pow,
Concat, ConcatSpace,
Eq, Neq, Lt, Le, Gt, Ge,
And, Or
```

Backends must map each operation according to the operand types produced by
semantic analysis. For example, numeric `Add` and string concatenation are not
the same runtime operation even though both are binary instructions.

### Control Flow

```text
label L0
jump L1
branch %cond ? L_then : L_else
return %value
return
```

Labels are local to a function. `branch` expects a boolean condition after
semantic checking/lowering. Backends usually translate labels to basic blocks.

### Calls

```text
%dst = call f(%a, %b)
call print(%value)
%dst = static_call Type_init(%self, %arg)
%dst = vcall %receiver.StaticType::method#slot(%arg)
%dst = base_call ParentType::method(%self, %arg)
```

Call forms:

- `Call`: global function or builtin name call.
- `StaticCall`: known function call used mainly for generated init/helper calls.
- `VirtualCall`: dynamic method dispatch through a method slot.
- `BaseCall`: parent method call from a `base(...)` expression.
- `ClosureCall`: call through a closure value.

The backend decides how builtins map to runtime functions.

### Objects and Attributes

```text
%obj = allocate Point
%value = getattr %obj, #0
setattr %obj, #1, %value
```

`Allocate` creates an uninitialized logical object. Generated init functions are
responsible for parent initialization and attribute initialization. The physical
allocation strategy belongs to the runtime/backend design.

`GetAttr` and `SetAttr` use stable logical attribute ids. Backends must map ids
to memory offsets using the runtime layout rules.

### Vectors

```text
%v = vector [%a, %b, %c]
%n = vector_len %v
vector_push %v, %value
%x = vector_get %v[%i]
vector_set %v[%i] = %value
```

Vectors are logical runtime values. The IR does not choose whether vectors are
homogeneous native arrays, boxed arrays, or generic runtime containers.

### Closures

```text
%closure = closure lambda_0 captures [%l0, %p1]
%result = closure_call %closure(%arg)
```

`MakeClosure` associates a lowered lambda function name with captured values.
The runtime/backend decides the closure environment layout.

### Type Operations

```text
%ok = type_test %value is TypeName
%casted = type_cast %value as TypeName
```

`TypeTest` produces a boolean. `TypeCast` produces a value typed as the target
type after semantic validation. Runtime failure behavior for invalid casts must
be defined by the backend/runtime design.

## Lowering Coverage

The current golden tests cover these source-level families:

- literals and static data;
- arithmetic, comparisons, boolean operations, and string concatenation;
- `let`, scope shadowing, and outer-scope assignment;
- `if`, `elif`, `while`, labels, jumps, and branches;
- global functions, recursion, and math builtins;
- object allocation, attributes, methods, inheritance, dynamic dispatch, and
  `base`;
- `is` and `as`;
- vector literals, indexing, vector generators, and `for`;
- lambdas, captures, and closure calls.

Important golden files:

```text
crates/hulk-lower/tests/golden/recursive_function.ir
crates/hulk-lower/tests/golden/10_inheritance_base.ir
crates/hulk-lower/tests/golden/12_vector_index.ir
crates/hulk-lower/tests/golden/13_vector_generator.ir
crates/hulk-lower/tests/golden/14_lambda_closure.ir
crates/hulk-lower/tests/golden/15_for_loop.ir
crates/hulk-lower/tests/golden/dynamic_dispatch_static_base.ir
crates/hulk-lower/tests/golden/big_ir_smoke.expected.ir
```

## Backend Readiness Checklist

Before implementing a backend from `IrProgram`, confirm the backend can answer:

- how `Number`, `Boolean`, and `String` map to target values;
- how `IrFunction` maps to target functions;
- how params, locals, temps, and places are represented;
- how labels, jumps, and branches map to target control flow;
- how `Call` names map to user functions versus runtime builtins;
- how static data is materialized;
- how logical object layouts map to memory;
- how method slots map to vtables or dispatch tables;
- how vectors and closures are allocated and passed;
- how `Unit`, `Null`, and `Unknown` are handled.

For an incremental LLVM backend, the smallest useful subset is:

```text
Number, Boolean,
Assign, Unary, Binary,
Label, Jump, Branch, Return,
Call for user functions and selected builtins.
```

Strings, objects, vectors, and closures should be added after a runtime ABI is
documented.

## Stability Rules

When changing `hulk-ir`:

- update this document in the same change;
- add or update `crates/hulk-ir/tests/display.rs` coverage for new display
  syntax;
- add or update `crates/hulk-lower/tests/golden/*.ir` when lowering output
  changes;
- keep backend-specific details out of `IrInstr` unless the IR boundary is
  intentionally being redesigned;
- run `cargo test -p hulk-ir` and `cargo test -p hulk-lower`.
