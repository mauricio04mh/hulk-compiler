# HULK Runtime ABI

This document defines how backend-generated LLVM IR talks to the HULK runtime.
It is the contract between `hulk-codegen-llvm` and the runtime support library.

The goal is to keep `hulk-ir` target-independent while giving the LLVM backend a
clear mapping for values, functions, builtins, heap objects, vectors, strings,
closures, and dynamic dispatch.

## Pipeline Position

```text
HULK source
  -> AST
  -> SemanticProgram/HIR
  -> Hulk IR
  -> LLVM IR
  -> runtime ABI calls
  -> executable/JIT
```

LLVM IR handles primitive arithmetic, comparisons, branches, calls, and stack
slots. The runtime handles operations that are part of HULK semantics but not
native LLVM concepts: printing, strings, heap allocation, vectors, objects,
vtables, closures, type tests, casts, and runtime failures.

## Implementation Strategy

The first implementation should use a small C runtime linked with generated
LLVM IR:

```bash
clang output.ll runtime/hulk_runtime.c -o program
./program
```

Later, the runtime can move to Rust or be split into multiple C/Rust files, but
the exported ABI names should remain stable.

## LLVM Type Mapping

The LLVM backend lowers `IrTypeRef` as follows:

```text
HULK / IR type        LLVM ABI type
------------------------------------------
Number               double
Boolean              i1 inside generated code, i8 at runtime ABI boundaries
String               ptr to %HulkString
Object               ptr to %HulkObject
User(T)              ptr to %HulkObject-compatible T allocation
Vector(T)            ptr to %HulkVector
Iterable(T)          ptr to %HulkObject for now
Functor(...)         ptr to %HulkClosure
Unknown              backend error
Unit                 ptr null / ignored result depending on context
Null                 null pointer of the expected pointer type
```

`Boolean` should use `i1` for branches and primitive comparisons. Runtime
functions should accept/return `i8` for C ABI simplicity; generated LLVM inserts
`zext i1 to i8` or `icmp ne i8 %x, 0` as needed.

`Object` is the common erased pointer type for heap values. User objects,
strings, vectors, and closures may have specialized runtime structs, but values
that flow through `Object` are passed as object-compatible pointers.

## Backend Minimum Subset

The first LLVM backend only needs this subset:

```text
Number, Boolean
functions
params, locals, temps
Assign, Unary, Binary
Label, Jump, Branch, Return
Call to user functions
Call to selected builtins: print, sqrt, sin, cos, exp, log, rand
```

For this subset:

- `Number` maps to `double`;
- `Boolean` maps to `i1`;
- `print(Number)` calls `hulk_print_number`;
- `print(Boolean)` calls `hulk_print_bool`;
- math builtins call runtime wrappers or LLVM/libm-compatible functions.

Static string literals, `print(String)`, and string concatenation are supported
in the current phase. Vectors, objects, virtual dispatch, closures, and type
operations can be added after the minimum backend emits valid LLVM and runs
simple programs.

## Function ABI

Each HULK IR function lowers to one LLVM function.

Name mapping:

```text
IR function name     LLVM function name
--------------------------------------
entry                @hulk_entry
fact                 @fact
Point_init           @Point_init
Point_getX           @Point_getX
lambda_0             @lambda_0
```

The generated module also defines a C-compatible `main`:

```llvm
define i32 @main() {
entry:
  call <entry-ret> @hulk_entry()
  ret i32 0
}
```

If `hulk_entry` returns `Object` or another non-primitive value, `main` ignores
the result unless the source program explicitly printed it.

Parameters and return values use the LLVM type mapping above. Method functions
receive `self` as their first explicit parameter because `hulk-lower` already
models `self` explicitly in IR.

## Locals, Temps, and SSA

The LLVM backend may implement IR locals and temps in either of two ways:

1. Direct SSA values when the place has a single definition.
2. `alloca` slots plus `load`/`store` when a local/temp is assigned across
   branches or loops.

The first backend should use `alloca` for all `IrPlace` values. This is simpler
and correct. Later an optimization pass or mem2reg can clean it up.

Example:

```text
%t0 = %p0 + 1
```

can lower to:

```llvm
%add = fadd double %p0, 1.0
store double %add, ptr %t0.addr
```

Reads from `%t0` lower to `load double, ptr %t0.addr`.

## Control Flow ABI

IR labels map to LLVM basic blocks local to the current function:

```text
label L1
jump L2
branch %cond ? L3 : L4
```

lowers to:

```llvm
L1:
  br label %L2

  br i1 %cond, label %L3, label %L4
```

Generated LLVM must ensure every basic block ends with a terminator:

```text
br
ret
unreachable
```

## Builtin ABI

The source-level builtin signatures remain as defined by semantic analysis.
The runtime ABI uses more specific functions:

