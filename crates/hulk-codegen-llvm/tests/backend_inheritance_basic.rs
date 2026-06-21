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
struct InheritanceCase {
    name: &'static str,
    source: &'static str,
    expected_stdout: &'static str,
}

const INHERITANCE_CASES: &[InheritanceCase] = &[
    InheritanceCase {
        name: "initializes inherited attributes",
        source: r#"
type Animal(name: String) {
    name: String = name;

    getName(): String => self.name;
}

type Dog(name: String) inherits Animal(name) {
    bark(): String => "woof";
}

let d = new Dog("Bolt") in {
    print(d.getName());
    print(d.bark());
};
"#,
        expected_stdout: "Bolt\nwoof\n",
    },
    InheritanceCase {
        name: "inherited method reads parent attribute",
        source: r#"
type Counter(value: Number) {
    value: Number = value;

    get(): Number => self.value;
}

type NamedCounter(name: String, value: Number) inherits Counter(value) {
    name: String = name;

    getName(): String => self.name;
}

let c = new NamedCounter("visits", 5) in {
    print(c.getName());
    print(c.get());
};
"#,
        expected_stdout: "visits\n5\n",
    },
    InheritanceCase {
        name: "override on concrete type",
        source: r#"
type Animal {
    speak(): String => "animal";
}

type Dog inherits Animal {
    speak(): String => "dog";
}

let d = new Dog() in print(d.speak());
"#,
        expected_stdout: "dog\n",
    },
    InheritanceCase {
        name: "base call in override",
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
    InheritanceCase {
        name: "base call with concat",
        source: r#"
type Animal(name: String) {
    name: String = name;

    describe(): String => "Animal " @ self.name;
}

type Dog(name: String) inherits Animal(name) {
    describe(): String => base() @ " Dog";
}

let d = new Dog("Bolt") in print(d.describe());
"#,
        expected_stdout: "Animal Bolt Dog\n",
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

    let temp_dir = match create_temp_dir("backend-inheritance-basic") {
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
fn basic_inheritance_programs_emit_llvm() {
    for case in INHERITANCE_CASES {
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
fn basic_inheritance_programs_execute_with_clang_when_available() {
    if !clang_is_available() {
        eprintln!("clang is not available; skipping LLVM inheritance execution checks");
        return;
    }

    for case in INHERITANCE_CASES {
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
