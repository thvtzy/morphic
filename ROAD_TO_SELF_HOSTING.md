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
- [ ] Tulis `validator.morph` (type checker lite)
- [ ] Tulis `printer.morph` (IR → text)
- [ ] Same validation pipeline as M1
- [ ] **Gate**: Both pass independently

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
