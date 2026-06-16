# Estado actual del compilador HULK

Fecha de auditoría: 2026-06-15  
Repositorio auditado: `mauricio04mh/hulk-compiler`  
Commit inspeccionado por el conector de GitHub: `39326e8ea85c5749aaf33b0ad3ac449eb9d8ec36`

> Este documento es una auditoría técnica y de planificación. No pretende maquillar el avance: una feature se marca como lista solo cuando hay código integrado, evidencia de uso y pruebas razonables. Cuando solo existe AST, gramática o una estructura de IR sin ejecución real, se marca como parcial.

---

## 1. Resumen ejecutivo

El repositorio está bastante avanzado en arquitectura de compilador: usa Rust, está organizado como workspace de Cargo, separa generadores de lexer/parser, frontend, análisis semántico, HIR, IR, lowering, backend LLVM mínimo, driver CLI y runtime C. La estructura general es más sólida que la de un proyecto puramente exploratorio.

Sin embargo, el compilador todavía no debe considerarse terminado para HULK completo. La razón principal es que el backend/runtime no ejecuta el subconjunto orientado a objetos ni varias construcciones ya modeladas por el frontend, semántica e IR. El proyecto sí puede llegar hasta AST, HIR e IR para muchos casos importantes, y además puede emitir/ejecutar LLVM para un subconjunto mínimo de números, booleanos, funciones, recursión, `while`, `print`, strings literales y builtins matemáticos. Pero las instrucciones IR para objetos, atributos, dispatch virtual, vectores, closures, `is/as` y varias operaciones de runtime son rechazadas explícitamente por el backend LLVM actual.

Avance aproximado por frente, estimado a partir de inspección de código y tests:

| Frente | Avance estimado | Lectura honesta |
|---|---:|---|
| Infraestructura Rust/workspace | 80% | Workspace claro, lockfile y CI básico. Falta README usable y guía de reproducción. |
| Frontend lexer/parser/AST | 80-85% | Muy buen modelado de sintaxis obligatoria y extensiones. Hay tests amplios. Riesgos: compatibilidad exacta entre parser usado por CLI y parser “full”, y cobertura de errores. |
| Semántica y tipos | 70-75% | Hay `TypeRegistry`, HIR tipado, resolución, scopes, herencia, protocolos, métodos, atributos y tests. Falta cerrar inferencia/criterio de anotaciones y más casos negativos integrales. |
| HIR/IR/lowering | 70% | IR tipo BANNER/3AC bastante completa, con `.TYPES`, `.DATA`, `.CODE`, golden tests y lowering de muchos constructs. Falta verificador de IR y ejecución real de la mayoría de instrucciones ricas. |
| Backend/runtime | 25-35% | Existe backend LLVM mínimo y runtime C para primitivos. No hay VM propia. Objetos, vectores, closures, dispatch, casts y string concat no ejecutan en LLVM. |
| Tests | 65-70% | Hay muchos unit/golden tests por crate. Faltan tests CLI y end-to-end completos para HULK real. La ejecución LLVM depende de `clang` y se salta si no está disponible. |
| Documentación/reporte | 30-35% | `docs/ir.md` y `docs/hir.md` ayudan. README está vacío. No se verificó reporte final ni ejemplos reproducibles de usuario. |

Estado end-to-end:

- **Sí existe end-to-end mínimo**: source -> parser -> sema/HIR -> IR -> LLVM -> runtime C -> ejecución, para casos primitivos cubiertos por `crates/hulk-codegen-llvm/tests/backend_minimal.rs` cuando `clang` está disponible.
- **No existe end-to-end completo de HULK obligatorio**: los programas con `type`, `new`, atributos, métodos, herencia, dispatch dinámico, vectores, closures, `is/as` o `range` no están ejecutados por el backend actual.
- **No existe VM/runtime propio para IR** en la estructura inspeccionada. La ruta real del repo hoy es más cercana a `HULK -> HIR -> HULK IR -> LLVM IR mínimo + runtime C`, no a `HULK -> IR BANNER -> VM propia`.

Mayor riesgo actual: el frontend/semántica/IR ya modelan un lenguaje grande, pero el backend solo ejecuta un subconjunto pequeño. Si se sigue agregando sintaxis o reglas semánticas sin cerrar ejecución, el proyecto puede terminar siendo un compilador que “imprime IR” pero no ejecuta HULK real.

Próximo objetivo recomendado: congelar features nuevos y cerrar un camino ejecutable para el subconjunto obligatorio mínimo orientado a objetos: strings con concatenación, objetos, atributos, métodos, `new`, herencia simple, dispatch virtual, `base`, `for/range` o una decisión explícita de recorte. Si se decide usar LLVM, conviene extender el backend LLVM actual en lugar de iniciar una VM desde cero a esta altura.

---

## 2. Metodología de análisis

### Archivos/carpetas inspeccionadas

Se inspeccionaron, mediante el conector de GitHub, estos archivos principales:

- `Cargo.toml`
- `Cargo.lock`
- `.github/workflows/ci.yml`
- `README.md`
- `crates/hulk-frontend/src/lib.rs`
- `crates/hulk-frontend/src/ast.rs`
- `crates/hulk-frontend/src/builder.rs`
- `crates/hulk-parsegen/testdata/grammars/hulk_types.gx`
- `crates/hulk-parsegen/testdata/specs/hulk_types.lx`
- `crates/hulk-lexgen/src/runtime/lexer.rs`
- `crates/hulk-sema/src/lib.rs`
- `crates/hulk-sema/src/analysis.rs`
- `crates/hulk-sema/src/checker.rs`
- `crates/hulk-sema/src/context.rs`
- `crates/hulk-sema/src/hir_builder.rs`
- `crates/hulk-sema/src/builtins.rs`
- `crates/hulk-sema/src/types.rs`
- `crates/hulk-ir/src/lib.rs`
- `crates/hulk-lower/src/lib.rs`
- `crates/hulk-driver/src/main.rs`
- `crates/hulk-codegen-llvm/src/lib.rs`
- `crates/hulk-codegen-llvm/tests/backend_minimal.rs`
- `crates/hulk-lower/tests/golden.rs`
- `crates/hulk-frontend/tests/full_parser.rs`
- `crates/hulk-sema/tests/analysis.rs`
- `runtime/hulk_runtime.c`
- `runtime/hulk_runtime.h`
- `docs/ir.md`
- `docs/hir.md`

También se usaron como referencia externa del proyecto la planificación integral y la documentación de HULK provistas al asistente.

### Comandos intentados

Se intentó clonar el repositorio en la sandbox para ejecutar build y tests localmente:

```bash
rm -rf /mnt/data/hulk-compiler && \
git clone https://github.com/mauricio04mh/hulk-compiler.git /mnt/data/hulk-compiler && \
cd /mnt/data/hulk-compiler && \
git rev-parse HEAD && \
find . -maxdepth 3 -type f | sort | sed 's#^./##' | head -200
```

Resultado real:

