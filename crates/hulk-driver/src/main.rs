use hulk_frontend::parse_hulk_types_program;
use hulk_sema::check_program;
use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();
    let (path, show_ast, show_check) = parse_args(&args);

    let source = fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("error: cannot read '{}': {}", path, e);
        process::exit(1);
    });

    // ── Parse ─────────────────────────────────────────────────────────────────
    let program = match parse_hulk_types_program(&source) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("parse error: {}", e);
            process::exit(2);
        }
    };

    // ── AST dump ──────────────────────────────────────────────────────────────
    if show_ast {
        println!("══════════════════════════════════════════");
        println!("  AST");
        println!("══════════════════════════════════════════");
        print_ast(&program);
    }

    // ── Semantic check ────────────────────────────────────────────────────────
    if show_check {
        println!();
        println!("══════════════════════════════════════════");
        println!("  Type Check");
        println!("══════════════════════════════════════════");
        match check_program(&program) {
            Ok(()) => println!("✓  no errors"),
            Err(errors) => {
                for err in &errors {
                    println!("✗  {}", err);
                }
                process::exit(3);
            }
        }
    }
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

// ── Argument parsing ──────────────────────────────────────────────────────────

fn parse_args(args: &[String]) -> (String, bool, bool) {
    if args.len() < 2 {
        eprintln!("usage: hulkc <file.hulk> [--ast] [--check] [--all]");
        eprintln!("  --ast    print the AST");
        eprintln!("  --check  run the type checker (default when no flag given)");
        eprintln!("  --all    print AST and run type checker");
        process::exit(1);
    }

    let path = args[1].clone();
    let flags: Vec<&str> = args[2..].iter().map(String::as_str).collect();

    let show_ast   = flags.contains(&"--ast")   || flags.contains(&"--all") || flags.is_empty();
    let show_check = flags.contains(&"--check") || flags.contains(&"--all") || flags.is_empty();

    (path, show_ast, show_check)
}
