# 🔍 M1 Features Audit — Method Calls, IR Field Access, Record Literals

**Phase 72 (2026-11-22), branch `docs/m1-features-audit`. AUDIT + PROPOSAL
ONLY: no production code changed, no spec re-authored. Every "current
state" claim below was measured from source on master @ `ef4a0e5`
(v0.6.0), not assumed.**

**Placement choice (recorded):** this audit is a standalone document —
the `M0_IR_AUDIT.md` precedent — linked from the ROAD's M1 section,
rather than a new ROAD section. The ROAD's phase notes are the running
record; a design audit with five pending owner decisions is a document
the owner reads once and ratifies, and it should not be interleaved
into the gate ledger. The ROAD M1 section carries a pointer and, once
ratified, the decisions themselves.

---

## 0. What this audit is for

The M1 gate (ROAD, §M1) demands: tokenizer spec written, golden suite,
LLM seed → MCTS → Z3 verify → codegen, cross-validation against the
Rust original, benchmark ≤ 2× — **Gate: 100% golden test pass + Z3
verified**. On 2026-11-21 the owner deferred that gate, deliberately,
until the language has **(1) method calls in spec expressions,
(2) a field-access node in the IR, and (3) record literals**, "so the
gate can pass in full (100% golden + Z3 verified) without compromising
its standard (e.g. by accepting executed agreement in place of Z3
verification for the Char classifiers…)". On 2026-11-22 the owner chose
to build exactly these three features. This audit prices that build —
and its headline finding is in §5: **the three features are necessary
but not sufficient for the gate as the owner stated it**, because the
Char classifiers cannot be Z3-verified under any of the three; a
fourth workstream (a Z3 Char encoding) is the only route to the full
standard, and it is presented as decision D5 rather than smuggled
into the plan.

Where the three features stand today (Phases 47–49 record): the
tokenizer module `examples/functional_tokenizer.morph` carries 8 specs.
The three Char classifiers are **exact by executed agreement, 774/774**
(Phase 49, `char_equation` seeds), Z3 **Inconclusive by construction**
(Char is Opaque to the encoder). `peek` / `next_char` are carried in
guard form but were measured **PARTIAL, never executed** in Phase 48:
no constraint seed reached any pool (24/24 spec-runs seedless), the
winners' generated code fails rustc (bodies perform arithmetic on the
state record itself), and "Z3 is Inconclusive throughout (field
equations over a record output are unencodable)". `skip_whitespace`
is carried only as far as it was ever stated (two postconditions).
`tokenize_one` / `tokenize_all` carry **signatures only** — their
constraints were dropped at re-authoring because they call methods on
Result values, and their Phase 48 "Verified" verdicts are recorded as
**vacuous** (an empty constraint set refutes nothing). 9 of the
original 18 tests live only in the mirror golden
(`tests/tokenizer_proof.rs`) because they are written with record
literals the test syntax cannot express.

---

## 1. Feature 1 — Method calls in spec expressions

### 1.1 The concrete need (enumerated, not paraphrased)

The dropped constraints survive verbatim in git history
(pre-re-authoring file, commit `acda3f9`). Exactly three constraint
lines, over exactly two specs:

| Spec | Constraint (original text) | Receiver | Methods called |
|---|---|---|---|
| `tokenize_one` | `output.is_err() \|\| len(output.unwrap().tokens) == len(state.tokens) + 1` | `output : StepResult = Result<TokenizerState, String>` | `is_err()`, `unwrap()` — then field `.tokens` on the unwrapped value |
| `tokenize_one` | `output.is_ok() \|\| state.pos >= len(state.source)` | `output : StepResult` | `is_ok()` |
| `tokenize_all` | `output.is_err() \|\| output.unwrap().len() >= len(state.tokens)` | `output : Result<List<Token>, String>` | `is_err()`, `unwrap()`, then **`.len()`** — a method on `List<Token>` |

The complete method set the tokenizer waits on is therefore four
zero-argument methods: **`is_ok()`, `is_err()`, `unwrap()` on
`Result<T, E>`, and `len()` on `List<T>`** (the method spelling of the
free function `len(x)` used everywhere else in the file). No method
with arguments, no method on a user-defined type, no chaining beyond
`unwrap()` → field / `unwrap()` → `len()`. Any design that covers
more than this is buying generality the gate does not need — priced
as such below.

### 1.2 Current state, layer by layer

- **AST** (`src/spec/ast.rs`): there is no MethodCall variant — the
  tokenizer file's own header says so. Calls are
  `Expr::Call(String, Vec<Expr>)`: a bare name, no receiver.
- **Parser** (`src/spec/parser.rs`, `parse_primary`): an identifier
  followed by `(` returns `Expr::Call` **immediately, without entering
  the postfix loop**; the `.field` / `[index]` postfix loop runs only
  on the bare-variable path. Consequences, measured from the code:
  `output.is_err()` parses `output` as a `Var`, consumes `.is_err` as
  a **field access**, then strands on the `()` — a parse error.
  `output.unwrap().tokens` is worse: nothing after a call result can
  postfix at all today.
