pub mod builtins;
pub mod checker;
pub mod context;
pub mod error;
pub mod resolver;
pub mod scope;
pub mod symbols;
pub mod types;

pub use checker::check_program;
pub use resolver::resolve_program;
