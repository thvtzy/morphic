# LIST_ENCODER_SCOPE.md — Phase 138: the frozen §3.9 scope + the 1.3 gate

The ratification + freeze document for **§3.9 — the List
element encoder**: the headline post-1.2 campaign that gives
the verifier an element model for List parameters, and for
the milestone it carries, **1.3**. Source material:
[`LIST_ENCODER_AUDIT.md`](LIST_ENCODER_AUDIT.md) (Phase 137 —
the wall inventory of its A1, the model analysis of its A2,
the payoff table of its A3, the sub-wall pricing of its A4,
the synthesis and decision list of its S1/S2, the gate sketch
of its S3; probe artifact `tests/phase137_encoder_audit.rs`,
7 probes, every audit claim a `P137|` line), over the library
frozen by [`STDLIB_SCOPE.md`](STDLIB_SCOPE.md) as amended by
its Amendments 1–2, the UF arm frozen by
[`G4_SCOPE.md`](G4_SCOPE.md) §4, and the package surface
frozen by [`EMBED_SCOPE.md`](EMBED_SCOPE.md) (§2.4 the
manifest, §6 R11 the label-upgrade mechanism).

**Ratification.** On 2026-11-24 the owner ratified the
audit's recommended package whole — *"Gas rekomendasi audit
— D1a sampe D7a semua"* — adopting decisions **D1a–D7a** as
stated in §1 below. Per D2a the two headline campaigns are
separate and §3.9 goes first: **this document freezes the
§3.9 campaign only.** D4a, D5a and D7a bind the
quantified-mod campaign that follows; they are recorded in
§1 as that campaign's standings, and their freeze is Phase
145's. This document freezes:

- **the element-model semantics** (§2): the array + length
  binding, the mirror discipline, the guard discipline, the
  equality form, the constructed-list bridge, and the
  Append/Concat answer — a later phase is judged clause by
  clause against it.
- **the frozen corpus** (§3): the seventeen Tier-2 entries
  with their required end standings, grouped by the sub-wall
  that carries each, plus the non-mover corpus whose
  standings this campaign must not move.
- **the honesty machinery** (§4): the fabricated-countermodel
  probe, the mirror-agreement battery, and the drift sweep,
  designed here so the build phases execute them rather than
  invent them.
- **the package obligations** (§5): the Phase 144 re-tier
  under EMBED_SCOPE R11, with the npm budget figure frozen.
- **the gate** (§6): the audit's 5-clause sketch finalized
  into gate text in the M1-gate style, closing line
  included, verbatim from the audit's draft.
- **the phase plan** (§8): Phases 139–144, each with entry
  condition and exit evidence.

Docs only: this phase changes no production code, no spec,
and no test. The campaign that executes against this freeze
is frozen in §8 (Phases 139–144).

---

## 1. The ratified decisions (D1a–D7a)

**D1 — Element model: (a) arrays + length.** A List
parameter binds as a Z3 `Array<Int, Int>` term **plus the
standing length term** (§2.1); list equality is the exact
form frozen at §2.4. Rejected: (b) sequences — native
concat/append and exact equality, but `seq.nth`'s
out-of-range unconstrainedness touches *every* index row
(the Phase 133 Char fabrication class in new clothes), and
sequence + quantifier solving is the solver's least
predictable fragment; (c) bounded unrolling — rejected on
Phase 90's measurement: it decides concrete points and can
never Verify a symbolic row, so it is a judging aid, not an
encoder. *Reason in one line: the corpus's rows are
index-quantified, not subsequence-shaped; arrays make those
rows direct, keep today's length term as the length half,
and confine the new fabrication surface to named, closable
points.*

**D2 — Campaign structure: (a) two campaigns, §3.9
first.** This freeze covers the §3.9 campaign (1.3);
quantified-mod follows under its own freeze (Phase 145).
Rejected: (b) one combined campaign, one gate — the two
halves share almost no machinery and their risks differ in
kind (fabrication-via-model vs incompleteness), so one gate
would blur two honesty stories; (c) gcd first — it spends
the small campaign's risk budget before the milestone-sized
one and delays fifteen entries for one. *Reason in one
line: shared instruments, separate builds (audit S1).*

