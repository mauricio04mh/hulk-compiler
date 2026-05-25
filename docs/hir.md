# SemanticProgram and HIR

This document describes the typed HIR produced by `hulk-sema`.

## SemanticProgram

`SemanticProgram` is the semantic output of `analyze_program`.

It contains:

```rust
SemanticProgram {
    hir: HirProgram,
    registry: TypeRegistry,
    functions: HashMap<String, FunctionType>,
}
```

- `hir` is the typed high-level intermediate representation.
- `registry` is the resolved type registry, including type hierarchy, constructor parameters, attributes, methods, and protocols.
- `functions` contains the known global and builtin function signatures.

This value is meant to be the input contract between `hulk-sema` and later compiler stages such as `hulk-lower`.

## AST vs HIR

The AST from `hulk-frontend` preserves source syntax. It can contain unresolved names, syntactic sugar, and expressions whose types are still implicit.

The HIR from `hulk-sema` preserves source-level structure where useful, but resolves semantic facts needed by later stages:

- every expression has a `Type`;
- local names are resolved to `SymbolId`;
- calls are classified as builtin, global function, or local functor calls;
- method calls carry dispatch information;
- member access is resolved to a concrete attribute;
- invalid assignment targets are rejected before HIR is returned;
- some syntax can be normalized before lowering.

HIR is still high-level. It is not an IR for execution, register allocation, or VM bytecode.

## What `analyze_program` Guarantees

`analyze_program(program)` performs the semantic pipeline for the HIR contract:

1. Runs name/type declaration resolution with `resolve_program`.
2. Builds the `TypeRegistry`.
3. Registers builtin constants, builtin functions, and global function signatures.
4. Converts declarations and the entry expression to typed HIR.
5. Accumulates semantic errors when possible.

On success, it returns a `SemanticProgram` that satisfies the HIR invariants below. On failure, it returns collected `SemanticError` values. During recovery, internal expressions may temporarily use `Type::Unknown`, but no `SemanticProgram` is returned when errors were collected.

`check_program` is a compatibility API over the same semantic source of truth: success and failure should be consistent with `analyze_program`.

## HIR Invariants

Successful HIR has these invariants:

- Every `HirExpr` has a `Type` in `HirExpr.ty`.
- Every `HirExpr` has a stable `HirId`.
- `HirExprKind::Var` has a resolved `SymbolId`.
- `HirParam` and `HirLetBinding` have resolved `SymbolId` values.
- `HirExprKind::Call` has a classified `HirCallee`:
  - `Builtin`
  - `GlobalFunction`
  - `LocalFunctor`
- `HirExprKind::MethodCall` has a `DispatchKind`.
- Virtual method calls carry the receiver static type, method name, and method signature.
- `HirExprKind::BaseCall` stores the resolved `parent_type` and `method_name`.
- `HirExprKind::Assign` only contains valid assignment targets:
  - local variable assignment;
  - assignment to an attribute of `self`.
- `HirExprKind::MemberAccess` only represents valid resolved attributes.
- Attribute access outside the owning type is rejected before HIR is returned.
- `SelfRef` only appears with the resolved symbol for `self` and the current type name.
- `New` references an existing type and has constructor arguments checked for arity and type compatibility.

## Use From `hulk-lower`

`hulk-lower` should consume `SemanticProgram`, not raw AST, when lowering semantic code.

Expected lowering responsibilities:

- Walk `SemanticProgram.hir`.
- Use `HirExpr.ty` instead of recomputing expression types.
- Use `SymbolId` to distinguish locals with the same source name.
- Use `HirCallee` to lower calls without repeating function lookup.
- Use `DispatchKind` to choose virtual, static, or base dispatch.
- Use `ResolvedMember` to lower attribute reads without repeating attribute lookup.
- Use `TypeRegistry` for layout, inheritance, protocol, and method metadata.

Lowering should not redo semantic validation. If an invariant is missing, that is a semantic analysis bug.

## Examples

The examples below use conceptual shapes, not exact `Debug` output.

### Let

Source:

```hulk
let x: Number = 5 in x + 1;
```

AST shape:

