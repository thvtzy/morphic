# Changelog

All notable changes to Morphic will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.2.0] — 2026-06-09

### Added
- **Real Z3 FFI integration** — z3 crate v0.20 via `gh-release` (prebuilt binary, no cmake needed)
- **Z3Session** — thread-local context API matching v0.20 design
- **Constraint → Z3 translation** — Int/Bool encoding for pre/postconditions
- **Verification tests** — 3 Z3 tests: SAT, UNSAT, arithmetic correctness
- **Default feature: z3-support** — Z3 verification enabled out of the box

### Changed
- **z3 crate**: 0.6 → 0.20 (major API refactor)
- **smt.rs**: Rewritten for real Z3 FFI (was subprocess-based)
- **verifier.rs**: Rewritten with Z3 constraint encoding
- **Cargo.toml**: `z3-support` uses `gh-release` (auto-downloads Z3 binary)

### Fixed
- All 16 v0.1 compile errors resolved
- Parser tests fixed
- HashMap/rayon imports throughout codebase

---

## [0.1.0] — 2026-06-09

### Added
- **Initial release** — Self-Synthesizing Programming Language
- `.morph` specification language (parser, AST, type checker)
- MCTS + Genetic Algorithm + LLM hybrid synthesis engine
- Z3 theorem prover integration for formal verification (SMT-LIB2)
- Multi-target code generation: Rust, C, WASM, Python, JavaScript
- CLI: `build`, `check`, `synthesize`, `init`, `lsp`
- 30+ IR node types for architecture-agnostic representation
- Pareto-optimal candidate selection
- Interactive synthesis mode with live dashboard
- Example specs: sort, binary search
- Full documentation: README, ARCHITECTURE, CONTRIBUTING

### Known Limitations
- Z3 integration uses subprocess (FFI bindings planned for v0.2)
- LLM integration requires external API or local model
- Template library is minimal (stub implementations)
- No self-hosting yet (compiler not written in Morphic)
- No WASM playground
- LSP is planned but not implemented
