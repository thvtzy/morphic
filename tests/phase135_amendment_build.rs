//! Phase 135 — the 1.2 higher-order library amendment build
//! (STDLIB_SCOPE.md Amendment 2, frozen at Phase 134;
//! G4_SCOPE.md D8's second freeze). The six entries —
//! `list_map`, `list_filter`, `list_fold`, `list_all`,
//! `list_any`, `range` — ship at Tier-2: hand bodies in the
//! Phase 115 For-loop vocabulary (the one vocabulary both
//! executed judges reach in full — the Map/Filter/Fold IR
//! forms do not compile in Rust under the standing
//! rendering, measured at Phase 135: E0631, a fn pointer
//! is not `FnMut(&i64) -> i64`), crowned executed-exact on
//! the frozen batteries (A2.3: 105 pinned points,
//! spec-silent 0) judged in-process through the
//! index-carrying route, so lambda-supplied and
//! named-supplied rows both bind. The `range`
//! prelude-merge check is a named instrument of the
//! freeze (A2.7 clause 2): the entry and the
//! quantifier-domain spelling `range(0, len(xs) - 1)`
//! must coexist through the prelude merge.
//!
//! The battery point tables in this file are the single
//! source the three executed routes share: the in-process
//! index route (below), the npm clean-host rehearsal (JS
//! text rendered from the same tables) and the facade
//! host rehearsal (Rust text rendered from the same
//! tables) — A2.3's "the same points are the packages'
//! host batteries".
//!
//! Configuration note (the standing one): pins that judge
//! or render the NATIVE prelude are gated
//! `#[cfg(not(feature = "wasm-playground"))]`.

use morphic::spec::ast::{FunctionSpec, TypeRef};
use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::engine::{
    CandidateImplementation, CollectionKind, IRBinOp, IRLiteral, IRNode, IRUnaryOp,
    Provenance, ScoreBreakdown, SpecIndex,
};
use morphic::synthesis::eval::{self, TestOutcome};
#[cfg(feature = "z3-support")]
use morphic::verify::verdict::VerificationStatus;
#[cfg(feature = "z3-support")]
use morphic::verify::verifier::Verifier;

// ── IR construction helpers (mirroring src/stdlib.rs) ───

fn v(n: &str) -> IRNode {
    IRNode::Var(n.into())
}
fn i(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}
fn boolean(b: bool) -> IRNode {
    IRNode::Literal(IRLiteral::Bool(b))
}
fn bop(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
    IRNode::BinOp(op, Box::new(l), Box::new(r))
}
fn if_(c: IRNode, t: IRNode, e: IRNode) -> IRNode {
    IRNode::If {
        condition: Box::new(c),
        then_branch: Box::new(t),
        else_branch: Box::new(e),
    }
}
fn block(stmts: Vec<IRNode>) -> IRNode {
    IRNode::Block(stmts)
}
fn alloc(name: &str, ty: TypeRef, initial: IRNode) -> IRNode {
    IRNode::Alloc {
        name: name.into(),
        ty,
        initial: Box::new(initial),
    }
}
fn assign(name: &str, value: IRNode) -> IRNode {
    IRNode::Assign {
        target: Box::new(v(name)),
        value: Box::new(value),
    }
}
fn for_(var: &str, start: IRNode, end: IRNode, body: IRNode) -> IRNode {
    IRNode::For {
        var: var.into(),
        start: Box::new(start),
        end: Box::new(end),
        body: Box::new(body),
    }
}
fn len_u(x: IRNode) -> IRNode {
    IRNode::UnaryOp(IRUnaryOp::Len, Box::new(x))
}
fn idx(coll: IRNode, index: IRNode) -> IRNode {
    IRNode::Index(Box::new(coll), Box::new(index))
}
fn empty_list() -> IRNode {
    IRNode::Collection {
        kind: CollectionKind::List,
        elements: vec![],
    }
}
fn append_stmt(name: &str, item: IRNode) -> IRNode {
    bop(IRBinOp::Append, v(name), item)
}
fn call(f: &str, args: Vec<IRNode>) -> IRNode {
    IRNode::Call {
        function: f.into(),
        args,
    }
}
fn ret(x: IRNode) -> IRNode {
    IRNode::Return(Box::new(x))
}
fn list_ty() -> TypeRef {
    TypeRef::List(Box::new(TypeRef::Int))
}
fn dbg(node: &IRNode) -> String {
    format!("{:?}", node)
}

// ── The six hand bodies, recorded ───────────────────────

fn expected_body(name: &str) -> IRNode {
    match name {
        "list_map" => block(vec![
            alloc("out", list_ty(), empty_list()),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                append_stmt("out", call("f", vec![idx(v("xs"), v("i"))])),
            ),
            ret(v("out")),
        ]),
        "list_filter" => block(vec![
            alloc("out", list_ty(), empty_list()),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                if_(
                    call("p", vec![idx(v("xs"), v("i"))]),
                    append_stmt("out", idx(v("xs"), v("i"))),
                    block(vec![]),
                ),
            ),
            ret(v("out")),
        ]),
        "list_fold" => block(vec![
            alloc("acc", TypeRef::Int, v("init")),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                assign("acc", call("f", vec![v("acc"), idx(v("xs"), v("i"))])),
            ),
            ret(v("acc")),
        ]),
        "list_all" => block(vec![
            alloc("ok", TypeRef::Bool, boolean(true)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                assign(
                    "ok",
                    bop(
                        IRBinOp::And,
                        v("ok"),
                        call("p", vec![idx(v("xs"), v("i"))]),
                    ),
                ),
            ),
            ret(v("ok")),
        ]),
        "list_any" => block(vec![
            alloc("ok", TypeRef::Bool, boolean(false)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                assign(
                    "ok",
                    bop(
                        IRBinOp::Or,
                        v("ok"),
                        call("p", vec![idx(v("xs"), v("i"))]),
                    ),
                ),
            ),
            ret(v("ok")),
        ]),
        "range" => block(vec![
            alloc("out", list_ty(), empty_list()),
            for_("i", v("lo"), v("hi"), append_stmt("out", v("i"))),
            ret(v("out")),
        ]),
        other => panic!("no recorded body for {other}"),
    }
}

const SIX: [&str; 6] = [
    "list_map",
    "list_filter",
    "list_fold",
    "list_all",
    "list_any",
    "range",
];

/// The frozen doc texts (Amendment 2 §A2.1), verbatim.
fn frozen_doc(name: &str) -> &'static str {
    match name {
        "list_map" => "list_map(xs, f) = the list of f applied to each element of xs, in order; the empty list gives the empty list (STDLIB_SCOPE.md Amendment 2). Tier-2: executed-exact on the full battery; Tier-1 awaits the list-element encoder (3.9) together with the uninterpreted-function arm (G4_AUDIT.md 2.1).",
        "list_filter" => "list_filter(xs, p) = the elements of xs for which p holds, in their original order; the empty list gives the empty list, and a predicate that holds for no element gives the empty list (STDLIB_SCOPE.md Amendment 2). Tier-2: executed-exact on the full battery; Tier-1 awaits the list-element encoder (3.9) together with the uninterpreted-function arm (G4_AUDIT.md 2.1).",
        "list_fold" => "list_fold(xs, init, f) = the left fold of xs under f, starting from the accumulator init: f(f(f(init, x0), x1), ...) in element order; the empty list gives init (STDLIB_SCOPE.md Amendment 2). Tier-2: executed-exact on the full battery; Tier-1 awaits the list-element encoder (3.9) together with the uninterpreted-function arm (G4_AUDIT.md 2.1).",
        "list_all" => "list_all(xs, p) = true iff p holds for every element of xs; the empty list gives true (vacuous truth) (STDLIB_SCOPE.md Amendment 2). Tier-2: executed-exact on the full battery; Tier-1 awaits the list-element encoder (3.9) together with the uninterpreted-function arm (G4_AUDIT.md 2.1).",
        "list_any" => "list_any(xs, p) = true iff p holds for at least one element of xs; the empty list gives false (STDLIB_SCOPE.md Amendment 2). Tier-2: executed-exact on the full battery; Tier-1 awaits the list-element encoder (3.9) together with the uninterpreted-function arm (G4_AUDIT.md 2.1).",
        "range" => "range(lo, hi) = the ascending list [lo, lo + 1, ..., hi - 1]; lo == hi gives the empty list, and lo > hi gives the empty list — there is no descending form and no precondition (STDLIB_SCOPE.md Amendment 2). Tier-2: executed-exact on the full battery; element-level verification awaits the list-element encoder (3.9).",
        other => panic!("no frozen doc for {other}"),
    }
}

