# ⚡ Morphic — Self-Synthesizing Programming Language

<p align="center">
  <a href="https://github.com/thvtzy/morphic/blob/master/LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20%7C%20Apache--2.0-blue.svg?style=for-the-badge" alt="License: MIT OR Apache-2.0"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/language-Rust-orange.svg?style=for-the-badge" alt="Language: Rust"></a>
  <a href="https://github.com/thvtzy/morphic/actions"><img src="https://img.shields.io/github/actions/workflow/status/thvtzy/morphic/ci.yml?branch=master&style=for-the-badge&label=CI" alt="CI Status"></a>
  <br>
  <a href="https://github.com/thvtzy/morphic/blob/master/CHANGELOG.md"><img src="https://img.shields.io/badge/version-v0.7.0-brightgreen?style=for-the-badge" alt="Version: v0.7.0"></a>
  <img src="https://img.shields.io/badge/status-alpha-orange?style=for-the-badge" alt="Status: Alpha">
  <img src="https://img.shields.io/badge/z3-verified-blue?style=for-the-badge" alt="Z3 Verified">
  <img src="https://img.shields.io/badge/LLM-Ollama-purple?style=for-the-badge" alt="LLM: Ollama">
  <br>
  <a href="https://github.com/thvtzy/morphic/stargazers"><img src="https://img.shields.io/github/stars/thvtzy/morphic?style=for-the-badge&color=yellow" alt="Stars"></a>
  <a href="https://github.com/thvtzy/morphic/tree/master/src"><img src="https://img.shields.io/badge/tests-555%20passed-green?style=for-the-badge" alt="Tests: 555"></a>
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
Every winning implementation is **judged by Z3 against the candidate itself** — proven, refuted with a **counterexample** (the exact inputs that break it), or honestly reported inconclusive — not just "tested."

```
$ morphic check sort.morph
ERROR: Postcondition violated
  Constraint: is_sorted(output)
  Counterexample:
    input = [3, 1, 2]
    output = [3, 1, 2]    ← implementation didn't sort
```

### 4. Template Library
3 structural templates (sort primitive, adjacent scan, binary search) plus 15 constraint-seed families seed the synthesis engine, alongside test-induced, LLM and random seeds. The right seeds are selected based on spec shape detection. (An earlier version of this section claimed "9 algorithmic patterns" — v0.4 archaeology, corrected per the Phase 106 audit: the other v0.4 builders were removed in Phase 59.)

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
│   ├── binary_search.morph       # Self-synthesizing binary search
│   ├── functional_tokenizer.morph # Tokenizer specs (M1/S4; re-authored 2026-11-21)
│   └── self_host/                # validator.morph + printer.morph + codegen.morph (v0.5 self-hosting specs)
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
│   │   ├── eval.rs               # IR interpreter — executes spec tests for real scoring
│   │   ├── templates.rs          # 3 structural templates
│   │   ├── selector.rs           # Pareto-optimal candidate selection
│   │   └── interactive.rs        # Interactive synthesis UI
│   ├── verify/
│   │   ├── mod.rs                # Verification module
│   │   ├── verifier.rs           # Z3-based formal verification
│   │   └── smt.rs                # Z3 FFI layer (v0.20)
│   ├── codegen/
│   │   ├── mod.rs                # Multi-target code generation
│   │   ├── m3_gate.rs            # M3 gate harness: composed winners vs original emitter, byte-compared
│   │   └── m4_harness.rs         # M4 composed-compiler harness + corpus gate (extract once, render twice)
│   ├── m2_synthesis_probe.rs     # M2 synthesis measurement harness
│   ├── m2_cross_validation.rs    # Behavioral cross-validation vs tests/m2_proof.rs mirrors
│   ├── m3_synthesis_probe.rs     # M3 synthesis measurement harness (Codegen module)
│   ├── m3_cross_validation.rs    # Behavioral cross-validation vs tests/m3_proof.rs mirrors
│   ├── tokenizer_synthesis_probe.rs    # Tokenizer synthesis measurement harness (S4)
│   └── tokenizer_cross_validation.rs   # Cross-validation vs tests/tokenizer_proof.rs mirrors
└── tests/
    ├── (integration tests)
    ├── m2_proof.rs               # Hand-written mirrors of the validator/printer specs
    ├── m3_proof.rs               # Hand-written mirrors + assemblies of the Codegen specs
    ├── tokenizer_proof.rs        # Hand-written tokenizer + mirrors of the tokenizer specs
    └── seed_body_golden.txt      # Byte-exact fixture of every constraint-seed body
