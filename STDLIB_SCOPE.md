# STDLIB_SCOPE.md — Phase 108: the frozen 1.0 standard-library scope + gate

The ratification + freeze document for the roadmap target **1.0 —
Production-ready, verified standard library**. Source material:
[`STDLIB_AUDIT.md`](STDLIB_AUDIT.md) (Phase 106 — the measured
family analysis of its §2, the priced blockers of its §3, the
packaging analysis of its §4, the gate sketch of its §5).

**Ratification.** On 2026-11-22 the owner ratified the audit's
recommended package whole — *"Gas rekomendasi audit — D1a sampe
D8a semua"* — adopting decisions **D1a–D8a** as stated in §1
below. This document freezes two things, and later phases tick
them by measurement only:

- **the scope** (§2): every 1.0 entry named, with signature,
  edge semantics, and tier. Nothing unnamed ships; nothing named
  is silently dropped — a dropped or re-tiered entry is a gate
  amendment, recorded.
- **the gate** (§3): the audit's 7-clause sketch finalized into
  gate text in the M1-gate style, closing line included.

Docs only: this phase changes no production code, no spec, and
no suite. The campaign that executes against this freeze is
mapped in the ROAD Phase 108 note (Phases 108–115; the audit's
plan numbering shifted by one when Phase 107 became the
north-star pin).

---

## 1. The ratified decisions (D1a–D8a)

**D1 — Scope: core.** (a) A core library of ~25–30 entries
(Int, Char, String basics, List per D7, Result/Option helpers)
ships as 1.0. Rejected: (b) broad scope (all 20 Appendix A
names + full String/Float) — the audit measured it unachievable
at the M1 standard today at any reasonable price; choosing it
would have chosen the Float + F5 + G4 campaigns too. *Reason in
one line: 1.0 is what the machinery can hold at the standard,
not a transcription of Appendix A.*

**D2 — Packaging: hybrid, prelude-delivered.** (a) A tiny
builtin core (what exists: `len`, `int_to_str`; new arms only
where a layer arm is unavoidable) plus bundled `.morph` library
modules merged by the compiler into every user module's index
as a prelude, at the SpecIndex construction point that already
exists — identical in CLI, API and playground. Rejected: (b)
all builtins — XL in aggregate by the Phase 102 precedent (one
builtin cost arms in five layers plus a bootstrap cascade, and
the Codegen corpus would churn per library addition forever);
(c) bundled modules with user-facing imports in 1.0 — pays the
import design now for no library benefit over the prelude (the
audit's probe: `import` parses and is then ignored; nothing
resolves, loads, or merges it). *Reason in one line: the
prelude merge is S–M, rides no cascade, and does not foreclose
real imports later.*

**D3 — Preconditions: close the §3.1 gap first.** (a) The
verifier learns to assume `require:` preconditions in both
judges (expression + statement), the encodable mirror updated
in lockstep, with teeth — Phase 109, before any entry is built.
Rejected: (b) scoping around it with total/implication-style
specs — possible, but the library's contracts would stop
reading like contracts. *Reason in one line: the gap is S–M to
close and taxes every precondition-bearing entry (clamp, div,
binary search, slicing) until it lands.*

**D4 — Float: out of 1.0.** (a) No Float entries ship.
Rejected: (b) Float in — it is blocked at three independent
layers (spec source has no FloatLit arm, so decimal literals
are a parse error; Float parameters bind Opaque in the
verifier; F3 formatting has no rendering in any layer), priced
parser S + encoder L + F3 L, and is its own campaign later.
*Reason in one line: three walls, none of them a library
family's business.*

**D5 — F5 / String transforms: out; basics only.** (a) 1.0
ships String basics; reverse/case/replace/split/join and the
per-char transform class do not ship. Rejected: (b) F5 as a
language feature in 1.0 (L–XL — there is no per-character
iteration or char→string conversion in the expression language,
so such bodies cannot even be stated); (c) shipping 2–3 named
transforms as builtins at per-entry cascade cost — no specific
transform was judged indispensable, so none is named.
*Reason in one line: the basics verify today (probe:
`string_is_empty` Verified); the transforms are a language
project, not a library family.*

**D6 — Higher-order entries (G4): out of 1.0.** (a) No
map/filter/fold/all/any entries ship; **G4 (interpreter
function values) is named the first post-1.0 build**, since
1.1's embedders will ask for map/filter before anything else.
Rejected: (b) G4 in 1.0 — L for function values plus L for the
encoder arms, and it re-opens encoder scope mid-gate; the
entries' own tests cannot even bind today (lambda inputs are
unbindable). *Reason in one line: the family fails one layer
earlier than evaluation — at test binding — and 1.0 does not
pay two L's for it.*

