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
struct ObjectCase {
    name: &'static str,
    source: &'static str,
    expected_stdout: &'static str,
}

const OBJECT_CASES: &[ObjectCase] = &[
    ObjectCase {
        name: "box get",
        source: r#"
type Box(value: Number) {
    value: Number = value;
    get(): Number => self.value;
}

let b = new Box(10) in print(b.get());
"#,
        expected_stdout: "10\n",
    },
    ObjectCase {
        name: "point methods",
        source: r#"
type Point(x: Number, y: Number) {
    x: Number = x;
    y: Number = y;

    getX(): Number => self.x;
    getY(): Number => self.y;
    sum(): Number => self.x + self.y;
}

let p = new Point(3, 4) in {
    print(p.getX());
    print(p.getY());
    print(p.sum());
};
"#,
        expected_stdout: "3\n4\n7\n",
    },
    ObjectCase {
        name: "string attribute and concat method",
        source: r#"
type Greeter(name: String) {
    name: String = name;
    hello(): String => "Hello" @@ self.name;
}

let g = new Greeter("Hulk") in print(g.hello());
"#,
        expected_stdout: "Hello Hulk\n",
    },
    ObjectCase {
        name: "inferred method parameter",
        source: r#"
type Accumulator {
    add(x) => x + 1;
}

let a = new Accumulator() in print(a.add(41));
"#,
        expected_stdout: "42\n",
    },
    ObjectCase {
        name: "inferred constructor parameter stored on object",
        source: r#"
type Box(value) {
    get() => value;
}

let b = new Box(42) in print(b.get());
"#,
        expected_stdout: "42\n",
    },
    ObjectCase {
        name: "inferred string constructor parameter",
        source: r#"
type Greeter(name) {
    hello(): String => "Hello" @@ name;
}

let g = new Greeter("Hulk") in print(g.hello());
"#,
        expected_stdout: "Hello Hulk\n",
    },
    ObjectCase {
        name: "method calls method on same object",
        source: r#"
type Point(x: Number, y: Number) {
    x: Number = x;
    y: Number = y;

    sum(): Number => self.x + self.y;
    description(): String => "sum = " @ self.sum();
}

let p = new Point(3, 4) in print(p.description());
"#,
        expected_stdout: "sum = 7\n",
    },
    ObjectCase {
        name: "method mutates attribute",
        source: r#"
type Counter(value: Number) {
    value: Number = value;

    inc(): Number => {
        self.value := self.value + 1;
        self.value;
    };
}

let c = new Counter(0) in {
    print(c.inc());
    print(c.inc());
};
"#,
        expected_stdout: "1\n2\n",
    },
    ObjectCase {
        name: "boolean attribute",
        source: r#"
type Flag(value: Boolean) {
    value: Boolean = value;
    get(): Boolean => self.value;
}

let a = new Flag(true) in {
    let b = new Flag(false) in {
        print(a.get());
        print(b.get());
    };
};
"#,
        expected_stdout: "true\nfalse\n",
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

    let temp_dir = match create_temp_dir("backend-objects-basic") {
        Ok(path) => path,
        Err(err) => return Some(Err(format!("failed to create temp dir: {err}"))),
    };

    let llvm_path = temp_dir.join("program.ll");
    let bin_path = temp_dir.join("program");
    let runtime_path = runtime_source_path();

    let result = (|| -> Result<String, String> {
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
fn basic_object_programs_emit_llvm() {
    for case in OBJECT_CASES {
        let llvm = emit_llvm_from_source(case.source)
            .unwrap_or_else(|err| panic!("{} should emit LLVM: {err}", case.name));
        assert!(
            llvm.contains("declare ptr @hulk_alloc_object(i64, i64, ptr)"),
            "{} should declare object allocation",
            case.name
        );
        assert!(
            llvm.contains("define i32 @main()"),
            "{} should emit a main wrapper",
            case.name
        );
    }
}

#[test]
fn basic_object_programs_execute_with_clang_when_available() {
    if !clang_is_available() {
        eprintln!("clang is not available; skipping LLVM object execution checks");
        return;
    }

    for case in OBJECT_CASES {
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
