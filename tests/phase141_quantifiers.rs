//! Phase 141 (W3) — the bounded-quantifier and exists
//! arms over element terms (LIST_ENCODER_SCOPE §2.7),
//! the constructed-list bridge's loop half (§2.6), and
//! the crowning of the eight element-level entries (§3).
//!
//! The quantifier arms' envelope, stated once: the
//! Phase 94 discipline stands (Int binders, no duplicate
//! names, quantifier-free bodies, pattern-free
//! `forall_const` / `exists_const`). W3's addition is
//! the guard supply W2 left to it: for a SINGLE binder
//! whose body reads paired-binding elements at indices
//! mentioning the binder, the arm derives the binder's
//! domain from the reads themselves — the conjunction
//! of each read's own in-range condition
//! `0 <= idx < len(list)` — and builds the formula with
//! that domain as the binder's guard (an antecedent for
//! `forall`, a conjunct for `exists`), encoding the body
//! with the domain conjuncts as its guard set. On every
//! corpus row the derived domain coincides with the
//! textual `range(...)` domain under the check's own
//! facts (the domain is the read list's length, or the
//! output's — whose length the body's facts equate);
//! where a body performs no binder-indexed paired read,
//! the Phase 94 bare form stands untouched (self-guarding
//! bodies, pure-Int rows, the tokenizer rows, gcd's
//! unbounded reading). A multi-binder body with element
//! reads declines (D6a: no multi-binder widening —
//! `list_sort`'s residue is exactly this shape).
//!
//! The loop half of the bridge: `IRNode::For` evaluates
//! in the statement encoder as its While desugaring
//! (counter bound to the start, condition `var < end`,
//! body followed by the increment), dispatched through
//! the standing lazy-hybrid loop route. The invariant
//! for a shipped library body's loop is STATED data —
//! `stdlib::shipped_loop_invariant`, authored beside the
//! bodies in `src/stdlib.rs` — and reaches the route
//! only through `verify_one`, by spec name, exactly as
//! the engine reaches `shipped_body`. The Phase 96
//! obligations (initiation / consecution / exit)
//! discharge the stated invariant before any fact from
//! it is used; a candidate invariant the body does not
//! preserve fails the route (the mutated-reverse pin
//! below). The bridge invents no element fact: every
//! element fact in an exit state is a conjunct of the
//! discharged invariant's text.
//!
//! Two guard extensions ride the statement layer (the
//! mirror has no statement layer; both are hypotheses
//! of the path being encoded, the statement-side dual
//! of W2's antecedent accumulation):
//! - a statement `If` evaluates each branch with its
//!   condition (resp. its negation) added to the guard
//!   set, plus the length well-formedness facts of the
//!   branch state's environment;
//! - the invariant route evaluates the loop body with
//!   the loop condition and the invariant's
//!   quantifier-free rows added to the guard set, plus
//!   the havoc'd state's length well-formedness facts.
//!
//! Length well-formedness (`len >= 0` for every list
//! length in scope) is asserted as a judge-session side
//! condition, on the D5a Char precedent
//! (`collect_char_conditions`): a length is a count, and
//! a countermodel at a negative length is not a
//! countermodel any execution can produce — before this
//! phase, the row `len(xs) >= 0` was REFUTED for the
//! `len` body (pinned below, retired in place). The
//! discharge itself (`guard_discharged`) is untouched:
//! well-formedness is not a guard hypothesis, and the
//! mirror's difference-logic discharge needs no seed
//! (the `len(xs) != 0` require specimen below pins both
//! sides still declining the constant read).
//!
//! Every test prints a `P141|...` line with the measured
//! standing; assertions pin exactly what the line
//! reports. In the no-Z3 build the verdicts are the
//! honest subset (Inconclusive throughout); the
//! encodability pins are configuration-independent
//! (real == pure per shape).

use morphic::spec::ast::FunctionSpec;
use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, Provenance, ScoreBreakdown, SpecIndex,
};
use morphic::synthesis::eval;
use morphic::verify::encodable::body_is_encodable_pure;
use morphic::verify::verdict::VerificationStatus;
use morphic::verify::verifier::{self, Verifier};

fn checked(source: &str) -> Vec<FunctionSpec> {
    let module = parser::parse(source).expect("probe source parses");
    typeck::check(module)
        .expect("probe source typechecks")
        .spec
        .functions
}

fn one(source: &str) -> FunctionSpec {
    checked(source).into_iter().next().expect("one spec")
}

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 141,
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

