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