- **Typechecker** (`src/spec/typeck.rs`, `check_expr`): `Call`
  resolves module specs first, then a hardcoded stdlib table
  (`len`, `sum`, `sort`, …), else "Called undefined function". There
  is no receiver typing to resolve a method against. The `Field` arm
  (line 207) returns the base's type unchecked — "field is deferred
  to runtime".
- **Interpreter**: constraints are never interpreted (tests judge
  bodies; Z3 judges constraints), so method calls need no `eval.rs`
  arm — but the **compiled runner** renders constraints into Rust
  asserts through `expr_to_rust`, which today renders `Field` (kind
  5) as the fixed text `??` (see §4). A method call that renders as
  `??` is a constraint the native judging route silently mangles.
- **Z3** (`src/verify/verifier.rs`, `encode_expr`): `Call` handles
  `len` and the parser's `List` literal specially; everything else
  goes to `sym_invoke` — the Phase 19 spec-to-spec inlining. That
  machinery does **not** extend to methods as-is: `sym_invoke`
  resolves a *name* against the module index, and `is_err` /
  `unwrap` are not specs and cannot be written as specs (they inspect
  a constructor the constraint language cannot pattern-match).
  Worse, `Sym` (line 309) has **no Result representation at all** —
  `Int / Bool / Str / List(Vec<Sym>) / Opaque` — so there is nothing
  for `is_err()` to test or `unwrap()` to project. Method calls in
  Z3 are therefore gated on the Result encoding designed in §1.3,
  which shares its machinery with Feature 2's record encoding.
- **Seed families** walk `Expr` structurally (`expr_to_ir`,
  guard/equation matchers in `synthesis/engine.rs`); an unrecognized
  variant becomes a `Hole` (§2.2), so a restored constraint with
  method calls would seed nothing until the lowering learns the node.

### 1.3 Candidate designs

