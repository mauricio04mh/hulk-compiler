use hulk_codegen_llvm::{LlvmCodegenError, emit_llvm};
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
struct SupportedCase {
    name: &'static str,
    source: &'static str,
    expected_stdout: &'static str,
}

const SUPPORTED_CASES: &[SupportedCase] = &[
    SupportedCase {
        name: "print number",
        source: "print(42);",
        expected_stdout: "42\n",
    },
    SupportedCase {
        name: "arithmetic",
        source: "{ print(1 + 2 * 3); print((10 - 4) / 2); print(2 ^ 3); }",
        expected_stdout: "7\n3\n8\n",
    },
    SupportedCase {
        name: "booleans",
        source: "{ print(true); print(false); print(true | false); print(true & false); print(!false); }",
        expected_stdout: "true\nfalse\ntrue\nfalse\ntrue\n",
    },
    SupportedCase {
        name: "comparisons",
        source: "{ print(3 < 5); print(3 <= 3); print(5 > 10); print(4 == 4); print(4 != 5); }",
        expected_stdout: "true\ntrue\nfalse\ntrue\ntrue\n",
    },
    SupportedCase {
        name: "global function call",
        source: "function inc(x: Number): Number => x + 1;\n\nprint(inc(4));",
        expected_stdout: "5\n",
    },
    SupportedCase {
        name: "recursive function",
        source: "function fact(n: Number): Number =>\n    if (n <= 1) 1 else n * fact(n - 1);\n\nprint(fact(5));",
        expected_stdout: "120\n",
    },
    SupportedCase {
        name: "while + assignment",
        source: "let x: Number = 0 in {\n    while (x < 3) {\n        x := x + 1;\n        print(x);\n    };\n};",
        expected_stdout: "1\n2\n3\n",
    },
    SupportedCase {
        name: "math builtins",
        source: "{ print(sqrt(9)); print(sin(0)); print(cos(0)); print(exp(0)); }",
        expected_stdout: "3\n0\n1\n1\n",
    },
    SupportedCase {
        name: "log builtin",
        source: "print(log(4, 64));",
        expected_stdout: "3\n",
    },
    SupportedCase {
        name: "print string",
        source: "print(\"Hello\");",
        expected_stdout: "Hello\n",
    },
    SupportedCase {
        name: "string literals",
        source: "{ print(\"Hello\"); print(\"A B\"); }",
        expected_stdout: "Hello\nA B\n",
    },
    SupportedCase {
        name: "string concat literals",
        source: "{ print(\"Hello\" @ \"World\"); }",
        expected_stdout: "HelloWorld\n",
    },
    SupportedCase {
        name: "string concat space",
        source: "{ print(\"Hello\" @@ \"World\"); }",
        expected_stdout: "Hello World\n",
    },
    SupportedCase {
        name: "string concat number",
        source: "{ print(\"x = \" @ 42); }",
        expected_stdout: "x = 42\n",
    },
    SupportedCase {
        name: "string concat bool",
        source: "{ print(\"ok = \" @ true); print(\"ok = \" @ false); }",
        expected_stdout: "ok = true\nok = false\n",
    },
    SupportedCase {
        name: "chained string concat",
        source: "{ print(\"a\" @ \"b\" @ \"c\"); }",
        expected_stdout: "abc\n",
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

    let temp_dir = match create_temp_dir("backend-minimal") {
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

fn codegen_error_from_source(source: &str) -> LlvmCodegenError {
    let ir = lower_ir_from_source(source).expect("source should lower to IR");
    emit_llvm(&ir).expect_err("LLVM codegen should fail cleanly")
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

#[test]
fn supported_minimal_programs_emit_llvm() {
    for case in SUPPORTED_CASES {
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
fn supported_minimal_programs_execute_with_clang_when_available() {
    if !clang_is_available() {
        eprintln!("clang is not available; skipping LLVM execution checks");
        return;
    }

    for case in SUPPORTED_CASES {
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
fn unsupported_vector_fails_cleanly() {
    let err = codegen_error_from_source("let v = [1, 2, 3] in print(v[0]);");
    assert_unsupported_error(err, &["unsupported", "newvector", "vector"]);
}

#[test]
fn unsupported_inherited_object_fails_cleanly() {
    let err = codegen_error_from_source(
        "type Parent {\n    x: Number = 10;\n}\n\ntype Child inherits Parent {\n    y: Number = 20;\n    getY(): Number => self.y;\n}\n\nlet c = new Child() in print(c.getY());",
    );
    assert_unsupported_error(err, &["unsupported", "inheritance"]);
}
