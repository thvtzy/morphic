# 🧬 Road to Self-Hosting — Pre-Flight Plan

> v0.5 Target: Morphic compiler written in Morphic
> Status: PLANNING — DO NOT EXECUTE WITHOUT REVIEW

---

## ⚠️ RISK ASSESSMENT

### What Can Go Wrong

| # | Risk | Severity | Likelihood | Impact |
|---|---|---|---|---|
| R1 | **Regression**: Hasil sintesis lebih buruk dari kode Rust asli — compiler jadi lebih lambat, lebih buggy, atau menghasilkan output yang salah | 🔴 Critical | Medium | Compiler rusak, semua downstream project terpengaruh |
| R2 | **Bootstrap paradox**: Menggunakan compiler yang belum terverifikasi untuk memverifikasi hasil sintesisnya sendiri — circular trust, error propagation | 🔴 Critical | High | Tidak ada kebenaran dasar (ground truth) |
| R3 | **Perf collapse**: Compiler hasil sintesis 10x lebih lambat dari Rust asli, bikin development cycle tidak usable | 🟠 High | High | Developer tidak bisa iterate, project mati |
| R4 | **Architecture mismatch**: Komponen hasil sintesis tidak compatible satu sama lain karena spec masing-masing tidak cukup ketat | 🟠 High | Medium | Komponen tidak bisa dirangkai, integrasi gagal |
| R5 | **Constraint explosion**: Spesifikasi formal untuk komponen sekompleks parser/type checker jadi terlalu besar buat MCTS + Z3, synthesis timeout atau memory overflow | 🔴 Critical | High | Synthesis gagal total, tidak ada output |
| R6 | **IR expressiveness gap**: Intermediate Representation tidak cukup kaya untuk mengekspresikan implementasi compiler-level (parsing, tree walking, code generation) | 🟠 High | Medium | Hasil sintesis tidak optimal, butuh workaround manual |
| R7 | **Test blindness**: Generated code lulus Z3 verification tapi gagal di real-world scenario yang tidak ter-encode di constraints | 🟡 Medium | High | Bug subtle yang hanya muncul di production |
| R8 | **Dependency entanglement**: Komponen baru bergantung pada komponen lama spesifik, bikin tidak bisa ganti komponen satu per satu | 🟡 Medium | Medium | Blocked progress, harus rewrite banyak komponen sekaligus |
| R9 | **LLM hallucination amplification**: LLM menghasilkan kode yang terlihat benar tapi punya bug — dan karena Z3 verifikasi hanya sebaik constraints-nya, bug lolos | 🟠 High | Medium | False confidence, bug tersembunyi |
| R10 | **Token budget explosion**: Synthesis compiler butuh konteks besar — MCTS tree + LLM prompt + Z3 queries bisa melebihi 1M token window | 🟡 Medium | Low | Synthesis gagal di tengah jalan |

---

## 🛡️ SAFETY NETS

### For Every Risk, A Mitigation

| Risk | Mitigation |
|---|---|
| R1 Regression | **Golden test suite**: Sebelum ganti komponen apapun, rekam output compiler Rust asli untuk 100+ input spec. Generated compiler HARUS menghasilkan output byte-identical dengan compiler asli untuk test suite yang sama. |
| R2 Bootstrap paradox | **Cross-validation**: Untuk setiap komponen hasil sintesis, jalankan test suite yang sama pada compiler Rust original vs compiler hasil sintesis. HASIL HARUS IDENTIK. Compiler hasil sintesis TIDAK BOLEH jadi satu-satunya source of truth. |
| R3 Perf collapse | **Benchmark gate**: Setiap komponen hasil sintesis harus di-benchmark. Batas toleransi: max 2x slower dari Rust original. Lebih dari itu → reject. |
| R4 Architecture mismatch | **Interface contracts**: Sebelum sintesis, tulis `.morph` interface specs yang mendefinisikan type signature + pre/postcondition untuk setiap komponen boundary. Synthesis hanya di-allow jika memenuhi interface contract. |
| R5 Constraint explosion | **Decompose**: Jangan sintesis komponen besar sekaligus. Pecah jadi sub-fungsi ≤200 lines Rust. Synthesis per sub-fungsi, lalu compose. |
| R6 IR expressiveness | **IR audit**: Sebelum synthesis, pastikan setiap target Rust pattern bisa di-represent di IR. Kalau ada pattern yang tidak bisa → tambahkan IR node type dulu. |
| R7 Test blindness | **Fuzzing**: Generate random `.morph` inputs, compile dengan compiler original vs compiler hasil sintesis, bandingkan output. Minimum 10,000 random inputs. |
| R8 Dependency entanglement | **Interface-first**: Definisikan trait/interface tiap komponen DULU sebelum implementasi. Generated code harus implement interface, bukan import konkrit. |
| R9 LLM hallucination | **Triple-check**: LLM candidate → MCTS scoring → Z3 verification → Golden test suite → Human review. Tidak ada satu checkpoint pun yang boleh skip. |
| R10 Token budget | **Multi-turn synthesis**: Jangan satu prompt besar. Sintesis per sub-komponen, dengan hasil sebelumnya sebagai context. |

