# 🔍 F2 Design — Int→Decimal-Text Conversion as a Language Feature

**Phase 99 (2026-11-22), branch `docs/f2-audit` from master @
`8de4a72`. MEASUREMENT + DESIGN ONLY: no production code
changed, no spec amended, `examples/functional_tokenizer.morph`
untouched. The only code is five scratch probe tests in
`src/verify/verifier.rs`'s test module, labeled Phase 99 —
they pin current behavior and the proposed encoder arm's
measured properties; the build phases below decide whether
they are promoted, restated, or dropped. Every claim about
current behavior carries a file/line anchor or a probe.**

**Placement choice (recorded):** standalone document, the
`M1_FEATURES_AUDIT.md` (Phase 72) and
`ENCODER_EXTENSION_DESIGN.md` (Phase 90) precedent. The ROAD
carries the Phase 99 note and a pointer; this document
carries the evidence, the candidate forms with prices, the
recommended form, the priced build plan, and the owner
decisions the plan depends on.

---

## 0. What this design is for

Gate re-run #3 (Phase 97) closed the encoder-extension
campaign with the M1 gate **open on exactly one sliver**:
the tokenize pair's executed agreement caps at **9/10**
(`tokenize_one`) and **7/8** (`tokenize_all`) against the
hand-written mirrors, and the sole deviation at both caps
is the **Err message text**. The mirrors build
`Unexpected character '<c>' at line <n>`; every synthesized
body builds the constant `"Unexpected character"`, because
Morphic has **no Int→decimal-text conversion** (finding
**F2**, open since the M3 campaign). The char part of the
message is already expressible (an indexed char reaches a
String through Append); only the line number is not.

The owner decided on 2026-11-22: **build F2 as a real
language feature** — close the gate by meeting it, not by
amending its text. The encoder-extension stack merged
(Phase 98, PR #9); this audit is the F2 campaign's first
phase. Its job: determine the smallest sound form of the
conversion, price each candidate form with its cascade
consequences, pin the semantics (including the Z3
negative-number divergence) with probes, and bring back a
ratifiable decision set.

**Headline findings, up front:**

1. **The payoff site is exactly one construction.** The
   shared `tokenize_step_body` builder
   (`src/synthesis/engine.rs`, the template seed of
   Phase 85 and — since Phase 92 — the resolver denotation
   for `tokenize_one`, one shared function) builds the
   constant message at engine.rs:5456–5466. No other body,
   spec, or seed needs the conversion for the gate (§1).
2. **The method-call form cannot reach the payoff site.**
   Spec `Expr::MethodCall` has no IR kind —
   `expr_to_ir` lowers it to a Hole by design
   (engine.rs:7339–7349, pinned) — and synthesized bodies
   are IR. A D1a-table method would serve spec expressions
   only. The plain **builtin function call** is the only
   candidate that reaches a body without a new node (§3).
3. **The exact Z3 arm is blocked by the pinned z3 crate,
   and it does not matter for the gate.** SMT-LIB's
   `int.to.str` exists in the bundled solver and its
   sign-split form expresses Rust's semantics exactly
   (probe 1) — but the z3 crate 0.20 safe API exposes no
   term constructor for it, and the raw context handle
   the FFI would need is crate-private (§2.2). The sound
   arm available today — **literal fold + uninterpreted
   function (UF) for symbolic arguments** — is measured
   sufficient: `tokenize_one`'s real rows never read the
   Err payload, and with a UF-built payload over a free
   line constant both rows still **Prove** at the
   faithful Err point, while row 2 still **Refutes** an
   unlicensed Err (probe 3). The UF's completeness cost
   is named and bounded (probe 4).
4. **The recommended form has no bootstrap cascade.**
   A builtin call reuses `Expr::Call` / `IRNode::Call`:
   no AST/IR kind changes, so the Codegen module's
   kind-code tables do not move, its 129 specs stand,
   the synthesized artifact stands, and M3/M4 do not
   re-run. The build is **S–M + S + S ≈ M** in total
   (§5–§6).

---

## 1. The exact payoff site (measured)

**The mirror's format.** `tests/tokenizer_proof.rs:129`
(the hand-written `tokenize_one`, the M1 cross-validation
original):

```rust
Some(other) => Err(format!("Unexpected character '{}' at line {}", other, state.line)),
```

Format facts, read off the construction: the literal
prefix `Unexpected character '` (one space, one single
quote), the offending char interpolated bare, then
`' at line ` (single quote, space, `at`, space, `line`,
space), then the line as Rust `Display` for the integer
type — decimal, no padding, no leading zeros, no `+`.
`state` at that point is the **post-skip** state (the
function binds `let state = skip_whitespace(state);` at
its head, tokenizer_proof.rs:106), and `line` is the
record's 1-based line counter (states are constructed
with `line: 1`; `next_char` maintains it). The battery's
canonical instance: `"Unexpected character '5' at line 1"`
(tests/tokenizer_proof.rs:591; the same string is the
cross-validation harness's expectation,
src/tokenizer_cross_validation.rs:2386, and its mirror
transcription at :342).