const LIST_SRC: &str = include_str!("../stdlib/list.morph");

fn list_specs() -> Vec<FunctionSpec> {
    let module = parser::parse(LIST_SRC).expect("list module parses");
    typeck::check(module).expect("list module checks").spec.functions
}

fn spec_of(specs: &[FunctionSpec], name: &str) -> FunctionSpec {
    specs
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("{name} is a list entry"))
        .clone()
}

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 135,
        body,
        spec_name: spec.name.clone(),
        spec: spec.clone(),
        score: 0.0,
        scores: ScoreBreakdown {
            test_pass_ratio: 0.0,
            constraint_score: 0.0,
            complexity_score: 0.0,
            quality_score: 0.0,
            synthesis_time_us: 0,
        },
        generation: 0,
        provenance: Provenance::HumanGuided,
        verified: false,
        verification_status: None,
    }
}

/// The prelude-merged index over the whole shipped
/// library — the index the index-carrying route judges
/// named rows through.
fn library_index() -> SpecIndex {
    SpecIndex::from_specs(&stdlib::prelude_specs())
}

// ── Spec-surface pins ───────────────────────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase135_six_specs_carry_the_frozen_surface() {
    let specs = list_specs();
    assert_eq!(
        specs.len(),
        18,
        "the list module: the standing 12 + the amendment's six"
    );
    for name in SIX {
        let spec = spec_of(&specs, name);
        assert_eq!(
            spec.doc.as_deref(),
            Some(frozen_doc(name)),
            "{name}: the doc text is Amendment 2's, verbatim"
        );
        let shipped = stdlib::shipped_body(name)
            .unwrap_or_else(|| panic!("{name} ships a body"));
        assert_eq!(
            dbg(&shipped),
            dbg(&expected_body(name)),
            "{name}: the shipped body is the recorded hand body"
        );
    }
    // The frozen signatures (A2.1), in the manifest's
    // spelling (morphic_signature's mechanical form).
    let sig = |name: &str| {
        morphic::codegen::bindings::morphic_signature(&spec_of(&specs, name))
    };
    assert_eq!(sig("list_map"), "(List<Int>, fn(Int) -> Int) → List<Int>");
    assert_eq!(sig("list_filter"), "(List<Int>, fn(Int) -> Bool) → List<Int>");
    assert_eq!(sig("list_fold"), "(List<Int>, Int, fn(Int, Int) -> Int) → Int");
    assert_eq!(sig("list_all"), "(List<Int>, fn(Int) -> Bool) → Bool");
    assert_eq!(sig("list_any"), "(List<Int>, fn(Int) -> Bool) → Bool");
    assert_eq!(sig("range"), "(Int, Int) → List<Int>");
    // Parameter names are the frozen ones (A2.1).
    let params = |name: &str| -> Vec<String> {
        spec_of(&specs, name)
            .params
            .iter()
            .map(|p| p.name.clone())
            .collect()
    };
    assert_eq!(params("list_map"), ["xs", "f"]);
    assert_eq!(params("list_filter"), ["xs", "p"]);
    assert_eq!(params("list_fold"), ["xs", "init", "f"]);
    assert_eq!(params("list_all"), ["xs", "p"]);
    assert_eq!(params("list_any"), ["xs", "p"]);
    assert_eq!(params("range"), ["lo", "hi"]);
    // The library stands at 35 entries + 2 builtins.
    assert_eq!(stdlib::prelude_specs().len(), 35, "35 library entries");
}

/// Every in-spec test of the six passes through the
/// index-carrying route — named rows bind (an unbound
/// named row would surface as Unbindable, never a skip),
/// and each HO entry carries at least one lambda-supplied
/// and one named-supplied row (A2.3's in-spec obligation).
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase135_in_spec_tests_pass_through_the_index_route() {
    let specs = list_specs();
    let index = library_index();
    for name in SIX {
        let spec = spec_of(&specs, name);
        let body = stdlib::shipped_body(name).unwrap();
        let outcomes = eval::test_outcomes_in(&body, &spec, &index);
        assert_eq!(
            outcomes.len(),
            spec.tests.len(),
            "{name}: one outcome per in-spec test"
        );
        assert!(
            outcomes.iter().all(|o| matches!(o, TestOutcome::Passed)),
            "{name}: every in-spec test passes through the index route \
             (a named row must bind, never skip), got {outcomes:?}"
        );
        let run = eval::run_spec_tests_in(&body, &spec, &index);
        assert_eq!(
            (run.passed, run.evaluable),
            (spec.tests.len(), spec.tests.len()),
            "{name}: executed-exact in-process, no spec-silent point"
        );
        println!(
            "P135PIN|{name}|in-spec {}/{} via index route",
            run.passed, run.evaluable
        );
    }
    // The supply-form classes, read off the checked
    // specs: a lambda row carries an Expr::Lambda in its
    // input; a named row carries a Var naming a shipped
    // Int entry in function position.
    for name in ["list_map", "list_filter", "list_fold", "list_all", "list_any"] {
        let spec = spec_of(&specs, name);
        let inputs: Vec<String> = spec
            .tests
            .iter()
            .map(|t| format!("{:?}", t.input))
            .collect();
        assert!(
            inputs.iter().any(|t| t.contains("Lambda")),
            "{name}: at least one lambda-supplied in-spec row"
        );
        assert!(
            inputs.iter().any(|t| {
                [
                    "int_abs", "int_sign", "int_is_even", "int_is_odd", "int_min",
                    "int_max",
                ]
                .iter()
                .any(|n| t.contains(&format!("Var(\"{n}\")")))
            }),
            "{name}: at least one named-supplied in-spec row"
        );
    }
}

/// THE RANGE PRELUDE-MERGE CHECK (A2.4.1 / A2.7 clause 2,
/// a named instrument): the `range` entry and the
/// quantifier-domain spelling `range(0, len(xs) - 1)` —
/// which the standing `list_is_sorted` spec already
/// carries — coexist through the prelude merge: every
/// library module checks with `range` among the prelude
/// specs, a probe module using BOTH spellings checks,
/// and the domain row still means what it meant
/// (list_is_sorted's in-spec tests pass through the
/// merged index; its full crown is Phase 115's pin).
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase135_range_entry_coexists_with_the_quantifier_domain() {
    for (_, source) in stdlib::LIBRARY_MODULES {
        let module = parser::parse(source).expect("library module parses");
        typeck::check(module).expect("library module checks through the merge");
    }
    // A probe module using both spellings at once: the
    // entry called as a function, and the quantifier
    // domain over a two-bound range.
    let probe = r#"
Probe {
    spec span_len {
        input: lo: Int, hi: Int
        output: Int
        constraint: output == len(range(lo, hi))
        test { input: (2, 5); expect: 3 }
        test { input: (5, 2); expect: 0 }
    }
    spec sorted_prefix {
        input: xs: List<Int>
        output: Bool
        constraint: forall i: Int in range(0, len(xs) - 1): xs[i] <= xs[i + 1] implies output == true
        constraint: (exists i: Int in range(0, len(xs) - 1): xs[i] > xs[i + 1]) implies output == false
        test { input: ([1, 2, 3]); expect: true }
        test { input: ([1, 3, 2]); expect: false }
    }
}
"#;
    let module = parser::parse(probe).expect("probe parses");
    let _checked = typeck::check(module).expect("both range spellings check in one module");
    let index = library_index();
    // The domain spelling still judges: list_is_sorted's
    // own in-spec tests pass against its shipped body
    // through the merged index.
    let specs = list_specs();
    let sorted = spec_of(&specs, "list_is_sorted");
    let body = stdlib::shipped_body("list_is_sorted").unwrap();
    let run = eval::run_spec_tests_in(&body, &sorted, &index);
    assert_eq!(
        (run.passed, run.evaluable),
        (sorted.tests.len(), sorted.tests.len()),
        "list_is_sorted judges whole through the merge"
    );
    println!("P135PIN|range|prelude-merge check: entry + quantifier domain coexist");
}