```text
Cloning into '/mnt/data/hulk-compiler'...
fatal: unable to access 'https://github.com/mauricio04mh/hulk-compiler.git/': Could not resolve host: github.com
```

Implicación: no se pudo ejecutar localmente `cargo build`, `cargo test`, `cargo run`, CLI ni pruebas end-to-end desde la sandbox. El análisis se basa en inspección de repositorio vía GitHub, contenido de tests y configuración de CI. Esto es una limitación importante.

### Comandos que deberían ejecutarse en un entorno local limpio

```bash
cargo fmt --check
cargo test --workspace --all-targets --locked
cargo run -p hulk-driver -- examples/<programa>.hulk --check
cargo run -p hulk-driver -- examples/<programa>.hulk --ir
cargo run -p hulk-driver -- examples/<programa>.hulk --emit-llvm
```

Para validar ejecución LLVM mínima:

```bash
cargo test -p hulk-codegen-llvm --test backend_minimal -- --nocapture
```

Si `clang` no está instalado, el test de ejecución se salta; por tanto, no basta con ver tests verdes si el entorno no tiene clang.

### Resultados de build/test observables sin ejecución local

- El workflow de CI existe y ejecuta `cargo test --workspace --all-targets --locked`.
- La API de status combinada del commit inspeccionado no devolvió statuses registrados (`statuses: []`). Esto no prueba que CI esté fallando; solo indica que no se obtuvo estado de checks mediante esa consulta.
- El README está vacío, por lo que la reproducibilidad desde cero no está documentada.

### Limitaciones

- No se pudo inspeccionar un árbol completo con `find` local.
- No se pudieron ejecutar comandos reales de compilación ni tests.
- No se pudo verificar si existen artefactos no indexados por búsqueda, como ejemplos sueltos, reporte LaTeX o assets.
- No se pudo medir cobertura real. La “cobertura aproximada” de este documento se infiere por tests existentes y código inspeccionado.

---

## 3. Arquitectura real del repositorio

El proyecto está organizado como un workspace de Rust con varios crates y un runtime C auxiliar. La arquitectura real es buena y modular.

| Módulo esperado | Existe en repo | Ruta real | Estado | Observaciones |
|---|---|---|---|---|
| Configuración workspace | Sí | `Cargo.toml`, `Cargo.lock` | Listo | Workspace con crates separados. Usa Rust edition 2024. |
| Lexer generator | Sí | `crates/hulk-lexgen` | Parcial/Listo | Hay runtime lexer, specs `.lx`, tests. Necesita más evidencia de errores de strings/comentarios complejos. |
| Parser generator | Sí | `crates/hulk-parsegen` | Parcial/Listo | Gramáticas `.gx`, LL(1), Pratt hook. Bien integrado al frontend. |
| Frontend AST | Sí | `crates/hulk-frontend/src/ast.rs` | Listo para modelado | AST cubre obligatorio y varias extensiones: protocolos, vectores, functors/lambdas. |
| AST builder | Sí | `crates/hulk-frontend/src/builder.rs` | Parcial/Listo | Construye Program/Decl/Expr desde CST. Riesgo en errores y en consistencia entre gramáticas. |
| Desugar | Parcial | En `hulk-sema/src/hir_builder.rs` | Parcial | `for` se normaliza a `let` + `while` en HIR. No hay crate separado `desugar`. |
| Semántica | Sí | `crates/hulk-sema` | Parcial alto | Tiene resolver, TypeRegistry, HIR, checker compat. Falta cierre de inferencia/errores exhaustivos. |
| HIR | Sí | `crates/hulk-sema/src/hir.rs`, `docs/hir.md` | Listo como contrato intermedio | Documentado y usado por lowering. |
| IR propia | Sí | `crates/hulk-ir`, `docs/ir.md` | Parcial alto | Representa tipos, data, funciones, objetos, vectores, closures, casts, dispatch. Falta verifier y ejecución completa. |
| Lowering | Sí | `crates/hulk-lower` | Parcial alto | Hay golden tests amplios. Genera instrucciones que backend LLVM aún no soporta. |
| Backend LLVM | Sí | `crates/hulk-codegen-llvm` | Parcial bajo/medio | Ejecuta subset primitivo; rechaza objetos/vectores/closures/casts. |
| VM propia | No claro/No | No aparece crate `hulk-vm` ni runtime IR | Falta | La ruta diseñada BANNER+VM no está implementada como VM. |
| Runtime | Sí, mínimo | `runtime/hulk_runtime.c`, `.h` | Parcial | Print, strings estáticos y math builtins. Sin heap de objetos/vectores/closures. |
| CLI | Sí | `crates/hulk-driver/src/main.rs` | Parcial | Dummpea AST/check/HIR/IR/LLVM. No tiene `run` ni compilación/linking final. |
| Tests unitarios | Sí | `crates/*/tests` | Parcial alto | Muchos tests frontend/sema/lower/codegen. Falta end-to-end completo y CLI. |
| Golden tests | Sí | `crates/hulk-lower/tests/golden.rs` | Listo para IR | Bastante buena base para estabilidad del lowering. |
| Examples | No claro | No verificado | Falta/No claro | La búsqueda no encontró ejemplos reproducibles. README vacío no los menciona. |
| Docs | Sí, parcial | `docs/ir.md`, `docs/hir.md` | Parcial | Buenas docs internas de HIR/IR. Falta README, arquitectura general, extensión y reporte. |
| CI/CD | Sí | `.github/workflows/ci.yml` | Parcial | Ejecuta tests del workspace. Falta fmt/lint/release/demo. |
| Reporte final | No claro | No verificado | Falta/No claro | No se encontró evidencia de reporte LaTeX. |

Arquitectura esperada vs real:

- Esperada por la planificación: `lexer`, `parser`, `ast`, `desugar`, `sema`, `ir`, `backend/runtime`, `cli`, `tests`, `examples`, `docs`, `report`, CI.
- Real: existe casi toda la columna técnica, pero `desugar` está embebido en sema/HIR, el backend real es LLVM mínimo en vez de VM propia, runtime es muy reducido, y la documentación de usuario está casi ausente.

---

## 4. Matriz de features HULK