```

---

## 📚 Documentation

| Document | Description |
|---|---|
| **[▶ Playground](https://thvtzy.github.io/morphic/)** | Morphic in the browser — write a spec, check it, and synthesize an implementation judged by executed tests, entirely client-side. Capability labels: parse/typecheck + interpreter-judged synthesis run in-browser; Z3 formal verification and compiled-runner scoring are native-only and are labelled unavailable in the UI, never simulated |
| **[GUIDE.md](GUIDE.md)** | Beginner tutorial — from zero to first synthesized function |
| **[LANGUAGE_REFERENCE.md](LANGUAGE_REFERENCE.md)** | Complete .morph language specification |
| **[ARCHITECTURE.md](ARCHITECTURE.md)** | Full compiler pipeline + design decisions |
| **[ROAD_TO_SELF_HOSTING.md](ROAD_TO_SELF_HOSTING.md)** | v0.5 self-hosting roadmap (milestones M0–M5; M2, M3, M4 and M5 gates closed 2026-11-20/21) + the v0.6 WASM-playground road (W1–W5, gate closed 2026-11-21) |
| **[CONTRIBUTING.md](CONTRIBUTING.md)** | How to contribute + good first issues |

---

## 📍 Current Status — 2026-11-21

v0.5 self-hosting: the **M2, M3, M4 and M5 gates are all closed**. The M2–M4 + tokenizer stack (PRs #2–#5) is merged to master, as is the M5 stack (Phases 50–64, PR #6). v0.6: the **WASM playground is released** — Morphic runs in the browser (last bullet).

- **M2 (validator + printer)** — closed 2026-11-20: Z3-verified winners **42/42**, 0 refuted, cross-validation **64/64 golden + 539/539 pinned**.
- **M3 (codegen)** — closed 2026-11-21: all 117 Codegen specs specced and measured (114/117 constraint-seeded; verified winners 117/117; executed agreement **354/354 golden + 2997/2997 pinned** at the final re-derivation). The gate: winners composed through the proof assemblies produce **byte-identical output to the original emitter over a 2,573-case corpus** (re-derived as the specs were amended — real operator symbols and closure rendering in Phase 55, the Clone bound on generated signatures in Phase 62), and the synthesized module compiles as a Rust library. Two scoped, owner-approved emitter unfreezes along the way (rendering trio; E0382 clone treatment); the spec language gained the `++` concat operator (F1) and char literals (F4).
- **M4 (integration)** — closed 2026-11-21: the composed compiler (original pipeline + the M3 composed codegen) renders **every check-passing spec in the repo byte-identically** — **143/143** at the final corpus re-derivation.
- **M5 (bootstrap)** — closed 2026-11-21: the bootstrap binary is the original compiler with its Rust codegen stage replaced — behind the `bootstrap-codegen` Cargo feature — by the synthesized Codegen module itself (117 winners, checked in as a compiled artifact and called through a hand-written adapter; the original emitter is kept and reachable with the flag off). The synthesized stage is byte-identical to the original over both gate corpora; the flag-on binary synthesized a held-out component it was not built from (RustVisibility, Phase 63); the sort roundtrip is behaviorally correct on every measured draw in both configurations (the generation campaign G1–G3 + G6 and the Clone-bound decision closed it); and the SC3 render cost is recorded at **1.590× the original (≤ 2× threshold)** — measured in the debug profile, the release build being impossible in the measuring sandbox (recorded in the ROAD).
- **Tokenizer (M1/S4)** — `functional_tokenizer.morph` re-authored into the accepted dialect (it passes `morphic check` for the first time) and measured: the three Char classifiers are **exact** (774/774 executed agreement via the `char_equation` seed family); the structural gaps that blocked the remaining specs (no field access in the IR; method calls not yet in the spec language) were closed by the M1-features campaign (method calls, IR field access, record literals, the Z3 Char scalar encoding), the encoder extension (denotations, a statement-mode encoder, Forall, loop invariants), and the F2 `int_to_str` builtin — **M1's formal gate is CLOSED** (gate re-run #4, Phase 103: all eight specs Z3-Verified ∧ 100% golden, benchmark ≤ 2×).

- **Playground (v0.6)** — Morphic in the browser, live at **[thvtzy.github.io/morphic](https://thvtzy.github.io/morphic/)**: the same compiler codebase in its browser profile (a library build — seeded, bounded, LLM-free — compiled to WASM; no Z3, no compiled runner) shipped as a fully static, client-side site. It checks specs with positioned diagnostics and synthesizes implementations judged by executed interpreter tests, with the passed / evaluable / total denominators shown exactly and formal verification labelled unavailable rather than simulated; synthesis runs in Web Workers with cancel, drafts persist on-device only, and the 611,728 B bundle sits under a 673,792 B CI-enforced budget. W1–W5 gates all passed (pinned-corpus agreement with the native CLI at every stage). Browser matrix: Chromium verified end-to-end; the Firefox/WebKit legs were not exercised in the measuring sandbox and are recorded as known-unverified in the release notes.

**555 tests passing.** Recorded residue, itemized in the ROAD: binary_search's crowns still do not compile (i64 index typing — G5), is_sorted's correct template still cannot crown through interpreter judging (owner-reserved), and the mutation stubs and GA/LLM decoupling (G7). Released as **v0.7.0** (2026-11-22, tag `v0.7.0` on master; v0.6.0 before it) — the release that closes the formal M1 gate: the language gained exactly the features the 2026-11-21 deferral named (method calls, IR field access, record literals), plus the Z3 Char encoding, the encoder extension, and the `int_to_str` builtin, and gate re-run #4 passed in full (Phase 103). Full record: [ROAD_TO_SELF_HOSTING.md](ROAD_TO_SELF_HOSTING.md).

---

## 🎯 Roadmap

| Version | Feature | Status |
|---|---|---|
| **0.1** | Parser + IR + MCTS stub + codegen (5 targets) | ✅ Done |
| **0.2** | Z3 FFI: real formal verification (bundled binary) | ✅ Done |
| **0.3** | LLM module: Ollama client + prompt + parser (local + remote) | ✅ Done |
| **0.4** | Template library: 9 algorithmic patterns + spec shape detection | ✅ Done |
| **0.5** | Self-hosting: Morphic compiler written in Morphic | ✅ Done — M0 ✅ · M2 ✅ · M3 ✅ · M4 ✅ · M5 ✅ gates closed · tokenizer measured (M1 formal gate closed at 0.7) |
| **0.6** | WASM playground: Morphic in the browser | ✅ Done — audit ✅ · scope ratified (full client-side, staged) · W1 ✅ · W2 ✅ · W3 ✅ · W4 ✅ · W5 public release ✅ — live at [thvtzy.github.io/morphic](https://thvtzy.github.io/morphic/) (Z3 + compiled runner native-only, labelled in the UI) |
| **0.7** | M1 gate closed: language features + encoder extension + `int_to_str` | ✅ Done — M1-features ✅ · encoder extension ✅ · F2 ✅ · gate re-run #4 PASS (Phase 103) — all eight tokenizer specs Z3-Verified ∧ 100% golden |
| **1.0** | Production-ready — verified standard library | 🎯 Target |
| **1.1** | Embeddable Morphic: verified functions as ready-to-import packages (npm/WASM + Rust crate), typed host bindings | 🎯 Target — after 1.0 |

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
