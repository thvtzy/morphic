# Changelog

All notable changes to Morphic will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- **M2 self-host specs** — `examples/self_host/validator.morph` (type checker lite: type-tag/node-kind domains, node arity table, lite tag compatibility, recursion depth guard) and `examples/self_host/printer.morph` (IR → text: type/kind/operator name lookups, parenthesis precedence rule, line-width rule), written in the dialect the current parser + typechecker verifiably accept
- **M2 proof tests** — `tests/m2_proof.rs`: pure-Rust mirror implementations with golden tests for every spec test case (including the multi-input specs the spec test syntax cannot bind), plus end-to-end `morphic check` pipeline tests with negative controls; the e2e self-host suite now also covers both new spec files
- **M2 synthesis probe** — `src/m2_synthesis_probe.rs` (test-only): runs the real pipeline (synthesize → Z3 verify → select → codegen) per spec for all 14 M2 specs at opt level 1, the only LLM-free configuration (opt levels 2–4 fail with `LlmError` in the default build — locked in by a unit test). First synthesis run results, recorded in `ROAD_TO_SELF_HOSTING.md`: synthesis completes for 14/14 specs, the pipeline's Z3 stage reports Verified for 4/14 (candidate-independent verdicts — see roadmap root-cause notes), and 0/14 generated implementations compile yet (codegen identifier/match-expression defects, unfilled holes)