| Feature | Estado | Evidencia en repo | Tests existentes | Huecos detectados | Prioridad |
|---|---|---|---|---|---|
| Expresiones numéricas | Listo para subset ejecutable | `Expr::Number`, binarios, sema `Type::Number`, IR `ConstNumber`, LLVM `fadd/fsub/...` | Frontend, sema, lower golden, backend minimal | Validar todos los errores y precedencia con CLI | Alta |
| Precedencia/asociatividad | Parcial/Listo | Pratt config en `hulk-frontend/src/lib.rs` | Tests frontend | Confirmar todos los casos borde: `^` right-assoc, `is/as`, llamadas encadenadas | Media |
| Strings literales | Parcial/Listo | Lexer string con escapes; AST/IR data; LLVM imprime strings | Frontend/codegen minimal | String concat `@`/`@@` no soportada en LLVM; falta heap/concatenación runtime | Crítica |
| Concatenación `@` y `@@` | Parcial/Roto en backend | AST/IR tienen `Concat`/`ConcatSpace`; LLVM retorna `UnsupportedInstruction("StringConcat")` | Lower golden | Implementar runtime/codegen; convertir Number/Boolean a String si aplica | Crítica |
| Booleanos | Listo para subset ejecutable | `Expr::Bool`, `Type::Boolean`, LLVM bool ops | Frontend/sema/backend minimal | Short-circuit real para `&`/`|` no claro; hoy se baja como binario | Media |
| Comparaciones | Listo para subset ejecutable | Sema valida Number; LLVM `fcmp` | Backend minimal | Comparación entre objetos/strings no clara | Media |
| Operadores lógicos | Parcial/Listo | `AND`, `OR`, `NOT` en lexer/AST/sema/IR/LLVM | Backend minimal | Confirmar semántica de evaluación perezosa si se exige | Media |
| Builtin `print` | Parcial/Listo | Sema firma `Object -> Object`; LLVM imprime Number/Boolean/String | Backend minimal | No imprime objetos/vectores; retorno `Object` puede chocar con backend tipado | Alta |
| Builtins math `sqrt/sin/cos/exp/log/rand` | Listo para subset numérico | `builtins.rs`, runtime C, LLVM declarations | Backend minimal | Semillas/random reproducible no documentado | Media |
| Constantes `PI`, `E` | Listo en sema/HIR | `builtin_constant_value` convierte a literales | Sema/lower probablemente | Confirmar CLI/golden específicos | Baja |
| `range` | Parcial | Firma semántica retorna `Iterable(Number)` | Tests de for/range indirectos | Runtime/LLVM no implementa `range`; no VM iterable real | Crítica para `for` |
| Bloques de expresiones | Listo/Parcial | AST `Block`, HIR/IR lower último valor | Tests frontend/sema/lower | Backend ok en primitivos, no objetos | Alta |
| Expresión global entrypoint | Listo | `Program { declarations, entry }`; grammar `DeclList Expr` | Múltiples tests | CLI solo imprime/dump; no `run` | Alta |
| Funciones inline | Listo/Parcial | `FunctionBody -> ARROW Expr SEMICOLON` | Frontend/sema/backend minimal | Sin inferencia completa de parámetros; backend limitado a tipos soportados | Alta |
| Funciones full-form | Parcial | `FunctionBody -> BlockExpr`; AST body Expr | Tests frontend | Validar ejecución LLVM con bloques complejos | Media |
| Llamadas a funciones | Listo/Parcial | HIR `HirCallee::GlobalFunction`; IR `Call`; LLVM user calls | Backend minimal | Tipos objeto/vector/string concat no soportados | Alta |
| Recursión | Listo para Number | Backend minimal incluye `fact` recursivo | `backend_minimal.rs` | Recursión con objetos/vectores no verificada | Media |
| `let` múltiple | Listo/Parcial | AST `bindings: Vec`; HIR scopes; IR locals | Tests sema/lower | Confirmar todos los casos de shadowing en frontend/CLI | Alta |
| Scopes léxicos | Parcial alto | `scopes: Vec<HashMap<...>>`, SymbolId en HIR | Tests `analysis.rs`, lower golden shadowing | Más negativos de captura/lambda/loops | Alta |
| Shadowing | Parcial/Listo | HIR SymbolId distingue símbolos; golden `scope_shadowing` | Golden lower | Mensajes de error con spans limitados | Media |
| Asignación destructiva `:=` | Parcial | AST `Assign`; HIR local/self.attr; IR `Assign`/`SetAttr` | Sema/lower | Backend solo soporta local primitivo; `self.attr :=` no ejecuta | Alta |
| `if/elif/else` expresión | Parcial alto | AST/HIR/IR; branch labels; sema unify | Frontend/sema/lower/backend minimal | LCA nominal debe reforzarse con tests OO; backend limitado | Alta |
| `while` expresión | Parcial alto | AST/HIR/IR; LLVM branch | Backend minimal while+assign | Valor de loop y casos de no entrada deben verificarse | Alta |
| `for` | Parcial | AST `For`; HIR desugaring a `let` + `while`; lower golden | Frontend/sema/lower golden | Falta ejecución runtime de iterables/range/vectors | Crítica |
| `type`/clases | Parcial alto en frontend/sema/IR | AST `TypeDecl`, TypeRegistry, IR `.TYPES` | Frontend/sema/lower golden | Backend LLVM rechaza objetos | Crítica |
| Atributos | Parcial | AST/HIR/IR `GetAttr/SetAttr`; privacidad en sema | Sema/lower | Sin memoria/heap ejecutable | Crítica |
| Métodos | Parcial | HIR dispatch; IR `VirtualCall/StaticCall/BaseCall` | Sema/lower | LLVM rechaza VirtualCall/StaticCall/BaseCall | Crítica |
| Constructores con argumentos | Parcial | Type params, `New`, init functions en lowering | Sema/lower | No ejecutan en backend | Crítica |
| `new` | Parcial | AST/HIR/IR `Allocate` + init | Tests frontend/sema/lower | LLVM rechaza `Allocate` | Crítica |
| `self` | Parcial alto | HIR introduce self en métodos; sema limita fuera de métodos | Tests sema | Backend objeto no ejecuta | Alta |
| `base` | Parcial alto | HIR resuelve parent/method; IR `BaseCall` | Tests sema/lower | LLVM rechaza `BaseCall` | Alta |
| Herencia simple | Parcial alto | TypeRegistry valida padres/ciclos; lower flatten layout | Sema/lower golden | No ejecución dinámica | Crítica |
| `Object`, `Number`, `String`, `Boolean` | Parcial | `Type` enum + builtins | Tests sema | Modelo runtime incompleto para `Object` y user types | Alta |
| Sobrescritura de métodos | Parcial alto | `validate_method_overrides` exacta firma | Sema tests | Falta ejecución dispatch | Alta |
| Polimorfismo/dispatch dinámico | Parcial | IR method slots y `VirtualCall` | Lower golden | No backend runtime/vtable | Crítica |
| Acceso a atributos | Parcial | Solo `self.attr` permitido; externos privados | Sema tests | Ejecución no disponible | Alta |
| Reglas de privacidad | Parcial/Listo en sema | `AttributeIsPrivate` | Sema tests | Herencia/atributos heredados y mensajes con spans a reforzar | Media |
| `is` | Parcial | AST/HIR/IR `TypeTest` | Frontend/lower golden | LLVM rechaza `TypeTest`; runtime type tags faltan | Alta |
| `as` | Parcial | AST/HIR/IR `TypeCast` | Frontend/lower golden | LLVM rechaza `TypeCast`; runtime error faltante | Alta |
| Inferencia de tipos | Parcial | Expresiones y lets infieren; parámetros sin tipo generan error | Sema/HIR | No hay inferencia general de parámetros/metodos; documentar estrategia básica | Alta |
| Protocolos | Parcial, candidato a extensión | AST/TypeRegistry/protocol conformance | Frontend/sema | No documentado como extensión propia; backend no lo necesita si se borra tras sema | Alta para entrega |
| Vectores | Parcial, candidato extensión | AST/HIR/IR `Vector*` | Frontend/sema/lower | LLVM/runtime no soporta `NewVector`, `VectorGet`, etc. | Media/Alta |
| Lambdas/functors | Parcial, candidato extensión | AST/HIR/IR `Lambda`, `MakeClosure`, `ClosureCall` | Frontend/lower | LLVM/runtime no soporta closures | Baja si falta tiempo |
| Macros | Falta | No se observó AST/grammar/runtime macro | No | No intentar salvo que sea requisito explícito | Baja |
| GC | Falta | No hay heap/GC completo | No | Solo necesario si se implementan objetos/vectores persistentes | Media/Alta |

