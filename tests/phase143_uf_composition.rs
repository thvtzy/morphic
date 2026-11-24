//! Phase 143 (W5) — UF composition over guarded element
//! terms (LIST_ENCODER_SCOPE §2.9), the last build phase
//! of the 1.3 List-encoder campaign: the four remaining
//! higher-order entries crowned, and gate clause 3's
//! pair measured.
//!
//! What §2.9 composes, point by point, and where each
//! point lands:
//!
//! - **Application at element terms.** A row's
//!   `f(xs[i])` encodes as the Phase 133 UF applied to
//!   the guarded `select` term — W2's guard discipline
//!   and W3's read-derived domain are the guard sources,
//!   exactly as for a bare read: an application at an
//!   unguarded index declines with its row (the
//!   `uf_oob` specimen below). The UF's sorts here are
//!   Int → Int and Int → Bool, inside the Phase 133
//!   slice.
//! - **Body-side characterizations**, one per
//!   higher-order entry, each exactly as strong as the
//!   frozen contract it serves, carried as stated data
//!   through the Phase 141 vehicle (the
//!   `shipped_loop_invariant` table — the obligations
//!   discharge the text against the body before any
//!   fact from it is used): **Map** — `len(out) == i`
//!   and, pointwise over the consumed prefix,
//!   `out[j] == f(xs[j])` (length preserved, the
//!   element at `j` is the UF's application at the
//!   input's element there); **Filter** — exactly its
//!   two frozen rows at the prefix (`len(out) <= i`,
//!   and `p(out[j]) == true` over the built prefix): no
//!   order, no completeness, no subsequence claim is
//!   stated, because the contract states none (§9 R6);
//!   **All** — the prefix conjunction in both
//!   directions (a failing element inside the prefix
//!   is recorded; a recorded failure is witnessed
//!   inside it); **Any** — the prefix disjunction,
//!   dually. **Fold** — §2.8's step row with `op = f`,
//!   carried at Phase 142; it stands here unregressed.
//! - **Congruence over elements, nothing more.** The
//!   same application on both sides of a row Verifies
//!   (the `uf_at_first` control); a row whose truth
//!   needs a fact about `f`'s values beyond congruence
//!   — a value at a point, injectivity — stands not
//!   Verified (the specimens below are Refuted: a UF
//!   ranges over every function, so a countermodel
//!   function exists and the refutation is genuine).
//!
//! The `IRNode::Map` / `IRNode::Filter` NODE bodies
//! stay unencodable on both sides of the mirror (the
//! Phase 137 pin stands): §2.9's characterizations are
//! of the nodes' meanings, asserted where the contracts
//! need them — the row side and the stated-invariant
//! route — not a node encoding in either encoder,
//! exactly as the Fold node's shape stood at Phase 142.
//!
//! Every test prints a `P143|...` line with the
//! measured standing; assertions pin exactly what the
//! line reports. In the no-Z3 build the verdicts are
//! the honest subset (Inconclusive throughout); the
//! encodability pins are configuration-independent
//! (real == pure per shape).

