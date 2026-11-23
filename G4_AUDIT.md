# G4 Audit — Function Values (Phase 127)

**Status: AUDIT ONLY.** Probes + this document; no feature code,
no verdict changes. Branch `docs/g4-127-audit` from master @
`4881c1d` (v1.1.0 closing docs). Suites green in all three
configurations with this phase's 12 probes
(`tests/phase127_g4_probes.rs`, every measurement below is a
`P127|` line from that file): **740/0/30** (default),
**740/0/30** (bootstrap-codegen), **522/0/22**
(no-default-features + wasm-playground) — each exactly +12
over the locally re-measured pristine-master baselines
(728/0/30, 728/0/30, 510/0/22). One record-keeping note: the
Phase 126 docs record the third configuration at 594; that
figure does not reproduce in this environment with the
documented invocation (pristine master measures 510/0/22,
failed and ignored identical), and this audit reports the
measured numbers rather than inheriting it. Sources: `STDLIB_AUDIT.md`
(§2.7, §3.5, §3.10, the price table), `STDLIB_SCOPE.md` (§2.7,
the eight removed names), `EMBED_AUDIT.md` §7, `EMBED_SCOPE.md`
§3/§7, `LANGUAGE_REFERENCE.md` (ground truth for syntax —
nothing here invents any), the Phase 61 and Phase 95 ROAD
notes.

G4 is the first post-1.0 language build (stdlib D6a): function
values in the interpreter — the gap that removed the
higher-order half of Appendix A at the 1.0 freeze and that
leaves the language's own two bundled higher-order examples,
`examples/sort.morph` and `examples/binary_search.morph`,
unbindable in-process to this day.

## Headline

**G4 is one build, not a staged pair — but the wall is not
where the stdlib audit recorded it, and two of its five layers
are already standing.**

The wall is a single absence — there is no function value —
seen from five layers. The audit's price was **L (interpreter)
+ L (encoder)**. Measured against the tree, that price splits
differently:

- **The interpreter L stands** — it is the build's centre:
  one `Value` variant, closure evaluation, application, test
  binding. Everything else is S–M slices around it.
- **The encoder L does not survive contact.** The only
  encoder slice that means anything without the §3.9 List
  element model is an uninterpreted-function arm for direct
  application — **S–M**. The Map/Filter/Fold node encodings
  the audit priced are vacuous without §3.9's elements; they
  belong to that campaign's XL, not to G4.
- **One layer is worse than recorded.** Applying a
  function-typed parameter in an expression position is a
  **type error today** — the wall sits in the *checker*, one
  layer before the binding wall §2.7 recorded. A spec cannot
  even *state* `output == f(f(x))`. The fix is small (S) but
  it is a precondition for everything else, and the stdlib
  audit never saw it because its probes stopped at test
  binding.
- **One layer is better than recorded.** The compiled runner
  already judges function values end to end: an apply body
  against lambda tests scores **2/2**, and the pipeline
  **crowns `sort` today at tpr 1.0** through it. The Phase 61
  Clone-bound wall does not bite inside the runner's
  concretized harness — it bites only the generic
  `morphic build` signature, a separate, owner-reserved
  emitter-contract question that G4 does not touch.
  `binary_search`'s crown, by contrast, sits at **tpr 0**:
  the runner judged its template and its compiled form
  failed — that is G5's index wall, and the in-process rescue
  G4 provides is exactly what bypasses it.
- **Only one emitter believes in function values.** Rust
  renders an apply body perfectly today. JavaScript accepts
  the parameter and returns `undefined` for the body; C types
  the parameter `void*`; Python passes. The embed unlock is
  real but it is a later amendment's work, not this build's.

**Milestone shape (recommendation, D8): 1.2 = the G4 language
build + the higher-order library amendment**, two freezes and
one gate in the 1.1 shape — five of the eight removed names
(`map`, `filter`, `fold`, `all`, `any`) plus `range` shipping
at Tier-2, with `zip` and `is_permutation` named as *not* G4
items (their walls are tuples and §3.9/§3.10 respectively —
measured, §2).

## 1. Function values today, layer by layer

The probe spec used throughout (checked form in the probes;
the constraint is the point):

```
spec apply_twice {
    input: f: fn(Int) -> Int, x: Int
    output: Int
    constraint: output == f(f(x))
    test { input: (|n| n + 1, 40); expect: 42 }
    test { input: (|n| n * 2, 3); expect: 12 }
}
```

