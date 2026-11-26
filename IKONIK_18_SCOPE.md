# IKONIK_18_SCOPE.md — Phase 168: the frozen 1.8 scope + the gate

The ratification + freeze document for **1.8 — the
smart-contract target (`--target solidity`)**, the first
milestone on Morphic's **Ikonik** rung. Source material:
[`IKONIK_18_AUDIT.md`](IKONIK_18_AUDIT.md) (Phase 167,
measured against v1.7.0 / `master` at `80c10a2`; probe
artifact `tests/phase167_solidity_probes.rs`, 8 probes,
every audit claim carrying a `P167|` line or file/line
citation).

**Ratification.** On 2026-11-26 the owner ratified the
audit's recommended package **D1a–D9a** under explicit
delegation (*"cocok aja. keputusan di kamu bagaimana
bagusnya."*), recorded here in the open so any decision
remains individually inspectable and vetoable. Per D9a
this milestone is one honesty story under one gate: a
pure/view Solidity contract target backed by Morphic's
standing Z3 verification and proof-carrying bundles.

This document freezes:

- **the ratified decisions** (§1): D1a–D9a with their
  audit grounds.
- **the milestone's contents and non-scope** (§2): what
  1.8 builds, lockstep version `1.8.0`, and explicit
  deferrals.
- **the type and IR contract** (§3): exact Morphic ↔
  Solidity type mapping, IR subset, memory-array helper,
  and mandatory hard-refusal boundary.
- **the `require:` and proof-boundary contract** (§4):
  runtime `require(...)` translation, hard refusal on
  quantifiers/unrenderable rows, transitive helper
  closure, and the mandatory NatSpec proof-boundary
  disclosure.
- **the contract container and CLI surface** (§5):
  `--target solidity` / `sol` (default `.sol` extension)
  and Phase 169's checked `--contract-from` + PCC bundle
  coupling.
- **the playground and dependency posture** (§6):
  `WinnerReport.renders` unchanged at 6, zero new Cargo
  dependencies, standing budgets unmoved.
- **the Solidity compilation and EVM execution harness**
  (§7): Phase 170's pinned `solc-js` + Node EVM runner in
  scratch `target/` and CI wiring.
- **the gate** (§8): five frozen clauses and the closing
  line verbatim from the audit draft.
- **the phase plan** (§9): Phases 168–171.

Phase 168 lands this freeze plus the core Solidity emitter
(§3, §4, §5.1). Phase 169 lands checked multi-spec contract
assembly + PCC coupling + committed DeFi exhibits (§5.2).
Phase 170 lands the `solc-js` + EVM execution harness (§7).
Phase 171 runs the gate (§8), wires CI, merges, and stamps
`v1.8.0`.

---

## 1. The ratified decisions (D1a–D9a)

**D1a — Integer mapping: `Int` → `int256`; `I8..I64` /
`U8..U64` → `int8..int64` / `uint8..uint64` with explicit
caveat that Z3 does not assume unsignedness unless stated
via `require:`; `U256`/`Address` and $> i64::\text{MAX}$
literals stay out of 1.8.**
*Grounds:* Morphic `Int` is signed (`int_abs`, `int_sign`,
negative arithmetic), so mapping `Int` → `uint256` (b) is
unsound; adding native `U256`/`Address` and 256-bit
literals across parser/typeck/eval/Z3 (c) is an XL
language overhaul. `int256` preserves signedness, strictly
contains `i64` ($[-2^{63}, 2^{63}-1] \subset [-2^{255},
2^{255}-1]$), and relies on explicit `require: x >= 0` as
the single source of non-negativity for both Z3 and EVM
(audit §2.2).

**D2a — `require:` translation: always-on runtime
`require(<cond>, "<msg>");` via a dedicated Solidity walker,
with hard `CodegenError` refusal on `Forall`/`Exists` or
any unrenderable expression — NEVER emit `require(true)`.**
*Grounds:* `constraint_to_rust` degrades `len(xs)` to
invalid `len(xs.clone())`, `xs[0]` to `??`, and `Forall`
to `true` (audit §3). Z3 assumes `require:` rows during
verification; silently dropping a `require:` in a smart
contract exposes the unverified domain to on-chain
attackers.

**D3a — Type and IR scope: direct-ABI subset (`Int`,
`Bool`, `I8..I64`, `U8..U64`, `String`, `List<T>` over
those scalars) and the 13 IR nodes covering all 18
direct-ABI stdlib entries (including a small `internal pure`
append helper for `IRBinOp::Append`); hard `CodegenError`
refusal on all unsupported types and IR nodes.**
*Grounds:* Scalar-only (b) would exclude realistic
array/batch contract specs (`non_empty_batch`, `list_sum`,
`list_is_sorted`) that need only 9 extra IR nodes;
inventing custom ABI tuples for `Option`/`Result` (c)
creates non-idiomatic ABI shapes outside Morphic's type
table (audit §2.1).

**D4a — Contract container and mutability: `pragma solidity
^0.8.24;` + `SPDX-License-Identifier: MIT`, `contract <Name>`
with `public pure` entrypoints and `internal pure`
transitive helpers.**
*Grounds:* Morphic functions have zero contract storage;
marking stateless functions `view` (c) triggers `solc`
Warning 2018; `library` with `internal pure` only (b)
cannot be deployed or called via `eth_call` without a
wrapper contract; `contract` with `public pure` deploys,
answers `eth_call`, and can be inherited or composed
directly (audit §4).

**D5a — CLI surface and PCC bundle coupling: wire `--target
solidity` (alias `sol`, default extension `.sol`) into
`codegen::generate` + `morphic build`, PLUS a checked
multi-spec contract + PCC assembly arm (`morphic build
--contract-from <file.morph>`) emitting `<Module>.sol`
(with NatSpec tier/verdict/battery/bundle-sha256) alongside
companion `<entry>.bundle.json` files (`bundleVersion: 1`
untouched).**
*Grounds:* Single-spec only (b) misses the proof-carrying
contract story; bumping `bundleVersion` to 2 (c) breaks
1.4/1.7 compatibility unnecessarily when PCC bundles are
already target-independent pure IR evidence (audit §4).

**D6a — Playground / `WinnerReport.renders`: `api::render(cand,
"solidity")` works in the library API, while
`WinnerReport.renders` remains at the standing 6 targets in
1.8.**
*Grounds:* Protects the frozen 771,072 B playground WASM
budget (18,868 B headroom) and `playground/corpus/run-agreement.mjs`'s
6-target pin from churn coupled into the Solidity milestone
(audit §1).

**D7a — Verification toolchain: pinned `solc` (npm `solc` /
solc-js) + lightweight Node EVM runner installed only in
scratch `target/` (the Phase 160 `jco` precedent), wired
into tests and CI for zero-warning compilation AND real EVM
bytecode execution + `require(...)` revert testing.**
*Grounds:* Native `solc` / Foundry are absent on the host
and default CI runners; `node` v24 + `npm` are present;
compile-only (b) would violate Morphic's discipline of
executing target artifacts against their batteries (audit
§5).

**D8a — Negative division and overflow boundary policy:
emit an explicit NatSpec proof-boundary disclosure on every
generated contract header (Z3 unbounded $\mathbb{Z}$ +
totalized ops vs EVM 256-bit checked revert `Panic(0x11)` /
`0x12`; Euclidean vs truncating division on negative
operands, exact on non-negative domains), without altering
Z3's global `sym_binop` in 1.8.**
*Grounds:* Refusing `Div`/`Mod` (b) kills `fee_bps` and
`linear_vested` even where `require:` restricts operands to
the exact non-negative domain; rewriting Z3's `Int::div`
globally (c) is a separate verifier-wide milestone (audit
§2.2).

**D9a — Milestone shape and versioning: one milestone 1.8
(Phases 168–171), one gate, lockstep `1.8.0` stamped at
Phase 171 only, zero new Cargo dependencies.**
*Grounds:* Emitter, PCC contract assembly, and EVM
execution harness form one end-to-end honesty story under
one gate (audit §7).

---

## 2. Milestone contents and explicit non-scope

**In scope (1.8, lockstep `1.8.0`):**
1. Core Solidity emitter (`src/codegen/solidity.rs`) behind
   `codegen::generate(cand, "solidity" | "sol")` and
   `api::render(cand, "solidity" | "sol")`, plus CLI
   `morphic build <file.morph> --target solidity` writing
   `<stem>.sol` by default (Phase 168).
2. Checked multi-spec contract + PCC bundle assembly
   (`morphic build --contract-from <file.morph>`) and a
   committed pure/view DeFi exhibit module (`DeFiMath`)
   (Phase 169).
3. Pinned `solc-js` + Node EVM execution harness in scratch
   `target/` verifying 0-warning compilation, ABI shape,
   100% battery execution, and runtime `require(...)`
   reverts (Phase 170).
4. Gate instrument, CI wiring, merge, and `v1.8.0` lockstep
   release stamp (Phase 171).

**Explicitly NOT in 1.8:**
- Stateful smart contracts (`storage`, `mapping`,
  `msg.sender`, `payable`, events, external calls,
  reentrancy).
- Native EVM spec types (`U256`, `Address`, `Bytes32`) or
  $> i64::\text{MAX}$ integer literals in Morphic's parser,
  typechecker, interpreter, or Z3 encoder.
- Changing Z3's global `Int::div` encoding or adding Z3
  unsignedness assumptions for `U8..U64`.
- User-defined `Record` → Solidity `struct` emission at the
  single-candidate `codegen::generate` seam.
- Adding `"solidity"` to `WinnerReport.renders` or the
  browser playground UI.
- Amending `examples/self_host/codegen.morph` or the frozen
  M3/M5 `bootstrap-codegen` dispatch mirror.
- Any new Cargo dependency in `Cargo.toml` / `Cargo.lock`.

---

## 3. Type mapping and IR contract (Phase 168)

### 3.1 Supported types (direct-ABI subset)

| Morphic `TypeRef` | Solidity parameter / return / local type | Notes |
|---|---|---|
| `Int` | `int256` | Signed 256-bit; strictly contains `i64`; Solidity `^0.8.24` checked arithmetic |
| `Bool` | `bool` | Exact |
| `I8` / `I16` / `I32` / `I64` | `int8` / `int16` / `int32` / `int64` | Exact ABI width; Z3 models as unbounded `Int` |
| `U8` / `U16` / `U32` / `U64` | `uint8` / `uint16` / `uint32` / `uint64` | Exact ABI width; Z3 does **not** assume `>= 0` without `require:` |
| `String` | `string memory` | `len(s)` renders as `int256(bytes(s).length)` (UTF-8 bytes, exact for ASCII and emptiness); `==`/`!=` via `keccak256(bytes(...))`; `++` via `string.concat(...)` |
| `List<T>` (for scalar `T` in `{Int, Bool, I8..I64, U8..U64}`) | `<T_sol>[] memory` | `len(xs)` renders as `int256(xs.length)`; `xs[i]` renders as `xs[uint256(i)]`; empty list `new <T_sol>[](0)` |
| `Unit` | *(void — no `returns (...)`)* | Return type only; refused in parameter position |

**Mandatory hard refusals (`CodegenError::InvalidIR` /
`CodegenError::InternalError` naming the unsupported type
and target `"solidity"`):** `Float`, `Char`, `Option<T>`,
`Result<T, E>`, `Function(...)`, `Map<K, V>`, `Set<T>`,
`Stream<T>`, `Tuple(...)`, `Array(T, n)` (unless scalar
fixed array is explicitly added later — refused in 1.8),
`Generic(_)`, `Ref(_)`, `Named(_)`, and nested `List<List<...>>`
or `List<String>`.

### 3.2 Supported IR subset and helpers

The emitter supports the 13 IR node forms measured across
all 18 direct-ABI stdlib entries plus expression `Let`,
`While`, and `UnaryOp::{Neg, Not, Abs, Clone}`:

- **Control & binding:** `Block`, `Let`, `If` (both
  statement/tail and ternary expression form), `For`
  (`for (int256 v = start; v < end; v++)`), `While`,
  `Alloc`, `Assign` (to variable or indexed `arr[uint256(i)]`),
  `Return`, `Typed` (transparent wrapper).
- **Values & expressions:** `Literal` (`Int`, `Bool`,
  `String`; `Float`/`Char`/`Unit` in value position refuse
  by name), `Var`, `BinOp` (`Add`, `Sub`, `Mul`, `Div`,
  `Mod`, `And`, `Or`, `Xor` [on `bool`: `a != b`], `Eq`,
  `Neq`, `Lt`, `Le`, `Gt`, `Ge`, `Concat` [on `String`:
  `string.concat(a, b)`], `Append` [on scalar `List<T>` via
  copy-extend helper]), `UnaryOp` (`Neg`, `Not`, `Abs`,
  `Len`, `Clone` [for `List<T>`: copy-clone helper so
  in-place mutation like `list_sort` never aliases the
  caller's memory slice]), `Collection` (`List` literals),
  `Index`, `Call` (builtins `len` + prelude/installed
  entries transitively closed as `internal pure` helpers).
- **Memory-array helpers (emitted ONLY when used, so scalar
  contracts carry zero dead code and zero `solc` warnings):**
  - `_morphic_append_int256(int256[] memory xs, int256 item) internal pure returns (int256[] memory)`
    (and typed siblings if needed for other scalar list types)
  - `_morphic_clone_int256(int256[] memory xs) internal pure returns (int256[] memory)`
    for `IRUnaryOp::Clone` on memory arrays (e.g. `list_sort`).
- **Mandatory hard refusals:** `Hole`, `Match` (unless
  simple scalar literal/wildcard if ever needed — refused
  by name if unsupported pattern), `Lambda`, `Closure`,
  `Map`, `Filter`, `Fold`, `Sort`, `Loop`, `Break`,
  `StructDef`, `TryPropagate`, `FieldAccess`, `RecordLit`,
  unknown `Call` names not in builtins or prelude/module
  scope, and `int_to_str` (refused by name — no decimal
  formatter in 1.8's minimal helper surface).

---

## 4. `require:` and proof-boundary contract

1. Every `spec.preconditions` row is emitted at the top of
   the function body as an always-on runtime guard:
   ```solidity
   require(<condition>, "<message>");
   ```
2. Supported `Constraint` variants in `constraint_to_solidity`:
   `True`, `Expr`, `Predicate`, `Not`, `Eq`, `Order`, `And`,
   `Or`, `Implies`.
3. Supported `Expr` variants in `expr_to_solidity`: `Var`,
   `IntLit`, `BoolLit`, `StringLit`, `BinOp`, `UnaryOp`
   (`Neg`, `Not`), `Length` (`int256(xs.length)` /
   `int256(bytes(s).length)`), `Index` (`xs[uint256(i)]`),
   `Call` (`len` or transitive prelude/module helper),
   `MethodCall` (`xs.len()`).
4. **Hard refusal (D2a):** `Constraint::Forall`,
   `Constraint::Exists`, and any unsupported `Expr` variant
   (`FloatLit`, `CharLit`, `Lambda`, `Comprehension`,
   `Field`, `RecordLit`, unsupported `MethodCall`, unknown
   `Call`) MUST return `Err(CodegenError)` naming the
   unrenderable precondition. Emitting `require(true, ...)`
   is strictly forbidden.
5. **Transitive precondition helper closure:** Prelude or
   installed helpers called inside `preconditions` (e.g.
   `require: list_is_sorted(xs)`) are transitively closed
   and emitted as `internal pure` helpers inside the
   contract alongside body helpers.
6. **Mandatory NatSpec proof-boundary header (D8a):** Every
   generated `.sol` file carries an explicit header stating:
   - Z3 verifies the Morphic IR over unbounded $\mathbb{Z}$
     with totalized division/modulo under the spec's
     `require:` assumptions.
   - Solidity `^0.8.24` enforces `require:` at runtime and
     reverts on 256-bit overflow/underflow (`Panic(0x11)`)
     or division/modulo by zero (`Panic(0x12)`).
   - Integer division/modulo on negative operands diverges
     between Z3 (Euclidean) and EVM (truncating toward
     zero); semantics are identical on non-negative domains
     (`a >= 0`, `b > 0`).
   - `String` length uses `bytes(s).length` (UTF-8 bytes,
     exact for ASCII and emptiness).

---

## 5. Contract container and CLI surface

### 5.1 Single-spec target (Phase 168)

- `codegen::generate(candidate, "solidity")` and
  `codegen::generate(candidate, "sol")` emit a complete,
  self-contained Solidity source file:
  - `// SPDX-License-Identifier: MIT`
  - `pragma solidity ^0.8.24;`
  - Proof-boundary NatSpec comment
  - `contract <PascalCaseName> { ... }`
  - Main function as `public pure`, transitive helpers as
    `internal pure` (deduplicated, sorted by name).
- `morphic build <file.morph> --target solidity` (or `sol`)
  uses `.sol` as the default output file extension when
  `-o` / `--output` is omitted.

### 5.2 Multi-spec checked contract assembly + PCC coupling (Phase 169)

- `morphic build --contract-from <file.morph> [--out <dir>] [--seed <u64>]`
  (sibling of `--package-from` and `--bundles-from`;
  refuses combinations with `--package`, `--facade`,
  `--bundles`, `--package-from`, `--bundles-from`, or a
  positional input by name):
  - Runs the shared `admit_user_specs` loop (seed 42 by
    default; measured rows only — any refused spec refuses
    the whole assembly with nothing written).
  - Produces one PCC bundle (`<entry>.bundle.json`) per
    admitted spec via `pcc::produce_bundle` (`bundleVersion: 1`
    untouched) and computes its SHA-256 via
    `registry::sha256_hex`.
  - Emits one `<ModuleName>.sol` contract containing all
    admitted functions (`public pure`), deduplicated
    transitive helpers (`internal pure`), and per-function
    NatSpec recording `tier`, `verdictBasis`, `battery`, and
    `bundleSha256`.
  - Ships a committed exhibit module (`examples/contracts/defi_math.morph`
    carrying the 6 audited `DeFiMath` specs).

---

## 6. Playground, budgets, and dependencies

- **Zero new Cargo dependencies:** `Cargo.toml` stays at
  the standing-13 dependency set (12 root + native `sha2`);
  `Cargo.lock` does not gain any package.
- **Playground untouched in 1.8 (D6a):**
  `WinnerReport.renders` keeps its 6 standing targets;
  `playground/budget.txt` stays frozen at **771,072 B**.
- **All standing budgets unmoved:** npm ≤ 76,800 B,
  library component ≤ 106,496 B, callback component
  ≤ 90,112 B, registry index ≤ 12,288 B, catalog ≤ 295,000 B.

---

## 7. Solidity compilation and EVM execution harness (Phase 170)

- Installed ONLY inside a scratch directory under `target/`
  (never global, never committed `node_modules/`), pinning
  exact versions of npm `solc` (solc-js, e.g. `0.8.28`) and
  a lightweight EVM runner (`@ethereumjs/evm` + ABI coder)
  under Node 24 (the Phase 160 `jco` precedent).
- The harness:
  1. Compiles every generated `.sol` via `solc` Standard
     JSON Input (`optimizer: { enabled: false }` or pinned
     settings) and asserts **0 errors and 0 warnings**.
  2. Deploys the compiled creation bytecode into the local
     EVM and executes ABI-encoded calls for **100% of every
     exhibit's and direct-ABI stdlib entry's frozen battery
     points**, asserting exact value agreement.
  3. Executes out-of-domain inputs violating `require:`
     rows and asserts EVM **revert** with the expected
     revert reason.
- Wired into `.github/workflows/ci.yml` at Phase 171.

---

## 8. The 1.8 gate (frozen — never amend)

1. **Core emitter & honesty refusals.**
   `codegen::generate(cand, "solidity")` (and `"sol"`)
   renders all 6 `DeFiMath` exhibits and all 18 direct-ABI
   stdlib entries cleanly; all 17 non-direct-ABI stdlib
   entries, unsupported types (`Float`, `Char`, `Option`,
   `Result`, `Function`, `Map`, `Set`, `Generic`, unknown
   `Named`), and unrenderable preconditions (`Forall`,
   `Exists`) refuse by name with `CodegenError` — zero
   silent `require(true)` or placeholder types.
2. **Real `solc` compilation (zero warnings).** Every
   generated exhibit contract and direct-ABI stdlib
   contract compiles cleanly under the pinned `solc` with
   **0 errors and 0 warnings**.
3. **Real EVM execution + runtime `require(...)` reverts.**
   The compiled EVM bytecode answers **100% of every
   exhibit's and direct-ABI stdlib entry's frozen battery
   points** identically to Morphic's interpreter/Rust
   output, AND **reverts** on out-of-domain inputs that
   violate `require:` (including `fee_bps(100, 10001)`,
   `debit_balance(100, 200)`, `int_clamp(5, 10, 0)`,
   `linear_vested(1000, 50, 0)`).
4. **Proof-carrying contract loop.** `morphic build
   --contract-from` emits `<Module>.sol` alongside
   `<entry>.bundle.json` files whose sha256 hashes match
   the contract's NatSpec provenance lines; `morphic
   verify-bundle` independently re-verifies every companion
   bundle (Tier-1 under Z3; honest refusal without Z3), and
   a tampered bundle refuses by name.
5. **Footprint, standings, and suites.** Zero new Cargo
   dependencies (standing-13 unchanged); playground WASM
   bundle within frozen 771,072 B; npm/component/registry
   budgets unmoved; CI Solidity step green; all three
   frozen test configurations green solo.

**Closing line:** *"1.8 closes when a Morphic specification
compiles to a zero-warning Solidity `pure` contract whose
`require:` preconditions revert on-chain, whose EVM bytecode
agrees point-for-point with the verified battery, and whose
accompanying PCC bundles let any auditor re-run the Z3 proof
from scratch."*

---

## 9. Phase plan

| Phase | Branch | Deliverables |
|---|---|---|
| **168** | `feat/ikonik-168-freeze-solidity` | Freeze `IKONIK_18_SCOPE.md` + core Solidity emitter (`src/codegen/solidity.rs`, `--target solidity` / `sol`, `.sol` CLI default extension, direct-ABI types, `require:` walker, transitive helpers, hard refusals, TDD suite) |
| **169** | `feat/ikonik-169-contract-pcc` | `morphic build --contract-from <file.morph>` checked assembly + NatSpec provenance/sha256 + companion PCC bundles + `examples/contracts/defi_math.morph` |
| **170** | `feat/ikonik-170-evm-harness` | Pinned `solc-js` + Node EVM harness in scratch `target/`: 0-warning compile + 100% battery execution + `require(...)` revert pins across `DeFiMath` and the 18 direct-ABI stdlib entries |
| **171** | `feat/ikonik-171-gate-merge` | Fresh 5-clause gate instrument (`tests/phase171_gate.rs`), CI Solidity step, PR + green CI merge, `v1.8.0` lockstep stamp |
