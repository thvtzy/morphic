# ⚡ Morphic — Self-Synthesizing Programming Language

<p align="center">
  <a href="https://github.com/thvtzy/morphic/blob/master/LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20%7C%20Apache--2.0-blue.svg?style=for-the-badge" alt="License: MIT OR Apache-2.0"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/language-Rust-orange.svg?style=for-the-badge" alt="Language: Rust"></a>
  <a href="https://github.com/thvtzy/morphic/actions"><img src="https://img.shields.io/github/actions/workflow/status/thvtzy/morphic/ci.yml?branch=master&style=for-the-badge&label=CI" alt="CI Status"></a>
  <br>
  <a href="https://github.com/thvtzy/morphic/blob/master/CHANGELOG.md"><img src="https://img.shields.io/badge/version-v0.4.0-brightgreen?style=for-the-badge" alt="Version: v0.4.0"></a>
  <img src="https://img.shields.io/badge/status-alpha-orange?style=for-the-badge" alt="Status: Alpha">
  <img src="https://img.shields.io/badge/z3-verified-blue?style=for-the-badge" alt="Z3 Verified">
  <img src="https://img.shields.io/badge/LLM-Ollama-purple?style=for-the-badge" alt="LLM: Ollama">
  <br>
  <a href="https://github.com/thvtzy/morphic/stargazers"><img src="https://img.shields.io/github/stars/thvtzy/morphic?style=for-the-badge&color=yellow" alt="Stars"></a>
  <a href="https://github.com/thvtzy/morphic/tree/master/src"><img src="https://img.shields.io/badge/tests-19%20passed-green?style=for-the-badge" alt="Tests: 19"></a>
</p>

<p align="center">
  <b>Write specifications. The compiler writes the code.</b><br>
  Not autocomplete. Not copilot. <i>Synthesis.</i>
</p>

---

## 🤔 The Problem

Every programming language in history asks you to write **HOW**. You write the algorithm. You write the loop. You write the data structure. You are a human compiler, translating intent into mechanics.

This made sense when computers were calculators. It makes no sense when we have theorem provers, language models, and search algorithms.

## ⚡ The Morphic Answer

**Morphic asks you to write WHAT. The compiler figures out HOW.**

```morphic
// Don't write quicksort. Write WHAT sorting means:
spec sort<T> {
    input: list: List<T>, cmp: fn(T, T) -> Bool
    output: List<T>

    constraint: is_permutation(list, output)   // Same elements
    constraint: is_sorted(output, cmp)          // In order

    optimize: time < O(n log n)
    optimize: space < O(n)

    test {
        input: List[3, 1, 4, 1, 5, 9]
        expect: List[1, 1, 3, 4, 5, 9]
    }
}
```

**The compiler synthesizes, verifies, and outputs an optimized implementation** — quicksort, mergesort, or something the LLM invents that's even better for your specific constraints.

---

## 🏗 Architecture

```
                    ┌──────────────┐
                    │  .morph file │  ← You write this
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │    PARSER    │  Hand-written recursive descent
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │   TYPE CK    │  Full type inference + constraint typing
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
     ┌────────▼─────┐ ┌───▼────┐ ┌─────▼──────┐
     │  LLM Seeding │ │Template│ │ Test-Induce│  ← Phase 1: Seed Candidates
     └────────┬─────┘ └───┬────┘ └─────┬──────┘
              │            │            │
              └────────────┼────────────┘
                           │
                    ┌──────▼───────┐
                    │  MCTS SEARCH │  ← Phase 2: Monte Carlo Tree Search
                    │  over IR AST │     over implementation space
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │   Z3 VERIFY  │  ← Phase 3: Formal verification
                    │  (SMT-LIB2)  │     Each candidate is proven correct
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │ GENETIC REFINE│ ← Phase 4: Crossover + Mutation
                    │  (GA on IR)  │     Evolution toward optimal
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │   CODEGEN    │  Output: Rust, C, WASM, Python, JS
                    └──────────────┘
```