**D7 — The List standard: two-tier gate.** (a) The gate runs
two tiers, frozen in §2 of this document: **Tier-1** (Z3-
Verified ∧ golden-exact through the production pipeline) and
**Tier-2** (List element-level entries: executed-exact through
the compiled runner on the full battery, the tier printed in
each entry's documentation). The §3.9 list-element encoder
(Z3 sequence/array element model — the audit's one XL blocker)
becomes the headline post-1.0 campaign, with this library as
its test corpus. Rejected: (b) closing §3.9 first — the purest
standard, but 1.0 slips by a whole campaign; (c) deferring
element-level entries — sort/reverse/contains absent from a
"production-ready" stdlib is a visible hole. *Reason in one
line: the library ships whole, and the standard bends exactly
where the audit measured it must — nowhere else.*

**D8 — Set/Map: out.** (a) No Set/Map entries ship. Rejected:
(b) Set/Map in — there is no representation at any layer (no
Set/Map `Value`, no encoder sort, no literal syntax), so it is
a type-system project, not a library family. *Reason in one
line: nothing exists to audit, let alone to ship.*

---

## 2. The frozen scope

### 2.0 Tiers, and how to read the tables

- **Builtin tier** — entries implemented as compiler builtins.
  Standard: exact on the Phase 100 per-layer battery
  (interpreter / encoder fold / compiled runner agreement) over
  the full battery including extremes.
- **Tier-1** — the entry's crowned body is **Z3-Verified ∧
  golden-exact** through the production pipeline, on the final
  tree.
- **Tier-2** — the entry is **executed-exact through the
  compiled runner on the full battery** (pinned points 100%;
  spec-silent points reported separately), Verified where the
  current encoder reaches, with the tier printed in the entry's
  documentation. Tier-2 exists only for the List element-level
  entries named in §2.6 — it is not a general escape hatch, and
  no entry outside §2.6 may claim it.

Entries are frozen at the concrete instantiations named below
(`List<Int>`, `Option<Int>`, `Result<Int, String>`, …).
Polymorphic forms are an amendment question — the audit does
not measure them, so this freeze does not assume them.
Signatures use the reference's type notation. "Depends on"
names campaign machinery that must land before the entry can
be crowned at its tier; the tier itself is not conditional.

### 2.1 Builtin tier (already shipped; held to the battery standard)

| Entry | Signature | Edge semantics |
|---|---|---|
| `len` | `(List<T> \| String) → Int` | The length; empty collection/string → 0. Shipped since before v0.7.0 (UnaryOp::Len + call arm). |
| `int_to_str` | `Int → String` | Exact Rust i64 Display: `0` → `"0"`, negatives carry `-`, both i64 extremes render exactly. Shipped in Phase 100 (v0.7.0). |

### 2.2 Int — Tier-1 (8 entries)

| Entry | Signature | Edge semantics | Depends on |
|---|---|---|---|
| `int_abs` | `Int → Int` | \|x\|; 0 → 0. **Domain note (frozen):** the Int minimum is excluded by precondition (`require:` x is not the Int minimum) — its mathematical absolute is unrepresentable in Int, and the exclusion is stated here so the gate's batteries pin it as a precondition boundary, not a divergence. | Phase 109 (precondition assumption) for the exclusion to be judged as written |
| `int_sign` | `Int → Int` | x < 0 → −1; x = 0 → 0; x > 0 → 1. The Int minimum maps to −1 (no exclusion needed). | — |
| `int_min` | `(Int, Int) → Int` | The smaller argument; equal arguments → that value. | — |
| `int_max` | `(Int, Int) → Int` | The larger argument; equal arguments → that value. | — |
| `int_clamp` | `(Int, Int, Int) → Int` | `require: lo <= hi`. x < lo → lo; x > hi → hi; otherwise x. lo = hi → lo for every x. The audit's §3.1 exhibit: the faithful body executes 3/3 and was VerificationFailed on a precondition-violating point before the fix. | Phase 109 |
| `int_is_even` | `Int → Bool` | True iff 2 divides x — sign-independent (−4 is even, −3 is not). | — |
| `int_is_odd` | `Int → Bool` | True iff 2 does not divide x; `int_is_odd(x) == !int_is_even(x)` for every x, negatives included. | — |
| `int_gcd` | `(Int, Int) → Int` | The greatest non-negative common divisor; gcd(0, 0) = 0; gcd(a, 0) = \|a\|; symmetric. **Domain note (frozen):** arguments equal to the Int minimum are excluded by precondition, inheriting `int_abs`'s exclusion. Body route: While + invariant (the Phase 93/96 statement-judge machinery's shape). | Phase 109 for the exclusion |

### 2.3 Char — Tier-1 (6 entries)

The Char scalar encoding (Phase 76) plus the char_equation seed
family carry this family (audit probe: `char_is_digit`
**Verified** 3/3 through the production pipeline). Ranges are
ASCII, frozen as written — Unicode classes are out of 1.0
scope.

| Entry | Signature | Edge semantics |
|---|---|---|
| `char_is_digit` | `Char → Bool` | True iff the char is in `'0'..='9'` (the audit probe's contract, verbatim). |
| `char_is_alpha` | `Char → Bool` | True iff the char is in `'A'..='Z'` or `'a'..='z'` (ASCII letters only). |
| `char_is_whitespace` | `Char → Bool` | The character set is exactly the standing tokenizer spec's `is_whitespace` contract, adopted unchanged — the library adopts the spec, it does not redefine it. |
| `char_is_uppercase` | `Char → Bool` | True iff the char is in `'A'..='Z'`; non-letters → false. |
| `char_is_lowercase` | `Char → Bool` | True iff the char is in `'a'..='z'`; non-letters → false. |
| `char_to_digit` | `Char → Option<Int>` | `'0'..='9'` → `Some(d)` with d the digit's value; every other char → `None`. Its own contract is stated by equality rows over constructed `Some`/`None` values (guard-table form), so it needs no payload projection; its *callers'* payload-bearing specs depend on the §2.5 Option extension. |

### 2.4 String basics — Tier-1 (1 entry, plus standing surface)

The shipped String surface also includes what already exists:
`len` (§2.1) and concatenation as the `++` operator (F1) — not
entries, named here so the family's boundary is explicit. The
one new entry is the audit's measured one (pipeline probe
`string_is_empty`: **Verified** 2/2):

| Entry | Signature | Edge semantics |
|---|---|---|
| `string_is_empty` | `String → Bool` | True iff `len(s) == 0`. |

Everything else String-shaped is excluded or out — see §2.8
(transforms out per D5a; `starts_with`/`ends_with`/`repeat`/
trim-class excluded as unmeasured, recorded there).

### 2.5 Result / Option — Tier-1 (3 entries, one enabling extension)

**Existing surface (not entries):** the closed method table's
`Result::{is_ok, is_err, unwrap}` already ships (D1a,
Phase 76). Result's hand standard is measured (audit probe:
`result_unwrap_or`'s constructor-Match body **Verified** 2/2
against the guarded-method spec); its pipeline gap is a
seed/search gap (a Match-from-rows seed family, S–M), which is
build scope for Phase 112, not a tier question.