### 1.1 Spec language — declares fine; application is a type error

Function types are fully in the language: `TypeRef::Function`
(ast.rs), the parser's `fn(T1, T2) -> R` arm, a parser test
(`parse_function_type_param`), and `LANGUAGE_REFERENCE.md`
documents them (§4 type table; spec examples with
`cmp: fn(T, T) -> Int` parameters; `type Callback = fn(Int,
Int) -> Bool`). **Lambda syntax exists today** — `Expr::Lambda`
in the AST, a parser arm, and the reference's Lambda
Expressions section documents `|x, y| x + y`, the arrow form
`|a, b| -> a > b`, and a typed `fn(x: Int, y: Int) -> x + y`
form. Both bundled examples' lambda-carrying test rows parse.

What does not exist is *application in expression positions*.
Measured admission taxonomy for a function-typed parameter
`f` (probe `phase127_where_application_is_admitted`):

| Shape | Standing |
|---|---|
| `constraint: output == f(f(x))` (postcondition) | **Err** — `check: In spec 'apply_twice': postcondition: Called undefined function: 'f'` |
| `constraint: forall i in xs: f(i) == f(i)` | **Err** — same, postcondition |
| `require: f(x) > 0` | **Err** — same, precondition |
| Passing `f` as an argument: `output == is_sorted(list, cmp)` | **Ok** — the checker's fiction table types the *name* `is_sorted`; `cmp` rides as an unexamined `Var` |
| A bare atom row: `constraint: cmp(a, b)` | **Ok, vacuously** — the parser lowers a whole-row call to `Constraint::Predicate`, and the Predicate arm never resolves the callee name; the row constrains nothing (and Predicate is unencodable, §3.10) |

Mechanism: `check_expr`'s Call arm resolves callees only
against module `function_sigs`, the `int_to_str` special case,
and the stdlib fiction table (`stdlib_return_type`) — never
against `env.variables`, where parameters live. A sibling
spec's *name* in Var position does resolve to
`TypeRef::Function` through `function_sigs`; the value it
would denote binds nothing anywhere (§1.2). This is the
audit's first correction to the record: **STDLIB_AUDIT §2.7
recorded the first wall as test binding; the first wall is
the checker.** Any G4 build starts here.

### 1.2 Interpreter — no function value, by name

`Value` (eval.rs) has eleven variants; none is a function.
The declines are explicit and named:

- A lambda in a test row is **Unbindable**, reason verbatim:
  *"input cannot be mapped to an interpreter value (it
  carries a value the interpreter cannot represent, e.g. a
  function/lambda)"*. `bind_test_env`'s own doc comment names
  the case: *"a lambda for a fn-typed parameter — the
  evaluator has no function values."*
- `value_of_expr` on a named sibling (`Expr::Var("double_it")`)
  returns **None** — a named-function reference binds nothing;
  there is nothing for it to bind *to*.
- Body evaluation declines by node name: `Lambda (function
  value)`, `Closure (function value)`, and Map / Filter /
  Fold / Sort as *(higher-order)*. A call whose callee is a
  parameter dies as `call to unknown function `f`` —
  `eval_call` knows List constructors, `len`, and `int_to_str`
  only.
- Measured standings: the probe spec binds **0/2** tests;
  `sort` binds **0/6**; `binary_search` binds **0/7**;
  in-process runs are `evaluable: 0` for every higher-order
  body probed.

**The Phase 95 residual, scoped precisely.** Phase 95 recorded
raw-pool template seeds at tpr 0.000 and named the cause
"context-free pool judging declines named calls." That
residual is **not** G4. Raw `eval` takes `(node, env)` — no
`SpecIndex` — so a body calling a *named sibling spec* has no
denotation in the pool by construction; module context enters
judging through compositions (synthesis) and denotations
(verifier), never through eval itself. G4's wall is the
adjacent one: applying a *parameter* and binding a *supplied*
function. The two share a symptom (tpr 0) and nothing else;
closing G4 does not move the Phase 95 number, and the audit
says so in the open so nobody books it as payoff.

### 1.3 The judges — one blind, one already complete

There are two executed judges, and their standings differ
absolutely (probe `phase127_compiled_runner_judges_what_the_interpreter_cannot`):

