# ⚡ Morphic — Self-Synthesizing Programming Language

<p align="center">
  <a href="https://github.com/thvtzy/morphic/blob/master/LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%20%7C%20Apache--2.0-blue.svg?style=for-the-badge" alt="License: MIT OR Apache-2.0"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/language-Rust-orange.svg?style=for-the-badge" alt="Language: Rust"></a>
  <a href="https://github.com/thvtzy/morphic/actions"><img src="https://img.shields.io/github/actions/workflow/status/thvtzy/morphic/ci.yml?branch=master&style=for-the-badge&label=CI" alt="CI Status"></a>
  <br>
  <a href="https://github.com/thvtzy/morphic/blob/master/CHANGELOG.md"><img src="https://img.shields.io/badge/version-v1.8.0-brightgreen?style=for-the-badge" alt="Version: v1.8.0"></a>
  <img src="https://img.shields.io/badge/status-alpha-orange?style=for-the-badge" alt="Status: Alpha">
  <img src="https://img.shields.io/badge/z3-verified-blue?style=for-the-badge" alt="Z3 Verified">
  <img src="https://img.shields.io/badge/LLM-Ollama-purple?style=for-the-badge" alt="LLM: Ollama">
  <br>
  <a href="https://github.com/thvtzy/morphic/stargazers"><img src="https://img.shields.io/github/stars/thvtzy/morphic?style=for-the-badge&color=yellow" alt="Stars"></a>
  <a href="https://github.com/thvtzy/morphic/tree/master/src"><img src="https://img.shields.io/badge/tests-666%20passed-green?style=for-the-badge" alt="Tests: 666"></a>
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
| **[EMBEDDING.md](EMBEDDING.md)** | Embedding guide — the 1.1 packages (`@thvtzy/morphic` on npm, `morphic` on crates.io): five-minute quickstart, import shapes, the type table with its named losses, what the tiers mean at the boundary |
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

**728 tests passing.** The **1.0 standard library** ships: 29 entries + 2 builtins delivered as a compiler prelude (no import — call by plain name in the CLI, the API and the playground), every entry held to the frozen 1.0 gate (Phase 117: PASS on the amended scope) — 18 Tier-1 entries Z3-Verified ∧ golden-exact through the production pipeline, 11 Tier-2 List entries executed-exact on their full batteries with the tier printed in their documentation, the builtins exact on their per-layer battery; cross-target honesty enforced (no silent free-call fallback for library entries); the playground bundle at 680,280 B inside its 771,072 B budget with the library as a separately-loaded asset. One entry was amended out in the open by the owner's decision (Amendment 1): `int_gcd`, returning with the post-1.0 quantified-mod campaign. Recorded residue, itemized in the ROAD: the §3.9 List-element encoder and G4 function values are the headline post-1.0 campaigns; NF-116-1/NF-116-2 stand as named findings. Released as **v1.0.0** (2026-11-23, tag `v1.0.0` on master; v0.7.0 before it) — the release that ships the verified standard library at the frozen gate. Full record: [ROAD_TO_SELF_HOSTING.md](ROAD_TO_SELF_HOSTING.md).

