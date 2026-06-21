use hulk_codegen_llvm::emit_llvm;
use hulk_frontend::parse_hulk_types_program;
use hulk_ir::IrProgram;
use hulk_lower::lower_program;
use hulk_sema::analyze_program;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
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
struct TypeOpCase {
    name: &'static str,
    source: &'static str,
    expected_stdout: &'static str,
}

const TYPE_OP_CASES: &[TypeOpCase] = &[
    TypeOpCase {
        name: "is exact type",
        source: r#"
type Animal {}
type Dog inherits Animal {}

let d = new Dog() in print(d is Dog);
"#,
        expected_stdout: "true\n",
    },
    TypeOpCase {
        name: "is parent type",
        source: r#"
type Animal {}
type Dog inherits Animal {}

let d = new Dog() in print(d is Animal);
"#,
        expected_stdout: "true\n",
    },
    TypeOpCase {
        name: "is sibling type false",
        source: r#"
type Animal {}
type Dog inherits Animal {}
type Cat inherits Animal {}

let a: Animal = new Dog() in print(a is Cat);
"#,
        expected_stdout: "false\n",
    },
    TypeOpCase {
        name: "as valid subtype",
        source: r#"
type Animal {}
type Dog inherits Animal {
    bark(): String => "woof";
}

let a: Animal = new Dog() in {
    let d: Dog = (a as Dog) in print(d.bark());
};
"#,
        expected_stdout: "woof\n",
    },
    TypeOpCase {
        name: "as valid parent keeps dynamic dispatch",
        source: r#"
type Animal {
    speak(): String => "animal";
}

type Dog inherits Animal {
    speak(): String => "dog";
}

let d = new Dog() in {
    let a: Animal = (d as Animal) in print(a.speak());
};
"#,
        expected_stdout: "dog\n",
    },
];

const INVALID_CAST_SOURCE: &str = r#"
type Animal {}
type Dog inherits Animal {}
type Cat inherits Animal {}

let a: Animal = new Cat() in {
    let d: Dog = (a as Dog) in print("bad");
};
"#;

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

fn compile_and_run_llvm(llvm: &str) -> Option<Result<Output, String>> {
    if !clang_is_available() {
        return None;
    }

    let temp_dir = match create_temp_dir("backend-type-ops") {
        Ok(path) => path,
        Err(err) => return Some(Err(format!("failed to create temp dir: {err}"))),
    };

    let llvm_path = temp_dir.join("program.ll");
    let bin_path = temp_dir.join("program");
    let runtime_path = runtime_source_path();

    let result = (|| -> Result<Output, String> {
        fs::write(&llvm_path, llvm).map_err(|err| format!("failed to write LLVM IR: {err}"))?;

        let needs_opaque_flag = Command::new("clang")
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| {
                s.split_whitespace()
                    .skip_while(|t| *t != "version")
                    .nth(1)
                    .and_then(|v| v.split('.').next())
                    .and_then(|maj| maj.parse::<u32>().ok())
            })
            .map(|maj| maj < 16)
            .unwrap_or(false);

        let mut clang_cmd = Command::new("clang");
        if needs_opaque_flag {
            clang_cmd.arg("-mllvm").arg("-opaque-pointers");
        }
        let compile = clang_cmd
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

        Command::new(&bin_path)
            .output()
            .map_err(|err| format!("failed to run compiled program: {err}"))
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

#[test]
fn type_ops_programs_emit_llvm() {
    for case in TYPE_OP_CASES {
        let llvm = emit_llvm_from_source(case.source)
            .unwrap_or_else(|err| panic!("{} should emit LLVM: {err}", case.name));
        assert!(
            llvm.contains("declare i8 @hulk_object_is(ptr, i64)"),
            "{} should declare runtime type test helper",
            case.name
        );
        assert!(
            llvm.contains("declare ptr @hulk_object_as(ptr, i64)"),
            "{} should declare runtime cast helper",
            case.name
        );
    }

    let llvm = emit_llvm_from_source(INVALID_CAST_SOURCE)
        .unwrap_or_else(|err| panic!("invalid cast case should emit LLVM: {err}"));
    assert!(llvm.contains("call ptr @hulk_object_as(ptr"));
}

#[test]
fn type_ops_programs_execute_with_clang_when_available() {
    if !clang_is_available() {
        eprintln!("clang is not available; skipping LLVM type op execution checks");
        return;
    }

    for case in TYPE_OP_CASES {
        let llvm = emit_llvm_from_source(case.source)
            .unwrap_or_else(|err| panic!("{} should emit LLVM: {err}", case.name));
        let output = compile_and_run_llvm(&llvm)
            .expect("clang availability changed during test run")
            .unwrap_or_else(|err| panic!("{} should compile and run: {err}", case.name));
        assert!(
            output.status.success(),
            "{} should exit successfully: {}",
            case.name,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            case.expected_stdout,
            "stdout mismatch for {}",
            case.name
        );
    }
}

#[test]
fn invalid_cast_fails_at_runtime_with_clang_when_available() {
    if !clang_is_available() {
        eprintln!("clang is not available; skipping LLVM invalid cast execution check");
        return;
    }

    let llvm = emit_llvm_from_source(INVALID_CAST_SOURCE)
        .unwrap_or_else(|err| panic!("invalid cast case should emit LLVM: {err}"));
    let output = compile_and_run_llvm(&llvm)
        .expect("clang availability changed during test run")
        .unwrap_or_else(|err| panic!("invalid cast case should compile: {err}"));

    assert!(
        !output.status.success(),
        "invalid cast should exit with a non-zero status"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("invalid type cast"),
        "stderr should mention invalid type cast, got: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
}
