use hulk_codegen_llvm::{LlvmCodegenError, emit_llvm};
use hulk_frontend::parse_hulk_types_program;
use hulk_ir::{
    FunctionId, IrFunction, IrFunctionKind, IrInstr, IrMethod, IrProgram, IrType, IrTypeRef,
    IrValue, MethodSlot, TypeId,
};
use hulk_lower::lower_program;
use hulk_sema::analyze_program;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct TestError(String);

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for TestError {}

#[derive(Debug, Clone, Copy)]
struct VTableCase {
    name: &'static str,
    source: &'static str,
    expected_stdout: &'static str,
}

const VTABLE_CASES: &[VTableCase] = &[
    VTableCase {
        name: "simple dynamic dispatch",
        source: r#"
type Animal {
    speak(): String => "animal";
}

type Dog inherits Animal {
    speak(): String => "dog";
}

let a: Animal = new Dog() in print(a.speak());
"#,
        expected_stdout: "dog\n",
    },
    VTableCase {
        name: "two dynamic subtypes",
        source: r#"
type Animal {
    speak(): String => "animal";
}

type Dog inherits Animal {
    speak(): String => "dog";
}

type Cat inherits Animal {
    speak(): String => "cat";
}

let a: Animal = new Dog() in {
    print(a.speak());
    let b: Animal = new Cat() in print(b.speak());
};
"#,
        expected_stdout: "dog\ncat\n",
    },
    VTableCase {
        name: "inherited method keeps parent slot",
        source: r#"
type Animal(name: String) {
    name: String = name;

    getName(): String => self.name;
}

type Dog(name: String) inherits Animal(name) {
    bark(): String => "woof";
}

let a: Animal = new Dog("Bolt") in print(a.getName());
"#,
        expected_stdout: "Bolt\n",
    },
    VTableCase {
        name: "override reads inherited attribute",
        source: r#"
type Animal(name: String) {
    name: String = name;

    describe(): String => "Animal " @ self.name;
}

type Dog(name: String) inherits Animal(name) {
    describe(): String => "Dog " @ self.name;
}

let a: Animal = new Dog("Bolt") in print(a.describe());
"#,
        expected_stdout: "Dog Bolt\n",
    },
    VTableCase {
        name: "base call stays direct",
        source: r#"
type Animal {
    speak(): String => "animal";
}

type Dog inherits Animal {
    speak(): String => "dog";
    parentSpeak(): String => base();
}

let d = new Dog() in {
    print(d.speak());
    print(d.parentSpeak());
};
"#,
        expected_stdout: "dog\nanimal\n",
    },
    VTableCase {
        name: "inherited method dispatches back through dynamic receiver",
        source: r#"
type Animal {
    speak(): String => "animal";
    describe(): String => "says " @ self.speak();
}

type Dog inherits Animal {
    speak(): String => "dog";
}

let a: Animal = new Dog() in print(a.describe());
"#,
        expected_stdout: "says dog\n",
    },
];

fn lower_ir_from_source(source: &str) -> Result<IrProgram, Box<dyn Error>> {
    let program = parse_hulk_types_program(source)?;
    let semantic = analyze_program(&program)
        .map_err(|errors| TestError(format!("semantic analysis failed: {errors:?}")))?;
    let ir = lower_program(&semantic)?;
    Ok(ir)
}

fn emit_llvm_from_source(source: &str) -> Result<String, Box<dyn Error>> {
    let ir = lower_ir_from_source(source)?;
    Ok(emit_llvm(&ir)?)
}

fn codegen_error_from_source(source: &str) -> LlvmCodegenError {
    let ir = lower_ir_from_source(source).expect("source should lower to IR");
    emit_llvm(&ir).expect_err("LLVM codegen should fail cleanly")
}

fn compile_and_run_llvm(llvm: &str) -> Option<Result<String, String>> {
    if !clang_is_available() {
        return None;
    }

    let temp_dir = match create_temp_dir("backend-vtables") {
        Ok(path) => path,
        Err(err) => return Some(Err(format!("failed to create temp dir: {err}"))),
    };

    let llvm_path = temp_dir.join("program.ll");
    let bin_path = temp_dir.join("program");
    let runtime_path = runtime_source_path();

    let result = (|| -> Result<String, String> {
        fs::write(&llvm_path, llvm).map_err(|err| format!("failed to write LLVM IR: {err}"))?;

        let compile = Command::new("clang")
            .arg(&llvm_path)
            .arg(&runtime_path)
            .arg("-lm")
            .arg("-o")
            .arg(&bin_path)
            .output()
            .map_err(|err| format!("failed to invoke clang: {err}"))?;
        if !compile.status.success() {
            return Err(format!(
                "clang failed with status {}: {}",
                compile.status,
                String::from_utf8_lossy(&compile.stderr).trim()
            ));
        }

        let run = Command::new(&bin_path)
            .output()
            .map_err(|err| format!("failed to run compiled program: {err}"))?;
        if !run.status.success() {
            return Err(format!(
                "compiled program exited with status {}: {}",
                run.status,
                String::from_utf8_lossy(&run.stderr).trim()
            ));
        }

        String::from_utf8(run.stdout)
            .map_err(|err| format!("compiled program emitted invalid UTF-8: {err}"))
    })();

    let _ = fs::remove_dir_all(&temp_dir);
    Some(result)
}

fn clang_is_available() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn runtime_source_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/hulk_runtime.c")
}

