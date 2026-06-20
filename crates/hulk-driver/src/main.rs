use hulk_codegen_llvm::emit_llvm;
use hulk_frontend::parse_hulk_types_program;
use hulk_lower::lower_program;
use hulk_sema::{analyze_program, check_program};
use std::{env, fs, path::PathBuf, process};

fn main() {
    let args: Vec<String> = env::args().collect();
    let (path, mode) = parse_args(&args);

    let source = fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("error: cannot read '{}': {}", path, e);
        process::exit(1);
    });

    // ── Parse ─────────────────────────────────────────────────────────────────
    let program = match parse_hulk_types_program(&source) {
        Ok(p) => p,
        Err(e) => {
            let msg = e.to_string();
            // Lexical errors contain "lex error" in the message; distinguish from
            // pure syntactic (grammar) errors with separate exit codes.
            if msg.contains("lex error") || msg.contains("Unterminated") || msg.contains("unexpected character") {
                eprintln!("LEXICAL error: {}", e);
                process::exit(1);
            }
            eprintln!("SYNTACTIC error: {}", e);
            process::exit(2);
        }
    };

    match mode {
        Mode::Compile => {
            compile_to_binary(&program);
        }
        Mode::Debug {
            show_ast,
            show_check,
            show_hir,
            show_ir,
            show_llvm,
        } => {
            if show_ast {
                println!("══════════════════════════════════════════");
                println!("  AST");
                println!("══════════════════════════════════════════");
                print_ast(&program);
            }

            if show_check {
                println!();
                println!("══════════════════════════════════════════");
                println!("  Type Check");
                println!("══════════════════════════════════════════");
                match check_program(&program) {
                    Ok(()) => println!("✓  no errors"),
                    Err(errors) => {
                        print_semantic_errors(&errors);
                    }
                }
            }

            if show_hir {
                println!();
                println!("══════════════════════════════════════════");
                println!("  HIR");
                println!("══════════════════════════════════════════");
                match analyze_program(&program) {
                    Ok(semantic) => println!("{:#?}", semantic.hir),
                    Err(errors) => {
                        print_semantic_errors(&errors);
                    }
                }
            }

            if show_ir {
                println!();
                println!("══════════════════════════════════════════");
                println!("  IR");
                println!("══════════════════════════════════════════");
                match analyze_program(&program) {
                    Ok(semantic) => match lower_program(&semantic) {
                        Ok(ir) => println!("{}", ir),
                        Err(error) => {
                            eprintln!("lowering error: {}", error);
                            process::exit(4);
                        }
                    },
                    Err(errors) => {
                        print_semantic_errors(&errors);
                    }
                }
            }

            if show_llvm {
                match analyze_program(&program) {
                    Ok(semantic) => match lower_program(&semantic) {
                        Ok(ir) => match emit_llvm(&ir) {
                            Ok(llvm) => print!("{llvm}"),
                            Err(error) => {
                                eprintln!("LLVM codegen error: {}", error);
                                process::exit(5);
                            }
                        },
                        Err(error) => {
                            eprintln!("lowering error: {}", error);
                            process::exit(4);
                        }
                    },
                    Err(errors) => {
                        print_semantic_errors(&errors);
                    }
                }
            }
        }
    }
}