---

## 🔄 ROLLBACK STRATEGY

```
┌─────────────────────────────────────────────────────────────┐
│                 ROLLBACK PROTOCOL                            │
│                                                              │
│  Level 1 — Component rollback:                               │
│    Kalau satu komponen hasil sintesis gagal test:            │
│    → Revert file itu ke versi Rust asli                      │
│    → git checkout src/spec/parser.rs                         │
│    → Investigasi kenapa spec-nya kurang ketat                │
│                                                              │
│  Level 2 — Full rollback:                                    │
│    Kalau beberapa komponen gagal atau integrasi kacau:       │
│    → git revert ke commit sebelum self-hosting                │
│    → Semua komponen kembali ke Rust asli                     │
│    → Re-evaluate strategy                                    │
│                                                              │
│  Level 3 — Emergency stop:                                   │
│    Kalau synthesis corruption terdeteksi:                    │
│    → Lock branch master, tidak ada push baru                 │
│    → Audit semua generated code                              │
│    → Tidak ada generated code yang masuk ke main branch      │
│      sebelum lulus full review                               │
└─────────────────────────────────────────────────────────────┘
```

### Branch Strategy

```
master (stable, semua Rust asli)
  │
  ├── feat/self-host-tokenizer    ← tokenizer.morph → tokenizer.rs
  ├── feat/self-host-typeck       ← typeck.morph → typeck.rs
  ├── feat/self-host-codegen      ← codegen.morph → codegen.rs
  │      │
  │      └── Setiap branch punya:
  │          ├── spec/*.morph           (spec)
  │          ├── generated/*.rs         (output synthesis)
  │          ├── tests/*_verify.rs      (cross-validation tests)
  │          └── BENCHMARK.md           (perf comparison)
  │
  └── feat/self-host-integration   ← Semua komponen dirangkai
```

---

## 🧪 TESTING STRATEGY

### The 4-Layer Test Pyramid for Self-Hosting

```
        ┌──────────┐
        │  FUZZING │  10,000 random inputs, cross-compare
        │ 10K runs │  original vs synthesized output
        ├──────────┤
        │INTEGRATION│  Full pipeline: spec → parse → typeck
        │   TESTS   │  → synthesize → verify → codegen
        ├──────────┤
        │  GOLDEN   │  100+ known .morph specs, recorded
        │  OUTPUTS  │  outputs. Must be byte-identical.
        ├──────────┤
        │   Z3      │  Per-constraint formal verification.
        │VERIFY     │  UNSAT = proven correct.
        └──────────┘
```

### Test Harness Design

```morphic
// Example: test harness spec for tokenizer
spec test_tokenizer_equivalence {
    input: source: String
    output: Result<List<Token>, ParseError>

    constraint: output == original_rust_tokenizer(source)

    // Generated from 100+ recorded test cases
    test "keyword spec" { input: "spec"; expect: Ok([Spec]) }
    test "generic fn" { input: "fn<T>"; expect: Ok([Fn, Lt, Ident("T"), Gt]) }
    // ...
}
```

---

## 📊 SUCCESS CRITERIA

