# IKONIK 1.8 AUDIT — smart-contract target (`--target solidity`), measured (Phase 167)

**What this is.** A scope audit of **1.8 — the smart-contract
target (`--target solidity`)**, the first milestone on the
**Ikonik** rung (pinned at Phase 107, `ROAD_TO_SELF_HOSTING.md:303`,
after Disebar closed at Phase 166 / v1.7.0). Morphic's wedge
in smart contracts is straightforward: on-chain math, fee
schedules, vesting curves, bounds checks, and pure accounting
transitions are small functions where a single off-by-one or
missing precondition drains a treasury — and Morphic already
synthesizes, Z3-verifies, and packages such functions as
independently re-verifiable proof-carrying bundles (PCC, 1.4).
This audit measures what exists in `src/codegen/`, `src/spec/`,
`src/verify/`, and `src/pcc.rs` today, where the real semantic
gaps lie between Morphic and Solidity/EVM, what toolchains are
present, and which pure/view contract exhibits already crown
and bundle cleanly.

**Method.** The tree as it stands at v1.7.0 (`master` at
`80c10a2`, branch `docs/ikonik-167-solidity-audit`): source
inspection where reading settles a fact, and 8 executable
probes in `tests/phase167_solidity_probes.rs` where a claim
needs live measurement. Every empirical claim below carries
its `P167|` line or exact file/line citation. **Zero product
code was modified.**

**Headline.** **No core premise fails — pure/view smart-contract
codegen (`--target solidity`) + PCC bundle coupling fits
cleanly on top of 1.7's compiler with zero new Cargo
dependencies — PROVIDED 1.8 is scoped honestly to stateless
`pure` functions and refuses every semantic lie at codegen
time.** Five real gaps were measured and shape the D-list:

1. **Morphic has no contract-state model** (`storage`,
   `mapping`, `msg.sender`, `payable`, events, external calls):
   specs are pure `input -> output` functions, so generated
   Solidity functions are stateless `pure` functions (usable
   directly via `eth_call` and callable from any stateful or
   `view` Solidity contract; marking them `view` triggers
   `solc` Warning 2018).
2. **`Int` must map to `int256`, NOT `uint256`**, and four
   concrete integer gaps stand measured: `Expr::IntLit(i64)`
   caps literals at `i64::MAX` ($\approx 9.22$ ETH in wei);
   `U64` has **no** non-negativity assumption in Z3 (counterexample
   `x = -1`); `U256`/`Address` parse as `TypeRef::Named` and
   bind `Sym::Opaque` (Inconclusive); and Z3's `Int::div` is
   Euclidean (`(-7) div 2 == -4`) while the interpreter, Rust,
   and Solidity `int256` truncate toward zero (`-3`) —
   agreeing identically on non-negative domains (`a >= 0`, `b > 0`).
3. **Solidity `require(...)` CANNOT inherit Rust's
   `constraint_to_rust` / `expr_to_rust`:** Rust emits
   debug-only `debug_assert!`, renders `xs[0]` as `??`,
   renders `len(xs)` as invalid `len(xs.clone())`, and
   silently drops quantified `require: forall ...` to `true`.
   In a smart contract, dropping an assumed `require:` is a
   security vulnerability: Solidity must emit always-on
   runtime `require(<cond>, ...)` and **refuse by name** on
   quantifiers or any unrenderable precondition.
4. **No Solidity compiler or Foundry binary is installed**
   on this host (`solc`, `solcjs`, `forge`, `anvil`, `cast`
   all absent); `node` v24.13.0 and `npm` 11.6.2 are present,
   giving a clean cross-platform verification path via pinned
   `solc` (solc-js) + lightweight Node EVM execution in
   scratch `target/` (the Phase 160 `jco` precedent).
5. **Six realistic pure/view DeFi exhibits already synthesize,
   Z3-verify at Tier-1 (`Verified`), pass 100% of their
   batteries, and produce re-verifiable PCC bundles TODAY**
   (`fee_bps`, `debit_balance`, `bounded_payout`,
   `linear_vested`, `can_withdraw`, `non_empty_batch`), with
   one measured boundary (`batch_total` calling statement-bodied
   `list_sum` hits `inline_calls`'s expression-only limit,
   while `list_sum` itself already ships as a Tier-1
   encoder-crowned PCC bundle).

Suites after the audit (probes only, solo across the three
frozen configurations; baselines 981/981/837 + 8/8/7 probe
teeth — the Z3 gap probe is `#[cfg(feature = "z3-support")]`-gated):
**bootstrap-codegen 989/0/29 · no-Z3 844/0/22 · default 988 +
1 proven `list_sum` Z3-timeout load flip** (`p167_contract_exhibits_synthesize_verify_and_pcc_bundle`
flipped `list_sum.bundle.json` to `Inconclusive` inside the
full default run — the standing Phases 164/166 `list_sum`
crown-flip class — and passed green in isolation and as a solo
binary `8/0/0`, as well as inside `bootstrap-codegen` 989/0/29).

---

## 1. Codegen, CLI, bootstrap, and playground surface