**The enabling extension (frozen as part of this scope):** the
closed method table extends to Option — `Option::{is_some,
is_none, unwrap}` — a D1a-style spec-side extension (encoder +
interpreter arms, S–M). MethodCall has no IR kind, so **no
bootstrap cascade rides on it** (the audit's §2.5 finding, from
Phase 99's D1a analysis). Without it, no spec can project a
symbolic `Some` payload and the Option entries' contracts are
unstatable for payload-bearing cases (the audit's §1.2 probe).

| Entry | Signature | Edge semantics | Depends on |
|---|---|---|---|
| `result_unwrap_or` | `(Result<Int, String>, Int) → Int` | `Ok(v)` → v; `Err(_)` → the default argument. Contract in the guarded form the audit's hand probe Verified (`r.is_err() \|\| output == r.unwrap()`, plus the Err row). | Match-from-rows seed family for pipeline crowning (Phase 112) |
| `option_unwrap_or` | `(Option<Int>, Int) → Int` | `Some(v)` → v; `None` → the default argument. Contract in the guarded method form, over the extended table. | Option table extension (Phase 112) |
| `option_ok_or` | `(Option<Int>, String) → Result<Int, String>` | `Some(v)` → `Ok(v)`; `None` → `Err(e)` with e the argument, unchanged. | Option table extension (Phase 112) |

### 2.6 List — Tier-1 length-level (1 entry) + Tier-2 element-level (11 entries)

The family's split is the audit's §2.6 measurement, uniform
across every probe: **the compiled runner executes nearly all
of it; the verifier binds a List parameter as its length only,
so no element-level entry can Verify at any body quality**
until the §3.9 encoder campaign lands post-1.0 (D7a). The
Tier-2 build (Phase 113) also lands the family's judging
machinery the audit priced into the List phase on either D7
branch: the interpreter For arm (S), the §3.4 runner-trigger
fix (S, owner-reserved since the generation campaign), and the
interpreter Reverse arm (S).

**Tier-1 (length-level):**

| Entry | Signature | Edge semantics |
|---|---|---|
| `list_is_empty` | `List<Int> → Bool` | True iff `len(xs) == 0` — a length-level contract, verifying on current machinery like its String sibling. |

**Tier-2 (element-level; executed-exact via the compiled
runner, tier printed in each entry's documentation):**

| Entry | Signature | Edge semantics |
|---|---|---|
| `list_reverse` | `List<Int> → List<Int>` | The elements in reverse order; empty → empty; singleton → itself. (The IR `Reverse` arm exists; the interpreter arm lands in Phase 113.) |
| `list_contains` | `(List<Int>, Int) → Bool` | True iff some element equals the target; empty → false. |
| `list_sum` | `List<Int> → Int` | The sum of the elements; empty → 0. Arithmetic is the language's standing Int arithmetic; the battery's extreme points are pinned executed points. |
| `list_product` | `List<Int> → Int` | The product of the elements; empty → 1; any zero element → 0. |
| `list_min` | `List<Int> → Option<Int>` | Empty → `None`; otherwise `Some(m)` with m the least element. (Option-returning, so the empty case is a value, not a precondition.) |
| `list_max` | `List<Int> → Option<Int>` | Empty → `None`; otherwise `Some(m)` with m the greatest element. |
| `list_take` | `(List<Int>, Int) → List<Int>` | The first n elements, with n clamped into `[0, len(xs)]`: n ≤ 0 → empty; n ≥ len(xs) → xs unchanged. |
| `list_drop` | `(List<Int>, Int) → List<Int>` | xs without its first n elements, with the same clamping: n ≤ 0 → xs unchanged; n ≥ len(xs) → empty. |
| `list_is_sorted` | `List<Int> → Bool` | True iff the list is non-strictly ascending (every adjacent pair a ≤ b); empty and singleton → true. The library form of Appendix A's `is_sorted`. Ships on the §3.4 trigger fix + interpreter For arm; no List entry ships while its template body is unjudgeable (the audit's §3.4 condition). |
| `list_sort` | `List<Int> → List<Int>` | The elements in non-strict ascending order; the output is a permutation of the input — the permutation property is judged on the executed battery (the `is_permutation` Predicate form is not a shipped entry; §2.7). Empty/singleton → itself. Body: the standing template body at the executed standard. Named explicitly, per the audit's §2.8: this family is where a silent two-tier standard would hide, so it does not get to be silent. |
| `list_binary_search` | `(List<Int>, Int) → Option<Int>` | `require:` the list is sorted (`list_is_sorted(xs)`) — assumed by the judges per D3a; the Tier-2 battery runs sorted inputs. Present → `Some(i)` with `xs[i]` equal to the target (with duplicate targets, any valid index satisfies the contract — the specific index is spec-silent and reported as such); absent → `None`. Hand body at the executed standard (G5 scoped around, the audit's §3.6). Named explicitly, per §2.8. |

### 2.7 Accounting: Appendix A's 18 dead names

Gate clause 7 ends the fiction — every dead name ships or is
removed. The accounting, frozen here:

- **Ship (10)**, as the `list_*` entries of §2.6: `is_sorted`
  → `list_is_sorted`, `contains` → `list_contains`, `reverse`
  → `list_reverse`, `sort` → `list_sort`, `sum` → `list_sum`,
  `product` → `list_product`, `min` → `list_min`, `max` →
  `list_max` (Appendix A's forms are the list forms, the
  audit's §1.1; the scalar forms ship as `int_min`/`int_max`),
  `take` → `list_take`, `drop` → `list_drop`.
- **Removed (8)**, with the reason recorded per name:
  `is_permutation` (its constraint form is a `Predicate` —
  unencodable, the audit's §3.10; the property survives inside
  `list_sort`'s executed battery, not as an entry), `filter`,
  `map`, `fold`, `all`, `any` (the higher-order class — out
  per D6a/G4), `zip` (tuple elements are unmodelled at every
  layer), `range` (no §2 analysis in the audit — excluded
  conservatively, §2.8).

### 2.8 Exclusions and ambiguity resolutions (recorded)

The audit's candidate lists were wider than this freeze in a
few places. Each narrowing below resolves an ambiguity the
audit left, conservatively (core only), and is recorded here
so the freeze is interpretable — none of these is a silent
drop, because none of them was ever a named entry of this
scope:

- **`div`/`rem` as entries — excluded.** The audit's §2.1
  lists "div/rem totalized forms" as candidates while its
  §3.1 groups div/mod with clamp as precondition-bearing
  entries. Resolution: no entries ship — division and
  remainder already exist as the language's `/` and `%`
  operators (§1.3 of the audit), and what precondition-bearing
  *uses* of them needed was the §3.1 fix (D3a), not library
  wrappers.
- **`int_pow` — excluded.** §2.1 names it only as "pow small",
  with no probe and no contract discussion. First amendment
  candidate once measured.
- **`char_is_alphanumeric` — excluded.** A pure disjunction of
  two shipped entries (`char_is_alpha || char_is_digit`) with
  no independent measurement in the audit.
- **`string_starts_with` / `string_ends_with` /
  `string_repeat` / trim-class — excluded.** §2.4 asserts
  coverage only in hedged form ("stated via slicing-free rows
  where the encoder reaches", "stated as properties"), with no
  probe behind any of them; the family's one measured entry is
  `string_is_empty`. First amendment candidates once measured.
- **Bool/truth helpers — no separate family.** The audit's
  §2.10 folds them into the Int/Char rows; this freeze follows
  it.
- **Generic/polymorphic entry forms — not assumed.** See
  §2.0: entries freeze at the concrete instantiations named.

### 2.9 The count

**30 library entries + 2 builtins = 32 named items.**

| Family | Entries | Tier-1 | Tier-2 | Builtin |
|---|---|---|---|---|
| Builtins (§2.1) | 2 | — | — | 2 |
| Int (§2.2) | 8 | 8 | — | — |
| Char (§2.3) | 6 | 6 | — | — |
| String basics (§2.4) | 1 | 1 | — | — |
| Result / Option (§2.5) | 3 | 3 | — | — |
| List (§2.6) | 12 | 1 | 11 | — |
| **Total** | **32** | **19** | **11** | **2** |

(30 library entries = the 32 named items minus the 2 builtins;
the library-entry tiers are 19 Tier-1 + 11 Tier-2.)

---

## 3. The 1.0 gate (frozen text)

*This is the gate. It is ticked by measurement on the final
tree, in the M1 gate's manner; its text is amended only by a
recorded owner decision, never at the gate run.*

1. **Scope frozen.** This document names every 1.0 entry with
   its signature, edge semantics (empty list, overflow, zero
   divisor, precondition), and tier. Nothing ships that is not
   named; nothing named is silently dropped — a dropped or
   re-tiered entry is a gate amendment, recorded.
2. **Every entry checks.** Each entry's spec, in the shipped
   library modules, passes `morphic check` as part of the
   prelude merge — the exact artifact users receive.
3. **Golden-exact.** Every entry carries in-spec tests
   covering empty/degenerate, singleton, typical, and
   adversarial points, plus a cross-validation battery against
   a hand-written Rust mirror; pinned points agree 100%,
   spec-silent points are reported separately (standing
   method).
4. **Verified, per tier.** For every Tier-1 entry, the
   production pipeline crowns a winner that is **Z3-Verified ∧
   golden-exact**; for every builtin, the per-layer battery
   (interpreter / encoder fold / compiled runner agreement,
   the Phase 100 precedent) is exact on its full battery
   including extremes. **No entry ships on executed evidence
   alone** — except the Tier-2 entries named in §2.6 (List
   element-level: executed-exact through the compiled runner
   on the full battery, Verified where the encoder reaches,
   the tier printed in the entry's documentation), a tier that
   exists because D7a created it. The tier list is part of
   this frozen scope (§2.9), not a footnote discovered at the
   gate run.
5. **Cross-target honesty.** Every entry renders on every
   codegen target that claims it; a target that cannot render
   an entry says so at build time — the silent free-call
   fallback that Appendix A's dead names hid behind is removed
   for library entries (measured by generated-Rust compile +
   run at minimum).
6. **Budgets.** The playground bundle stays within the
   standing budget — **771,072 B**, no silent re-baseline (the
   library ships compressed or as a separately-loaded asset if
   that is what staying inside takes); the library build (all
   entries crowned from clean) completes within a stated
   wall-clock on the CI machine; per-entry synthesis stays
   inside the playground profile's caps for the interactive
   subset.
7. **Documentation.** Appendix A is rewritten from "(Planned)"
   to per-entry reality: signature, tier, semantics, one
   example per entry; every shipped entry has a doc comment in
   its module. The 18 dead names either ship or are removed
   from the table, per the accounting frozen in §2.7 — the
   fiction ends either way.

**Closing line.** *Every entry in the frozen 1.0 scope stands
Z3-Verified ∧ 100% golden-exact through the production
pipeline — or, for builtins, exact on the per-layer battery;
for Tier-2 entries, executed-exact on the full battery with
the tier named in its documentation — on the final tree, with
the bundle inside budget.*

**Deliberately left out of the gate**, with the audit's
reasons (§5): per-entry complexity guarantees (the verifier
does not enforce `optimize:` bounds — a clause claiming them
would be unmeasurable); Set/Map and Float entries (D8a/D4a —
not scope, silence); higher-order entries (D6a); a user-facing
import mechanism (the prelude ships the library; imports are a
language feature with their own gate); API-stability promises
beyond the release (1.0 is the first version such a promise
could attach to; the gate does not pre-commit 1.x policy).

---

## Amendment 1 — `int_gcd` removed from the 1.0 scope (2026-11-23)

**Owner decision (2026-11-23).** `int_gcd` is amended **out of
the 1.0 scope**. This amendment is appended in the open; the
frozen scope and gate text above are not silently edited.

The entry's contract and blocker record are preserved by
reference: the frozen contract remains stated in §2.2, the
measurement and blocker are recorded in the ROAD Phase 111
note, and the standing pin in
`tests/phase111_cross_validation.rs` continues to require its
absence from the prelude, its faithful body's 6/6 execution,
and its **Inconclusive** verdict. The blocker is two-sided: no
seed family builds Euclid's algorithm, and the statement judge
cannot crown a faithful hand body because Z3 returns Unknown
on the quantified divisibility-preservation obligations over
`%`.

`int_gcd` returns with the **post-1.0 quantified-mod
campaign**: judge-side quantified-mod work plus an Euclid seed
family. Until that campaign closes it, `int_gcd` is not a 1.0
entry, is not re-tiered, and does not ship.

**Amended 1.0 totals:** **29 library entries + 2 builtins =
31 named items** (**Tier-1 18 / Tier-2 11 / builtin 2**). The
gate in §3 is run against this amended scope.

---

## Amendment 2 — the higher-order entries + `range` (the 1.2 amendment, frozen 2026-11-23)

**Freeze (Phase 134).** The six entries named in outline by
[`G4_SCOPE.md`](G4_SCOPE.md) §5 — the five higher-order names
the G4 language build unblocked, plus `range` — are frozen
here, as **Amendment 2** to this scope, in the Amendment 1
manner: appended in the open, the frozen 1.0 text above
untouched. The milestone is 1.2 (G4_SCOPE D8a: the G4
language build + this amendment, two freezes, one gate); the
language build this amendment rests on is complete (Phases
129–133: checker admission, closure values, in-process
higher-order judging, named-reference supply, the UF arm).
Sources: G4_SCOPE §5 (the outline) and §7 (the gate this
amendment is measured by), [`G4_AUDIT.md`](G4_AUDIT.md) §2.1
(the payoff table), and the package freeze it extends,
[`EMBED_SCOPE.md`](EMBED_SCOPE.md). Later phases tick this
amendment by measurement only; a dropped, re-tiered, or
re-signed entry is a further recorded amendment, never a
silent edit.

### A2.1 The six entries — signatures, edge semantics, docs

Signatures are stated in this document's §2 form (the form
the packages' manifest `signature` field carries). Entries
freeze at the concrete instantiations named — §2.0's
discipline carries: polymorphic forms (a `list_map` over
`List<Char>`, a `list_fold` whose accumulator is not Int)
are not frozen and do not ship in 1.2. Parameter names are
as the docs name them; they are part of the frozen surface
(the generated bindings take their parameter names from the
specs).

| Entry | Signature | Edge semantics |
|---|---|---|
| `list_map` | `(List<Int>, fn(Int) -> Int) → List<Int>` | `output[i] == f(xs[i])` for every i, order and length preserved; empty → empty. |
| `list_filter` | `(List<Int>, fn(Int) -> Bool) → List<Int>` | The elements of xs for which p answers true, in their original relative order; empty → empty; none pass → empty; all pass → xs unchanged. |
| `list_fold` | `(List<Int>, Int, fn(Int, Int) -> Int) → Int` | The **left** fold in element order: acc starts at `init`, each step replaces acc with `f(acc, x)`; empty → `init`. The accumulator is Int (the concrete instantiation, above). Argument order `(xs, init, f)` mirrors the constraint language's standing `fold(xs, init, f)` form (the `list_sum` / `list_product` constraints). |
| `list_all` | `(List<Int>, fn(Int) -> Bool) → Bool` | True iff p holds of every element; **empty → true** (vacuous truth — the `forall` meaning). |
| `list_any` | `(List<Int>, fn(Int) -> Bool) → Bool` | True iff p holds of at least one element; **empty → false** (the `exists` meaning). |
| `range` | `(Int, Int) → List<Int>` | The ascending list `[lo, lo+1, …, hi−1]`, step 1, length `max(0, hi − lo)`; **`lo == hi` → empty; `lo > hi` → empty** (a reversed range is empty — there is no descending form and no precondition). Bounds whose difference overflows Int are out-of-domain by the standing checked-arithmetic discipline (no value exists to agree on); battery points keep lengths small. |

Two recorded resolutions behind the table:

- **`range`'s form is the two-bound form.** The audit's
  probe measured a one-bound specimen (`range_upto`,
  `[0..n)`, judging 2/2 in-process) — a feasibility probe,
  not a signature. The frozen form is `range(lo, hi)` =
  `[lo, hi)`, whose `(0, n)` case is exactly the probed
  form (the probe's two points open the battery, A2.3, in
  that spelling), because the constraint language's own
  quantifier domain already spells both `range(n)` and
  `range(lo, hi)` — `list_is_sorted` ships
  `range(0, len(xs) - 1)` — so the two-bound entry is the
  general form of a spelling users already read, and the
  probed hand body (Alloc + For + Assign/Concat) generalizes
  by its For bounds alone. The entry's name is the
  outline's (`range`, G4_SCOPE §5); the quantifier-domain
  `range(…)` is a parser position, not a call — Phase 135's
  prelude-merge check (A2.7) is the instrument that proves
  the two live together, and a collision there is a finding
  that stops the build, not a rename done silently.
- **Application discipline rides along.** A supplied
  function's applications spend the standing evaluation
  budget and nest under the standing depth bound (G4_SCOPE
  §2.3): a battery or host point whose applications exhaust
  them declines the judgement — never a wrong value, never
  a hang. The batteries below use total, shallow functions
  throughout, as every standing battery uses in-domain
  points throughout.

**The docs, frozen verbatim.** No `doc:` texts existed for
these entries before this amendment; the six texts below
are authored here, in the standing `doc:` style of
`stdlib/list.morph` (the Phase 115 docs), and are frozen
verbatim — Phase 135 writes them into the specs' `doc:`
fields exactly as written, and the packages' manifest
`doc` fields and JSDoc/rustdoc carriage carry them verbatim
from there (the EMBED_SCOPE §2.4 discipline). Any later
edit to a text below is a further recorded amendment.

> `list_map`: "list_map(xs, f) = the list of f applied to each element of xs, in order; the empty list gives the empty list (STDLIB_SCOPE.md Amendment 2). Tier-2: executed-exact on the full battery; Tier-1 awaits the list-element encoder (3.9) together with the uninterpreted-function arm (G4_AUDIT.md 2.1)."

> `list_filter`: "list_filter(xs, p) = the elements of xs for which p holds, in their original order; the empty list gives the empty list, and a predicate that holds for no element gives the empty list (STDLIB_SCOPE.md Amendment 2). Tier-2: executed-exact on the full battery; Tier-1 awaits the list-element encoder (3.9) together with the uninterpreted-function arm (G4_AUDIT.md 2.1)."

> `list_fold`: "list_fold(xs, init, f) = the left fold of xs under f, starting from the accumulator init: f(f(f(init, x0), x1), ...) in element order; the empty list gives init (STDLIB_SCOPE.md Amendment 2). Tier-2: executed-exact on the full battery; Tier-1 awaits the list-element encoder (3.9) together with the uninterpreted-function arm (G4_AUDIT.md 2.1)."

> `list_all`: "list_all(xs, p) = true iff p holds for every element of xs; the empty list gives true (vacuous truth) (STDLIB_SCOPE.md Amendment 2). Tier-2: executed-exact on the full battery; Tier-1 awaits the list-element encoder (3.9) together with the uninterpreted-function arm (G4_AUDIT.md 2.1)."

> `list_any`: "list_any(xs, p) = true iff p holds for at least one element of xs; the empty list gives false (STDLIB_SCOPE.md Amendment 2). Tier-2: executed-exact on the full battery; Tier-1 awaits the list-element encoder (3.9) together with the uninterpreted-function arm (G4_AUDIT.md 2.1)."

> `range`: "range(lo, hi) = the ascending list [lo, lo + 1, ..., hi - 1]; lo == hi gives the empty list, and lo > hi gives the empty list — there is no descending form and no precondition (STDLIB_SCOPE.md Amendment 2). Tier-2: executed-exact on the full battery; element-level verification awaits the list-element encoder (3.9)."

### A2.2 Tiers

All six entries ship at **Tier-2** — executed-exact on the
full battery (A2.3), the tier printed in each entry's
documentation (A2.1) and in the packages' surfaces
(EMBED_SCOPE §2.4) — per the G4 decisions and the audit's
payoff table. **The Tier-1 upgrade path is named, as future
work and not a 1.2 obligation:** the five higher-order
entries Verify only when the §3.9 list-element encoder
lands **together with** the UF arm composed over element
applications (a body's `f(xs[i])` needs both an element
model and a function symbol — either alone is vacuous for
these contracts); `range` needs §3.9 alone (no function
argument, no UF content — its Inconclusive standing is the
element cap every Tier-2 List entry lives under). When
that campaign lands, the upgrades re-issue as label changes
at a lockstep release, without byte changes to the
functions (the EMBED_SCOPE R11 mechanism), with this
library — now including these six — as its test corpus
(the D7a design).

### A2.3 The golden batteries (frozen counts)

Each entry carries a cross-validation battery in the
Phase 115 form (pinned points against hand-written mirrors,
executed in-process; the same points are the packages' host
batteries under A2.7). **Function arguments are supplied
both ways across every higher-order entry's battery** —
lambda rows **and** named references (Phase 132's supply
form) — with the named points naming shipped entries where
a natural one exists, as the table records. Named points
respect the named entry's own frozen domain (no point feeds
the Int minimum to `int_abs` — its §2.2 precondition
excludes it — exactly as the standing `int_abs` battery
treats that point as the boundary it is).

**Spec-silent points: 0 for all six entries.** Every point
of the six contracts is fully determined by A2.1's
semantics — there is no index-choice class (the
`list_binary_search` case) and no underdetermined point in
any of the six; the counts below are therefore pinned
counts entire.

| Entry | Pinned | Of which lambda-supplied | Of which named-supplied | Named entries used |
|---|---|---|---|---|
| `list_map` | **20** | 12 | 8 | `int_abs` (5), `int_sign` (3) |
| `list_filter` | **20** | 12 | 8 | `int_is_even` (5), `int_is_odd` (3) |
| `list_fold` | **18** | 12 | 6 | `int_min` (3), `int_max` (3) |
| `list_all` | **16** | 10 | 6 | `int_is_even` (3), `int_is_odd` (3) |
| `list_any` | **16** | 10 | 6 | `int_is_even` (3), `int_is_odd` (3) |
| `range` | **15** | — (no function argument) | — | — |
| **Total** | **105** | | | |

The counts sit inside the standing Tier-2 band (the 1.0
List batteries run 13–28 pinned). The point classes each
battery covers, frozen with the counts:

- **`list_map` (20):** lambda — empty under two functions,
  singleton, typical, negation over mixed signs, constant
  function, identity (incl. at the Int extremes, one point
  each), duplicates, a longer list, a zero element; named —
  `int_abs` over mixed signs / negatives / empty /
  singleton / an Int-maximum element, `int_sign` over
  typical / negatives-and-zero / empty.
- **`list_filter` (20):** lambda — empty, all-pass,
  none-pass, typical mixed, negatives, a zero boundary,
  singleton pass and fail, duplicates, an extremes point,
  evenness spelled as a lambda; named — `int_is_even` over
  typical / empty / none-pass / negatives / singleton,
  `int_is_odd` over typical / all-pass / empty.
- **`list_fold` (18):** lambda — empty → init under two
  operations, singleton, typical sum and product shapes, a
  non-commutative operation (subtraction — the left-fold
  order is pinned by the point, not assumed), init
  dominating, negatives, init 0 vs init ≠ 0 over one list,
  non-overflowing extremes, a longer list; named —
  `int_min` with init above the minimum / init below every
  element (init wins) / empty, `int_max` likewise with a
  negatives point.
- **`list_all` (16):** lambda — empty → true, all-pass,
  first-element fail, last-element fail, singleton pass and
  fail, negatives, a zero boundary, duplicates, a longer
  all-pass; named — `int_is_even` all-even / one-odd /
  empty, `int_is_odd` all-odd / one-even / negatives.
- **`list_any` (16):** lambda — empty → false, none-pass,
  first-element pass, last-element pass, singleton pass and
  fail, negatives, a zero boundary, all-pass, duplicates;
  named — `int_is_even` one-even / all-odd / empty,
  `int_is_odd` one-odd / all-even / negatives.
- **`range` (15):** the audit probe's two points first, in
  the frozen spelling — `(0, 3) → [0, 1, 2]`, `(0, 0) →
  []` — then `lo == hi` → [], two reversed points
  (`lo > hi`, one ending at 0) → [], singletons at both
  signs, typical spans, a negative-to-positive span, an
  all-negative span, a length-10 span.

**In-spec tests** follow the standing classes (§3 clause
3's: empty/degenerate, singleton, typical, adversarial),
and for the five higher-order entries each spec carries at
least one lambda-supplied and one named-supplied test row —
the named rows judged through the prelude-merged index
(Phase 132's route), like every stdlib spec test.

### A2.4 Package obligations (frozen here, executed at Phase 135)

1. **Both packages gain exactly the six exports** — 35 in
   total. npm: the root module gains the six functions,
   six subpath shim pairs are added, and
   `morphic.manifest.json` gains six rows (name, the A2.1
   signature, `tier: "tier-2"`,
   `verdictBasis: "executed-exact"`, `battery` per A2.3,
   `doc` verbatim per A2.1). The facade gains the six
   functions under `morphic::stdlib` and the same six
   manifest rows in its copy. Nothing else is exported;
   no standing export moves.
2. **The JavaScript emitter's higher-order arms** — the
   measured scope, against what Phase 120 left standing.
   Today every form below reaches a catch-all:
   `ir_to_js_expr` renders Lambda / Closure / Map / Filter
   / Fold as the literal `undefined`, a Call whose callee
   is a bound variable falls through `js_call_form` to the
   same `undefined`, and a statement-position Map / Filter
   / Fold renders `// unsupported` in `ir_to_javascript`.
   Phase 135 adds exactly:
   - a **Lambda arm** in `ir_to_js_expr` — the
     arrow-function form (parameters + expression body);
   - a **Closure arm** in `ir_to_js_expr` — the same form;
     the capture list has no JavaScript runtime form
     (JavaScript closures capture the enclosing scope
     lexically). The one seam is named, not hidden: the
     interpreter's capture is a by-value snapshot
     (G4_SCOPE §2.2), so the amendment's bodies form
     closures only over parameters and over locals not
     rebound after the closure's creation — the discipline
     the natural bodies already keep (the Map / Filter /
     Fold operands are the parameter itself or closed
     lambdas) — and battery agreement is the pin: a body
     whose meaning turned on rebinding a captured name
     after creation would be a Phase 135 finding, not a
     rendering;
   - an **application arm** — a `Call` whose callee names
     a **bound function variable** (the entry's
     function-typed parameters; a local bound to a function
     value) renders as a direct JavaScript call. Callees
     that are neither builtins, constructors, shipped
     entries, nor bound function variables keep the
     standing refusal — no silent form is invented for
     them;
   - **Map / Filter / Fold arms** in `ir_to_js_expr` — the
     collection operand composed with the function operand
     rendered through the Lambda / Closure / Var forms
     above (the `.map` / `.filter` / `.reduce` shapes are
     Phase 135's rendering text; this freeze fixes the
     nodes and the operand forms that must compose), and
     `ir_to_javascript`'s bare-expression-statement group
     extends to the same three nodes in the same change,
     so renderability does not depend on a node's position.
   - **The Sort arm is not in scope** — no amendment body
     is a Sort body (G4_SCOPE §5 names lambda + application
     + Map/Filter/Fold only); its JavaScript standing stays
     the catch-all, recorded here so the absence is a
     decision.
   - **The TypeScript boundary, recorded as this
     amendment's interpretation of G4_SCOPE §5 with §6:**
     the six exports ship and answer at runtime; the
     `.d.ts` type of a function-typed parameter keeps the
     EMBED_SCOPE §3 stance (`never` — no callable
     TypeScript function type is frozen in 1.2), and a
     typed higher-order TypeScript surface is the later
     embed amendment's question (G4_SCOPE §6, priced S–M
     atop G4). The manifest's `signature` field carries
     the true Morphic signature (A2.1), so the boundary is
     stated in the package's own surface, not hidden by
     it. The facade has no such boundary: `ty_to_rust`'s
     Function arm is already complete (fn-pointer types),
     so facade parity is fully typed.
3. **The crown-check judging-site wiring** (Phase 132's
   recorded hand-off). The pipeline's judging sites —
   including the package crown check inside
   `morphic build --package` / `--facade` — kept the
   context-free route at Phase 132, which cannot bind a
   named reference. Phase 135 wires the package crown
   check to judge through the **index-carrying route**
   (the `test_outcomes_in` family) with the prelude-merged
   `SpecIndex` in hand, so the new entries' named-supply
   points bind during crown re-verification. **Non-mover
   obligation:** the 29 standing entries' crown
   re-verification outcomes must be identical on the index
   route — their points carry no named supply, so the two
   routes must agree exactly — measured and reported at
   Phase 135, per Phase 132's record.
4. **Version lockstep 1.2.0 at the gate** (EMBED_SCOPE R6
   carries): both packages re-assemble at 1.2.0 when
   Phase 136 stamps the project version — not before, and
   no registry publish precedes the gate (R5 carries).
5. **Documentation surfaces:** `LANGUAGE_REFERENCE.md`
   Appendix A gains the six entries' rows (signature,
   tier, semantics, one example — the §3 clause-7 form in
   which the 1.0 tiers are printed), and `EMBEDDING.md`
   names the six exports for package users, with the
   A2.4.2 TypeScript boundary stated where a package user
   reads it.

### A2.5 Budgets

- **Playground bundle: 771,072 B stands** (G4_SCOPE §7
  clause 4). The library ships as the separately-loaded
  asset (the Phase 112 route), so the six entries' data
  does not spend the bundle; the gate measures it standing.
- **The npm package budget at 35 exports: 76,800 B**
  (unpacked mass, the EMBED_SCOPE §4 measure), derived by
  the W4 discipline from Phase 122's actuals plus the
  measured per-entry increment:

| Component | Basis | Bytes |
|---|---|---|
| Base: the 29-export package | Phase 122's measured actual (unpacked) | 58,815 |
| — root-module body + doc block (`index.js`) | List-family average over the standing package (band 178–1,309) | 584 |
| — export-list line (`index.js`) | 412 B of export tail ÷ 29 | 14 |
| — typings block (`index.d.ts`) | List-family average | 460 |
| — manifest row | List-family average, raw bytes | 536 |
| — subpath shim pair (`.js` + `.d.ts`) | Average over the 29 pairs | 86 |
| — `package.json` exports entry | The standing entry's raw form | 90 |
| **Per-entry increment** | | **1,770** |
| Six entries | 6 × 1,770 | 10,620 |
| **Composite measurement** | 58,815 + 10,620 | **69,435** |
| + 10% margin, rounded up to a whole KiB | 69,435 × 1.10 = 76,378.5 → 75 KiB | **76,800 B** |

  The increment is measured on the List family because the
  six are List-family entries whose docs sit in that
  family's length band (A2.1's texts run with the standing
  Tier-2 docs). Two notes, recorded so the derivation is
  not a surprise at Phase 135: the tree's current assembly
  stands at 59,092 B (Phase 125's guide-linked README
  regeneration added 277 B of package furniture over the
  Phase 122 base) — inside the margin, not a second base;
  and the frozen figure lands **below** the 1.1 budget
  (79,872 B) deliberately — a budget re-derived from
  measured actuals replaces the bracket-derived figure it
  succeeds, because a budget that only grows by inheritance
  has stopped measuring anything.
- **The Rust crate carries no size budget** (EMBED_SCOPE
  R8 carries).

### A2.6 Owner prerequisites at the gate (named, not tasks)

Recorded here so Phase 136 has no surprises; none of these
is a task for Phase 134 or 135, and none needs a decision:

- **The registry publishes remain the owner's act**
  (EMBED_SCOPE R5/R12 carry): after a PASS at Phase 136,
  from the tagged v1.2.0 tree on the owner's PC —
  `npm publish --access public` from `packages/npm` and
  `cargo publish` from `packages/rust` — with the owner's
  own credentials, as at 1.1.
- **npm needs a fresh login or token.** The 1.1 publish's
  granular token (the Bypass-2FA `morphic-publish` token)
  was revoked after use — owner-confirmed on 2026-11-23 —
  so no standing npm credential exists for 1.2: the owner
  signs in (`npm login`) or mints a fresh granular token on
  his PC at publish time.
- **crates.io stands as the owner left it at 1.1:** the
  `cargo login` token on his PC persists if he kept it
  (his choice at 1.1); if not, a fresh token is created by
  him the same way, at publish time.

### A2.7 The gate's amendment instruments

G4_SCOPE §7 is the 1.2 gate; this section states the
instruments its clauses 2 and 4 run for the new entries,
so Phase 136 executes them rather than inventing them.

**Clause 2 (the amendment stands at its tiers) — in-process,
on the final tree:**

- every entry's spec **checks** as part of the prelude
  merge — the shipped library modules under `morphic
  check`, the §3 clause-2 form (this is also the
  instrument that proves `range` the entry and `range(…)`
  the quantifier domain live together, A2.1);
- **in-spec tests judged in-process through the
  index-carrying route** (`run_spec_tests_in` over the
  prelude-merged index): 100% Passed on the evaluable —
  a named-supply row that does not bind fails this
  instrument; it does not skip silently;
- **the frozen batteries (A2.3): 105 pinned points**
  executed in-process at 100% against the hand-written
  mirrors, spec-silent points reported per entry (0,
  frozen);
- **tier printing verified by inspection:** this
  amendment, the six `doc:` texts as shipped (verbatim
  against A2.1), and Appendix A's six rows (A2.4.5).

**Clause 4 (the surfaces answer) — on the final tree:**

- **npm clean-host rehearsal:** a fresh project installs
  the packed 1.2.0 tarball; all six exports are called on
  their A2.3 battery points under node — function
  arguments passed as JavaScript lambdas and as the
  package's own exports (the named cases of A2.3) — at
  100%. Non-mover: the 29 standing exports' clean-host set
  (217 points, the Phase 121/122 instrument) re-runs at
  100%;
- **facade host rehearsal:** a fresh cargo project on the
  facade calls all six at their battery points with
  fn-pointer / closure arguments at 100%. Non-mover: the
  29 entries' host battery (1,112 points, the Phase 124
  instrument) re-runs at 100%;
- **budgets measured:** playground bundle ≤ 771,072 B;
  npm unpacked mass ≤ **76,800 B** (A2.5); the cascade
  instrument reported — Codegen spec count against the
  standing 130 and the synthesized-artifact bytes —
  movement named as a finding, per clause 4;
- **crown re-verification:** both assemblies re-run their
  crown checks on the gate tree through the index-carrying
  route (A2.4.3) and reproduce the checked-in packages
  byte-for-byte (the Phase 126 clause-4 discipline).

### A2.8 Exclusions, recorded in this freeze

- **`zip` and `is_permutation`** — excluded from 1.2, with
  G4_SCOPE §6's measured reasons (`zip`: the missing tuple
  value, a decision of its own; `is_permutation`: the
  §3.10 Predicate form plus the §3.9 element model). The
  Phase 128 plan required this freeze to record them; it
  does.
- **Polymorphic forms of the six** — not frozen (A2.1).
- **A callable TypeScript function type at the package
  boundary** — not frozen in 1.2 (A2.4.2's recorded
  interpretation); the later embed amendment's question.

**Amended totals.** The library stands at **35 library
entries + 2 builtins = 37 named items** (**Tier-1 18 /
Tier-2 17 / builtin 2**); the package surface at 1.2 is
**35 exports** (18 Tier-1 / 17 Tier-2). The 1.0 gate in §3
was run against Amendment 1's scope; the 1.2 gate
(G4_SCOPE §7) runs against the scope as amended by
Amendments 1 and 2 together.