/// The six's verdicts — the Tier-2 standing as Phase 142
/// (W4) leaves it. At Phase 135 all six verified
/// Inconclusive (their quantified element rows were
/// outside the encoder; nothing about them was refuted —
/// the Phase 115 pin form). Phase 141's W3 arms crowned
/// `range` — one of the eight named in LIST_ENCODER_
/// SCOPE §3 — so its pin retired in place per §9 R10:
/// `range` now Verifies with its shipped body. At Phase
/// 142 the fold-form arm crowned `list_fold` — §3.3
/// carries it as W4 + W5 jointly, and the composition
/// it needed is §2.9's Fold point (the prefix step over
/// UF applications), which the W4 build carries — so
/// its pin retires in place the same way
/// (tests/phase142_fold_form.rs). The four remaining
/// higher-order entries' element rows wait on W5's UF
/// composition proper (Phase 143) and stay Inconclusive.
#[cfg(feature = "z3-support")]
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase135_verdicts_the_tier2_standing_after_w4() {
    let specs = list_specs();
    for name in SIX {
        let spec = spec_of(&specs, name);
        let body = stdlib::shipped_body(name).unwrap();
        let verdict = Verifier::with_specs(&specs)
            .verify_one(&candidate(&spec, body), &spec)
            .status;
        if name == "range" {
            assert_eq!(
                verdict,
                VerificationStatus::Verified,
                "range: crowned at Phase 141 (W3) — Verified with its shipped body"
            );
        } else if name == "list_fold" {
            assert_eq!(
                verdict,
                VerificationStatus::Verified,
                "list_fold: crowned at Phase 142 (W4) — Verified with its shipped body"
            );
        } else {
            assert_eq!(
                verdict,
                VerificationStatus::Inconclusive,
                "{name}: element-level verdicts wait on W5's UF composition — \
                 Inconclusive is the honest Tier-2 standing"
            );
        }
    }
}

// ── The frozen batteries (A2.3): point tables ───────────
//
// Supplier tables: each function argument is spelled
// three ways — the Morphic test-input form, the
// JavaScript host form, the Rust host form — with a
// hand-written mirror computing the expectations. Named
// suppliers spell the shipped entry's name in all three
// hosts (the package's own export / the facade's own
// function is the argument — A2.7's named cases).

struct F1 {
    morph: &'static str,
    js: &'static str,
    rust: &'static str,
    m: fn(i64) -> i64,
}
struct P1 {
    morph: &'static str,
    js: &'static str,
    rust: &'static str,
    m: fn(i64) -> bool,
}
struct F2 {
    morph: &'static str,
    js: &'static str,
    rust: &'static str,
    m: fn(i64, i64) -> i64,
}

fn mi_inc(x: i64) -> i64 { x + 1 }
fn mi_dbl(x: i64) -> i64 { x * 2 }
fn mi_neg(x: i64) -> i64 { 0 - x }
fn mi_id(x: i64) -> i64 { x }
fn mi_c7(_: i64) -> i64 { 7 }
fn mi_sq(x: i64) -> i64 { x * x }
fn mi_abs(x: i64) -> i64 { x.abs() }
fn mi_sign(x: i64) -> i64 { x.signum() }

const INC: F1 = F1 { morph: "|x| x + 1", js: "(x) => x + 1n", rust: "|x: i64| x + 1", m: mi_inc };
const DBL: F1 = F1 { morph: "|x| x * 2", js: "(x) => x * 2n", rust: "|x: i64| x * 2", m: mi_dbl };
const NEG: F1 = F1 { morph: "|x| 0 - x", js: "(x) => 0n - x", rust: "|x: i64| 0 - x", m: mi_neg };
const IDF: F1 = F1 { morph: "|x| x", js: "(x) => x", rust: "|x: i64| x", m: mi_id };
const C7: F1 = F1 { morph: "|x| 7", js: "(_x) => 7n", rust: "|_: i64| 7", m: mi_c7 };
const SQ: F1 = F1 { morph: "|x| x * x", js: "(x) => x * x", rust: "|x: i64| x * x", m: mi_sq };
const ABS: F1 = F1 { morph: "int_abs", js: "int_abs", rust: "int_abs", m: mi_abs };
const SIGN: F1 = F1 { morph: "int_sign", js: "int_sign", rust: "int_sign", m: mi_sign };

fn pb_pos(x: i64) -> bool { x > 0 }
fn pb_even(x: i64) -> bool { x % 2 == 0 }
fn pb_odd(x: i64) -> bool { x % 2 != 0 }
fn pb_gt2(x: i64) -> bool { x > 2 }
fn pb_neg(x: i64) -> bool { x < 0 }
fn pb_ge0(x: i64) -> bool { x >= 0 }
fn pb_true(_: i64) -> bool { true }
fn pb_false(_: i64) -> bool { false }

const POS: P1 = P1 { morph: "|x| x > 0", js: "(x) => x > 0n", rust: "|x: i64| x > 0", m: pb_pos };
const EVENL: P1 = P1 { morph: "|x| x % 2 == 0", js: "(x) => x % 2n === 0n", rust: "|x: i64| x % 2 == 0", m: pb_even };
const GT2: P1 = P1 { morph: "|x| x > 2", js: "(x) => x > 2n", rust: "|x: i64| x > 2", m: pb_gt2 };
const NEGP: P1 = P1 { morph: "|x| x < 0", js: "(x) => x < 0n", rust: "|x: i64| x < 0", m: pb_neg };
const GE0: P1 = P1 { morph: "|x| x >= 0", js: "(x) => x >= 0n", rust: "|x: i64| x >= 0", m: pb_ge0 };
const TRUEP: P1 = P1 { morph: "|x| true", js: "(_x) => true", rust: "|_: i64| true", m: pb_true };
const FALSEP: P1 = P1 { morph: "|x| false", js: "(_x) => false", rust: "|_: i64| false", m: pb_false };
const EVEN: P1 = P1 { morph: "int_is_even", js: "int_is_even", rust: "int_is_even", m: pb_even };
const ODD: P1 = P1 { morph: "int_is_odd", js: "int_is_odd", rust: "int_is_odd", m: pb_odd };

fn mf_add(a: i64, x: i64) -> i64 { a + x }
fn mf_mul(a: i64, x: i64) -> i64 { a * x }
fn mf_sub(a: i64, x: i64) -> i64 { a - x }
fn mf_first(a: i64, _: i64) -> i64 { a }
fn mf_min(a: i64, x: i64) -> i64 { a.min(x) }
fn mf_max(a: i64, x: i64) -> i64 { a.max(x) }

const ADD: F2 = F2 { morph: "|a, x| a + x", js: "(a, x) => a + x", rust: "|a: i64, x: i64| a + x", m: mf_add };
const MUL: F2 = F2 { morph: "|a, x| a * x", js: "(a, x) => a * x", rust: "|a: i64, x: i64| a * x", m: mf_mul };
const SUB: F2 = F2 { morph: "|a, x| a - x", js: "(a, x) => a - x", rust: "|a: i64, x: i64| a - x", m: mf_sub };
const FIRST: F2 = F2 { morph: "|a, x| a", js: "(a, _x) => a", rust: "|a: i64, _: i64| a", m: mf_first };
const MINF: F2 = F2 { morph: "int_min", js: "int_min", rust: "int_min", m: mf_min };
const MAXF: F2 = F2 { morph: "int_max", js: "int_max", rust: "int_max", m: mf_max };

const MAX: i64 = i64::MAX;
const MIN: i64 = i64::MIN;