- **In-process** (`run_spec_tests`): 0 evaluable for every
  lambda-carrying spec — §1.2.
- **Compiled runner** (`runner::rescore_finalists_report`,
  rustc + the generated harness, which renders test lambdas
  as Rust closures and concretizes generic element types):
  an apply body `f(f(x))` against the two lambda tests scores
  **Scored { passed: 2, total: 2 }, tpr 1.0**; sort's
  SortPrimitive template scores **Scored { 6, 6 }, tpr 1.0**;
  binary_search's template scores **Scored { 0, 7 }, tpr
  0.0** — *judged and failed at compilation*, the G5 wall
  (the template's i64 list indices; the harness's
  concretization cures sort's element typing, not index
  typing).

The production pipeline (seeded profile, seed 42; probe
`phase127_pipeline_crowns_for_the_bundled_ho_specs`):

| Spec | Candidates | Crown | Winner tpr | Verdict |
|---|---|---|---|---|
| `sort` | 21 | the SortPrimitive template body | **1.0** | Inconclusive |
| `binary_search` | 25 | the BinarySearch template body | **0.0** | Inconclusive |

So `sort` is *already crowned faithfully* — by exactly one
judge, through a compiled detour, with the generic-signature
Clone question (Phase 61, owner-reserved) still open for
`morphic build` output. What G4 adds for sort is the
in-process route: bindable tests, an evaluating interpreter,
a crown that does not depend on rustc. For `binary_search`,
G4 is the rescue outright: once tests bind and the
interpreter can apply `cmp`, its template body — While, Index,
Alloc, all in the Phase 95 evaluated subset — scores in-process,
and the compiled route's G5 failure stops mattering.