**The body today.** `tokenize_step_body`
(src/synthesis/engine.rs:5213ff; the STEP construction
extracted in Phase 92 and shared verbatim between the
`tokenize_template` seed family and the resolver's
`tokenize_one` denotation) builds its error arm at
engine.rs:5456–5466:

```rust
IRNode::Alloc { name: "msg", ty: TypeRef::String,
                initial: lit_s("Unexpected character") },
IRNode::Return(Call("Err", [Var("msg")]))
```

The builder's own comment (engine.rs:5563–5571) names the
deviation and its cause: a Char reaches a String only
through Append, and the line number is an Int, "whose
conversion to text is finding F2". A grep for the constant
across `src/` returns exactly one production construction
site (engine.rs:5462); every other hit is a test, a
harness mirror, or a benchmark/profile transcription of
the mirror itself.

**What the body becomes with a conversion in hand.** The
dispatch already binds `s` = the post-skip state and
`c` = `s.source[s.pos]` (engine.rs:5494–5530) — the same
`s.line` the token stamps already read (D4a). The error
arm becomes, in IR shapes that all exist today:

```
Alloc msg: String = "Unexpected character '"
Append(msg, c)                        // statement position: push(char)
Assign msg = Concat(msg, "' at line ")
Assign msg = Concat(msg, int_to_str(s.line))   // ← the one new call
Return Err(msg)
```

Every non-conversion step is a standing shape with
standing semantics in all three judges: statement-position
Append(Str, Char) is the Phase 95 interpreter arm
(eval.rs:854–867) and the encoder's `stmt_append`
(verifier.rs:2738ff, the (Str, CharStr) form); Concat
(Str, Str) is sym_binop's arm (verifier.rs:2036), the
interpreter's arm (eval.rs:845–850), and the Rust
emitter's Phase 29 ownership-treated `+`. The line read
is a FieldAccess on the post-skip state — the mirror's
`state.line`, 1-based by the state's construction and
maintenance, unchanged.

