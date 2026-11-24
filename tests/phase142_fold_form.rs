//! Phase 142 (W4) — the fold-form arm (LIST_ENCODER_SCOPE
//! §2.8), run as the measurement the freeze prescribes:
//! the fold's prefix definition — base + step, and
//! nothing stronger — and the three fold contracts
//! judged jointly under §3.2's W4-insufficiency rule
//! (R4).
//!
//! The characterization the arm may assert, exactly as
//! frozen (§2.8): base — the fold over the empty prefix
//! is `init`; step — the fold over the prefix of length
//! `k + 1` is `op(fold over the prefix of length k,
//! select(xs, k))`, for `k` in range. No closed form, no
//! commutativity or associativity fact, nothing about
//! folds over reordered or filtered lists, no value at
//! any point the prefix definition does not determine.
//! The build realizes it by structural recursion on the
//! length — the second realization §2.8 names: a Z3
//! recursive function definition per op over (array,
//! length, init) whose two branches are the base and
//! step rows. (The first realization — the two rows as
//! universally quantified axioms — was built and
//! measured first: it discharges the proofs, but every
//! session carrying the axioms answers Unknown in the
//! refutation direction, so the teeth below could not
//! exist under it; the phase record carries the
//! measurement.) For `list_sum` / `list_product` the op
//! is the contract's closed lambda; for `list_fold` the
//! op is the parameter `f` itself — a UF under the
//! Phase 133 arm, so the definition's step branch is
//! stated over UF applications and congruence is all it
//! carries (§2.9's Fold point).
//!
//! The shipped bodies are For loops accumulating; the
//! vehicle is Phase 141's stated-invariant machinery —
//! the invariant states the accumulator equals the fold
//! over the consumed prefix, in the prefix form
//! (`acc == fold(list_take(xs, i), init, op)`; the fold
//! arm denotes a fold over `list_take(xs, i)` directly
//! as the prefix fold at the clamped count, exact for
//! every `i` by `list_take`'s frozen contract — the
//! take-clamp specimen below separates that denotation
//! from a raw-count term).
//!
//! The exit, per R4: the arm discharges all three rows
//! honestly, or all three stay Tier-2 with the
//! insufficiency named — no partial credit, no
//! intermediate tier. The pins below state the measured
//! exit; the ROAD note carries it in its first lines.
//!
//! Every test prints a `P142|...` line with the measured
//! standing; assertions pin exactly what the line
//! reports. In the no-Z3 build the verdicts are the
//! honest subset (Inconclusive throughout); the
//! encodability pins are configuration-independent
//! (real == pure per shape).

use morphic::spec::ast::{FunctionSpec, TypeRef};
use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, IRUnaryOp, Provenance, ScoreBreakdown,
    SpecIndex,
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
        id: 142,
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
fn alloc(name: &str, initial: IRNode) -> IRNode {
    IRNode::Alloc {
        name: name.into(),
        ty: TypeRef::Int,
        initial: Box::new(initial),
    }
}
fn assign(name: &str, value: IRNode) -> IRNode {
    IRNode::Assign {
        target: Box::new(v(name)),
        value: Box::new(value),
    }
}
fn for_loop(var: &str, end: IRNode, body: IRNode) -> IRNode {
    IRNode::For {
        var: var.into(),
        start: Box::new(i(0)),
        end: Box::new(end),
        body: Box::new(body),
    }
}
fn ret(value: IRNode) -> IRNode {
    IRNode::Return(Box::new(value))
}
fn block(stmts: Vec<IRNode>) -> IRNode {
    IRNode::Block(stmts)
}

/// The accumulating loop shape all three shipped bodies
/// share, parameterized so the teeth can mutate one
/// joint at a time.
fn accumulate(init: IRNode, step: IRNode, tail: IRNode) -> IRNode {
    block(vec![
        alloc("acc", init),
        for_loop("i", len_u(v("xs")), assign("acc", step)),
        ret(tail),
    ])
}

fn sum_body() -> IRNode {
    accumulate(
        i(0),
        bin(IRBinOp::Add, v("acc"), idx(v("xs"), v("i"))),
        v("acc"),
    )
}

// ── The three, crowned (the measurement's exit) ────────