fn verdict(spec: &FunctionSpec, body: IRNode) -> VerificationStatus {
    Verifier::new().verify_one(&candidate(spec, body), spec).status
}

fn enc_pair(spec: &FunctionSpec, body: &IRNode) -> (bool, bool) {
    (
        verifier::body_is_encodable(spec, body),
        body_is_encodable_pure(spec, body, &SpecIndex::empty()),
    )
}

fn v(n: &str) -> IRNode {
    IRNode::Var(n.into())
}
fn i(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}
fn bin(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
    IRNode::BinOp(op, Box::new(l), Box::new(r))
}
fn len_u(of: IRNode) -> IRNode {
    IRNode::UnaryOp(morphic::synthesis::engine::IRUnaryOp::Len, Box::new(of))
}
fn idx(coll: IRNode, index: IRNode) -> IRNode {
    IRNode::Index(Box::new(coll), Box::new(index))
}

// ── The forall arm ─────────────────────────────────────

#[test]
fn phase141_forall_arm_encodes_element_rows() {
    // The identity body against the element forall:
    // declines at W2 (the arm cleared the guard set and
    // the reads had no source); W3's derived domain is
    // the read's own range, and the row Verifies.
    let id = one(
        r#"Probe { spec id_forall {
            input: xs: List<Int>
            output: List<Int>
            constraint: forall i: Int in range(len(xs)): output[i] == xs[i]
            test { input: [1, 2]; expect: [1, 2] }
        } }"#,
    );
    let status = verdict(&id, v("xs"));
    println!("P141|forall|identity={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // Teeth: the same body against the REVERSED claim
    // Refutes (a two-element list is a countermodel) —
    // the arm decides the row, it does not rubber-stamp
    // every forall it can encode.
    let rev = one(
        r#"Probe { spec rev_claim {
            input: xs: List<Int>
            output: List<Int>
            constraint: forall i: Int in range(len(xs)): output[i] == xs[(len(xs) - 1) - i]
            test { input: [1, 2]; expect: [2, 1] }
        } }"#,
    );
    let status = verdict(&rev, v("xs"));
    println!("P141|forall|identity_vs_reversed={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The antecedent-inside-the-binder shape
    // (list_drop's guarded rows, distilled): the row's
    // own case antecedent rides inside the forall, the
    // domain comes from the reads.
    let drop_head = one(
        r#"Probe { spec drop_head {
            input: xs: List<Int>, n: Int
            output: List<Int>
            constraint: n <= 0 implies output == xs
            constraint: forall i: Int in range(len(output)): n <= 0 implies output[i] == xs[i]
            test { input: ([1, 2], 0); expect: [1, 2] }
        } }"#,
    );
    let status = verdict(&drop_head, v("xs"));
    println!("P141|forall|antecedent_inside={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The named boundary of the derived domain: a
    // binder whose textual range names a DIFFERENT list
    // than the body reads. The arm reads the quantifier
    // over the indices its reads denote (the only range
    // the guard discipline can state, §2.3: a row's
    // truth may not be decided by a beyond-length
    // select); the corpus's standing forms name the
    // read lists' own lengths, where the two coincide.
    let cross = one(
        r#"Probe { spec cross_range {
            input: xs: List<Int>, ys: List<Int>
            output: Bool
            constraint: (forall i: Int in range(len(ys)): xs[i] == 0) implies output == true
            test { input: ([0], [1, 2]); expect: true }
        } }"#,
    );
    let status = verdict(&cross, IRNode::Literal(IRLiteral::Bool(true)));
    println!("P141|forall|cross_range_boundary={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase141_forall_multi_binder_element_rows_decline() {
    // D6a: no multi-binder widening. A two-binder
    // element row (list_sort's row shape, distilled)
    // stays unencodable: the identity body would Refute
    // if the row encoded, so Inconclusive is the
    // decline's signature, not a pass.
    let spec = one(
        r#"Probe { spec two_bind {
            input: xs: List<Int>
            output: List<Int>
            constraint: forall i: Int, j: Int in range(len(output)): i > j || output[j] <= output[i]
            test { input: [2, 1]; expect: [2, 1] }
        } }"#,
    );
    let status = verdict(&spec, v("xs"));
    println!("P141|forall|two_binder={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── The exists arm ─────────────────────────────────────

#[test]
fn phase141_exists_arm_encodes_element_rows() {
    // The P137 probe_exists shape with its constant
    // body: at W2 the exists had no arm at all; the
    // row's consequent is the body's value, so it
    // Verifies — and with the constant-false body the
    // same row Refutes (a one-element list attains 7):
    // the arm decides in both directions.
    let ex = one(
        r#"Probe { spec probe_exists {
            input: xs: List<Int>
            output: Bool
            constraint: (exists i: Int in range(len(xs)): xs[i] == 7) implies output == true
            test { input: [7]; expect: true }
        } }"#,
    );
    let status = verdict(&ex, IRNode::Literal(IRLiteral::Bool(true)));
    println!("P141|exists|constant_true={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
    let status = verdict(&ex, IRNode::Literal(IRLiteral::Bool(false)));
    println!("P141|exists|constant_false={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The bare path: an exists whose body performs no
    // paired read encodes in the Phase 94 form (the
    // binder ranges over Int as parsed). `exists i:
    // i == i + 1` is false, so the implication's body
    // Verifies on the false body — a decline would read
    // Inconclusive instead.
    let bare = one(
        r#"Probe { spec bare_exists {
            input: n: Int
            output: Bool
            constraint: (exists i: Int in range(10): i == i + 1) implies output == true
            test { input: 0; expect: false }
        } }"#,
    );
    let status = verdict(&bare, IRNode::Literal(IRLiteral::Bool(false)));
    println!("P141|exists|bare_pure_int={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Length well-formedness ─────────────────────────────

#[test]
fn phase141_length_wellformedness_is_a_side_condition() {
    // Retired in place from the Phase 141 exploration
    // probe: before this phase the pair below was
    // VerificationFailed — the solver's countermodel
    // was `len(xs) = -1`, a length no execution can
    // produce. Lengths now carry their well-formedness
    // (`>= 0`) as a judge-session side condition, on
    // the D5a Char precedent.
    let spec = one(
        r#"Probe { spec len_nonneg {
            input: xs: List<Int>
            output: Int
            constraint: output == len(xs)
            constraint: len(xs) >= 0
            test { input: [1, 2]; expect: 2 }
        } }"#,
    );
    let body = IRNode::Call {
        function: "len".into(),
        args: vec![v("xs")],
    };
    let status = verdict(&spec, body);
    println!("P141|len|nonneg={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Mirror agreement on the quantifier shapes ──────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase141_mirror_agreement_on_quantifier_shapes() {
    // (a) A require-guarded body read under a spec whose
    // postcondition is an element forall: both sides
    // encode the BODY (the predicates judge bodies; the
    // row's own encoding is the verifier's, and the
    // mirror's silence about rows is its standing
    // boundary).
    let guarded = one(
        r#"Probe { spec guarded_read {
            input: xs: List<Int>, i: Int
            output: Int
            require: 0 <= i && i < len(xs)
            constraint: forall j: Int in range(len(xs)): xs[j] == xs[j]
            constraint: output == xs[i]
            test { input: ([10, 20], 1); expect: 20 }
        } }"#,
    );
    let body = idx(v("xs"), v("i"));
    let pair = enc_pair(&guarded, &body);
    println!("P141|mirror|require_guarded={pair:?}");
    assert_eq!(pair, (true, true));

    // (b) A forall REQUIRE contributes no length facts
    // on either side: the real side's guard set gains
    // the forall TERM (a hypothesis about elements,
    // which entails no in-range fact for a free index),
    // the mirror harvests no fact from the row — an
    // unguarded body read declines on both.
    let freq = one(
        r#"Probe { spec forall_require {
            input: xs: List<Int>, i: Int
            output: Int
            require: forall j: Int in range(len(xs)): xs[j] >= 0
            constraint: output == xs[i]
            test { input: ([10, 20], 1); expect: 20 }
        } }"#,
    );
    let pair = enc_pair(&freq, &body);
    println!("P141|mirror|forall_require={pair:?}");
    assert_eq!(pair, (false, false));

    // (c) An exists postcondition changes nothing for a
    // body's own unguarded read: both decline.
    let epost = one(
        r#"Probe { spec exists_post {
            input: xs: List<Int>
            output: Int
            constraint: (exists i: Int in range(len(xs)): xs[i] == 7) implies xs[0] == 7
            constraint: output == xs[0]
            test { input: [7]; expect: 7 }
        } }"#,
    );
    let pair = enc_pair(&epost, &idx(v("xs"), i(0)));
    println!("P141|mirror|exists_post_unguarded={pair:?}");
    assert_eq!(pair, (false, false));

    // (d) The discharge is NOT seeded: a `len(xs) != 0`
    // require still discharges nothing on either side
    // (the mirror states no ≠ fact; the real side's
    // discharge is W2's entailment over the guard terms
    // alone — length well-formedness lives in judge
    // sessions and statement-layer guard extensions,
    // never in `guard_discharged`).
    let neq = one(
        r#"Probe { spec neq_require {
            input: xs: List<Int>
            output: Int
            require: len(xs) != 0
            constraint: output == xs[0]
            test { input: [7]; expect: 7 }
        } }"#,
    );
    let pair = enc_pair(&neq, &idx(v("xs"), i(0)));
    println!("P141|mirror|neq_require={pair:?}");
    assert_eq!(pair, (false, false));

    // (e) The eight shipped bodies under their own
    // specs: statement bodies decline the expression
    // predicates on BOTH sides (their verdicts move
    // through the statement route — the crown pin
    // below — not through `body_is_encodable`).
    let specs = stdlib::prelude_specs();
    for name in [
        "list_reverse",
        "list_contains",
        "list_min",
        "list_max",
        "list_take",
        "list_drop",
        "list_is_sorted",
        "range",
    ] {
        let spec = specs.iter().find(|s| s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let pair = enc_pair(spec, &body);
        println!("P141|mirror|shipped|{name}={pair:?}");
        assert_eq!(pair, (false, false), "{name}");
    }
}

