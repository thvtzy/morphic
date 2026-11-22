# 🔍 Encoder Extension Design — While in the Encoder + Forall Encoding

**Phase 90 (2026-11-22), branch `feat/encoder-ext-90-feasibility`.
MEASUREMENT + DESIGN ONLY: no production code changed, no spec
amended, `examples/functional_tokenizer.morph` untouched. Every
"measured" claim below comes from the Phase 90 probe suite (seven
z3-gated tests in `src/verify/verifier.rs`, commit `ca3d289`),
run on master @ `e9c17ab` (the M1-features campaign merged via
PR #8).**

**Placement choice (recorded):** standalone document, the
`M1_FEATURES_AUDIT.md` precedent. The ROAD carries the Phase 90
note and a pointer; this document carries the evidence, the
recommended strategies, and the phased build plan (Phases 91–97)
with the owner-ratification points the plan depends on.

---

## 0. What this design is for

The M1 gate re-run #2 (Phase 88) closed with the gate **open** on
exactly two named blockers, both routing through the encoder:

- **(α) skip_whitespace's golden half (4/12).** Its spec states
  only a progress floor. Phase 87 measured the strengthening
  candidates: (a) a boundary row encodes but leaves no Verified
  body possible (loop-free bodies cannot cross an unbounded
  whitespace run); (b) is vacuous; (a)+(b) still admit overshoot;
  (c) — the exact pin, *all-whitespace-between*, a Forall over
  positions — is statable but **unencodable**
  (`encode_constraint` → None). Closing α needs (c) encodable
  **and** a body able to verify under the strengthened rows —
  i.e. a While-capable body.
- **(β) the tokenize pair's Verified∧exact split.** The
  `tokenize_template` bodies execute 9/10 and 7/8 against the
  mirrors (sole deviation: Err message text, F2) but Z3 returns
  Inconclusive: `sym_eval` has no While arm and `skip_whitespace`
  has no resolver denotation. The crowned Verified bodies execute
  0/10 and 0/8. No single body is both Verified and
  executed-exact.

The owner commissioned the encoder extension on 2026-11-22 as
the only route to a full M1 close. This phase measures the two
parts separately — **Part 1: While in the encoder** (plus the
denotation question), **Part 2: Forall encoding** — and prices
the build.

**Headline findings, up front:**

1. **The While wall is not one wall.** Behind the missing While
   arm sit four more, each measured in situ: constructor-pattern
   `Match` is unencodable (`sym_match` skips Constructor arms
   wholesale); `UnaryOp::Clone` is unencodable; `sym_ite` has no
   (List, List) arm, so merging a state record whose list field
   is carried unchanged fails; and Append has neither a
   length-only-list arm nor a (Str, CharStr) arm. Any one of
   them kills the faithful bodies.
2. **The denotation gap is wider than (β) states.** `next_char`
   has **no resolver denotation either**: `record_completion_body`
   exists (engine.rs:4810) but is seed-only — it is not in the
   `callee_body` chain (engine.rs:6416) — so today *no body that
   calls next_char is Z3-evaluable at all*. A While arm alone
   creates no denotations; the resolver needs its own extension
   (one chain entry for next_char; keyed denotations for skip
   and tokenize_one).
3. **Bounded unrolling is a refutation and point instrument,
   never a Verified path** — structurally, not incidentally:
   exhaustion (the loop wanting one more iteration than the
   fuel) is reachable at every depth, so under the sound
   discipline below every symbolic verdict is Unknown — measured
   at fuels 1/2/4 for skip (current rows, (a), (a)+(c) in both
   encodings) and fuels 1/2 for the tokenize pair. Its decisive
   products are **refutations** (tokenize_one's row 2 is Refuted
   symbolically — the `" 5"` countermodel — and at the point)
   and **exact point verdicts** (minimal resolving fuel equals
   the mirror trip count at every battery point).
4. **Loop invariants are the only route to symbolic Verified**,
   and the one invariant whose VC looked risky — skip's
   ∀-between invariant under (a)+(c) — has its consecution
   **discharged by Z3 in 5–6 ms**, with and without a `str.at`
   pattern. The IR While node already carries an
   `invariant: Option<String>` field (engine.rs:202, commented
   *"Z3 loop invariant"*); nothing populates it and nothing
   reads it.
5. **Forall is not hopeless — it is cheap.** The (c) row encoded
   through `z3::ast::forall_const` judges **Proven at all 12
   skip battery points (0–25 ms)** for the mirror output, and
   **Refuted for both overshoot outputs**; a bounded expansion
   of the same row is Proven 12/12 at ~0 ms. The quantifier was
   never the hard part; the body under it was (finding 3).
6. **Even a perfect encoder does not Verify the faithful
   tokenize body against the standing rows.** At `" 5"`
   (whitespace, then a bad char) the faithful STEP body errs,
   and row 2's R2-α license reads the *pre-skip* char — a
   space — so row 2 **refutes** it (measured at the point and
   symbolically). The β-skip license (Err licensed by the char
   at the *skipped* position), unstatable in Phase 83 because
   skip had no denotation, becomes statable and encodable the
   moment skip denotes — and measured here, it admits the
   faithful Err and refutes a fabricated one. **This is an
   owner ratification point (R2), not an encoder task.**
7. **A Verified∧exact body must still be *found*, and the
   template cannot crown today even if Verified.** The
   interpreter returns NotEvaluable for the statement/loop
   subset (eval.rs:196–217), so the template's tpr is 0.000;
   `composite_score` (engine.rs:1620) is 85% tpr and the
   selector must not grow a Verified-preference
   (selector.rs:1717). The build plan therefore has **three**
   workstreams — encoder, resolver denotations, interpreter —
   not one.

---

## 1. The exact current boundaries

All citations are to master @ `e9c17ab` production code.

| # | Boundary | Site | Consequence |
|---|----------|------|-------------|
| W1 | `sym_eval_flow` has no statement arms: While, Loop, Break, Alloc, Assign, TryPropagate (and Hole/Lambda/etc.) → None | verifier.rs:698, catch-all ~829–843 | Every loop body is Inconclusive |
| W2 | `sym_match` skips Constructor patterns: `IRPattern::Constructor(..) => continue` (verifier.rs:903); a Match whose arms are all constructors yields zero arms → None at `arms.pop()?` (verifier.rs:907) | verifier.rs:871–925 | `match peek(x) { Some(ch) => …, None => … }` — the canonical loop condition — is unencodable even with W1 fixed |
| W3 | `sym_unary_ir` has no Clone arm ("Head/Tail/Init/Last/Reverse/Clone are outside…", verifier.rs:1594–1596) | verifier.rs:1588 | The hand bodies' `peek(Clone cur)` is unencodable (the interpreter evaluates Clone fine; executed 39/39) |
| W4 | `sym_ite` (verifier.rs:1118) has arms for Int/Bool/Str/Char/CharStr/ListLen/Opaque/Record/Option/Result — **no (List, List) arm**; its Record arm recurses into itself | verifier.rs:1118–1190 | Merging two states whose concrete token list is carried unchanged fails; this is *why* list parameters bind as `ListLen` |
| W5 | `sym_binop_ir` Append: only `(Append, Sym::List, item)` (verifier.rs:1257). No ListLen arm, no (Str, CharStr) arm | verifier.rs:1257 | The template's `toks` Append (length-only form) and lexeme Append both die — *behind* the statement wall |
| W6 | `SpecIndex::callee_body` (engine.rs:6416) chains 9 seed builders; `record_completion_body` (engine.rs:4810) is **not among them** — next_char denotes nowhere; skip_whitespace and tokenize_one have no builder at all (skip's two rows are satisfied by identity and every partial skip — underdetermined, Phase 81/87) | engine.rs:6416–6428, 4810, 843 | No body calling next_char/skip/tokenize_one is evaluable, whatever W1 does |
| W7 | `encode_constraint`: Forall/Exists/Predicate → None | verifier.rs:1659–1661 | Row (c) unencodable. (The parser also discards the range collection — `let _collection`, parser.rs:468 — so a Forall row must fold its range into the body as implications; the AST keeps `vars` + `body` only, spec/ast.rs:161) |
| W8 | Literal-tag boundaries at concrete points: `unwrap` of a literal `Err(...)` is unencodable (Phase 76); a constant index into a literal string out of bounds is unencodable (verifier.rs:~1065) and `sym_eval`'s If evaluates both branches eagerly | verifier.rs (Phase 76 sites) | Row 1 of tokenize_one / tokenize_all's row judge **Unknown at literal Err outputs** (their dead `output.unwrap()` disjuncts don't encode) and peek is unencodable at a literal EOF point — Phase 87's "literal-OOB artifact", re-measured in §2.1. Symbolic judging (Free tags, symbolic indices) is unaffected |
| W9 | The interpreter's evaluable subset excludes While/Loop/Alloc/Assign/TryPropagate (eval.rs:196–217, `NotEvaluable`), and constructor Match patterns never match in-process (Phase 75 residual) | eval.rs:196–217 | The template bodies' tpr is 0.000; they cannot crown regardless of Z3 (see finding 7) |
| W10 | IR `While` carries `invariant: Option<String>` (engine.rs:198–203) — raw uninterpreted text, rendered only as a codegen comment; populated only by a codegen test fixture (m3_gate.rs), read by no verifier code. IR `Loop` has **no** invariant field | engine.rs:198–203 | The invariant strategy needs a semantics for the field, an author, and a VC loop — all new |

Cross-cutting: `verify_one` binds parameters as free constants,
evaluates the body **once** via `sym_eval`, and judges each
constraint in a fresh `Z3Session` with
`timeout_per_constraint_ms = 5_000` (verifier.rs:44). Any
extension must preserve the "unencodable ⇒ Unknown, never a
guess" discipline, and must be mirrored in
`src/verify/encodable.rs` (the abstract no-Z3 mirror of
`sym_eval_flow`, cross-checked against it on a harvested
corpus) — every price below includes that lockstep.

---

## 2. Part 1 — While in the encoder

### 2.1 The probe instrument

The probes implement, inside the verifier's test module only, a
**statement-mode symbolic evaluator** layered on the production
primitives (`sym_eval`, `sym_call`, `encode_constraint`,
`verify_constraint`, the Phase 76 Sym forms):

- Statements: Block sequencing, Alloc/Assign as binds, Return
  and Break as recorded path contributions, If, While/Loop.
- **Two execution modes.** *Guided:* while the solver decides a
  branch condition under the current path (forced-true or
  forced-false, checked with `p90_decide` = assumptions + Char
  side conditions + pc), execution follows the single decided
  branch — binds replace outright, no ite merge, terms stay
  one-iteration sized. *Merging:* the first undecidable branch
  flips the state to classic pc-narrowing symbolic execution
  (binds merge by ite) for the rest of the run.
- **Loops are unrolled to a fuel bound** with the BMC
  discipline of §2.2.
- Probe-only extensions standing in for the production fixes
  they price: clone elision (W3), constructor matching (W2),
  structural ite with a (List, List) elementwise arm (W4),
  Append over ListLen (+1) and (Str, CharStr) (W5), and a
  denotation map (W6) holding next_char's `record_completion`
  seed body (pulled from a real synthesis pool by provenance —
  the `pool_seed` route of tokenizer_benchmark.rs), the Phase
  85 hand skip body, and the production `tokenize_template`
  STEP body.
- Point states bind `pos` as an unfolded `(n + 0)` term so
  indices route through the totalized CharStr path (W8's
  literal-OOB case otherwise makes peek unencodable at EOF
  points before any loop question is reached).

### 2.2 The soundness discipline for unrolling (exact statement)

Unrolled evaluation of a body returns **(paths, exhausted)**:
the per-path output values, and a disjunction of the path
conditions under which some loop still wanted another
iteration when its fuel ran out. Verdicts for a row `f` over an
outcome are:

- **Refuted** ⟺ some path has `pc ∧ ¬exhausted ∧ ¬f`
  satisfiable (a countermodel on a path the unrolling
  *completed*);
- **Proven** ⟺ every path has `pc ∧ ¬exhausted ∧ ¬f`
  unsatisfiable **and** `exhausted` is unsatisfiable (no
  completed path violates the row, and no path was cut off);
- **Unknown** otherwise.

Corollary (the structural fact): for a loop whose trip count is
unbounded over the input space, `exhausted` is satisfiable at
every fuel — some input always wants one more iteration — so
**unrolling can never Verify such a body symbolically, at any
depth**. It can Refute decisively (a countermodel on a
completed path is a real countermodel) and it can decide
concrete points exactly (where the trip count is finite and
known, fuel = trips completes every path). This is the
discipline any production unrolling must implement verbatim;
anything weaker (e.g. treating fuel-exhausted paths as exits)
manufactures false Verified verdicts.

### 2.3 Measured evidence — points

Minimal resolving fuel = the fuel at which the outcome first
concretizes (exhaustion unsatisfiable, exactly one live path);
predicted by the Rust mirrors' trip counts.

**skip_whitespace, hand While body** (`phase90_while_skip_points`)
— 12/12 points resolve at exactly the trip count
(trips 0,1,5,0,2,1,0,3,1,3,1,0 over the cross-validation
battery), outputs equal the mirror's, both current rows Proven
at every point, and strengthening row (a) **Proven at all 12
points** — including the two EOF outputs that judged Unknown in
Phase 87 (under numeral indices, W8; under term indices the row
encodes through CharStr and decides). Teeth: the identity body
is Proven under the current rows and **Refuted by (a)**; a
≤1-char partial skip is **Refuted by (a)** on the run-5 point
and Proven on a run-1 point.

**tokenize_one, production template STEP body**
(`phase90_while_step_points`) — 10/10 battery points resolve at
the predicted fuel: `max(skip_trips, ident_trips + 1)` on the
ident path (the Loop runs one extra iteration to observe the
non-ident char and break), `skip_trips` otherwise. Outputs
equal the mirror's exactly on Ok points; on Err points the tag
agrees and the message text is the standing F2 deviation. Both
rows Proven at every Ok point; at Err points row 2 is Proven
and row 1 judges Unknown (W8's unwrap boundary — an encoding
limit at concrete points, not a body fault).

**The `" 5"` diagnostic (same test).** The faithful body errs.
Production `verify_constraint` judges row 1 Unknown (W8) and
**row 2 Refuted** — the R2-α license reads the pre-skip char.
The **β-skip license**, built manually over the denoted skip
output (`output.is_ok() || <the char at the skipped position
is not a token start>`), **admits** that same Err (¬f UNSAT)
and **refutes** a fabricated Err at `"spec"` (¬f SAT). The
license was unstatable in Phase 83 for exactly one reason —
skip had no denotation — and is encodable the moment it does.

**tokenize_all, production template LOOP body**
(`phase90_while_loop_points`) — 8/8 sources resolve at
`max(outer_iterations, inner_max)` (outer counts the final
no-progress step; inner is the max step need), token lists
equal the mirror's exactly, the spec's row Proven at every Ok
point (Unknown at the `"spec 5"` Err point, W8 again).

### 2.4 Measured evidence — symbolic states

**skip** (`phase90_while_skip_symbolic`): the hand body over a
fully symbolic state judges **Unknown at fuels 1, 2, 4** for
both current rows, for (a), for (a)+(c) through the true
forall encoding, and for (a)+(c) through the bounded expansion
(judge times 4–43 ms). The §2.2 corollary, measured four ways.

**tokenize_one STEP** (`phase90_while_step_symbolic`,
#[ignore]d instrument, ~36 s): ablations first — with no skip
denotation the body **does not evaluate**; with denotations but
no list-append form it **dies at the first Append** (W5/W6
measured in situ). With both it evaluates (12 return paths):
row 1 **Unknown** (exhaustion-bounded), row 2 **Refuted** at
fuels 1 and 2 — the `" 5"` countermodel is symbolic, so this
body is un-Verifiable against the standing rows at *any*
encoder strength (finding 6).

**tokenize_all LOOP** (`phase90_while_loop_symbolic`): the
ablation without a tokenize_one denotation does not evaluate.
At fuel 1 the body evaluates (2 paths) and the row judges
**Unknown**. A fuel-2 run exceeded a 280 s wall without
finishing (guided decisions of 0.6–3.2 s each inside the nested
denotations, then merging) — the practical cost wall for
nested-denotation unrolling. The fuel-1 row formula's tree
print measured **921,091 chars** (see §2.5 on why tree prints
are the wrong size measure but the right warning).

### 2.5 Measured evidence — costs and failure modes

- **Naive pc-narrowing unrolling explodes.** On the one-trip
  point `" "` the merging-mode evaluation cost **1 / 3 / 15 /
  118 / 896 ms at fuels 0–4** (~×8 per iteration): each
  iteration re-evaluates peek (a full `str.at` scan expression)
  over the ite-merged state, and merged terms at least double.
  Solver-guided decisions make point evaluation **flat (5 ms
  at fuels 0–8)** — but guidance is a *point* technique: over
  symbolic states the first condition is undecidable and the
  merging costs return.
- **Solver time is the other wall.** Inside the symbolic STEP
  evaluation, individual guided-decision checks measured
  0.9 s, 1.6 s, and one 5 s timeout (→ merging fallback).
  Production's per-constraint budget is 5 s
  (verifier.rs:44); statement-mode bodies multiply the number
  of checks per verdict, so the budget question is real
  (flagged in §5, not decided here).
- **Term identity must be node identity.** Comparing Z3 terms
  by Debug string cost **96 s** on the STEP body (printing a
  shared DAG as a tree is exponential in its depth); Z3
  hash-conses and its AST `==` is `Z3_is_eq_ast` — O(1). Any
  production merge/assembly code must use node identity, never
  string forms. (The 921,091-char formula above is the same
  phenomenon viewed from the other side.)
- **Path assembly is a probe artifact, not a production
  need.** The probe's statement evaluator returns path lists
  that denoted calls must assemble into one value (ite fold,
  with a solver-concretization fallback); production
  `sym_invoke` evaluates a callee body *in the caller's
  environment* and gets one value directly. The production
  design in §2.6 therefore extends `sym_eval_flow` itself and
  needs no assembly layer.

### 2.6 The denotation question — settled

**A While arm does not make a skip denotation derivable, and
the resolver needs its own extension — in three pieces, all
measured:**

1. **next_char (S).** `record_completion_body` already builds
   the body; it is simply absent from the `callee_body` chain.
   Adding it as a chain entry (the Phase 81 complementary-guard
   precedent: appended at the END) denotes next_char. The
   builder declines on skip/peek shapes (Phase 81 tests), so
   the entry is safe. With it, the hand skip body becomes
   evaluable the moment W1–W3 land.
2. **skip_whitespace (part of M).** No row-derived builder can
   denote skip — its rows are satisfied by identity and every
   partial skip (underdetermined by construction). The
   denotation must be **keyed**: a builder that recognizes the
   spec's shape (a state→state function whose rows are the
   standing monotone/length pair over a module with the three
   classifiers) and returns the hand body (Alloc/While over
   peek/next_char — the Phase 85 transcription). This is the
   `tokenize_template` seed-builder pattern applied to the
   resolver: deterministic, spec-shape-keyed, declines
   otherwise.
3. **tokenize_one (part of M).** Its denotation is the
   `tokenize_template` STEP body, keyed the same way (the
   builder exists as a seed builder in engine.rs). tokenize_all
   needs no denotation of its own for the gate — its LOOP body
   *is* the candidate; it needs tokenize_one's denotation to
   be judged, which (3) supplies.

Depth is not a concern: the inlining cap `MAX_INLINE_DEPTH = 8`
(engine.rs:6301) accommodates LOOP → STEP → skip → next_char →
peek with room to spare.

### 2.7 Strategies — recommended and rejected

**Recommended: hybrid, three layers.**

- **Layer 1 — statement-mode `sym_eval_flow`** (Alloc/Assign/
  Block/Break/Return/TryPropagate + While/Loop) with **bounded
  unrolling under the §2.2 discipline** as the refutation and
  point-decision path. This is what makes the faithful bodies
  *evaluable and refutable* — it converts today's blanket
  Inconclusive into decisive verdicts where they exist
  (row 2's refutation is information the gate needs).
- **Layer 2 — loop invariants as the Verified path.** Give the
  While node's `invariant` text a semantics: a module-dialect
  constraint over the loop's locals, parsed by the spec parser
  at verification time; the verifier checks the three VCs
  (initiation, consecution, exit-implication) and, when they
  discharge, evaluates the loop's effect as the invariant's
  exit form rather than by unrolling. Add the same field to
  `Loop` (a field addition renumbers no IR kind codes; the
  Phase 73 append discipline is respected). The invariants the
  gate needs are **frame+progress** facts — for the tokenize
  loops: source unchanged, tokens only grow (LOOP: `len`
  monotone), pos monotone; for skip under (a)+(c): the
  ∀-between invariant of Part 2 — authored by the same keyed
  builders that supply the denotations (§2.6), not expected
  from MCTS search.
- **Layer 3 — the small arms** (W2 constructor Match, W3 Clone,
  W4 (List, List) ite, W5 Append forms) and the resolver
  denotations (§2.6), without which neither layer has bodies
  to work on; plus the interpreter subset (W9) so the bodies
  can crown (finding 7).

**Rejected alternatives:**

- *Pure unrolling as the Verified path* — refuted structurally
  (§2.2 corollary) and measured (§2.4). Any design that
  "unrolls deeper" is buying refutations at ×8 per iteration.
- *Invariants without unrolling* — loses the decisive
  refutations and all point evidence; and the invariant VCs
  for the tokenize loops, while plausible (frame+progress),
  are the least-proven piece (§5). Unrolling is also the
  fallback that keeps every verdict honest while invariants
  are authored.
- *A general denotation synthesis from rows* — skip's rows
  provably underdetermine its body; a general mechanism would
  denote *something* satisfying the rows (identity does) and
  call it the meaning of skip, which is exactly the α failure
  wearing a resolver costume. Keyed denotations are honest
  about being authored.

**Prices** (campaign calibration): statement-mode core +
unrolling + verdict plumbing **L**; invariant representation +
VC generation **L**; resolver denotations **M** (chain entry S
within it); interpreter statement/loop subset **M**; small
arms **S** each family (Match S–M). Every item includes the
`encodable.rs` lockstep.

---

## 3. Part 2 — Forall encoding

### 3.1 What the CharStr bridge actually provides

`source[i]` symbolically is `Sym::CharStr` (verifier.rs, Phase
76): the Z3 **String** term `s.at(i)` — not an array, not a
function. Length is `str.len(source)`; the indexed char
supports equality/order against another indexed char (string
order on length-≤1 strings *is* scalar order, with the
out-of-bounds empty string sorting below every real char) and
against a **constant** Char (converted to its one-char
string). Against a *symbolic* Char parameter it is
unencodable (the safe z3 API has `str.at` but no
`str.to_code`) — a named boundary that does **not** bite here:
`is_whitespace` inlines (through `callee_body`) to comparisons
against literal chars.

The (c) row as ratified in Phase 87's pricing —
`forall i: Int in range(len(output.source)): i < state.pos ||
i >= output.pos || is_whitespace(output.source[i])` — parses
to `Constraint::Forall { vars: [(i, Int)], body }` with the
range folded into the body as implications (the parser
discards the collection expression, parser.rs:468; the
typechecker binds the var and checks the body). The body
encodes through production `encode_constraint` with the bound
var as a fresh Int constant and `output`/`state` as the
judged values; the quantified assertion closes with
`z3::ast::forall_const` (z3 0.20, ast/mod.rs:700).

### 3.2 Measured evidence (`phase90_forall_encoding_measurement`)

- **Production re-pin:** `encode_constraint` on the (c) row
  returns None today (W7).
- **forall_const encoding at points:** the mirror skip output
  satisfies (c) — **Proven at 12/12 battery points**, judge
  times **0–25 ms**. Quantifier discharge over `str.at` at
  concrete states is fast and decisive.
- **Bounded expansion** (the conjunction over offsets
  `0..(output.pos − state.pos)` of `idx ≥ output.pos ∨
  is_whitespace(source[idx])` — exact whenever the expansion
  bound covers the range, which at a point it does by
  construction): **Proven 12/12 at ~0 ms**.
- **Teeth:** both overshoot outputs Phase 87 identified
  (`"a b"` → pos 2, `"ab cd"` → pos 3) are **Refuted by (c)**
  under **both** encodings. The identity output and the
  ≤1-char partial output are **admitted** by (c) — their
  ranges are empty, so (c) is vacuous there. That is the
  division of labor Phase 87 predicted, now measured end to
  end: **(a) kills identity and partial skips; (c) kills
  overshoot; the exact pin is (a)+(c) jointly.**
- **Symbolic discharge — the invariant consecution.** The
  one-step consecution VC for the ∀-between invariant
  `H(p₀, p) = ∀ i, p₀ ≤ i < p → is_whitespace(source[i])`
  (with `p₀ ≤ p < len(source)` and `is_whitespace(source[p])`
  as hypotheses, conclusion `H(p₀, p+1)`): **Proven bare
  (5 ms) and Proven with a `str.at` trigger pattern (6 ms)**.
  Z3's quantifier instantiation handles the one-step
  `str.at` extension shape the invariant route depends on.
- **The row is not the symbolic bottleneck.** Judged over the
  unrolled symbolic skip outcome, (a)+(c) is Unknown at fuel 4
  (§2.4) — the *body's* exhaustion binds, exactly as the
  discipline requires; the row encoding itself discharges
  wherever the body completes.

### 3.3 Recommendation and price

**Encode Forall via `forall_const` as the primary form**
(single- and multi-var), with the **bounded expansion as a
fallback** for shapes where the range is fuel-bounded and the
quantifier resists (the expansion shares Part 1's §2.2
discipline: exact when the bound covers the range, honestly
Unknown beyond it). Rejected: bounded-only (the skip
invariant's consecution is a forall over an *unbounded* range
— §3.2 shows the true quantifier discharges it, so the
bounded form cannot substitute); array/Skolem encodings (no
array theory for strings in the safe API, and unnecessary on
this evidence).

**Price: S–M** for the `forall_const` arm in
`encode_constraint` (+ `encodable.rs` mirror), **S** for the
bounded fallback. The expensive half of closing α was never
this row — it is the While body under it (Part 1) and the
owner's ratification of the row itself (R1, §4).

---

## 4. The phased build plan (Phases 91–97)

Each phase is one branch, one gate, suites in all three
configurations (baselines at Phase 90: **497/0/30 default,
497/0/30 bootstrap-codegen, 405/0/22 no-Z3** — the Phase 90
probes included; production baselines before them 491/491/405).

**Phase 91 — encoder small arms (S).** Clone → identity in
`sym_unary_ir`; (List, List) elementwise arm in `sym_ite`;
Append over ListLen (n → n+1) and (Str, CharStr) concat in
`sym_binop_ir`; constructor arms in `sym_match` (tag
conditions + payload binds + the standing exhaustiveness
discipline). `encodable.rs` lockstep for each.
*Gate:* suites green; the Phase 76 boundary tests re-pinned
where a boundary intentionally moves; peek/next_char executed
agreement 39/39 unchanged.

**Phase 92 — resolver denotations (M).** `record_completion_body`
appended to the `callee_body` chain (next_char denotes);
keyed denotation builders for skip_whitespace (the hand While
body) and tokenize_one (the template STEP body), declining on
any other spec shape. *Gate:* `callee_body` returns Some for
next_char/skip_whitespace/tokenize_one; the denoted skip body
executes 12/12 against the mirror through the cross-validation
harness; no standing verdict moves except next_char-dependent
Inconclusives resolving to their measured values.

**Phase 93 — statement-mode encoder + bounded unrolling (L).**
The §2.7 Layer 1 build in `sym_eval_flow` + `verify_one`'s
verdict plumbing (exhaustion-aware, §2.2 verbatim), with the
`encodable.rs` mirror. *Gate:* the Phase 90 probe batteries
re-run as production tests — points resolve at the predicted
fuels, outputs mirror-exact, symbolic verdicts Unknown/Refuted
exactly as measured in §2.4; **teeth:** identity and partial
bodies under (a), the `" 5"` refutation — and no false Verified
anywhere in the suites.

**Phase 94 — Forall encoding (S–M).** The §3.3 build:
`forall_const` arm + bounded fallback in `encode_constraint`
(+ mirror). *Gate:* the (c) row encodes; point teeth as
measured in §3.2 (mirror Proven 12/12, overshoot Refuted,
identity/partial admitted) reproduced through production
machinery. No spec is amended in this phase — the row's
adoption is R1's, in Phase 96.

**Phase 95 — interpreter statement/loop subset (M).** Alloc/
Assign/While/Loop/Break/TryPropagate evaluation + constructor
Match matching + Str Append in `src/synthesis/eval.rs`, so
denoted and template bodies earn honest tpr. *Gate:* the
template STEP/LOOP bodies' in-spec tests pass in-process at
the rates the mirrors predict (STEP: keyword-spec passes;
LOOP: its tests pass); cross-validation executed agreement
unchanged (774/774 classifiers, 39/39 peek/next_char).

**Phase 96 — loop invariants (L).** Semantics for the While
`invariant` text (+ the field added to Loop), VC generation
(initiation/consecution/exit), and the keyed builders
authoring the frame+progress invariants for the tokenize
loops and the ∀-between invariant for skip (joining Phase 94).
**This phase executes the owner ratifications R1–R3 (§5)** —
the strengthened specs and, if ratified, the round-3 row —
and is where the gate's boxes become tickable. *Gate:*
skip_whitespace Verified against (a)+(c) with executed
agreement 12/12; tokenize_one/tokenize_all Verified∧exact on
their batteries (modulo R3's disposition of message text).

**Phase 97 — crowning campaign + M1 gate re-run #3 (M).**
Full synthesis with all extensions live; the Verified∧exact
bodies must be *found* by the search (§5's G5 caveat);
cross-validation re-run; gate boxes 4 and 6 judged; merge
decision to the owner with the gate record.

Dependencies: 91 → 92 → 93 is the spine; 94 is independent of
93 and can land in parallel order; 95 is independent; 96 needs
93 + 94 (+ the ratifications); 97 needs all.

---

## 5. Owner-ratification points and what could still fail

**Ratifications the plan depends on (owner decisions, not
encoder work):**

- **R1 — skip_whitespace spec strengthening (a)+(c).** Without
  it, α stays open *by spec*: the current rows admit the
  identity body, which is Verified today and executes 4/12.
  The encoder can make (c) encodable (Phase 94) and a While
  body Verifiable under it (Phases 93+96); only the owner can
  put the rows in the spec.
- **R2 — tokenize_one row 2, round 3 (R2-β-skip).** Measured
  in §2.3/§2.4: the faithful body is **Refuted** by the
  standing row 2 at `" 5"`, at the point and symbolically. No
  encoder strength changes that — the row licenses Err by the
  pre-skip char. The β-skip form (license reads the char at
  the skipped position, via skip's denotation) is statable
  once Phase 92 lands, and measured here to admit the faithful
  body while keeping its teeth. Without R2, (β) cannot close
  in full; the honest gate outcome would be Verified∧exact
  *except* the `" 5"` class, named.
- **R3 — F2 (Int→decimal conversion for interpolated
  strings).** The template's sole executed deviation is the
  Err message text (the mirror interpolates
  `"Unexpected character '{}' at line {}"`; the template's is
  constant, because the language has no Int→string
  conversion). "Exact" at 10/10 and 8/8 needs either F2 (a
  language decision) or a gate-text reading that excludes
  message text. The encoder cannot fix a language gap.

**What could still fail even if everything builds:**

- **Crowning is a search outcome (the G5 precedent).** The
  plan makes Verified∧exact bodies exist, be evaluable, and
  earn tpr; `composite_score` (85% tpr) and a selector that
  must not grow a Verified-preference decide the winner. A
  Verified∧exact body in the pool is not a crowned body.
  Phase 97 is sized M partly for this risk.
- **The tokenize loops' invariants are argued, not yet
  discharged.** Skip's ∀-between consecution is Proven
  (§3.2); the LOOP's frame+progress invariant set is
  sufficient *for the rows at stake* by inspection of the
  rows (they speak only of token-count and length facts),
  but no VC for it has been run. Phase 96's gate is where
  that argument becomes evidence — or doesn't.
- **Z3 quantifier/string fragility at larger shapes.** The
  measured discharges are at battery scale (sources ≤ 15
  chars, one bound var, one-step consecution). Initiation VCs
  over the full module environment and multi-var Foralls are
  unmeasured; the bounded fallback (§3.3) is the hedge.
- **Solver-budget pressure.** Statement-mode judging
  multiplies solver checks per verdict, and individual checks
  already touch the 5 s cap (§2.5). Whether
  `timeout_per_constraint_ms` remains the right budget for
  loop bodies is a Phase 93 measurement, not an assumption.
- **W8 stays.** The literal-tag boundaries (unwrap at a
  literal Err, literal-OOB index) are untouched by this plan:
  they constrain *point-wise* Z3 judging only. The gate's
  currency is symbolic verdicts, which they do not affect —
  but any future point-wise judging inherits them, named.

---

## 6. Probe inventory and suite record

Seven tests, all in `src/verify/verifier.rs` (commit
`ca3d289`), all `#[cfg(feature = "z3-support")]`:

| Test | What it measures | Result |
|------|------------------|--------|
| `phase90_while_skip_points` | Hand skip body, 12 points: fuels, outputs, rows, (a), teeth | Pass |
| `phase90_while_skip_symbolic` | Symbolic skip: Unknown at fuels 1/2/4 under current rows, (a), (a)+(c) ×2 encodings | Pass |
| `phase90_while_step_points` | Template STEP, 10 points + the `" 5"`/β-skip diagnostic | Pass |
| `phase90_while_step_symbolic` | STEP symbolic: ablations (W5/W6), 12 paths, row 1 Unknown, row 2 Refuted | Pass (#[ignore]d, ~36 s) |
| `phase90_while_loop_points` | Template LOOP, 8 sources: fuels, token lists, row | Pass |
| `phase90_while_loop_symbolic` | LOOP symbolic: ablation, fuel-1 Unknown | Pass |
| `phase90_forall_encoding_measurement` | (c) via forall_const + bounded expansion: points, teeth, consecution | Pass |

Suites with the probes: **497/0/30 default, 497/0/30
bootstrap-codegen, 405/0/22 no-Z3** (baselines 491/491/405;
delta = +6 live probes, +1 ignored, zero in the no-Z3
configuration by construction — every probe is z3-gated).
No production file changed; the only modified file is
`src/verify/verifier.rs`'s test module.