const THREE: &[&str] = &["list_sum", "list_product", "list_fold"];

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase142_the_fold_three_crowned() {
    // The phase's exit standing under §3.2/§3.3 + R4:
    // each of the three Verifies through the production
    // pipeline with its shipped body — its fold row
    // Proven under exactly §2.8's base + step — and
    // stays battery-exact under the interpreter.
    let specs = stdlib::prelude_specs();
    let verifier = Verifier::with_specs(&specs);
    for name in THREE {
        let spec = specs.iter().find(|s| &s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let run = eval::run_spec_tests(&body, spec);
        println!(
            "P142|crown|{name}|battery={}/{}",
            run.passed, run.evaluable
        );
        assert_eq!(run.passed, run.evaluable, "{name} battery");
        assert!(run.evaluable > 0, "{name} battery is non-empty");
        let result = verifier.verify_one(&candidate(spec, body), spec);
        for (row, r) in result.constraint_results.iter().enumerate() {
            println!("P142|crown|{name}|post[{row}]|holds={}", r.holds);
        }
        println!("P142|crown|{name}|verify={:?}", result.status);
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

// ── The arm decides fold rows in both directions ───────

#[test]
fn phase142_fold_rows_encode_and_decide() {
    // A fold over a concrete list is computed outright
    // (the prefix definition at a known length): the
    // exact body Verifies, the off-by-one body Refutes.
    let conc = one(
        r#"Probe { spec conc_fold {
            input: n: Int
            output: Int
            constraint: output == fold([1, 2, 3], 0, |a, v| a + v)
            test { input: 0; expect: 6 }
        } }"#,
    );
    let status = verdict(&conc, i(6));
    println!("P142|row|concrete_exact={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
    let status = verdict(&conc, i(7));
    println!("P142|row|concrete_off_by_one={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The prefix form as a row: `fold(list_take(xs, i),
    // ...)` denotes the fold over the consumed prefix.
    // With `i` in range, a constant-zero body Refutes
    // (the one-element prefix [5] is a countermodel) —
    // the row encodes and the step row gives it a real
    // value.
    let pref = one(
        r#"Probe { spec take_fold {
            input: xs: List<Int>, i: Int
            output: Int
            require: 0 <= i
            require: i <= len(xs)
            constraint: output == fold(list_take(xs, i), 0, |a, v| a + v)
            test { input: ([5], 1); expect: 5 }
        } }"#,
    );
    let status = verdict(&pref, i(0));
    println!("P142|row|take_prefix_const={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The clamp is the denotation, not a raw count:
    // with `i <= 0` the prefix is empty whatever `xs`
    // holds, so the zero body Verifies. A raw-count
    // term would leave the fold at a negative count
    // unconstrained and this would stand Inconclusive.
    let empty = one(
        r#"Probe { spec take_fold_empty {
            input: xs: List<Int>, i: Int
            output: Int
            require: i <= 0
            constraint: output == fold(list_take(xs, i), 0, |a, v| a + v)
            test { input: ([5], 0); expect: 0 }
        } }"#,
    );
    let status = verdict(&empty, i(0));
    println!("P142|row|take_prefix_empty={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The op must be closed: a lambda capturing a spec
    // parameter is outside the arm's slice (§2.8's ops
    // are the contracts' closed lambdas and the UF
    // parameter itself), so the row declines — a
    // constant body is Inconclusive, never Refuted by
    // an invented term.
    let capturing = one(
        r#"Probe { spec fold_capture {
            input: xs: List<Int>, y: Int
            output: Int
            constraint: output == fold(xs, 0, |a, v| a + v + y)
            test { input: ([1], 10); expect: 11 }
        } }"#,
    );
    let status = verdict(&capturing, i(0));
    println!("P142|row|capturing_op={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // §2.8's limit, pinned: the same sum claimed under
    // BOTH argument orders of the op. The two folds are
    // different ops; their equality at a symbolic length
    // is an induction the prefix definition does not
    // assert (and this probe name carries no stated
    // invariant), so the pair does not crown — the arm
    // asserts base + step per op and nothing across ops.
    let both_orders = one(
        r#"Probe { spec sum_both_orders {
            input: xs: List<Int>
            output: Int
            constraint: output == fold(xs, 0, |a, v| a + v)
            constraint: output == fold(xs, 0, |a, v| v + a)
            test { input: [1, 2]; expect: 3 }
        } }"#,
    );
    let status = verdict(&both_orders, sum_body());
    println!("P142|row|both_orders={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Teeth: wrong accumulators never Verify ─────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase142_teeth_wrong_bodies_do_not_verify() {
    let specs = stdlib::prelude_specs();
    let verifier = Verifier::with_specs(&specs);
    let spec_of = |name: &str| specs.iter().find(|s| s.name == name).expect("entry ships");

    // The shipped sum loop returning `acc + 1`: the
    // loop still closes under the stated invariant, and
    // the row then Refutes outright (at the empty list
    // the body says 1, the fold says 0).
    let body = accumulate(
        i(0),
        bin(IRBinOp::Add, v("acc"), idx(v("xs"), v("i"))),
        bin(IRBinOp::Add, v("acc"), i(1)),
    );
    let status = verifier
        .verify_one(&candidate(spec_of("list_sum"), body), spec_of("list_sum"))
        .status;
    println!("P142|teeth|sum_return_plus_one={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // A constant body against list_sum Refutes (a
    // one-element list is a countermodel) — the fold
    // row is a real claim, not a rubber stamp.
    let status = verifier
        .verify_one(&candidate(spec_of("list_sum"), i(0)), spec_of("list_sum"))
        .status;
    println!("P142|teeth|sum_constant={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The sum loop with the accumulator started at 1:
    // the stated invariant is ATTEMPTED and initiation
    // fails (1 is not the fold over the empty prefix),
    // so the route falls to exhaustion — Inconclusive,
    // never a Verified bought by an invariant the body
    // does not keep.
    let body = accumulate(
        i(1),
        bin(IRBinOp::Add, v("acc"), idx(v("xs"), v("i"))),
        v("acc"),
    );
    let status = verifier
        .verify_one(&candidate(spec_of("list_sum"), body), spec_of("list_sum"))
        .status;
    println!("P142|teeth|sum_wrong_init={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // list_product judged with an ADDING step: the
    // product invariant's consecution fails (a fold
    // that multiplies is not preserved by a step that
    // adds) — Inconclusive, never Verified.
    let body = accumulate(
        i(1),
        bin(IRBinOp::Add, v("acc"), idx(v("xs"), v("i"))),
        v("acc"),
    );
    let status = verifier
        .verify_one(
            &candidate(spec_of("list_product"), body),
            spec_of("list_product"),
        )
        .status;
    println!("P142|teeth|product_add_step={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // list_fold with the UF applied in swapped order:
    // congruence cannot equate `f(select, acc)` with
    // the step row's `f(acc, select)` — consecution
    // fails and the verdict is Inconclusive.
    let body = accumulate(
        v("init"),
        call("f", vec![idx(v("xs"), v("i")), v("acc")]),
        v("acc"),
    );
    let status = verifier
        .verify_one(&candidate(spec_of("list_fold"), body), spec_of("list_fold"))
        .status;
    println!("P142|teeth|fold_swapped_args={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // A constant body against list_fold Refutes: the
    // UF's value at one step is a genuine countermodel
    // (the parameter ranges over every function).
    let status = verifier
        .verify_one(&candidate(spec_of("list_fold"), i(0)), spec_of("list_fold"))
        .status;
    println!("P142|teeth|fold_constant={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Drift: the eight stand, the W5 four crowned, the residue stands ─

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase142_standings_after_w5() {
    // The eight crowned at Phase 141 do not move at W4
    // — and do not move at W5 either.
    let specs = stdlib::prelude_specs();
    let verifier = Verifier::with_specs(&specs);
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
        let spec = specs.iter().find(|s| &s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let status = verifier.verify_one(&candidate(spec, body), spec).status;
        println!("P142|stand|{name}={status:?}");
        #[cfg(feature = "z3-support")]
        assert_eq!(status, VerificationStatus::Verified, "{name}");
        #[cfg(not(feature = "z3-support"))]
        assert_eq!(status, VerificationStatus::Inconclusive, "{name}");
    }
    // At Phase 142 the four higher-order entries stood
    // Inconclusive here (W5's, not yet built). At Phase
    // 143 the UF-composition arm crowned them
    // (tests/phase143_uf_composition.rs), so their pins
    // retire in place per §9 R10: they Verify with
    // their shipped bodies. binary_search is the named
    // stretch and sort the standing residue: neither
    // moves at W4 or W5 (list_fold's move is the crown
    // test's, above).
    for name in ["list_map", "list_filter", "list_all", "list_any"] {
        let spec = specs.iter().find(|s| &s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let status = verifier.verify_one(&candidate(spec, body), spec).status;
        println!("P142|stand|{name}={status:?}");
        #[cfg(feature = "z3-support")]
        assert_eq!(status, VerificationStatus::Verified, "{name}");
        #[cfg(not(feature = "z3-support"))]
        assert_eq!(status, VerificationStatus::Inconclusive, "{name}");
    }
    for name in ["list_binary_search", "list_sort"] {
        let spec = specs.iter().find(|s| &s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let status = verifier.verify_one(&candidate(spec, body), spec).status;
        println!("P142|stand|{name}={status:?}");
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
fn phase142_probe_pair_holds() {
    // §2.3's honesty instrument, re-pinned at W4: the
    // unguarded read still produces no term anywhere,
    // and the guarded sibling still Verifies.
    let spec = one(OOB_SPEC);
    let body = idx(v("xs"), len_u(v("xs")));
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P142|probe|oob_read|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(!real && !pure);
    assert_eq!(status, VerificationStatus::Inconclusive);

    let spec = one(LAST_SPEC);
    let body = idx(v("xs"), bin(IRBinOp::Sub, len_u(v("xs")), i(1)));
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P142|probe|last_read|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(real && pure);
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Mirror agreement on the fold shapes ────────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase142_mirror_agreement_on_fold_shapes() {
    // W4's acceptance is row-side (constraint rows and
    // invariant rows), where the mirror's standing
    // boundary is silence — it judges BODIES. The fold
    // BODY shapes therefore agree as declines on both
    // sides: no body-side fold arm was built (the
    // shipped bodies are For loops; a `Fold` node or a
    // `fold(...)` call body has no body-side denotation
    // in either encoder, and giving the call one would
    // crown list_fold's sweep denotation by reflexivity
    // whether or not its shipped body proved).
    let specs = stdlib::prelude_specs();
    let fold_spec = specs
        .iter()
        .find(|s| s.name == "list_fold")
        .expect("entry ships");

    let fold_node = IRNode::Fold {
        operation: Box::new(v("f")),
        initial: Box::new(v("init")),
        over: Box::new(v("xs")),
    };
    let pair = enc_pair(fold_spec, &fold_node);
    println!("P142|mirror|fold_node={pair:?}");
    assert_eq!(pair, (false, false));

    let fold_call = call("fold", vec![v("xs"), v("init"), v("f")]);
    let pair = enc_pair(fold_spec, &fold_call);
    println!("P142|mirror|fold_call_body={pair:?}");
    assert_eq!(pair, (false, false));

    for name in THREE {
        let spec = specs.iter().find(|s| &s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let pair = enc_pair(spec, &body);
        println!("P142|mirror|shipped|{name}={pair:?}");
        assert_eq!(pair, (false, false), "{name}");
    }

    // A fold-shaped REQUIRE contributes no in-range
    // fact on either side: alone, it leaves a read
    // unguarded in both encoders; beside the range
    // requires that do carry the fact, the read encodes
    // on both sides (the fold row itself now encodes on
    // the real side — as a guard term it is inert: it
    // constrains no index).
    let guarded = one(
        r#"Probe { spec fold_req_guarded {
            input: xs: List<Int>, i: Int
            output: Int
            require: 0 <= i
            require: i < len(xs)
            require: fold(xs, 0, |a, v| a + v) == 0
            constraint: output == xs[i]
            test { input: ([0], 0); expect: 0 }
        } }"#,
    );
    let pair = enc_pair(&guarded, &idx(v("xs"), v("i")));
    println!("P142|mirror|fold_require_guarded={pair:?}");
    assert_eq!(pair, (true, true));

    let alone = one(
        r#"Probe { spec fold_req_alone {
            input: xs: List<Int>, i: Int
            output: Int
            require: fold(xs, 0, |a, v| a + v) == 0
            constraint: output == xs[i]
            test { input: ([0], 0); expect: 0 }
        } }"#,
    );
    let pair = enc_pair(&alone, &idx(v("xs"), v("i")));
    println!("P142|mirror|fold_require_alone={pair:?}");
    assert_eq!(pair, (false, false));
}