```text
HULK builtin call        Runtime function
-----------------------------------------------------
print(Number)           void @hulk_print_number(double)
print(Boolean)          void @hulk_print_bool(i8)
print(String)           void @hulk_print_string(ptr)
Concat                  ptr @hulk_string_concat(ptr, ptr)
ConcatSpace             ptr @hulk_string_concat_space(ptr, ptr)
print(Object/User)      not implemented yet
sqrt(Number)            double @hulk_sqrt(double)
sin(Number)             double @hulk_sin(double)
cos(Number)             double @hulk_cos(double)
exp(Number)             double @hulk_exp(double)
log(Number, Number)     double @hulk_log(double, double)
rand()                  double @hulk_rand()
range(Number, Number)   ptr @hulk_range_new(double, double)
```

Although HULK models `print` as returning `Object`, the first backend should
treat it as returning `null Object` after emitting the side-effecting runtime
call. This matches current programs that use `print` mainly for side effects.

Minimum C declarations:

```c
void hulk_print_number(double value);
void hulk_print_bool(unsigned char value);
void hulk_print_string(struct HulkString* value);
struct HulkString* hulk_string_concat(struct HulkString* left, struct HulkString* right);
struct HulkString* hulk_string_concat_space(struct HulkString* left, struct HulkString* right);

double hulk_sqrt(double value);
double hulk_sin(double value);
double hulk_cos(double value);
double hulk_exp(double value);
double hulk_log(double base, double value);
double hulk_rand(void);
```

## Static Data ABI

`IrDataValue::Number` and `IrDataValue::Boolean` can be emitted as LLVM constants
directly.

`IrDataValue::String` should lower to a global byte array plus a `HulkString`
descriptor:

```llvm
@str_0_data = private unnamed_addr constant [6 x i8] c"hello\00"
@str_0 = private unnamed_addr constant %HulkString { i64 5, ptr @str_0_data }
```

Generated uses of `@s0` become a pointer to the descriptor, not a pointer to the
raw bytes.

## String ABI

Runtime representation:

```c
typedef struct HulkString {
    long long len;
    const char* data;
} HulkString;
```

For the current backend phase, the runtime only needs printing support for
static string literals:

```c
void hulk_print_string(HulkString* value);
```

Static strings are immutable and do not need to be freed in the first
implementation. Concatenation results are heap allocated, but there is no GC or
free strategy yet. Equality remains future work.

## Object ABI

All heap objects begin with a common header:

```c
typedef struct HulkVTable HulkVTable;

typedef struct HulkObject {
    HulkVTable* vtable;
} HulkObject;
```

Each user type has a generated concrete layout whose first field is compatible
with `HulkObject`:

```c
typedef struct Point {
    HulkVTable* vtable;
    double x;
    double y;
} Point;
```

Inheritance must preserve prefix layout:

```c
typedef struct ColoredPoint {
    HulkVTable* vtable;
    double x;
    double y;
    HulkString* color;
} ColoredPoint;
```

This lets a child pointer be used where a parent pointer is expected.

Allocation ABI:

```c
HulkObject* hulk_alloc_object(long long type_id, long long size_bytes);
```

Generated LLVM may also emit direct `malloc` calls initially, but the preferred
ABI is a runtime allocation wrapper so garbage collection or diagnostics can be
added later.

## Attribute ABI

`AttrId` is a logical field id from Hulk IR. The LLVM backend maps it to a byte
offset or a generated struct field index using `.TYPES`.

```text
GetAttr object, #id -> load from computed field
SetAttr object, #id -> store into computed field
```

The runtime ABI does not need generic `getattr`/`setattr` functions for normal
compiled code. The backend should generate direct field access once layouts are
known.

## VTable and Dispatch ABI

Runtime vtable representation:

```c
typedef void* HulkMethodPtr;

typedef struct HulkVTable {
    long long type_id;
    HulkVTable* parent;
    long long method_count;
    HulkMethodPtr* methods;
} HulkVTable;
```

`MethodSlot` from Hulk IR indexes into `methods`.

```text
VirtualCall receiver.StaticType::method#slot(args)
```

lowers to:

```text
load receiver->vtable
load vtable->methods[slot]
cast function pointer to expected signature
call function pointer(receiver, args...)
```

`StaticCall` and `BaseCall` lower to direct LLVM calls. For `BaseCall`, the
backend calls the known parent method function, bypassing the receiver vtable.

## Vector ABI

Initial generic representation:

```c
typedef enum HulkValueTag {
    HULK_VALUE_NUMBER,
    HULK_VALUE_BOOL,
    HULK_VALUE_OBJECT,
    HULK_VALUE_STRING,
} HulkValueTag;

typedef struct HulkValue {
    HulkValueTag tag;
    union {
        double number;
        unsigned char boolean;
        HulkObject* object;
        HulkString* string;
    } as;
} HulkValue;

typedef struct HulkVector {
    long long len;
    long long capacity;
    HulkValue* data;
} HulkVector;
```