### The Synthesis Pipeline

| Phase | Technique | Role |
|---|---|---|
| **1. Seed** | LLM + Templates + Test Induction | Generate diverse initial candidates |
| **2. Search** | Monte Carlo Tree Search | Explore implementation space intelligently |
| **3. Verify** | Z3 Theorem Prover (SMT-LIB2) | Formally verify every candidate |
| **4. Refine** | Genetic Algorithm | Crossover, mutate toward optimal |
| **5. Select** | Pareto-optimal frontier | Choose best tradeoff |
| **6. Generate** | Multi-target codegen | Rust / C / WASM / Python / JS |

---

## 🚀 Quick Start

```bash
# Install (requires Rust toolchain)
git clone https://github.com/thvtzy/morphic
cd morphic
cargo build --release

# Create a new project
morphic init my-sort
cd my-sort

# Build (synthesize implementation)
morphic build main.morph --target rust

# Watch synthesis in real-time
morphic synthesize main.morph
```

### Output

`morphic build main.morph` outputs:

```rust
// Auto-generated by Morphic — verified correct, complexity O(n log n)
//
// Synthesized via MCTS (4,327 candidates evaluated)
// Z3-verified: all 15 constraints hold

pub fn sort<T: Ord + Clone>(list: Vec<T>) -> Vec<T> {
    if list.len() <= 1 {
        return list;
    }
    let pivot = list[0].clone();
    let (left, right): (Vec<_>, Vec<_>) = list[1..]
        .iter()
        .partition(|x| x < &&pivot);
    let mut result = sort(left);
    result.push(pivot);
    result.extend(sort(right));
    result
}

#[cfg(test)]
mod tests { /* ... all test cases generated ... */ }
```

---

## 🧬 Key Innovations

### 1. IR-First Design
The Intermediate Representation (IR) is the heart of Morphic. It's:
- **Language-agnostic** — one IR, multiple output languages
- **Mutable by design** — holes, nodes that expect synthesis to fill them
- **Verifiable** — direct lowering to SMT-LIB2
- **Evolvable** — crossover and mutation operators work at the IR level

### 2. Hybrid Synthesis
Morphic combines three synthesis paradigms:
- **Deductive** (test cases → pattern matching)
- **Stochastic** (MCTS + genetic search)
- **Neural** (LLM candidate generation)

None alone is sufficient. Together, they cover each other's weaknesses.

### 3. Formal Guarantees
Every implementation that compiles is **proven correct** by Z3, not just "tested." If a postcondition fails, Z3 gives you a **counterexample** — the exact inputs that break it.

```
$ morphic check sort.morph
ERROR: Postcondition violated
  Constraint: is_sorted(output)
  Counterexample:
    input = [3, 1, 2]
    output = [3, 1, 2]    ← implementation didn't sort
```

### 4. Template Library
9 algorithmic patterns seed the synthesis engine: divide-and-conquer, binary search, two-pointer, sliding window, fold, iterator chain, linear scan, memoization, simple compute. The right template is auto-selected based on spec shape detection.

### 5. Self-Optimizing
The IR that Morphic outputs can be fed back as input. The compiler can **optimize its own generated code.** Morphic functions can call other Morphic functions — each is a spec, not an implementation, so the compiler can cross-optimize.

---

## 📁 Project Structure