/// list_map: 20 points — 12 lambda, 8 named
/// (int_abs ×5, int_sign ×3), A2.3's classes in order.
fn map_points() -> Vec<(Vec<i64>, &'static F1)> {
    vec![
        (vec![], &INC),
        (vec![], &NEG),
        (vec![5], &DBL),
        (vec![1, 2, 3], &INC),
        (vec![3, -5, 0, -2], &NEG),
        (vec![1, 2, 3], &C7),
        (vec![MAX], &IDF),
        (vec![MIN], &IDF),
        (vec![4, 4, 4], &INC),
        (vec![3, 1, 4, 1, 5, 9, 2, 6], &DBL),
        (vec![0, 5], &INC),
        (vec![2, -3, 4], &SQ),
        (vec![3, -4, 0, -6], &ABS),
        (vec![-1, -2, -3], &ABS),
        (vec![], &ABS),
        (vec![-7], &ABS),
        (vec![MAX, -1], &ABS),
        (vec![5, -2, 9], &SIGN),
        (vec![-8, 0, -1], &SIGN),
        (vec![], &SIGN),
    ]
}

/// list_filter: 20 points — 12 lambda, 8 named
/// (int_is_even ×5, int_is_odd ×3).
fn filter_points() -> Vec<(Vec<i64>, &'static P1)> {
    vec![
        (vec![], &POS),
        (vec![1, 2, 3], &TRUEP),
        (vec![1, 2, 3], &FALSEP),
        (vec![5, 1, 4, 2, 3], &GT2),
        (vec![-3, -1, 2, -4], &NEGP),
        (vec![0, 1, -1], &POS),
        (vec![7], &POS),
        (vec![-7], &POS),
        (vec![2, 2, 3, 3], &EVENL),
        (vec![MAX, MIN], &POS),
        (vec![1, 2, 3, 4, 5, 6], &EVENL),
        (vec![], &EVENL),
        (vec![1, 2, 3, 4, 5, 6], &EVEN),
        (vec![], &EVEN),
        (vec![1, 3, 5], &EVEN),
        (vec![-4, -3, -2, -1], &EVEN),
        (vec![8], &EVEN),
        (vec![1, 2, 3, 4], &ODD),
        (vec![1, 3, 5], &ODD),
        (vec![], &ODD),
    ]
}

/// list_fold: 18 points — 12 lambda, 6 named
/// (int_min ×3, int_max ×3). Point 6 pins the LEFT
/// order: ((100 − 1) − 2) − 3 = 94; a right fold
/// would give 98.
fn fold_points() -> Vec<(Vec<i64>, i64, &'static F2)> {
    vec![
        (vec![], 42, &ADD),
        (vec![], 7, &MUL),
        (vec![5], 10, &ADD),
        (vec![1, 2, 3, 4], 0, &ADD),
        (vec![2, 3, 4], 1, &MUL),
        (vec![1, 2, 3], 100, &SUB),
        (vec![9, 9, 9], 5, &FIRST),
        (vec![-5, 5, -10], 3, &ADD),
        (vec![1, 2, 3], 0, &ADD),
        (vec![1, 2, 3], 10, &ADD),
        (vec![MAX, -1], 0, &ADD),
        (vec![3, 1, 4, 1, 5, 9, 2, 6], 0, &ADD),
        (vec![4, 2, 8], 99, &MINF),
        (vec![4, 2, 8], 1, &MINF),
        (vec![], 5, &MINF),
        (vec![4, 2, 8], 0, &MAXF),
        (vec![4, 2, 8], 99, &MAXF),
        (vec![-5, -1, -9], -7, &MAXF),
    ]
}

/// list_all: 16 points — 10 lambda, 6 named
/// (int_is_even ×3, int_is_odd ×3).
fn all_points() -> Vec<(Vec<i64>, &'static P1)> {
    vec![
        (vec![], &POS),
        (vec![1, 2, 3], &POS),
        (vec![0, 1, 2], &POS),
        (vec![1, 2, 0], &POS),
        (vec![7], &POS),
        (vec![0], &POS),
        (vec![-1, -2, -3], &NEGP),
        (vec![3, 0, 5], &GE0),
        (vec![2, 2, 2], &EVENL),
        (vec![2, 4, 6, 8, 10, 12], &EVENL),
        (vec![2, 4, 6], &EVEN),
        (vec![2, 3, 4], &EVEN),
        (vec![], &EVEN),
        (vec![1, 3, 5], &ODD),
        (vec![1, 2, 3], &ODD),
        (vec![-1, -3], &ODD),
    ]
}

/// list_any: 16 points — 10 lambda, 6 named
/// (int_is_even ×3, int_is_odd ×3).
fn any_points() -> Vec<(Vec<i64>, &'static P1)> {
    vec![
        (vec![], &POS),
        (vec![0, -1, -2], &POS),
        (vec![5, 0, 0], &POS),
        (vec![0, 0, 5], &POS),
        (vec![7], &POS),
        (vec![0], &POS),
        (vec![-1, -2], &NEGP),
        (vec![0, -1], &GE0),
        (vec![1, 2, 3], &POS),
        (vec![0, 0, 0], &EVENL),
        (vec![1, 4, 3], &EVEN),
        (vec![1, 3, 5], &EVEN),
        (vec![], &EVEN),
        (vec![2, 5, 4], &ODD),
        (vec![2, 4, 6], &ODD),
        (vec![-2, -3], &ODD),
    ]
}

/// range: 15 points — the audit probe's two first, in
/// the frozen spelling (A2.3).
fn range_points() -> Vec<(i64, i64)> {
    vec![
        (0, 3),
        (0, 0),
        (4, 4),
        (5, 2),
        (3, 0),
        (5, 6),
        (-3, -2),
        (2, 7),
        (0, 5),
        (-2, 2),
        (-5, -2),
        (10, 20),
        (-1, 0),
        (0, 1),
        (-7, -7),
    ]
}

fn range_mirror(lo: i64, hi: i64) -> Vec<i64> {
    (lo..hi).collect()
}

// ── Point text in the three hosts' spellings ────────────

fn fmt_int(n: i64) -> String {
    if n == i64::MIN {
        "(0 - 9223372036854775807 - 1)".to_string()
    } else if n < 0 {
        format!("(0 - {})", n.unsigned_abs())
    } else {
        n.to_string()
    }
}
fn fmt_list(xs: &[i64]) -> String {
    let parts: Vec<String> = xs.iter().map(|n| fmt_int(*n)).collect();
    format!("[{}]", parts.join(", "))
}
fn js_int(n: i64) -> String {
    format!("{n}n")
}
fn js_list(xs: &[i64]) -> String {
    let parts: Vec<String> = xs.iter().map(|n| js_int(*n)).collect();
    format!("[{}]", parts.join(", "))
}
fn rs_int(n: i64) -> String {
    match n {
        i64::MIN => "i64::MIN".to_string(),
        i64::MAX => "i64::MAX".to_string(),
        _ => n.to_string(),
    }
}
fn rs_list(xs: &[i64]) -> String {
    let parts: Vec<String> = xs.iter().map(|n| rs_int(*n)).collect();
    format!("vec![{}]", parts.join(", "))
}