| # | Criteria | Threshold | Measurement |
|---|---|---|---|
| SC1 | **Z3 verification** | 100% constraints pass | `cargo test` in CI |
| SC2 | **Golden test equivalence** | 100% output match | Automated diff against recorded outputs |
| SC3 | **Performance** | ≤ 2x slower than Rust original | Benchmark suite (criterion.rs or similar) |
| SC4 | **Fuzzing pass rate** | ≥ 99.9% match across 10K inputs | Randomized input generation |
| SC5 | **Self-compile** | Morphic can compile its own specs | `morphic build tokenizer.morph` produces valid tokenizer |
| SC6 | **Bootstrap** | Generated compiler can compile a spec → generates correct code | Full roundtrip test |
| SC7 | **No regression** | Existing 25 tests still pass | `cargo test` unchanged |

---

## 🪜 EXECUTION MILESTONES

### M0: IR Audit (before writing any specs)
- [ ] Review semua 32 IRNode variants — cukupkah untuk compiler-level code?
- [ ] Identifikasi Rust patterns di compiler saat ini yang TIDAK bisa di-represent di IR
- [ ] Tambahkan IR node types jika perlu
- [ ] **Gate**: Semua pattern expressible di IR

### M1: Tokenizer (simplest component)
- [ ] Tulis `tokenizer.morph` spec (~100 lines)
- [ ] Tulis golden test suite (50+ recorded outputs)
- [ ] LLM seed → MCTS → Z3 verify → codegen
- [ ] Cross-validate: generated tokenizer vs Rust original
- [ ] Benchmark: must be ≤ 2x slower
- [ ] **Gate**: 100% golden test pass + Z3 verified

### M2: Pretty-Printer + Validator
- [x] Tulis `validator.morph` (type checker lite)
- [x] Tulis `printer.morph` (IR → text)
- [ ] Same validation pipeline as M1
- [ ] **Gate**: Both pass independently