---

## 5. Estado del frontend

### Lexer

El lexer runtime existe en `crates/hulk-lexgen/src/runtime/lexer.rs`. Produce tokens con `kind`, `lexeme`, offsets y posición de línea/columna. Maneja EOF, símbolos, strings, números, identificadores/keywords, whitespace y comentarios de línea.

Evidencia relevante:

- `LexError` contiene `message`, `start`, `line`, `column`.
- `lex_hulk(input, spec)` devuelve `Vec<Token>` o `LexError`.
- `skip_ignored` maneja whitespace y comentario de línea configurado.
- `try_match_string` maneja escapes de comillas, backslash, newline `\n`, tab `\t`, y reporta strings sin cerrar.
- `try_match_number` maneja enteros y fracciones si el spec lo permite.
- El spec HULK actual define keywords, operadores, identificadores, números, strings, whitespace y `//`.

Estado: **parcial alto / casi listo**.

Huecos/riesgos:

- No se vio soporte para comentarios de bloque. Si HULK oficial no los exige, no es problema; si se esperan, faltan.
- El número parece aceptar enteros y fracciones simples, pero no se vio soporte para exponentes científicos.
- El spec declara `ident IDENT start=letter rest=letter|digit|_`, lo cual respeta que el identificador debe comenzar por letra.
- La cobertura de errores léxicos debe reforzarse con ejemplos inválidos: strings sin cerrar, escapes inválidos, `_x`, `8ball`, caracteres raros, comentario al final de archivo.

### Parser

El frontend expone varias funciones de parseo:

- `parse_hulk_expr_program`
- `parse_hulk_functions_program`
- `parse_hulk_control_program`
- `parse_hulk_types_program`
- `parse_hulk_full_program`

El pipeline lee gramáticas `.gx`, specs `.lx`, normaliza gramática, calcula FIRST/FOLLOW, construye tabla LL(1), instala un Pratt parser para expresiones y luego convierte CST a AST con `AstBuilder`.

La gramática `hulk_types.gx` cubre:

- programa como `DeclList Expr ProgramEnd EOF`;
- funciones;
- parámetros y anotaciones de tipo;
- tipos, herencia y miembros;
- protocolos;
- `if/elif/else`;
- `while`;
- `for`;
- `let`;
- bloques;
- expresiones operacionales delegadas al Pratt parser.

El Pratt parser configura:

- asignación `:=` right-assoc;
- `|`, `&`, igualdad, comparación;
- concatenación `@`, `@@`;
- suma/resta, multiplicación/división/módulo;
- potencia;
- operadores unarios `!`, `-`, `+`;
- primarios `NUMBER`, `IDENT`, `STRING`, `TRUE`, `FALSE`;
- `new`, `self`, `base`, `is`, `as`, brackets y lambdas/functor arrow.

Estado: **parcial alto**.

Riesgos:

- El CLI usa `parse_hulk_types_program`, no `parse_hulk_full_program`. Si `hulk_full` tiene más sintaxis que `hulk_types`, el usuario puede creer que una feature existe porque pasa tests de frontend full, pero no estar disponible en el comando principal.
- Es necesario documentar qué gramática es la oficial para entrega.
- La recuperación de errores sintácticos existe vía `ParseErrorList`, pero no se inspeccionó a fondo la calidad de mensajes.

### AST

El AST está bien modelado. `Program` tiene declaraciones y expresión entrypoint. `Decl` cubre `Function`, `Type`, `Protocol`. `Expr` cubre prácticamente todo lo esperado:

- literales;
- variables;
- unarios/binarios;
- asignación;
- `let`;
- llamadas;
- bloques;
- `if`;
- `while`;
- `for`;
- `new`;
- acceso a miembro;
- llamada a método;
- `self`;
- `base`;
- `is`/`as`;
- vectores;
- lambdas.

Estado: **listo como modelo sintáctico**.

Riesgos:

- `Span` guarda línea y columna, pero su `PartialEq` siempre retorna true para facilitar tests. Eso está bien para tests de igualdad de AST, pero hay que asegurarse de que los errores usen spans reales y no pierdan ubicación.
- Algunas features opcionales/extensiones ya están en AST. Eso no significa que estén completas en sema/backend.

---

## 6. Estado de semántica y tipos

La semántica está concentrada en `crates/hulk-sema`. El entrypoint real es `analyze_program`, que hace:

1. `resolve_program(program)`;
2. `TypeRegistry::build(program)`;
3. `HirBuilder::new(registry).analyze_program(program)`.

`check_program` actualmente es una API de compatibilidad que llama `analyze_program(program).map(|_| ())`. Existe un checker legacy en `checker.rs`, pero está marcado como `#[allow(dead_code)]` y no es la fuente principal de verdad.

### Fortalezas

- Hay separación real entre AST y HIR.
- El HIR asigna tipo a cada expresión.
- Los símbolos locales se resuelven a `SymbolId`, lo cual permite distinguir shadowing.
- Las llamadas se clasifican como builtin, función global o functor local.
- Las llamadas a método llevan `DispatchKind`.
- `TypeRegistry` guarda tipos, protocolos, atributos, métodos, constructor params y padres.
- Se validan duplicados de tipos/protocolos, atributos/métodos y métodos de protocolo.
- Se valida herencia: padres inexistentes, herencia de primitivos y ciclos.
- Se valida override con firma exacta.
- Se modelan protocolos y conformidad estructural.
- Se pre-registra protocolo builtin `Iterable`.
- `for` se desazucara/normaliza a `let _iter$N = iterable in while (_iter$N.next()) let x = _iter$N.current() in body`.
- Atributos privados: acceso externo a `a.x` se rechaza; `self.x` se permite solo dentro del tipo correspondiente.
- `base()` se resuelve al método padre correspondiente.
- Hay tests semánticos para HIR, `for`, self, base, privacidad, new arity, métodos desconocidos y vectores.

### Debilidades o zonas incompletas