**1.1 Embeddable ships** on top of it: the same 29 entries as two ready-to-import packages whose verdicts were produced natively and travel with the artifacts — **`@thvtzy/morphic`** (pre-generated ESM + TypeScript declarations: Int as `bigint` end-to-end, Option as `T | null`, Result as a discriminated union; 59,092 B unpacked inside the frozen 79,872 B budget) and the slim crates.io facade **`morphic`** (the 29 functions as real Rust under `morphic::stdlib`, zero dependencies, docs.rs-safe) — both carrying the machine-readable tier manifest (`morphic.manifest.json`), both held to the frozen 1.1 gate (Phase 126: PASS in full; the gate text was never amended), and versioned in lockstep with the project. The embedding guide is [EMBEDDING.md](EMBEDDING.md) (five-minute quickstart, the type table with its losses named, the deferrals in the open: no runtime-compiler package, the Component Model as the post-1.1 step). Riding in the same stack: the JavaScript emitter completion, the Char-pattern lockstep cascade (finding F4's caveat retired unconditionally) and the global NF-116-2 fix (generated Rust renders `require:` preconditions genuinely). Released as **v1.1.0** (2026-11-23, tag `v1.1.0` on master); the two registry publishes are the owner's release acts and follow the tag (the checklist stands in the ROAD Phase 126 note).

**1.2 Higher-Order ships** on top of it: the language itself gains function values (the G4 build) — a Morphic spec can take a function as a parameter, apply it, and be judged for it by the interpreter that ships: closures with by-value snapshot capture, function arguments supplied as lambda literals or as named references to shipped entries, in-process higher-order judging (`sort` at 6/6 and `binary_search` at 7/7, crowned by the interpreter rather than rescued by a compiler), and a Z3 uninterpreted-function arm that verifies direct-application specs on congruence alone (nothing beyond congruence is claimed). The library grows by Amendment 2's six entries — `list_map`, `list_filter`, `list_fold` (the left fold), `list_all`, `list_any`, and `range` — all **Tier-2 executed-exact** on their frozen 105-point batteries with function arguments supplied both ways, bringing the library to **35 entries + 2 builtins (18 Tier-1 / 17 Tier-2)** and both packages to 35 exports: **`@thvtzy/morphic`** at 70,152 B unpacked inside the re-derived 76,800 B budget (the higher-order exports answer any JavaScript function at runtime — a host lambda or the package's own exports; the `.d.ts` keeps the frozen `never` boundary for function-typed parameters, the manifest carries the true signatures) and the facade **`morphic`** at 56,029 B with the six fully typed as `fn` pointers. Both held to the frozen 1.2 gate (Phase 136: PASS in full; the gate text was never amended), and versioned in lockstep with the project. The playground bundle stands at 739,314 B inside the standing 771,072 B budget. Released as **v1.2.0** (2026-11-24, tag `v1.2.0` on master); the two registry publishes for 1.2.0 are the owner's release acts and follow the tag (the checklist stands in the ROAD Phase 136 note).

**1.3 List-encoder ships** on top of it: the §3.9 campaign teaches the verifier to judge list elements, not just lengths — a symbolic `List<Int>` binds as a paired length + element array, indexing encodes only through a discharged in-range guard (never totalized), list equality is pointwise over the range, quantifiers judge over the domain the body's own reads derive, folds encode as the frozen prefix definition (structural recursion — the axiom form was measured and withdrawn for deciding proofs but not refutations), and function applications compose over guarded element terms. **Fifteen of the seventeen Tier-2 entries are crowned Z3-Verified ∧ battery-exact with their shipped bodies** and re-tier to Tier-1 under the label-only re-issue rule (function code bytes in both packages hash-identical to the 1.2.0 assemblies): the element-model eight (`list_reverse`, `list_contains`, `list_min`, `list_max`, `list_take`, `list_drop`, `list_is_sorted`, `range`), the fold three (`list_sum`, `list_product`, `list_fold`), and the higher-order four (`list_map`, `list_filter` — of exactly its two frozen rows, the tier certifying the frozen contract, not the name's gloss — `list_all`, `list_any`), bringing the library to **35 entries + 2 builtins (33 Tier-1 / 2 Tier-2)**. The two that stay Tier-2 name their reasons: `list_sort` (the standing residue — the §3.10 `is_permutation` Predicate row and the two-binder forall row) and `list_binary_search` (the named stretch, judged and not landed: its `require:` names a quantifier-shaped contract with no expression-level denotation the precondition machinery can assume). Both packages re-issue in lockstep: **`@thvtzy/morphic`** at 71,265 B unpacked inside the frozen 76,800 B budget and the facade **`morphic`** at 57,005 B. Both held to the frozen 1.3 gate (Phase 144: PASS in full; the gate text was never amended). The playground bundle stands at 751,360 B inside the standing 771,072 B budget. Released as **v1.3.0** (2026-11-24, tag `v1.3.0` on master); the two registry publishes for 1.3.0 are the owner's release acts and follow the tag (the checklist stands in the ROAD Phase 144 note).

