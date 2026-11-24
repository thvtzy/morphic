//! Phase 137 — scope audit probes for the two headline
//! post-1.2 campaigns: (A) the §3.9 List element encoder
//! and (B) quantified-mod / `int_gcd`'s return.
//!
//! Every test prints a `P137|...` line with the measured
//! standing; assertions pin exactly what the line reports.
//! In the no-Z3 build the verdicts are the honest subset
//! (Inconclusive throughout) — the pins branch on the
//! feature exactly as the Phase 127/133 probes do.

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
        id: 137,
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

fn v(n: &str) -> IRNode {
    IRNode::Var(n.into())
}
fn i(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}
fn bin(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
    IRNode::BinOp(op, Box::new(l), Box::new(r))
}

// ── Half A: the wall, per Tier-2 entry ─────────────────

const TIER2: &[&str] = &[
    "list_reverse",
    "list_contains",
    "list_sum",
    "list_product",
    "list_min",
    "list_max",
    "list_take",
    "list_drop",
    "list_is_sorted",
    "list_sort",
    "list_binary_search",
    "list_map",
    "list_filter",
    "list_fold",
    "list_all",
    "list_any",
    "range",
];

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase137_all_tier2_entries_stand_inconclusive() {
    // The §3.9 wall stated as one sweep: every Tier-2
    // entry's shipped (faithful, battery-exact) body is
    // Inconclusive today. An element encoder that works
    // moves entries off this line loudly, one by one.
    let specs = stdlib::prelude_specs();
    for name in TIER2 {
        let spec = specs.iter().find(|s| &s.name == name).expect("entry ships");
        let body = stdlib::shipped_body(name).expect("entry has a shipped body");
        let status = Verifier::with_specs(&specs).verify_one(&candidate(spec, body), spec).status;
        println!("P137|A|tier2|{name}={status:?}");
        assert_eq!(status, VerificationStatus::Inconclusive, "{name}");
    }
}