// ── The For arm of the statement encoder ───────────────

fn count_body() -> IRNode {
    // c = 0; for i in 0..len(xs) { c = c + 1 }; return c
    IRNode::Block(vec![
        IRNode::Alloc {
            name: "c".into(),
            ty: morphic::spec::ast::TypeRef::Int,
            initial: Box::new(i(0)),
        },
        IRNode::For {
            var: "i".into(),
            start: Box::new(i(0)),
            end: Box::new(len_u(v("xs"))),
            body: Box::new(IRNode::Assign {
                target: Box::new(v("c")),
                value: Box::new(bin(IRBinOp::Add, v("c"), i(1))),
            }),
        },
        IRNode::Return(Box::new(v("c"))),
    ])
}

#[test]
fn phase141_for_arm_unrolls_concrete_and_declines_symbolic() {
    // The For arm exists: with the trip count pinned by
    // a require, the desugared loop unrolls exactly and
    // the body Verifies — at W2 a For node fell to the
    // expression encoder and declined whole. (The trip
    // count is 1 because verify_one's production
    // unrolling fuel is 1 by design, Phase 93: exact
    // multi-iteration unrolling is the battery route's
    // caller-supplied fuel, and symbolic trip counts
    // belong to the invariant route — the crown below.)
    let conc = one(
        r#"Probe { spec count1 {
            input: xs: List<Int>
            output: Int
            require: len(xs) == 1
            constraint: output == 1
            test { input: [1]; expect: 1 }
        } }"#,
    );
    let status = verdict(&conc, count_body());
    println!("P141|for|concrete_trip={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The same body at a symbolic trip count, with no
    // stated invariant for this (probe) spec: the
    // unrolling exhausts and the route declines —
    // Inconclusive, never a guessed exit.
    let sym = one(
        r#"Probe { spec count_any {
            input: xs: List<Int>
            output: Int
            constraint: output == len(xs)
            test { input: [1, 2, 3]; expect: 3 }
        } }"#,
    );
    let status = verdict(&sym, count_body());
    println!("P141|for|symbolic_trip={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── The eight, crowned ─────────────────────────────────

const EIGHT: &[&str] = &[
    "list_reverse",
    "list_contains",
    "list_min",
    "list_max",
    "list_take",
    "list_drop",
    "list_is_sorted",
    "range",
];

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase141_the_eight_crowned() {
    // The phase's exit standing (LIST_ENCODER_SCOPE
    // §3): each of the eight Verifies through the
    // production pipeline with its shipped body — every
    // postcondition row Proven — and stays battery-exact
    // under the interpreter (the executed half of the
    // Tier-1 shape, standing since 1.0/1.2).
    let specs = stdlib::prelude_specs();
    let verifier = Verifier::with_specs(&specs);
    for name in EIGHT {
        let spec = specs.iter().find(|s| &s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let run = eval::run_spec_tests(&body, spec);
        println!(
            "P141|crown|{name}|battery={}/{}",
            run.passed, run.evaluable
        );
        assert_eq!(run.passed, run.evaluable, "{name} battery");
        assert!(run.evaluable > 0, "{name} battery is non-empty");
        let result = verifier.verify_one(&candidate(spec, body), spec);
        for (row, r) in result.constraint_results.iter().enumerate() {
            println!("P141|crown|{name}|post[{row}]|holds={}", r.holds);
        }
        println!("P141|crown|{name}|verify={:?}", result.status);
        #[cfg(feature = "z3-support")]
        {
            assert_eq!(result.status, VerificationStatus::Verified, "{name}");
            assert!(
                result.constraint_results.iter().all(|r| r.holds),
                "{name}: every row Proven"
            );
        }
        #[cfg(not(feature = "z3-support"))]
        assert_eq!(result.status, VerificationStatus::Inconclusive, "{name}");
    }
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase141_the_residue_stands_after_w5() {
    // The drift discipline's other half, as the later
    // arms leave it. At Phase 141 nine stood here: the
    // fold-form three (W4), the HO five (W5),
    // binary_search (its stretch is judged at Phase
    // 144) and sort (the standing residue). At Phase
    // 142 (W4) the fold-form arm crowned list_sum,
    // list_product and list_fold (the crown evidence is
    // tests/phase142_fold_form.rs), so their pins retire
    // from this list in place, per §9 R10. At Phase 143
    // (W5) the UF-composition arm crowned list_map,
    // list_filter, list_all and list_any
    // (tests/phase143_uf_composition.rs), so their pins
    // retire the same way. The two that remain do not
    // move at W5: binary_search is the named stretch,
    // sort the standing residue. Phase 144 judged the
    // stretch: it does not land — binary_search's
    // require is not assumable in the crown route
    // (its callee's contract has no expression-level
    // denotation; measured in isolation in
    // tests/phase144_gate_merge.rs) — so both pins
    // stand exactly as asserted here, their reasons
    // now measured and named on the entries' surfaces.
    let specs = stdlib::prelude_specs();
    let verifier = Verifier::with_specs(&specs);
    for name in ["list_sort", "list_binary_search"] {
        let spec = specs.iter().find(|s| &s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let status = verifier.verify_one(&candidate(spec, body), spec).status;
        println!("P141|stand|{name}={status:?}");
        assert_eq!(status, VerificationStatus::Inconclusive, "{name}");
    }
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase141_crown_teeth_wrong_bodies_do_not_verify() {
    let specs = stdlib::prelude_specs();
    let verifier = Verifier::with_specs(&specs);

    // The identity body against list_reverse's
    // contract: the element row Refutes on a
    // two-element countermodel. The crown is the
    // shipped body's, not the spec's.
    let spec = specs
        .iter()
        .find(|s| s.name == "list_reverse")
        .expect("entry ships");
    let status = verifier.verify_one(&candidate(spec, v("xs")), spec).status;
    println!("P141|teeth|reverse_identity={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // A mutated shipped body — appends `xs[i]` where
    // the shipped body appends `xs[(len(xs) - 1) - i]` —
    // carries the same For shape, so the stated
    // invariant is ATTEMPTED for it: consecution fails
    // (the mutated body does not preserve it), the
    // route falls to exhaustion, and the verdict is
    // Inconclusive — never a Verified bought by an
    // invariant the body does not keep.
    let mutated = IRNode::Block(vec![
        IRNode::Alloc {
            name: "out".into(),
            ty: morphic::spec::ast::TypeRef::List(Box::new(
                morphic::spec::ast::TypeRef::Int,
            )),
            initial: Box::new(IRNode::Collection {
                kind: morphic::synthesis::engine::CollectionKind::List,
                elements: vec![],
            }),
        },
        IRNode::For {
            var: "i".into(),
            start: Box::new(i(0)),
            end: Box::new(len_u(v("xs"))),
            body: Box::new(bin(IRBinOp::Append, v("out"), idx(v("xs"), v("i")))),
        },
        IRNode::Return(Box::new(v("out"))),
    ]);
    let status = verifier.verify_one(&candidate(spec, mutated), spec).status;
    println!("P141|teeth|reverse_mutated={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // range with a constant-empty body: the length row
    // Refutes (hi > lo is a countermodel).
    let spec = specs.iter().find(|s| s.name == "range").expect("entry ships");
    let empty = IRNode::Collection {
        kind: morphic::synthesis::engine::CollectionKind::List,
        elements: vec![],
    };
    let status = verifier.verify_one(&candidate(spec, empty), spec).status;
    println!("P141|teeth|range_empty={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}