**1.4 — the agent channel + the honesty engine — ships** on top of it: the Disebar rung's first milestone puts Morphic where AI agents already work, and turns its packages' claims into evidence strangers can re-check. **`morphic mcp`** is a hand-rolled stdio JSON-RPC 2.0 server (zero new dependencies — the lock graph is byte-identical to v1.3.0's) over the compiler's DTO API, serving exactly four tools — `check`, `synthesize`, `verify`, `library` — whose arguments are source text only (no path, no key, no model argument exists): synthesis runs only the seeded, bounded, LLM-free playground profile (same (source, seed) → same crowned body), and every verdict served is the native verifier's own, produced in-process at call time — a wrong body stands VerificationFailed through the tool, and the subcommand ships only in the Z3-enabled build, refusing by name elsewhere. **Proof-carrying code** carries the other half: bodies travel in a stable, versioned, parseable IR text serialization (**`morphic-ir/1`**, round-trip-pinned over all 35 shipped bodies and the full node inventory), one **re-verification bundle** per entry ships at `packages/bundles/` (35 files, 267,156 B — a sibling artifact in neither package's file set) holding its spec, its body in IR text, the claimed verdict, the executed battery and the crown route that judged it, and any holder of the binary re-judges it from (spec, body) alone with **`morphic verify-bundle`** (tamper refused by name, shape by shape) or re-verifies every shipped crown at once with **`morphic verify --library`** (35/35). Z3 proof objects are excluded as a mechanism, not deferred: the pinned solver exposes no portable proof format a stranger could validate without re-solving — which is re-verification with extra steps. The library stands untouched at **35 entries + 2 builtins (33 Tier-1 / 2 Tier-2)**. Both packages re-issue in lockstep: **`@thvtzy/morphic`** at 71,873 B unpacked inside the frozen 76,800 B budget and the facade **`morphic`** at 57,613 B. Both held to the frozen 1.4 gate (Phase 151: PASS in full; the gate text was never amended). The playground bundle stands at 751,206 B inside the standing 771,072 B budget. Released as **v1.4.0** (2026-11-24, tag `v1.4.0` on master); the registry publishes for 1.4.0 are the owner's release acts and follow the tag, as do the still-pending 1.2.0 and 1.3.0 publishes (the checklist stands in the ROAD Phase 151 note).

**1.5 — at the edge and in the editor — ships** on top of it: the Disebar rung's second milestone takes proven functions to where they are served and written. A **Cloudflare Workers template** (`templates/cloudflare-workers/`) serves the shipped package at the edge — a module worker routing `POST /fn/<name>`, the JSON/BigInt boundary as handler code, named error shapes never thrown — and the **user-package verb** (`morphic build --package-from <file.morph>`) packages a user's own module the same way, admitting only rows its own seeded run measured: Verified or executed-exact in, anything else refused by name with nothing written, and no flag by which a tier, a verdict, or a battery figure could be supplied. **`morphic lsp`** is now a real **diagnostics-only language server** (Content-Length framed JSON-RPC over stdio, full-document sync over `api::check`): parse errors publish at their true positions; typecheck errors publish as one file-level diagnostic at 0:0 whose message is the verbatim typecheck message prefixed `unpositioned: ` — the editor states plainly what the compiler does not know. The **VS Code extension** (`packages/vscode/`) is the thin client for it: exactly the frozen surface (the language, a decoration-only highlighting grammar, one setting — `morphic.serverPath`), its one dependency (`vscode-languageclient`) quarantined in its own package, distributed as a sideload `.vsix` (**`morphic-1.5.0.vsix`**, 486,879 B, committed in the repo; Marketplace publishing is the owner's later act). The library stands untouched at **35 entries + 2 builtins (33 Tier-1 / 2 Tier-2)**. Both packages re-issue in lockstep: **`@thvtzy/morphic`** at 71,873 B unpacked inside the frozen 76,800 B budget and the facade **`morphic`** at 57,613 B; the re-verification bundles regenerate byte-identical (267,156 B); the Cargo dependency set is unchanged (lock graph identical to v1.4.0's). Both held to the frozen 1.5 gate (Phase 157: PASS in full; the gate text was never amended). The playground bundle stands at 752,223 B inside the standing 771,072 B budget. Released as **v1.5.0** (2026-11-25, tag `v1.5.0` on master); the registry publishes for 1.5.0 are the owner's release acts and follow the tag, as do the still-pending 1.2.0, 1.3.0, and 1.4.0 publishes — publishing just the latest is enough (the checklist stands in the ROAD Phase 157 note).