/// The battery runner (the Phase 115 cross-validation
/// form, on the index route): a one-spec module carries
/// the entry's signature with the battery as its tests;
/// the shipped body must pass every pinned point — a
/// named row that does not bind fails here, never skips.
#[cfg(not(feature = "wasm-playground"))]
fn run_battery_in(entry: &str, signature: &str, pinned: Vec<(String, String)>) {
    let mut src = String::from("Battery135 { spec bat {\n");
    src.push_str(signature);
    src.push('\n');
    for (input, expect) in &pinned {
        src.push_str(&format!("    test {{ input: {input}; expect: {expect} }}\n"));
    }
    src.push_str("} }");
    let module = parser::parse(&src)
        .unwrap_or_else(|e| panic!("{entry}: battery source must parse: {e}"));
    let checked = typeck::check(module)
        .unwrap_or_else(|e| panic!("{entry}: battery source must check: {e}"));
    let spec = &checked.spec.functions[0];
    let body = stdlib::shipped_body(entry)
        .unwrap_or_else(|| panic!("{entry} ships a body"));
    let index = library_index();
    let outcomes = eval::test_outcomes_in(&body, spec, &index);
    assert_eq!(outcomes.len(), pinned.len(), "{entry}: battery size");
    for (idx, outcome) in outcomes.iter().enumerate() {
        assert!(
            matches!(outcome, TestOutcome::Passed),
            "{entry}: pinned point {} ({}) must pass, got {:?}",
            idx,
            pinned[idx].0,
            outcome
        );
    }
    println!("P135XV|{entry}|pinned={}|agree=100%", pinned.len());
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase135_batteries_agree_105_of_105_in_process() {
    let map: Vec<(String, String)> = map_points()
        .iter()
        .map(|(xs, f)| {
            (
                format!("({}, {})", fmt_list(xs), f.morph),
                fmt_list(&xs.iter().map(|x| (f.m)(*x)).collect::<Vec<_>>()),
            )
        })
        .collect();
    assert_eq!(map.len(), 20, "list_map battery: 20 pinned (A2.3)");
    run_battery_in(
        "list_map",
        "    input: xs: List<Int>, f: fn(Int) -> Int\n    output: List<Int>",
        map,
    );

    let filter: Vec<(String, String)> = filter_points()
        .iter()
        .map(|(xs, p)| {
            (
                format!("({}, {})", fmt_list(xs), p.morph),
                fmt_list(&xs.iter().copied().filter(|x| (p.m)(*x)).collect::<Vec<_>>()),
            )
        })
        .collect();
    assert_eq!(filter.len(), 20, "list_filter battery: 20 pinned (A2.3)");
    run_battery_in(
        "list_filter",
        "    input: xs: List<Int>, p: fn(Int) -> Bool\n    output: List<Int>",
        filter,
    );

    let fold: Vec<(String, String)> = fold_points()
        .iter()
        .map(|(xs, init, f)| {
            (
                format!("({}, {}, {})", fmt_list(xs), fmt_int(*init), f.morph),
                fmt_int(xs.iter().fold(*init, |a, x| (f.m)(a, *x))),
            )
        })
        .collect();
    assert_eq!(fold.len(), 18, "list_fold battery: 18 pinned (A2.3)");
    run_battery_in(
        "list_fold",
        "    input: xs: List<Int>, init: Int, f: fn(Int, Int) -> Int\n    output: Int",
        fold,
    );

    let all: Vec<(String, String)> = all_points()
        .iter()
        .map(|(xs, p)| {
            (
                format!("({}, {})", fmt_list(xs), p.morph),
                xs.iter().all(|x| (p.m)(*x)).to_string(),
            )
        })
        .collect();
    assert_eq!(all.len(), 16, "list_all battery: 16 pinned (A2.3)");
    run_battery_in(
        "list_all",
        "    input: xs: List<Int>, p: fn(Int) -> Bool\n    output: Bool",
        all,
    );

    let any: Vec<(String, String)> = any_points()
        .iter()
        .map(|(xs, p)| {
            (
                format!("({}, {})", fmt_list(xs), p.morph),
                xs.iter().any(|x| (p.m)(*x)).to_string(),
            )
        })
        .collect();
    assert_eq!(any.len(), 16, "list_any battery: 16 pinned (A2.3)");
    run_battery_in(
        "list_any",
        "    input: xs: List<Int>, p: fn(Int) -> Bool\n    output: Bool",
        any,
    );

    let range: Vec<(String, String)> = range_points()
        .iter()
        .map(|(lo, hi)| {
            (
                format!("({}, {})", fmt_int(*lo), fmt_int(*hi)),
                fmt_list(&range_mirror(*lo, *hi)),
            )
        })
        .collect();
    assert_eq!(range.len(), 15, "range battery: 15 pinned (A2.3)");
    run_battery_in(
        "range",
        "    input: lo: Int, hi: Int\n    output: List<Int>",
        range,
    );
}

// ── The crown-check wiring (A2.4.3) ─────────────────────

/// Non-mover obligation, measured: the 29 standing
/// entries' crown outcomes are IDENTICAL on the two
/// judging routes — every in-spec outcome of every
/// standing entry, context-free vs index-carrying.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase135_crown_outcomes_identical_on_both_routes_for_the_29() {
    let index = library_index();
    let mut total_points = 0usize;
    for spec in stdlib::prelude_specs() {
        if SIX.contains(&spec.name.as_str()) {
            continue;
        }
        let body = stdlib::shipped_body(&spec.name).unwrap();
        let free = eval::test_outcomes(&body, &spec);
        let indexed = eval::test_outcomes_in(&body, &spec, &index);
        assert_eq!(
            format!("{free:?}"),
            format!("{indexed:?}"),
            "{}: crown outcomes identical on both routes",
            spec.name
        );
        total_points += free.len();
    }
    println!(
        "P135PIN|crown-routes|29 entries, {total_points} in-spec points identical on both routes"
    );
}

/// The rewired crown check itself: `check_tier2_body`
/// now judges through the index route, so a Tier-2
/// entry whose in-spec tests carry named supply (the
/// six) verifies — and still refuses a wrong body. On
/// the context-free route every one of the six failed
/// this check (their named rows were Unbindable); that
/// is the hand-off Phase 132 recorded.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase135_crown_check_judges_through_the_index_route() {
    use morphic::codegen::package;
    let specs = list_specs();
    for name in SIX {
        let spec = spec_of(&specs, name);
        let shipped = stdlib::shipped_body(name).unwrap();
        assert!(
            package::check_tier2_body(&spec, &shipped).is_ok(),
            "{name}: the rewired crown check accepts the shipped body \
             (named rows bind through the index)"
        );
        let wrong = IRNode::Literal(IRLiteral::Int(0));
        assert!(
            package::check_tier2_body(&spec, &wrong).is_err(),
            "{name}: a wrong body is still refused by name"
        );
    }
    // The standing entries keep passing the same check.
    for name in ["list_sum", "list_reverse", "list_binary_search"] {
        let spec = spec_of(&specs, name);
        let shipped = stdlib::shipped_body(name).unwrap();
        assert!(
            package::check_tier2_body(&spec, &shipped).is_ok(),
            "{name}: the standing Tier-2 check stands"
        );
    }
}

// ── The JS emitter's higher-order arms (A2.4.2) ─────────
//
// Hand specs + constructed bodies in the Phase 120
// style: the arms' rendering text is pinned, and the
// rendered functions run under node against JS-supplied
// functions (battery agreement is the pin for the
// Closure snapshot seam, per the freeze).

#[cfg(not(feature = "wasm-playground"))]
fn hand_spec(source: &str, name: &str) -> FunctionSpec {
    let module = parser::parse(source).expect("hand module parses");
    module
        .functions
        .into_iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("spec {name} in hand module"))
}

#[cfg(not(feature = "wasm-playground"))]
fn js_of_hand(source: &str, name: &str, body: IRNode) -> String {
    let spec = hand_spec(source, name);
    morphic::codegen::generate(&candidate(&spec, body), "javascript")
        .expect("hand JS generation")
}

#[cfg(not(feature = "wasm-playground"))]
fn run_node(label: &str, module_js: &str, driver_js: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase135-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("mod.mjs"), module_js).unwrap();
    std::fs::write(dir.join("driver.mjs"), driver_js).unwrap();
    let out = std::process::Command::new("node")
        .arg(dir.join("driver.mjs"))
        .output()
        .unwrap_or_else(|e| panic!("{label}: node must run (node v24 on PATH): {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "{label}: node driver failed\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}\n--- module ---\n{module_js}"
    );
    stdout
}

#[cfg(not(feature = "wasm-playground"))]
const EQ_FN_JS: &str = r#"
function show(v) {
    return JSON.stringify(v, (k, x) => typeof x === "bigint" ? x.toString() + "n" : x);
}
function eq(a, b) {
    if (typeof a === "bigint" || typeof b === "bigint") return a === b;
    if (a === null || b === null) return a === b;
    if (Array.isArray(a) || Array.isArray(b)) {
        return Array.isArray(a) && Array.isArray(b) && a.length === b.length
            && a.every((v, i) => eq(v, b[i]));
    }
    return a === b;
}
"#;