One more measured wrinkle, because the same number appears
with two different standings: in the configuration *without*
the compiled runner (the browser build), both crowns read
**tpr 1.0 — vacuously**. The engine's scoring carries an
explicit fallback: a spec with tests but zero evaluable
assigns test score 1.0 ("tests assumed passed — legacy
structural score", engine.rs). Natively, the runner replaces
that fallback with a measurement for finalists (sort's 1.0 is
real; binary_search's 0.0 is real). In the playground nothing
replaces it, so a higher-order spec there crowns at a perfect
score with **zero tests judged**. The playground's own
bindability surface reports the truth separately (0 bindable),
but the crown number alone would mislead — and it is exactly
the hole G4 closes: once tests bind and bodies evaluate,
`evaluable > 0` and the fallback never fires for these specs
in any configuration.

### 1.4 Synthesis — the IR believes; no family builds

The IR already carries `Lambda { params, body }`, `Closure {
params, captures, body }`, `Map`, `Filter`, `Fold`, and `Sort`
nodes (engine.rs). Measured production of them:

- **No seed family constructs a higher-order node.** Every
  construction site in the engine is a test or a walk; the
  constraint-seed families build from constraint shapes, and a
  constraint that applies `f` cannot check (§1.1), so no
  family can even be *aimed* at an apply-style spec today.
- **Two templates consume a function argument**: SortPrimitive
  and BinarySearch both call the comparator parameter directly
  as an IR `Call` — the deliberate Phase 61 design (the IR
  `Sort` node is avoided; `sort_by(cmp)` is E0631 against the
  generated signature). They are why the runner standings in
  §1.3 exist at all.
- The LLM seed route is ungated text; any higher-order node it
  emitted would decline in-process (§1.2) and score only via
  the runner.

Consequence for pricing: G4 needs **no new seed family** to
pay off. The library entries ship hand bodies under the D7a
precedent (as `list_sort` did); a family that *synthesizes*
map/filter-shaped bodies from constraints is a separate,
later decision (D7).

### 1.5 Verifier — Opaque binding, honest Inconclusive, no poisoning

- `fresh_sym`'s catch-all binds function-typed parameters as
  **`Sym::Opaque`** — the comment at the arm names function
  types explicitly, alongside Float, Unit, tuples, sets, maps.
- A body that applies its parameter is **not encodable**
  (`body_is_encodable` = false; `encode_expr` returns None for
  `Expr::Lambda`, and `encodable.rs` declines Lambda / Closure
  / Map / Filter / Fold / Sort). Verdict for the probe spec:
  **Inconclusive**.
- **No poisoning**: a spec that declares a function parameter
  and never applies it (`ignore_f`: `constraint: output == x`)
  **Verifies today**. Function parameters cost nothing where
  they are unused.
- Function equality has no home: `constraint: output == (f ==
  g)` *checks* (coarse BinOp typing admits it) and verifies
  **Inconclusive**. On the executed side, `values_equal` falls
  back to derived `==` — a closure variant added naively would
  silently acquire *syntactic* equality (body AST + captured
  environment), which is why equality is a decision (D5), not
  a default.

The Z3 option the design space actually contains: bind the
parameter as an **uninterpreted function** and encode
application as UF application. That buys congruence facts
only — `f(f(x))` becomes a term, `output == f(f(x))` becomes
provable — and nothing extensional. It is the whole of the
encoder's G4 slice; §4 prices it at S–M.

### 1.6 Emitters — Rust believes; the rest decline or dissemble

For the apply body `f(f(x))` (probe
`phase127_emitter_standings_per_target`, verbatim):

- **Rust — complete.** `pub fn apply_render(f: fn(i64) -> i64,
  x: i64) -> i64 { f(f(x)) }`, and the generated test module
  passes a closure (`apply_render(|n| n, 5)`). Lambda / Closure
  / Map / Filter / Fold / Sort all render (iterator adapters);
  a Map body renders `(xs).iter().map(|x| { (x + 1)
  }).collect()`. Function types render as fn pointers via
  `ty_to_rust`.
- **JavaScript — signature yes, body no.** `function
  apply_render(f, x) { return undefined; }` — the parameter is
  accepted (untyped, a JS function value in waiting) and the
  application falls to the Phase 120 subset fallback. The Map
  node likewise renders `return undefined;`. Silently wrong —
  the NF-116-1 class; Map was never in the embeddable subset.
- **C — `void*`.** `int apply_render(void* f, int x) { /*
  unsupported */; }` — the parameter is typed as an opaque
  pointer and the body is a comment. Also NF-116-1 class.
- **Python — skeletal.** `def apply_render(f: Any, x: int) ->
  int: pass  # unsupported`.
- **WASM (text)** — the export shell renders; bodies are the
  standing comment stubs (Phase 118's measurement).

**Bootstrap cascade surface — measured, and G4 core does not
touch it.** The self-host corpus (`examples/self_host/
codegen.morph`) mirrors the *Rust codegen's* higher-order
renderings as spec equations — `rust_expr_lambda_text`,
`rust_filter_text`, `rust_fold_text` and their call forms are
in the corpus today. G4's core (checker admission, interpreter
values, test binding, the UF arm) changes **no rendering**, so
the cascade surface is zero for the core build — provided the
build does not touch `ty_to_rust`'s Function arm or the Sort
template's generated signature. Both are named non-goals: the
signature question is Phase 61's owner-reserved Clone-bound
decision, and it stays reserved.

## 2. The payoff surface

### 2.1 The eight removed names (STDLIB_SCOPE §2.7)

| Name | Removal reason (frozen) | G4 verdict — measured |
|---|---|---|
| `map` | Higher-order (D6a → G4) | **Ships in 1.2 at Tier-2.** Spec states once application checks (D3): `list_map(xs, f)` with len + per-index rows. Hand body (Map node or For + apply) judges executed-exact once closures bind. Tier-1 waits on §3.9 elements + the UF arm together. |
| `filter` | Higher-order (D6a → G4) | **Ships in 1.2 at Tier-2.** Contract is the subsequence shape — statable only partially in rows; the standing Tier-2 discipline (doc + battery, cf. `list_binary_search`'s disclosed scan) carries it. |
| `fold` | Higher-order (D6a → G4) | **Ships in 1.2 at Tier-2.** The IR Fold node evaluates once its operation is a value; contracts ride the battery, as fold contracts do everywhere. |
| `all` | Higher-order (D6a → G4) | **Ships in 1.2 at Tier-2.** The checker already fictions `all`/`any` as Bool-returning names; a real entry replaces the fiction. Tier-1 needs §3.9 + UF. |
| `any` | Higher-order (D6a → G4) | **Ships in 1.2 at Tier-2.** As `all`. |
| `range` | No §2 analysis; excluded conservatively | **Not a G4 item — Tier-2-ready today.** Probe `phase127_range_is_not_a_g4_item`: a hand body in the shipped Phase 95/115 vocabulary (Alloc + For + Assign/Concat building `[0..n)`) judges **2/2 in-process on this tree**; its Inconclusive verdict is the §3.9 element cap every Tier-2 List entry already lives under. It needs only a scope amendment, no language work. |
| `zip` | Tuple elements unmodelled at every layer | **Not a G4 item.** Probe `phase127_zip_wall_is_tuples_not_functions`: a zip test's *expected output* (a list of tuples) maps to **no interpreter value** (`value_of_expr` = None), and a tuple construction likewise. The wall is the missing tuple *value*, on the data side. G4 changes nothing; zip waits on a tuple-value decision of its own. |
| `is_permutation` | Constraint form is a `Predicate` — unencodable (§3.10) | **Not a G4 item.** Its wall is the Predicate form + the §3.9 element model, both named campaigns elsewhere. (The property already lives inside `list_sort`'s executed battery.) G4 changes nothing. |

Net: **G4 unblocks five names; one name was never blocked
(`range`); two names were misfiled under G4 by proximity
(`zip`, `is_permutation`) and this audit re-files them with
measurements.**

### 2.2 The language's own specs

- `sort` — judges in-process after G4 (tests bind; the
  template's Alloc/For/Index/Clone vocabulary plus `cmp`
  application is the evaluated subset). Its crown stops
  depending on the compiled runner. Verified remains out of
  reach (§3.9 elements, §3.10 Predicate, the comparator's
  semantics) — G4 does not promise it.
- `binary_search` — rescued outright (§1.3): in-process
  judging bypasses the compiled form's G5 failure.
- **User specs that become writable** (the probe spec is the
  type specimen): any spec that takes behaviour as a parameter
  — apply/transform shapes, comparators, predicates over
  caller-supplied tests, callbacks in the reference's own
  `type Callback = fn(Int, Int) -> Bool` shape. Today each of
  these dies at the checker (§1.1) before any other layer is
  consulted; after D3 + the interpreter build they check,
  judge in-process, and — for direct-application bodies, with
  D6 — verify.

## 3. The design decisions

Each decision carries a recommended **(a)**. Under the owner's
delegation for this campaign (recorded 2026-11-23), the parent
ratifies on this audit's evidence; deviations from a
recommendation must state the contradicting probe.

### D1 — The value model: closures, named references, or both

- **(a) Closures (recommended).** `Value` gains a function
  variant carrying parameters, a body, and a captured
  environment. This is the only option that binds the test
  rows the language *already documents and ships*: sort's and
  binary_search's tests pass lambda literals, and §1.2's
  Unbindable reason is lambda-shaped. It also evaluates the
  IR's existing Lambda/Closure/Map/Filter/Fold nodes without
  new machinery per node.
- (b) Named-function references only (defunctionalization): a
  reference names a spec in scope; no capture. **Measured
  insufficient as the whole answer**: it leaves every lambda
  test row in the tree unbindable — the recorded G4 wall is
  lambda-shaped, so (b) alone does not close it. It is also
  not free: raw eval has no `SpecIndex` (§1.2), so references
  need an index-carrying judging path the codebase does not
  currently give the interpreter.
- (c) Both in one build: closures + named supply together.
  Sound, and the end state either way; priced +S–M over (a)
  with the ripple concentrated in `bind_test_env`'s signature.
  D4 stages it instead.

### D2 — Capture semantics

- **(a) By-value snapshot at creation (recommended).** The
  interpreter's `Env` is an owned `HashMap<String, Value>`;
  a closure clones the bindings it captures (the IR `Closure`
  node already lists captures *by name*, resolved at creation)
  — this is the only semantics the Value model affords without
  a rewrite. Corollary, named: **a Let-bound function does not
  capture itself** — capture resolves against the enclosing
  environment at creation. Recursion remains a named-spec
  feature (the denotation/index routes already discipline it);
  no fixpoint syntax exists in the grammar and G4 adds none.
- (b) Capture by reference: needs shared-mutable cells in
  `Value` — a Value-model rewrite (L on its own) for no named
  payoff.

### D3 — Checker admission of application

- **(a) Admit application of function-typed parameters
  (recommended).** `check_expr`'s Call arm, when the callee
  name is not a spec/builtin/fiction, resolves it in
  `env.variables`; a `TypeRef::Function` callee checks with
  the checker's standing coarse argument discipline (the same
  looseness the rest of expression typing lives with — the
  strict per-argument validation of the `int_to_str` precedent
  was for a builtin, not a class). Nested application
  (`f(f(x))`) falls out. The Predicate side door (§1.1) is
  named and left as-is — closing it is a §3.10 conversation.
- (b) Strict validation of argument and return types at every
  application: inconsistent with the checker's discipline
  everywhere else; buys precision the judges do not consume.

### D4 — Named-reference supply at judging time

How a function argument is *supplied*, completely: test rows
already supply **lambda literals** (documented syntax, both
arrow forms — no new syntax is part of G4, and none is
needed). The remaining supply route is **by name** — a test
row naming a sibling spec or a shipped entry (`list_map(xs,
int_abs)`-style batteries; a user passing their own helper).

- **(a) Include it in this build, staged after closures
  (recommended).** A name in a function position at binding
  time resolves through the module index to a closure built
  from that spec's body. S–M. Without it, entry batteries and
  user specs can only ever pass lambdas — viable, but the
  library's own documentation examples want named entries.
- (b) Defer: lambdas only. The build still closes the wall;
  the amendment's batteries carry lambda spellings throughout.

### D5 — Equality on function values

- **(a) There is none (recommended).** Judged comparisons
  decline function values rather than compare them;
  `values_equal` must be taught this explicitly, because its
  derived-`==` fallback would otherwise hand closures a
  syntactic equality (body + captured env) that means nothing
  (§1.5). In constraints, `f == g` stays what it measurably is
  today — checkable, unverifiable — and the reference should
  say so. What the language does instead: compare function
  *results* on points (test rows), which is extensional where
  it matters and honest about where it stops.
- (b) Extensional equality on sampled points: a judging
  fiction wearing a semantics' clothes. Rejected on the
  honesty standard every prior campaign applied.

### D6 — The Z3 stance: uninterpreted functions

- **(a) Include the UF arm in this build, as its own phase
  (recommended).** A function-typed parameter binds as a Z3
  uninterpreted function of its declared signature;
  application encodes as UF application; `encodable.rs`
  mirrors exactly that slice. Payoff, stated at its true size:
  direct-application specs (the probe spec's class) can reach
  **Verified** on congruence facts; nothing about Map/Filter/
  Fold bodies changes (those encodings are §3.9's). S–M.
  Staged last among the language phases so a slip cannot block
  the judging work, and the gate names it as its own clause.
- (b) Defer the whole encoder question to the §3.9 campaign:
  apply-style specs stay Inconclusive after G4; the executed
  judges carry the milestone alone. Defensible; the audit's
  judgement is that the arm is cheap, bounded, and exercises
  the encoder a campaign before §3.9 needs it load-bearing.

### D7 — A seed family for higher-order bodies

- **(a) None in G4 (recommended).** The amendment's entries
  ship hand bodies under the D7a precedent (`list_sort`,
  `list_binary_search` shipped the same way). Judging — the
  thing G4 exists to fix — does not need generation.
- (b) A map/filter seed family now (S–M, the `char_guard_table`
  shape): pays only if pipeline-crowning higher-order entries
  becomes a gate demand. It is not one in §6.

### D8 — Milestone shape

- **(a) 1.2 = G4 + the higher-order amendment (recommended).**
  One milestone, two freezes (the G4 scope freeze, then the
  amendment freeze), one gate — the 1.1 shape. The language
  build's payoff *is* the library amendment; splitting them
  ships a capability with no surface.
- (b) G4 as pure language work in 1.2, amendment in 1.3: the
  bundled examples judging in-process is a real line, but a
  milestone whose user-visible content is two examples is
  thinner than the project's gate tradition.

## 4. Pricing

Against STDLIB_AUDIT §3.5 / the price table ("G4 function
values: L, plus encoder arms L"):

| Layer | Audit said | Measured price | Evidence |
|---|---|---|---|
| Checker admission | *not recorded* | **S** | One Call-arm fallback to `env.variables`; the admission taxonomy (§1.1) bounds exactly which shapes change |
| Interpreter values + application + binding | L | **L** (confirmed) | One variant, but it touches eval, `value_of_expr`, `bind_test_env`, `values_equal`, the budget discipline (application must spend the standing loop/call budget), and the W3 bindability surface |
| HO node judging (Map/Filter/Fold/Sort arms) | (inside the L) | **S–M** | Mechanical once application exists; each arm is a loop the interpreter already knows how to run |
| Named-reference supply | *not recorded* | **S–M** | `bind_test_env` gains the module index; closure-from-spec-body construction |
| Encoder: UF arm | L ("arms for the nodes") | **S–M** | One binding arm + one application arm + the encodable mirror; the node encodings the audit priced are §3.9's, re-filed |
| Encoder: HO node encodings | (the other half of that L) | **not G4** | Vacuous without §3.9's element model; rides that campaign |
| Seed family | — | **0** (D7a) | Hand bodies per precedent |
| Emitters: Rust | — | **0** | Measured complete (§1.6) |
| Emitters: JS HO arms | — | **S–M**, in the *amendment* phase, not G4 core | Lambda + application + Map/Filter/Fold arms atop the Phase 120 subset; needed only when the amendment ships entries to npm |
| Emitters: C / WAT / Python | — | **0 in this build** | Their fn standings are named (§1.6) and stay; C's `void*` is recorded as NF-class debt, not silently inherited |
| Bootstrap cascade | — | **0** | No rendering changes in the core build (§1.6); the corpus's HO equations mirror renderings G4 does not touch |

**Total: one L core with S–M slices around it** — the same
order as the audit's L + L, with the second L re-scoped: a
third of it lands here (UF arm), the rest was always §3.9's.

## 5. Phase plan

- **Phase 128 — Ratification + G4 scope freeze.** D1–D8
  recorded; `G4_SCOPE.md` freezes the corpus (the probe spec's
  class + the bundled examples), the budget discipline for
  application, and the non-goals (the Sort signature, the
  Predicate door, C/WAT/Python standings). *S.*
- **Phase 129 — Checker admission (D3).** The apply probe spec
  checks end to end; the admission taxonomy re-measured; no
  other check outcome moves. *S.*
- **Phase 130 — Interpreter function values (D1a, D2a, D5a).**
  The `Value` variant, Lambda/Closure evaluation, application
  in `eval_call`, lambda binding in `value_of_expr` /
  `bind_test_env`; `values_equal` declines function values;
  application spends the standing budget. The probe spec's
  tests judge in-process 2/2. *M–L — the core.*
- **Phase 131 — Higher-order node judging.** Map / Filter /
  Fold / Sort evaluation arms. `sort` judges in-process 6/6
  and `binary_search` 7/7 — the latter's crown no longer
  routes through rustc. *S–M.*
- **Phase 132 — Named-reference supply (D4a).** Test rows may
  name sibling specs and shipped entries in function
  positions. *S–M.*
- **Phase 133 — Encoder UF arm (D6a).** Function parameters
  bind as uninterpreted functions; the probe spec's class
  reaches Verified; the corpora re-measured for verdict drift
  (none outside the frozen corpus). *S–M.*
- **Phase 134 — 1.2 amendment freeze.** `list_map`,
  `list_filter`, `list_fold`, `list_all`, `list_any`, `range`
  frozen at Tier-2 (D7a standard, tiers printed); `zip` and
  `is_permutation` recorded as excluded with their measured
  reasons. *S.*
- **Phase 135 — Amendment build.** Entries + batteries +
  reference docs; the JS emitter's higher-order arms so the
  npm surface answers the new entries' batteries; facade
  parity. *M.*
- **Phase 136 — 1.2 gate, merge, v1.2.0.** *M.*

## 6. Gate sketch (1.2)

In the established shape — every clause measurable on the
final tree:

1. **The language's own higher-order specs judge in-process.**
   `sort` at 6/6 and `binary_search` at 7/7 through
   `run_spec_tests` on the final tree, and the frozen G4
   corpus (apply-class specs) likewise — no function-valued
   judgement on the final tree depends on the compiled runner.
2. **The amendment stands at its tiers.** Every entry in the
   frozen 1.2 scope stands at its named tier (Tier-2
   executed-exact for the five higher-order entries and
   `range`), batteries green in-process, tiers printed in the
   scope doc and the guide.
3. **Verdict honesty.** No verdict changes outside the frozen
   corpus: the 1.0/1.1 corpora and the W3 pinned corpus
   re-measured; function-valued comparisons decline (D5);
   every body the UF arm cannot encode remains Inconclusive,
   named.
4. **The surfaces answer.** The npm package answers the
   amendment entries' batteries at 100% under node and the
   Rust facade likewise; the playground bundle within the
   standing 771,072 B budget; the cascade instrument (Codegen
   spec count 130, artifact bytes) reported — movement is a
   named finding, never a surprise.
5. **Suites green in all three configurations** on the final
   tree.

Draft closing line, in the established style:

> *1.2 closes when a Morphic spec can take a function, apply
> it, and be judged for it by the interpreter that ships —
> and when the five higher-order entries and `range` stand at
> their named tiers on the final tree, with `sort` and
> `binary_search` crowned by that interpreter rather than
> rescued by a compiler.*

## 7. Interactions, named

- **§3.9 (the List element encoder).** G4 does not need it:
  Tier-2 judging is executed, and the UF arm concerns scalar
  application, not elements. §3.9's price is unchanged by G4;
  its *payoff* grows — with elements (§3.9) and functions
  (this build) both present, the five higher-order entries
  become Tier-1 candidates, which neither campaign achieves
  alone. Sequencing stays as ratified: G4 first (D6a), §3.9
  as the headline campaign after.
- **The quantified-mod campaign (`int_gcd`'s return).**
  Independent — measured disjoint: that campaign is judge-side
  quantified Int reasoning plus a Euclid seed family; G4 is
  the value model plus application. No shared layer, no shared
  price. Either order works; the standing order keeps G4
  first.
- **The embed packages.** Function types have *no mapping* in
  EMBED_SCOPE §3 (the bindings generator maps `TypeRef::
  Function` to `never`) — a named non-mapping, frozen for 1.1.
  What G4 eventually unlocks there: host callbacks into
  Morphic functions and higher-order exports, priced when an
  embed amendment takes it (the JS arms are S–M atop G4,
  §4). **None of it is part of this build**: 1.2's npm work is
  only the emitter arms the amendment's own entries need
  (Phase 135), not a bindings change.
- **The bootstrap cascade.** Zero for the core build,
  measured (§1.6): the corpus mirrors emitter renderings, and
  G4 core changes no rendering. The two tripwires are named in
  the Phase 128 non-goals — `ty_to_rust`'s Function arm and
  the Sort template's generated signature (Phase 61's
  owner-reserved Clone-bound question; G4 neither needs nor
  takes it).

## Appendix — probe index

`tests/phase127_g4_probes.rs` (12 probes, all green at the
baseline configurations; verdict asserts follow the standing
`#[cfg(feature = "z3-support")]` discipline):

| Probe | Pins |
|---|---|
| `phase127_apply_spec_end_to_end` | §1.1/§1.2/§1.5: check Err verbatim; bindable 0/2 with the Unbindable reason verbatim; body unencodable; Inconclusive |
| `phase127_where_application_is_admitted` | §1.1: the five-shape admission taxonomy, incl. the Predicate side door |
| `phase127_apply_body_declines_by_name_in_eval` | §1.2: `call to unknown function `f``; `Lambda (function value)`; `Closure (function value)` |
| `phase127_fn_param_does_not_poison_unrelated_verification` | §1.5: `ignore_f` Verifies |
| `phase127_fn_equality_has_no_home` | §1.5/D5: `f == g` checks, Inconclusive |
| `phase127_named_function_reference_binds_nothing` | §1.2/D1: named refs check-Err in application, bind None as values |
| `phase127_bundled_examples_standings` | §1.2/§1.3: sort 0/6 + binary_search 0/7 bindable; sort template evaluable 0; binary_search template Scored 0/7 via the runner (G5) |
| `phase127_compiled_runner_judges_what_the_interpreter_cannot` | §1.3: apply body Scored 2/2; sort template in-process 0, compiled tpr 1.0 |
| `phase127_pipeline_crowns_for_the_bundled_ho_specs` | §1.3: seeded pipeline crowns — sort tpr 1.0, binary_search tpr 0.0 |
| `phase127_emitter_standings_per_target` | §1.6: Rust complete; JS `return undefined`; C `void*` + unsupported; Python pass; JS Map `return undefined` |
| `phase127_range_is_not_a_g4_item` | §2.1: range hand body judges 2/2 today |
| `phase127_zip_wall_is_tuples_not_functions` | §2.1: tuple values map to None on both sides of a zip test |