- La inferencia de parámetros de funciones/métodos no es general. Si un parámetro no tiene anotación, `parameter_type` reporta `CannotInferParameterType`. Esto puede ser defendible como “estrategia básica”, pero debe documentarse claramente. HULK permite anotaciones opcionales y espera algún grado de inferencia; si la evaluación exige inferir parámetros no anotados, esto es un hueco.
- `print` se tipa como `Object -> Object`, pero el backend LLVM solo imprime `Number`, `Boolean` y `String`. Esta diferencia puede permitir pasar sema a programas que luego no codegen.
- `range` existe como builtin semántico y retorna `Iterable(Number)`, pero no se vio implementación runtime/codegen.
- Protocolos parecen implementados semánticamente, pero no documentados como extensión propia.
- Hay que revisar si `self` puede ser sombreado por `let` o parámetros según la especificación. La implementación introduce `self` como símbolo especial en métodos; HULK permite que `self` no sea keyword en la especificación de referencia, pero el lexer lo trata como keyword/token `SELF`. Esto puede ser una desviación defendible, pero debe documentarse.
- La relación de conformidad nominal existe vía `is_descendant_of`, pero conviene reforzar tests de LCA en ramas con tipos hermanos y ancestros.
- `is/as` se validan solo como tipos existentes y se bajan a IR; falta runtime real para dynamic type test/cast.
- Los mensajes de error no se auditaron exhaustivamente con spans. La semántica tiene `SemanticError`, pero muchos errores se construyen sin span de AST.

Estado global de sema: **parcial alto, pero no cerrado**.

---

## 7. Estado de IR y lowering

Existe una IR propia en `crates/hulk-ir`. Está documentada en `docs/ir.md`. No es LLVM IR; es una IR typed, de tres direcciones, con secciones similares a BANNER:

- `.TYPES`
- `.DATA`
- `.CODE`

La IR modela:

- `IrProgram { types, data, functions, entry }`;
- tipos primitivos, user types, vectores, iterables, functors y unknown;
- layouts de tipos con atributos y métodos;
- static data;
- funciones entry/global/method/lambda;
- params, locals, temps;
- valores y lugares de escritura;
- instrucciones de control flow;
- llamadas;
- objetos (`Allocate`, `GetAttr`, `SetAttr`);
- dispatch (`VirtualCall`, `StaticCall`, `BaseCall`);
- vectores;
- closures;
- `TypeTest`/`TypeCast`;
- `Return`.

Esto es una base muy buena para una IR tipo BANNER, aunque no es exactamente una BANNER minimalista “todo es número”. Es más alta y más tipada, lo cual puede ser positivo: desacopla frontend/backend y permite que backends diferentes decidan representación runtime.

### Lowering

`crates/hulk-lower` baja `SemanticProgram`/HIR a `IrProgram`.

Soporta:

- entry function sintética;
- lowering de funciones globales;
- layouts de tipos con flattening de herencia;
- slots de métodos;
- inicializadores de tipos;
- métodos con `self` explícito;
- literales;
- variables;
- unarios/binarios;
- `let`;
- bloques;
- asignación local y `self.attr`;
- `if` con labels/branch;
- `while` con labels/branch;
- llamadas a builtin/global/functor;
- `new` como `Allocate` + llamada a init;
- acceso a atributos;
- métodos virtual/static/base;
- `TypeTest`/`TypeCast`;
- vectores y vector generator;
- lambdas con captures.

`HirExprKind::For` queda como unsupported en lowering directo, pero esto es aceptable si el contrato de HIR garantiza que `for` ya fue normalizado antes de llegar al lowering. Los tests de HIR verifican que el `for` no queda como `For` y se transforma a `let` + `while`.

### Tests de IR

`crates/hulk-lower/tests/golden.rs` es una fortaleza. Tiene golden tests para:

- literales Number/Boolean/String;
- let aritmético;
- string concat;
- if/elif;
- assignment/while;
- function call;
- type/method;
- inheritance/base;
- `is/as`;
- vector index/generator;
- lambda closure;
- for loop;
- shadowing;
- outer scope assignment;
- attribute initializer;
- dynamic dispatch/static base;
- operators full;
- math builtins;
- recursive function;
- big IR smoke.

### Huecos

- No existe verificador de IR dedicado. `docs/ir.md` reconoce explícitamente que los invariantes se sostienen por tests de lowering.
- La IR no tiene intérprete/VM propio.
- La IR genera operaciones ricas que el backend LLVM no soporta aún.
- No hay parser textual de IR ni herramienta para ejecutar `.ir` directamente.
- No hay especificación runtime completa para layout de objetos, strings dinámicos, vectores, closures, vtables o GC.

Estado: **IR/lowering parcial alto, buena base, pero no ejecutable por sí sola**.

---

## 8. Estado del backend/runtime/VM

### VM propia

No se encontró una VM propia para ejecutar la IR. Tampoco aparece un crate tipo `hulk-vm`, `hulk-runtime-ir`, `hulk-interpreter` o similar en el workspace raíz. La ruta real actual es LLVM IR mínimo más runtime C.

Estado: **falta** si el diseño final pretende ser `IR propia -> VM/runtime propio`.

### Backend LLVM

Existe `crates/hulk-codegen-llvm`. Emite LLVM textual y declara funciones runtime:

- `hulk_print_number`
- `hulk_print_bool`
- `hulk_print_string`
- `hulk_sqrt`
- `hulk_sin`
- `hulk_cos`
- `hulk_exp`
- `hulk_log`
- `hulk_pow`
- `hulk_rand`

Soporta principalmente:

- Number como `double`;
- Boolean como `i1`/`i8` para print;
- String literal como descriptor `%HulkString`;
- locals/temps con `alloca`;
- asignación;
- unarios;
- binarios numéricos;
- comparaciones;
- boolean ops;
- branches/labels;
- llamadas a funciones/builtins;
- `main` wrapper.

Pero rechaza explícitamente:

- `Allocate`
- `GetAttr`
- `SetAttr`
- `VirtualCall`
- `StaticCall`
- `BaseCall`
- `NewVector`
- `VectorLen`
- `VectorPush`
- `VectorGet`
- `VectorSet`
- `MakeClosure`
- `ClosureCall`
- `TypeTest`
- `TypeCast`
- string concatenation

Esto significa que el backend no ejecuta todavía el corazón orientado a objetos de HULK ni las extensiones ya modeladas.

### Runtime C

`runtime/hulk_runtime.c` implementa:

- impresión de números;
- impresión de booleanos;
- impresión de strings estáticos;
- math builtins;
- `rand`.

No implementa:

- heap de objetos;
- atributos;
- vtables;
- strings dinámicos/concatenación;
- vectores;
- iteradores/range;
- closures;
- casts dinámicos;
- errores runtime;
- GC.

### Tests end-to-end actuales

`crates/hulk-codegen-llvm/tests/backend_minimal.rs` cubre programas soportados:

- `print(42)`;
- aritmética;
- booleanos;
- comparaciones;
- función global;
- función recursiva `fact`;
- `while + assignment`;
- builtins matemáticos;
- `print` de string literal.

El mismo archivo incluye tests que esperan error limpio para vectores y objetos. Esto es positivo porque el proyecto no pretende falsamente soportarlos en backend, pero confirma que todavía faltan para HULK completo.