#[cfg(not(feature = "wasm-playground"))]
fn cases_driver(fn_name: &str, cases_js: &str) -> String {
    format!(
        "import {{ {fn_name} }} from \"./mod.mjs\";\n{EQ_FN_JS}\nconst cases = {cases_js};\nlet fails = 0;\nfor (const [i, c] of cases.entries()) {{\n    let got;\n    try {{ got = {fn_name}(...c.args); }} catch (e) {{ console.log(`case ${{i}} threw: ${{e}}`); fails++; continue; }}\n    if (!eq(got, c.expect)) {{ console.log(`case ${{i}}: got ${{show(got)}} want ${{show(c.expect)}}`); fails++; }}\n}}\nconsole.log(`RESULT ${{cases.length}} ${{fails}}`);\nif (fails) process.exit(1);\n"
    )
}

#[cfg(not(feature = "wasm-playground"))]
fn lam1(body: IRNode) -> IRNode {
    IRNode::Lambda {
        params: vec![("x".to_string(), TypeRef::Int)],
        body: Box::new(body),
    }
}

const SPEC_APPLY1: &str = "Probe {\nspec apply1 {\n    input: x: Int, f: fn(Int) -> Int\n    output: Int\n    constraint: output == f(x)\n}\n}";
const SPEC_APPLY2: &str = "Probe {\nspec apply2 {\n    input: a: Int, b: Int, f: fn(Int, Int) -> Int\n    output: Int\n    constraint: output == f(a, b)\n}\n}";
const SPEC_LOCAL: &str = "Probe {\nspec with_local {\n    input: x: Int\n    output: Int\n    constraint: output == x + 1\n}\n}";
const SPEC_MAP: &str = "Probe {\nspec mapped {\n    input: xs: List<Int>\n    output: List<Int>\n    constraint: len(output) == len(xs)\n}\n}";
const SPEC_MAP_VAR: &str = "Probe {\nspec mapped_var {\n    input: xs: List<Int>, f: fn(Int) -> Int\n    output: List<Int>\n    constraint: len(output) == len(xs)\n}\n}";
const SPEC_FILTER: &str = "Probe {\nspec filtered {\n    input: xs: List<Int>\n    output: List<Int>\n    constraint: len(output) <= len(xs)\n}\n}";
const SPEC_FOLD: &str = "Probe {\nspec folded {\n    input: xs: List<Int>, init: Int\n    output: Int\n    constraint: len(xs) >= 0\n}\n}";
const SPEC_CLOSED: &str = "Probe {\nspec closed_over {\n    input: xs: List<Int>, k: Int\n    output: List<Int>\n    constraint: len(output) == len(xs)\n}\n}";

/// The application arm: a Call naming a bound function
/// variable (a function-typed parameter; a local bound
/// to a function value) renders as a direct call — and
/// runs. Every other unbound callee keeps the standing
/// refusal.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase135_js_application_arm() {
    // A function-typed parameter, applied.
    let js = js_of_hand(
        SPEC_APPLY1,
        "apply1",
        ret(call("f", vec![v("x")])),
    );
    assert!(js.contains("return f(x);"), "the application arm renders a direct call:\n{js}");
    let out = run_node(
        "apply1",
        &js,
        &cases_driver(
            "apply1",
            "[{ args: [5n, (x) => x * 3n], expect: 15n }, { args: [7n, (x) => 0n - x], expect: -7n }, { args: [0n, (x) => x + 100n], expect: 100n }]",
        ),
    );
    assert!(out.contains("RESULT 3 0"), "apply1 under node: {out}");

    // A two-argument function parameter.
    let js2 = js_of_hand(
        SPEC_APPLY2,
        "apply2",
        ret(call("f", vec![v("a"), v("b")])),
    );
    assert!(js2.contains("return f(a, b);"), "two-argument application:\n{js2}");
    let out2 = run_node(
        "apply2",
        &js2,
        &cases_driver(
            "apply2",
            "[{ args: [10n, 3n, (a, b) => a - b], expect: 7n }, { args: [2n, 5n, (a, b) => a * b], expect: 10n }]",
        ),
    );
    assert!(out2.contains("RESULT 2 0"), "apply2 under node: {out2}");

    // A local bound to a Lambda value, applied.
    let local_body = IRNode::Let {
        name: "g".into(),
        value: Box::new(lam1(bop(IRBinOp::Add, v("x"), i(1)))),
        body: Box::new(ret(call("g", vec![v("x")]))),
    };
    let js3 = js_of_hand(SPEC_LOCAL, "with_local", local_body);
    assert!(
        js3.contains("const g = ((x) => ((x + 1n)));"),
        "the Lambda arm renders the arrow form:\n{js3}"
    );
    assert!(js3.contains("return g(x);"), "the local applies directly:\n{js3}");
    let out3 = run_node(
        "local",
        &js3,
        &cases_driver("with_local", "[{ args: [41n], expect: 42n }, { args: [-1n], expect: 0n }]"),
    );
    assert!(out3.contains("RESULT 2 0"), "with_local under node: {out3}");

    // An unbound callee keeps the standing refusal, in
    // both positions.
    let js4 = js_of_hand(SPEC_APPLY1, "apply1", ret(call("nosuchfn", vec![v("x")])));
    assert!(
        js4.contains("return undefined;"),
        "an unbound callee in expression position keeps the refusal:\n{js4}"
    );
    let js5 = js_of_hand(
        SPEC_APPLY1,
        "apply1",
        block(vec![call("nosuchfn", vec![v("x")]), ret(v("x"))]),
    );
    assert!(
        js5.contains("// unsupported"),
        "an unbound callee in statement position keeps the refusal:\n{js5}"
    );
}