> **Progress note (2026-11-20):** `examples/self_host/validator.morph` (7 specs: type-tag/node-kind domains, arity table, lite tag compatibility, depth guard) and `examples/self_host/printer.morph` (7 specs: type/kind/operator name lookups, precedence `needs_parens`, line-width rule) are written and are the first self-host specs verified end-to-end by the real pipeline: both pass `morphic check` (parser + typechecker in the compiled binary), enforced by `tests/m2_proof.rs` — which also carries pure-Rust mirror implementations with golden tests for every spec test case (the M0 Phase B pattern), plus negative controls proving `check` rejects invalid specs. Full suite: 59 passed / 0 failed.
>
> **Still open for the M2 gate:** the synthesis run has now been done (see the synthesis-run note below), but it did not produce a single compilable, test-passing implementation, and there is still no cross-validation of synthesized output against a Rust original — the Rust code in `tests/m2_proof.rs` is a hand-written mirror, not generated code. The gate therefore stays unchecked.
>
> **Baseline discovery while doing M2:** none of the pre-existing example specs (`sort.morph`, `binary_search.morph`, `functional_tokenizer.morph`, `self_host/is_sorted.morph`, `self_host/count_matching.morph`) currently pass `morphic check` — they use syntax the parser does not implement (no module wrapper, `if/then`, `implies`, record/list literals in tests, spaced operators; two files also trip a byte-boundary panic in the parser's error-snippet code at `src/spec/parser.rs:1094`). The M2 specs are written in the parser's actually-supported dialect, documented in their file headers. Wiring the older examples into `check` (or fixing the parser gaps) is future work, separate from M2.

> **Synthesis run (2026-11-20, branch `m2-synthesis-run`):** all 14 M2 specs were run through the real pipeline (parse → typecheck → synthesize → Z3 `verify_all` → `select_best` → codegen) via the test-only probe in `src/m2_synthesis_probe.rs` (`cargo test -- --ignored --nocapture m2_synthesis_probe`; generated winners are written to `target/m2-synthesis/`). Only opt level 1 can run: it is the sole LLM-free configuration. Opt levels 2–4 set `use_llm = true` and fail immediately in `seed_population` with `LlmError("No LLM backend enabled")` — the default build compiles neither LLM feature (`llm-local` / `llm-remote`), and no Ollama server exists in this environment (verified via CLI `build -O 2` / `-O 3` and locked in by the unit test `synthesis_opt3_blocked_without_llm_backend`). Candidate counts and times below are from one probe run; both vary between runs (randomized engine), while the Z3 verdicts and winner provenance were identical across 3 runs.
>
> | Spec | Synth (opt 1) | Candidates | Time | Z3 verdict (pipeline) | Winner | Holes in winner | Generated Rust compiles / passes spec tests |
> |---|---|---|---|---|---|---|---|
> | validator `is_known_type_tag` | ok | 18 | 5 ms | 0/18 verified | TestInduced | 1 | no / no |
> | validator `is_known_node_kind` | ok | 17 | 4 ms | 0/17 verified | TestInduced | 1 | no / no |
> | validator `node_arity` | ok | 18 | 18 ms | **18/18 Verified** | TestInduced | 1 | no / no |
> | validator `is_literal_kind` | ok | 19 | 5 ms | 0/19 verified | TestInduced | 1 | no / no |
> | validator `tags_compatible` | ok | 11 | 202 ms | 0/11 verified | Mutation | 1–3 | no / n-a (no in-spec tests) |
> | validator `validate_kind_tag` | ok | 15 | 4 ms | 0/15 verified | TestInduced | 1 | no / no |
> | validator `max_depth_ok` | ok | 19 | 4 ms | 0/19 verified | TestInduced | 1 | no / no |
> | printer `type_tag_name` | ok | 17 | 215 ms | **17/17 Verified** | TestInduced | 1 | no / no |
> | printer `node_kind_name` | ok | 19 | 227 ms | **19/19 Verified** | TestInduced | 1 | no / no |
> | printer `binop_symbol` | ok | 18 | 183 ms | **18/18 Verified** | TestInduced | 1 | no / no |
> | printer `needs_parens` | ok | 17 | 4 ms | 0/17 verified | Mutation | 0–1 | no / n-a (no in-spec tests) |
> | printer `is_infix_kind` | ok | 19 | 3 ms | 0/19 verified | TestInduced | 1 | no / no |
> | printer `line_width_ok` | ok | 17 | 5 ms | 0/17 verified | TestInduced | 1 | no / no |
> | printer `print_node_kind` | ok | 20 | 4 ms | 0/20 verified | TestInduced | 1 | no / no |
>
> **Why the run cannot close the gate — root causes, all verified in the code:**
> 1. **The Z3 stage is candidate-independent.** `Verifier::verify_one` (`src/verify/verifier.rs`) encodes only the spec's postconditions; the candidate body is never encoded (`is_well_formed` is a stub returning `true`). Its encoder maps boolean-valued subexpressions to int `0` (`encode_expr_int` fallback) and any `Or`-rooted formula to `true` (`encode_expr_bool` fallback), so the verdict is decided by constraint *syntax*: guard-style (`||`) lookup constraints always hold, `output == <bool expr>` equations never hold. The 4 "Verified" specs above are exactly the guard-style ones — a property of the spec text, identical for every candidate, not evidence that a synthesized implementation is correct.
> 2. **The search never executes tests.** `evaluate_candidate` (`src/synthesis/engine.rs`) returns `test_pass_ratio = 1.0` unconditionally and a constant `constraint_score = 0.7`; ranking is driven by AST-shape heuristics (depth/size) only.
> 3. **Winners keep their holes.** The test-induced seed's wildcard arm is a `Hole` and no pipeline stage fills it (the template builders also return bare holes); codegen renders holes as `panic!("Hole not filled")`. 13/14 winners carry ≥ 1 hole.
> 4. **Rust codegen defects (overlaps M3).** Spec test names are emitted verbatim as Rust fn identifiers — hyphens make 12/14 generated files unparseable; `Match` bodies are emitted as statements whose arm values are discarded (`value;`), which is E0308 once names are sanitized (proven on `is_literal_kind` and `node_kind_name`); unsupported IR in expression position becomes `/* UNSUPPORTED */` and unknown calls become `_()` (the two Mutation winners). Net: **0/14 generated files compile**, so 0/14 can be executed against their spec tests or cross-validated against the `tests/m2_proof.rs` mirrors.
>
> **Remaining for the M2 gate:** (a) an LLM backend (feature + server) or a scorer that actually executes spec tests, so the M1 pipeline shape (LLM seed → MCTS → Z3 → codegen) can run as described; (b) a verifier that encodes/evaluates the candidate itself, so "Verified" is a statement about the implementation; (c) the codegen fixes in (4) plus hole filling, so generated code compiles; only then (d) cross-validation against the mirrors. Until (a)–(c) land, "Same validation pipeline as M1" and the M2 gate stay unchecked — deliberately, per-spec evidence above.
>
> **Codegen fix follow-up (2026-11-20, branch `fix/codegen-output`):** defects (4a) and (4b) are fixed with regression tests — emitted Rust identifiers are sanitized (`rust_ident`: invalid chars → `_`, leading digit → `_` prefix, keywords → `_` suffix, applied at every Rust emission point), and match arms now yield their values as tail expressions (`ir_to_rust_tail`). Re-running the probe and compiling every winner with `rustc --edition 2021 --crate-type lib`: **8/14 compile** (was 0/14), **0/14 compile free of hole/unsupported markers** — every winner in this run still carries ≥ 1 hole (12× 1 hole, `needs_parens` 2, `tags_compatible` 3). The 6 remaining failures split into: 2 files (`needs_parens`, `tags_compatible`, the Mutation winners) still on defect (4c) — `_()` in an `if` condition does not parse; and 4 String-returning files (`binop_symbol`, `node_kind_name`, `print_node_kind`, `type_tag_name`) on a newly exposed defect (4d): `IRLiteral::String` is emitted as a `&str` literal while the spec return type is `String` (E0308) — previously masked by the `value;` discard. Fixing (4d), (4c), and hole filling (root cause 3) remains open, so item (c) above is only partially done.
>
> **Codegen fix follow-up 2 (2026-11-20, branch `fix/codegen-output-2`):** defect (4d) is FIXED with regression tests — codegen now threads the expected type to the emission points that know one (`ir_to_rust_expr_typed`: the spec return type for match arms and `Return` expressions, an `Alloc`'s declared type, a `Typed` node's own annotation), and only there is a string literal emitted owned (`"s".to_string()`); everywhere else it stays a `&str`, guarded by a negative test. Re-running the probe and compiling every winner as above: **12/14 compile** (was 8/14 — all 4 String-returning files now compile), **0/14 compile free of hole/unsupported markers** (unchanged: every winner in this run still carries exactly 1 hole, so the hole `panic!` marker remains in all 14; hole filling is root cause 3, still open). Defect (4c) is now diagnosed precisely and is NOT a codegen defect: the `_()` in the earlier run's `if` conditions is `IRNode::Call { function: "_", args: [] }`, emitted verbatim by codegen — the node itself is manufactured by the engine's random tree generator (`random_ir_tree`, `src/synthesis/engine.rs`, arm 3) as a placeholder leaf, a sibling of the real `Hole` leaves the same generator produces. It is an unfilled node in function position (engine territory, root cause 3 family); no function named `_` exists in any spec, template, or builtin, so there is nothing for codegen to emit — the fix belongs to the engine (stop generating placeholder calls, or fill them), and codegen was deliberately left unchanged for it. Variance note: winner shapes are randomized per run — in this run the two Mutation winners (`needs_parens`, `tags_compatible`) contained no `_()` at all and instead failed on E0425 (`return result;` after a `Hole` — the `Var("result")` leaf is another `random_ir_tree`/`random_filling` artifact). Item (c) above remains partially done: what is left is hole filling and the engine-side placeholder nodes.
>
> **Engine placeholder + hole-ranking follow-up (2026-11-20, branch `fix/engine-placeholders`):** defect (4c) is FIXED at the source — `random_ir_tree` arm 3 now emits an honest `Hole` instead of `Call { function: "_", args: [] }` (regression test: property sweep over generated trees asserting no `Call` to `"_"`; zero `_()` in every winner file since). Holes also cost something for the first time: `evaluate_candidate` decays correctness credit (`test_pass_ratio`, `constraint_score`) ×0.5 per hole in the body, and `select_best` hard-gates — if any hole-free candidate exists, a holed candidate cannot win; only when none exists does selection fall back to ranking the holed pool (unit tests cover both branches). Re-running the probe 3× and compiling every winner as above: **winner holes collapsed from 14× exactly 1 (every prior run) to 14× 0, 14× 0, and 13× 0 + 1× 1** — the single holed winner (`node_arity`, run 3) is the fallback case: no hole-free candidate reached its verified pool that run. **Compiles went DOWN: 0/14, 0/14, 1/14 (was 12/14); compiles free of hole/unsupported markers 0/14 (unchanged).** That is a change in *which* candidates win, not a codegen regression: the previously compiling winners were tidy trees carrying one hole, while the hole-free winners now crowned come from the random/Mutation family and fail on the next layer of generator artifacts — stray `Var("result")` leaves (E0425; the `random_ir_tree` arm 0 / `random_filling` family flagged in follow-up 2), unbound `a`/`b` vars (E0425), E0308 type mismatches, and `return` in statement position. Net: hole filling is no longer the binding constraint — the population produces hole-free candidates for nearly every spec — candidate *quality* is. The generator invents names that are never bound, and evaluation still cannot execute spec tests to tell a correct hole-free tree from a nonsensical one. Next blockers, in order: (1) stop the generator emitting free-variable leaves (`Var("result")`/`Var("output")` where nothing binds them) or bind them at codegen; (2) a scorer that actually executes spec tests (root cause (a) above), so ranking rewards correctness, not just completeness. Item (c) stays partially done. Suite after this branch: 89 passed / 0 failed / 3 ignored.

### M3: Codegen (biggest risk)
- [ ] Tulis `codegen.morph` (IR → Rust, 1 target dulu)
- [ ] Ini komponen paling kompleks — perlu decomposition jadi sub-specs
- [ ] **Gate**: Generated codegen produces byte-identical Rust output

### M4: Integration
- [ ] Rangkai tokenizer + validator + codegen
- [ ] Self-compile test: gunakan generated compiler untuk compile `sort.morph`
- [ ] Bandingkan output dengan compiler Rust original
- [ ] **Gate**: Byte-identical output untuk 100+ test specs

### M5: Bootstrap
- [ ] Generated compiler menyintesis komponen baru
- [ ] Binary baru = semua komponen dari hasil sintesis
- [ ] Roundtrip: binary baru compile `sort.morph` → output = Rust original
- [ ] **Gate**: Full bootstrap, no regression

---

## 🚫 ANTI-PATTERNS (JANGAN DILAKUKAN)

| ❌ Anti-pattern | ✅ Correct approach |
|---|---|
| Langsung sintesis komponen besar (parser 1000+ lines) | Decompose jadi sub-fungsi ≤200 lines |
| Percaya hasil Z3 tanpa cross-validation | Selalu cross-validate dengan compiler original |
| Hapus kode Rust asli setelah generated code masuk | Keep both, gunakan feature flag untuk switch |
| Satu branch untuk semua komponen | Satu branch per komponen, merge bertahap |
| Skip benchmark karena "masih prototype" | Benchmark setiap komponen, tetapkan baseline |
| Gunakan generated compiler untuk generate compiler lain | Gunakan Rust compiler original sebagai bootstrap |
| Spesifikasi terlalu longgar → hasil synthesis valid tapi salah | Spesifikasi ketat dengan golden test suite |

---

## 📋 PRE-FLIGHT CHECKLIST

Sebelum mulai eksekusi, pastikan:

- [ ] **IR Audit done** (M0) — tidak ada gap ekspresi
- [ ] **Golden test suite siap** — minimal 50 recorded inputs dengan expected output
- [ ] **Benchmark harness siap** — bisa ukur perf generated code vs original
- [ ] **Branch strategy documented** — semua tahu di mana bekerja
- [ ] **CI/CD support** — automated cross-validation di setiap push
- [ ] **Rollback drill** — pernah latihan revert generated code ke Rust original
- [ ] **LLM access confirmed** — Ollama running, model downloaded, tested
- [ ] **Token budget verified** — synthesis tidak OOM atau timeout
- [ ] **Semua tim/contributor informed** — tidak ada yang kaget

---

## 📈 SIGN-OFF

| Role | Name | Date | Signature |
|---|---|---|---|
| Author | thvtzy | ___ | __________ |
| Reviewer | ___ | ___ | __________ |

**DO NOT PROCEED WITHOUT ALL GATES GREEN.**

---

*"Self-hosting is not a feature. It's a proof that the language works."*
