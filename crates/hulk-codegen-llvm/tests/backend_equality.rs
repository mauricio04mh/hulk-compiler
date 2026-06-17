use hulk_codegen_llvm::emit_llvm;
use hulk_frontend::parse_hulk_types_program;
use hulk_ir::IrProgram;
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
struct EqualityCase {
    name: &'static str,
    source: &'static str,
    expected_stdout: &'static str,
}

const EQUALITY_CASES: &[EqualityCase] = &[
    EqualityCase {
        name: "string literal equality",
        source: r#"
{
    print("hello" == "hello");
    print("hello" == "world");
    print("hello" != "world");
}
"#,
        expected_stdout: "true\nfalse\ntrue\n",
    },
    EqualityCase {
        name: "dynamic string equality after concat",
        source: r#"
{
    let s = "he" @ "llo" in {
        print(s == "hello");
        print(s != "hello");
    };
}
"#,
        expected_stdout: "true\nfalse\n",
    },
    EqualityCase {
        name: "object identity same reference",
        source: r#"
type Box {}

let a = new Box() in {
    let b = a in print(a == b);
};
"#,
        expected_stdout: "true\n",
    },
    EqualityCase {
        name: "object identity different objects",
        source: r#"
type Box {}

let a = new Box() in {
    let b = new Box() in print(a == b);
};
"#,
        expected_stdout: "false\n",
    },
    EqualityCase {
        name: "object identity across parent type",
        source: r#"
type Animal {}
type Dog inherits Animal {}

let d = new Dog() in {
    let a: Animal = d in {
        print(a == d);
        print(a != d);
    };
};
"#,
        expected_stdout: "true\nfalse\n",
    },
    EqualityCase {
        name: "number and boolean equality still works",
        source: r#"
{
    print(4 == 4);
    print(4 != 5);
    print(true == true);
    print(true != false);
}
"#,
        expected_stdout: "true\ntrue\ntrue\ntrue\n",
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

fn compile_and_run_llvm(llvm: &str) -> Option<Result<String, String>> {
    if !clang_is_available() {
        return None;
    }

    let temp_dir = match create_temp_dir("backend-equality") {
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

#[test]
fn equality_programs_emit_llvm() {
    for case in EQUALITY_CASES {
        let llvm = emit_llvm_from_source(case.source)
            .unwrap_or_else(|err| panic!("{} should emit LLVM: {err}", case.name));
        assert!(
            llvm.contains("define i32 @main()"),
            "{} should emit a main wrapper",
            case.name
        );
    }
}

#[test]
fn equality_programs_execute_with_clang_when_available() {
    if !clang_is_available() {
        eprintln!("clang is not available; skipping LLVM equality execution checks");
        return;
    }

    for case in EQUALITY_CASES {
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
