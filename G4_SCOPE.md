# G4_SCOPE.md — Phase 128: the frozen G4 scope + the 1.2 gate

The ratification + freeze document for **G4 — function values**:
the first post-1.0 language build (stdlib D6a), and for the
milestone it carries, **1.2 = the G4 language build + the
higher-order library amendment** (D8a). Source material:
[`G4_AUDIT.md`](G4_AUDIT.md) (Phase 127 — the layer-by-layer
measurement of its §1, the payoff table of its §2, the decision
list of its §3, the pricing of its §4, the phase plan of its §5,
the gate sketch of its §6; probe artifact
`tests/phase127_g4_probes.rs`, 12 probes, every audit claim a
`P127|` line), over the library frozen by
[`STDLIB_SCOPE.md`](STDLIB_SCOPE.md) (§2.7, the eight removed
names) and the boundary frozen by
[`EMBED_SCOPE.md`](EMBED_SCOPE.md) (§3, function types unmapped).

**Ratification — by delegation, recorded as such.** On
2026-11-23 the owner delegated this campaign round's decisions
to Muse — *"aku beri kamu hak buat mutusin kali ini"* ("I give
you the right to decide this time") — and **D1a–D8a were
ratified (a) by Muse on 2026-11-23 under that delegation**.
Each recommendation was checked against the audit's probe
evidence before ratification; no probe contradicted a
recommendation, so no deviation was taken, and the decisions
stand exactly as the audit recommended them (§1). The
delegation's scope is this campaign's D-list, not a standing
blanket. This document freezes, and later phases tick by
measurement only:

- **the language semantics** (§2): the closure value model,
  capture, application, supply, and the no-equality rule, in
  contract form — a later phase is judged clause by clause
  against it.
- **the G4 corpus** (§3): the probe specs and the two bundled
  exhibits, with their required end standings — the corpus the
  gate's clause 1 measures.
- **the UF arm's scope** (§4): congruence only, its may/may-not
  boundary stated.
- **the gate** (§7): the audit's 5-clause sketch finalized into
  gate text in the M1-gate style, closing line included.
- **the suite configurations** (§9): the three configurations
  every later phase cites, with exact invocations and measured
  baselines — resolving the Phase 126/127 figure discrepancy.

Docs only: this phase changes no production code, no spec, and
no test. The one measurement it adds is §9's, run under the
documented invocations. The campaign that executes against
this freeze is frozen in §8 (Phases 129–136).

---

## 1. The ratified decisions (D1a–D8a)

**D1 — The value model: (a) closures.** `Value` gains a
function variant carrying parameters, a body, and a captured
environment. Rejected: (b) named-function references only —
measured insufficient as the whole answer, because the
recorded G4 wall is lambda-shaped (both bundled examples'
tests pass lambda literals; audit §1.2, §3 D1); (c) both in
one build — sound, but D4a stages the named route instead.
*Reason in one line: closures are the only model that binds
the test rows the language already documents and ships.*

**D2 — Capture semantics: (a) by-value snapshot at
creation.** The interpreter's `Env` is an owned map; a closure
clones the bindings it captures. Corollary, frozen with it:
**a Let-bound function does not capture itself** — capture
resolves against the enclosing environment at creation.
Rejected: (b) capture by reference — a `Value`-model rewrite
(L on its own) for no named payoff.

**D3 — Checker admission: (a) application of function-typed
parameters is admitted** under the checker's standing coarse
argument discipline (the looseness the rest of expression
typing lives with). The strict per-argument validation of the
`int_to_str` precedent was for a builtin, not a class.
Rejected: (b) strict validation at every application —
inconsistent with the checker's discipline everywhere else,
buying precision the judges do not consume.

**D4 — Function supply: (a) lambda literals now,
named-reference supply included and staged after closures.**
Test rows already supply lambda literals in the documented
syntax — no new syntax is part of G4. The by-name route (a
test row naming a sibling spec or a shipped entry in a
function position) lands as its own phase (Phase 132), after
the closure core. Rejected: (b) defer named supply out of the
milestone — the library's own documentation examples want
named entries, and entry batteries would carry lambda
spellings throughout.

**D5 — Equality on function values: (a) there is none.**
Judged comparisons decline function values rather than compare
them; `values_equal` is taught this explicitly, because its
derived-`==` fallback would otherwise hand closures a
syntactic equality (body AST + captured environment) that
means nothing (audit §1.5). Rejected: (b) extensional equality
on sampled points — a judging fiction wearing a semantics'
clothes.

**D6 — The Z3 stance: (a) an uninterpreted-function arm,
included as its own phase** (Phase 133), scoped at §4.
Rejected: (b) defer the whole encoder question to the §3.9
campaign — defensible, but the arm is cheap, bounded, and
exercises the encoder a campaign before §3.9 needs it
load-bearing.

**D7 — A seed family for higher-order bodies: (a) none in
G4.** The amendment's entries ship hand bodies under the
stdlib D7a precedent (`list_sort`, `list_binary_search`
shipped the same way). Judging — the thing G4 exists to fix —
does not need generation. Rejected: (b) a map/filter seed
family now — it pays only if pipeline-crowning higher-order
entries becomes a gate demand; §7 makes no such demand.

**D8 — Milestone shape: (a) 1.2 = the G4 language build +
the higher-order library amendment** — one milestone, two
freezes (this one, then the amendment freeze at Phase 134),
one gate (§7). Rejected: (b) G4 as pure language work in 1.2
with the amendment in 1.3 — a milestone whose user-visible
content is two bundled examples is thinner than the project's
gate tradition; the language build's payoff *is* the library
amendment.

---

## 2. The frozen semantics contract (D1a–D5a)

The contract the build phases are judged against, clause by
clause. "Declines" throughout means the interpreter's standing
decline discipline — a named `NotEvaluable`/refusal with a
reason, never a panic, a hang, or a silent wrong value.

**2.1 The closure value (D1a).** `Value` gains exactly one
function variant, carrying: the parameter list, the body, and
the captured environment (an owned snapshot of bindings, §2.2).
The IR's existing `Lambda` and `Closure` nodes evaluate to
this variant; no other node constructs a function value. The
higher-order nodes (`Map`, `Filter`, `Fold`, `Sort`) consume
this variant (Phase 131) — they gain no value machinery of
their own.

**2.2 Capture (D2a).** Capture is a **by-value snapshot at
creation**: the bindings a closure captures are cloned from
the environment in force where the lambda is evaluated. For
the IR `Closure` node, captures resolve **by name** against
the enclosing environment at creation (the node's standing
form). A function bound by `Let` **does not capture itself**:
its captured environment is the one enclosing the `Let`.
There is **no fixpoint syntax** in G4 and none is added;
recursion remains what it is today — a named-spec feature,
disciplined by the denotation/index routes. A closure body
may call *named specs* through those standing routes; a
closure cannot name itself, and mutual recursion between
closures is not a G4 form.

**2.3 Application.** Applying a closure binds its parameters
to the argument values in the captured environment extended
with those bindings, and evaluates the body there. Applying a
non-function value declines by name. **Application spends the
standing evaluation budget** — the interpreter's shared
loop/call budget discipline (the `MAX_LOOP_ITERATIONS`
standing from Phase 95): closure application is not an
unbounded recursion channel beside the budget, and exhaustion
declines as `NotEvaluable`, exactly as loop exhaustion does.

**2.4 The checker (D3a).** `check_expr`'s Call arm, when the
callee name resolves against neither module signatures, the
`int_to_str` special case, nor the stdlib fiction table,
falls back to the variable environment; a callee of
`TypeRef::Function` type is admitted with the checker's
standing coarse argument discipline. Nested application
(`f(f(x))`) falls out of the same arm. Passing a function-typed
value as an argument stands as it does today (audit §1.1: Ok
via the fiction table). The bare-atom side door — a whole-row
call the parser lowers to `Constraint::Predicate`, whose
checker arm never resolves the callee name — is **left
exactly as-is**; closing it is a §3.10 conversation (§6).

**2.5 Supply (D4a).** A function argument is supplied in
exactly two forms:
- **Lambda literals**, in the spec language's existing,
  documented test syntax (the `|x, y| …` form, the arrow form,
  the typed `fn(x: Int, …) -> …` form — `LANGUAGE_REFERENCE.md`
  is the ground truth; G4 adds no syntax). Available from
  Phase 130.
- **Named references**: a name standing in a function position
  in a test row (or an entry battery) resolves through the
  module index to a closure built from that spec's body —
  sibling specs and shipped entries alike. Available from
  Phase 132, staged after closures by D4a; before it lands,
  the lambda form is the only supply and batteries spell
  lambdas.

**2.6 Equality (D5a).** There is **no equality on function
values**, decided here rather than inherited:
`values_equal` declines function values explicitly — the
derived-`==` fallback never sees a closure. Judged comparisons
(expected-output equality in test judging, `==` in evaluated
positions) decline function values rather than compare them.
In constraints, `f == g` remains what it measurably is today —
checkable, and Inconclusive at the verifier (§2.7) — and the
language reference's standing is recorded as such. What the
language does instead, frozen as the idiom: compare function
*results* on points, via test rows — extensional where it
matters, honest about where it stops.

**2.7 The verifier standing (outside §4's arm).** A
function-typed parameter binds as `Sym::Opaque` in every
position the UF arm does not cover. A body that applies its
parameter outside the UF slice stays unencodable →
Inconclusive. **An unused function parameter never poisons**:
a spec that declares one and never applies it verifies
exactly as it does today (the `ignore_f` exhibit, §3, Verifies
on the audit tree and must keep Verifying at every phase).

**2.8 Emitters standing.** G4 changes **no rendering**. The
Rust target's higher-order renderings stand as measured
complete (audit §1.6); `ty_to_rust`'s Function arm and the
Sort template's generated signature (Phase 61's owner-reserved
Clone-bound question) are **tripwires**: the build does not
touch them, and the bootstrap cascade for the core build is
therefore zero, as the audit measured. The JS higher-order
arms the amendment needs are Phase 135's, not the core's (§5);
the C/WAT/Python standings are §6 exclusions.

---

## 3. The frozen G4 corpus

The corpus the gate's clause 1 measures: the audit's probe
specs (checked forms in `tests/phase127_g4_probes.rs`) plus
the language's two bundled higher-order examples as named
exhibits. Each exhibit's **required end standing** is frozen
here; a phase that moves an exhibit short of its standing has
not landed, and a standing may be changed only by a recorded
amendment to this freeze.

The type specimen, verbatim from the audit:

```
spec apply_twice {
    input: f: fn(Int) -> Int, x: Int
    output: Int
    constraint: output == f(f(x))
    test { input: (|n| n + 1, 40); expect: 42 }
    test { input: (|n| n * 2, 3); expect: 12 }
}
```

| Exhibit | Required end standing | Lands at |
|---|---|---|
| `apply_twice` (apply-class specimen) | **Writable + judgeable end to end**: checks (§2.4); its two lambda tests bind and judge **in-process 2/2**; reaches **Verified** under the UF arm (§4) | 129 · 130 · 133 |
| `apply_fa` (application under `forall`) | Checks end to end (the audit's Err shape flips to Ok) | 129 |
| `apply_req` (application in `require:`) | Checks end to end | 129 |
| `pass_f` (fn parameter passed as an argument) | Stands Ok, as today — no regression | 129 |
| `apply_atom` (bare-atom row → `Predicate`) | Standing **unchanged** by design (§2.4's side door; §6) | — |
| `apply_once` (bindable body applying its parameter) | Evaluates in-process — the `call to unknown function 'f'` decline is gone | 130 |
| `ignore_f` (fn parameter declared, never applied) | **Verifies**, as today — the no-poisoning pin (§2.7) | every phase |
| `same_f` (`output == (f == g)`) | Checks; executed comparisons decline the function values (§2.6); verdict Inconclusive | 130 · 133 |
| `double_it` + `apply_named` (named-reference exhibits) | A test row naming `double_it` in a function position binds and judges in-process | 132 |
| `apply_render` (compiled-runner exhibit) | Its standing compiled-runner score (**Scored 2/2**) does not regress — G4 adds the in-process route, it does not tax the compiled one | 131 |
| **`sort`** (`examples/sort.morph`) | Judges **in-process at 100%: 6/6** through `run_spec_tests` — its crown no longer depends on the compiled runner | 131 |
| **`binary_search`** (`examples/binary_search.morph`) | Judges **in-process at 100%: 7/7** through `run_spec_tests` — the G5 compiled-form failure is bypassed, not fixed (§6) | 131 |

Two standings the corpus deliberately does **not** require:
`sort` Verified (its walls are §3.9 elements, §3.10 Predicate
form, and comparator semantics — none is G4's), and any
movement in the Phase 95 raw-pool number (§6).

---

## 4. The UF arm's frozen scope (D6a)

Phase 133's encoder slice, bounded here:

- A function-typed parameter binds as a **Z3 uninterpreted
  function** of its declared signature; application of that
  parameter encodes as UF application; `encodable.rs` mirrors
  exactly this slice — no more of the encoder learns about
  functions in this build.
- **What it may produce:** `Verified` for direct-application
  specs whose provable content is **congruence only** — the
  `apply_twice` class, where `output == f(f(x))` becomes a
  term equation Z3 discharges by congruence. This is the
  arm's whole payoff, stated at its true size.
- **What it may not produce:** any fact about a function's
  values beyond congruence — no extensionality, no
  injectivity or surjectivity, no value at any point that the
  spec does not itself state. Nothing about `Map` / `Filter`
  / `Fold` bodies: those encodings are vacuous without the
  §3.9 element model and are re-filed under that campaign
  (§6), not approximated here.
- **Everything beyond congruence remains Inconclusive** —
  the standing verdict, never a fabricated one. Verdict
  movement outside the frozen corpus (§3) is a gate clause 3
  violation, measured at Phase 133 and again at the gate.

---

## 5. The 1.2 amendment — outline (frozen at Phase 134)

Named here so the milestone's shape is fixed; **no detail in
this section is frozen** — signatures, batteries, tier lines
and package-budget impact are Phase 134's freeze, in the
stdlib amendment tradition.

- **Entries:** `list_map`, `list_filter`, `list_fold`,
  `list_all`, `list_any`, and `range` — each at **Tier-2**
  (executed-exact on its full battery, the tier printed in
  its documentation), per the audit's payoff table (§2.1 of
  the audit: the five names unblocked by G4; `range` never
  blocked, needing only this amendment).
- **Bodies:** hand bodies per D7a — no seed family (D7a of
  this freeze). Tier-2's disclosure discipline applies as it
  did at 1.0 (the `list_binary_search` precedent: where a
  body's shape is narrower than its contract's letter, the
  `doc:` says so).
- **Surfaces:** the JavaScript emitter gains its higher-order
  arms (lambda + application + the `Map`/`Filter`/`Fold`
  nodes, atop the Phase 120 embeddable subset) so the npm
  package answers the new entries' batteries; the Rust facade
  reaches parity through the already-complete Rust rendering.
  Function *types* at the package boundary are not part of
  the amendment (§6).

---

## 6. Explicit exclusions

Named out of the 1.2 milestone, each with its measured home —
an exclusion here is a routing decision, not a dismissal:

- **`zip`** — its wall is the missing **tuple value** on the
  data side (a zip test's expected output maps to no
  interpreter value; audit §2.1, measured). It waits on a
  tuple-value decision of its own; G4 changes nothing for it.
- **`is_permutation`** — its walls are the `Predicate`
  constraint form (§3.10) and the §3.9 element model. (The
  property already lives inside `list_sort`'s executed
  battery.)
- **Z3 encodings of the `Map`/`Filter`/`Fold` nodes** —
  vacuous without §3.9's List element model; **re-filed
  under the §3.9 campaign** by the audit, and not
  approximated by §4's UF arm.
- **C / WAT / Python higher-order body rendering** — the
  NF-116-1 class: C types a function parameter `void*` and
  comments the body out; Python passes; WAT renders its
  standing shell. Their standings stay exactly as the audit
  measured them; closing them is future emitter work, not
  this build's.
- **Function types at the embed-package boundary** —
  EMBED_SCOPE §3's mapping of `TypeRef::Function` to `never`
  stands for 1.2. Host callbacks and higher-order exports are
  a later embed amendment's question, priced S–M atop G4
  when one takes it.
- **The Phase 95 raw-pool residual** (template seeds at tpr
  0.000 in context-free pool judging) — **disjoint cause,
  measured**: raw `eval` takes no `SpecIndex`, so *named
  sibling calls* have no denotation in the pool by
  construction. G4's wall is the adjacent one (applying a
  parameter, binding a supplied function). Closing G4 does
  not move the Phase 95 number, and no phase in §8 books
  payoff there.
- **The Predicate bare-atom side door** (§2.4) and
  **§3.10 generally** — the checker arm that never resolves
  a Predicate callee name stays as-is; it is that campaign's
  conversation.
- **The Sort template's generated signature** (Phase 61's
  Clone-bound question) — owner-reserved; G4 neither needs
  nor takes it (§2.8's tripwire).
- **Capture by reference and fixpoint syntax** — D2a's
  rejected branches; not deferred features, decided
  non-features of this build.

---

## 7. The 1.2 gate (frozen text)

*This is the gate. It is ticked by measurement on the final
tree, in the M1 gate's manner; its text is amended only by a
recorded owner decision, never at the gate run.*

**The clauses:**

1. **The language's own higher-order specs judge
   in-process.** `sort` at **6/6** and `binary_search` at
   **7/7** through `run_spec_tests` on the final tree, and
   the frozen G4 corpus (§3) at its required end standings
   likewise — no function-valued judgement on the final tree
   depends on the compiled runner.
2. **The amendment stands at its tiers.** Every entry in
   the frozen 1.2 amendment scope (Phase 134) stands at its
   named tier — Tier-2 executed-exact for the five
   higher-order entries and `range` — batteries green
   in-process, tiers printed in the amendment scope document
   and in the guide.
3. **Verdict honesty.** No verdict changes outside the
   frozen corpus (§3): the 1.0/1.1 corpora and the W3 pinned
   corpus re-measured on the final tree; function-valued
   comparisons decline (D5a, §2.6); every body the UF arm
   cannot encode remains **Inconclusive**, named — nothing
   beyond congruence is claimed (§4).
4. **The surfaces answer.** The npm package answers the
   amendment entries' batteries at 100% under node and the
   Rust facade likewise; the playground bundle stands within
   the standing **771,072 B** budget; the cascade instrument
   (Codegen spec count **130**, synthesized-artifact bytes)
   is reported — movement is a named finding, never a
   surprise.
5. **Suites green in all three configurations** on the
   final tree — the configurations being exactly those
   defined in §9 of this freeze, cited by invocation.

**Closing line:**

> *1.2 closes when a Morphic spec can take a function, apply
> it, and be judged for it by the interpreter that ships —
> and when the five higher-order entries and `range` stand
> at their named tiers on the final tree, with `sort` and
> `binary_search` crowned by that interpreter rather than
> rescued by a compiler.*

---

## 8. The frozen phase plan (129–136)

128 is this freeze. Prices are the audit's measured ones
(§4 of the audit), restated per phase.

- **Phase 129 — Checker admission (D3a).** *S.*
  Entry: this freeze. Work: the §2.4 Call-arm fallback.
  Exit: the admission taxonomy re-measured — the three Err
  shapes (postcondition, `forall`, `require:`) check end to
  end; `pass_f` unregressed; **no other check outcome in the
  standing corpora moves**.
- **Phase 130 — Interpreter function values (D1a, D2a,
  D5a).** *L — the core.* Entry: Phase 129. Work: the
  `Value` variant, Lambda/Closure evaluation, application in
  `eval_call`, lambda binding in `value_of_expr` /
  `bind_test_env`, the §2.3 budget discipline,
  `values_equal`'s explicit decline. Exit: `apply_twice`'s
  tests judge in-process **2/2**; `apply_once` evaluates;
  `ignore_f` still Verifies; `same_f`'s comparisons decline.
- **Phase 131 — Higher-order node judging.** *S–M.*
  Entry: Phase 130. Work: the `Map` / `Filter` / `Fold` /
  `Sort` evaluation arms consuming §2.1's variant. Exit:
  **`sort` 6/6 and `binary_search` 7/7 in-process at 100%**;
  `apply_render`'s compiled standing unregressed.
- **Phase 132 — Named-reference supply (D4a).** *S–M.*
  Entry: Phase 130 (staged after closures, per D4a; runs
  after 131 in plan order). Work: binding-time resolution of
  a name in a function position through the module index to
  a closure built from that spec's body. Exit: the
  `apply_named` exhibits bind and judge in-process; shipped
  entries are suppliable by name in test rows.
- **Phase 133 — Encoder UF arm (D6a).** *S–M.* Entry:
  Phase 130; staged last among the language phases so a slip
  cannot block the judging work. Work: §4's slice — the UF
  binding arm, the application arm, the `encodable.rs`
  mirror. Exit: the `apply_twice` class reaches **Verified**
  on congruence; the standing corpora re-measured with **no
  verdict drift outside the frozen corpus**.
- **Phase 134 — 1.2 amendment freeze.** *S.* Entry: the
  language build complete (129–133). Work: the amendment
  scope document freezing §5's six entries — signatures,
  contracts, batteries, tier lines, package-budget impact.
  Exit: the amendment frozen in the Phase 108/119 tradition;
  `zip` and `is_permutation` recorded in it as excluded,
  with §6's measured reasons.
- **Phase 135 — Amendment build.** *M.* Entry: Phase 134.
  Work: the six entries + batteries + reference docs; the
  JS emitter's higher-order arms; both packages regenerated
  under their crown checks. Exit: every amendment entry
  executed-exact in-process on its full battery, and both
  packages answer those batteries (gate clause 4 rehearsed).
- **Phase 136 — 1.2 gate, merge, v1.2.0.** *M.* Entry:
  Phase 135. Work: the §7 gate run clause-by-clause on the
  final tree; on PASS, the stack merge and the v1.2.0
  release in the standing form (the registry publishes
  following the tag as the owner's release acts, per the
  1.1 precedent). Exit: gate verdict recorded per clause;
  a failing clause stops the merge, as every gate before
  this one has.

**Total: one L core (130) with S–M slices around it** — the
audit's L + L re-scoped: a third of the encoder's L lands
here as the UF arm, the rest was always §3.9's (§6).

---

## 9. Housekeeping — the suite-configuration resolution

Phase 127 flagged a discrepancy: the Phase 126 record (and
Phases 124/125, and the CHANGELOG's [1.1.0] line) cite the
third suite configuration at **594/0/22**, while Phase 127,
measuring under `--no-default-features --features
wasm-playground`, found pristine master at **510/0/22** and
its own tree at 522/0/22. **Resolved here, by measurement:
neither figure is a mis-record. Two different invocations
have shared one label — "the third configuration" / "no-Z3"
— and each series is exact for its own invocation.**

The evidence, all measured in this phase on this tree
(`feat/g4-128-scope` = the audit tree, master + 12 probes):

| Invocation | This tree | Pristine master @ `4881c1d` |
|---|---|---|
| `cargo test` | **740/0/30** | 728/0/30 |
| `cargo test --features bootstrap-codegen` | **740/0/30** | 728/0/30 |
| `cargo test --no-default-features` | **606/0/22** | **594/0/22** |
| `cargo test --no-default-features --features wasm-playground` | **522/0/22** | 510/0/22 |

(The master column for the first, second and fourth rows is
Phase 127's own measurement; the third row's master figure is
what Phases 124–126 recorded on their trees, and this phase's
606 = 594 + the 12 probes reproduces it exactly.)

- **The 594 lineage is `cargo test --no-default-features`.**
  The long "no-Z3" series (…426 → 474 → 498 → 518 → 533 →
  545 → 564 → 578 → 587 → 594 across Phases 104–126) moves in
  lockstep with the default count at every step — each phase's
  teeth land "in every configuration" — which is true only of
  this invocation. Phase 114's note states the definition
  outright: *"The no-Z3 configuration is `cargo test
  --no-default-features`; the `wasm-playground` feature
  profile is the WASM build's own."* The Phase 126 gate
  record and the [1.1.0] CHANGELOG line therefore **stand
  correct as recorded** — no amendment to them is needed or
  made.
- **The 510/522 series is the browser build profile**,
  `--no-default-features --features wasm-playground` —
  Phase 66's configuration, and the one Phase 127 measured.
  It is a *build profile* (the playground's), not a suite
  configuration: adding the `wasm-playground` marker cfg's
  out the native-prelude test surface **by design** (Phase
  112's embedded-data design; the surface was cfg-gated at
  Phase 117). The delta is exact and one-directional,
  verified by a `-- --list` diff in this phase: **84 tests**
  present under `--no-default-features` and absent under the
  browser profile, **zero** in the other direction — the
  `#[cfg(not(feature = "wasm-playground"))]` surface across
  the Phase 110–124 test files (prelude, cross-validations,
  the stdlib asset, honesty, bindings, the two packages, the
  JS batteries, the Char cascade).
- **The deltas, for the record:** default → no-Z3 is 134
  (the default-feature surfaces: z3-support, compiled-runner,
  interactive, llm-client); no-Z3 → browser profile is 84
  (the surface above). Ignored stands at 30 in the two native
  configurations and 22 in both no-Z3 invocations.

**The three configurations, frozen for citation.** From
Phase 129 onward, suite lines cite exactly these, in this
order, and "no-Z3" means the third:

1. `cargo test` — baselines: master **728/0/30**, this tree
   **740/0/30**.
2. `cargo test --features bootstrap-codegen` — baselines:
   master **728/0/30**, this tree **740/0/30**.
3. `cargo test --no-default-features` ("no-Z3") — baselines:
   master **594/0/22**, this tree **606/0/22**.

The browser profile is reported when it is relevant (WASM
work), always under its full invocation, and is never cited
as "the third configuration" again. Any further measurement
in a phase note names its invocation in full. Phase 127's
numbers stand as measured — under the browser profile's
invocation, which its note now reads as.