**Target dispatch (`src/codegen/mod.rs:12-43`).** `codegen::generate`
currently dispatches six targets (`"rust"`, `"c"`, `"wasm"`,
`"python"`, `"javascript" | "js"`, `"ir"`). Probed:
`P167|target|solidity refused: Unsupported target language: solidity`
and `P167|target|sol refused: Unsupported target language: sol`.
Adding a dedicated module `src/codegen/solidity.rs` wired into
`codegen::generate` requires **zero new Cargo dependencies**
(the standing-13 manifest in `Cargo.toml` stays untouched).

**Bootstrap isolation (`bootstrap-codegen`).** `generate_rust_route`
(`src/codegen/mod.rs:472-481`) swaps ONLY the `"rust"` route
under `#[cfg(feature = "bootstrap-codegen")]`. The M3/M5
self-host mirrors (`src/codegen/bootstrap_adapter.rs:90-99`,
`src/codegen/synthesized_codegen.rs:2446`, `src/codegen/m3_gate.rs:4230-4250`,
`tests/m3_proof.rs:2641-2664`) pin the pre-1.8 route table
(`0..=5`, `6 = Err`) over a fixed test list that does **not**
include `"solidity"`. Adding `"solidity"` to `codegen::generate`
therefore works identically in `default`, `bootstrap-codegen`,
and `--no-default-features` with **zero M3/M5 cascade** (the
frozen M3 dispatch mirror stays the Rust-bootstrap mirror,
recorded openly).

**CLI default extension defect (`src/main.rs:827-829`).**
`Command::Build` writes its default output path via
`input.with_extension(target)`. For `--target solidity`, that
would write `<stem>.solidity` instead of `<stem>.sol`. The
CLI's `Build` arm needs a target-to-extension mapping for
`"solidity" | "sol" => "sol"` (and updated help text at
`src/main.rs:143`, which still lists only `rust, c, wasm`).

**Playground & DTO surface (`src/api.rs:627`, `playground/`).**
`api::synthesize_spec` renders a fixed 6-target array
(`P167|api-renders|count=6 targets=rust,c,wasm,python,javascript,ir`),
and `playground/corpus/run-agreement.mjs:157` pins
`w.renders.length !== 6`. Meanwhile, the Phase 166 playground
WASM bundle stands at **752,204 B against the frozen 771,072 B
cap** (`playground/budget.txt`), leaving **18,868 B** of
headroom. Putting `"solidity"` into `WinnerReport.renders`
immediately would pull the Solidity emitter into the WASM
graph, require updating the playground UI + corpus runner, and
risk the frozen byte budget. Keeping `api::render(cand, "solidity")`
available while leaving `WinnerReport.renders` at 6 in 1.8
(decision D6) protects the budget and keeps 1.8 decoupled from
playground churn.

**Module-level `TypeDef` absence (`src/codegen/mod.rs:3219-3222`).**
`CandidateImplementation` carries `FunctionSpec`, not the
enclosing module's `Vec<TypeDef>` (`candidate.spec.types()`
returns `Vec::new()`). User-defined `type Foo = record { ... }`
declarations therefore cannot be emitted as Solidity `struct`s
through `codegen::generate(&candidate, "solidity")` alone
without widening the codegen signature or carrying module
declarations in a contract-assembly entry point.

---

## 2. Type mapping Morphic ↔ Solidity & measured semantic gaps

### 2.1 Stdlib & IR inventory against Solidity ABI

Auditing all 35 frozen standard-library entries
(`stdlib/*.morph`, `P167|stdlib-types|...`, `P167|stdlib-gaps|...`):

| Bucket | Count | Entries | Solidity ABI fit |
|---|---:|---|---|
| Scalar `Int` / `Bool` only | **7** | `int_abs`, `int_min`, `int_max`, `int_clamp`, `int_sign`, `int_is_even`, `int_is_odd` | Exact value types (`int256`, `bool`) |
| Direct ABI (`Int`, `Bool`, `String`, `List<Int>`) | **18** (incl. the 7 above) | + `string_is_empty`, `list_is_empty`, `list_reverse`, `list_contains`, `list_sum`, `list_product`, `list_take`, `list_drop`, `list_is_sorted`, `list_sort`, `range` | `int256`, `bool`, `string memory`, `int256[] memory` (with named `string` length & `memory` append notes below) |
| Uses `Char` | **6** | `char_is_digit`, `char_is_alpha`, `char_is_whitespace`, `char_is_uppercase`, `char_is_lowercase`, `char_to_digit` | No native Unicode scalar type in Solidity — refuse in 1.8 |
| Uses `Option<T>` / `Result<T, E>` | **7** (1 overlaps `Char`: `char_to_digit`) | `char_to_digit`, `result_unwrap_or`, `option_unwrap_or`, `option_ok_or`, `list_min`, `list_max`, `list_binary_search` | No algebraic sum types in Solidity ABI — refuse in 1.8 |
| Higher-order (`fn(...)`) | **5** | `list_map`, `list_filter`, `list_fold`, `list_all`, `list_any` | Solidity ABI forbids function-typed public/external parameters — refuse in 1.8 |