**1.6 — the Component Model boundary — ships** on top of it: the Disebar rung's third milestone carries the same proven functions to a boundary no single language owns. The **library component** (`packages/component/`) wraps the facade's 30 non-HO entries — one glue line per export behind wit-bindgen, built by **plain cargo** (`--target wasm32-wasip2`; wasm-tools 1.263.0 + wasmtime 51.0.0 as prebuilt pins, wit-bindgen `=0.63.0`, cargo-component measured and not adopted) — into a true WASM component exporting `morphic:library/functions`, its artifact of record (**`morphic-library.wasm`**, 96,100 B inside its frozen 106,496 B budget, reproduced byte-for-byte by a fresh build) answering its frozen batteries exactly under wasmtime. The five higher-order entries cross in a **second component** (`packages/component-callbacks/`, **`morphic-callbacks.wasm`**, 81,840 B inside 90,112 B) whose function arguments are **host imports** (`map-apply`, `pred`, `combine`) — a component's imports are satisfied at instantiation for the whole component, so folding them in would lock out every consumer that only wants `int_clamp`, and WIT has no function-typed values: no exported signature anywhere carries one, and the callback's correctness is the host's, stated plainly in its README. The WIT carries types, not tiers — the tier rows stay in the manifest, their evidence in the PCC bundles — and the WASI import surface std attaches is accepted and named as a wart (the strip is a follow-up). Distribution is the repo + the GitHub release assets; no registry takes a bare component today. The library stands untouched at **35 entries + 2 builtins (33 Tier-1 / 2 Tier-2)**. Both packages re-issue in lockstep: **`@thvtzy/morphic`** at 71,873 B unpacked inside the frozen 76,800 B budget and the facade **`morphic`** at 57,613 B; the re-verification bundles regenerate byte-identical (267,156 B); the extension re-issues as **`morphic-1.6.0.vsix`** (486,883 B). Both held to the frozen 1.6 gate (Phase 161: PASS in full; the gate text was never amended). The playground bundle stands at 752,223 B inside the standing 771,072 B budget. Released as **v1.6.0** (2026-11-25, tag `v1.6.0` on master); the registry publishes for 1.6.0 are the owner's release acts and follow the tag, as do the still-pending 1.2.0, 1.3.0, 1.4.0, and 1.5.0 publishes — publishing just the latest is enough (the checklist stands in the ROAD Phase 161 note). Next on the ladder: **1.7 — registry (a)**, prelude bundles with install-time re-verification.