/// The Lambda / Closure / Map / Filter / Fold arms:
/// arrow forms, `.map` / `.filter` / `.reduce`
/// compositions, statement-position renderability, and
/// node agreement — including a Closure over a captured
/// parameter (the snapshot seam's battery pin) and a
/// Map whose function operand is the parameter itself.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase135_js_ho_node_arms() {
    // Map over a Lambda.
    let map_body = ret(IRNode::Map {
        function: Box::new(lam1(bop(IRBinOp::Add, v("x"), i(1)))),
        over: Box::new(v("xs")),
    });
    let js = js_of_hand(SPEC_MAP, "mapped", map_body);
    assert!(js.contains(".map("), "the Map arm composes .map:\n{js}");
    assert!(js.contains("=>"), "the Lambda arm is the arrow form:\n{js}");
    let out = run_node(
        "map",
        &js,
        &cases_driver(
            "mapped",
            "[{ args: [[1n, 2n, 3n]], expect: [2n, 3n, 4n] }, { args: [[]], expect: [] }, { args: [[-5n, 0n]], expect: [-4n, 1n] }]",
        ),
    );
    assert!(out.contains("RESULT 3 0"), "mapped under node: {out}");

    // Map whose function operand is the parameter Var.
    let map_var_body = ret(IRNode::Map {
        function: Box::new(v("f")),
        over: Box::new(v("xs")),
    });
    let js_mv = js_of_hand(SPEC_MAP_VAR, "mapped_var", map_var_body);
    assert!(js_mv.contains("(xs).map(f)"), "the Var operand composes directly:\n{js_mv}");
    let out_mv = run_node(
        "mapvar",
        &js_mv,
        &cases_driver(
            "mapped_var",
            "[{ args: [[3n, -4n], (x) => x < 0n ? -x : x], expect: [3n, 4n] }]",
        ),
    );
    assert!(out_mv.contains("RESULT 1 0"), "mapped_var under node: {out_mv}");

    // Filter over a Lambda predicate.
    let filter_body = ret(IRNode::Filter {
        predicate: Box::new(lam1(bop(
            IRBinOp::Eq,
            bop(IRBinOp::Mod, v("x"), i(2)),
            i(0),
        ))),
        over: Box::new(v("xs")),
    });
    let js_f = js_of_hand(SPEC_FILTER, "filtered", filter_body);
    assert!(js_f.contains(".filter("), "the Filter arm composes .filter:\n{js_f}");
    let out_f = run_node(
        "filter",
        &js_f,
        &cases_driver(
            "filtered",
            "[{ args: [[1n, 2n, 3n, 4n, 5n, 6n]], expect: [2n, 4n, 6n] }, { args: [[]], expect: [] }]",
        ),
    );
    assert!(out_f.contains("RESULT 2 0"), "filtered under node: {out_f}");

    // Fold over a two-parameter Lambda: .reduce(op, init).
    let fold_body = ret(IRNode::Fold {
        operation: Box::new(IRNode::Lambda {
            params: vec![
                ("a".to_string(), TypeRef::Int),
                ("x".to_string(), TypeRef::Int),
            ],
            body: Box::new(bop(IRBinOp::Add, v("a"), v("x"))),
        }),
        initial: Box::new(v("init")),
        over: Box::new(v("xs")),
    });
    let js_fo = js_of_hand(SPEC_FOLD, "folded", fold_body);
    assert!(js_fo.contains(".reduce("), "the Fold arm composes .reduce:\n{js_fo}");
    let out_fo = run_node(
        "fold",
        &js_fo,
        &cases_driver(
            "folded",
            "[{ args: [[1n, 2n, 3n, 4n], 0n], expect: 10n }, { args: [[], 42n], expect: 42n }, { args: [[5n], 10n], expect: 15n }]",
        ),
    );
    assert!(out_fo.contains("RESULT 3 0"), "folded under node: {out_fo}");

    // A Closure capturing a parameter: JS captures
    // lexically; the body never rebinds k, so snapshot
    // and lexical capture agree — the freeze's pin.
    let closed_body = ret(IRNode::Map {
        function: Box::new(IRNode::Closure {
            params: vec![("x".to_string(), TypeRef::Int)],
            captures: vec!["k".to_string()],
            body: Box::new(bop(IRBinOp::Add, v("x"), v("k"))),
        }),
        over: Box::new(v("xs")),
    });
    let js_c = js_of_hand(SPEC_CLOSED, "closed_over", closed_body);
    assert!(js_c.contains(".map("), "the Closure arm composes:\n{js_c}");
    let out_c = run_node(
        "closed",
        &js_c,
        &cases_driver(
            "closed_over",
            "[{ args: [[1n, 2n, 3n], 10n], expect: [11n, 12n, 13n] }, { args: [[], 5n], expect: [] }]",
        ),
    );
    assert!(out_c.contains("RESULT 2 0"), "closed_over under node: {out_c}");

    // Statement position: a bare Map statement renders
    // as an expression statement, not the refusal.
    let stmt_body = block(vec![
        IRNode::Map {
            function: Box::new(lam1(bop(IRBinOp::Add, v("x"), i(1)))),
            over: Box::new(v("xs")),
        },
        ret(v("xs")),
    ]);
    let js_s = js_of_hand(SPEC_MAP, "mapped", stmt_body);
    assert!(
        !js_s.contains("// unsupported"),
        "a statement-position Map renders:\n{js_s}"
    );
    assert!(js_s.contains(".map("), "the statement carries the composition:\n{js_s}");
    let out_s = run_node(
        "stmtmap",
        &js_s,
        &cases_driver("mapped", "[{ args: [[9n, 8n]], expect: [9n, 8n] }]"),
    );
    assert!(out_s.contains("RESULT 1 0"), "statement Map under node: {out_s}");

    // The Sort arm is NOT in scope (the recorded
    // decision): its standing stays the catch-all.
    let sort_body = ret(IRNode::Sort {
        comparator: Box::new(IRNode::Lambda {
            params: vec![
                ("a".to_string(), TypeRef::Int),
                ("b".to_string(), TypeRef::Int),
            ],
            body: Box::new(bop(IRBinOp::Lt, v("a"), v("b"))),
        }),
        collection: Box::new(v("xs")),
    });
    let js_so = js_of_hand(SPEC_MAP, "mapped", sort_body);
    assert!(
        js_so.contains("return undefined;"),
        "Sort keeps the standing catch-all (A2.4.2's recorded exclusion):\n{js_so}"
    );
}

// ── The packages (A2.4.1) + host rehearsals (A2.7) ──────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase135_packages_carry_the_six_at_35() {
    use morphic::codegen::{facade, package};
    // npm: file set, mass, manifest rows.
    let files = package::assemble_npm_package().expect("the npm package assembles");
    assert_eq!(files.len(), 4 + 3 + 2 * 35, "77 files in the assembled package");
    let total = package::unpacked_mass(&files);
    println!(
        "P135MASS|npm|total={total}|budget={}",
        package::NPM_UNPACKED_BUDGET
    );
    assert!(
        total <= package::NPM_UNPACKED_BUDGET,
        "the 35-export package is {total} B unpacked, inside the frozen 76,800 B budget (A2.5)"
    );
    assert_eq!(package::NPM_UNPACKED_BUDGET, 76_800, "the re-derived budget (A2.5)");
    let by_path: std::collections::BTreeMap<&str, &str> = files
        .iter()
        .map(|f| (f.path.as_str(), f.contents.as_str()))
        .collect();
    let manifest: serde_json::Value =
        serde_json::from_str(by_path["morphic.manifest.json"]).expect("manifest parses");
    let rows = manifest["exports"].as_array().expect("manifest rows");
    assert_eq!(rows.len(), 35, "35 manifest rows");
    let want_rows: [(&str, &str, usize); 6] = [
        ("list_map", "(List<Int>, fn(Int) -> Int) → List<Int>", 20),
        ("list_filter", "(List<Int>, fn(Int) -> Bool) → List<Int>", 20),
        ("list_fold", "(List<Int>, Int, fn(Int, Int) -> Int) → Int", 18),
        ("list_all", "(List<Int>, fn(Int) -> Bool) → Bool", 16),
        ("list_any", "(List<Int>, fn(Int) -> Bool) → Bool", 16),
        ("range", "(Int, Int) → List<Int>", 15),
    ];
    for (name, signature, pinned) in want_rows {
        let row = rows
            .iter()
            .find(|r| r["name"] == name)
            .unwrap_or_else(|| panic!("{name}: manifest row present"));
        assert_eq!(row["signature"], signature, "{name}: the A2.1 signature");
        assert_eq!(row["tier"], "tier-2", "{name}: tier-2 in the manifest");
        assert_eq!(
            row["verdictBasis"], "executed-exact",
            "{name}: verdict basis in the manifest"
        );
        assert_eq!(row["battery"]["pinned"], pinned, "{name}: battery pinned");
        assert_eq!(row["battery"]["specSilent"], 0, "{name}: battery spec-silent");
        assert_eq!(
            row["doc"],
            frozen_doc(name),
            "{name}: the doc travels verbatim into the manifest"
        );
        // The subpath shim pair exists.
        assert_eq!(
            by_path[format!("{name}.js").as_str()],
            format!("export {{ {name} }} from \"./index.js\";\n"),
            "{name}: subpath shim"
        );
        assert!(by_path.contains_key(format!("{name}.d.ts").as_str()));
    }
    // The root module defines the six; the declarations
    // carry §3's `never` for the function-typed
    // parameters (A2.4.2's boundary, stated in the
    // package's own surface).
    for name in SIX {
        assert!(
            by_path["index.js"].contains(&format!("function {name}(")),
            "{name}: defined in the root module"
        );
    }
    assert!(
        by_path["index.d.ts"]
            .contains("export declare function list_map(xs: bigint[], f: never): bigint[];"),
        "list_map's declaration carries the never boundary"
    );
    assert!(
        by_path["index.d.ts"]
            .contains("export declare function range(lo: bigint, hi: bigint): bigint[];"),
        "range's declaration is fully typed (no function parameter)"
    );

    // Facade: 35 functions, the six fully typed.
    let facade_files = facade::assemble_rust_facade().expect("the facade assembles");
    let module = &facade_files
        .iter()
        .find(|f| f.path == "src/stdlib.rs")
        .expect("facade carries src/stdlib.rs")
        .contents;
    assert_eq!(module.matches("pub fn ").count(), 35, "35 facade functions");
    for sig in [
        "pub fn list_map(xs: Vec<i64>, f: fn(i64) -> i64) -> Vec<i64>",
        "pub fn list_filter(xs: Vec<i64>, p: fn(i64) -> bool) -> Vec<i64>",
        "pub fn list_fold(xs: Vec<i64>, init: i64, f: fn(i64, i64) -> i64) -> i64",
        "pub fn list_all(xs: Vec<i64>, p: fn(i64) -> bool) -> bool",
        "pub fn list_any(xs: Vec<i64>, p: fn(i64) -> bool) -> bool",
        "pub fn range(lo: i64, hi: i64) -> Vec<i64>",
    ] {
        assert!(module.contains(sig), "the facade is fully typed: {sig}");
    }
    println!("P135PIN|packages|npm 77 files, facade 35 functions, six rows verbatim");
}