Walking the shipped IR bodies of the 7 scalar entries and the
18 direct-ABI entries (`P167|ir-scalar7|...`, `P167|ir-abi18|...`):

- **Scalar 7:** `nodes={"BinOp", "If", "Literal", "Var"}`,
  `binops={"Eq", "Gt", "Lt", "Mod", "Neq", "Sub"}`, `unops={}`
  — only **4** IR node variants.
- **Direct-ABI 18:** `nodes={"Alloc", "Assign", "BinOp", "Block",
  "Call", "Collection", "For", "If", "Index", "Literal", "Return",
  "UnaryOp", "Var"}`, `binops={"Add", "And", "Append", "Eq", "Ge",
  "Gt", "Le", "Lt", "Mod", "Mul", "Neq", "Or", "Sub"}`,
  `unops={"Len"}` — only **13** IR node variants.

Two Solidity-specific mechanics surfaced by that IR walk:

1. **`IRBinOp::Append` on `int256[] memory`:** Solidity `memory`
   arrays have fixed length after allocation and have **no
   `.push()`** (only `storage` arrays do). Supporting `Append`
   (used by `list_reverse`, `list_take`, `list_drop`, `range`)
   in `pure` functions requires emitting a small `internal pure`
   copy-and-extend helper (`_morphic_append_int256`, $O(n)$ per
   step, clean and bounded for pure arrays) — or refusing
   `Append` if lists are kept read-only.
2. **`String` length (`bytes(s).length`):** Solidity `string`
   has no `.length` member; `bytes(s).length` counts UTF-8
   **bytes**, whereas Morphic `len(s)` counts Unicode **code
   points**. They agree identically for `string_is_empty`
   (`bytes(s).length == 0` iff code-point length is 0) and for
   all ASCII strings, and diverge on multi-byte UTF-8 — a named
   loss that must be documented in the type table.

### 2.2 The four measured `Int` ↔ `int256`/`uint256` gaps

| # | Dimension | Measured reality in Morphic v1.7.0 | Solidity `^0.8.x` reality | Consequence for 1.8 |
|---|---|---|---|---|
| 1 | **Signedness** | `Int` is signed everywhere: negative literals `(0 - 7)`, subtraction, `int_abs`, `int_sign`, `int_clamp`, `int_min`, `int_max` (`stdlib/int.morph`). | `int256` is signed $[-2^{255}, 2^{255}-1]$; `uint256` is unsigned $[0, 2^{256}-1]$ and reverts on `< 0`. | **`Int` MUST map to `int256`**, never `uint256`. Every `i64` value fits strictly inside `int256` ($[-2^{63}, 2^{63}-1] \subset [-2^{255}, 2^{255}-1]$). |
| 2 | **Unsigned / EVM types in Z3** | `U8..U64` parse into `TypeRef::U8..U64`, **but** `verifier.rs:1769-1777` binds all of them to unbounded signed `z3::ast::Int` with **no** `>= 0` bound: `P167\|z3-u64\|body=(x>=0) against output==true status=VerificationFailed counterexample={x: Int(-1)}`. Meanwhile `U256` and `Address` parse as `TypeRef::Named` (`P167\|type-parse\|...`) and bind `Sym::Opaque` (`P167\|z3-named-u256\|output==x status=Inconclusive`). | `uint8..uint64`, `uint256`, `address` enforce exact bit-width / unsigned invariants at the ABI and arithmetic layers. | Do **not** claim Z3 verifies unsignedness from `U64` alone — non-negativity must come from explicit `require: x >= 0` (which Z3 assumes AND Solidity enforces). Adding native `U256`/`Address` to parser + typeck + eval + Z3 is a separate language milestone, not 1.8 codegen. |
| 3 | **Integer literal ceiling** | `parser.rs:1242-1257` parses `Expr::IntLit(i64)`. Probed (`P167\|int-literal\|...`): `9223372036854775807` (`i64::MAX`) parses; `9223372036854775808` and `100000000000000000000` ($10^{20}$ wei = 100 ETH) **fail to parse** (`Invalid integer: number too large to fit in target type`). | Solidity supports arbitrary-precision rational compile-time literals up to 256 bits (`1e18`, `100 ether`, `type(uint256).max`). | Specs in 1.8 cannot write literals $> 9.22 \times 10^{18}$ (constants like basis points `10000`, durations, caps, `1e18` = $10^{18} < i64::\text{MAX}$ all fit; $> 9.22$ ETH in wei must be passed as parameters or computed). Documented honestly as a 1.7 language boundary. |
| 4 | **Overflow & negative division** | Z3 (`verifier.rs:2902-2915`) proves over unbounded $\mathbb{Z}$ with totalized `Div`/`Mod`. **Measured divergence (`P167\|z3-neg-div\|...`):** `eval` computes `(-7) / 2 == -3` (truncating toward zero), while Z3 proves `(-7) / 2 == -4` (SMT-LIB Euclidean `Int::div`) and refutes `-3`! Under `require: a >= 0` and `require: b > 0` (`P167\|z3-pos-div\|...`), Z3, interpreter, Rust, and Solidity agree identically. | Solidity `^0.8.0` reverts with `Panic(0x11)` on overflow/underflow and `Panic(0x12)` on div/mod by zero; `/` and `%` on `int256` truncate toward zero (`-7 / 2 == -3`). | Z3 `Verified` guarantees functional correctness on every **non-reverting** EVM execution (partial correctness modulo overflow/div-by-zero), exact for division on non-negative domains (`a >= 0, b > 0`). Every generated contract header must state this boundary explicitly. |