**1.7 — the registry — ships** on top of it: the Disebar rung's fourth and last named milestone makes proven functions *publishable* — anyone can put a proven function into the catalog, and no one has to take its word for it. The **catalog** lives in this repo (`registry/`: a static JSON index plus one proof-carrying bundle per entry; publishing is a pull request), seeded with **37 entries** — the 35 library bundles byte-identical to `packages/bundles/`, plus a user module's pair produced by the new production arm, **`morphic build --bundles-from`**. The client is one command family — **`morphic registry` add / list / remove / verify**: an install resolves the entry against the index, fetches the bundle, checks its **sha256 against the index row**, and **re-runs the bundle's proof** before writing anything; what lands on the machine is the bundle file plus a **receipt recording what the re-run measured** (the index's tier fields are browsing labels, never the mechanism of trust). Installed entries then join the prelude automatically — precedence **user > stdlib > installed** — so an installed name is callable from any fresh spec, judged and generated in every shell exactly like a library entry, while an installed bundle can never shadow a stdlib name, however it got onto the disk. The milestone's one new dependency is `sha2`, kept out of the browser graph by its target-table placement. The library stands untouched at **35 entries + 2 builtins (33 Tier-1 / 2 Tier-2)**. Both packages re-issue in lockstep: **`@thvtzy/morphic`** at 71,873 B unpacked inside the frozen 76,800 B budget and the facade **`morphic`** at 57,613 B; the re-verification bundles regenerate byte-identical (267,156 B); both components re-issue at `morphic:library@1.7.0` at unmoved sizes (96,100 B / 81,840 B); the extension re-issues as **`morphic-1.7.0.vsix`** (486,883 B). Held to the frozen 1.7 gate (Phase 166: PASS in full; the gate text was never amended). The playground bundle stands at 752,204 B inside the standing 771,072 B budget. Released as **v1.7.0** (2026-11-26, tag `v1.7.0` on master); the registry publishes for 1.7.0 are the owner's release acts and follow the tag. With 1.7 the **Disebar rung is complete** — usable (1.0/1.1) and spread (MCP, edge, LSP/VS Code, components, registry).

