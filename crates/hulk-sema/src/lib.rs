pub mod analysis;
pub mod builtins;
pub mod checker;
pub mod context;
pub mod error;
pub mod hir;
mod hir_builder;
pub mod resolver;
pub mod scope;
pub mod symbols;
pub mod types;

pub use analysis::analyze_program;
pub use checker::check_program;
pub use resolver::resolve_program;
