# 🤝 Contributing to Morphic

> v0.1.0 — Pre-alpha. Everything is in flux. Breaking changes are expected.
> Your contributions shape the foundation.

---

## Code of Conduct

Respect the synthesis. Respect each other. Constructive feedback only.

---

## Getting Started

### Prerequisites
- **Rust** 1.75+ (`rustup install stable`)
- **Git** 2.40+
- **Z3** 4.12+ (optional, for verification backend)
- **Ollama** or llama.cpp (optional, for LLM synthesis)

### Setup

```bash
git clone https://github.com/thvtzy/morphic.git
cd morphic
cargo build

# With Z3 verification (recommended):
cargo build --features z3-static

# With local LLM:
cargo build --features llm-local

# Everything:
cargo build --all-features
```

### Run

```bash
# Build a spec
cargo run -- build examples/sort.morph

# Interactive synthesis
cargo run -- synthesize examples/sort.morph

# Run tests
cargo test
```

---

## Architecture Quick-Start

Read `ARCHITECTURE.md` for the full pipeline.

The codebase is organized as a compiler pipeline:

```
src/
├── main.rs              ← CLI (entry point, argument parsing)
├── spec/                 ← The .morph language
│   ├── ast.rs            ← AST types (read this first)
│   ├── parser.rs         ← Source → AST
│   └── typeck.rs         ← Type checker
├── synthesis/            ← The synthesis engine
│   ├── engine.rs         ← Core (MCTS + GA + LLM)
│   ├── selector.rs       ← Candidate selection
│   └── interactive.rs    ← Live UI
├── verify/               ← Formal verification
│   ├── verifier.rs       ← Z3 pipeline
│   └── smt.rs            ← SMT-LIB2 translation
└── codegen/              ← Code generation
    └── mod.rs            ← Multi-target output
```

**Recommended reading order for new contributors:**
1. `README.md` — what Morphic is
2. `ARCHITECTURE.md` — how it works
3. `src/spec/ast.rs` — the data model
4. `src/spec/parser.rs` — how specs are parsed
5. `src/synthesis/engine.rs` — the core algorithm
6. `src/codegen/mod.rs` — how IR becomes code

---

## What We Need Help With

### 🔥 Critical Path (v0.2)

| Task | Difficulty | Description |
|---|---|---|
| Z3 FFI integration | Hard | Replace subprocess with `z3` crate FFI for faster verification |
| Syn crate integration | Medium | Use `syn` crate to parse Rust LLM output → IR |
| Template library | Medium | Build out the template pattern library (divide-conquer, DP, greedy, etc.) |
| IR ↔ Rust roundtrip | Hard | Convert IR to Rust AST and back (for self-hosting) |

### 🟡 Nice to Have

| Task | Difficulty | Description |
|---|---|---|
| WASM playground | Medium | Morphic in the browser via WASM |
| VSCode extension | Medium | Syntax highlighting + LSP for .morph files |
| More examples | Easy | Write .morph specs for classic algorithms |
| Documentation | Easy | Improve docs, add diagrams, write tutorials |
| Property-based test generation | Medium | Auto-generate test cases from constraints |

### 🟢 Good First Issues

| Task | Description |
|---|---|
| Add more template patterns | `src/synthesis/engine.rs` — `build_divide_and_conquer`, etc. need real implementations |
| Improve parser error messages | `src/spec/parser.rs` — make errors more helpful |
| Add more IR node visitors | `src/synthesis/engine.rs` — implement the stub functions |
| Write example specs | `examples/` — add .morph files for interesting problems |
| Improve README diagrams | ASCII art or mermaid diagrams |

---

## Development Workflow

1. **Fork** the repo
2. **Branch**: `feature/your-feature-name` or `fix/your-fix-name`
3. **Commit**: conventional commits (`feat:`, `fix:`, `docs:`, `refactor:`)
4. **Test**: `cargo test` must pass
5. **Format**: `cargo fmt` before committing
6. **PR**: describe what and why, reference issues

### Commit Message Format

```
type(scope): short description

Longer explanation if needed. What, why, how.

Co-Authored-By: Your Name <email>
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`

---

## Testing

```bash
# Unit tests
cargo test

# Specific module
cargo test --lib spec::parser

# With all features
cargo test --all-features

# Benchmarks (future)
cargo bench
```

---

## Questions?

Open an issue or start a discussion. This is early-stage — your questions help shape the documentation.

---

*Built with Rust, Z3, and the conviction that compilers should do more work than programmers.*