**D3 — The Tier-1 bar: (a) full Verified or the entry
stays Tier-2.** Tier-1 keeps its standing definition —
Z3-Verified ∧ battery-exact through the production pipeline
— with no intermediate standing. Rejected: (b) a named
intermediate tier ("Verified modulo the element model's
axioms") — a tier the packages would have to explain
forever and the manifest cannot state honestly in one
field; (c) upgrade on executed evidence + model review —
that is Tier-2's evidence with a Tier-1 label. *Reason in
one line: the project's whole wedge is that Tier-1 means
**proven**; an entry whose rows do not close stays Tier-2,
named, no shame in it.*

**D4 — gcd's tier on return: (a) Tier-1 or no return.**
Recorded here as the quantified-mod campaign's standing
(audit B2): the faithful Euclid body already executes 6/6
today, so a Tier-2 return would add nothing over the Phase
111 pin. Not frozen by this document — its freeze is Phase
145's. Its 1.3 consequence is a non-mover: `int_gcd`'s pin
(contract preserved, body executed-exact, verdict
Inconclusive) is part of §3.4's corpus and this campaign
does not touch it.

**D5 — The judge route for quantified-mod: (a) J1
instantiation discipline first, J2 dvd-normalization if
measured insufficient.** Recorded as that campaign's
standing (audit B2); J3 (a gcd-specific decision route)
stands rejected as primary — a second verdict authority is
a permanent honesty surface. Not frozen here (Phase 145).

**D6 — 1.3 exclusions: (a) §3.10, multi-binder widening,
and non-Int/nested/function element sorts are out.** The
full exclusion list is §7. `list_sort` is named standing
residue, not silently half-upgraded: its row 1 is the
§3.10 `is_permutation` Predicate and its row 3 is a
two-binder forall past the Phase 94 envelope — elements are
necessary for it, not sufficient. Rejected: (b) include
§3.10 — it is its own decision and its own campaign
surface; (c) include all element sorts — the corpus's
Tier-2 surface is `List<Int>` throughout, and Char-element
lists carry the Phase 133 side-condition class. *Reason in
one line: 1.3's gate stays measurable.*

**D7 — The Euclid seed family: (a) built after the judge
half lands, in the quantified-mod campaign.** Recorded as
that campaign's standing (audit B2): a family built before
the judge can crown its product generates candidates the
pipeline must score executed-only, which the hand body
already provides. Not frozen here (Phase 145).

---

## 2. The frozen semantics contract (D1a, D3a, D6a)

The contract the build phases are judged against, clause by
clause. "Declines" throughout means the encoder's standing
discipline — the shape is unencodable, the verdict
Inconclusive, never a totalized value, a silent assumption,
or a fabricated verdict in either direction.

**2.1 The binding (sub-wall W1).** Where the verifier today
binds a List-typed value as `Sym::ListLen` — a parameter, a
record field, a statement-mode havoc/freshening of a
List-typed local (the standing `fresh_sym` sites) — a
`List<Int>` value instead binds as a **pair**:

- the **standing length term**, unchanged in name and form
  (`{path}.len`, the `Sym::ListLen` payload): `len` reads
  it, `sym_len` and the length totalizations consume it
  exactly as today. **The length binding does not move.**
  Every length-level standing in the tree stands on it —
  `list_is_empty`'s Tier-1 crown, the tokenizer's
  length-observed rows, the Phase 76 record-field binding.
- a fresh **array term** of Z3 sort `Array<Int, Int>` over
  the same binding path, carrying the elements: index `i`'s
  element is `select(array, i)`, meaningful exactly on
  `0 ≤ i < len` (§2.3).

The pairing is one binding, not two models: any code path
that produces a List value in the verifier produces the
pair or declines the shape. The concrete-list form
(`Sym::List(Vec<Sym>)`) is not replaced — it meets the pair
through the bridge (§2.5). The pair engages **only for
`List<Int>`** (§9 R9): a List of any other element sort
keeps the `ListLen` binding exactly, and its element rows
decline exactly as today — no verdict for a Char-, String-,
nested- or function-element list moves in either direction
in 1.3.

**2.2 The mirror discipline.** `encodable.rs` learns the
paired form beside `Abs::ListLen` (the audit's name for the
shape: `Abs::ArrayLen`), extended **in lockstep, shape for
shape**, with the real encoder at every phase: a shape is
encodable in a build phase exactly when both sides accept
it. The Phase 133 instrument is the standing one — an
agreement battery pinning the real and pure encodability
predicates **equal in both directions, per shape** — and a
shape one side accepts and the other declines is a defect
by definition, as it was for the UF arm.

**2.3 The guard discipline (load-bearing, verbatim).**
**No index term is ever produced without its in-range proof
arising from the row's own guard.** Unpacked, so a build
phase can be judged against it:

- An index expression `xs[i]` over a paired-binding list
  encodes to `select(array, i)` only where the encoding
  context carries the in-range fact `0 ≤ i < len(xs)` from
  the row's own hypotheses: the bounded-quantifier guard
  that introduces `i` (`forall i: Int in range(lo, hi)` —
  §2.7), a `require:` or constraint hypothesis in scope
  that states it, an antecedent of the row being encoded,
  or an index that is itself a concrete in-range value.
- Where no such guard exists, the row is **unencodable** —
  it declines (Inconclusive). It is never totalized: no
  default element, no clamped index, no `select` left for
  the solver to interpret freely, in either the
  constraint encoder or the statement-mode (body) encoder.
- The array's content at indices `≥ len` is **unobservable
  to every verdict**: no row's truth may be decided by a
  beyond-length `select`. §2.4's equality form is the
  load-bearing instance; the discipline is general.

This is the Phase 133 Char lesson applied to elements: a
model value no execution could produce must never decide a
row — in the Verify direction (a false spec crowned) or in
the refutation direction (a true spec refuted through
beyond-length junk a countermodel invented).

**2.4 Equality — the exact form.** For two paired-binding
lists `xs`, `ys`:

> `xs == ys` encodes as
> `len(xs) == len(ys) ∧ ∀ i. 0 ≤ i < len(xs) → select(xs, i) == select(ys, i)`.

That is the frozen form — the weakest form that is exactly
Morphic list equality. Raw Z3 array equality is **not** the
form: array extensionality ranges over every index,
including beyond the length, so `array_xs == array_ys`
lets beyond-length content decide a row (two executions of
the same list whose arrays differ beyond the length are the
same Morphic list and different arrays). An implementation
may discharge the frozen form through raw array equality
**only** under a construction that makes the two coincide
on every value the encoder can produce — e.g. every array
in a verification built as stores over one shared constant
base array, so beyond-length content is common by
construction (§9 R1). Where neither the pointwise form nor
such a canonicalization is in place, equality declines.
Equality between a paired binding and a concrete list
crosses the bridge (§2.5) and then takes this same form.
Inequality is its negation, no separate form.

**2.5 The constructed-list bridge.** Bodies build lists;
the model must meet them in the same paired form:

- A list **literal** `[a, b, c]` binds as the store-chain
  of its element terms over the constant base array, with
  length its count — which is exactly how the standing
  concrete form `Sym::List(Vec<Sym>)` crosses into the
  pair: element `i` is the literal's `i`-th term, and any
  comparison against a paired binding takes §2.4's form
  over the chain.
- A body **output** the contract's rows observe elementwise
  binds through the bridge in the same way: the
  construction's element facts, as the body's encoding
  establishes them, are the output's array; its length is
  exact. For a loop-built output (`range` is the named
  exhibit — its body appends `lo + k` in a For loop), the
  element facts are the ones the Phase 96 invariant
  machinery states over the built prefix; the bridge binds
  the completed output as the array those facts describe.
- Where a construction's element facts **cannot be
  carried** — a loop whose invariant does not state them, a
  construction shape the bridge does not cover — the output
  stays length-bound (the `ListLen` standing) and element
  rows over it decline, named. The bridge never invents an
  element fact the body's encoding did not establish.

**2.6 Append and Concat — the named answer.** Append has
no native array story; the contract gives it the measured
one instead of a totalization (audit A2, risk 3):

- **Append** (the single-element push, the IR's `Append`)
  over a paired-binding list is **one store**: array
  `store(a, len, x)`, length `len + 1` — exact, in-model.
  Over a constructed list it extends the bridge's
  store-chain (§2.5). The standing length totalization
  (`n → n+1` with elements unmodelled) is thereby retired
  for paired bindings: an appended list's elements are
  modelled, not merely counted.
- **Concat / Merge** (list ++ list) of two paired-binding
  lists has no finite store-chain: it **declines
  elementwise** — its length still encodes exactly
  (`len = len(xs) + len(ys)`, so length rows over a
  concatenation stand), and any element row over it is
  unencodable. This is a named residue, not a silent
  totalization. Measured against the frozen corpus (§3):
  no §3 entry's contract rows require a symbolic Concat —
  `range`'s appends are over a constructed list (the
  bridge's chain), and the take/drop/reverse bodies build
  by index, not by concatenation. A build phase that finds
  a corpus row needing it has found a freeze-amendment
  conversation, not a licence to totalize (§9 R3).

**2.7 Quantifier forms (sub-wall W3).** The corpus's
element rows are the standing bounded forms, one binder,
over index ranges:

- **Forall-over-range** at element level:
  `forall i: Int in range(lo, hi): P(…, xs[i], …)` encodes
  with the range as the binder's guard — the Phase 94
  envelope's shape one level down, and §2.3's canonical
  guard source. The envelope's other limits stand:
  one binder, Int, the tight form (D6a — no multi-binder
  widening).
- **The exists arm**: `exists i: Int in range(lo, hi): P`
  over the same bounded form — `list_min`/`list_max`'s
  membership rows are its exhibits. This arm is an encoder
  form over the standing bounded-quantifier syntax; it is
  **not** a `Constraint::Predicate` encoding, and §3.10
  proper stays out (§7). A negated bounded forall (the
  `list_contains` shape: a forall in an implication's
  antecedent) encodes by the same arms — quantifiers are
  terms, their position in the row does not change their
  form.

**2.8 The fold form (sub-wall W4).** Three frozen
contracts are fold forms: `list_sum`'s
`output == fold(xs, 0, |acc, v| acc + v)`, `list_product`'s
with `1` and `*`, and `list_fold`'s `output == fold(xs,
init, f)`. The contract the encoder must meet is the fold's
**definition over prefixes**, stated here as the only
characterization W4 may assert:

- base: the fold over the empty prefix is `init`;
- step: the fold over the prefix of length `k + 1` is
  `op(fold over the prefix of length k, select(xs, k))`,
  for `k` in range.

Whether the build realizes this as an uninterpreted fold
term over (array, length, init) carrying the two rows as
axioms, or by structural recursion on the length, is the
build's choice; the characterization is frozen. The two
rows are definitional — true of every fold — so a Verified
verdict under them is a proof about the fold as defined,
and anything they do not determine stays Inconclusive. For
`list_sum`/`list_product` the `op` is the closed lambda of
the contract; for `list_fold` the `op` is the parameter `f`
itself, and the step row composes W4 with W5 (§2.9). No
other axiom about a fold may be asserted — in particular
nothing about folds over reordered or filtered lists.

**2.9 UF composition (sub-wall W5).** Phase 133's UF arm
composes over the element model at exactly these points:

- **Application at element terms**: `f(xs[i])` in a row
  encodes as the UF applied to the guarded `select` term —
  §2.3 binds here as everywhere: an application at an
  unguarded index declines with its row. The UF's sorts are
  Int → Int / Int → Bool, inside the Phase 133 slice.
- **Body-side characterizations**, one per higher-order
  node, each exactly as strong as the frozen contract it
  serves: **Map** — the output's element at `i` is
  `f(select(xs, i))`, pointwise, length preserved;
  **Fold** — §2.8's step row with `op = f`; **All/Any** —
  the quantified predicate applications of their contract
  rows, both directions; **Filter** — exactly its two
  frozen rows (the length bound, and the predicate holding
  at every output element): no stronger characterization
  (no order, no completeness, no subsequence claim) is
  asserted for Filter's body than its contract states.
- **The claim is congruence over elements, nothing more**:
  same element in, same value out, at guarded indices.
  Any row whose truth needs a fact about `f`'s values
  beyond congruence — a value at a point, injectivity,
  monotonicity — stays Inconclusive (the §4 specimen pins
  the class). This is §4 of G4_SCOPE carried over element
  terms, with the same may/may-not boundary.

**2.10 Solver and judging discipline.** The verdict layer
is untouched: solver Unknown maps to Inconclusive, per-row
solver budget is the standing one, and this campaign adds
no path around either (audit A2, risk 4). The build is
verifier-side only: the interpreter, the compiled runner,
the seed families and the executed batteries do not change
— an entry's battery-exactness at the re-tier is measured
against the same frozen batteries it ships with today.

**2.11 What does not change.** The length term's name and
form (§2.1); the concrete-list form and its machinery
(indexing, elementwise ite, Append/Concat over concrete
lists — the audit's A1 control, which Verifies today and
must keep Verifying); `sym_ite` over paired bindings
(merging two paired bindings merges lengths by the standing
ite and arrays by the same ite — a merged list's elements
are the branches' elements at each index, which is exactly
what the array ite says); the Phase 76 Char side-condition
discipline (no new side-condition class is created —
Char-element lists are out, §7); the Phase 109
precondition machinery (`require:` rows are assumed as
today — `list_binary_search`'s stretch, §3.3, turns on
exactly this); the Phase 96 invariant machinery, which W3
and the bridge consume but do not amend.

---

## 3. The frozen corpus and required standings

The corpus the gate's clause 1 measures: the library's
seventeen Tier-2 entries, grouped by the sub-wall that
carries each, plus the non-mover corpus of §3.4. An entry's
**required end standing** is frozen here; "Verified ∧
battery-exact" means Z3-Verified through the production
pipeline ∧ 100% agreement with the entry's frozen
cross-validation battery (the EMBED_SCOPE §2.4 manifest
counts — pinned points, spec-silent points excluded exactly
as the batteries record them), the Tier-1 definition of
D3a. A standing may be changed only by a recorded amendment
to this freeze. The campaign's baseline instrument is the
audit's sweep: **all seventeen stand Inconclusive today**
(`P137|A|tier2|*`, 17/17).

### 3.1 Group 1 — the element model alone (W1–W3)

Eight entries whose rows are index, equality, and bounded
quantifier forms over the parameter and the constructed
output. Required end standing for each: **Verified ∧
battery-exact**, Tier-1 at the re-tier.

| Entry | Its rows (stdlib/list.morph, frozen) | Carried by |
|---|---|---|
| `list_reverse` | length row; `forall i in range(len(xs)): output[i] == xs[(len(xs) − 1) − i]` | W2 index, W3 forall |
| `list_contains` | forall-implies-true row; negated-forall-implies-false row | W3 (both quantifier positions, §2.7) |
| `list_min` | empty → `None`; forall bound row over `output.unwrap()`; **exists** membership row | W3 exists arm |
| `list_max` | the same three shapes, dual | W3 exists arm |
| `list_take` | three length rows (clamp cases); the `output == xs` equality row at `n ≥ len(xs)`; index forall | W2 equality, W3 |
| `list_drop` | three length rows; the `output == xs` equality row at `n ≤ 0`; two guarded index foralls | W2 equality, W3 |
| `list_is_sorted` | short-list row; adjacent-pair foralls over `range(0, len(xs) − 1)` | W3 |
| `range` | two length rows over the constructed output; `forall i in range(len(output)): output[i] == lo + i` | W1 bridge (§2.5), W3 |

### 3.2 Group 2 — conditional on the fold-form arm (W4)

| Entry | Its row | Required end standing |
|---|---|---|
| `list_sum` | `output == fold(xs, 0, \|acc, v\| acc + v)` | **Verified ∧ battery-exact if W4 lands** |
| `list_product` | `output == fold(xs, 1, \|acc, v\| acc * v)` | **Verified ∧ battery-exact if W4 lands** |

**The W4-insufficiency rule (frozen).** W4's exit is a
measurement, not a promise (§8, Phase 142). If that
measurement shows the fold-form arm cannot discharge these
rows honestly — the §2.8 characterization asserted and the
rows still not closing, or closing only under a stronger
assertion than §2.8 allows — then `list_sum` and
`list_product` **stay Tier-2**: their standings are
unchanged, the reason is named in the Phase 142 record and
carried on their re-tier surfaces (§5) in place of an
upgrade, and gate clause 1 measures them at
Tier-2-unchanged-with-the-named-residue. There is no
partial credit and no intermediate tier (D3a). The same
rule governs `list_fold`'s fold half (§3.3).

### 3.3 Group 3 — UF composition (W5), and the two named non-obligations

The five higher-order entries, at the honesty the audit
measured. Required end standings:

| Entry | Its rows | Required end standing |
|---|---|---|
| `list_map` | length row; `forall i in range(len(xs)): output[i] == f(xs[i])` | **Verified ∧ battery-exact** (pointwise congruence + the Map characterization, §2.9) |
| `list_filter` | length bound `len(output) ≤ len(xs)`; `forall i in range(len(output)): p(output[i]) == true` | **Verified ∧ battery-exact — of exactly these two rows** (§9 R6) |
| `list_fold` | `output == fold(xs, init, f)` | **Verified ∧ battery-exact if W4 lands** (the fold form with `op = f`, W4 + W5 jointly); under the §3.2 rule it otherwise stays Tier-2 with the reason named |
| `list_all` | forall-counterexample-implies-false row; forall-holds-implies-true row | **Verified ∧ battery-exact** |
| `list_any` | forall-witness-implies-true row; forall-absent-implies-false row | **Verified ∧ battery-exact** |

**`list_filter`'s honesty clause.** Its contract is
deliberately the weakest of the five (Amendment 2 froze it
so: a length bound and a predicate-holds row — no order, no
completeness). Tier-1 for `list_filter` certifies **that
frozen contract**, discharged in full — as Tier-1 always
certifies a contract, never the English gloss of an entry's
name (D3a, §9 R6). The re-tier surfaces (§5) carry the
contract's rows, so what was proven is what a reader sees.

**The stretch — `list_binary_search`.** **Not a gate
obligation.** Its `require:` names `list_is_sorted(xs)`, so
its crown route inherits sorted's proof; it becomes
reachable only after `list_is_sorted` stands Tier-1 (§3.1)
*and* the require is assumable in the crown route (the
Phase 109 machinery, §2.11). If both hold at Phase 144 and
its rows close, Phase 144 may record it Tier-1 as a **named
bonus**, with the evidence stated; if either fails, it
stays Tier-2 and nothing in the gate notices. Its shipped
body remains the disclosed first-match scan of its 1.0
documentation, and its battery's 3 spec-silent
duplicate-target points stand exactly as frozen — the
battery-exactness measure excludes them as the battery
records them (§9 R5).

**Standing residue — `list_sort`.** Tier-2, unchanged, a
residue **named** rather than silently half-upgraded
(D6a): row 1 is `is_permutation(xs, output)`, a
`Constraint::Predicate` (§3.10's unencoded class); row 3 is
a two-binder forall (`forall i, j`) past the Phase 94
envelope. Elements are necessary for this entry, not
sufficient; both remaining walls are §7 exclusions with
their own future decisions. Its executed battery stands as
shipped. `is_permutation` the entry does not exist (removed
in 1.0) and nothing in this campaign ships it — its §3.10
half is exactly `list_sort`'s row 1.

### 3.4 The non-mover corpus

Everything not named in §§3.1–3.3 must stand exactly as it
stands on the freeze tree; movement anywhere here is a
defect, measured per phase by §4's sweep:

- **The eighteen Tier-1 standings** (the Int family's
  seven, the Char family's six, `string_is_empty`, the
  Result/Option three, `list_is_empty`) and the two
  builtins' per-layer battery standings — unchanged.
- **The 201-spec verdict sweep** (the Phase 133 pin, green
  at the Phase 136 gate): **166 Verified / 25 NO_BODY /
  3 Inconclusive (`smallest`, `sorted_probe`,
  `total_or_empty`) / 1 VerificationFailed
  (`chained_fallback`)** — byte-identical outside §3's
  named entries and the phases' named exhibits.
- **`int_gcd`'s pin** (D4a's non-mover): contract preserved
  and pinned, the faithful Euclid body executes 6/6,
  verdict Inconclusive — untouched by 1.3; its campaign is
  Phases 145–147.
- **The executed standings**: the tokenizer crowns, `sort`
  6/6 and `binary_search` 7/7 in-process, `apply_twice`
  Verified under the UF arm, the Phase 95/115/135 battery
  standings — the build is verifier-side (§2.10) and moves
  none of them.
- **The Phase 137 probe pins**: the five decline shapes and
  the 17/17 Inconclusive sweep flip **only** at the phase
  that names them, in place, with the story (§9 R10).

---

## 4. Verdict-honesty machinery

Designed here, executed by the build phases and re-run at
the gate. Three instruments, all standing from Phase 139:

**4.1 The fabricated-countermodel probe.** Built at Phase
140 (the first phase that can produce an index term) and
pinned at every later phase and at the gate. Two specimens,
a pair — the discipline is proven by the contrast, not by
either alone:

- **`oob_read` (must never Verify).** A spec whose only
  index is out of range by construction — input
  `xs: List<Int>`, contract row `output == xs[len(xs)]`,
  body the direct read. Every execution of that body
  declines (the interpreter's out-of-bounds decline), so no
  executable value satisfies the row; under a totalizing
  encoder both sides would be the same unconstrained
  `select` and the spec would Verify by reflexivity — the
  fabrication this campaign exists to refuse. Required
  standing, from Phase 140 on: **not Verified** — the row
  declines under §2.3 (the index `len(xs)` carries no
  in-range guard; `0 ≤ len(xs) < len(xs)` is false), and
  the specimen stands unencodable / Inconclusive.
- **`last_read` (the guarded sibling; must Verify once W2
  lands).** The same shape with the guard the row itself
  supplies: `require: len(xs) ≥ 1`, contract row
  `output == xs[len(xs) − 1]`, body the direct read.
  Required standing from Phase 140: **Verified** — proving
  the discipline declines the *guard's absence*, not the
  read's shape.

**4.2 Mirror agreement.** Per shape, per phase: the §2.2
battery pins the real encoder's and `encodable.rs`'s
encodability predicates equal in both directions on every
shape the phase newly accepts (binding shapes at 139;
index/equality at 140; quantifier/exists at 141; fold at
142; UF-at-elements at 143). A disagreement is a defect by
definition, fixed in the phase that finds it, never carried.

**4.3 The drift sweep.** The §3.4 sweep re-run at every
build phase and at the gate: verdict movement is confined
to the entries and exhibits the phase names; the failed set
is byte-identical outside them. A movement nobody named
stops the phase, as it has stopped phases before.

---

## 5. Package obligations at the re-tier (Phase 144)

The re-tier is EMBED_SCOPE **R11** executed: Tier-2 entries
this campaign verifies re-issue with upgraded tier data at a
lockstep release; the functions' behaviour and bytes do not
change.

**5.1 What moves, and only what moves.** For each entry
whose standing §3 moves to Tier-1: its manifest row's
`tier` / `verdictBasis` (`tier-2` / `executed-exact` →
`tier-1` / `z3-verified`), the tier lines in its JSDoc
(npm) and rustdoc (facade), the package READMEs' tier
lines, EMBEDDING.md's tier counts, Appendix A's tier line
for the entry, and the tier sentence of its `doc:` text in
`stdlib/list.morph` — the `doc:` tier sentence is tier
data, carried verbatim by the manifest discipline, so it
is updated at the source and re-carried; the contract
sentences of every `doc:` are untouched. For `list_sum`,
`list_product` and `list_fold` under the §3.2 rule, and for
every entry that does not move, the surfaces instead carry
the standing Tier-2 line with the residue named (the Phase
142 reason, for the fold three). **A function body byte
moving at the re-tier is a finding**, named — the Phase 136
stamp precedent: the re-assembly's diff surface is version
strings + tier data, nothing else.

**5.2 The npm budget — the figure stands.** The re-issued
package's unpacked mass is inside **76,800 B** — the
standing figure of Amendment 2 §A2.5, **not re-derived and
not raised**. Derivation of why it stands: the 1.2.0
package measures **70,152 B** unpacked against that budget
(headroom 6,648 B); the re-issue adds no export, no file
and no function byte — its entire delta is label bytes on
the §5.1 surfaces (tier strings, `verdictBasis` strings —
which *shrink*, `executed-exact` → `z3-verified` — tier
sentences of comparable length, and version strings). A
label-only delta is bounded by those surfaces and cannot
plausibly spend the headroom; Phase 144 measures the actual
delta and reports it. A measurement above 76,800 B **fails
gate clause 4** rather than re-basing the budget — a budget
that grows to meet its package has stopped measuring
anything (A2.5's moral, applied to this freeze itself).
The Rust crate carries no size budget (EMBED_SCOPE R8
carries). The playground bundle budget — **771,072 B**,
standing — is measured at the gate as it stands; the stdlib
asset's delta is the moved entries' `doc:` tier sentences,
reported.

**5.3 Lockstep and the owner's acts.** Version **1.3.0**
across the crate and both packages at the release
(EMBED_SCOPE R6 mechanics: `packageVersion` =
`morphicVersion` = 1.3.0, published only from the tagged
tree, under this repo's one changelog). The registry
publishes are release acts **after** a PASS, and they
remain **the owner's** (EMBED_SCOPE R5/R12 + Amendment 2
A2.6 carry): from the tagged v1.3.0 tree on the owner's PC,
`npm publish --access public` from `packages/npm` and
`cargo publish` from `packages/rust`. Named prerequisite,
not a task: **npm needs a fresh login or token on the
owner's PC** — the 1.1 granular token was revoked after use
(owner-confirmed 2026-11-23) and no standing npm credential
exists; the crates.io token stands as the owner left it.

---

## 6. The 1.3 gate (frozen text)

*This is the gate. It is ticked by measurement on the final
tree, in the M1 gate's manner; its text is amended only by a
recorded owner decision, never at the gate run.*

**The clauses:**

1. **Named entries at their frozen standings.** Every
   entry named in §3 stands at its required end standing
   on the final tree: the eight of §3.1 **Z3-Verified ∧
   battery-exact** through the production pipeline;
   `list_sum` and `list_product` Verified ∧ battery-exact
   if the fold-form arm landed, else Tier-2 unchanged with
   the Phase 142 reason named on their surfaces (§3.2);
   the five of §3.3 at their stated standings (`list_fold`
   under the same rule); `list_binary_search` and
   `list_sort` at their named non-obligation / residue
   standings; every other Tier-2 entry unchanged, its
   residue named row-by-row.
2. **Verdict honesty.** The corpus drift sweep (§4.3)
   moves only the entries and exhibits the phases named;
   the mirror-agreement battery (§4.2) passes on every
   shape the encoder newly accepts; and the
   fabricated-countermodel probe (§4.1) stands at its
   required standings — **`oob_read` not Verified, its
   guarded sibling `last_read` Verified** — no index term
   in the tree's verdicts was produced without its in-range
   guard.
3. **The higher-order five at their frozen tiers, UF
   composition measured.** The composed specimen Verifies
   (a Map-shaped body judged against `list_map`'s rows
   through the composed arms); a beyond-congruence element
   row — a value-at-a-point claim about `f` — stands **not
   Verified**; the composition claims congruence over
   elements and nothing beyond it (§2.9).
4. **Packages re-issued honestly.** Both packages
   re-issued at **1.3.0** under EMBED_SCOPE R11: tier /
   `verdictBasis` upgrades **only** where clause 1 moved an
   entry, function bytes unchanged (the re-assembly diff
   surface is version strings + tier data); the npm
   package's unpacked mass inside **76,800 B** (§5.2); the
   playground bundle inside the standing **771,072 B**; the
   cascade instrument (Codegen spec count, synthesized-
   artifact bytes) reported — the encoder build is
   native-side, so movement there is a named finding, never
   a surprise.
5. **Suites green in the three frozen configurations** on
   the final tree — the configurations being exactly those
   defined in §9 of `G4_SCOPE.md`, cited by invocation
   (baselines on the freeze tree: **809/0/29, 809/0/29,
   673/0/22** — the v1.2.0 suites plus the audit's seven
   probe teeth).

**Closing line:**

> *1.3 closes when the library's element-level entries are
> proven where their contracts allow proof — each at the
> tier its rows can honestly carry, and no verdict in the
> tree bought with an element no execution could produce.*

---

## 7. Explicit exclusions

Named out of the 1.3 milestone (D6a), each with its
measured home — an exclusion here is a routing decision,
not a dismissal:

- **§3.10 Predicate encoding** — `list_sort`'s row 1
  (`is_permutation(xs, output)` as a `Constraint::Predicate`)
  and the `is_permutation` entry question. Its own decision,
  its own campaign surface. The bounded **exists encoder
  arm** W3 needs (§2.7) is *not* this: it is an encoder form
  over the standing bounded-quantifier syntax, not a
  Predicate encoding, and it buys nothing toward §3.10.
- **Multi-binder envelope widening** — `list_sort`'s row 3
  (`forall i, j`). The Phase 94 envelope stays at one
  binder; widening it is a separate measured question.
- **Element sorts other than Int** — Char, String, Bool and
  Float elements (Char carries the Phase 133 side-condition
  class; Float is out at three layers since the stdlib
  audit), **nested lists**, and **function-typed elements**.
  1.3's element model is `List<Int>` only (§2.1, §9 R9);
  every other list keeps the standing length-only binding
  and standings.
- **The quantified-mod campaign** — `int_gcd`'s return, the
  J1/J2 judge routes (D5a), the Euclid seed family (D7a):
  Phases 145–147 under their own freeze (Phase 145), with
  D4a/D5a/D7a recorded at §1 as its standings.
- **`list_sort` and `is_permutation`** — standing residue
  (§3.3), named: Tier-2 and absent respectively, by the two
  walls above, not by this campaign's incompleteness.
- **The rejected models** — sequences (D1a's (b)) and
  bounded unrolling (D1a's (c)) are not deferred features of
  this campaign; the array model's contract (§2) is the
  whole build.
- **Emitter and package-surface changes** — none: the
  re-tier is label-only at the packages (§5.1); no emitter
  gains an arm in 1.3, and function types at the
  embed-package boundary keep EMBED_SCOPE §3's `never`
  standing.

---

## 8. The frozen phase plan (139–144)

138 is this freeze. Prices are the audit's measured ones
(A4), restated per phase. Every build phase carries §4's
instruments — the drift sweep and the mirror-agreement
battery for the shapes it accepts — and cites its suites in
the three configurations of G4_SCOPE §9.

- **Phase 139 — W1: binding + mirror.** *M.*
  Entry: this freeze. Work: §2.1's paired binding at the
  standing `fresh_sym` List sites; §2.2's mirror form;
  §2.5's bridge for literals (the concrete form's
  store-chain crossing). Exit: the length standings
  unmoved — `list_is_empty` and every length-observed row
  Verify exactly as on the freeze tree; concrete-list
  specimens Verify through the paired form; the
  mirror-agreement battery passes on the binding shapes;
  **no element row discharges yet** — the §3 sweep stands
  17/17 Inconclusive, stated as this phase's expectation,
  and the drift sweep is otherwise byte-identical.
- **Phase 140 — W2: index + equality arms, guard
  discipline.** *M.* Entry: Phase 139. Work: §2.3's guarded
  `select`; §2.4's equality form; §2.6's Append store form
  and Concat decline. Exit: **the fabricated-countermodel
  probe (§4.1) is built and at standing** — `oob_read` not
  Verified, `last_read` Verified; guarded index specimens
  Verify; the equality rows of `list_take`/`list_drop`
  encode (their full standings land with W3's quantifiers);
  mirror agreement on the index/equality shapes; drift
  confined to the named specimens.
- **Phase 141 — W3: quantifier arms (forall-over-range,
  exists).** *M.* Entry: Phase 140. Work: §2.7's two arms in
  both encoders + mirror. Exit: **the eight of §3.1 stand
  Verified ∧ battery-exact** (reverse, contains, min, max,
  take, drop, is_sorted, range — `range`'s crown is the
  bridge's exhibit, its loop-carried element facts closing
  its index row); mirror agreement on the quantifier
  shapes; drift confined to the eight.
- **Phase 142 — W4: the fold-form arm.** *M.* Entry: Phase
  141. Work: §2.8's base + step characterization in the
  encoder + mirror, for the closed-lambda operations.
  Exit: **the measurement** — `list_sum` and `list_product`
  stand Verified ∧ battery-exact under exactly §2.8's
  characterization, **or** the insufficiency is measured
  and recorded with its reason, the two stay Tier-2 under
  §3.2's rule, and the phase exits on the measurement
  either way. `list_fold`'s fold half is ready for Phase
  143 in both outcomes (its standing follows the same
  rule).
- **Phase 143 — W5: UF composition over elements.** *S–M.*
  Entry: Phase 141, plus Phase 142's verdict (composition
  for map/filter/all/any does not need the fold form;
  `list_fold` does). Work: §2.9's application-at-elements
  and the four body-side characterizations. Exit: **the
  five of §3.3 stand at their frozen standings** (map,
  filter, all, any Verified ∧ battery-exact; fold per the
  joint rule); the composed specimen Verifies; the
  beyond-congruence specimen stands not Verified; drift
  confined to the five.
- **Phase 144 — Re-tiering + 1.3 gate + merge + v1.3.0.**
  *M.* Entry: Phase 143. Work: §5's re-issue under the
  crown checks (both packages regenerated, tier data only);
  the doc surfaces (Appendix A, EMBEDDING.md, the moved
  entries' `doc:` tier sentences); the §6 gate run
  clause-by-clause on the final tree; on PASS, the stack
  merge and the **v1.3.0** release in the standing form
  (the registry publishes following the tag as the owner's
  release acts, §5.3). Exit: gate verdict recorded per
  clause; a failing clause stops the merge, as every gate
  before this one has.

**Total ≈ XL, as the audit confirmed** — four M sub-walls,
one S–M composition, the M re-tier/gate, and this freeze:
the stdlib audit's one XL, split into named, separately
priced walls (A4).

---

## 9. Resolutions (freeze-time, recorded)

The audit left the following open; each is resolved here,
conservatively, from the audit's evidence, and recorded so
the freeze is interpretable. None is a silent guess: each
states its basis.

- **R1 — The equality form is pointwise-over-range, not
  raw array equality.** §2.4 freezes
  `len-eq ∧ ∀ i ∈ [0, len). select-eq`. The audit's D1a
  shorthand ("array equality conjoined with length
  equality") is *not* the frozen form: raw array equality
  ranges beyond the length, so beyond-length content —
  which no execution observes — could decide a row in the
  refutation direction (a true `output == xs` refuted
  because two constructions of the same list differ past
  the end). Basis: audit A2(ii)'s own beyond-length
  warning. Raw array equality remains available to an
  implementation only under a canonical-base construction
  that makes the forms coincide on every producible value;
  the *form* is frozen, the mechanism is the build's.
- **R2 — The `ListLen` arm in `stmt_sym_identical` is not
  a semantics.** A reader of `src/verify/verifier.rs` will
  find `(Sym::ListLen(x), Sym::ListLen(y)) => term_eq(x, y)`
  and may mistake it for a standing length-equality
  encoding of list equality. It is the statement-mode merge
  fast path's structural-identity test (AST node identity
  of the *same* length term), not the equality encoder —
  the verdict path's `sym_eq` declines symbolic list
  equality today, exactly as the audit probed
  (`P137|A|decline`, equality row). Basis: code reading at
  freeze time. Recorded so no build phase treats that site
  as a totalization to preserve or extend; §2.4 is the only
  equality form this campaign builds.
- **R3 — Append is a store; Concat declines.** §2.6.
  Basis: audit A2(ii) (appending to a symbolic array is a
  store-chain, not a native op) and A2 risk (3) (a
  totalization without an element story is a named residue
  or nothing), measured against the frozen corpus: no §3
  contract row needs a symbolic Concat — the corpus's
  appends are over constructed lists (the bridge's chain,
  §2.5) and its builders work by index. If a build phase
  finds a corpus row that needs one, the finding is
  reported and the freeze amended by the owner — never
  totalized silently.
- **R4 — The W4-insufficiency rule.** §3.2: if the
  fold-form measurement fails, `list_sum`, `list_product`
  (and `list_fold`, jointly) stay Tier-2 with the reason
  named in the Phase 142 record and on their re-tier
  surfaces; the gate measures exactly that. Basis: D3a
  (no intermediate tier) + the audit's conditional pricing
  of W4 (A3/A4: sum/product are "Tier-1 reachable,
  conditional" — the conditionality is the freeze's to
  state, and it is stated as a rule, not discovered at the
  gate).
- **R5 — `list_binary_search` is a stretch, not a
  promise.** §3.3: not a gate obligation; a Phase 144 bonus
  only, recorded with evidence if both of its conditions
  (sorted Tier-1 first, the require assumable in the crown
  route) measurably hold. Its first-match disclosure and
  its 3 spec-silent battery points stand as frozen in
  1.0/1.1 — Tier-1 for the scan body, if it ever lands,
  certifies the search rows with the require assumed, and
  the spec-silent points remain excluded from the
  battery-exactness measure exactly as the battery records
  them. Basis: audit A3's stretch pricing.
- **R6 — Tier-1 certifies the frozen contract, not the
  name.** Stated for `list_filter` (§3.3) and general:
  every entry's Tier-1 at the re-tier is Verified ∧
  battery-exact **against its contract as frozen in
  `STDLIB_SCOPE.md` and its amendments** — the rows are the
  claim. Where a contract is deliberately weaker than its
  entry's name suggests (filter's two rows), the re-tier
  surfaces carry the rows, so the proof's content is what a
  reader sees. Basis: Amendment 2's deliberate filter
  contract + D3a.
- **R7 — The npm budget figure stands at 76,800 B.** §5.2:
  not re-derived, not raised; the derivation (label-only
  delta inside 6,648 B of measured headroom) is stated
  there, and a Phase 144 measurement above the figure fails
  clause 4 instead of re-basing it. Basis: A2.5's
  anti-inheritance moral + the 1.2.0 package's measured
  70,152 B actual. The facade carries no budget (EMBED_SCOPE
  R8 carries); the playground bundle budget stands at
  771,072 B and is measured, not derived, at the gate.
- **R8 — What remains the owner's (prerequisites, not
  decisions).** No open item in this freeze requires an
  owner decision: D1a–D7a settled the decision space the
  audit opened. The execution prerequisites are §5.3's
  (registry publish credentials at Phase 144's release
  acts; a fresh npm login/token, the 1.1 token having been
  revoked). And one standing is the owner's to freeze
  later: the quantified-mod campaign's scope (Phase 145),
  for which §1 records D4a/D5a/D7a as the adopted
  standings, not as frozen text.
- **R9 — The paired binding engages for `List<Int>`
  only.** §2.1: every other element sort keeps `Sym::ListLen`
  and its exact standings — no verdict for a non-Int-element
  list moves in either direction in 1.3, and the drift sweep
  (§4.3) measures that as part of the non-mover corpus.
  Basis: D6a + the audit's corpus fact (the Tier-2 surface
  is `List<Int>` throughout).
- **R10 — The audit's probes retire in place, by name.**
  The `P137|` pins are this campaign's baseline instrument
  (the Phase 127 probes' role in G4): a pin flips only at
  the phase that names it, in place, with its story
  recorded; `LIST_ENCODER_AUDIT.md` itself is a frozen
  source and is never edited. Basis: the G4 campaign's probe
  discipline (Phases 129–133 flipped the P127 pins in place,
  each with its reason at the site).