```
morphic/
├── Cargo.toml                    # Rust project config
├── README.md                     # This file
├── GUIDE.md                      # Beginner tutorial
├── LANGUAGE_REFERENCE.md         # Complete language spec
├── ARCHITECTURE.md               # Compiler pipeline
├── examples/
│   ├── sort.morph                # Self-synthesizing sort
│   └── binary_search.morph       # Self-synthesizing binary search
├── src/
│   ├── main.rs                   # CLI entry point
│   ├── spec/
│   │   ├── mod.rs                # Specification module
│   │   ├── ast.rs                # AST types
│   │   ├── parser.rs             # .morph parser (recursive descent)
│   │   └── typeck.rs             # Type checker + constraint validator
│   ├── llm/
│   │   ├── mod.rs                # LLM module
│   │   ├── client.rs             # Ollama + OpenAI + Anthropic clients
│   │   ├── prompt.rs             # Prompt engineering for synthesis
│   │   └── parser.rs             # Code extraction from LLM outputs
│   ├── synthesis/
│   │   ├── mod.rs                # Synthesis module
│   │   ├── engine.rs             # MCTS + Genetic + LLM engine
│   │   ├── templates.rs          # 9 algorithmic patterns
│   │   ├── selector.rs           # Pareto-optimal candidate selection
│   │   └── interactive.rs        # Interactive synthesis UI
│   ├── verify/
│   │   ├── mod.rs                # Verification module
│   │   ├── verifier.rs           # Z3-based formal verification
│   │   └── smt.rs                # Z3 FFI layer (v0.20)
│   └── codegen/
│       └── mod.rs                # Multi-target code generation
└── tests/
    └── (integration tests)
```

---

## 📚 Documentation

| Document | Description |
|---|---|
| **[GUIDE.md](GUIDE.md)** | Beginner tutorial — from zero to first synthesized function |
| **[LANGUAGE_REFERENCE.md](LANGUAGE_REFERENCE.md)** | Complete .morph language specification |
| **[ARCHITECTURE.md](ARCHITECTURE.md)** | Full compiler pipeline + design decisions |
| **[ROAD_TO_SELF_HOSTING.md](ROAD_TO_SELF_HOSTING.md)** | v0.5 pre-flight plan — risks, safeties, milestones |
| **[CONTRIBUTING.md](CONTRIBUTING.md)** | How to contribute + good first issues |

---

## 🎯 Roadmap

| Version | Feature | Status |
|---|---|---|
| **0.1** | Parser + IR + MCTS stub + codegen (5 targets) | ✅ Done |
| **0.2** | Z3 FFI: real formal verification (bundled binary) | ✅ Done |
| **0.3** | LLM module: Ollama client + prompt + parser (local + remote) | ✅ Done |
| **0.4** | Template library: 9 algorithmic patterns + spec shape detection | ✅ Done |
| **0.5** | Self-hosting: Morphic compiler written in Morphic | 🔨 Next |
| **0.6** | WASM playground: Morphic in the browser | 📅 Planned |
| **1.0** | Production-ready — verified standard library | 🎯 Target |

---

## 📜 License

This project is dual-licensed under either:

- **[MIT License](LICENSE-MIT)** — simple, permissive
- **[Apache License 2.0](LICENSE-APACHE)** — includes patent protection

at your option. See [NOTICE](NOTICE) for details.

This is the same dual-license used by the Rust compiler, Tokio, Serde, and most of the Rust ecosystem.

---

## 🤝 Contributing

Morphic is alpha — ideas welcome, PRs celebrated.

1. Read [ARCHITECTURE.md](ARCHITECTURE.md) and [GUIDE.md](GUIDE.md)
2. Pick an issue tagged `good-first-issue`
3. Install dependencies: Rust 1.75+, Z3 (bundled via `gh-release`)
4. Submit PR with tests

---

## 📚 Background Reading

- *Program Synthesis* — Gulwani, Polozov, Singh (2017)
- *Monte Carlo Tree Search* — Coulom (2006), Kocsis & Szepesvári (2006)
- *Z3: An Efficient SMT Solver* — de Moura & Bjørner (2008)
- *Genetic Programming* — Koza (1992)
- *Large Language Models for Code* — Chen et al. (2021)

---

<p align="center">
  <b>Morphic</b> — <i>Don't write code. Write what you want.</i>
</p>