Required operations:

```c
HulkVector* hulk_vector_new(long long capacity);
void hulk_vector_push(HulkVector* vector, HulkValue value);
long long hulk_vector_len(HulkVector* vector);
HulkValue hulk_vector_get(HulkVector* vector, long long index);
void hulk_vector_set(HulkVector* vector, long long index, HulkValue value);
```

This generic representation is simple and supports mixed object values. A later
optimization can specialize `Vector(Number)` into packed `double` arrays.

LLVM backend responsibility:

- box primitive values into `HulkValue` before vector insertion;
- unbox `HulkValue` after vector reads according to the statically known vector
  element type;
- convert HULK numeric indexes to integer indexes. The first backend should cast
  `double` index values to `i64` after semantic checking.

## Iterable and Range ABI

For now `Iterable(T)` is object-like. The runtime can implement range as an
object with `next` and `current` method slots compatible with the IR lowering
for `for` and vector generators.

Required range constructor:

```c
HulkObject* hulk_range_new(double start, double end);
```

Required method behavior:

```text
next()    -> Boolean
current() -> Number
```

The actual methods may be exposed as generated/runtime functions placed into the
range vtable.

## Closure ABI

Runtime representation:

```c
typedef struct HulkClosure {
    void* function;
    long long capture_count;
    HulkValue* captures;
} HulkClosure;
```

Required operations:

```c
HulkClosure* hulk_closure_new(void* function, long long capture_count, HulkValue* captures);
```

`ClosureCall` lowers by loading the function pointer, casting it to the expected
signature, and passing the closure environment plus explicit arguments.

Preferred generated lambda signature:

```text
lambda(args...) in source
```

lowers to:

```llvm
define <ret> @lambda_0(ptr %env, <arg0>, <arg1>, ...)
```

For lambdas without captures, `%env` may be null.

## Type Test and Cast ABI

Each vtable carries a `type_id` and a parent pointer. Type tests walk the parent
chain:

```c
unsigned char hulk_is_type(HulkObject* object, long long target_type_id);
HulkObject* hulk_checked_cast(HulkObject* object, long long target_type_id);
```

Lowering:

```text
TypeTest value is T -> hulk_is_type(value, T_id)
TypeCast value as T -> hulk_checked_cast(value, T_id)
```

`hulk_checked_cast` should abort with a runtime error if the value is not of the
target type.

Primitive type tests and casts can be handled directly by the backend when the
static type is primitive. User-object tests should use runtime type metadata.

## Null, Unit, and Runtime Errors

`Null` maps to a null pointer of the expected pointer type.

`Unit` has no independent runtime representation. If a value is required, use
null `Object`. If no value is required, emit no result.

Runtime errors should abort with a diagnostic and non-zero exit:

```c
void hulk_runtime_error(const char* message);
```

Expected runtime errors include:

- invalid cast;
- null dereference;
- vector index out of bounds;
- invalid closure call;
- allocation failure.

## Memory Management

The first runtime may intentionally leak heap allocations. This is acceptable
for compiler validation and short-running test programs.

The ABI should still route allocations through runtime helpers where possible so
future memory management can be added without changing generated LLVM too much.

## Initial Runtime File Layout

Suggested files:

```text
runtime/hulk_runtime.h
runtime/hulk_runtime.c
```

The header declares exported runtime functions and structs. The C file provides
the implementation.

## Implementation Milestones

1. Primitive backend:
   - `Number`, `Boolean`;
   - arithmetic/comparison/control flow;
   - user functions and recursion;
   - `print(Number)`;
   - math builtins.

2. String runtime:
   - static string descriptors;
   - string print;
   - `@` and `@@`.

3. Vector runtime:
   - vector allocation;
   - push/get/set/len;
   - range and iteration.

4. Object runtime:
   - object allocation;
   - generated layouts;
   - attributes;
   - direct method calls.

5. Dynamic dispatch:
   - vtables;
   - virtual calls;
   - base calls;
   - type tests and casts.

6. Closures:
   - closure allocation;
   - capture boxing;
   - closure calls.

## Open Decisions

These decisions should be revisited when implementing the corresponding backend
stage:

- whether all heap values should share a tagged `HulkValue` representation or
  only vectors/closures should use boxed values;
- whether strings and vectors should embed a `HulkObject` header so they can flow
  through `Object` uniformly;
- whether generated object layouts should be emitted as LLVM structs or accessed
  through byte offsets;
- whether method pointer casts should be generated directly in LLVM or hidden
  behind runtime helper calls;
- whether `print` should eventually return a real `Object` value or keep using
  null `Object` as its result.