**1.8 — the smart-contract target (`--target solidity`) — ships** as the first milestone on the **Ikonik** rung: Morphic specifications now compile directly to zero-warning Solidity `^0.8.24` pure contracts backed by Z3 verification and proof-carrying bundles. **`morphic build --target solidity`** (alias `sol`, default `.sol` extension) and **`api::render(cand, "solidity")`** emit self-contained `contract` containers (`public pure` entrypoints + deduplicated `internal pure` transitive prelude helpers + on-demand memory-array helpers) over the direct-ABI type subset (`Int`→`int256`, `Bool`, `I8..I64`, `U8..U64`, `String`, `List<scalar>`), with always-on runtime `require(...)` preconditions (hard-refusing quantifiers and unrenderable forms — never emitting `require(true)`), explicit NatSpec proof-boundary disclosure (Z3 unbounded $\mathbb{Z}$ vs EVM `int256` checked `Panic(0x11)`/`0x12` and negative-division semantics), and hard `CodegenError` refusals on every unsupported type or IR node. **`morphic build --contract-from <file.morph>`** assembles checked multi-spec contracts alongside companion `bundleVersion: 1` PCC bundles whose SHA-256 digests are embedded into each function's NatSpec so any auditor can re-run the Z3 proof from scratch via `morphic verify-bundle`; a committed 6-spec `DeFiMath` exhibit ships at `examples/contracts/defi_math.morph`. A pinned `solc@0.8.28` + `@ethereumjs/evm@3.1.1` + `ethers@6.13.4` Node EVM harness (in scratch `target/solidity-harness/`, zero new Cargo dependencies, wired into CI) verifies **0 errors / 0 warnings**, 100% real EVM battery execution across `DeFiMath` and all 18 direct-ABI stdlib entries, and runtime `require(...)` + `Panic(0x11)` reverts. Held to the frozen 1.8 gate (Phase 171: PASS in full; the gate text was never amended). All surfaces re-issue in lockstep at **1.8.0** (`@thvtzy/morphic` 71,873 B, `morphic` facade 57,613 B, bundles byte-identical 267,156 B, components `morphic:library@1.8.0` at 80,558 B / 66,011 B, `morphic-1.8.0.vsix` 486,881 B). Released as **v1.8.0** (2026-11-26, tag `v1.8.0` on master).
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
| **1.0** | Production-ready — verified standard library | ✅ Done — scope frozen (D1a–D8a) · §3.1 preconditions ✅ · prelude packaging ✅ · 29 entries + 2 builtins ✅ · cross-target honesty ✅ · gate PASS (Phase 117, as amended by Amendment 1) — released as v1.0.0 |
| **1.1** | Embeddable Morphic: verified functions as ready-to-import packages (npm/WASM + Rust crate), typed host bindings | ✅ Done — scope frozen (Embed D1a–D8a) · JS emitter completion ✅ · typed bindings + tier manifest ✅ · npm package ✅ · Rust facade crate ✅ · Char cascade + NF-116-2 global fix ✅ · gate PASS (Phase 126) — released as v1.1.0 |
| **1.2** | Higher-order Morphic: function values in the language (G4) + the higher-order standard library | ✅ Done — scope frozen (G4 D1a–D8a, ratified by Muse under the owner's delegation) · checker admission ✅ · closure values ✅ · in-process HO judging ✅ · named-reference supply ✅ · UF arm ✅ · Amendment 2: six Tier-2 entries ✅ · gate PASS (Phase 136) — released as v1.2.0 |
| **1.3** | The §3.9 List encoder: paired List binding, guarded indexing, pointwise equality, read-derived quantifier domains, the fold-form arm, UF composition — fifteen entries crowned Tier-1 | ✅ Done — audit (Phase 137) · scope frozen (D1a–D7a, owner-ratified) · W1 binding+mirror ✅ · W2 index/equality + probe pair ✅ · W3 quantifiers: the eight crowned ✅ · W4 fold-form: the three crowned ✅ · W5 UF composition: the four crowned ✅ · stretch judged, not landed (binary_search stays Tier-2, reason named) · gate PASS (Phase 144) — released as v1.3.0 |
| **1.4** | Disebar I — the agent channel: the MCP server (four tools over stdio JSON-RPC) + proof-carrying code (re-verification bundles over the `morphic-ir/1` serialization) | ✅ Done — audit (Phase 148) · scope frozen (D1a–D8a, owner-ratified) · MCP server ✅ · IR text serialization + bundles + `verify-bundle` + `verify --library` ✅ · gate PASS (Phase 151) — released as v1.4.0 |
| **1.5** | Disebar II — at the edge and in the editor: the Cloudflare Workers template + the user-package verb (`build --package-from`, measured-rows-only admission) + the LSP diagnostics-only server + the VS Code extension (.vsix sideload) | ✅ Done — audit (Phase 152) · scope frozen (D1a–D7a, owner-ratified) · edge template ✅ · user-package verb ✅ · LSP server ✅ · VS Code extension ✅ · gate PASS (Phase 157) — released as v1.5.0 |
| **1.6** | Disebar III — the WASM Component Model: the library component (the facade's 30 non-HO entries behind `morphic:library/functions`) + the callback component (the five higher-order entries over host-imported callbacks) | ✅ Done — audit (Phase 158) · scope frozen (D1a–D7a, ratified by Muse under the owner's delegation) · library component ✅ · callback component ✅ · gate PASS (Phase 161) — released as v1.6.0 |
| **1.7** | Disebar IV — registry (a): prelude bundles with install-time re-verification (on 1.4's PCC machinery) | ✅ Done — audit (Phase 162) · scope frozen (D1a–D8a, ratified by Muse under the owner's delegation) · freeze + production arm + seed catalog ✅ · registry verbs + install flow ✅ · discovery at the prelude funnel ✅ · gate PASS (Phase 166) — released as v1.7.0 |
| **1.8** | Ikonik I — smart-contract target (`--target solidity`): zero-warning Solidity `^0.8.24` pure contracts, runtime `require(...)` guards, `--contract-from` + PCC bundle coupling, real EVM execution harness | ✅ Done — audit (Phase 167) · scope frozen (D1a–D9a, owner-ratified) · core emitter ✅ · `--contract-from` + PCC + `DeFiMath` exhibit ✅ · pinned `solc-js` + Node EVM harness ✅ · gate PASS + CI wiring (Phase 171) — released as v1.8.0 |

**North Star:** Morphic's long-term ambition is to become as well-known as Python and relevant in everyday use, with proven-correct code as its wedge in the AI-agent era — see the Phase 107 note in [ROAD_TO_SELF_HOSTING.md](ROAD_TO_SELF_HOSTING.md).

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