**Unsupported types — hard refusal table.** `Float` (no
floating point in Solidity), `Char`, `Option<T>`, `Result<T, E>`,
`Function(...)`, `Map<K, V>` (Solidity `mapping` is storage-only
and illegal in `pure` signatures), `Set<T>`, `Stream<T>`,
`Tuple` (unless explicitly flattened as multi-return later),
`Generic`, `Ref`, `Named` (unknown/record types when no struct
defs exist) MUST return `Err(CodegenError::...)` naming the
type and target `"solidity"`, never emit placeholder types.

---

## 3. `require:` → `require(...)` mapping & honesty audit

`require:` rows parse into `FunctionSpec.preconditions: Vec<Constraint>`
(`src/spec/parser.rs:210-214`). Across the 35 stdlib entries,
3 carry `require:` rows (`P167|stdlib-requires|count=3 entries=[("int_abs", 1), ("int_clamp", 1), ("list_binary_search", 1)]`).

Probing 7 realistic `require:` shapes through the parser and
Rust emitter (`P167|require-ast|...`, `P167|require-rust|...`):

| Row | Morphic `require:` | Parsed `Constraint` | Current Rust `constraint_to_rust` output | Solidity requirement |
|---|---|---|---|---|
| 0 | `lo <= hi` | `Expr(BinOp)` | `debug_assert!((lo <= hi), ...)` | `require((lo <= hi), "...");` |
| 1 | `x != 0 && lo >= 0` | `Expr(BinOp)` | `debug_assert!(((x != 0) && (lo >= 0)), ...)` | `require(((x != 0) && (lo >= 0)), "...");` |
| 2 | `list_is_sorted(xs)` | `Predicate` | `debug_assert!(list_is_sorted(xs.clone()), ...)` | `require(list_is_sorted(xs), "...");` + emit transitive `internal pure` helper |
| 3 | `len(xs) > 0` | `Expr(BinOp)` | `debug_assert!((len(xs.clone()) > 0), ...)` *(invalid Rust!)* | `require((int256(xs.length) > 0), "...");` |
| 4 | `xs[0] >= 0` | `Expr(BinOp)` | `debug_assert!((?? >= 0), ...)` *(invalid Rust catch-all!)* | `require((xs[0] >= 0), "...");` (with bounds check `xs.length > 0` / non-negative index) |
| 5 | `hi >= (0 - x)` | `Expr(BinOp)` | `debug_assert!((hi >= (0 - x)), ...)` | `require((hi >= (0 - x)), "...");` |
| 6 | `forall i: Int in range(0, len(xs)): xs[i] >= 0` | `Forall` | `debug_assert!(true, ...)` *(silent drop!)* | **HARD REFUSAL** (`Err(CodegenError)`) — never emit `require(true)`! |

**Three load-bearing takeaways:**

1. **Solidity MUST NOT port `constraint_to_rust` / `expr_to_rust`
   blindly.** Rows 3, 4, and 6 above prove that Rust's walker
   either emits non-compiling syntax (`len(xs.clone())`, `??`)
   or silently turns a quantified `require:` into `true`.
2. **Dropping an assumed `require:` on-chain is a soundness
   breach.** Because Phase 109's verifier assumes every
   `require:` row when proving postconditions (`verifier.rs:229-253`),
   a contract that omits a `require:` allows an attacker to call
   the function outside its verified domain. Therefore, any
   `Forall`, `Exists`, or unsupported `Expr` inside
   `preconditions` MUST fail Solidity generation with a named
   `CodegenError`.
3. **Transitive precondition closure.** Just as
   `generate_rust_with_library` (`src/codegen/mod.rs:372-405`)
   follows library calls inside both the body AND `preconditions`,
   the Solidity emitter must close over library/installed helpers
   referenced by `require:` rows and emit them as `internal pure`
   functions inside the generated contract.
4. **Dialect note measured during probing:** A constraint row
   cannot begin with `(` (`require: (0 - x) <= hi` fails parse
   with `Expected identifier, found '<'`, matching
   `examples/self_host/codegen.morph:31`); writing `hi >= (0 - x)`
   or `x != (0 - 1)` parses cleanly.

---

## 4. Contract container, mutability, and PCC coupling