/// The npm clean-host rehearsal (A2.7 clause 4's
/// instrument, run as build verification): the
/// assembled package, written to a scratch dir, answers
/// all 105 battery points under node — function
/// arguments as JavaScript lambdas AND as the package's
/// own exports (the named cases).
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase135_npm_clean_host_rehearsal() {
    use morphic::codegen::package;
    let files = package::assemble_npm_package().expect("the npm package assembles");
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase135-npmhost-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for f in &files {
        std::fs::write(dir.join(&f.path), &f.contents).unwrap();
    }
    let mut d = String::new();
    d.push_str("import * as lib from \"./index.js\";\n");
    d.push_str("const { list_map, list_filter, list_fold, list_all, list_any, range, int_abs, int_sign, int_is_even, int_is_odd, int_min, int_max } = lib;\n");
    d.push_str(EQ_FN_JS);
    d.push_str("let total = 0, fails = 0;\nfunction check(name, got, want) { total++; if (!eq(got, want)) { console.log(name + \": got \" + show(got) + \" want \" + show(want)); fails++; } }\n");
    for (idx, (xs, f)) in map_points().iter().enumerate() {
        let want: Vec<i64> = xs.iter().map(|x| (f.m)(*x)).collect();
        d.push_str(&format!(
            "check(\"list_map#{idx}\", list_map({}, {}), {});\n",
            js_list(xs),
            f.js,
            js_list(&want)
        ));
    }
    for (idx, (xs, p)) in filter_points().iter().enumerate() {
        let want: Vec<i64> = xs.iter().copied().filter(|x| (p.m)(*x)).collect();
        d.push_str(&format!(
            "check(\"list_filter#{idx}\", list_filter({}, {}), {});\n",
            js_list(xs),
            p.js,
            js_list(&want)
        ));
    }
    for (idx, (xs, init, f)) in fold_points().iter().enumerate() {
        let want = xs.iter().fold(*init, |a, x| (f.m)(a, *x));
        d.push_str(&format!(
            "check(\"list_fold#{idx}\", list_fold({}, {}, {}), {});\n",
            js_list(xs),
            js_int(*init),
            f.js,
            js_int(want)
        ));
    }
    for (idx, (xs, p)) in all_points().iter().enumerate() {
        let want = xs.iter().all(|x| (p.m)(*x));
        d.push_str(&format!(
            "check(\"list_all#{idx}\", list_all({}, {}), {});\n",
            js_list(xs),
            p.js,
            want
        ));
    }
    for (idx, (xs, p)) in any_points().iter().enumerate() {
        let want = xs.iter().any(|x| (p.m)(*x));
        d.push_str(&format!(
            "check(\"list_any#{idx}\", list_any({}, {}), {});\n",
            js_list(xs),
            p.js,
            want
        ));
    }
    for (idx, (lo, hi)) in range_points().iter().enumerate() {
        d.push_str(&format!(
            "check(\"range#{idx}\", range({}, {}), {});\n",
            js_int(*lo),
            js_int(*hi),
            js_list(&range_mirror(*lo, *hi))
        ));
    }
    d.push_str("console.log(`RESULT ${total} ${fails}`);\nif (fails || total !== 105) process.exit(1);\n");
    std::fs::write(dir.join("driver.mjs"), &d).unwrap();
    let out = std::process::Command::new("node")
        .arg(dir.join("driver.mjs"))
        .output()
        .unwrap_or_else(|e| panic!("npm host: node must run: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success() && stdout.contains("RESULT 105 0"),
        "npm clean-host: the six answer 105/105 under node\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    println!("P135HOST|npm|105/105");
}

/// The facade host rehearsal (A2.7 clause 4's
/// instrument, run as build verification): the
/// generated facade module compiles standalone under
/// rustc --test and answers all 105 battery points
/// with fn-pointer / closure arguments.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase135_facade_host_rehearsal() {
    use morphic::codegen::facade;
    let files = facade::assemble_rust_facade().expect("the facade assembles");
    let mut source = files
        .iter()
        .find(|f| f.path == "src/stdlib.rs")
        .expect("facade carries src/stdlib.rs")
        .contents
        .clone();
    let mut h = String::from("\n#[cfg(test)]\nmod p135_battery {\n    use super::*;\n");
    let mut emit = |name: &str, lines: &[String]| {
        h.push_str(&format!("    #[test]\n    fn p135_{name}() {{\n"));
        for line in lines {
            h.push_str(&format!("        assert_eq!({line});\n"));
        }
        h.push_str("    }\n");
    };
    let map_lines: Vec<String> = map_points()
        .iter()
        .map(|(xs, f)| {
            let want: Vec<i64> = xs.iter().map(|x| (f.m)(*x)).collect();
            format!("list_map({}, {}), {}", rs_list(xs), f.rust, rs_list(&want))
        })
        .collect();
    emit("list_map", &map_lines);
    let filter_lines: Vec<String> = filter_points()
        .iter()
        .map(|(xs, p)| {
            let want: Vec<i64> = xs.iter().copied().filter(|x| (p.m)(*x)).collect();
            format!(
                "list_filter({}, {}), {}",
                rs_list(xs),
                p.rust,
                rs_list(&want)
            )
        })
        .collect();
    emit("list_filter", &filter_lines);
    let fold_lines: Vec<String> = fold_points()
        .iter()
        .map(|(xs, init, f)| {
            let want = xs.iter().fold(*init, |a, x| (f.m)(a, *x));
            format!(
                "list_fold({}, {}, {}), {}",
                rs_list(xs),
                rs_int(*init),
                f.rust,
                rs_int(want)
            )
        })
        .collect();
    emit("list_fold", &fold_lines);
    let all_lines: Vec<String> = all_points()
        .iter()
        .map(|(xs, p)| {
            format!(
                "list_all({}, {}), {}",
                rs_list(xs),
                p.rust,
                xs.iter().all(|x| (p.m)(*x))
            )
        })
        .collect();
    emit("list_all", &all_lines);
    let any_lines: Vec<String> = any_points()
        .iter()
        .map(|(xs, p)| {
            format!(
                "list_any({}, {}), {}",
                rs_list(xs),
                p.rust,
                xs.iter().any(|x| (p.m)(*x))
            )
        })
        .collect();
    emit("list_any", &any_lines);
    let range_lines: Vec<String> = range_points()
        .iter()
        .map(|(lo, hi)| {
            format!(
                "range({}, {}), {}",
                rs_int(*lo),
                rs_int(*hi),
                rs_list(&range_mirror(*lo, *hi))
            )
        })
        .collect();
    emit("range", &range_lines);
    h.push_str("}\n");
    source.push_str(&h);
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase135-facadehost-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let src_path = dir.join("facade_stdlib.rs");
    std::fs::write(&src_path, &source).unwrap();
    let bin_path = dir.join("facade_stdlib_test");
    let compile = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2021",
            "--test",
            src_path.to_str().unwrap(),
            "-o",
            bin_path.to_str().unwrap(),
        ])
        .output()
        .expect("rustc runs");
    assert!(
        compile.status.success(),
        "the generated facade module must compile standalone:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = std::process::Command::new(&bin_path)
        .output()
        .expect("the compiled test binary runs");
    let stdout = String::from_utf8_lossy(&run.stdout).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        run.status.success() && stdout.contains("test result: ok. 6 passed"),
        "facade host: the six answer their batteries (105 points in 6 tests), got: {stdout}"
    );
    println!("P135HOST|facade|105/105");
}
