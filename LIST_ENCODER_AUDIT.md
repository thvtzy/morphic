# LIST_ENCODER_AUDIT.md — Phase 137: the §3.9 List element encoder + quantified-mod / `int_gcd`

The scope audit for the two headline post-1.2 campaigns, in the
Phase 106 / 118 / 127 tradition: probes first
(`tests/phase137_encoder_audit.rs`, 7 probes, every claim below a
`P137|` line), priced routes, a decision list with recommendations,
gate sketches, phase-plan sketches. Audit only — no production code
changed. Tree: master @ `1c033da` (v1.2.0); suites 802/802/666;
library 35 entries + 2 builtins (Tier-1 18 / Tier-2 17).

**Name choice.** The deliverable is named for Half A because §3.9 is
the larger campaign and the file sits beside `ENCODER_EXTENSION_DESIGN.md`
(the Phase 90 encoder audit); Half B is a full second half, not an
appendix — the combined scope is stated here so a future reader is
not surprised by gcd inside a "list" file.

---

## Half A — §3.9, the List element encoder

### A1. The wall, precisely

A List-typed parameter binds **length only**. The binding site is
`fresh_sym` in `src/verify/verifier.rs`:

```rust
TypeRef::List(_) => Sym::ListLen(z3::ast::Int::new_const(format!("{path}.len")))
```

`Sym::ListLen(Int)` carries one Int term; no element term exists.
The mirror discipline binds the same way (`Abs::ListLen`,
`src/verify/encodable.rs`). Concrete list *literals* are a different
story — they bind as `Sym::List(Vec<Sym>)` with real elements, and
the machinery over them (indexing, elementwise ite, Append/Concat)
already exists. **The missing piece is the parameter binding, not
the list-value machinery** — measured: a body that builds a concrete
list and indexes it encodes and **Verifies** today
(`P137|A|concrete|encodable=true|verify=Verified`), while a
length-only contract over a parameter also Verifies
(`P137|A|control|len_only=Verified`).

Every element-level decline shape, probed by name
(`P137|A|decline|...`):