**Mutability: `pure` (not `view`).** Morphic functions declare
zero storage variables and read no block/transaction context.
In Solidity `^0.8.x`, marking a stateless function `view`
emits compiler warning `2018` (*"Function state mutability can
be restricted to pure"*). Emitting `public pure` for entrypoints
and `internal pure` for transitive helpers compiles with **zero
warnings**, allows direct `eth_call` (zero gas off-chain), and
allows any stateful or `view` contract to call or inherit it.

**Container: `contract` vs `library`.**
- `contract <Name>` with `public pure` functions can be
  deployed directly onto an EVM and invoked via standard ABI
  calls (Foundry, ethers, viem, Remix, Node EVM harness) with
  zero library-linking overhead, AND can be inherited or called
  from other contracts.
- A `library` with `internal pure` functions cannot be deployed
  and tested standalone without generating a second harness
  contract.

**PCC bundle coupling (`src/pcc.rs`).** A Morphic PCC bundle
(`morphic-reverification-bundle`, `bundleVersion: 1`) is
already **target-independent**: it carries `(spec, body in
morphic-ir/1, tier, verdictBasis, verdict, crownRoute, battery)`
and **no target source**. That gives 1.8 a high-trust workflow
with **zero bundle-format bump**:
- `morphic build <file.morph> --target solidity` emits a
  standalone single-spec `.sol` contract (following the standing
  `build` arm, default extension `.sol`).
- A checked multi-spec contract assembly arm —
  `morphic build --contract-from <file.morph>` (sibling of
  `--package-from` and `--bundles-from`, sharing
  `codegen::package::admit_user_specs` + `pcc::produce_bundle`)
  — synthesizes every spec in a module under seed 42, admits
  only measured crowns, emits:
  1. One `<ModuleName>.sol` contract containing all admitted
     functions as `public pure` + transitive stdlib helpers as
     `internal pure` + NatSpec recording each function's tier,
     verdict basis, battery counts, and companion bundle sha256;
  2. One `<entry>.bundle.json` PCC bundle per admitted function
     in the same output directory (produced via `pcc::produce_bundle`,
     so any auditor can independently re-run Z3 with
     `morphic verify-bundle <entry>.bundle.json`);
  3. An explicit NatSpec proof-boundary header documenting the
     Z3 $\mathbb{Z}$ vs EVM `int256` checked-revert & negative-division
     contract.

---

## 5. Toolchain availability & verification harness

Probed on this Windows host (`P167|toolchain|...`):

| Tool | Status | Version |
|---|---|---|
| `solc` | **absent** | — |
| `solcjs` | **absent** | — |
| `forge` / `anvil` / `cast` | **absent** | — |
| `node` | **present** | `v24.13.0` |
| `npm` / `npx` | **present** | `11.6.2` |

GitHub Actions `ubuntu-latest` likewise has no preinstalled
`solc` or Foundry, while Node and Python are present.

**Three verification routes priced:**

1. **(a) Pinned `solc` (npm `solc` / solc-js, e.g. `0.8.28`) +
   lightweight Node EVM executor (e.g. `@ethereumjs/evm` +
   `@ethereumjs/abi` or `ethers` ABI coder) installed ONLY in
   scratch `target/`** — the exact Phase 160 `jco` precedent.
   - **Pros:** 100% cross-platform (Windows dev PC + Linux CI)
     under existing Node 24; zero OS-specific binary shims;
     zero Cargo dependencies; compiles generated `.sol` with
     official `solc` (asserting **zero errors and zero warnings**),
     **AND executes real EVM bytecode** against every Morphic
     battery point AND asserts runtime **revert** when a
     `require:` precondition is violated!
   - **Cost:** S–M (one small Node harness script + pinned exact
     package versions in scratch `target/`, never global, never
     committed `node_modules/`).
2. **(b) Pinned native `solc` static binary (sha256-verified,
   Phase 161 `wasm-tools` precedent) only.**
   - **Pros:** Native binary speed.
   - **Cons:** Separate Linux/Windows archive URLs + hashes, and
     **compile-only** — proves syntax/typecheck, NOT EVM
     runtime behavior or `require(...)` reverts.
3. **(c) Foundry (`forge` / `anvil`).**
   - **Cons:** Heavier external installer (`foundryup`),
     historically higher friction on native Windows CI/dev vs
     Node-based `solc-js` + EVM.

---

## 6. Candidate pure/view contract exhibits + PCC probe results

Six pure/view smart-contract specifications (`DeFiMath` in
`tests/phase167_solidity_probes.rs`) were run through the
standing seeded pipeline (seed 42), Z3 verifier,
`pcc::produce_bundle`, and `pcc::verify_bundle_text` with
**zero new product code** (`P167|exhibit|...`, `P167|pcc|...`):

| Exhibit | Signature | `require:` count | Z3 status (default) | Battery | PCC bundle bytes | Re-verifies? | Exercises |
|---|---|---:|---|---|---:|---|---|
| `fee_bps` | `(Int, Int) -> Int` | 3 (`amount>=0`, `bps>=0`, `bps<=10000`) | **Verified (Tier-1)** | 5/5/5 | 4,211 B | Yes (pipeline) | DeFi bps math, non-negative `/` |
| `debit_balance` | `(Int, Int) -> Int` | 2 (`amount>=0`, `balance>=amount`) | **Verified (Tier-1)** | 4/4/4 | 3,354 B | Yes (pipeline) | Accounting debit, underflow guard |
| `bounded_payout` | `(Int, Int, Int) -> Int` | 1 (`min_out<=max_out`) | **Verified (Tier-1)** | 5/5/5 | 4,244 B | Yes (pipeline) | Stdlib `int_clamp` composition (nested `If`) |
| `linear_vested` | `(Int, Int, Int) -> Int` | 2 (`total>=0`, `duration>0`) | **Verified (Tier-1)** | 6/6/6 | 5,648 B | Yes (pipeline) | Vesting cliff+cap (`int_clamp` + `*` + `/`) |
| `can_withdraw` | `(Int, Int, Int) -> Bool` | 2 (`balance>=0`, `limit>=0`) | **Verified (Tier-1)** | 5/5/5 | 5,361 B | Yes (pipeline) | Pure `Bool` withdrawal policy predicate |
| `non_empty_batch` | `List<Int> -> Bool` | 0 | **Verified (Tier-1)** | 3/3/3 | 2,460 B | Yes (pipeline) | `int256[] memory` + `len` (`int256(amounts.length)`) |

**Measured synthesis boundary (`P167|exhibit-boundary|batch_total`,
`P167|pcc-stdlib|list_sum`):**
- A user spec `batch_total` written as `output == list_sum(amounts)`
  crowns `Inconclusive` (1/3/3) because `list_sum`'s shipped body
  is statement-shaped (`Block/Alloc/For`), which `SpecIndex::inline_calls`
  declines to inline into a constraint-induced seed (the standing
  Phase 165 `weights_sorted` boundary).
- Meanwhile, `list_sum` itself already ships at
  `packages/bundles/list_sum.bundle.json` (7,394 B) and
  re-verifies as **Tier-1 / Z3-Verified (`encoder` route, 8/8/8)**.
  So loop-bodied array math (like `list_sum` and `list_is_sorted`)
  is exercised directly through the 18 direct-ABI stdlib entries
  and their shipped PCC bundles.
- In `--no-default-features` (no-Z3), all 6 `DeFiMath` exhibits
  honestly crown `Inconclusive` / `tier-2` (`ExecutedExact`,
  100% battery) and re-verify at Tier-2, while `list_sum.bundle.json`
  refuses by name as unexamined — preserving the standing
  no-Z3 honesty contract.

---

## 7. Synthesis — recommended shape, gate sketch, and D-list

### Recommended milestone shape

One milestone **1.8**, four phases (168–171), one gate,
lockstep **1.8.0**, **zero new Cargo dependencies**:

| Phase | Work | Price |
|---|---|---|
| 168 | Freeze (`IKONIK_18_SCOPE.md`) + core Solidity emitter (`src/codegen/solidity.rs`, `--target solidity` / `sol`, `.sol` CLI default extension, direct-ABI types, `require:` → `require(...)`, transitive stdlib `internal pure` helpers, `_morphic_append_int256` memory-array helper, hard refusals on unsupported types/quantifiers/IR) | M |
| 169 | Checked contract assembly + PCC coupling (`morphic build --contract-from <file.morph>` emitting `<Module>.sol` + NatSpec provenance/boundary header + companion `.bundle.json` PCC bundles) + committed exhibit module (`examples/contracts/defi_math.morph` or chosen home) | M |
| 170 | End-to-end Solidity compilation + EVM execution harness (pinned `solc-js` + Node EVM runner in scratch `target/`): zero `solc` warnings, ABI verification, 100% battery execution on `DeFiMath` + direct-ABI stdlib entries, AND runtime revert verification on violated `require:` preconditions | M |
| 171 | 1.8 Gate (all clauses verified fresh) + CI Solidity compile/EVM step + merge + `v1.8.0` lockstep stamp | S–M |

### Gate sketch — 1.8

1. **Core emitter & honesty refusals.** `codegen::generate(cand, "solidity")`
   (and `"sol"`) renders every direct-ABI exhibit and all 18
   direct-ABI stdlib entries cleanly; unsupported types (`Float`,
   `Char`, `Option`, `Result`, `Function`, `Map`, `Set`,
   `Generic`, unknown `Named`) and unrenderable preconditions
   (`Forall`, `Exists`) refuse by name with `CodegenError` —
   zero silent `require(true)` or placeholder types.
2. **Real `solc` compilation (zero warnings).** Every generated
   exhibit contract and direct-ABI stdlib contract compiles
   cleanly under pinned `solc` (`^0.8.24` / pinned `0.8.28`)
   with **0 errors and 0 warnings** (no Warning 2018 mutability
   lint, no unused vars, no shadowing).
3. **Real EVM execution + runtime `require(...)` reverts.**
   The compiled EVM bytecode answers **100% of every exhibit's
   and direct-ABI stdlib entry's frozen battery points**
   identically to Morphic's interpreter/Rust output, AND
   **reverts** on out-of-domain inputs that violate `require:`
   (e.g. `fee_bps(100, 10001)`, `debit_balance(100, 200)`,
   `int_clamp(5, 10, 0)`, `linear_vested(1000, 50, 0)`).
4. **Proof-carrying contract loop.** `morphic build --contract-from`
   emits `<Module>.sol` alongside `<entry>.bundle.json` files
   whose sha256 hashes match the contract's NatSpec provenance
   lines; `morphic verify-bundle` independently re-verifies
   every companion bundle (Tier-1 under Z3; honest refusal
   without Z3), and a tampered bundle refuses by name.
5. **Footprint, standings, and suites.** Zero new Cargo
   dependencies (standing-13 unchanged); playground WASM bundle
   within frozen 771,072 B; npm/component/registry budgets
   unmoved; CI Solidity step green; all three frozen test
   configurations green solo.

*Closing line (draft):* "1.8 closes when a Morphic specification
compiles to a zero-warning Solidity `pure` contract whose
`require:` preconditions revert on-chain, whose EVM bytecode
agrees point-for-point with the verified battery, and whose
accompanying PCC bundles let any auditor re-run the Z3 proof
from scratch."

### Decisions (D1–D9 — awaiting owner ratification before freeze)

**D1 — Integer mapping (`Int` vs `int256`/`uint256`).**
- **(a) `Int` → `int256` (signed 256-bit, Solidity `^0.8.x`
  checked arithmetic); `I8..I64`/`U8..U64` → `int8..int64` /
  `uint8..uint64` with explicit caveat that Z3 does not assume
  unsignedness unless stated via `require:`; `U256`/`Address`
  and $> i64::\text{MAX}$ literals stay out of 1.8.**
- (b) Map `Int` → `uint256` globally.
- (c) Add native `U256`, `Address`, and 256-bit literals across
  Morphic's parser, typechecker, interpreter, and Z3 verifier
  inside 1.8.
*Recommendation: (a)* — (b) is unsound on Morphic's signed
`Int` semantics (`int_abs`, `int_sign`, negative arithmetic);
(c) turns a target milestone into an XL language/verifier
overhaul. (a) is strictly wider than `i64`, sound on signed
values, and keeps `require: x >= 0` as the single explicit
source of non-negativity for both Z3 and EVM.

**D2 — `require:` translation and unrenderable preconditions.**
- **(a) Always-on runtime `require(<cond>, "<msg>");` via a
  dedicated `constraint_to_solidity` / `expr_to_solidity`
  walker (supporting boolean ops, comparisons, arithmetic,
  unary ops, `len`, `Index`, and transitive library calls),
  with HARD codegen refusal (`Err(CodegenError)`) on `Forall`,
  `Exists`, or any unrenderable expression — NEVER emit
  `require(true)`.**
- (b) Mirror `constraint_to_rust` (emitting `require(true)` for
  quantifiers).
*Recommendation: (a)* — Z3 assumes `require:` rows during
verification; emitting `require(true)` on a smart contract
exposes the unguarded domain to on-chain attackers.

**D3 — Type and IR scope (honest refusal boundary).**
- **(a) Direct-ABI subset: `Int`, `Bool`, `I8..I64`, `U8..U64`,
  `String` (documenting UTF-8 byte length), and `List<T>` over
  those scalar types, covering all 13 IR nodes used by the 18
  direct-ABI stdlib entries (including a small `internal pure`
  `_morphic_append_int256` helper for `IRBinOp::Append`); hard
  `CodegenError` refusal on `Float`, `Char`, `Option`, `Result`,
  `Function`, `Map`, `Set`, `Stream`, `Tuple`, `Generic`,
  `Ref`, `Record`/unknown `Named`, and unsupported IR nodes.**
- (b) Scalar `Int`/`Bool` only (7 stdlib entries; excludes
  `List<Int>` and `String`).
- (c) Invent custom ABI tuple encodings for `Option`/`Result`
  (e.g. `(bool, int256)`).
*Recommendation: (a)* — (b) excludes realistic batch/array
contract logic (`non_empty_batch`, `list_sum`, `list_is_sorted`)
that only needs 9 extra IR nodes; (c) invents non-idiomatic
ABI shapes outside Morphic's type table.

**D4 — Contract container and mutability.**
- **(a) `pragma solidity ^0.8.24;` + `SPDX-License-Identifier`,
  `contract <Name>` with `public pure` entrypoints and
  `internal pure` transitive helpers.**
- (b) `library <Name>` with `internal pure` functions only.
- (c) Mark generated functions `view`.
*Recommendation: (a)* — (c) triggers `solc` Warning 2018 on
stateless functions; (b) cannot be deployed or called via
`eth_call` without a wrapper contract; (a) deploys and tests
directly AND can be inherited or composed by stateful contracts.

**D5 — CLI surface and PCC bundle coupling.**
- **(a) Wire `--target solidity` (alias `sol`) into
  `codegen::generate` + `morphic build` (default output
  extension `.sol`), PLUS a checked multi-spec contract + PCC
  assembly verb (`morphic build --contract-from <file.morph>`)
  sharing `admit_user_specs` + `pcc::produce_bundle` to emit
  `<Module>.sol` with NatSpec provenance (tier, verdict basis,
  battery, companion bundle sha256) alongside its
  `<entry>.bundle.json` files — leaving `bundleVersion: 1`
  untouched.**
- (b) Single-spec `morphic build --target solidity` only (no
  multi-spec contract assembly or companion PCC bundles).
- (c) Bump PCC to `bundleVersion: 2` to embed Solidity source
  inside the bundle JSON.
*Recommendation: (a)* — (b) misses the proof-carrying contract
story; (c) breaks 1.4/1.7 bundle compatibility unnecessarily
when bundles are already target-independent pure IR evidence.

**D6 — Playground / `WinnerReport.renders` surface.**
- **(a) Support `api::render(cand, "solidity")` in the library
  API, but keep `WinnerReport.renders` at the standing 6 targets
  in 1.8 (leaving `playground/` and `run-agreement.mjs`'s
  6-target pin untouched and protecting the 18,868 B WASM
  headroom).**
- (b) Add `"solidity"` as a 7th render tab in `WinnerReport.renders`
  and `playground/` during 1.8.
*Recommendation: (a)* — avoids coupling playground UI/budget
re-baselining into the Solidity milestone; a playground tab
can be added later once emitter bytes are measured.

**D7 — Solidity compilation and EVM execution toolchain.**
- **(a) Pinned `solc` (npm `solc` / solc-js, e.g. `0.8.28`) +
  lightweight Node EVM runner installed only in scratch
  `target/` (the Phase 160 `jco` precedent — runs on both
  Windows and Linux under existing Node 24, zero new Cargo
  deps, zero global installs), verifying both zero-warning
  compilation AND real EVM bytecode execution + `require(...)`
  reverts in tests and CI.**
- (b) Pinned native `solc` binary only (compile-only, no EVM
  execution).
- (c) Foundry (`forge`/`anvil`).
*Recommendation: (a)* — Morphic never claims target correctness
without executing the artifact against the battery; (a) proves
both zero-warning compilation and real EVM execution + revert
behavior identically on Windows and Linux CI.

**D8 — Negative division and overflow boundary policy.**
- **(a) Document the exact proof boundary in every generated
  contract's NatSpec header and in `IKONIK_18_SCOPE.md` (Z3
  unbounded $\mathbb{Z}$ + totalized ops vs EVM 256-bit checked
  revert `Panic(0x11)`/`0x12`; Euclidean vs truncating division
  on negative operands, exact on non-negative domains), without
  altering Z3's global `sym_binop` in 1.8.**