### Fixed
- **Hole replacement walker** — `replace_at_path` (under `replace_hole`, used by MCTS expansion and rollout) implemented only the `Block` path step; holes under any other node kind (match arms, If branches, Let bodies, …) were silently left in place, so MCTS "completions" were still-holed clones of their parent and were never harvested. All `PathElem` variants are now walked.
- **MCTS seed-root harvest** — the MCTS built a single tree rooted at one random population member, so completions of the test-induced seed (the only candidate shaped by measured behavior) almost never entered the candidate pool. A second tree is now rooted at each holed test-induced seed. Measured effect (synthesis probe, 3 runs × 14 specs): winners measuring test-pass-ratio 1.000 went from 0/42 to 36/42 (every tested spec, every run; the rest are the two test-less specs at their neutral 0.5), winners at 0.000 from 29/42 to 0/42, and generated winners compiling under rustc from 0/14 to 1/14 per run — the remaining failures are type-blind wildcard fillings (E0308), the next fix in line. Full notes in `ROAD_TO_SELF_HOSTING.md`.
- **Type-directed hole fillings** — `HoleRef` now carries the hole's `expected_type` (recorded by seed construction but previously unreachable: the ref held only a path), and `generate_hole_fillings` / `random_filling` draw from a per-type literal menu when it is known (Bool → true/false, Int → 0/1, String → `""`, List → empty list, …), with the spec tests' expected outputs added to the expansion menu. Types with no honest literal menu (named/generic, Option — the IR has no Option literal node —, Map/Set/Array) and all untyped holes keep the legacy menu exactly. Measured effect (probe, 3 runs × 14 specs): winner test-pass-ratio distribution unchanged (36/42 at 1.000, 0/42 at 0.000), generated winners compiling under rustc from 1/14 to **12/14 per run** — every tested spec now compiles; error classes across the 42 files fell from 37× E0308 + 2× E0069 to 9× E0308 (all in the two test-less specs, whose winners are whole-body type-wrong random trees — no seed, no truth signal) and 0× E0069/E0425. Full notes in `ROAD_TO_SELF_HOSTING.md`.
- **Candidate return-type gate** — selection now checks whole candidate bodies against the spec's declared return type. A new conservative IR type inference (`selector::return_type_verdict`) claims a body's type only when certain (literals, agreeing collections/branches, Let/Block tails, parameter-typed Vars, `len`/List builtins, Typed nodes; a provably mistyped `return` anywhere also counts), and `select_best` excludes provably mistyped candidates whenever a matching or unprovable one exists — falling back to the old ranking, documented, only when every candidate is provably mistyped. Composite weights and scoring are untouched, and the tpr-1.0 winners on tested specs are unaffected (regression-tested). Measured effect (probe, 3 runs × 14 specs): winner tpr distribution unchanged (36/42 at 1.000, 0/42 at 0.000); compiles 12–13/14 per run — the gate works, but in ~half of draws a test-less spec's hole-free pool is uniformly mistyped (nothing sound exists to crown) and the commonest surviving shape is a loop-rooted body the inference is deliberately blind to, so the remaining gap is generation-side pool composition, not selection. Full notes in `ROAD_TO_SELF_HOSTING.md`.
- **Codegen statement/tail emission** — Collections in statement position are now terminated (`vec![];`): the unterminated emission was a syntax error whenever a statement followed (still present in 2 of 3 probe runs after the gate). Genuine value positions route through the existing tail renderer instead: the function body (its value is the return value — bare-expression bodies like `(actual + expected);` no longer return `()`), and `If` branch / `Let` body positions, mirroring match arms. Statement-shaped bodies emit byte-identical code, so previously compiling winners are unchanged. Measured effect (3 fresh probe runs): the syntax/`;` error class is gone from all 42 winner files; remaining errors 6× E0308 (was 9×), 0× E0069/E0425. Full notes in `ROAD_TO_SELF_HOSTING.md`.
- **Type-directed random generation** — fresh candidate bodies (the random seeds, the pool's foundation: MCTS completions inherit their root shape) are now generated in the context of the spec's declared return type, reusing the selection verdict's certainty rules: type-plausible root shapes, If/Match tails in the same type context, loops only as statements inside Blocks ending in a typed tail, Returns only around return-typed expressions, and a shape menu restricted to what the Rust emitter can actually render (no `Call`/`Index`/`Concat`/`Len` shapes, no constant-zero divisors). The legacy untyped generator is byte-for-byte unchanged for callers without a type context. Measured effect: seed bodies provably mistyped for their spec fell from 314/1000 (31.4%, verdict-checked over the M2 signatures) to **0/1000 — with 1000/1000 provably matching** — and the sound-pool premise for the two test-less specs became structural (60/60 synthesis attempts contain a hole-free non-mistyped candidate; was ≈0.53–0.7 per attempt). Probe (3 runs × 14 specs): winner tpr distribution unchanged (36/42 at 1.000, 0/42 at 0.000); generated winners compiling under rustc **14/14 in every run** (was 12–13/14), with 0 error instances of any class across the 42 files (was 6× E0308). The two test-less specs' winners are type-sound but still semantically arbitrary — they have no executable tests (the spec syntax cannot bind tuple test inputs), so no truth signal exists for them. Full notes in `ROAD_TO_SELF_HOSTING.md`.

- **Codegen emitter completeness** — the Rust expression emitter (`ir_to_rust_expr`) fell through to `/* UNSUPPORTED */` for 19 of the 28 `IRNode` variants (Hole, Block, Let, loops, Match, Return, Closure, Map/Filter/Fold/Sort, Alloc, Assign, Index, Loop, Break, StructDef, TryPropagate) and degraded `BinOp` Xor / `UnaryOp` Head/Tail/Init/Last to unparseable `UNKNOWN_OP`/`UNKNOWN_UNOP` operands; any of these in an If condition, operand, argument or Let value made the generated file unparseable. The match is now exhaustive (no wildcard — rustc guards completeness): every variant renders to a parseable expression (Hole → `panic!("Hole not filled")`, Index → `(c)[i]`, TryPropagate → `(e)?`, Xor → `^`, Head/Last index, Tail/Init slice, Block/Let/loops/Match/StructDef as block or native expressions, Return/Break parenthesized, Alloc → a loud `panic!` naming the shape), block-like expressions are parenthesized in condition/scrutinee positions, and Set/Map collections are fully qualified (`std::collections::…`, previously E0433 with no import generated). The `Call`/`Map`/`Filter`/`Fold`/`Sort` statement arms are now terminated with `;` (the Collection arm's Phase 11 hazard), with value positions routed through the tail renderer so bare bodies and match arms keep yielding their values. Measured effect (probe, 3 runs × 14 specs): winner tpr distribution unchanged (36/42 at 1.000, 0/42 at 0.000), compiles stay **14/14 in every run**, and the marker sweep is clean — 0 `UNSUPPORTED` / `UNKNOWN_OP` occurrences across all 42 winner files, 0 error instances of any class. Full notes in `ROAD_TO_SELF_HOSTING.md`.

Tests: 157 passed / 0 failed, 3 ignored (123 unit + 8 e2e + 21 M2 proof + 5 tokenizer proof; ignored: 2 e2e CLI tests requiring a full binary build, 1 synthesis probe)

---

## [0.4.0] — 2026-06-09

### Added
- **Template library** — 9 algorithmic patterns (v0.4)
- **LLM module** — Ollama + OpenAI + Anthropic clients (v0.3)
- **Prompt engineering** — spec → structured synthesis prompt
- **Response parser** — multi-format code extraction
- **IRNode::Index variant** — collection indexing in IR
- **Spec shape detection** — auto-select matching templates

### Changed
- **reqwest** — now required dependency (was optional)
- **Cargo.toml** — v0.4.0, simplified feature flags

Tests: 19/19 passing (5 parser + 3 Z3 + 3 prompt + 3 parser + 3 templates + 1 client + 1 selector)

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
