// ┌────────────────────────────────────────────────────────────┐
// │  MORPHIC — Self-Synthesizing Programming Language         │
// │  "Write specs. The compiler writes the implementation."   │
// │                                                            │
// │  Library root (Phase 66, v0.6 W1). The compiler core as   │
// │  a library: the CLI in src/main.rs is a thin shell over   │
// │  this crate, and crates/morphic-wasm wraps it for the     │
// │  browser. One codebase, two shells — the playground is    │
// │  a configuration of THIS compiler (feature seams in       │
// │  Cargo.toml), never a forked "browser compiler".          │
// │                                                            │
// │  The `llm` module is deliberately NOT part of the         │
// │  library: nothing in the pipeline imports it (the         │
// │  engine's LLM seed call is self-contained, feature-       │
// │  gated inline code), and its HTTP client must never       │
// │  enter the WASM dependency graph. It stays in the         │
// │  binary target, beside the CLI's fs/process surface.      │
// └────────────────────────────────────────────────────────────┘

pub mod api;
pub mod codegen;
pub mod lsp;
pub mod mcp;
pub mod pcc;
pub mod spec;
pub mod stdlib;
pub mod synthesis;
pub mod timeutil;
pub mod verify;