- (b) Refuse `IRBinOp::Div` and `IRBinOp::Mod` in Solidity.
- (c) Change Z3's global `Int::div` encoding inside 1.8.
*Recommendation: (a)* — (b) kills `fee_bps` and `linear_vested`
even though their `require:` rows guarantee non-negative
operands where Z3 and EVM match identically; (c) is a separate
verifier-wide campaign with blast radius across all existing
proofs.

**D9 — Milestone shape and versioning.**
- **(a) One milestone 1.8 (phases 168–171), one gate, lockstep
  `1.8.0` stamped at Phase 171 only, zero new Cargo dependencies.**
- (b) Split emitter (1.8) and contract+PCC assembly (1.9).
*Recommendation: (a)* — emitter, PCC contract assembly, and
EVM execution harness share one honesty story and one gate,
matching the 4-phase rhythm of 1.6 and 1.7.

---

### Named findings and deferrals (not in 1.8)

- **Stateful contracts (`storage`, `mapping`, `msg.sender`,
  `payable`, events, external calls, reentrancy):** Morphic's
  spec language is pure functional (`input -> output`). Stateful
  smart-contract synthesis requires a state-transition / effect
  spec model — a future Ikonik milestone, explicitly not 1.8.
- **Native EVM types (`U256`, `Address`, `Bytes32`) and
  256-bit integer literals:** Currently `Expr::IntLit(i64)`
  caps literals at `i64::MAX`, `U64` carries no Z3 range bound,
  and `U256`/`Address` bind `Sym::Opaque`. Closing those at the
  root is a language/verifier milestone, not a codegen target.
- **Z3 Euclidean vs truncating negative division:** Measured
  here (`P167|z3-neg-div|...`) as a standing Z3-vs-interpreter
  / Rust / Solidity divergence on negative operands; disclosed
  honestly in 1.8 and deferred to a verifier-specific phase.
- **User-defined `Record` → Solidity `struct`:** Blocked at the
  single-candidate `codegen::generate` seam because
  `CandidateImplementation` carries no module `TypeDef`s;
  deferred unless widened in a later phase.
- **M3/M5 `bootstrap-codegen` dispatch mirror:** `synthesized_codegen::rust_target_route`
  remains the frozen pre-1.8 Rust-bootstrap mirror; `"solidity"`
  routes cleanly in `codegen::generate` under all three build
  configurations without touching M3/M5.
- **Statement-callee inlining (`batch_total` calling `list_sum`):**
  Recorded as the standing Phase 165 `SpecIndex::inline_calls`
  expression-only boundary; `list_sum` itself ships as a
  Tier-1 encoder-crowned PCC bundle.
