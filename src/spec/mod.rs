// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC SPECIFICATION LANGUAGE                          │
// │  The surface language users write specs in               │
// └──────────────────────────────────────────────────────────┘

pub mod parser;
pub mod typeck;
pub mod ast;

// Re-export core types
pub use ast::{Spec, FunctionSpec, Constraint, Param, TypeRef, Invariant};
pub use parser::parse;
pub use typeck::check;
