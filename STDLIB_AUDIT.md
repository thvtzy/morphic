# STDLIB_AUDIT.md — Phase 106: the 1.0 standard-library audit

The feasibility + scope audit for the roadmap target **1.0 —
Production-ready, verified standard library**, in the lineage of
Phase 72 (`M1_FEATURES_AUDIT.md`), Phase 90
(`ENCODER_EXTENSION_DESIGN.md`) and Phase 99 (`F2_DESIGN.md`):
measurement and design only — no production code changed, no spec
amended. All measurements were taken on master @ `82b1b77`
(v0.7.0; M1 gate closed) through two named probe artifacts kept
in the tree: `tests/phase106_probes.rs` (direct-call surface,
hand-body verdicts, the import probe, the Option probe) and
`tests/phase106_pipeline_probes.rs` (production-pipeline runs:
`api::synthesize_seeded_spec`, seed 42, native default config
including the compiled-runner stage). Pinned inputs (a spec's own
tests / golden points) and spec-silent inputs are reported
separately throughout, per standing method.

---

## 0. Headline verdict

**1.0 is achievable, but not as "Appendix A, implemented."** The
honest shape is a **core standard library of roughly 25–30 entries
across Int, Char, String basics, List basics, and Result/Option
helpers**, shipped as a **hybrid**: the two builtins that already
exist (`len`, `int_to_str`) plus bundled `.morph` library modules
the compiler merges as a prelude — gated at the M1 standard
(every entry Z3-Verified ∧ golden-exact through the production
pipeline, or, for builtins, exact on the per-layer battery).

Three findings shape everything below:

1. **The Appendix A surface is a typechecker fiction.** Of the
   20 names the reference calls the standard library, exactly
   **2 are implemented** (`len`, `int_to_str`). The other 18
   typecheck — with coarse return types and, except for
   `int_to_str`, no argument validation at all — and then fail
   at every judge: not evaluable in the interpreter, unencodable
   in the verifier. A user who trusts Appendix A today writes
   specs that check green and can never be judged.
2. **Two new blockers the campaign history had not named** —
   the verifier **never assumes `require:` preconditions**, and
   **Float literals cannot be written in spec source at all**.
   The first makes a whole class of natural specs (clamp,
   divide, binary search) unverifiable *as written* even with a
   perfect hand-written body (measured: faithful `int_clamp`
   body, 3/3 executed, **VerificationFailed** on the
   precondition-violating counterexample `x=1, lo=1, hi=0`).
   The second closes the Float family before F3 is even
   reached.