#[test]
fn phase137_length_level_still_verifies() {
    // The wall is element-level, not list-level: a
    // length-only contract over the same parameter kind
    // Verifies today (the control).
    let spec = one(
        r#"Probe { spec len_only {
            input: xs: List<Int>
            output: Int
            constraint: output == len(xs)
            test { input: [1, 2]; expect: 2 }
        } }"#,
    );
    let body = IRNode::Call { function: "len".into(), args: vec![v("xs")] };
    let status = verdict(&spec, body);
    println!("P137|A|control|len_only={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase137_decline_shapes_by_name() {
    // (1) Indexing into a symbolic list. RETIRED IN
    // PLACE at Phase 140 (W2) per LIST_ENCODER_SCOPE §9
    // R10: the pin below was the audit's standing — at
    // Phase 137 the List parameter bound length-only
    // (`ListLen`), so the read had no element term at
    // all. W1 (Phase 139) bound the pair; W2 (Phase 140)
    // built the index arm under §2.3's guard discipline,
    // and this specimen's requires (`i >= 0`,
    // `i < len(xs)`) are exactly its guard: the read now
    // encodes on both sides of the mirror and the spec
    // Verifies. The unguarded shape still declines —
    // Phase 140's own battery pins it (the §4.1 probe
    // pair: `oob_read` never Verifies).
    let idx = one(
        r#"Probe { spec probe_index {
            input: xs: List<Int>, i: Int
            output: Int
            require: i >= 0
            require: i < len(xs)
            constraint: output == xs[i]
            test { input: ([10, 20], 1); expect: 20 }
        } }"#,
    );
    let body = IRNode::Index(Box::new(v("xs")), Box::new(v("i")));
    let enc = verifier::body_is_encodable(&idx, &body);
    let status = verdict(&idx, body);
    println!("P137|A|decline|symbolic_index|encodable={enc}|verify={status:?}");
    assert!(enc, "the require-guarded index encodes since Phase 140 (W2)");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // (2) List equality between two symbolic lists.
    // RETIRED IN PLACE at Phase 140 (W2) per
    // LIST_ENCODER_SCOPE §9 R10: at Phase 137 the two
    // parameters bound length-only and equality had no
    // arm at all. W2 built the equality arm in the
    // frozen §2.4 form — equal lengths ∧ pointwise
    // equality over [0, len), never raw array equality
    // (R1) — and the shape now encodes on both sides of
    // the mirror and Verifies (the body denotes exactly
    // the row's claim). Its teeth are Phase 140's pins:
    // two free pairs are refutably unequal in general,
    // so the form is a semantics, not a rubber stamp.
    let eq = one(
        r#"Probe { spec probe_list_eq {
            input: xs: List<Int>, ys: List<Int>
            output: Bool
            constraint: output == (xs == ys)
            test { input: ([1], [1]); expect: true }
        } }"#,
    );
    let body = bin(IRBinOp::Eq, v("xs"), v("ys"));
    let enc = verifier::body_is_encodable(&eq, &body);
    let status = verdict(&eq, body);
    println!("P137|A|decline|list_equality|encodable={enc}|verify={status:?}");
    assert!(enc, "pair equality encodes in the §2.4 form since Phase 140 (W2)");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // (3) An element quantifier as the whole contract
    // (list_contains' shape, distilled).
    let fa = one(
        r#"Probe { spec probe_all_zero {
            input: xs: List<Int>
            output: Bool
            constraint: (forall i: Int in range(len(xs)): xs[i] == 0) implies output == true
            constraint: (forall i: Int in range(len(xs)): xs[i] != 0) implies output == false
            test { input: [0, 0]; expect: true }
        } }"#,
    );
    let body = IRNode::Literal(IRLiteral::Bool(true));
    let status = verdict(&fa, body);
    println!("P137|A|decline|element_forall={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // (4) An exists-over-elements row (list_min's third
    // row shape).
    let ex = one(
        r#"Probe { spec probe_exists {
            input: xs: List<Int>
            output: Bool
            constraint: (exists i: Int in range(len(xs)): xs[i] == 7) implies output == true
            test { input: [7]; expect: true }
        } }"#,
    );
    let status = verdict(&ex, IRNode::Literal(IRLiteral::Bool(true)));
    println!("P137|A|decline|element_exists={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // (5) A Map body applying a parameter (G4_SCOPE §6's
    // re-filing: HO node encodings belong to §3.9).
    let map_spec = one(
        r#"Probe { spec probe_map_body {
            input: xs: List<Int>, f: fn(Int) -> Int
            output: List<Int>
            constraint: len(output) == len(xs)
            test { input: ([1], |n| n + 1); expect: [2] }
        } }"#,
    );
    let map_body = IRNode::Map { function: Box::new(v("f")), over: Box::new(v("xs")) };
    let enc = verifier::body_is_encodable(&map_spec, &map_body);
    let pure = body_is_encodable_pure(&map_spec, &map_body, &SpecIndex::empty());
    let status = verdict(&map_spec, map_body);
    println!("P137|A|decline|map_body|encodable={enc}|mirror={pure}|verify={status:?}");
    assert!(!enc && !pure, "Map bodies stay unencodable on both sides of the mirror");
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase137_concrete_lists_do_encode() {
    // The boundary is symbolic-vs-concrete, not lists as
    // such: a body building a concrete list and indexing
    // it encodes and Verifies today (Sym::List exists for
    // literal lists). The encoder's missing piece is the
    // PARAMETER binding, not the list value machinery.
    let spec = one(
        r#"Probe { spec probe_concrete {
            input: x: Int
            output: Int
            constraint: output == x
            test { input: 5; expect: 5 }
        } }"#,
    );
    let list = IRNode::Call {
        function: "List".into(),
        args: vec![v("x"), i(9)],
    };
    let body = IRNode::Index(Box::new(list), Box::new(i(0)));
    let enc = verifier::body_is_encodable(&spec, &body);
    let status = verdict(&spec, body);
    println!("P137|A|concrete|encodable={enc}|verify={status:?}");
    #[cfg(feature = "z3-support")]
    {
        assert!(enc);
        assert_eq!(status, VerificationStatus::Verified);
    }
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Half B: quantified-mod and gcd ─────────────────────

const GCD_SPEC: &str = r#"Probe { spec int_gcd {
    input: a: Int, b: Int
    output: Int
    require: a != (0 - 9223372036854775807 - 1)
    require: b != (0 - 9223372036854775807 - 1)
    constraint: output >= 0
    constraint: a != 0 || b != 0 || output == 0
    constraint: b != 0 || a == 0 || output == int_abs(a)
    constraint: a != 0 || b == 0 || output == int_abs(b)
    constraint: output == 0 || (a % output == 0 && b % output == 0)
    constraint: forall d: Int in range(int_abs(a) + int_abs(b) + 1): d == 0 || d <= output || a % d != 0 || b % d != 0 || a == 0 || b == 0
    test { input: (12, 18); expect: 6 }
    test { input: (0, 0); expect: 0 }
    test { input: (7, 0); expect: 7 }
    test { input: (0, (0 - 5)); expect: 5 }
    test { input: ((0 - 12), 18); expect: 6 }
    test { input: (17, 5); expect: 1 }
} }"#;

fn abs_of(x: &str) -> IRNode {
    IRNode::If {
        condition: Box::new(bin(IRBinOp::Lt, v(x), i(0))),
        then_branch: Box::new(bin(IRBinOp::Sub, i(0), v(x))),
        else_branch: Box::new(v(x)),
    }
}