**No other body or spec needs the conversion for the
gate.** The other six specs' outputs contain no
number-bearing text (classifiers return Bool; peek a
Char; next_char/skip return states whose fields are
carried, never rendered). `tokenize_all` has no message
construction of its own: its LOOP body propagates the
STEP's Err through TryPropagate (engine.rs:5659ff), so
fixing the STEP construction fixes both caps. The
Codegen module's specs likewise state no conversion —
the numeric-Display conversions its pipeline needs live
in the M5 bootstrap adapter as hand-written Rust shim
code (ROAD M5 record, items (4)/(7): the adapter performs
them with Rust's own `to_string()`/Display), outside the
M1 gate and untouched by this feature; re-expressing
them in Morphic is a future owner decision, not priced
here.

---

## 2. Semantics

### 2.1 The reference: Rust `Display` for the integer value

The compiled mirrors are the ground truth (the M1 gate
cross-validates against them), and the mirror's
interpolation is `format!("{}", n)`. Probe 5
(`phase99_probe_rust_display_reference`) pins the table
the interpreter and every emitter must reproduce:

| n | rendering |
|---|---|
| 0 | `"0"` |
| 7 | `"7"` |
| −1 | `"-1"` |
| −42 | `"-42"` |
| 1,000,000 | `"1000000"` |
| i64::MAX | `"9223372036854775807"` |
| i64::MIN | `"-9223372036854775808"` |

Decimal, no grouping, no leading zeros, no `+`, a single
leading `-` for negatives — including the i64::MIN
corner, whose magnitude is not representable as a
positive i64 (any build that negates-then-renders must
not compute `-n` in i64; Rust's own `format!` is the
implementation at every concrete layer, so this is a
test-table obligation, not an implementation hazard).

One pre-existing doc/impl gap, recorded so the build
does not silently inherit it: LANGUAGE_REFERENCE §4
calls `Int` "arbitrary-precision", but every
implementation layer is i64 — `IRLiteral::Int(i64)`
(engine.rs:383), `Value::Int(i64)` in the interpreter,
`Expr::IntLit(i64)` (ast.rs:206). The conversion's
domain is therefore the i64 value domain, exactly like
every other Int operation today. (The Z3 side binds Int
parameters as mathematical integers; §2.3 states what
that means for the arm.)

### 2.2 What Z3 can encode — measured

SMT-LIB's string theory has the operator: `int.to.str`
(theory Strings, §4.9 in the cvc5/Z3 lineage —
`str.from_int` in the SMT-LIB 2.6/2.7 text is the same
function under its newer name). Probe 1
(`phase99_probe_int_to_str_smtlib_semantics`) measures
the bundled solver's ground semantics through
`Solver::from_string` — the only reachable surface, see
below:

- `int.to.str(0) = "0"`, `(42) = "42"`,
  `(1000000) = "1000000"` — SAT, Rust-identical;
- `int.to.str(42) = "042"` — UNSAT (no leading zeros);
- **`int.to.str(-42) = ""` — SAT; `int.to.str(-42) =
  "-42"` — UNSAT.** The SMT-LIB definition maps every
  negative to the **empty string**. This diverges from
  Rust's Display on the entire negative half-line.
- The **sign-split formula** `ite(n < 0, "-" ++
  int.to.str(-n), int.to.str(n))` at ground points
  −42 and 7 yields `"-42"` and `"7"` — SAT. So the
  solver *can* decide the exact Rust conversion,
  negatives included, **if** the term can be built.

**The blocker is term construction, not the solver.**
Source-checked against the pinned crates:

- The **z3 crate 0.20** safe API (Cargo.lock:2586–2589)
  exposes no conversion constructor: `ast/string.rs`
  offers `at`, `substr`, `concat`, regex and order
  operations — no `int_to_str`/`str_to_int` — and
  `ast/int.rs` offers arithmetic and real/bv
  conversions only.
- **z3-sys 0.11** (the transitive FFI crate) does bind
  the symbol: `Z3_mk_int_to_str`
  (z3-sys-0.11.0/src/generated/functions.rs:1680), and
  `Z3_mk_str_to_int` beside it (:1678). Calling it
  needs the raw `Z3_context` of the terms it must
  compose with. Morphic's verifier builds every term
  in the z3 crate's **thread-local** context; that
  context's handle is `pub(crate)` inside the z3 crate
  (`Context { pub(crate) z3_ctx }`, z3-0.20.0
  src/context.rs:47ff; `ContextInternal` likewise
  `pub(crate)`). This is the same wall the Phase 76
  CharStr doc records for `str.to_code`
  (verifier.rs:1027–1032: "the raw context handle the
  FFI would need is not public"). **Adding z3-sys as a
  direct dependency does not unlock it** — a second,
  raw context's ASTs cannot mix with the thread-local
  context's terms.
- The remaining exact routes are out of scope for this
  campaign, named for the record: upgrading the z3
  crate to a version whose safe API wraps the operator
  (unmeasured here; a dependency migration with its
  own regression surface), or an SMT-LIB text
  round-trip per conversion term (parse a solver from
  a string, extract the term — fragile, context-bound,
  rejected).

### 2.3 The proposed arm: literal fold + uninterpreted function

The arm the build can land today, in `sym_call`'s shape
(the `len` precedent, verifier.rs:1761) and its
`encode_expr` twin (verifier.rs:4562ff):

- **Literal fold.** When the argument encodes to a
  `Sym::Int` whose value is a known i64 (`as_i64()`),
  the result is `Sym::Str` of the Rust rendering —
  computed by the same `format!("{}", n)` the mirrors
  use. Exact, negatives and extremes included.
- **UF fallback.** Otherwise the result is an
  application of a declared uninterpreted function
  `int_to_str : Int → String` (`z3::FuncDecl`, safe
  API). Z3's congruence gives the one property the arm
  needs for free: equal input terms yield equal output
  terms, everywhere the term appears.

**Soundness.** A UF is *some* function Int→String. A
row Proven under the UF is proven for **every**
interpretation of the symbol, hence for the real
conversion — Proven verdicts are never bought.
**Completeness cost, named:** a row that *reads* the
converted text (compares it, measures it) cannot be
Proven through a UF term — the solver is free to
interpret it otherwise. Probe 4
(`phase99_probe_uf_completeness_boundary`) measures
exactly this: a scratch row `output == Err("Unexpected
character '5' at line 1")` is **not** provable when the
payload is a UF application (even at the literal
argument 1 — pc ∧ ¬row is SAT), and **is** provable
when the payload is the literal fold (UNSAT). The cost
lands only where a spec states text-level facts about
a conversion of a *symbolic* value; no standing spec
does (the tokenize rows read tags, lengths, and state
fields — §2.4).

**Refutation discipline (a build obligation, not an
option).** Symmetrically, a **Refuted** whose
countermodel assigns the UF is untrustworthy — the
model may be refuting a function the real conversion
isn't. The Phase 97 per-disjunct precedent (the W8
recovery *never Refutes*) is the house rule for
exactly this shape. The build must therefore degrade:
when a judged encoding contains the UF symbol, a
VerificationFailed is reported as Inconclusive
**unless** the countermodel's UF applications all
agree with the real conversion at the model's concrete
arguments (a checkable condition — evaluate the model,
compare `format!`). For the gate this rule never fires:
neither tokenize crown's rows read the payload, so no
countermodel over them assigns the UF a load-bearing
value (probe 3's teeth, below). The rule is priced
into Phase 100 because the feature is general-purpose,
not tokenizer-only.

**Domain note.** The UF ranges over Z3's mathematical
integers; the language's values are i64 (§2.1). A
payload-reading row over a beyond-i64 value would be
reasoning about a value no execution can produce —
the same over-approximation the encoder already makes
for Int parameters everywhere (they bind as unbounded
Z3 Ints today). No new gap is introduced.

### 2.4 Does the STEP body still Verify? — probed

Probe 3 (`phase99_probe_uf_arm_rows_still_decide`)
constructs the Err output **exactly as the post-F2
body builds it** — literal prefix `"Unexpected
character '5' at line "` (the probe's point fixes the
char) concatenated with a UF application over a **free**
line constant — and judges `tokenize_one`'s **real
rows** (the examples module as amended through
Phase 96) through production `verify_constraint`:

- At the faithful Err point (state `" 5"`, pos 0):
  **row 1 Proven, row 2 Proven.** Row 1 decides on
  `is_err()`; row 2 on the β-skip license over the
  input state. The payload term blocks nothing.
- Teeth: the same UF-payload Err at `"spec 5"` (ident
  start at the post-skip position — no license) is
  **Refuted by row 2**, as it must be: the UF buys no
  admission.

The body-level reading: Phase 97's Verified STEP
verdict was earned over a body whose message was a
plain literal. The post-F2 body differs only in the
`msg` construction; its Err outcome is the value probe
3 judges, and its Ok outcomes are untouched. So the
STEP composition is expected to **hold Verified** with
the arm in place — stated as a probe-backed expectation
whose body-level re-measurement is Phase 101's gate,
not as a claim this audit makes. Probe 2
(`phase99_probe_encoders_decline_conversion_today`)
pins the starting point: today both encoders decline
a conversion call outright (`sym_eval` → None,
`encode_expr` → None), which is why the constant
message — a literal — was the only buildable form.

### 2.5 The concrete layers are exact by construction

Interpreter and emitters face no divergence: they
compute the rendering directly. The interpreter arm is
`Value::Int(n) → Value::Str(format!("{}", n))`
(eval.rs `eval_call`, the `len` arm at :668 is the
shape); the Rust emitter arm is `(n).to_string()`
(`ir_call_text`, codegen/mod.rs:987 — the `len` arm at
:992 is the shape). Negatives and i64 extremes are
exact because Rust's Display is the implementation,
not a re-implementation — probe 5's table is the test
obligation. C/Python/JS/WAT need no arm and get none:
their IR expression walkers have **no Call arm at
all** — every call, `len` and constructors included,
falls to the standing catch-all constant
(ir_to_c_expr → `"0"`, ir_to_python_expr → `None`,
ir_to_js_expr → `"undefined"`, ir_to_wasm_expr →
`i32.const 0`; codegen/mod.rs:1593ff, 1696ff, 1760ff,
1661ff). A conversion call inherits exactly that
stance — no worse than any builtin call today, and
the WAT stance stays the recorded loud fallback of
the Phase 77 precedent.

---

## 3. Candidate forms

| Form | New syntax/node | Price | Reaches the IR body? | Bootstrap cascade |
|---|---|---|---|---|
| **(a) Builtin function call** `int_to_str(x)` | none — `Expr::Call` / `IRNode::Call` | **S–M** | **yes** | **none** |
| (b) Method call `x.to_str()` via D1a table | none (spec side) | M | **no** — no IR kind | none, but gate-fatal |
| (c) New unary conversion node | new `IRUnaryOp` variant (+ a spec form) | **L** | yes | **full** (kind tables, Codegen specs, artifact, M3/M4) |
| (d) String interpolation `"…{x}…"` | lexer/parser syntax | **L** | only via desugar to (a) | (a)'s, plus parser surface |

### (a) Builtin function in the closed-call tradition — RECOMMENDED

A named conversion in the standard-library call
tradition of Appendix A (`len`, `sum`, `range` — the
names `stdlib_return_type` already keys on,
typeck.rs:173–185), callable in spec expressions as
`Expr::Call` and in bodies as `IRNode::Call`. No new
AST/IR node; the parser needs nothing (a call is
generic syntax, parser.rs's primary/postfix chain);
`expr_to_ir` lowers it generically (engine.rs:7303).

Layers gaining an arm — one each, all in the `len`
precedent's shape:

| Layer | Site | Arm |
|---|---|---|
| Typechecker | `stdlib_return_type` (typeck.rs:173) | `"int_to_str" → TypeRef::String` (one entry; the table's standing coarseness — it does not inspect argument types — is inherited, as `len` inherits it; arity is enforced by every layer below, as today) |
| Spec-test mapping | `value_of_expr` (eval.rs:986) | **none** — it routes `Expr::Call` through `expr_to_ir` → `eval_call`, so the interpreter arm covers it |
| Interpreter (IR) | `eval_call` (eval.rs:621) | `"int_to_str"` on one `Value::Int` → `Value::Str(format!)`; wrong arity/type is a named NotEvaluable, as `len`'s arm |
| Interpreter (statements) | Phase 95 subset | **none** — statement bodies evaluate calls through `eval_call` |
| Z3 encoder, body side | `sym_call` (verifier.rs:1750) | §2.3: literal fold, else UF |
| Z3 encoder, constraint side | `encode_expr` Call arm (verifier.rs:4562) | the same |
| Z3 statement judge | Phase 93 layer | **none** — value positions delegate to `sym_eval` (Phase 93 record) |
| Encodability mirror | `abs_call` (encodable.rs:607) | `(Int Known n) → Str Known(format!)`; `(Int _) → Str Unknown` — the `abs_len` shape |
| Rust emitter (IR) | `ir_call_text` (codegen/mod.rs:987) | `"int_to_str"` → `({}).to_string()` |
| Rust emitter (spec expr) | `expr_to_rust` (codegen/mod.rs:1954) | **none needed** — plain calls already render by name; a spec-side use in a test expectation would render a call to a helper the generated module does not define — the standing caveat for stdlib names in that walker, exercised by no corpus case (no spec states the conversion) |
| C / Python / JS / WAT | — | **none** — no Call arm exists; standing catch-all stance inherited (§2.5) |
| Resolver / seeds | — | **no seed family produces it** — no search family enumerates a builtin-call vocabulary a new name would silently join (calls in generated bodies come from module specs, constructors, and the hand builders). The one producer that changes is the hand-authored `tokenize_step_body` (§1), in Phase 101, not the feature phase |
| Docs | LANGUAGE_REFERENCE App. A | one row: `int_to_str : Int → String` |

**Bootstrap cascade: none.** No enum gains a variant,
so the Codegen module's kind-code tables
(examples/self_host/codegen.morph:83–123 — collection,
IR-node, literal, pattern, spec-expression, constraint
kinds) do not move; the module stays at **129 specs**;
the synthesized artifact
(src/codegen/synthesized_codegen.rs) stands; M3/M4 do
not re-run. The build still *verifies* the freeze
(Codegen probe 129/129 and the cross-validation
standings ride the suites) — evidence, not cascade.
Playground: the new arms are code in the WASM graph,
so the bundle grows by their bytes; Phase 98 measured
727,028 B against the 771,072 B budget (44,044 B of
headroom) — re-measured in Phase 101, no re-baseline
expected. One standing-behavior note: a user spec
*literally named* `int_to_str` would be shadowed by
the builtin in `sym_call`/`eval_call` — the identical
standing treatment of `len`, `Ok`, `Err` today; the
name is recorded as reserved-by-builtin in the
reference row.

### (b) Method-call form via the D1a table

`state.line.to_str()` — spec-side only. The D1a table
(typeck.rs:434ff) could gain `Int: to_str() -> String`
in one match arm, and `encode_expr`'s MethodCall arm
(verifier.rs:4640) and `eval_method_call` (eval.rs:686)
could judge it. **But bodies are IR, and the IR has no
method-call kind**: `expr_to_ir` lowers every
MethodCall to a Hole (engine.rs:7339–7349, a pinned
design fact since Phase 73). The template builder
constructs IRNodes by hand; there is no IR it could
build. Making (b) reach a body means adding an IR
kind — which is form (c)'s cascade wearing (b)'s
syntax. As a spec-only nicety it is also *unneeded*:
no row states a conversion. **Verdict: M-priced,
gate-fatal, rejected by measurement** — listed so the
rejection is on record, per the audit's obligation to
price every candidate.

### (c) New AST/IR node (unary conversion op)

`IRUnaryOp::IntToStr` appended at the end of its enum
(engine.rs:387ff), plus a spec-side form (a new
`UnaryOp` variant or a dedicated `Expr` kind —
either way a kind-code table grows). Every exhaustive
match on the op across interpreter, encoder,
encodable mirror, and five emitters gains an arm
(rustc-forced, the Phase 73 pattern). The Codegen
module synthesizes codegen *for the IR*: its specs
pin the kind tables, so a new kind is new Codegen
specs, a regenerated synthesized artifact, and the
M3/M4 re-runs — the Phase 78 cascade (123 → 129 specs
for two kinds) at single-kind scale. The Z3 problem
is **identical** to (a)'s (the arm is the same fold +
UF; a node buys no encoding). **Price L.** Buys:
nothing the gate needs that (a) does not. Rejected
on price alone.

### (d) String interpolation syntax

`"Unexpected character '{c}' at line {line}"` as
literal syntax. A lexer/parser change (string
literals are scanned as opaque text today;
interpolation braces would need escaping rules
against the record-literal brace grammar), then
either a new `Expr` kind — form (c)'s cascade — or a
parse-time desugar into Concat + Append +
`int_to_str(...)` calls — form (a)'s layers **plus**
the parser surface, the reference documentation, and
a new class of parse errors. **Price L.** It buys
sugar over (a) for a construction exactly one body
needs once. Rejected on price; recorded as a possible
future syntax decision for the owner, independent of
F2.

---

## 4. Scope line

**In: `Int` only** — the one conversion a measured
body needs, in the one form (a plain call) every layer
already knows how to carry.

**Out, explicitly:**

- **Float → text (F3).** A separate finding with its
  own divergences (Rust renders `1.0` as `"1"`; Z3's
  real→string path differs again; the M3 record pins
  the mirror behavior). F2's arm pattern does not
  transfer for free, and no gate body needs it. F3
  stays open, undecided, the owner's.
- **Char → String.** Already expressible where bodies
  need it: Append(Str, Char) in the interpreter and
  the CharStr bridge in the encoder are the lexeme
  shape the tokenizer itself uses. A conversion call
  would be sugar; not built.
- **Bool → text.** No measured need anywhere in the
  corpora. The (a) pattern could add it later at the
  same S price if a body ever needs it; not built
  speculatively.
- **Fixed-width integers (I8…U64).** Not a separate
  case: the implementation's integer values are
  uniformly `Value::Int(i64)` regardless of the
  declared width, and the typechecker's stdlib table
  is name-keyed and coarse (§3a). The builtin takes
  the language's Int value, full stop.

---

## 5. Recommendation

**Form (a): a builtin function `int_to_str : Int →
String`**, in the Appendix A call tradition, with the
§2.3 encoder arm (literal fold + UF, with the
refutation-degradation rule), the §2.5 concrete arms,
and the §1 template construction deferred to its own
phase. Reasoning, in one paragraph: it is the only
form that reaches the payoff site without a new node
((b) cannot reach a body at all; (c) and (d) pay a
full cascade for the same encoding); its per-layer
cost is one `len`-shaped arm at a time, each with a
standing precedent and a standing test shape; it
moves no kind table, so the bootstrap artifact and
the frozen standings are evidence to re-check, not
work to redo; and its one soundness subtlety — the
UF — is measured, bounded to payload-reading rows
that no standing spec states, and governed by a
refutation rule in the house's own Phase 97 style.

The name `int_to_str` mirrors the SMT-LIB operator
the encoder arm conceptually implements and the
stdlib's descriptive snake_case (`is_sorted`,
`is_permutation`); `to_string` was weighed and set
aside (it promises a receiver-style generality the
function does not have, and reads as a method in a
language whose methods are a closed table), as was
`int_to_string` (no precedent for the long form in
the table). The name is the owner's to ratify (D2).

---

## 6. Build plan (priced, each phase with its own gate)

**Phase 100 — the builtin, end to end (S–M).**
Typechecker entry; LANGUAGE_REFERENCE Appendix A
row; `eval_call` arm; `sym_call` + `encode_expr`
arms (fold + UF); the refutation-degradation rule in
the row judges; `encodable.rs` mirror arm;
`ir_call_text` arm. Evidence gate: a per-layer
conversion battery agrees everywhere — the probe 5
table (0, positives, negatives, both i64 extremes)
through the interpreter, the compiled Rust rendering,
and the encoder at literal points; the probe 3/4
shapes promoted to production tests (UF rows decide;
payload-reading row under UF is Unknown, never
Verified, never Refuted-by-UF); the encodable
cross-check corpus gains the conversion shapes with
full agreement. Frozen standings unmoved:
cross-validation and the Codegen probe ride the
suites; suites stated against the Phase 99 counts.

**Phase 101 — the payoff + re-measurement (S).**
`tokenize_step_body`'s error arm builds the full
message (§1's construction; the denotation inherits
it through the shared builder — no second site).
Evidence gate: cross-validation ×3 — the composition
crowns' executed agreement **10/10 and 8/8 on both
routes** with Z3 standings unmoved (Verified);
`morphic check` valid, in-spec tests 18/18 (no spec
is edited — the rows never named the message);
benchmark spot re-run (the generated side changed,
so Phase 97's parentage-by-diff does not apply —
one harness execution, equality gate first);
playground bundle re-measured against the 771,072 B
budget (headroom 44,044 B at Phase 98).

**Phase 102 — M1 gate re-run #4 (S, docs).**
Clause-by-clause exactly as written (Phases 80/88/97
precedent). On Phase 101's evidence the expectation
is clauses 4 and 6 tick and the verdict is **PASS** —
stated as an expectation, not a promise; the re-run
reports what it measures. Merge/release follow the
owner's call after the verdict, as before.

**Total: ≈ M.** What could still fail, named: the
body-level STEP verdict is probe-backed at the row
level (§2.4) but re-measured only in Phase 101 — if
the composed body surprises (a path the probe's
point did not exercise), Phase 101's gate catches it
with the arm already built, and the residue would be
a judging question, not a language question. The UF
refutation rule touches the judges' shared paths —
Phase 100's frozen-standings evidence is the guard
against a verdict moving anywhere it shouldn't.

---

## 7. Decisions for the owner

In the D1–D5 precedent's shape — lettered options,
recommendation first.

- **D1 — the form.** **(a) builtin function call**
  (recommended; S–M, no cascade, reaches the body) /
  (b) D1a method (M, cannot reach an IR body —
  gate-fatal) / (c) new unary node (L, full cascade,
  same encoding) / (d) interpolation syntax (L,
  sugar over (a)).
- **D2 — the name.** **(a) `int_to_str`**
  (recommended; SMT-LIB echo, stdlib style) /
  (b) `to_string` / (c) `int_to_string`.
- **D3 — the scope.** **(a) Int only** (recommended;
  F3 Float stays a separate open finding; Char/Bool
  conversions not built) / (b) Int + Bool + Char
  sugar in the same campaign (adds S; no measured
  need).
- **D4 — the encoder arm.** **(a) literal fold + UF
  with the refutation-degradation rule**
  (recommended; measured in probes 3–4) /
  (b) literal fold + decline symbolic arguments —
  recorded plainly: under (b) the STEP body's Err
  arm is unencodable (its line is a symbolic field
  read), the composition returns to Inconclusive,
  and **the gate cannot close**; (b) is listed so
  its consequence is on record, not as a viable
  alternative.

---

## 8. Probe inventory and suite record

Five tests, all in `src/verify/verifier.rs` (this
phase's only code), labeled Phase 99 scratch probes:

| Test | What it measures | Result |
|---|---|---|
| `phase99_probe_int_to_str_smtlib_semantics` | SMT-LIB `int.to.str` ground semantics via `Solver::from_string`: positives/zero Rust-exact, no leading zeros, **negatives → `""`**, sign-split formula expresses Rust | Pass |
| `phase99_probe_rust_display_reference` | The Rust `format!` table (0, ±, extremes) the concrete layers must match | Pass |
| `phase99_probe_encoders_decline_conversion_today` | Both production encoders decline `Call(int_to_str)` today | Pass |
| `phase99_probe_uf_arm_rows_still_decide` | UF-built Err payload over a free line: real rows 1+2 **Proven** at `" 5"`; row 2 **Refutes** the unlicensed Err at `"spec 5"` | Pass |
| `phase99_probe_uf_completeness_boundary` | Payload-reading row: unprovable under UF (SAT), provable under the literal fold (UNSAT) | Pass |

All five run in 0.03 s together. Suites with the
probes: **542/0/30 default, 542/0/30
bootstrap-codegen, 417/0/22 no-Z3** (baselines
537/537/416; delta = exactly the +5 probes — four
z3-gated, and the Display-reference probe ungated,
hence +1 in the no-Z3 configuration). No production
file changed; the examples file untouched; no spec
amended.