Estado backend/runtime: **parcial bajo/medio**.

---

## 9. Estado de la extensión propia

No se encontró una documentación explícita que diga: “Nuestra extensión es X”, con motivación, sintaxis, semántica, cambios en AST/sema/IR/backend y tests. El README está vacío, y aunque hay docs de HIR/IR, no hay sección de extensión.

Sí existen varias features no triviales que pueden convertirse en extensión defendible:

### Opción A: Protocolos estructurales

Evidencia:

- `Decl::Protocol` en AST.
- `ProtocolDecl`, `ProtocolMethod`.
- Grammar `ProtocolDecl -> PROTOCOL IDENT ProtocolParent ...`.
- `TypeRegistry` registra protocolos.
- Valida métodos de protocolos.
- Implementa conformidad estructural con covarianza de retorno y contravarianza de parámetros.
- Builtin `Iterable` como protocolo.

Ventajas:

- Es mayormente compile-time. No exige cambios grandes de runtime.
- Ya está bastante avanzado.
- Tiene valor académico alto: tipado estructural sobre sistema nominal.
- Es defendible si se documenta bien.

Riesgos:

- En la documentación oficial de HULK, los protocolos son una extensión/feature avanzada existente. Si el requisito exige una extensión “propia” inventada por el equipo, hay que explicar la variante propia o elegir otra.
- Falta documentación y ejemplos.

Recomendación: **mejor candidata si queda poco tiempo**.

### Opción B: Vectores y comprehensions

Evidencia:

- AST `VectorLiteral`, `VectorGenerator`, `VectorIndex`.
- TypeRef `Vector`.
- HIR/sema para tipos de vector.
- IR `NewVector`, `VectorGet`, `VectorPush`.
- Golden tests.

Ventajas:

- Muy visible en demo.
- Modifica sintaxis y semántica claramente.
- Buena para reporte.

Riesgos:

- Backend LLVM/runtime no soporta vectores.
- Requiere memoria, layout, bounds, indexing y posiblemente iteración.

Recomendación: buena extensión si se decide completar runtime de vectores; más costosa que protocolos.

### Opción C: Lambdas/functors

Evidencia:

- AST `Lambda`.
- TypeRef `Functor`.
- HIR `LocalFunctor`.
- IR `MakeClosure`, `ClosureCall`.
- Golden tests.

Ventajas:

- Alta calidad académica.
- Muy expresiva.

Riesgos:

- Closures y captures requieren runtime no trivial.
- LLVM backend rechaza `MakeClosure` y `ClosureCall`.

Recomendación: **no priorizar si falta tiempo**.

Conclusión de extensión: hoy hay implementación parcial de varias extensiones, pero ninguna está cerrada como entrega completa. Para la defensa, conviene escoger una sola. La opción más conveniente es **Protocolos estructurales**, porque toca sintaxis, AST y type checker, pero puede desaparecer antes del backend. Si el profesor exige ejecución visible de la extensión, elegir vectores solo si se implementa runtime mínimo.

---

## 10. Estado de pruebas

| Tipo de prueba | Estado | Evidencia | Huecos |
|---|---|---|---|
| Lexer | Parcial | Tests en `crates/hulk-lexgen/tests`: parse spec, runtime lex, end-to-end según búsqueda | Faltan casos negativos exhaustivos visibles en auditoría: escapes inválidos, strings sin cerrar, identificadores inválidos. |
| Parser generator | Parcial/Listo | Tests en `crates/hulk-parsegen/tests`: gx frontend, LL(1), hulk grammars | Validar errores sintácticos de programas HULK grandes. |
| Frontend AST | Listo/Parcial alto | `crates/hulk-frontend/tests/full_parser.rs`, `control_flow.rs`, `types_frontend.rs`, etc. | Separar tests del parser usado por CLI vs full parser. |
| Semántica válida | Parcial alto | `crates/hulk-sema/tests/analysis.rs`, `method_calls.rs`, `iterables.rs`, etc. | Más programas integrales con clases/herencia/for/range. |
| Semántica inválida | Parcial | Tests de duplicates, resolver, type checker, análisis de errores | Reforzar spans/mensajes y combinaciones complejas. |
| IR/lowering | Fuerte | `crates/hulk-lower/tests/golden.rs` con muchos snapshots | Agregar verificador de IR y tests de invariantes. |
| Backend LLVM | Parcial | `backend_minimal.rs` prueba emisión y ejecución si `clang` existe | No cubre objetos/vectores/closures/casts; ejecución se salta sin clang. |
| End-to-end completo | Falta/Parcial bajo | Solo subset mínimo LLVM | Faltan source -> ejecución para HULK OO obligatorio. |
| CLI | Falta/No claro | No se encontraron tests `assert_cmd` o similares | Agregar tests de flags, exit codes, stdout/stderr. |
| Extensión | Parcial | Tests de protocolos/vectores/lambdas en frontend/sema/lower | Falta documentación y, para vectores/lambdas, backend. |
| Examples | Falta/No claro | No se encontró evidencia en búsqueda | Crear ejemplos válidos/invalidos reproducibles. |

Regla recomendada desde ahora: cada bug nuevo debe dejar un test nuevo en el crate más bajo que lo reproduzca y, si llega al backend, un test end-to-end.

---

## 11. Estado de CI/CD, README y reporte

### CI/CD

Existe `.github/workflows/ci.yml` con:

- trigger en push y PR a `main`;
- checkout;
- toolchain Rust stable;
- `cargo test --workspace --all-targets --locked`.

Estado: **básico correcto**.

Falta:

- `cargo fmt --check`;
- `cargo clippy --workspace --all-targets -- -D warnings` o una variante menos estricta;
- instalación opcional de `clang` si se quieren ejecutar tests LLVM reales;
- cache de Cargo;
- build de release;
- validación de ejemplos;
- artefacto de reporte si existe.

### README

`README.md` está vacío. Esto es un riesgo alto para entrega. Un evaluador que clone el repo no sabe:

- qué es el proyecto;
- cómo instalar Rust/clang;
- cómo correr tests;
- cómo usar el CLI;
- qué features están soportadas;
- qué subset ejecuta;
- cuál es la extensión;
- cómo generar IR/LLVM;
- qué limitaciones son conocidas.

Estado: **falta crítico**.

### Documentación interna

Existen documentos útiles:

- `docs/ir.md`: explica posición de la IR, invariantes, `.TYPES`, `.DATA`, `.CODE`, tipos, valores, layouts e instrucciones.
- `docs/hir.md`: explica `SemanticProgram`, AST vs HIR, invariantes y ejemplos de HIR.

Estado: **parcial bueno**.

### Reporte

No se verificó reporte LaTeX ni PDF final. Si no existe, debe crearse urgentemente. Debe incluir:

- arquitectura;
- decisiones de diseño;
- frontend;
- semántica;
- HIR/IR;
- backend real;
- extensión;
- pruebas;
- limitaciones;
- conclusiones.