fn create_temp_dir(prefix: &str) -> Result<PathBuf, std::io::Error> {
    let unique = format!(
        "{}-{}-{}-{}",
        prefix,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let path = std::env::temp_dir().join(unique);
    fs::create_dir(&path)?;
    Ok(path)
}

fn assert_unsupported_error(err: LlvmCodegenError, keywords: &[&str]) {
    let rendered = err.to_string().to_lowercase();
    assert!(
        keywords
            .iter()
            .any(|keyword| rendered.contains(&keyword.to_lowercase())),
        "expected unsupported LLVM codegen error mentioning one of {:?}, got {:?}",
        keywords,
        err
    );
    assert!(
        matches!(
            &err,
            LlvmCodegenError::UnsupportedInstruction { .. }
                | LlvmCodegenError::UnsupportedOperation { .. }
                | LlvmCodegenError::UnsupportedType { .. }
        ),
        "expected unsupported LLVM codegen error, got {:?}",
        err
    );
}

fn vtable_slot_program(methods: Vec<IrMethod>) -> IrProgram {
    let mut functions = vec![IrFunction {
        id: FunctionId(0),
        name: "entry".to_string(),
        kind: IrFunctionKind::Entry,
        params: vec![],
        locals: vec![],
        temps: vec![],
        return_type: IrTypeRef::Number,
        body: vec![IrInstr::Return(Some(IrValue::ConstNumber(0.0)))],
    }];

    for (index, method) in methods.iter().enumerate() {
        functions.push(IrFunction {
            id: FunctionId(index as u32 + 1),
            name: method.function.clone(),
            kind: IrFunctionKind::Method {
                owner_type: "Widget".to_string(),
                method_name: method.name.clone(),
            },
            params: vec![],
            locals: vec![],
            temps: vec![],
            return_type: IrTypeRef::Number,
            body: vec![IrInstr::Return(Some(IrValue::ConstNumber(index as f64)))],
        });
    }

    IrProgram {
        types: vec![IrType {
            id: TypeId(0),
            name: "Widget".to_string(),
            parent: None,
            attributes: vec![],
            methods,
        }],
        data: vec![],
        functions,
        entry: FunctionId(0),
    }
}

#[test]
fn vtable_programs_emit_llvm() {
    for case in VTABLE_CASES {
        let llvm = emit_llvm_from_source(case.source)
            .unwrap_or_else(|err| panic!("{} should emit LLVM: {err}", case.name));
        assert!(
            llvm.contains("%HulkVTable = type { i64, ptr, i64, ptr }"),
            "{} should declare vtable type",
            case.name
        );
        assert!(
            llvm.contains("declare ptr @hulk_object_method(ptr, i64)"),
            "{} should declare vtable lookup helper",
            case.name
        );
        assert!(
            llvm.contains("call ptr @hulk_object_method(ptr"),
            "{} should use runtime vtable lookup",
            case.name
        );
    }
}

#[test]
fn vtable_methods_are_emitted_in_slot_order() {
    let program = vtable_slot_program(vec![
        IrMethod {
            slot: MethodSlot(1),
            name: "second".to_string(),
            function: "Widget_second".to_string(),
        },
        IrMethod {
            slot: MethodSlot(0),
            name: "first".to_string(),
            function: "Widget_first".to_string(),
        },
    ]);

    let llvm = emit_llvm(&program).expect("codegen should pass");
    assert!(llvm.contains(
        "@Widget_vtable_methods = private constant [2 x ptr] [ptr @Widget_first, ptr @Widget_second]"
    ));
}

#[test]
fn non_contiguous_vtable_slots_fail_cleanly() {
    let program = vtable_slot_program(vec![IrMethod {
        slot: MethodSlot(1),
        name: "missing_zero".to_string(),
        function: "Widget_missing_zero".to_string(),
    }]);

    let err = emit_llvm(&program).expect_err("codegen should reject non-contiguous slots");
    match err {
        LlvmCodegenError::UnsupportedOperation { message } => {
            assert!(message.contains("non-contiguous method slots"));
            assert!(message.contains("expected slot 0"));
            assert!(message.contains("found slot 1"));
            assert!(message.contains("Widget"));
        }
        other => panic!("expected UnsupportedOperation, got {other:?}"),
    }
}

#[test]
fn vtable_programs_execute_with_clang_when_available() {
    if !clang_is_available() {
        eprintln!("clang is not available; skipping LLVM vtable execution checks");
        return;
    }

    for case in VTABLE_CASES {
        let llvm = emit_llvm_from_source(case.source)
            .unwrap_or_else(|err| panic!("{} should emit LLVM: {err}", case.name));
        let stdout = compile_and_run_llvm(&llvm)
            .expect("clang availability changed during test run")
            .unwrap_or_else(|err| panic!("{} should compile and run: {err}", case.name));
        assert_eq!(
            stdout, case.expected_stdout,
            "stdout mismatch for {}",
            case.name
        );
    }
}

#[test]
fn unsupported_vector_still_fails_cleanly() {
    let err = codegen_error_from_source("let v = [1, 2, 3] in print(v[0]);");
    assert_unsupported_error(err, &["unsupported", "newvector", "vector"]);
}

#[test]
fn unsupported_closure_still_fails_cleanly() {
    let err =
        codegen_error_from_source("let f: (Number) -> Number = (x: Number) => x + 1 in f(4);");
    assert_unsupported_error(err, &["unsupported", "closure"]);
}