fn compile_to_binary(program: &hulk_frontend::ast::Program) {
    let semantic = match analyze_program(program) {
        Ok(s) => s,
        Err(errors) => {
            for e in &errors {
                eprintln!("SEMANTIC error: {}", e);
            }
            process::exit(3);
        }
    };

    let ir = match lower_program(&semantic) {
        Ok(ir) => ir,
        Err(e) => {
            eprintln!("lowering error: {}", e);
            process::exit(4);
        }
    };

    let llvm = match emit_llvm(&ir) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("LLVM codegen error: {}", e);
            process::exit(5);
        }
    };

    // Write IR to a temp file
    let ir_path = PathBuf::from("output.ll");
    fs::write(&ir_path, &llvm).unwrap_or_else(|e| {
        eprintln!("error: cannot write IR: {}", e);
        process::exit(6);
    });

    // Find runtime: look next to the executable, then in CWD
    let runtime_path = find_runtime();

    // Invoke clang to produce ./output
    // clang < 16 requires -opaque-pointers to handle `ptr` type; clang 16+ removed the flag
    let needs_opaque_flag = process::Command::new("clang")
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

    let mut cmd = process::Command::new("clang");
    if needs_opaque_flag {
        cmd.arg("-mllvm").arg("-opaque-pointers");
    }
    let status = cmd
        .arg(&ir_path)
        .arg(&runtime_path)
        .arg("-lm")
        .arg("-o")
        .arg("output")
        .status()
        .unwrap_or_else(|e| {
            eprintln!("error: cannot invoke clang: {}", e);
            process::exit(7);
        });

    let _ = fs::remove_file(&ir_path);

    if !status.success() {
        process::exit(status.code().unwrap_or(8));
    }
}

fn find_runtime() -> PathBuf {
    // Try relative to the executable (repo_root/runtime/hulk_runtime.c)
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("runtime").join("hulk_runtime.c");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    // Fallback: relative to CWD
    PathBuf::from("runtime/hulk_runtime.c")
}

// ── Pretty-print the AST ─────────────────────────────────────────────────────

fn print_ast(program: &hulk_frontend::ast::Program) {
    println!("declarations: {}", program.declarations.len());
    for (i, decl) in program.declarations.iter().enumerate() {
        println!("\n[{}] {}", i, decl_summary(decl));
        println!("{:#?}", decl);
    }
    println!("\nentry expression:");
    println!("{:#?}", program.entry);
}

fn decl_summary(decl: &hulk_frontend::ast::Decl) -> String {
    use hulk_frontend::ast::Decl;
    match decl {
        Decl::Function(f) => format!("function {}", f.name),
        Decl::Type(t) => format!("type {}", t.name),
        Decl::Protocol(p) => format!("protocol {}", p.name),
    }
}

fn print_semantic_errors(errors: &[hulk_sema::error::SemanticError]) -> ! {
    for err in errors {
        eprintln!("SEMANTIC error: {}", err);
    }
    process::exit(3);
}

// ── Argument parsing ──────────────────────────────────────────────────────────

enum Mode {
    Compile,
    Debug {
        show_ast: bool,
        show_check: bool,
        show_hir: bool,
        show_ir: bool,
        show_llvm: bool,
    },
}

fn parse_args(args: &[String]) -> (String, Mode) {
    if args.len() < 2 {
        eprintln!(
            "usage: hulkc <file.hulk> [--ast] [--check] [--hir|--dump-hir] [--ir] [--emit-llvm] [--all]"
        );
        eprintln!("  (no flags)   compile and produce ./output");
        eprintln!("  --ast        print the AST");
        eprintln!("  --check      run the type checker");
        eprintln!("  --hir        print the HIR produced by semantic analysis");
        eprintln!("  --dump-hir   alias for --hir");
        eprintln!("  --ir         print the lowered IR");
        eprintln!("  --emit-llvm  print generated LLVM IR");
        eprintln!("  --all        print AST and run type checker");
        process::exit(1);
    }

    let path = args[1].clone();
    let flags: Vec<&str> = args[2..].iter().map(String::as_str).collect();

    if flags.is_empty() {
        return (path, Mode::Compile);
    }

    let show_ast = flags.contains(&"--ast") || flags.contains(&"--all");
    let show_check = flags.contains(&"--check") || flags.contains(&"--all");
    let show_hir = flags.contains(&"--hir") || flags.contains(&"--dump-hir");
    let show_ir = flags.contains(&"--ir");
    let show_llvm = flags.contains(&"--emit-llvm");

    (
        path,
        Mode::Debug {
            show_ast,
            show_check,
            show_hir,
            show_ir,
            show_llvm,
        },
    )
}