| Shape | Encodable | Verdict |
|---|---|---|
| Indexing a symbolic list (`output == xs[i]`) | **false** | Inconclusive |
| Equality of two symbolic lists (`xs == ys`) | **false** | Inconclusive |
| Element `forall` as the contract (contains-shape) | — (row unencodable) | Inconclusive |
| Element `exists` row (list_min's membership row) | — | Inconclusive |
| Map/Filter/Fold **body** nodes (G4_SCOPE §6 re-filing) | **false**, mirror agrees | Inconclusive |

And the wall stated as one sweep: **all 17 Tier-2 entries'
shipped, battery-exact bodies stand Inconclusive**
(`P137|A|tier2|*`, 17/17). That sweep is the campaign's baseline
instrument: progress is entries leaving the line, one by one, loudly.

Totalizations that interact: `sym_len` reads the length term;
Append over `ListLen` grows it (n → n+1); `sym_ite` merges two
`ListLen`s by merging lengths. Any element model must keep every
one of these honest — a merged or appended list's *elements* are
exactly where a careless model fabricates (see A2 risks).

### A2. Candidate element models

The pinned z3 crate is **0.20**; both candidate sorts exist in it
(measured in the vendored source: `Sort::seq` at sort.rs:119,
`Sort::array` at sort.rs:102; `ast::Seq` and `ast::Array` are
first-class AST types).

**(i) Z3 Sequences — `Seq<Int>` per list.**
A list parameter binds as one sequence term; length is `seq.len`,
indexing is `seq.nth` (partial — out-of-range is unconstrained,
which matches nothing in Morphic: the interpreter declines OOB),
equality is sequence equality (exact), concatenation/append are
native. Element quantifiers become quantifiers over indices into
`nth`, or over subsequences.
*Can express:* every standing corpus form — index rows, equality
rows (take/drop's `output == xs`), forall-over-range rows (as
forall over an index Int with a length guard — the Phase 94
envelope's shape, one level down), fold/map bodies only via an
additional recursion/UF layer (sequences alone do not evaluate a
fold).
*Cannot / risks:* `nth` out-of-range unconstrainedness is the
Char-lesson hazard in new clothes: a proof step that reads
`xs[i]` without the in-range side condition the spec's own rows
carry can be refuted — or worse, *verified* — through a value no
Morphic execution can produce. The discipline that saves it is
the one the encoder already uses elsewhere: **index terms are
only produced where the row's own guard proves the index in
range; otherwise the row declines** (Inconclusive, never a
surrogate). Sequence reasoning with quantifiers is also the
solver's least predictable fragment — verdict *timeouts*
(Unknown) will appear before verdict *errors* do; the campaign
must budget solver time per row and treat Unknown as
Inconclusive, as today.

**(ii) Arrays + length — `Array<Int, Int>` + a separate length term.**
The parameter binds as an array term plus today's `ListLen` term;
indexing is `select`, equality is array equality *conjoined with
length equality* (array extensionality alone equates lists of
different lengths whose arrays agree on all indices — including
beyond the length — so the length conjunct is load-bearing, and
an array value beyond the length is a second fabrication surface:
two lists equal on `[0, len)` but differing beyond it are the
same Morphic list and different arrays; the model must never let
a beyond-length `select` decide a row).
*Can express:* index rows and equality (with the conjunct);
forall-over-range rows directly (forall i, 0 ≤ i < len → …) —
the most natural fit for the corpus's actual row shapes, which
are overwhelmingly index-quantified, not subsequence-shaped.
*Cannot / risks:* Append/Concat become `store`-chains / a UF —
appending to a symbolic array is not native; the standing
`ListLen` Append totalization (n → n+1) has no array analogue
short of a fresh array + a pointwise axiom, which is a
quantified axiom of exactly the class this audit prices as
fragile in Half B. List *literals* and built lists (today's
`Sym::List`) need a bridge into array form (a chain of stores
over a constant array — mechanical, S).

**(iii) Bounded unrolling** (a third candidate, named to be
rejected with evidence): expand a symbolic list into N element
variables for a fixed N. The Phase 90 statement-mode probe
already measured this class of answer for loops: it decides
concrete points and **can never Verify a symbolic row** — it is
a judging aid, not an encoder. Rejected as the model; it remains
what executed batteries already do.

**Mirror discipline.** Whichever model is chosen, `encodable.rs`
must learn exactly the same slice (`Abs::Seq` or `Abs::ArrayLen`
beside `Abs::ListLen`), and the Phase 133-style agreement battery
(real vs pure predicate, both directions, per shape) is the
campaign's standing instrument. A shape the real encoder accepts
and the mirror declines (or vice versa) is a defect by
definition, as it was for the UF arm.

**Fabricated-verdict risks, named (the Phase 133 Char lesson
applied):** (1) unconstrained OOB `nth`/`select` values deciding
rows — closed by the guard discipline above; (2) beyond-length
array content deciding equality — closed by the length conjunct +
never selecting beyond a proven bound; (3) Append/Concat
totalizations that invent elements — the standing n → n+1 length
totalization must gain an element story or the affected bodies
stay unencodable (a *named* residue, not a silent totalization);
(4) solver Unknown dressed as either verdict — the verdict layer
already maps Unknown → Inconclusive; the campaign must not add a
path around it.

### A3. The payoff table (per Tier-2 entry)

Measured basis: each entry's frozen contract rows (stdlib/list.morph)
classified against A1's decline shapes + the sweep. "Element model
alone" = A2's binding + index/equality/quantifier arms, no UF
composition, no §3.10 work.

| Entry | With element model alone | Additionally needs |
|---|---|---|
| `list_reverse` | **Tier-1 reachable** — index rows only | — |
| `list_contains` | **Tier-1 reachable** — forall + a negated-forall row | — |
| `list_sum` | **Tier-1 reachable, conditional** — contract is a `fold(...)` *constraint form*; the encoder must evaluate the constraint-side fold over a sequence/array (a fold *interpreter* in the encoder, or the row re-expressible — the contract is frozen, so the encoder learns it) | fold-form arm (priced in A4) |
| `list_product` | same as `list_sum` | fold-form arm |
| `list_min` | **Tier-1 reachable** — forall bound rows + the `exists` membership row (the exists arm is part of the build, §3.10's small half) | exists arm |
| `list_max` | same as `list_min` | exists arm |
| `list_take` | **Tier-1 reachable** — length rows + index rows + one equality row (`output == xs` at the clamp) | — |
| `list_drop` | **Tier-1 reachable** — same shapes as take | — |
| `list_is_sorted` | **Tier-1 reachable** — adjacent-pair forall rows over indices | — |
| `range` | **Tier-1 reachable** — output is *constructed*, its rows are length + index over the output; needs the constructed-list bridge (literal/store chain) meeting the parameter model | constructed-list bridge |
| `list_binary_search` | **Walled regardless (contract side)** — its `require:` names `list_is_sorted(xs)`, so it inherits sorted's proof; and its shipped body is the disclosed first-match scan, whose *index* the contract deliberately leaves spec-silent — Tier-1 for the scan body would prove the search rows, which are encodable with elements + the require assumed (Phase 109 machinery) — so: reachable **only after** `list_is_sorted` is itself Tier-1 *and* the require is assumable in the crown route; priced as a stretch row, not a promise | sorted Tier-1 first |
| `list_sort` | **Walled regardless in 1.3 scope** — row 1 is `is_permutation(xs, output)`, a `Constraint::Predicate` (§3.10's unencoded class), and row 3 is a **two-binder** forall (`forall i, j`) past the Phase 94 single-binder envelope. Elements are necessary, not sufficient | §3.10 Predicate arm + multi-binder envelope |
| `list_map` | Not with elements alone | **UF composition**: rows quantify `f(xs[i])` — UF arm (Phase 133) applied at element terms the element model produces; the two arms must compose (UF range Int ✓, domain = element terms) |
| `list_filter` | Not with elements alone — and its contract is the weakest of the five (length bound + predicate-holds rows; it does not state order/completeness, deliberately, per Amendment 2) — even composed, what Verifies is exactly those rows | UF composition |
| `list_fold` | Not with elements alone — its contract *is* a fold form over a UF | fold-form arm + UF composition |
| `list_all` | Not with elements alone | UF composition (predicate applications at elements) |
| `list_any` | Not with elements alone | UF composition |

Count: **9 entries Tier-1-reachable on the element model alone**
(reverse, contains, min, max, take, drop, is_sorted, range = 8,
plus sum/product conditional on the fold-form arm = 10 with it),
**5 need UF composition** (the HO five), **1 stretch**
(binary_search), **1 walled** (sort). `is_permutation` the *entry*
does not exist (removed in 1.0, §2.7) — its §3.10 half is exactly
list_sort's row 1; nothing in this campaign ships it.

### A4. Price

The stdlib audit called §3.9 **the one XL**. **Confirmed as XL in
aggregate, but it splits into named sub-walls with separate
prices**, and the split is the campaign's real shape:

| Sub-wall | Price |
|---|---|
| W-§3.9-1 Binding + mirror (parameter binds as the chosen model; `encodable.rs` parity; concrete-list bridge) | M |
| W-§3.9-2 Index + equality arms under the guard discipline (incl. Append/Concat element stories or their named residue) | M |
| W-§3.9-3 Quantifier arms: forall-over-range at element level + the exists arm | M |
| W-§3.9-4 The fold-form arm (constraint-side `fold(...)` evaluation over the model) — gates sum/product/fold | M |
| W-§3.9-5 UF composition (Phase 133's arm applied at element terms) — gates the HO five | S–M |
| W-§3.9-6 The re-tiering itself: per-entry crown re-runs, manifest/package regeneration, doc/tier surfaces (EMBED_SCOPE R11's label-upgrade mechanism) | S–M |

Phase sketch (Half A): **138** freeze (model choice ratified) ·
**139** W1 binding+mirror · **140** W2 index/equality · **141** W3
quantifiers+exists · **142** W4 fold-form · **143** W5 UF
composition · **144** re-tiering + gate + merge + release (1.3).
Each build phase carries the drift sweep (no verdict moves outside
the entries it names) and the mirror-agreement battery.

---

## Half B — quantified-mod and `int_gcd`'s return

### B1. The blocker, re-measured on the current tree

The Phase 111 pin, re-run as probes (`P137|B|...`) — **both halves
stand exactly as pinned; Phases 96 (invariants) and 133 (UF arm)
moved neither**:

- **Execution is not the gap:** the faithful Euclid body executes
  **6/6** (`P137|B|gcd|executed=6/6`).
- **Half (ii), the judge:** with the full preservation invariant
  (four quantified divisibility-preservation forall rows + sum
  bound), the verdict is **Inconclusive**
  (`P137|B|gcd|full_invariant=Inconclusive`) — Z3 returns Unknown
  on the consecution obligations, whose shape is:
  `∀d. d | x ∧ d | y → d | (x mod y)` and its converse direction,
  universally quantified over the divisor with `%` on symbolic
  operands on both sides.
- **The preservation rows are load-bearing:** with the plain
  invariant (`x >= 0; y >= 0`) the same body is
  **VerificationFailed** (`P137|B|gcd|plain_invariant`) — refuted,
  not merely unproven. Any route that weakens the invariant to
  something dischargeable-but-insufficient is refuted by this pin.
- **Half (i), the seed:** a census of the synthesis layer finds
  **zero** Euclid/gcd mentions in `templates.rs` + `engine.rs`
  (`P137|B|seed|euclid_mentions=0`) — no seed family constructs a
  While+Mod Euclid shape, so even a crowned hand body would leave
  the pipeline unable to *find* gcd on its own.

**What DOES discharge today** (the wall's exact contour):
unquantified symbolic mod reasoning Verifies — a body `a % 3`
against its range rows is **Verified** (Z3's native div/mod
totalization carries ground-modulus facts); and a **concrete** gcd
instance (both arguments pinned by `require:` rows, the divisor
forall bounded at 31, every mod ground) is **Verified**
(`P137|B|mod|symbolic_mod_range`, `P137|B|mod|concrete_gcd`).
So the wall is neither `%` nor foralls nor gcd's statement — it is
**quantified divisibility preservation over symbolic operands**:
`∀d` composed with `%` where the modulus is itself a loop variable.

Constraint forms each half needs, stated exactly:
- The *contract's* greatest-ness row: one bounded forall over
  divisors with `%` atoms at symbolic `a`, `b` and bound `d`
  (already in the frozen contract; the parser discards the
  `range(...)` bound, so the row is read unbounded with the
  boundary rows pinning the zero cases — Phase 111's record).
- The *invariant's* consecution: four forall rows of the shape
  above + initiation/exit over the same atoms. This is the
  obligation class that returns Unknown.

### B2. The fix, priced per half

**Judge side — three candidate routes, honesty risks named:**

(J1) **Quantifier-instantiation discipline for dvd rows (S–M).**
Teach the statement judge a dedicated instantiation pass for
preservation-shaped foralls: instantiate the divisor quantifier
at the finitely many divisors the *proof* needs (the loop's own
`x`, `y`, `x mod y` terms) rather than leaving Z3's e-matching to
wander over `%`. This is a heuristic *completeness* aid: it can
only turn Unknown into a genuine proof or a genuine countermodel
(the instantiated obligations are still checked by Z3 in full) —
**no fabricated-verdict risk**, because nothing is assumed, only
instantiated. Risk is the ordinary one: it may still not close
(the instantiation set is a guess), in which case the verdict
stays Inconclusive and the campaign has spent S–M learning so.

(J2) **A dvd-normalization encoding (M).** Rewrite `a % d == 0`
atoms into the existential form `∃k. a == d * k` (and `!=` into
its negation) before encoding, giving the solver linear-arithmetic
shape instead of mod-of-symbolic shape. Honesty: the rewrite is
*equivalence-preserving only with the divisor-nonzero side
condition* — Morphic's `%` totalizes `a % 0` (the interpreter's
standing totalization), while `∃k. a == 0 * k` is `a == 0`. Rows
must therefore carry `d == 0` as an explicit disjunct — the frozen
contract and invariant already spell exactly that disjunct
(`d == 0 || ...`), which is why this route is viable at all; an
encoder that dropped the disjunct would fabricate. The mirror
must learn the same rewrite or decline the same rows.

(J3) **A gcd-specific decision route (M, rejected as primary).**
Pattern-match the Euclid shape (While + Mod + swap) and discharge
it with a bespoke verified-descent argument outside Z3. Honesty
risk is structural: a special-case prover is a second verdict
authority; every shape it mis-recognizes is a potential
fabrication, and its boundary (which bodies qualify) becomes a
permanent honesty surface. It also buys *nothing else* (B3).
Kept as the fallback if J1+J2 both fail measurement.

Recommended judge route: **J1 first (measure), J2 if J1 is
insufficient** — both are general machinery, both preserve the
one-verdict-authority discipline.

**Seed side (S).** One seed family in the standing discipline:
a `euclid` family emitting the Phase 111 body shape (Alloc abs,
While y≠0 with the Mod swap, Return x) parameterized on the
input names, with the full preservation invariant attached as
the candidate's invariant text — the family is only worth
building *after* the judge can crown its product (otherwise it
generates candidates the pipeline must score executed-only,
which the hand body already provides). Price S once the judge
half lands; wasted if built before.

**gcd's tier on return:** the 1.0-era contract aimed **Tier-1**,
and this audit **confirms Tier-1 as the return tier** — the
contract is Int-only, its rows are the J1/J2 obligation class,
and a Tier-2 return (executed-exact hand body) would add nothing
the Phase 111 pin doesn't already prove (the body already
executes 6/6 today). gcd returns Verified or it does not return.

### B3. What else quantified-mod unlocks

Measured against the corpus + stdlib: **nothing else found.**
No other shipped entry or corpus spec carries a quantified
divisibility-preservation obligation (the corpus's other forall
rows are index/element rows — Half A's — or the Phase 94
constant-envelope class, already discharged). The unlock is
gcd-shaped specs as a *class* a user could write (any
divisibility-invariant loop), not a second named entry. Stated
plainly so the campaign is not sold on a phantom payoff table:
**Half B's payoff is one entry, `int_gcd`, at Tier-1, plus a
spec class.**

---

## Synthesis

### S1. Combined or separate?

**Separate campaigns, §3.9 first** — reasons:
- The two halves share almost no machinery: Half A is a binding +
  term-model build in the verifier; Half B is quantifier
  discipline over Int rows. Neither's probes, arms, or mirror
  changes are inputs to the other.
- Their risks differ in kind: A's is fabrication-via-model
  (managed by the guard discipline); B's is incompleteness
  (Unknown) with a refutation pin guarding the other direction.
  One gate would blur two honesty stories.
- A's payoff is 9–15 entries and a milestone (1.3); B's is one
  entry — B fits *inside* a later milestone as a named half, or
  rides after A's gate without blocking it.
- What they DO share: the audit (this file), the drift-sweep
  discipline, the mirror-agreement discipline, and the
  re-tiering machinery of W-§3.9-6 (gcd's return re-uses the
  package/manifest regeneration route A builds). Shared
  instruments, separate builds.

**Order:** §3.9 campaign (1.3) → quantified-mod (gcd's return,
its own small campaign, 1.4-shaped or folded into the next
milestone's scope by the owner's call at its freeze).

### S2. Decisions (recommendations in the house style)

**D1 — Element model.** (a) **Arrays + length** (`Array<Int,Int>`
+ the standing length term, equality conjoined with length
equality). (b) Sequences. (c) Bounded unrolling.
*Recommendation: (a)* — the corpus's rows are index-quantified,
not subsequence-shaped; arrays make those rows direct, keep
today's `ListLen` term as the length half, and confine the new
fabrication surface to two named, closable points (beyond-length
selects, append axioms). (b) is the honest runner-up: native
concat/append, but `nth`'s OOB unconstrainedness touches *every*
index row, and sequence+quantifier solving is the least
predictable fragment. (c) is rejected by Phase 90's measurement
(it can never Verify a symbolic row).

**D2 — Campaign structure.** (a) Two campaigns, §3.9 first, gcd
after. (b) One combined campaign, one gate. (c) gcd first.
*Recommendation: (a)* — per S1; (c) spends the small campaign's
risk budget before the milestone-sized one and delays 15
entries for 1.

**D3 — The Tier-1 bar for upgraded entries.** (a) Full Verified
(the standing Tier-1 definition) or the entry stays Tier-2.
(b) A named intermediate standing ("Verified modulo the element
model's axioms"). (c) Upgrade on executed evidence + model
review.
*Recommendation: (a)* — (b) is a tier the packages would have to
explain forever and the manifest cannot state honestly in one
field; the project's whole wedge is that Tier-1 means *proven*.
An entry whose rows don't close stays Tier-2, named, no shame
in it (list_sort may land exactly there).

**D4 — gcd's tier on return.** (a) Tier-1 or no return.
(b) Tier-2 return acceptable. *Recommendation: (a)* — per B2:
Tier-2 adds nothing over today's pin.

**D5 — The judge route for quantified-mod.** (a) J1
instantiation discipline, then J2 dvd-normalization if measured
insufficient. (b) J2 first. (c) J3 gcd-specific route.
*Recommendation: (a)* — cheapest general machinery first, each
step measured against the two pins (full invariant must Verify;
plain invariant must stay refuted).

**D6 — Scope exclusions for §3.9.** (a) Exclude: §3.10 Predicate
encoding (list_sort's row 1 — its own decision), multi-binder
envelope widening (list_sort's row 3), nested lists / non-Int
element sorts in 1.3 (the corpus's Tier-2 surface is `List<Int>`
throughout; Char-element lists carry the Phase 133 side-condition
class), function-typed elements. (b) Include §3.10. (c) Include
all element sorts.
*Recommendation: (a)* — 1.3's gate stays measurable; sort is
named as the standing residue, not silently half-upgraded.

**D7 — The seed family (Half B).** (a) Build the Euclid family
after the judge half lands, in the same campaign. (b) Build it
first. (c) No family — ship the hand body.
*Recommendation: (a)* — (c) repeats 1.0's D7a scope-around as a
permanent answer for an entry whose whole point is the pipeline
crowning it; (b) generates uncrownable candidates.

### S3. Gate sketches

**§3.9 campaign gate (1.3), draft clauses:**
1. Every entry named Tier-1 in the campaign freeze stands
   Z3-Verified ∧ battery-exact through the production pipeline
   on the final tree; every other Tier-2 entry's standing is
   unchanged and its residue named row-by-row.
2. Verdict honesty: the corpus drift sweep moves only the
   entries the freeze names; the mirror-agreement battery
   passes on every shape the encoder newly accepts; no index
   term is ever produced without its in-range guard (pinned by
   a fabricated-countermodel probe: a spec whose row is false
   only through an OOB read must not Verify).
3. The HO five stand at the tiers the freeze names with the UF
   composition measured, not asserted (a composed specimen
   Verifies; a beyond-congruence element row stays
   Inconclusive).
4. Packages re-issued at the lockstep version with upgraded
   `verdictBasis` rows only where clause 1 moved an entry
   (EMBED_SCOPE R11); bundles inside budgets.
5. Suites green in the three frozen configurations.
*Closing line (draft):* "1.3 closes when the library's
element-level entries are proven where their contracts allow
proof — each at the tier its rows can honestly carry, and no
verdict in the tree bought with an element no execution could
produce."

**Quantified-mod gate, draft clauses:**
1. `int_gcd` stands Tier-1: Z3-Verified ∧ 6/6 battery-exact
   through the production pipeline, the Euclid seed family
   crowning the faithful body (not only the hand body judged).
2. The refutation pin holds: the plain-invariant body stays
   VerificationFailed; a wrong-body Euclid variant is refuted
   or Inconclusive, never Verified.
3. Drift sweep: no verdict outside gcd-shaped specimens moves.
4. Packages re-issued with `int_gcd` as the 36th entry at
   Tier-1 (manifest row, both hosts' batteries at 100%).
5. Suites green in the three frozen configurations.
*Closing line (draft):* "int_gcd returns when the judge can
prove the loop that computes it — divisibility preserved at
every step, stated by the spec, discharged by the solver, and
crowned by the pipeline that ships it."

### S4. Phase-plan sketch

| Phase | Work | Price |
|---|---|---|
| 138 | §3.9 freeze (D1–D7 ratified for Half A; gate text frozen) | S |
| 139 | W1 binding + mirror (arrays+length) | M |
| 140 | W2 index + equality arms, guard discipline | M |
| 141 | W3 quantifier arms (forall-over-range, exists) | M |
| 142 | W4 fold-form arm | M |
| 143 | W5 UF composition over elements | S–M |
| 144 | Re-tiering + 1.3 gate + merge + release | M |
| 145 | Quantified-mod freeze + J1 build (measure; J2 if needed) | S + M |
| 146 | Euclid seed family + gcd re-tiering | S |
| 147 | Quantified-mod gate + merge + release | S–M |

Half A total ≈ **XL** (confirmed, split per A4). Half B total ≈
**M** (J1 path) to **L** (if J2 is also needed).