- **(a) Native node + closed builtin table.** New
  `Expr::MethodCall { receiver, name, args }`; the typechecker
  resolves against a closed table keyed by receiver type —
  `Result`: `is_ok() -> Bool`, `is_err() -> Bool`,
  `unwrap() -> T`; `List`: `len() -> Int` — and rejects any other
  method with an error naming it. Z3: `Result<T, E>` gains a
  symbolic form (discriminant + payload, §2.3's machinery);
  `is_err`/`is_ok` encode as the discriminant test, `unwrap` as the
  payload projection — total and sound under exactly the guard rows
  the tokenizer writes (`is_err() || …unwrap()…`: on the Err branch
  the projection is an unconstrained payload value and the
  disjunction is already satisfied). `expr_to_rust` renders the
  method syntax verbatim (Rust's own `Result` methods have the same
  names — the compiled-runner asserts become honest Rust).
  *Trade-off:* a closed table is a language wart the day a user
  wants a fifth method; accepted deliberately, with (c) recorded as
  the extension path.
- **(b) Parse-time desugar.** `r.is_err()` rewrites in the parser to
  `Call("__result_is_err", [r])`, `l.len()` to `Call("len", [l])`,
  etc. No new AST variant, no kind-table fallout (§4). *Trade-offs:*
  the invented names leak everywhere — diagnostics, the LLM prompt
  formatter, `expr_to_rust` (which still needs arms under the fake
  names), and every future tool; `unwrap()` still needs the §1.3(a)
  Z3 Result encoding, so the saving is only the node, bought with a
  permanent fiction in the AST. The codebase's existing desugars
  (`"List"`, `"__tuple__"`) are literal forms, not operations with
  semantics — this would be a different kind of lie.
- **(c) General UFCS resolution.** A method call resolves to any
  module spec or stdlib function whose first parameter's type
  matches the receiver's. Most extensible — user types gain
  "methods" for free — and the largest typechecker surface: arity
  and shadowing rules, ambiguity when two specs match, and the four
  builtins *still* need native semantics (they are not definable as
  specs, per §1.2). Buys generality no current spec exercises.

**Recommendation: (a)** — native node, closed table of exactly the
four measured methods, (c) recorded as the future extension (the
table is consulted first; UFCS can be layered later without changing
any accepted program's meaning).

---

## 2. Feature 2 — A field-access node in the IR

### 2.1 The gap, stated precisely (it is not where it looks)

Field access **exists** in the spec language: `Expr::Field(Box<Expr>,
String)` (`src/spec/ast.rs`), parsed by the postfix loop, formatted
by `llm/prompt.rs`, walked by the engine's `expr_mentions_var` /
`expr_calls_indexed` / `expr_same`, and documented in
LANGUAGE_REFERENCE §11 ("Field Access & Indexing"). The owner's
deferral names "a field-access node **in the IR**" — and that is
exactly where the hole is. Measured, layer by layer:

- **Lowering** (`synthesis/engine.rs`, `expr_to_ir`, line 5463): the
  match lowers literals, `Var`, `Call`, `BinOp`, `UnaryOp` — and
  **everything else falls into the `_ => IRNode::Hole` arm**.
  `Field`, `Index`, `Lambda`, `Length`, `Comprehension` all become
  holes. (Note the asymmetry: `IRNode::Index` *exists* and is
  evaluated/encoded/rendered — only the lowering refuses it. Field
  has no IR node at all.) Every constraint-derived seed built from a
  constraint mentioning `state.pos` therefore carries a hole where
  the field was — the direct cause of Phase 48's "no constraint seed
  reached any pool (24/24 seedless)" for the record specs.
- **IR** (`synthesis/engine.rs`, `IRNode`, line 177): 28 variants,
  kind codes 0–27 in the Codegen specs. **No field-access variant,
  and no record-construction variant.** `StructDef { name, fields,
  body }` is a *declaration wrapper*, not a value. `Collection`
  covers List/Set/Map/Array/Tuple only.
- **Interpreter** (`synthesis/eval.rs`): `Value` (line 22) is
  `Int / Float / Bool / Str / Char / Unit / List` — **no record
  value, no Option value, no Result value**. Phase 48's verdict is
  the consequence: "the interpreter's `Value` has no record/Option/
  Result variant, so there is nothing to bind `state` to" — the five
  record specs could only be judged through compiled runners.
  `TryPropagate` evaluates to `NotEvaluable("…no Result values")`;
  `Match` constructor patterns return `Ok(None)` (unmatchable).
- **Z3** (`verify/verifier.rs`): `encode_expr` maps
  `Expr::Field(..)` to `None` (line 1011) — **one field access makes
  the entire constraint unencodable**, hence Inconclusive.
  `param_env` (line 411) binds only Int-family/Bool/String params to
  constants; a `TokenizerState` param binds `Sym::Opaque`, so even a
  field-free constraint over a record param starts from an opaque
  blob. `Sym` has no record form.
- **Typechecker**: the `Field` arm returns the base type without
  looking the field up — `peek`'s constraints typecheck today only
  because nothing checks that `TokenizerState` has a `pos`.

So Feature 2 is four coordinated pieces: an IR node, an interpreter
value, a Z3 value, and a real typechecker arm — plus the lowering
arms for `Field` **and** `Index` (the cheap sibling the same phase
should not leave behind).

### 2.2 The concrete need

Record types are declared nominally in the module
(`TypeKind::Record` exists in the AST; LANGUAGE_REFERENCE §14):

- `Token { kind: TokenKind (= Int), lexeme: String, line: Int, col: Int }`
- `TokenizerState { source: String, pos: Int, line: Int, col: Int, tokens: List<Token> }`

Field **reads** in the carried constraints (current file):
`state.pos`, `state.source`, `state.source[state.pos]`,
`output.pos`, `output.line`, `output.col`, `output.source`,
`len(state.tokens)`, `len(output.source)` — Int, String, and
List-of-record fields, one level deep, plus reads on the *unwrapped*
state in the dropped constraints (`output.unwrap().tokens`).
Field reads also appear inside the mirror's domain in
`tests/tokenizer_proof.rs` (Rust structs with the same fields).

Bodies must also **construct** updated records (`next_char` returns a
whole new `TokenizerState`; `tokenize_one` returns one with a token
appended). Construction is Feature 3's literal at spec level; its IR
half (a record-construction node) is priced with Feature 2's fallout
in §4, because the fallout is paid per IR variant either way.

### 2.3 Candidate designs

- **(a) Native node + record values at every layer.**
  `IRNode::FieldAccess { base, field }`;
  `Value::Record(Vec<(String, Value)>)`;
  `Sym::Record(Vec<(String, Sym)>)` — the `Sym::List(Vec<Sym>)`
  precedent: a record param binds as a record of per-field fresh
  constants (fields whose types have no sort — Char, nested records
  pending their own arms — bind Opaque leaves, honestly);
  `encode_expr`'s `Field` arm becomes projection by name; record
  equality in `sym_binop` is fieldwise conjunction; `expr_to_ir`
  lowers `Field` and `Index`; the typechecker's `Field` arm resolves
  the declared field type when the base's type is a record declared
  in the module, and keeps today's coarse deferral otherwise
  (bounding the tightening risk to declared records — measured
  against every gate corpus in Phase 74). *Trade-off:* the full
  §4 fallout — every exhaustive `IRNode` match in the compiler gains
  an arm, and the self-hosted Codegen specs gain a kind row. That
  bill is real and is the main price of this whole campaign.
- **(b) Desugar to a builtin call.** `expr_to_ir` lowers
  `Field(x, "pos")` to `Call { function: "__field_pos", … }` or a
  generic `__field(x, "pos")`. Saves the IR variant and its emitter
  arms — and nothing else: the interpreter and Z3 still need record
  *values* (§2.1), so the expensive halves are paid anyway, while
  bodies become opaque to fingerprints, templates, mutation, and
  every reader of generated IR. Poor trade, recorded for
  completeness.
- **(c) Records as tuples.** Lower record types to positional
  tuples; field access becomes `Index` with a compile-time field
  position. Reuses the existing `IRNode::Index` everywhere.
  *Trade-offs:* field names vanish from diagnostics, renders, and
  Z3 counterexamples; reordering a type's fields silently rewires
  every spec; and it fights the nominal `TypeKind::Record` the
  language already declares and the reference already documents.
  The mirror structs in `tests/tokenizer_proof.rs` are named-field
  Rust — the cross-validation story gets worse, not better.

**Recommendation: (a)** — the node, the values, and the honest
fallout. It is the only design under which "field equations over a
record output" (Phase 48's unencodable class) become encodable,
which is the gate's actual requirement.

---

...[truncated 27053 chars]
## 3. Feature 3 — Record literals

### 3.1 The concrete need

The 9 tests that live only in the mirror golden (original file,
`acda3f9`): `peek` ×2, `next_char` ×1, `skip_whitespace` ×2,
`tokenize_one` ×2, `tokenize_all` ×2. Their literal shapes, exactly:

- `TokenizerState` literals, named-field form:
  `{ source: "hi", pos: 0, line: 1, col: 1, tokens: [] }` — as test
  inputs *and* as expected outputs (`next_char`, `skip_whitespace`).
- `Token` literals **nested inside list literals**:
  `{ kind: 0, lexeme: "spec", line: 1, col: 1 }`.
- Literals wrapped in constructor calls: `Ok({ … })`,
  `expect: Some('h')` / `None()` (already expressible — calls).
- One level of nesting only; no literal omits a field; field order
  in the text matches declaration order (but nothing may depend on
  that — see D3).

The same construction is needed in **body position**: a synthesized
`next_char` body must build the output state. So the literal is a
general expression form, not test-only syntax sugar — its IR half
(`IRNode::RecordLit` or an equivalent construction node) is what
bodies are made of.

### 3.2 Current state

- **Parser**: `parse_primary` has no `{` expression arm — a record
  literal today is "Expected expression". List literals exist as the
  parser-generated `Call("List", …)`; tuples as
  `Call("__tuple__", …)`; those two precedents define how this
  codebase absorbs a literal form.
- **Typechecker**: `check_expr` is bottom-up with **no expected-type
  propagation** — nothing in the checker knows that a test's input
  slot wants a `TokenizerState`. The one place an expected type
  exists is test *binding* (`eval.rs::bind_test_env` knows each
  param's `TypeRef`). Nominal declarations to resolve against do
  exist (`TypeKind::Record`).
- **Interpreter**: `value_of_expr` maps literals to `Value`s and
  falls back to `expr_to_ir` + `eval` for composites — with no
  record `Value` (§2.1), a record literal could not bind as a test
  input even if it parsed. Feature 3's binding story rides on
  Feature 2's `Value::Record`.
- **Reference**: LANGUAGE_REFERENCE §14 documents record **type
  declarations** only; §1's literal table has no record row. **The
  literal syntax is reference-silent — an owner decision**, which is
  why D3 exists. (The reference's own future-notation in §14,
  `Some { value: T }` for ADT variants, is the closest thing to a
  house style.)
- **G2 interaction** (recorded, bounded): finding G2 is the Codegen
  specs' flat (kind, lit) codes being unable to see through
  `Typed` wrappers when deciding Match-scrutinee bareness — mirror-
  only by design. A record literal under a `Typed` wrapper (which is
  how typed bodies carry nodes) hits the same flat-code blindness in
  `rust_match_scrutinee_bare` (kind 22 unpinned). No new machinery is
  needed — the new kinds join the recorded G2 pattern — but the
  audit states it so the fallout list in §4 is complete. Record
  **patterns** (matching against a record literal) are needed by no
  spec in the module — `expr_to_pattern` gains no arm; explicitly
  out of scope.

### 3.3 Candidate designs

- **(a) Named construction: `TokenizerState { source: "hi", … }`.**
  The type name is part of the syntax (the §14 future-notation's
  shape; Rust's struct-literal shape). Parser: identifier followed
  by `{` in expression position → `Expr::RecordLit { type_name,
  fields }`. Typechecker: resolve the name to a declared
  `TypeKind::Record`; the field set must match exactly (missing /
  extra / duplicate = error naming the field); each field expression
  is checked against its declared type at the checker's standing
  coarse precision. No expected-type machinery anywhere; the literal
  is self-describing in renders, diagnostics, and Z3
  counterexamples. *Cost, stated plainly:* the tokenizer's 9 tests
  are re-authored from the bare form to the named form — a FORM
  change of exactly the Phase 47 kind, semantics preserved, recorded
  in the file header.
- **(b) Bare literals + expected-type inference:
  `{ source: "hi", … }`** — the original file's actual syntax, so
  the 9 tests restore **verbatim** with zero re-authoring.
  *Trade-offs:* the parser produces an untyped node; the checker
  must grow expected-type propagation (test input → param type;
  `expect` → return type; constraint sides → the compared side's
  type; through `Ok(...)` → the constructor's type argument), a
  mechanism that exists nowhere today, with real ambiguity corners
  (the empty literal `{}`; two declared records sharing a field
  set; a literal in a position with no expected type). Every one of
  those corners becomes either an error rule or a silent guess.
- **(c) Positional construction: `TokenizerState("hi", 0, 1, 1, [])`**
  reusing `Call`. Zero new syntax, zero new AST. *Trade-offs:*
  order-dependent and unreadable at five fields; reordering a
  type's fields silently rewires every literal; the original
  author's named form is direct evidence of the house preference.

**Recommendation: (a)** — named construction. The re-authoring cost
is one file's header note; the inference machinery of (b) is a
permanent tax on the checker to save it.

---

## 4. The fallout, priced once, honestly

Two new `Expr` variants and (under D2(a)) two new `IRNode` variants
touch every layer that matches exhaustively. The precedents are
F4 (Phase 45: one variant, arms only, one phase) and Phase 55/62
(spec amendments + re-synthesis + artifact regen + gate cascade).
The specific bill:

- **Kind codes.** `examples/self_host/codegen.morph` pins
  `SpecExprKindCode` 0–12 (declaration order of `Expr`) and IR kind
  codes 0–27. F4's precedent is instructive: `CharLit` took **no**
  kind code — it got a dedicated arm in both emitter glues
  (`m3_gate.rs:1044`, `bootstrap_adapter.rs:431`) because its
  rendering is a per-char transformation (F5-inexpressible).
  Method/record rendering is the opposite case — templates over
  `++`, exactly what the spec language *can* state — so the
  Phase 55 route applies: **append** codes 13 = MethodCall,
  14 = RecordLit (spec-Expr) and 28 = FieldAccess, 29 = RecordLit
  (IR) at the ends of the tables; **never renumber** (renumbering
  rewrites every pinned row in the module). This constrains
  Phase 73: the new `Expr` variants are appended at the **end** of
  the enum, not inserted near their kin.
- **Native matches.** Thirteen files hold matches over the 28 IR
  variants (`synthesis/eval.rs`, `selector.rs`,
  `verify/verifier.rs` + `verify/encodable.rs`, `codegen/mod.rs`,
  `synthesis/engine.rs`, `synthesis/templates.rs`,
  `codegen/m3_gate.rs`, `m4_harness.rs`,
  `codegen/bootstrap_adapter.rs`, `synthesis/runner.rs`,
  `llm/prompt.rs`, `api.rs`), at least six exhaustive or
  `unreachable!`-guarded — rustc enumerates most of the bill, which
  is the good news; the wildcard sites (fingerprints, estimators,
  template walks) must be audited by hand, which is the real work.
  The `verify/encodable.rs` pure mirror must gain the new arms **in
  lockstep** — its cross-check test against the Z3 predicate is the
  evidence, and it is also what keeps the browser build's
  encodability answers honest.
- **The synthesized artifact.** `src/codegen/synthesized_codegen.rs`
  (111,558 B at v0.6.0) is generated from the Codegen specs; new
  kind rows mean spec amendments in `codegen.morph`, re-synthesis
  of the affected specs, artifact regeneration, adapter arms, and
  the full cascade re-run: **M3 gate (2,573-case corpus), M4 gate
  (143/143), M5 S1/S2 equivalence re-proofs** — the Phase 56/62
  pattern, priced as its own phase (Phase 78 below), not folded
  into any other.
- **Emitter freeze.** The byte-frozen surface is the **Rust
  emitter** (`expr_to_rust` + the `ir_to_rust*` family in
  `src/codegen/mod.rs`), pinned by the Codegen specs and the M3
  corpus. Rendering the new kinds there is a **scoped unfreeze of
  the Phase 55 kind**: new arms only, no existing byte moves, and
  the corpus re-run is the proof. It needs the owner's scoped
  approval at Phase 77 exactly as Phases 29/39/55/62 did. The C /
  WAT / Python / JavaScript emitters are not under a byte gate;
  they gain the new nodes under their existing conventions and are
  re-measured through the playground corpus. One adjacent need,
  flagged now: generated code that *uses* record values needs the
  record **type declarations** rendered into generated modules —
  today the runner prelude supplies the tokenizer's type shapes
  (Phase 48); whether that prelude mechanism is promoted into the
  emitter is a Phase 77 design question, not an assumption.

---

## 5. The entangled fourth decision — and the sufficiency finding

### 5.1 Token columns: START vs END (decision D4)

Pinned on both sides since Phase 47, decided by neither: the spec's
tests stamp each token with the line/col where the token **STARTS**
(`tokenize_one` "keyword-spec": `spec` at col 1; `tokenize_all`
"simple-spec": `sort` at col 6 — its first character); the M0
hand-written tokenizer stamps the position **AFTER** the token (its
`push_token`, `tests/tokenizer_proof.rs:79`, reads the
already-advanced state — `spec` would stamp col 5). The mirrors
follow the spec; a dedicated test in `tests/tokenizer_proof.rs`
(line ~209) pins the divergence itself. The choice is entangled with
everything in this campaign: the M1 golden suite is built from
whichever side is normative, and choosing later re-opens the golden.

- **(a) START is normative** *(recommended)*. The spec is the
  contract every other artifact is judged against — the standing
  M2/M3 discipline (mirrors follow the spec, never the reverse).
  The hand-written tokenizer's `push_token` is corrected to stamp
  the pre-advance position; the mirror already carries the
  START-stamping helper (line ~262), so the correction is small and
  the divergence test flips to an agreement test. START is also the
  industry norm for token positions.
- **(b) END is normative.** The spec's tests and the mirrors are
  re-authored to END stamps; the spec bends to the M0 artifact.
  Defensible only if the M0 tokenizer's behavior is itself a
  requirement — nothing in the record says it is; it was a proof
  artifact (M0 Phase B), written before the spec existed as a
  contract.

### 5.2 The sufficiency finding (decision D5)

**The three features do not, by themselves, let the M1 gate pass as
the owner stated it.** The gate's "Z3 verified" must cover the whole
module, and the three Char classifiers' constraints are Char
equations: Char params bind `Sym::Opaque`, Char literals encode
Opaque, and `sym_binop` has no (comparison, Opaque, Opaque) arm —
the Phase 49 record states the family's verdicts are "Inconclusive
by construction". Features 1–3 change none of that. The owner's
deferral text names the unacceptable compromise explicitly —
"accepting executed agreement in place of Z3 verification for the
Char classifiers" — so this audit may not quietly re-scope the
gate. The honest options:

- **(a) Add a Z3 Char encoding as a fourth workstream**
  *(recommended — the only route to the owner's stated standard).*
  Encode Char as its Unicode scalar value on the Int sort, with the
  range side-condition (0 ≤ c ≤ 0x10FFFF, excluding the surrogate
  gap) asserted where Char params bind. Comparisons — the only
  operations the classifiers perform — encode directly. Fallout,
  stated now: the `char_equation` seed family's reason to exist
  shrinks (its admission gate exists *because* Char is
  unencodable; the ordinary equation family's gate would begin
  admitting the classifiers) — the families' recorded meanings must
  be re-stated, not silently merged (the Phase 49 disjointness
  note); the classifiers' standing 774/774 executed-agreement
  result must be re-measured, not assumed carried; and the F4
  caveat (a wildcard completion crowned for lack of a Z3 backstop)
  is exactly what this encoding retires.
- **(b) Ship the three features; leave M1 open on the Char clause
  alone.** Honest and smaller — five of eight specs become
  gate-eligible; the gate stays formally open with a one-line
  recorded reason instead of three. The owner rejected this
  *shape* in 2026-11-21 (deferral "so the gate can pass in full"),
  so it is listed for completeness, not recommended.

### 5.3 The encodability chain (what "Z3 verified" actually requires, per spec)

So the owner sees the true shape of Phase 76/80, the verification
needs of the five non-Char specs, traced through the encoder as it
will stand after Features 1–3:

| Spec | Needs | Status after F1–F3 (+D5 where marked) |
|---|---|---|
| `peek` | record projection; `Option<Char>` construction + equality; **String index at a symbolic position yielding a Char** | Option rides the Result tag+payload machinery (D1). **The String-index→Char bridge is the campaign's top technical risk**: Z3's string sort can express indexing, but bridging its result to a scalar-Int Char (D5) is new encoder work with no precedent in the codebase; if it does not land, `peek` stays Inconclusive and the gate re-run must say so. |
| `next_char` | record projection + record construction + equality; a spec-to-spec call (`peek(state)`) inside constraints — Phase 19 inlining, which extends once the callee's body is encodable | Encodable in principle; gated on `peek`'s body being synthesizable + encodable |
| `skip_whitespace` | record projection; String length | Encodable; its *behavioral* content was always thin (two postconditions) — verification will be exactly as strong as what the spec states |
| `tokenize_one` | Result methods (D1); record projection through `unwrap()`; List length | Encodable under D1(a) + D2(a) |
| `tokenize_all` | Result methods (D1); `unwrap().len()` | Encodable under D1(a) + D2(a) |
| classifiers ×3 | Char comparisons | Encodable **only** under D5(a) |

And the standing caveat no feature removes: encodability is
necessary, not sufficient — synthesis must still *find* the exact
bodies. Phase 48's pools held no exact body for any record spec;
whether the constraint seeds these features unlock (equation /
guard families over now-lowerable constraints) produce them is
what Phases 79–80 measure. This audit promises the *ability* to
judge; the gate re-run reports what was judged.

---

## 6. Owner decisions (ratify by letter)

- **D1 — Method calls.** (a) Native `Expr::MethodCall` + closed
  builtin table — `Result::{is_ok, is_err, unwrap}`, `List::len` —
  UFCS recorded as the future extension. **[RECOMMENDED]**
  (b) Parse-time desugar to invented builtin calls.
  (c) General UFCS resolution now.
- **D2 — IR field access.** (a) `IRNode::FieldAccess` +
  `Value::Record` + `Sym::Record`, typechecker resolves declared
  fields. **[RECOMMENDED]**
  (b) Desugar to a `__field` builtin call (values still required).
  (c) Records-as-tuples positional lowering.
- **D3 — Record literals.** (a) Named construction
  `Type { field: value, … }`; the 9 tests re-authored as a recorded
  FORM change. **[RECOMMENDED]**
  (b) Bare `{ field: value, … }` + expected-type inference; the
  9 tests restore verbatim.
  (c) Positional constructor calls.
- **D4 — Token columns.** (a) START normative; the hand-written
  tokenizer's `push_token` is corrected; the pin flips to
  agreement. **[RECOMMENDED]**
  (b) END normative; spec tests + mirrors re-authored.
- **D5 — The Char clause (sufficiency).** (a) Add the Z3 Char
  scalar encoding as a fourth workstream in this campaign.
  **[RECOMMENDED — the only route to the gate as stated]**
  (b) Ship Features 1–3; M1 stays open on the Char clause alone.

---

## 7. Implementation plan (proposed Phases 73–80; sizes are relative — S/M/L/XL — calibrated to prior art: F4 was one phase for one variant across all layers)

**Phase 73 — AST + parser (M).** Append `MethodCall` and `RecordLit`
at the **end** of the `Expr` enum (§4's kind-code constraint);
restructure the parser's postfix handling so chains compose after
calls and parenthesized expressions (`output.unwrap().tokens` is
the acceptance shape); parse the D1/D3 syntaxes; `expr_to_ir` gains
`Field`, `Index`, `MethodCall`, `RecordLit` lowerings (the IR nodes
themselves land in Phase 75/76's terms — this phase lowers to them
or the phase order swaps 73↔75 for the IR enum; recorded as a
sequencing detail for the phase brief, not a decision). Every
exhaustive `Expr` match in the compiler gains its arms
(`llm/prompt.rs` formatter, the parser's own walks,
`expr_mentions_var` / `expr_calls_indexed` / `expr_same`,
`expr_to_pattern` — no record-pattern arm, §3.2).
*Evidence:* full suite green; in a **scratch copy** (the example file
is not touched yet), the three dropped constraints and the 9 mirror
tests parse. *Risk:* parser regression across every corpus — the
suite + all module checks are the net.

**Phase 74 — Typechecker (M).** Method resolution against the D1
table (receiver-typed; unknown method = error naming it); the
`Field` arm resolves declared field types for module-declared
records and defers otherwise (§2.3); `RecordLit` field-set checking
(exact set, per-field coarse type check). Negative tests pin each
new error. *Evidence:* the scratch tokenizer module — constraints
restored, tests restored — passes `morphic check`; every standing
gate corpus still checks (the Field tightening measured, not
assumed). *Risk:* the Field tightening rejects a spec somewhere in
the corpora — if it does, that is a finding to record, not a check
to loosen silently.

**Phase 75 — Interpreter values (L).** `Value::Record` + the
`FieldAccess` / record-construction evaluation arms +
`value_of_expr` / `bind_test_env` for record literals, so the 9
tests **bind in-process** for the first time. Scoped sub-item,
priced separately inside the phase: `Value::Option` / `Value::Result`
+ constructor-pattern matching in `Match` (today `Ok(None)`) +
`TryPropagate` semantics — needed for bodies returning
`Option`/`Result` to execute in-process at all (e.g. `peek`'s
expected outputs binding as values, not just its inputs).
*Evidence:* the record specs execute on the interpreter route in
the tokenizer probe; the compiled-runner route remains as judge
where the interpreter still cannot go, and the report says which
route judged what. *Risk:* value-equality and rendering of records
in test output (the Phase 54 lesson: test-value rendering is a
spec-pinned surface — new renderings are additive only).

**Phase 76 — Z3 encodings (XL — the campaign's deep phase).**
`Sym::Record` + per-field param binding; `Field` projection in
`encode_expr` and the `FieldAccess` arm in `sym_eval`; record
equality fieldwise; the Result/Option tag+payload form + the D1
method arms (`is_ok`/`is_err`/`unwrap`, `len`); `encodable.rs`
mirror arms in lockstep; **and, if D5(a) is ratified,** the Char
scalar encoding (param binding + literal/comparison arms + the
range side-condition) with the `char_equation`/equation family
meanings re-stated in the ROAD. The String-index→Char bridge
(§5.3) is attempted inside this phase and reported either way.
*Evidence:* the encodability cross-check test green; a probe shows
the five record specs' constraints **encode** (non-`None`) against
hand-written exact bodies — encodability proven before synthesis
is asked to find anything. *Risk:* the highest in the campaign;
Z3 string theory is the one place the encoder has no precedent.

**Phase 77 — Native emitters (M; ⚠ SCOPED UNFREEZE).**
`expr_to_rust` arms for `MethodCall` (verbatim Rust method syntax)
and `RecordLit` (Rust struct literal); the five target emitters
gain `FieldAccess` / record-construction arms under their existing
conventions; the record-type-declaration question (§4) decided and
implemented (prelude promotion or emitter-rendered declarations).
**This phase needs the owner's scoped unfreeze approval for the
Rust emitter, in the Phase 29/39/55/62 form: new arms only, no
existing byte moves.** *Evidence:* the M3 corpus re-run shows zero
byte movement on pre-existing kinds; generated tokenizer test
modules compile (the Phase 48 rustc failures are the regression
baseline). 

**Phase 78 — Codegen-spec fallout + bootstrap cascade (L).**
`codegen.morph` amendments: kind codes appended (13/14 spec-Expr,
28/29 IR); rendering specs for the new kinds (the Phase 55 family
pattern); re-synthesis of the affected Codegen specs; artifact
regeneration; adapter arms; **M3 gate re-run (2,573-case corpus),
M4 gate re-run (143/143), M5 S1/S2 equivalence re-proofs**;
playground corpus re-run (13/13 + 5/5) and bundle re-measured
against the 673,792 B budget. *Evidence:* every gate reproduces;
any byte that moves outside the new-kind rows fails the phase.
*Risk:* re-synthesis draws differ run to run — the cascade follows
the standing seeded/draw discipline of Phases 56/62.

**Phase 79 — Tokenizer completion (M).** The example file is
re-authored for the last time in this campaign: `tokenize_one` /
`tokenize_all` constraints restored from the original text (under
the ratified D1 syntax); the 9 tests restored in-spec (named or
bare per D3); the header's re-authoring record extended; D4 applied
(the divergence pin flips to whichever agreement the owner chose).
The S4 measurement re-runs (probe + cross-validation, the Phase 48
instruments). *Evidence:* the per-spec table against the Phase 48
baseline — constraint seeds reaching pools, bodies compiling,
verdicts non-vacuous where constraints exist. 

**Phase 80 — M1 gate re-run (M).** The ROAD M1 checklist executed
as written: spec ✓ (already), golden suite, pipeline run,
cross-validation of the generated tokenizer against the Rust
original, benchmark ≤ 2×, and the gate line — **100% golden pass +
Z3 verified** — ticked or not, with the verdict recorded either
way and any residual (the §5.3 bridge, a spec whose exact body
synthesis still cannot find) named in the gate note, not smoothed
over. *Evidence:* the gate note itself.

**Total shape:** eight phases, one of them XL (76), two L (75, 78),
one gated on a scoped unfreeze (77). For calibration: F4 — one
literal kind, most layers already carrying the value — took one
phase; this campaign adds two AST variants, two IR variants, three
new value forms, two new Z3 forms, and a bootstrap cascade.

---

## 8. What does NOT change

- **Released tags** `v0.5.0` and `v0.6.0` and their artifacts —
  immutable; this campaign is post-v0.6 work on master.
- **The recorded gate results** for M2, M3, M4, M5 and W1–W5. The
  re-runs in Phases 77–78 must *reproduce* them; a re-run that
  moves a pre-existing byte or verdict is a phase failure to fix,
  never a gate note to edit.
- **The playground's honesty contract** (v0.6 D2): browser verdicts
  remain `Inconclusive`; the new encodings are native-Z3 only. The
  `encodable.rs` mirror keeps the browser build's encodability
  answers in lockstep with native — agreement of the *predicate*,
  not a browser verifier.
- **The Char classifiers' standing result** (774/774 executed
  agreement) — re-measured under D5(a), never silently carried.
- **The emitter freeze** outside Phase 77's scoped approval, and
  the freeze's surviving scope everywhere else.
- **The mirror discipline**: mirrors follow the spec; the one
  deliberate exception is D4 itself, which is an owner decision
  about which artifact is normative — taken openly, in §5.1,
  before any file moves.
- **No spec file is re-authored by this audit.** The tokenizer
  restorations happen in Phase 79, under ratified decisions, with
  the file's header record extended — the Phase 47 pattern.

*End of audit. Decisions D1–D5 await the owner's letters; the phase
briefs for 73–80 are written from this document once they land.*

---

**Ratified 2026-11-22.** The owner accepted all five recommendations:
**D1a**, **D2a**, **D3a**, **D4a**, **D5a** (see §6 for the lettered
options). The phase briefs for 73–80 are written from this document
under those decisions; Phase 77's scoped Rust-emitter unfreeze still
requires its own owner approval when that phase opens, in the
Phase 29/39/55/62 form.