---

## 12. Riesgos principales

| Riesgo | Evidencia | Impacto | Probabilidad | Mitigación concreta |
|---|---|---:|---:|---|
| Backend no soporta HULK OO | LLVM rechaza `Allocate`, `GetAttr`, `VirtualCall`, `StaticCall`, `BaseCall` | Muy alto | Alta | Implementar runtime de objetos/vtables o recortar explícitamente; priorizar `new`, attr, method, dispatch. |
| No hay VM propia pese al diseño BANNER | Workspace no incluye crate VM; IR solo se imprime/baja a LLVM | Alto | Alta | Decidir formalmente: terminar LLVM o implementar VM mínima. No intentar ambas. |
| README vacío | `README.md` sin contenido | Alto | Alta | Escribir README con instalación, uso, features, limitaciones y demos. |
| Extensión no documentada | Hay protocolos/vectores/lambdas, pero no elección explícita | Alto | Alta | Elegir una extensión, documentarla y crear tests/demo. |
| Features opcionales aumentan deuda | AST/sema/lower soportan vectores/lambdas, backend no | Medio/Alto | Alta | Congelar features; no agregar macros ni optimizaciones. |
| Inferencia incompleta | Parámetros sin tipo generan error | Medio/Alto | Media | Documentar estrategia básica o implementar inferencia mínima; tests de decisión. |
| `range` semántico sin runtime | Builtin existe, pero runtime/LLVM no lo implementa | Alto | Alta | Implementar `Range` o bajar `range`/`for` a estructuras ejecutables. |
| String concat no ejecuta | LLVM rechaza `StringConcat` | Alto | Alta | Agregar runtime `hulk_string_concat` y lowering/codegen. |
| Tests end-to-end dependen de clang disponible | Test se salta si no hay clang | Medio | Media | CI debe instalar clang o separar test que falle si no hay backend. |
| Sin tests de CLI | No se observó suite del driver | Medio | Alta | Agregar `assert_cmd` o integración con `Command`. |
| Spans de errores semánticos insuficientes | Muchos errores no cargan span visible | Medio | Media | Propagar `Span` en HIR/errores y tests de mensajes. |
| No hay verificador de IR | `docs/ir.md` reconoce que no existe verifier | Medio | Media | Implementar `hulk-ir::verify(&IrProgram)`. |

---

## 13. Plan recomendado para terminar

### Bloque crítico para que compile/ejecute end-to-end

#### Tarea 1: decidir backend final y documentarlo

- Qué hacer: elegir formalmente entre LLVM como backend final o VM propia para IR.
- Recomendación: por el estado actual, **terminar LLVM mínimo ampliado** es más corto que construir una VM nueva.
- Dónde tocar: `README.md`, `docs/ir.md`, reporte.
- Test: ninguno técnico; checklist de documentación.
- Criterio de terminado: README dice claramente “backend final: LLVM + runtime C” o “IR VM propia”, y los comandos de demo coinciden.

#### Tarea 2: implementar string concatenation

- Qué hacer: soportar `@` y `@@` en LLVM/runtime.
- Dónde tocar:
  - `runtime/hulk_runtime.c/.h`
  - `crates/hulk-codegen-llvm/src/lib.rs`
  - quizá `crates/hulk-ir` si se decide separar concat number/string.
- Test:
  - `print("a" @ "b"); -> ab`
  - `print("x" @@ "y"); -> x y`
  - `print("n=" @ 42);` si se exige coerción Number->String.
- Criterio de terminado: golden IR sigue igual o se ajusta; LLVM ejecuta concat y CI lo prueba.

#### Tarea 3: ejecutar objetos mínimos

- Qué hacer: implementar `Allocate`, `SetAttr`, `GetAttr`, `StaticCall` para inicializadores y métodos simples sin herencia primero.
- Dónde tocar:
  - `runtime/hulk_runtime.c/.h` para heap objeto básico;
  - `crates/hulk-codegen-llvm/src/lib.rs` para structs/pointers o llamadas runtime;
  - posiblemente `crates/hulk-ir` si falta metadata de layouts.
- Test:
  ```hulk
  type Point(x: Number) {
      x: Number = x;
      getX(): Number => self.x;
  }
  let p = new Point(3) in print(p.getX());
  ```
- Criterio de terminado: imprime `3` pasando source -> LLVM -> clang -> runtime.

#### Tarea 4: ejecutar herencia y dispatch

- Qué hacer: implementar method slots/vtables para `VirtualCall` y `BaseCall`.
- Dónde tocar:
  - `hulk-codegen-llvm` para vtable/global method table;
  - runtime C si se delega dispatch al runtime.
- Test:
  ```hulk
  type A { f(): Number => 1; }
  type B inherits A { f(): Number => base() + 1; }
  let x: A = new B() in print(x.f());
  ```
- Criterio de terminado: dispatch dinámico llama implementación de `B`, y `base()` llama `A.f`.

#### Tarea 5: implementar `range`/`for` ejecutable

- Qué hacer: decidir si `range` será un objeto `Range` con `next/current` o un builtin especial del backend.
- Dónde tocar:
  - `runtime/hulk_runtime.c/.h`
  - `hulk-codegen-llvm`
  - quizá builtins/types.
- Test:
  ```hulk
  for (x in range(0, 3)) print(x);
  ```
- Criterio de terminado: salida `0`, `1`, `2` y no solo IR.

#### Tarea 6: CLI de ejecución o instrucciones reproducibles

- Qué hacer: agregar `--run` o documentar pipeline `--emit-llvm > out.ll && clang out.ll runtime/hulk_runtime.c -lm -o out && ./out`.
- Dónde tocar: `crates/hulk-driver/src/main.rs`, README.
- Test: integración CLI con programa temporal.
- Criterio de terminado: un usuario clona y ejecuta un `.hulk` siguiendo README.

### Bloque de cierre semántico

#### Tarea 7: decidir política de inferencia

- Qué hacer: documentar que la inferencia es básica y exige anotaciones en parámetros, o implementar inferencia mínima para parámetros por uso aritmético/comparación.
- Dónde tocar: `crates/hulk-sema/src/hir_builder.rs`, `docs/hir.md`, README.
- Test:
  - válido si se implementa: `function inc(x) => x + 1; print(inc(2));`
  - inválido/documentado si no: debe producir error claro.
- Criterio de terminado: comportamiento explícito y tests alineados.

#### Tarea 8: reforzar LCA/conformidad en `if`

- Qué hacer: tests de ramas con tipos hermanos, padre común y Object.
- Dónde tocar: `crates/hulk-sema/tests`.
- Test: `if (cond) new B() else new C()` donde B y C heredan A.
- Criterio de terminado: tipo inferido esperado y asignabilidad correcta.

#### Tarea 9: errores con spans

- Qué hacer: propagar span en errores semánticos principales.
- Dónde tocar: `SemanticError`, HIR builder, resolver.
- Test: assert de mensaje con línea/columna para undefined variable, invalid argument, private attr.
- Criterio de terminado: errores importantes muestran ubicación útil.