```text
Let(
  bindings = [x: Number = Number(5)],
  body = Binary(Add, Var(x), Number(1))
)
```

HIR shape:

```text
HirExpr ty=Number kind=Let {
  bindings: [
    HirLetBinding {
      name: "x",
      symbol: SymbolId(N),
      ty: Number,
      value: HirExpr ty=Number kind=Number(5)
    }
  ],
  body: HirExpr ty=Number kind=Binary {
    op: Add,
    left: HirExpr ty=Number kind=Var {
      name: "x",
      symbol: SymbolId(N)
    },
    right: HirExpr ty=Number kind=Number(1)
  }
}
```

The variable use points to the same `SymbolId` as the binding.

### Function Call

Source:

```hulk
function inc(x: Number): Number => x + 1;
inc(4);
```

HIR entry shape:

```text
HirExpr ty=Number kind=Call {
  callee: GlobalFunction {
    name: "inc",
    signature: FunctionType {
      params: [Number],
      return_type: Number
    }
  },
  args: [
    HirExpr ty=Number kind=Number(4)
  ]
}
```

Builtin calls use `HirCallee::Builtin` instead:

```hulk
print(42);
```

```text
HirExpr kind=Call {
  callee: Builtin { name: "print", signature: ... },
  args: [...]
}
```

### Method Call

Source:

```hulk
type A {
    f(): Number => 1;
}

new A().f();
```

HIR entry shape:

```text
HirExpr ty=Number kind=MethodCall {
  object: HirExpr ty=UserType("A") kind=New {
    type_name: "A",
    args: []
  },
  method: "f",
  args: [],
  dispatch: Virtual {
    receiver_static_type: UserType("A"),
    method_name: "f",
    signature: FunctionType {
      params: [],
      return_type: Number
    }
  }
}
```

The method lookup has already been performed by `hulk-sema`.

### base()

Source:

```hulk
type A {
    f(): Number => 1;
}

type B inherits A {
    f(): Number => base() + 1;
}
```

HIR inside `B.f`:

```text
HirExpr ty=Number kind=Binary {
  op: Add,
  left: HirExpr ty=Number kind=BaseCall {
    parent_type: "A",
    method_name: "f",
    args: []
  },
  right: HirExpr ty=Number kind=Number(1)
}
```

`base()` does not remain an unresolved keyword. It is resolved to the parent type and the current method name.

### new

Source:

```hulk
type Point(x: Number, y: Number) {}
new Point(1, 2);
```

HIR entry shape:

```text
HirExpr ty=UserType("Point") kind=New {
  type_name: "Point",
  args: [
    HirExpr ty=Number kind=Number(1),
    HirExpr ty=Number kind=Number(2)
  ]
}
```

The constructor type exists, arity is checked, and each argument is checked against the constructor parameter type.

### Member Access

Source:

```hulk
type A {
    x: Number = 1;
    f(): Number => self.x;
}
```

HIR inside `A.f`:

```text
HirExpr ty=Number kind=MemberAccess {
  object: HirExpr ty=UserType("A") kind=SelfRef {
    symbol: SymbolId(N),
    type_name: "A"
  },
  member: "x",
  resolved: Attribute {
    owner_type: "A",
    attr_name: "x",
    ty: Number
  }
}
```

External attribute access, such as `a.x` outside the owning type, is rejected by semantic analysis.

### For Desugaring

Source:

```hulk
for (x in xs) x + 1;
```

HIR currently normalizes `for` before lowering. The final HIR should not contain `HirExprKind::For`.

Conceptual normalized shape:

```text
Let {
  bindings: [
    _iter$N = xs
  ],
  body: While {
    condition: MethodCall {
      object: Var(_iter$N),
      method: "next",
      dispatch: Virtual { ... },
      ty: Boolean
    },
    body: Let {
      bindings: [
        x = MethodCall {
          object: Var(_iter$N),
          method: "current",
          dispatch: Virtual { ... },
          ty: ElementType
        }
      ],
      body: Binary(Add, Var(x), Number(1))
    }
  }
}
```

The generated iterator binding has its own `SymbolId` and uses a name outside the user identifier space. The loop variable has its own `SymbolId` and the element type of the iterable or vector.