use morphic::spec::ast::{FunctionSpec, TypeRef};
use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::engine::{
    CandidateImplementation, CollectionKind, IRBinOp, IRLiteral, IRNode, IRUnaryOp, Provenance,
    ScoreBreakdown, SpecIndex,
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
        id: 143,
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
fn b(x: bool) -> IRNode {
    IRNode::Literal(IRLiteral::Bool(x))
}
fn bin(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
    IRNode::BinOp(op, Box::new(l), Box::new(r))
}
fn call(name: &str, args: Vec<IRNode>) -> IRNode {
    IRNode::Call {
        function: name.into(),
        args,
    }
}
fn len_u(of: IRNode) -> IRNode {
    IRNode::UnaryOp(IRUnaryOp::Len, Box::new(of))
}
fn idx(coll: IRNode, index: IRNode) -> IRNode {
    IRNode::Index(Box::new(coll), Box::new(index))
}
fn block(stmts: Vec<IRNode>) -> IRNode {
    IRNode::Block(stmts)
}
fn ret(value: IRNode) -> IRNode {
    IRNode::Return(Box::new(value))
}
fn empty_list() -> IRNode {
    IRNode::Collection {
        kind: CollectionKind::List,
        elements: vec![],
    }
}
fn alloc_list(name: &str) -> IRNode {
    IRNode::Alloc {
        name: name.into(),
        ty: TypeRef::List(Box::new(TypeRef::Int)),
        initial: Box::new(empty_list()),
    }
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
fn for_(var: &str, end: IRNode, body: IRNode) -> IRNode {
    IRNode::For {
        var: var.into(),
        start: Box::new(i(0)),
        end: Box::new(end),
        body: Box::new(body),
    }
}
fn append_stmt(name: &str, item: IRNode) -> IRNode {
    bin(IRBinOp::Append, v(name), item)
}
fn if_(cond: IRNode, then: IRNode, els: IRNode) -> IRNode {
    IRNode::If {
        condition: Box::new(cond),
        then_branch: Box::new(then),
        else_branch: Box::new(els),
    }
}

/// The For-loop skeleton the four shipped bodies share,
/// parameterized so the teeth can mutate one joint at a
/// time (the step each body performs per element).
fn loop_body(step: IRNode) -> IRNode {
    block(vec![
        alloc_list("out"),
        for_("i", len_u(v("xs")), step),
        ret(v("out")),
    ])
}

// ── The four, crowned ──────────────────────────────────

const FOUR: &[&str] = &["list_map", "list_filter", "list_all", "list_any"];

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase143_the_four_crowned() {
    // The phase's exit standing (LIST_ENCODER_SCOPE
    // §3.3): each of the four Verifies through the
    // production pipeline with its shipped body — every
    // postcondition row Proven under §2.9's composition
    // — and stays battery-exact under the interpreter
    // through the index-carrying route (the entries'
    // named-supply tests bind there, Phase 132/135).
    // For list_filter the Verified certifies exactly
    // its two frozen rows (§9 R6) — the teeth below
    // measure that boundary in both directions.
    let specs = stdlib::prelude_specs();
    let index = SpecIndex::from_specs(&specs);
    let verifier = Verifier::with_specs(&specs);
    for name in FOUR {
        let spec = specs.iter().find(|s| &s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let run = eval::run_spec_tests_in(&body, spec, &index);
        println!(
            "P143|crown|{name}|battery={}/{}",
            run.passed, run.evaluable
        );
        assert_eq!(run.passed, run.evaluable, "{name} battery");
        assert_eq!(run.evaluable, spec.tests.len(), "{name} battery is whole");
        let result = verifier.verify_one(&candidate(spec, body), spec);
        for (row, r) in result.constraint_results.iter().enumerate() {
            println!("P143|crown|{name}|post[{row}]|holds={}", r.holds);
        }
        println!("P143|crown|{name}|verify={:?}", result.status);
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

// ── Gate clause 3's pair, measured ─────────────────────

#[test]
fn phase143_clause3_composed_specimen_verifies() {
    // The composed specimen: list_map's own rows over a
    // fresh spec, judged against a Map-shaped body — a
    // While loop appending `f(xs[i])` — whose invariant
    // is carried by the body itself (the Phase 96
    // route), not by the library's name-keyed table.
    // The proof genuinely composes a UF application
    // over guarded element terms: body side, the append
    // stores `f(select(xs, i))` under the loop
    // condition's guard; row side, the forall reads
    // `f(select(xs, i))` under the derived domain; the
    // stated invariant is the Map characterization at
    // the prefix, discharged by the obligations before
    // use. The four crowns above compose through the
    // same arms (their invariants are the same texts,
    // name-keyed); this specimen shows the composition
    // is a property of the arms, not of the shipped
    // names.
    let spec = one(
        r#"Probe { spec map_composed {
            input: xs: List<Int>, f: fn(Int) -> Int
            output: List<Int>
            constraint: len(output) == len(xs)
            constraint: forall i: Int in range(len(xs)): output[i] == f(xs[i])
            test { input: ([1, 2], |n| n + 1); expect: [2, 3] }
        } }"#,
    );
    let body = block(vec![
        alloc_list("out"),
        alloc("i", TypeRef::Int, i(0)),
        IRNode::While {
            condition: Box::new(bin(IRBinOp::Lt, v("i"), len_u(v("xs")))),
            invariant: Some(
                "len(out) == i; 0 <= i; len(xs) >= 0 implies i <= len(xs); \
                 forall j: Int in range(len(out)): out[j] == f(xs[j])"
                    .into(),
            ),
            body: Box::new(block(vec![
                assign(
                    "out",
                    bin(
                        IRBinOp::Append,
                        v("out"),
                        call("f", vec![idx(v("xs"), v("i"))]),
                    ),
                ),
                assign("i", bin(IRBinOp::Add, v("i"), i(1))),
            ])),
        },
        ret(v("out")),
    ]);
    let status = verdict(&spec, body);
    println!("P143|clause3|composed_specimen={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The positive control at a single point: the same
    // UF application on both sides of the row, at a
    // require-guarded element — congruence over the
    // element term, and the row Verifies. This is all
    // §2.9 claims; the next test is what it refuses.
    let spec = one(
        r#"Probe { spec uf_at_first {
            input: xs: List<Int>, f: fn(Int) -> Int
            output: Int
            require: len(xs) >= 1
            constraint: output == f(xs[0])
            test { input: ([5], |n| n + 1); expect: 6 }
        } }"#,
    );
    let body = call("f", vec![idx(v("xs"), i(0))]);
    let status = verdict(&spec, body);
    println!("P143|clause3|same_application={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase143_clause3_beyond_congruence_not_verified() {
    // A value-at-a-point claim about `f` over elements:
    // the row says the UF's value at the first element
    // IS the element. The body (the direct read) is a
    // real program; the row is false for real functions
    // (a countermodel function — `f` constant 0 over a
    // one-element list — refutes it), so the verdict is
    // a genuine VerificationFailed, never a Verified
    // bought by giving the UF a value.
    let spec = one(
        r#"Probe { spec uf_at_first {
            input: xs: List<Int>, f: fn(Int) -> Int
            output: Int
            require: len(xs) >= 1
            constraint: output == f(xs[0])
            test { input: ([5], |n| n + 1); expect: 6 }
        } }"#,
    );
    let status = verdict(&spec, idx(v("xs"), i(0)));
    println!("P143|clause3|value_at_a_point={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // Injectivity over elements: the row claims equal
    // applications imply equal elements. A constant
    // function over a two-element list is a
    // countermodel — Refuted, not Verified. Congruence
    // is the whole claim §2.9 makes; this row needs
    // more, and more is not asserted.
    let spec = one(
        r#"Probe { spec uf_injective {
            input: xs: List<Int>, f: fn(Int) -> Int
            output: Bool
            require: len(xs) >= 2
            constraint: f(xs[0]) == f(xs[1]) implies xs[0] == xs[1]
            test { input: ([1, 2], |n| n + 1); expect: true }
        } }"#,
    );
    let status = verdict(&spec, b(true));
    println!("P143|clause3|injectivity={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // An application at an UNGUARDED index declines
    // with its row (§2.9's first point — §2.3 binds
    // here as everywhere): the body `f(xs[len(xs)])`
    // is unencodable on both sides of the mirror and
    // the verdict is Inconclusive — the UF never gets
    // applied to a term no execution could produce.
    let spec = one(
        r#"Probe { spec uf_oob {
            input: xs: List<Int>, f: fn(Int) -> Int
            output: Int
            constraint: output == f(xs[len(xs)])
            test { input: ([5], |n| n + 1); expect: 6 }
        } }"#,
    );
    let body = call("f", vec![idx(v("xs"), len_u(v("xs")))]);
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P143|clause3|unguarded_application|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(!real && !pure);
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Teeth: wrong bodies never Verify ───────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase143_teeth_wrong_bodies_do_not_verify() {
    let specs = stdlib::prelude_specs();
    let verifier = Verifier::with_specs(&specs);
    let spec_of = |name: &str| specs.iter().find(|s| s.name == name).expect("entry ships");

    // list_map with the identity appended instead of
    // the application: the stated invariant is
    // ATTEMPTED for it (same For shape, the library's
    // supply is name-keyed) — consecution fails (the
    // stored element is `xs[i]`, the invariant claims
    // `f(xs[i])`, and congruence equates nothing), the
    // route falls to exhaustion: Inconclusive.
    let body = loop_body(append_stmt("out", idx(v("xs"), v("i"))));
    let status = verifier
        .verify_one(&candidate(spec_of("list_map"), body), spec_of("list_map"))
        .status;
    println!("P143|teeth|map_identity_append={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // list_map returning the input: the pointwise row
    // Refutes — a countermodel function (constant 0
    // over [5]) separates `xs[i]` from `f(xs[i])`.
    let status = verifier
        .verify_one(&candidate(spec_of("list_map"), v("xs")), spec_of("list_map"))
        .status;
    println!("P143|teeth|map_return_xs={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // list_map returning the empty list: the length row
    // Refutes on any non-empty input.
    let status = verifier
        .verify_one(
            &candidate(spec_of("list_map"), empty_list()),
            spec_of("list_map"),
        )
        .status;
    println!("P143|teeth|map_empty={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // list_filter appending unconditionally: the
    // invariant's predicate row is attempted and
    // consecution fails (the stored element carries no
    // `p` fact) — Inconclusive, never Verified.
    let body = loop_body(append_stmt("out", idx(v("xs"), v("i"))));
    let status = verifier
        .verify_one(&candidate(spec_of("list_filter"), body), spec_of("list_filter"))
        .status;
    println!("P143|teeth|filter_unconditional={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // list_filter appending exactly the FAILING
    // elements: consecution fails the same way (the
    // branch's own fact is `p(xs[i]) == false`) —
    // Inconclusive.
    let body = loop_body(if_(
        IRNode::UnaryOp(IRUnaryOp::Not, Box::new(call("p", vec![idx(v("xs"), v("i"))]))),
        append_stmt("out", idx(v("xs"), v("i"))),
        block(vec![]),
    ));
    let status = verifier
        .verify_one(&candidate(spec_of("list_filter"), body), spec_of("list_filter"))
        .status;
    println!("P143|teeth|filter_inverted={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // list_all with a constant-true body: the
    // counterexample row Refutes (a predicate false at
    // one element of a one-element list).
    let status = verifier
        .verify_one(&candidate(spec_of("list_all"), b(true)), spec_of("list_all"))
        .status;
    println!("P143|teeth|all_const_true={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // list_all with the disjunctive step (any's body):
    // the prefix-conjunction invariant is attempted,
    // consecution fails — Inconclusive.
    let body = block(vec![
        alloc("ok", TypeRef::Bool, b(true)),
        for_(
            "i",
            len_u(v("xs")),
            assign(
                "ok",
                bin(IRBinOp::Or, v("ok"), call("p", vec![idx(v("xs"), v("i"))])),
            ),
        ),
        ret(v("ok")),
    ]);
    let status = verifier
        .verify_one(&candidate(spec_of("list_all"), body), spec_of("list_all"))
        .status;
    println!("P143|teeth|all_or_step={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // list_any with a constant-false body: the witness
    // row Refutes dually.
    let status = verifier
        .verify_one(&candidate(spec_of("list_any"), b(false)), spec_of("list_any"))
        .status;
    println!("P143|teeth|any_const_false={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // list_any with the conjunctive step (all's body):
    // consecution of the prefix-disjunction invariant
    // fails — Inconclusive.
    let body = block(vec![
        alloc("ok", TypeRef::Bool, b(false)),
        for_(
            "i",
            len_u(v("xs")),
            assign(
                "ok",
                bin(IRBinOp::And, v("ok"), call("p", vec![idx(v("xs"), v("i"))])),
            ),
        ),
        ret(v("ok")),
    ]);
    let status = verifier
        .verify_one(&candidate(spec_of("list_any"), body), spec_of("list_any"))
        .status;
    println!("P143|teeth|any_and_step={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── R6 measured: the tier certifies the contract ───────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase143_filter_certifies_exactly_its_two_rows() {
    // §9 R6, in both directions. The frozen contract of
    // list_filter is a length bound and a
    // predicate-holds row — deliberately the weakest of
    // the five (Amendment 2 froze it so). A body that
    // keeps AT MOST the first element, and only when it
    // passes, satisfies both rows exactly — and drops
    // passing elements the name's gloss promises to
    // keep. It Verifies: Tier-1 certifies the frozen
    // contract, never the gloss, and the re-tier
    // surfaces carry the rows so a reader sees what was
    // proven.
    let specs = stdlib::prelude_specs();
    let verifier = Verifier::with_specs(&specs);
    let spec = specs
        .iter()
        .find(|s| s.name == "list_filter")
        .expect("entry ships");
    let body = loop_body(if_(
        bin(
            IRBinOp::And,
            call("p", vec![idx(v("xs"), v("i"))]),
            bin(IRBinOp::Eq, v("i"), i(0)),
        ),
        append_stmt("out", idx(v("xs"), v("i"))),
        block(vec![]),
    ));
    let status = verifier.verify_one(&candidate(spec, body), spec).status;
    println!("P143|r6|filter_first_element_only={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The other direction: the always-empty body
    // satisfies the two rows semantically, yet stands
    // Inconclusive — not a contract failure but an
    // encoder boundary, named: the predicate row reads
    // the output at the symbolic binder, and over a
    // CONCRETE-list value that read has no pair and no
    // derived domain, so the row declines for this
    // body. The crown route is the pair the statement
    // layer builds; a verdict is never bought by
    // splitting the row on the output's length by hand.
    let status = verifier
        .verify_one(&candidate(spec, empty_list()), spec)
        .status;
    println!("P143|r6|filter_empty={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Drift: the eleven stand, the residue stands ────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase143_the_rest_stand() {
    let specs = stdlib::prelude_specs();
    let verifier = Verifier::with_specs(&specs);
    // The eight crowned at Phase 141 and the fold three
    // crowned at Phase 142 do not move at W5.
    for name in [
        "list_reverse",
        "list_contains",
        "list_min",
        "list_max",
        "list_take",
        "list_drop",
        "list_is_sorted",
        "range",
        "list_sum",
        "list_product",
        "list_fold",
    ] {
        let spec = specs.iter().find(|s| &s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let status = verifier.verify_one(&candidate(spec, body), spec).status;
        println!("P143|stand|{name}={status:?}");
        #[cfg(feature = "z3-support")]
        assert_eq!(status, VerificationStatus::Verified, "{name}");
        #[cfg(not(feature = "z3-support"))]
        assert_eq!(status, VerificationStatus::Inconclusive, "{name}");
    }
    // binary_search is the named stretch (judged at
    // Phase 144), sort the standing residue: neither
    // moves at W5.
    for name in ["list_binary_search", "list_sort"] {
        let spec = specs.iter().find(|s| &s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let status = verifier.verify_one(&candidate(spec, body), spec).status;
        println!("P143|stand|{name}={status:?}");
        assert_eq!(status, VerificationStatus::Inconclusive, "{name}");
    }
}

// ── The probe pair holds ───────────────────────────────

const OOB_SPEC: &str = r#"Probe { spec oob_read {
    input: xs: List<Int>
    output: Int
    constraint: output == xs[len(xs)]
    test { input: [10, 20]; expect: 0 }
} }"#;

const LAST_SPEC: &str = r#"Probe { spec last_read {
    input: xs: List<Int>
    output: Int
    require: len(xs) >= 1
    constraint: output == xs[len(xs) - 1]
    test { input: [10, 20]; expect: 20 }
} }"#;

#[test]
fn phase143_probe_pair_holds() {
    // §2.3's honesty instrument, re-pinned at W5: the
    // unguarded read still produces no term anywhere,
    // and the guarded sibling still Verifies.
    let spec = one(OOB_SPEC);
    let body = idx(v("xs"), len_u(v("xs")));
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P143|probe|oob_read|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(!real && !pure);
    assert_eq!(status, VerificationStatus::Inconclusive);

    let spec = one(LAST_SPEC);
    let body = idx(v("xs"), bin(IRBinOp::Sub, len_u(v("xs")), i(1)));
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P143|probe|last_read|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(real && pure);
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Mirror agreement on the UF-at-elements shapes ──────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase143_mirror_agreement_on_uf_element_shapes() {
    // §4.2 for W5's shapes. The composition's acceptance
    // is row-side and invariant-side; the mirror judges
    // BODIES, and the two encoders agree per shape:
    //
    // - a UF application at a require-guarded element
    //   encodes on both sides (Int → Int and
    //   Int → Bool alike);
    // - the same application unguarded declines on
    //   both sides (with the `uf_oob` specimen above);
    // - the Map / Filter NODE bodies decline on both
    //   sides — §2.9 characterizes the nodes' meanings
    //   where the contracts need them, it does not add
    //   a node encoding (the Phase 137 pin's standing);
    // - the four shipped bodies decline on both sides —
    //   they are statement bodies, judged by the
    //   statement route, outside both expression
    //   predicates, as at W3/W4.
    let guarded = one(
        r#"Probe { spec uf_guarded {
            input: xs: List<Int>, i: Int, f: fn(Int) -> Int
            output: Int
            require: i >= 0
            require: i < len(xs)
            constraint: output == f(xs[i])
            test { input: ([5], 0, |n| n + 1); expect: 6 }
        } }"#,
    );
    let pair = enc_pair(&guarded, &call("f", vec![idx(v("xs"), v("i"))]));
    println!("P143|mirror|uf_at_guarded_element={pair:?}");
    assert_eq!(pair, (true, true));

    let pred_guarded = one(
        r#"Probe { spec pred_guarded {
            input: xs: List<Int>, i: Int, p: fn(Int) -> Bool
            output: Bool
            require: i >= 0
            require: i < len(xs)
            constraint: output == p(xs[i])
            test { input: ([5], 0, |n| n > 0); expect: true }
        } }"#,
    );
    let pair = enc_pair(&pred_guarded, &call("p", vec![idx(v("xs"), v("i"))]));
    println!("P143|mirror|pred_at_guarded_element={pair:?}");
    assert_eq!(pair, (true, true));

    let unguarded = one(
        r#"Probe { spec uf_unguarded {
            input: xs: List<Int>, i: Int, f: fn(Int) -> Int
            output: Int
            constraint: output == f(xs[i])
            test { input: ([5], 0, |n| n + 1); expect: 6 }
        } }"#,
    );
    let pair = enc_pair(&unguarded, &call("f", vec![idx(v("xs"), v("i"))]));
    println!("P143|mirror|uf_at_unguarded_element={pair:?}");
    assert_eq!(pair, (false, false));

    let map_spec = one(
        r#"Probe { spec probe_map_node {
            input: xs: List<Int>, f: fn(Int) -> Int
            output: List<Int>
            constraint: len(output) == len(xs)
            test { input: ([1], |n| n + 1); expect: [2] }
        } }"#,
    );
    let map_node = IRNode::Map {
        function: Box::new(v("f")),
        over: Box::new(v("xs")),
    };
    let pair = enc_pair(&map_spec, &map_node);
    println!("P143|mirror|map_node={pair:?}");
    assert_eq!(pair, (false, false));

    let filter_spec = one(
        r#"Probe { spec probe_filter_node {
            input: xs: List<Int>, p: fn(Int) -> Bool
            output: List<Int>
            constraint: len(output) <= len(xs)
            test { input: ([1], |n| n > 0); expect: [1] }
        } }"#,
    );
    let filter_node = IRNode::Filter {
        predicate: Box::new(v("p")),
        over: Box::new(v("xs")),
    };
    let pair = enc_pair(&filter_spec, &filter_node);
    println!("P143|mirror|filter_node={pair:?}");
    assert_eq!(pair, (false, false));

    let specs = stdlib::prelude_specs();
    for name in FOUR {
        let spec = specs.iter().find(|s| &s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let pair = enc_pair(spec, &body);
        println!("P143|mirror|shipped|{name}={pair:?}");
        assert_eq!(pair, (false, false), "{name}");
    }
}