3. **The List family is where the M1 standard gets expensive.**
   The verifier binds a `List` parameter as **its length only**
   — elements are unmodelled — so no element-level List entry
   (reverse, contains, sort, map) can be Z3-Verified today at
   any body quality. List entries are judgeable *executed*
   (compiled runner; `count_matching`'s template body scores
   1.000, `sort`'s 1.000 on the concretized harness) but the
   symbolic half of the M1 standard needs a list-element
   encoder — an XL item, the stdlib analogue of Phase 90's
   W-list. Whether 1.0 pays that price or runs a two-tier gate
   for List entries is the owner's D7 call below.

Everything else is buildable at S–M per family on machinery
that already exists (seed families, the statement judge,
invariants, the F2 builtin tradition) — once the packaging
spine exists, because **there is no module system**: `import`
is parsed and then ignored (probe: `morphic check` accepts a
module importing a nonexistent file).

---

## 1. Inventory — what exists today

### 1.1 The Appendix A surface (20 names, 2 real)

`LANGUAGE_REFERENCE.md` Appendix A ("Standard Library
(Planned)", "functions assumed to be available in
constraints") names: `is_sorted`, `is_permutation`, `len`,
`int_to_str`, `contains`, `reverse`, `sort`, `filter`, `map`,
`fold`, `sum`, `product`, `min`, `max`, `all`, `any`, `range`,
`zip`, `take`, `drop`. The typechecker's stdlib table
(`stdlib_return_type`, typeck.rs) accepts all 20 and returns a
coarse type; only `int_to_str` (Phase 100) has a strict
argument-validating arm.

Probe `phase106_appendix_a_direct_call_surface` writes, for
each name, a spec whose postcondition is `output == name(args)`
with pinned tests, and judges the direct-call body
`Call(name, args)` with an empty module index:

| Name | Typechecks | Interpreter | Verifier | Notes |
|---|---|---|---|---|
| `len` | ✓ | ✓ evaluates | Verified | Real builtin (UnaryOp::Len + call arm) |
| `int_to_str` | ✓ (strict) | ✓ evaluates | Verified | Real builtin (Phase 100) |
| `is_sorted` | ✓ | ✗ unknown function | Inconclusive | Name only |
| `is_permutation` | ✓ | ✗ | Inconclusive | Also a `Constraint::Predicate` — unencodable form |
| `contains` | ✓ | ✗ (tests bind) | Inconclusive | Element equality unmodelled anyway |
| `reverse` | ✓ | ✗ | Inconclusive | IR `UnaryOp::Reverse` exists; interpreter declines it |
| `sort` | ✓ | ✗ (tests **unbindable** — lambda comparator) | Inconclusive | G4 in the tests themselves |
| `filter` | ✓ | ✗ (tests unbindable) | Inconclusive | G4 |
| `map` | ✓ | ✗ (tests unbindable) | Inconclusive | G4; IR Map node renders in Rust codegen |
| `fold` | ✓ | ✗ (tests unbindable) | Inconclusive | G4 |
| `sum`, `product` | ✓ | ✗ | Inconclusive | Name only |
| `min`, `max` | ✓ | ✗ | Inconclusive | Name only (list forms) |
| `all`, `any` | ✓ | ✗ (tests unbindable) | Inconclusive | G4 |
| `range` | ✓ | ✗ | Inconclusive | Name only |
| `zip` | ✓ | ✗ | Inconclusive | Name only; tuple elements unmodelled |
| `take`, `drop` | ✓ | ✗ | Inconclusive | Name only |

"Tests unbindable" is the standing W3 pin: a test whose input
contains a lambda cannot be bound by the interpreter at all
(0 evaluable tests), so higher-order entries fail one layer
*earlier* than evaluation — at test binding.

### 1.2 The closed method table (4 methods, Result + List only)

`Result::{is_ok, is_err, unwrap}` and `List::len` (D1a,
Phase 76). **Option has no methods at all** — probe
`phase106_option_has_no_eliminator_in_spec_expressions`:
`x.unwrap()` and `x.is_some()` on `Option<Int>` are type
errors naming the closed table, and spec expressions have no
match form, so a spec can discriminate `None` from `Some` only
by equality and can **never project a symbolic `Some`
payload**. Option helpers are inexpressible in constraints
today, not merely unimplemented.

### 1.3 Operators and expression forms that are real

`++` (concat, F1), comparisons, arithmetic incl. `%`, string
indexing (totalized), `.len()`, `int_to_str`, record literals
and field access, `match` in *bodies* (not in constraints),
`forall` constraints over Int binders (Phase 94; the parser
discards the forall's collection expression — binders only).

### 1.4 The synthesis seed/template library (not 9 templates)

The README's "9 algorithmic patterns" is v0.4 archaeology.
Measured today: **3 structural templates**
(`SortPrimitive`, `AdjacentScan`, `BinarySearch` —
templates.rs; the other v0.4 builders were removed in Phase 59
as stale/pseudo-call), **13 constraint-seed families**
(equation, guard_table, guarded_identity, composition_guard,
multi_guard, expr_pin_guard, pair_guard_table,
mixed_pair_guard_table, complementary_guard, record_completion,
tokenize_template, char_equation, forall_fold), plus
test-induced, LLM and random seeds. This matters for 1.0: the
families are the reason scalar entries crown reliably
(§2), and none of them constructs a list-transforming body
from constraints — the forall-fold family builds For-loop
accumulations, which the interpreter declines and the
statement judge reaches only through denotations/invariants.

### 1.5 The examples corpus

8 `.morph` files, **156 specs, 301 in-spec tests**:
codegen.morph 130/221 (the self-hosting corpus), tokenizer
8/18, validator 7/22, printer 7/19, sort 1/6, binary_search
1/7, is_sorted 1/5, count_matching 1/3. There is **no `std/`
directory and no bundled `.morph` library anywhere in the
repo**. The CLI `build` compiles only the *first* spec of one
file. `morphic init`'s generated template references a
nonexistent `morphic-std` crate and a `use morphic::std::*;`
syntax the parser does not support — the packaging story a new
user meets today is fictional.

### 1.6 What a user must hand-roll today

Everything in §1.1's 18 dead names, plus: `abs`/`min`/`max`/
`clamp`/`sign` over scalars, all Char predicates beyond what a
spec states inline, string emptiness/prefix/suffix tests,
Option anything, `unwrap_or` (specifiable only via the guarded
method form — §2.5), list reversal/membership/slicing, and any
sorting or searching stated without lambda comparators
(the language's own sort/binary_search examples need lambdas
their own test harness cannot bind).

---

## 2. Candidate scope, by family — measured feasibility

Three standards are distinguished per entry, because the audit
kept finding them diverging:

- **S — specifiable**: a user can *state* the entry's contract
  in a constraint today.
- **H — hand-body verifiable**: a correct hand-written body
  reaches **Verified** under `verify_one` today.
- **P — pipeline-crownable**: the production pipeline (seed 42
  probe profile) crowns a winner that is Verified ∧ exact on
  the pinned tests today.

### 2.1 Int math — READY (the model family)

Hand probe: `int_abs` (UnaryOp::Abs body) — 2/2 executed,
**Verified**. Pipeline: scalar/guard-shaped specs are the seed
families' home territory. Candidates: `abs`, `sign`, `min`,
`max` (scalar), `clamp`, `is_even`/`is_odd`, `gcd` (While +
invariant — the Phase 93/96 machinery's shape), `pow` small,
`div`/`rem` totalized forms.

**The one caveat is §3.1**: `clamp`'s faithful body executes
3/3 but is **VerificationFailed** because the judge does not
assume `lo <= hi`. Pipeline `int_clamp`: Inconclusive, 1/3 —
the search gap and the judging gap are separate; both must
close for clamp-class entries (precondition-bearing specs) to
ship at standard. Entries stated as total functions with
implication-style rows verify today; that contortion should
not be the library's house style.

### 2.2 Char — READY

Pipeline probe `char_is_digit` (`'0' <= c && c <= '9'`):
**Verified**, 3/3/3 — the Phase 89 Char scalar encoding plus
the char_equation family. Candidates: `is_digit`,
`is_alpha`, `is_alphanumeric`, `is_whitespace` (already a
tokenizer spec — the library can adopt it), `is_uppercase`,
`is_lowercase`, `to_digit` (Option-returning; needs §2.5's
Option work for its *callers'* specs, not for its own body).

### 2.3 Float — BLOCKED at three independent layers

1. **Source**: the spec parser has **no FloatLit arm** — a
   decimal literal in a `.morph` file is a parse error
   (probe-pinned: `float_abs` written with `2.5` literals
   fails to parse). Float values exist only as AST/test
   values constructed programmatically.
2. **Encoder**: Float parameters bind **Opaque** in the
   verifier. Hand probe with AST-attached tests: `float_abs`
   executes 2/2, verifies **Inconclusive** — and always will,
   for any body, until Float gets a sort.
3. **Formatting (F3)**: no Float→String rendering exists in
   any layer; Rust's f64 Display (shortest round-trip) is not
   expressible over Z3 Reals even if layer 2 lands.

Candidates (`abs`, `min`, `max`, `floor`/`ceil` as Int
conversions, `is_nan`…) are all downstream of at least layers
1+2. **Recommendation: Float is out of 1.0** (D4) — it is its
own campaign (parser S, encoder L with NaN/Inf/div semantics
decisions, F3 L), not a library family.

### 2.4 String — split: basics READY, transforms killed by F5

Pipeline probe `string_is_empty` (`output == (len(s) == 0)`):
**Verified**, 2/2/2. The Z3 string bridge (Sym::Str/CharStr)
plus `len`/`++`/indexing covers: `is_empty`, `len` (have it),
`concat` (have it as `++`), `starts_with`/`ends_with` stated
via slicing-free rows where the encoder reaches, `repeat`
small, `trim`-class stated as properties.

**Transforms are the F5 wall, exactly as scoped**: pipeline
probe `string_reverse` — Inconclusive, 0/3, winner a junk
composite; there is no per-character iteration or
char→string conversion in the expression language, so a
`.morph`-authored reverse/upper/lower/replace cannot even be
*stated as a body*, and the constraint side can state
reversal only on literal lists/strings the encoder folds.
F5 kills: reverse, case mapping, replace, split/join,
per-char map — i.e. roughly the back half of any credible
String API — **for the bundled-module route**. The builtin
route can ship individual transforms as Rust-native arms
(the `int_to_str` tradition) at per-entry cascade cost (§4).
**Recommendation: 1.0 ships String basics; transforms are
out** unless the owner takes F5 as a language project (D5).

### 2.5 Result / Option — Result READY, Option needs one table decision

Hand probe `result_unwrap_or` (constructor-Match body over
the guarded-method spec `r.is_err() || output == r.unwrap()`
plus the Err row): 2/2 executed, **Verified** — the D1a
machinery carries the family. Pipeline probe: Inconclusive
1/2 — the pipeline crowns a test-overfit Match on the literal
test values; the gap is seed/search (a Match-from-rows seed
family is S–M), not judging. Candidates: `unwrap_or`,
`is_ok`/`is_err` (have them), `map_ok`-class bodies (Match in
bodies works), `ok_or`-style conversions.

Option: bodies are equally buildable (Match over
Some/None evaluates and encodes), but §1.2 — **no spec can
project a `Some` payload**, so Option entries' contracts are
unstatable for payload-bearing cases. Closing it is a D1a-
style extension of the method table to Option
(`is_some`/`is_none`/`unwrap`), S–M including the encoder and
interpreter arms; MethodCall has no IR kind, so **no
bootstrap cascade** rides on it (Phase 99's D1a finding).
**Recommendation: in** (D-list, part of the core build).

### 2.6 List, non-higher-order — executed YES, symbolic NO (the D7 family)

| Probe | Result |
|---|---|
| Hand `list_reverse` (UnaryOp::Reverse body) | BodyNotEvaluable ("unary operator Reverse is outside the evaluable subset"), Inconclusive |
| Pipeline `list_reverse` | Inconclusive, interp 1/2 (identity body wins the empty test) |
| Pipeline `list_contains` | Inconclusive, interp 2/3 (constant-false body) |
| Pipeline `list_map_inc` (IR Map body exists in codegen) | Inconclusive, interp 1/2 — Map node **BodyNotEvaluable** in-process; Rust codegen *does* render `.iter().map(...)` |
| Pipeline `sum_via_fold` | Inconclusive, interp 1/3 (constant-0 body) |
| Existing `count_matching` | Inconclusive; **pipeline tpr 1.000** via compiled runner (For body), interp 0/3 |
| Existing `sort` (generic) | Inconclusive; **pipeline tpr 1.000** via compiled runner on the concretized harness, interp 0/0/6 |
| Existing `binary_search` | Inconclusive; pipeline tpr **0.000** (G5), interp 0/0/7 |
| Existing `is_sorted` | Inconclusive; interp 4/5 via a constant-true winner — the AdjacentScan template body never reaches the compiled runner (§3.4) |

The pattern is uniform: **the compiled runner can execute
nearly all of it; the in-process interpreter declines For /
Reverse / the HO nodes; the verifier cannot model elements
at all** (List params bind as ListLen). So every List entry
today caps at Inconclusive, however good the body. Three
sub-questions decide the family's 1.0 shape:

- **Interpreter For/Reverse arms** (S / S): bounded, total
  semantics; closes the in-process judging gap for
  template-shaped bodies (count_matching class).
- **The is_sorted wall** (§3.4): S, owner-reserved.
- **List-element encoding** (XL): Z3 sequence/array model for
  lists — the only route to Verified for reverse/contains/
  sort-class entries. This is a verifier campaign on the
  scale of Phases 92–96, with its own design phase.

Length-level entries (`len`, `is_empty`, `count_where` stated
over lengths…) verify today where their rows are
length/forall-shaped; element-level entries do not.

### 2.7 Higher-order List (map/filter/fold/all/any) — BLOCKED by G4

The entries' own *tests* cannot bind (lambda inputs, §1.1);
the interpreter holds no function values (G4); the IR HO
nodes decline in-process and have no encoder arms; the
compiled runner judges some HO-shaped bodies only when a
concrete lambda is supplied by a *body*, never from a spec's
test. Codegen renders the nodes — codegen is the only layer
that believes in them. Closing = G4 (L: `Value::Function` +
closure semantics in eval and test binding) **plus** encoder
arms for the nodes (L) — or scoping HO out of 1.0 and
shipping the concrete instantiations users actually need
(`map` over named module functions is inexpressible without
function values too). **Recommendation: out of 1.0** (D6);
revisit for 1.1/1.2, where embedding will want them anyway.

### 2.8 Sorting / searching — the generation campaign's evidence, restated

`sort` and `binary_search` remain the two hardest specs in
the repo (Phases 57–62 record): sort's template body executes
perfectly through the compiled runner but its postcondition
pair (`is_sorted` forall-rows + `is_permutation` Predicate)
is unencodable in both halves; binary_search's template body
doesn't compile (G5: i64 indices into a Vec — the language
has no usize — and a by-value comparator on an Index place,
E0277), and its correctness rows need sortedness — a
precondition (§3.1) over elements (§2.6). As **library
entries** both are shippable at the *executed* standard today
(hand bodies + runner batteries) and at the *Verified*
standard only after the XL list encoder + §3.1. They should
be named explicitly in the gate either way — they are the
family where a silent two-tier standard would hide.

### 2.9 Set / Map — OUT, no representation

No Set/Map `Value`, no encoder sort, no literal syntax in
spec source beyond the type names the fresh_sym catch-all
binds Opaque. Nothing to audit further: out (D8), no probe
wasted.

### 2.10 Feasibility table (compressed)

| Family | S | H | P | 1.0 verdict |
|---|---|---|---|---|
| Int math | ✓ | ✓ (total-form) | ✓ scalar shapes | **Core** (clamp-class needs §3.1) |
| Char predicates | ✓ | ✓ | ✓ (Verified probe) | **Core** |
| Bool/truth helpers | ✓ | ✓ | ✓ | Core (fold into Int/Char rows) |
| String basics | ✓ | ✓ | ✓ (Verified probe) | **Core** |
| String transforms | partial | ✗ F5 | ✗ | Out (or builtin-priced singles) |
| Float | ✗ (no literals) | ✗ Opaque | ✗ | **Out** — own campaign |
| Result helpers | ✓ | ✓ Verified | ✗ seed gap (S–M) | **Core** |
| Option helpers | ✗ payload | ✓ bodies | ✗ | Core **after** Option table ext. (S–M) |
| List length-level | ✓ | ✓ | partial | **Core** |
| List element-level | ✓ | ✗ ListLen | ✗ (executed ✓ via runner) | **D7**: XL encoder, or two-tier, or defer |
| HO List | ✗ tests unbindable | ✗ G4 | ✗ | **Out** (G4 = L + encoder L) |
| Sort/search | ✓ | ✗ | executed-only | Named in gate; follows D7 |
| Set/Map | ✗ | ✗ | ✗ | **Out** |

---

## 3. Blockers, named and priced

Sizes: S ≈ one phase, M ≈ 2–3 phases, L ≈ a campaign slice,
XL ≈ a campaign. "Scope around" = 1.0 ships without closing
it, at the stated cost to the library's shape.

### 3.1 NEW (Phase 106): the verifier never assumes `require:` preconditions

`verify_one` and the statement judge encode postconditions
over **all** inputs; `require:` rows are parsed and used only
in the LLM prompt (engine.rs). Measured: faithful `int_clamp`
body, 3/3 executed, **VerificationFailed** with counterexample
`{x: 1, lo: 1, hi: 0}` — a point the spec's own precondition
excludes. Every precondition-bearing entry inherits this:
clamp (lo≤hi), div/mod (b≠0), sqrt-class, binary_search
(sortedness), slice (bounds). **Close: S–M** — assert the
encoded precondition rows as assumptions in both judges
(expression + statement), with the encodable-mirror updated
in lockstep and teeth (a body correct only off-precondition
must still Refute on-precondition; a spec with an
unencodable precondition declines whole, never silently).
**Scope around: possible but corrosive** — total functions +
implication-style postconditions everywhere; the library's
specs stop reading like contracts. **Recommendation: close
it, early (Phase 108)** — it is the cheapest blocker on this
list and it taxes every family until it lands.

### 3.2 NEW (Phase 106): Float literals unparseable in spec source

The source parser has no FloatLit arm; decimal literals are
a parse error (probe-pinned). **Close: S** (parser arm +
round-trip tests) — but worthless alone: §2.3 layers 2–3
stand behind it. Recorded so the Float campaign prices all
three layers, not just F3.

### 3.3 F3 — Float→String (Display)

Open since the generation campaign. Rust f64 Display is
shortest-round-trip decimal; over a Z3 Real sort there is no
exact rendering to fold to, and an `int_to_str`-style UF arm
would carry a heavier degradation surface (every Float
formatting row reads the payload). **Price: L**, gated on
the Float encoder (L) landing first. **Recommendation: out
of 1.0** with the rest of Float (D4).

### 3.4 The is_sorted judging-coverage wall (G1 residual)

The compiled runner — the standing workaround judge for
bodies the interpreter declines — deliberately does not fire
for Template provenance on specs whose tests the interpreter
*can* bind (`runner::trigger`). is_sorted's tests bind, its
correct AdjacentScan body is a For loop the interpreter
declines, so the body is never executed by any judge and the
pipeline crowns a constant-true 4/5 instead (probe §2.6).
**Close: S** — the trigger change is one line and is
owner-reserved precisely because it changes what the runner
certifies; pairing it with the interpreter For arm (§2.6)
makes the in-process judge reach the same bodies honestly.
**Recommendation: close it** as part of the List phase,
whichever D7 branch is taken — no List entry should ship
while its template body is unjudgeable.

### 3.5 G4 — interpreter function values

Lambda/Closure evaluation and lambda test-binding are the
interpreter's oldest named decline. It blocks: HO entries
entirely (§2.7), sort/binary_search's own example tests
(0 bindable), and any future entry taking a function.
**Close: L** (a `Value::Function` with captured environment,
eval arms for Lambda/Closure + HO nodes' in-process
semantics, test binding of lambdas). The compiled runner
already proves the semantics are uncontentious — this is
plumbing, not research. **Recommendation: out of 1.0's
critical path** (D6); schedule as the first 1.1-enabling
item, since embedders will ask for map/filter before
anything else.

### 3.6 G5 — binary_search generation discipline

Template body doesn't compile in the runner harness (i64
indices, no usize in the language; by-value comparator on an
Index place). **Close: M** (index-type discipline in the
template + runner concretization, or a language-level index
conversion). Blocks binary_search as a *synthesized* entry;
a hand body at the executed standard is unaffected.
**Recommendation: scope around in 1.0** (ship the hand body
if D7 admits executed-tier entries; fix with the List
campaign otherwise).

### 3.7 G7 — mutation stubs + LLM-seed coupling

Five mutation operators remain stubs, and the default CLI
profile (opt ≥ 2) aborts candidate generation when the LLM
seed errors — the stdlib crowning runs must therefore use
the seeded, LLM-free profile the probes used (which is also
the playground profile). **Close: M. Scope around: free**
(pin the gate's crowning runs to the seeded profile, as the
M1 gate's harness already does). **Recommendation: scope
around**; G7 stays backlog.

### 3.8 G1/G2 residuals

G1 is infrastructure-complete (the runner exists); its
residual is §3.4's trigger policy. G2 (mirror/rendering
coverage of the Codegen corpus) bites only if stdlib entries
must themselves be *self-hosted* through the synthesized
emitter — they must not (§4): bundled `.morph` modules are
compiled by the native pipeline, and only new *builtins*
touch the Codegen corpus. **No 1.0 price** under the
recommended packaging; flagged so the alternative (every
entry a builtin) prices honestly in §4.

### 3.9 The ListLen wall (the XL one)

List parameters bind as length-only symbols; element reads
on symbolic lists, element equality, and forall-over-elements
rows (whose collection expression the parser discards) are
unencodable. This single fact caps §2.6/§2.8 at
Inconclusive. **Close: XL** — a Z3 sequence/array element
model threaded through fresh_sym, encode_expr (Index on
symbolic lists), the forall arm (binders over elements with
the collection restored — a parser change), the statement
judge, and the encodable mirror; Phase 90's W-list is the
direct ancestor and its BMC/invariant machinery is the
landing surface. **Scope around: the D7 two-tier gate**
(List entries ship executed-exact, Verified where the
current encoder reaches) or **defer element-level entries**
to a 1.1/1.2 list campaign. No fourth option exists; the
audit's job is to make the owner choose with the price
attached.

### 3.10 Exists / Predicate constraint forms

`encode_constraint` declines `Exists` and `Predicate`
wholesale. `is_permutation` (a Predicate) is half of sort's
contract; Exists is the natural shape for membership/search
rows (contains, find). **Close: M** for an Int-binder Exists
arm (the forall arm's dual — but it only pays off once
binders can range over list *elements*, i.e. after §3.9);
Predicate needs a definition mechanism (named predicate =
spec call — partially present via resolver denotations).
**Recommendation: price inside the §3.9 campaign**, not as
a standalone 1.0 item.

### 3.11 For loops: interpreter + statement-encoder reach

The interpreter declines For; the statement judge reaches
For-bodies only via denotations. Natural list bodies
(templates included) are For-shaped. **Close: S (interpreter
arm — bounded iteration over a length the evaluator knows)
+ M (statement judge: desugar to the While/invariant
route)**. The interpreter half is recommended with §3.4 in
the List phase regardless of D7.

### 3.12 Price summary

| Blocker | Price | In 1.0? |
|---|---|---|
| §3.1 Precondition assumption gap (NEW) | S–M | **Yes — close first** |
| §3.4 is_sorted runner wall | S | **Yes** (with List phase) |
| §3.11 For in interpreter | S | **Yes** (with List phase) |
| Option method-table extension | S–M | **Yes** (core enabler) |
| §3.2 Float literal parsing | S | Only with Float (no) |
| §3.10 Exists arm | M | Inside §3.9 campaign |
| G5 binary_search discipline | M | Scope around |
| G7 mutation/LLM coupling | M | Scope around (seeded profile) |
| §3.3 F3 + Float encoder | L + L | **No** — Float campaign later |
| G4 function values | L (+L encoder) | **No** — first post-1.0 item |
| F5 string transforms (language feature) | L–XL | **No** — basics only |
| §3.9 List-element encoder | XL | **D7 decides** |

---

## 4. Packaging — how a standard library ships

### 4.1 Route (a): bundled `.morph` modules + imports — the mechanism does not exist

Probe `phase106_import_is_parsed_but_not_resolved_by_check`:
`import "std/collections.morph";` parses into `Spec.imports`
(name + optional alias), and `morphic check` returns ok for a
module importing a **nonexistent file**. Nothing in the
pipeline — parser, typechecker, SpecIndex, CLI — resolves,
loads, or merges an import. Qualified calls (`algo::is_sorted`)
are not parseable (call names are bare identifiers). What
*does* exist is intra-file multi-spec modules (the tokenizer
is 8 specs in one file; Codegen is 130) and a module-scoped
SpecIndex the resolver already composes through.

So route (a) honestly prices as: **a loader/merger (M)** —
resolve import paths against a bundled library directory,
parse + typecheck the target, merge its specs into the
importing module's index with collision rules — plus
qualified-name or alias semantics in the parser/typechecker
(M if namespaced calls are wanted; S if imports merge
unqualified with collision errors). Bootstrap cascade:
**none** — `.morph` modules never touch the Codegen corpus
unless their specs join it. Playground: library sources ship
as data (strings) in the bundle; ~1 KB per entry-spec,
compressible, and only the *index* grows the WASM graph.

### 4.2 Route (b): grow the builtin tables — priced by Phase 102

`int_to_str` is the complete price list for one builtin:
arms in the typechecker (table + strict validation),
interpreter, verifier (two sites + the D4a guard),
encodable mirror, Rust emitter, **and** — the expensive
surprise — a bootstrap cascade when the builtin must render
through the synthesized emitter: Phase 102 cost a new
Codegen spec family (129→130), +11 M3 cases, artifact
regeneration (+1,513 B), and full re-proofs, for **one**
name. Len-class builtins that predate the corpus ride
cheaper; any *new* builtin a stdlib entry needs in generated
code should be assumed to cost the cascade. At 25–30 entries
route (b) is **XL in aggregate** and makes the Codegen corpus
— the project's most load-bearing artifact — churn per
library addition forever. It remains the *only* route for
entries no `.morph` body can express (F5 transforms, Float
formatting).

### 4.3 Route (c): hybrid — RECOMMENDED

A **tiny builtin core** (what exists: `len`, `int_to_str`;
new only where a layer arm is unavoidable — the Option
method-table extension is spec-side and rideless, §2.5) plus
**bundled `.morph` library modules** for everything
expressible, delivered through a **compiler prelude**: the
compiler parses the bundled library sources and merges them
into every user module's index at check/synthesize time.
The prelude variant of route (a) is strictly cheaper than
user-facing imports (no path resolution, no alias syntax, no
per-file loader UX — the merge point is the SpecIndex
construction that already exists, S–M), it works identically
in CLI, API and playground (one merge site), and it does not
foreclose real imports later (the same merger serves both).
User-facing `import` resolution can then be its own
post-1.0 phase with the semantics designed once, not twice.

Consequences to record: bundled specs join the *seeded*
synthesis world — a library entry is a spec + its crowned
body, shipped as data; entries' bodies are found by the
pipeline at library-build time (CI), not at user time.
Bundle impact: library sources ≈ 25–40 KB of text against
**41,091 B of standing headroom** (bundle 729,981 B vs
budget 771,072 B) — tight enough that the gate (§5) must
carry a bundle clause and the library should ship compressed
or as a separately-loaded asset in the playground; the
native binary is unconstrained. Versioning: the library
versions with the compiler (no separate crate — the
`morphic init` template's fictional `morphic-std` reference
should be corrected to say so).

---

## 5. The 1.0 gate proposal

In the M1 gate's style — measurable clauses, a closing line,
no clause the machinery cannot check. Draft text for owner
ratification (Phase 107 freezes it with the scope):

1. **Scope frozen.** A scope document (the successor of this
   audit's §2.10) names every 1.0 entry with its signature,
   edge semantics (empty list, overflow, zero divisor,
   precondition), and tier. Nothing ships that is not named;
   nothing named is silently dropped — a dropped entry is a
   gate amendment, recorded.
2. **Every entry checks.** Each entry's spec, in the shipped
   library modules, passes `morphic check` as part of the
   prelude merge — the exact artifact users receive.
3. **Golden-exact.** Every entry carries in-spec tests
   covering empty/degenerate, singleton, typical, and
   adversarial points, plus a cross-validation battery
   against a hand-written Rust mirror; pinned points agree
   100%, spec-silent points reported separately (standing
   method).
4. **Verified.** For every Tier-1 entry, the production
   pipeline crowns a winner that is **Z3-Verified ∧
   golden-exact**; for every builtin, the per-layer battery
   (interpreter / encoder fold / compiled runner agreement,
   the Phase 100 precedent) is exact on its full battery
   incl. extremes. **No entry ships on executed evidence
   alone** — unless the owner's D7 decision creates an
   explicit Tier-2 (List element-level entries: executed-
   exact through the compiled runner on the full battery,
   Verified where the encoder reaches, tier printed in the
   entry's doc). The tier list is part of the frozen scope,
   not a footnote discovered at the gate run.
5. **Cross-target honesty.** Every entry renders on every
   codegen target that claims it; a target that cannot render
   an entry says so at build time — the silent free-call
   fallback that §1.1's dead names hide behind is removed
   for library entries (measured by generated-Rust compile +
   run at minimum).
6. **Budgets.** Playground bundle within the standing budget
   (no silent re-baseline); library build (all entries
   crowned from clean) completes within a stated wall-clock
   on the CI machine; per-entry synthesis inside the
   playground profile's caps for the interactive subset.
7. **Documentation.** Appendix A is rewritten from
   "(Planned)" to per-entry reality: signature, tier,
   semantics, one example per entry; every shipped entry has
   a doc comment in its module. The 18 dead names either ship
   or are removed from the table — the fiction ends either
   way.

**Closing line** (the M1 line, restated for a library):
*Every entry in the frozen 1.0 scope stands Z3-Verified ∧
100% golden-exact through the production pipeline — or, for
builtins, exact on the per-layer battery; for Tier-2 entries
(if D7 creates the tier), executed-exact on the full battery
with the tier named in its documentation — on the final
tree, with the bundle inside budget.*

**Deliberately left out**, with reasons: per-entry
complexity guarantees (the verifier does not enforce
`optimize:` bounds — a gate clause claiming them would be
unmeasurable); Set/Map and Float entries (§2.3/§2.9 — not
scope, silence); HO entries (§2.7); a user-facing import
mechanism (§4.3 — the prelude ships the library; imports are
a language feature with their own gate); API-stability
promises beyond the release (1.0 is the first version the
promise could attach to; the gate should not pre-commit
1.x policy).

---

## 6. Build plan (phases after 106) and owner decisions

### 6.1 Phase plan

Sizes assume the recommended decisions (D1 core scope, D2
hybrid/prelude, D3 close, D4/D5/D6 out, D7 = owner's pick —
the plan forks at Phase 112 and both branches are priced).

| Phase | Work | Size |
|---|---|---|
| 107 | Ratify D1–D8; freeze scope + gate text (`STDLIB_SCOPE.md`; Appendix A marked interim) | S |
| 108 | §3.1 precondition assumption in both judges + mirror + teeth | S–M |
| 109 | Packaging spine: bundled library modules + prelude merge (CLI/API/playground parity), collision rules, `morphic init` template corrected | M |
| 110 | Int + Char core entries: specs, crowning runs, mirrors, docs (≈12 entries) | M |
| 111 | String basics + Result helpers + Option table extension and Option helpers (≈10 entries) | M |
| 112a *(D7 = close §3.9)* | List-element encoder campaign: design (112) + build phases — sequences/arrays, Index on symbolic lists, forall-over-elements (parser collection restore), Exists arm, statement-judge parity | **XL** (design M + 3–5 build phases) |
| 112b *(D7 = two-tier / defer)* | List phase on current machinery: interpreter For + Reverse arms (S), §3.4 trigger (S), length-level entries Verified, element-level entries at Tier-2 executed standard with runner batteries; sort/binary_search hand bodies (G5 scoped around) | M–L |
| 113 | Cross-target honesty pass (clause 5 machinery), bundle packaging of the library, docs rewrite of Appendix A | M |
| 114 | Gate run (all clauses measured on the final tree) + merge + **v1.0.0** release | S–M |

Total: **≈ L** on branch 112b; **≈ XL+** on branch 112a.
Either way Phases 107–111 (≈ M–L) are common prefix and no
work is wasted by deciding D7 after 111 — the encoder
campaign's design phase can even run parallel to 110/111 if
the owner wants the option kept warm.

### 6.2 Owner decisions (D-style, with recommendations)

- **D1 — Scope size.** (a) **Core** (~25–30 entries: Int,
  Char, String basics, List per D7, Result/Option) —
  *recommended*: it is what the machinery can hold at the
  M1 standard. (b) Broad (all 20 Appendix names + full
  String/Float) — not achievable at standard today at any
  reasonable price; choosing it chooses the Float + F5 +
  G4 campaigns too.
- **D2 — Packaging.** (a) **Hybrid, prelude-delivered**
  (§4.3) — *recommended*. (b) All builtins — XL aggregate,
  perpetual Codegen churn. (c) Bundled modules with
  user-facing imports in 1.0 — pays the import design now
  for no library benefit over the prelude.
- **D3 — Preconditions.** (a) **Close §3.1 in Phase 108** —
  *recommended* (S–M, taxes everything until closed).
  (b) Scope around with total/implication-style specs —
  the library's contracts stop reading like contracts.
- **D4 — Float.** (a) **Out of 1.0** — *recommended*
  (three-layer block, §2.3; its own campaign later).
  (b) In — adds parser S + encoder L + F3 L and delays
  everything else.
- **D5 — F5 / String transforms.** (a) **Out; basics
  only** — *recommended*. (b) In as a language feature
  (L–XL). (c) Ship 2–3 transforms as builtins at cascade
  price each — only if a specific transform is judged
  indispensable; name it in Phase 107.
- **D6 — Higher-order entries (G4).** (a) **Out of 1.0;
  G4 is the first post-1.0 build** — *recommended*.
  (b) In — L + L and it re-opens encoder scope mid-gate.
- **D7 — The List standard.** (a) **Two-tier gate**
  (Tier-2 = executed-exact, tier documented per entry) —
  *recommended for 1.0*: the library ships whole, the
  standard bends exactly where the audit measured it
  must, and §3.9 becomes the headline 1.1/1.2 campaign
  with the library as its test corpus. (b) Close §3.9
  first (XL) — purest standard, 1.0 slips by a campaign.
  (c) Defer element-level entries — smallest library;
  sort/reverse/contains absent from a "production-ready"
  stdlib is a visible hole.
- **D8 — Set/Map.** (a) **Out** — *recommended* (no
  representation exists). (b) In — a type-system project,
  not a library family.

---

## 7. Probe inventory + suite record

Probes (kept, named, on this branch): `tests/phase106_probes.rs`
— `phase106_appendix_a_direct_call_surface` (20-name table,
§1.1), `phase106_hand_body_boundaries` (int_abs Verified;
int_clamp 3/3-executed/VerificationFailed §3.1; float source
parse error + AST float_abs 2/2/Inconclusive §2.3;
result_unwrap_or Verified; list_reverse BodyNotEvaluable;
map_inc BodyNotEvaluable), `phase106_option_has_no_eliminator_in_spec_expressions`
(§1.2), `phase106_import_is_parsed_but_not_resolved_by_check`
(§4.1). `tests/phase106_pipeline_probes.rs` — 14 production
pipeline runs (§2): int_clamp, char_is_digit, string_is_empty,
result_unwrap_or, string_reverse, list_map_inc, list_reverse,
sum_via_fold, list_contains, sort_ints, and the four standing
example specs (is_sorted, count_matching, sort, binary_search)
read from their files unmodified.

Suite record (this branch, final tree): **573/0/30 default,
573/0/30 bootstrap-codegen, 444/0/22 no-Z3** (the standing
`--no-default-features --features wasm-playground` profile) —
in every configuration exactly the v0.7.0 baseline (555/555/
426) **plus the 18 probe tests** (4 + 14), zero failures.
The two verdict-bearing probes gate their Z3 assertions
behind `#[cfg(feature = "z3-support")]` (the Phase 99/100
precedent); their executed/typecheck facts are pinned in
every profile, and the pipeline probes assert only pipeline
completion in every profile — the outcome line is the datum.