### Bloque de extensión

#### Tarea 10: elegir “Protocolos estructurales” como extensión principal

- Qué hacer: formalizarla en `docs/extension_protocolos.md` o README.
- Dónde tocar: docs/reporte, tests sema.
- Test:
  ```hulk
  protocol Printable { value(): String; }
  type A { value(): String => "A"; }
  function show(x: Printable): String => x.value();
  print(show(new A()));
  ```
- Criterio de terminado: parsea, type-checkea, y si solo es compile-time, explicar que se borra después de sema. Si retorna String sin concat, puede ejecutarse tras backend de métodos.

#### Tarea 11: no priorizar lambdas salvo que sobre tiempo

- Qué hacer: dejarlas como experimental si no hay backend.
- Dónde tocar: README “Limitaciones”.
- Test: mantener golden lower pero no prometer ejecución.
- Criterio de terminado: no se presenta como feature final ejecutable si no ejecuta.

### Bloque de tests

#### Tarea 12: agregar tests CLI

- Qué hacer: tests con `Command`/`assert_cmd` para `--check`, `--ir`, `--emit-llvm`.
- Dónde tocar: `crates/hulk-driver/tests/cli.rs`.
- Test: archivo temporal con `print(42);`.
- Criterio de terminado: CI ejecuta driver y valida exit codes.

#### Tarea 13: agregar e2e obligatorios

- Qué hacer: carpeta `tests/e2e` o crate de integración que compile y ejecute programas.
- Dónde tocar: `crates/hulk-codegen-llvm/tests` o `tests/e2e`.
- Test mínimos:
  - strings concat;
  - object attr/method;
  - inheritance/base;
  - for/range;
  - invalid semantic case.
- Criterio de terminado: CI ejecuta todos con clang instalado.

#### Tarea 14: agregar verifier de IR

- Qué hacer: `hulk_ir::verify(program)`.
- Dónde tocar: `crates/hulk-ir/src/verify.rs`.
- Test: bajar golden programs y verificar invariantes.
- Criterio de terminado: every golden lowering passes verifier.

### Bloque de documentación/reporte

#### Tarea 15: escribir README mínimo real

- Qué hacer: README con proyecto, requisitos, build, test, CLI, features, extensión, limitaciones, ejemplos.
- Dónde tocar: `README.md`.
- Test: una persona externa puede seguirlo.
- Criterio de terminado: clonado limpio + comandos funcionan.

#### Tarea 16: crear ejemplos reproducibles

- Qué hacer: `examples/valid`, `examples/invalid`, `examples/extension`.
- Dónde tocar: nueva carpeta `examples/`.
- Test: script o CI que corre todos los válidos.
- Criterio de terminado: cada ejemplo aparece en README.

#### Tarea 17: reporte final

- Qué hacer: escribir reporte en `report/`.
- Dónde tocar: `report/main.tex` o equivalente.
- Test: compila a PDF si se usa LaTeX.
- Criterio de terminado: explica decisiones reales y limitaciones, no solo lista archivos.

---

## 14. Checklist final de entrega

- [ ] Build limpio en clon nuevo.
- [ ] `cargo fmt --check` pasa.
- [ ] `cargo test --workspace --all-targets --locked` pasa.
- [ ] CI en verde y con clang si se usa LLVM ejecución.
- [ ] README no vacío con instrucciones reproducibles.
- [ ] CLI documentado y probado.
- [ ] Existe comando o flujo claro para `source -> ejecución`.
- [ ] Expresiones numéricas completas.
- [ ] Strings literales y concatenación ejecutan.
- [ ] Booleanos, comparaciones y lógicos ejecutan.
- [ ] Builtins `print`, `sqrt`, `sin`, `cos`, `exp`, `log`, `rand`, `PI`, `E` documentados y probados.
- [ ] `range` ejecuta o se documenta fuera de alcance.
- [ ] Bloques y expresión global como entrypoint.
- [ ] Funciones inline y full-form.
- [ ] Llamadas y recursión.
- [ ] `let` múltiple, scopes y shadowing.
- [ ] Asignación destructiva local y, si aplica, `self.attr`.
- [ ] `if/elif/else` como expresión.
- [ ] `while` como expresión.
- [ ] `for` ejecuta o queda documentado con limitación explícita.
- [ ] `type`, atributos, métodos, constructores, `new` ejecutan.
- [ ] `self`, `base`, herencia simple y override ejecutan.
- [ ] Polimorfismo/dispatch virtual probado.
- [ ] Privacidad de atributos probada.
- [ ] `is` y `as` ejecutan o se documentan como solo IR.
- [ ] Política de inferencia documentada.
- [ ] Extensión elegida documentada.
- [ ] Extensión tiene sintaxis, semántica y tests.
- [ ] Extensión ejecuta o se justifica como compile-time.
- [ ] IR documentada y golden tests actualizados.
- [ ] Backend/runtime documentado.
- [ ] Examples válidos/invalidos presentes.
- [ ] Reporte final completo.
- [ ] Demo defendible con 3-5 programas preparados.

---

## 15. Conclusión

El proyecto está lejos de ser un esqueleto: tiene una arquitectura seria, un frontend amplio, semántica/HIR con buena intención profesional, una IR propia bastante rica y una suite de golden tests útil. Para planificación, esto es una base fuerte.

Pero todavía no está listo como compilador completo de HULK. La brecha decisiva está en ejecución. Hoy el proyecto demuestra que puede analizar y bajar muchos programas a IR, y que puede ejecutar un subconjunto primitivo vía LLVM, pero no ejecuta el núcleo orientado a objetos obligatorio: clases, objetos, atributos, métodos, herencia, dispatch dinámico y constructs derivados como `for/range` sobre iterables.

El camino más corto para terminar no es agregar más features. Es cerrar el backend existente. Dado que ya hay `hulk-codegen-llvm` y runtime C, lo más pragmático es extender esa ruta en vez de iniciar una VM desde cero, salvo que el equipo tenga una razón fuerte para volver al diseño BANNER+VM.

Qué no conviene intentar a esta altura:

- macros;
- inferencia avanzada;
- GC sofisticado;
- lambdas/closures ejecutables si todavía no corren objetos;
- optimizaciones de IR;
- reescrituras grandes del parser o AST;
- mantener simultáneamente LLVM y VM como objetivos finales.

Prioridad desde ahora:

1. Documentar el estado real y el backend elegido.
2. Arreglar README y reproducibilidad.
3. Implementar ejecución de strings concat y objetos mínimos.
4. Implementar dispatch/herencia/base.
5. Cerrar `for/range` o recortarlo explícitamente.
6. Elegir protocolos estructurales como extensión principal y documentarla.
7. Agregar end-to-end tests que demuestren lo que se va a defender.

Si el equipo hace eso, el proyecto puede convertirse en una entrega defendible: no por tener todo perfecto, sino por tener un pipeline coherente, probado, ejecutable y honestamente documentado.