fn euclid_body(invariant: &str) -> IRNode {
    IRNode::Block(vec![
        IRNode::Alloc { name: "x".into(), ty: morphic::spec::ast::TypeRef::Int, initial: Box::new(abs_of("a")) },
        IRNode::Alloc { name: "y".into(), ty: morphic::spec::ast::TypeRef::Int, initial: Box::new(abs_of("b")) },
        IRNode::While {
            condition: Box::new(bin(IRBinOp::Neq, v("y"), i(0))),
            body: Box::new(IRNode::Block(vec![
                IRNode::Alloc { name: "t".into(), ty: morphic::spec::ast::TypeRef::Int, initial: Box::new(v("y")) },
                IRNode::Assign { target: Box::new(v("y")), value: Box::new(bin(IRBinOp::Mod, v("x"), v("y"))) },
                IRNode::Assign { target: Box::new(v("x")), value: Box::new(v("t")) },
            ])),
            invariant: Some(invariant.to_string()),
        },
        IRNode::Return(Box::new(v("x"))),
    ])
}

const FULL_INVARIANT: &str = "x >= 0; y >= 0; \
    forall d: Int in range(int_abs(a) + int_abs(b) + 1): d == 0 || a % d != 0 || b % d != 0 || x % d == 0; \
    forall d: Int in range(int_abs(a) + int_abs(b) + 1): d == 0 || a % d != 0 || b % d != 0 || y % d == 0; \
    forall d: Int in range(int_abs(a) + int_abs(b) + 1): d == 0 || x % d != 0 || y % d != 0 || a % d == 0; \
    forall d: Int in range(int_abs(a) + int_abs(b) + 1): d == 0 || x % d != 0 || y % d != 0 || b % d == 0; \
    x + y <= int_abs(a) + int_abs(b)";

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase137_gcd_blocker_remeasured() {
    let functions = checked(GCD_SPEC);
    let spec = &functions[0];

    // Execution is not the gap (Phase 111's standing).
    let run = eval::run_spec_tests(&euclid_body(FULL_INVARIANT), spec);
    println!("P137|B|gcd|executed={}/{}", run.passed, run.evaluable);
    assert_eq!((run.passed, run.evaluable), (6, 6));

    // (a) The full preservation invariant: still
    // Inconclusive on the current tree (Phases 96/133
    // moved other things; neither discharges quantified
    // divisibility preservation).
    let status = Verifier::with_specs(&functions)
        .verify_one(&candidate(spec, euclid_body(FULL_INVARIANT)), spec)
        .status;
    println!("P137|B|gcd|full_invariant={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // (b) The plain invariant is REFUTED — the
    // preservation rows are load-bearing, not ceremony.
    let status = Verifier::with_specs(&functions)
        .verify_one(&candidate(spec, euclid_body("x >= 0; y >= 0")), spec)
        .status;
    println!("P137|B|gcd|plain_invariant={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase137_mod_ground_facts_do_discharge() {
    // What DOES discharge today: ground / unquantified
    // mod reasoning. Z3's native div/mod handles a
    // symbolic dividend with a literal modulus and a
    // stated range — the wall is the QUANTIFIED
    // preservation shape, not `%` itself.
    let spec = one(
        r#"Probe { spec mod_range {
            input: a: Int
            output: Int
            require: a >= 0
            constraint: output >= 0
            constraint: output < 3
            constraint: a % 3 == output || a % 3 != output
            test { input: 7; expect: 1 }
        } }"#,
    );
    let body = bin(IRBinOp::Mod, v("a"), i(3));
    let status = verdict(&spec, body);
    println!("P137|B|mod|symbolic_mod_range={status:?}");
    // The body's own value IS a % 3, so output in [0,3)
    // must follow from Z3's mod totalization for a >= 0.
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // And a CONCRETE gcd instance discharges: with both
    // arguments pinned by require rows, the bounded
    // forall over divisors is the Phase 94 envelope and
    // every mod in it is ground.
    let concrete = one(
        r#"Probe { spec gcd_concrete {
            input: a: Int, b: Int
            output: Int
            require: a == 12
            require: b == 18
            constraint: output >= 0
            constraint: output == 0 || (a % output == 0 && b % output == 0)
            constraint: forall d: Int in range(31): d == 0 || d <= output || a % d != 0 || b % d != 0
            test { input: (12, 18); expect: 6 }
        } }"#,
    );
    let status = verdict(&concrete, i(6));
    println!("P137|B|mod|concrete_gcd={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase137_no_euclid_seed_family() {
    // Half (i) of the blocker, re-measured as a census:
    // no template/seed family in the synthesis layer
    // constructs a While+Mod Euclid shape. The census is
    // textual over the two seed sources — a family added
    // later changes this count loudly.
    let templates = include_str!("../src/synthesis/templates.rs");
    let engine = include_str!("../src/synthesis/engine.rs");
    let hits = templates.matches("Euclid").count()
        + engine.matches("Euclid").count()
        + templates.matches("gcd").count()
        + engine.matches("gcd").count();
    println!("P137|B|seed|euclid_mentions={hits}");
    assert_eq!(hits, 0, "no Euclid seed family exists in the synthesis layer");
}
