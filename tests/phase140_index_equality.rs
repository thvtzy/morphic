//! Phase 140 (W2) — the index and equality arms over the
//! paired binding (LIST_ENCODER_SCOPE §2.3, §2.4, §2.6),
//! plus the §4.1 fabricated-countermodel probe pair.
//!
//! The index arm produces the `select` term ONLY under
//! the guard discipline: an index term is produced solely
//! when its in-range proof arises from the row's own
//! guard — a `require:` hypothesis in scope, an antecedent
//! of the row being encoded, or a concrete index provably
//! in range (the bounded-quantifier range is W3's source).
//! An unguarded index declines in BOTH encoders, never
//! totalizes. The equality arm encodes Eq/Neq over two
//! paired bindings (and pair vs concrete list through the
//! §2.5 bridge) in the §2.4 form — equal lengths ∧
//! pointwise equality over [0, len) — never raw array
//! equality (R1). Concat of two pairs takes §2.6's named
//! answer: it binds as its exact length (the length-only
//! form), element rows over it unencodable.
//!
//! Guard sourcing, stated once for both sides (pinned
//! below): the BODY side sources its guards from the
//! spec's `require:` rows (verify_one evaluates the body
//! under the precondition terms; the statement route's
//! evaluation context carries the same terms). The
//! CONSTRAINT side sources its guards from the same
//! `require:` terms PLUS each row's own antecedents
//! (encoding an Implies row adds the encoded antecedent
//! to the guard set for the consequent). The mirror
//! sources the same facts from the spec's require rows,
//! in a difference-logic domain over parameter / length
//! bases — a sound subset of the solver's entailment, so
//! the mirror never accepts a read the real side declines.
//!
//! The probe pair (§4.1) is the phase's exit standing:
//! `oob_read` must NOT Verify — its read declines for want
//! of a guard, in every context, and is rescued by no
//! totalization; `last_read` (the same read guarded by
//! `require: len(xs) >= 1`) must Verify.
//!
//! The sweep-identity pin for this phase is the standing
//! instrument, `phase133_corpus_verdict_drift`, and the
//! 17/17 Tier-2 Inconclusive sweep is
//! `phase137_all_tier2_entries_stand_inconclusive` (their
//! rows are quantifier-shaped — W3's — so they stand).
//! The P137 decline-shape pins for symbolic index and
//! symbolic list equality retire in place per R10 in
//! `phase137_encoder_audit.rs`; the specimens here are
//! this phase's own.
//!
//! Every test prints a `P140|...` line with the measured
//! standing; assertions pin exactly what the line reports.
//! In the no-Z3 build the verdicts are the honest subset
//! (Inconclusive throughout); the encodability pins are
//! configuration-independent (real == pure per shape).

use morphic::spec::ast::{FunctionSpec, TypeRef};
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, Provenance, ScoreBreakdown, SpecIndex,
};
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
        id: 140,
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

/// The two encodability predicates, real and mirror.
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
fn len(of: IRNode) -> IRNode {
    call("len", vec![of])
}
fn list(items: Vec<IRNode>) -> IRNode {
    call("List", items)
}
fn idx(coll: IRNode, at: IRNode) -> IRNode {
    IRNode::Index(Box::new(coll), Box::new(at))
}

// ── The §4.1 probe pair ───────────────────────────────

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
fn phase140_oob_read_never_verifies() {
    let spec = one(OOB_SPEC);
    let body = idx(v("xs"), len(v("xs")));
    // Non-rescue evidence, part 1: the read declines in
    // BOTH encodability predicates — no `select` term is
    // produced for it anywhere, so there is nothing for a
    // totalization to rescue. The index is `len(xs)`
    // itself: the guard it would need (len(xs) < len(xs))
    // is unsatisfiable, so no hypothesis set discharges it.
    let (real, pure) = enc_pair(&spec, &body);
    println!("P140|probe|oob_read|encodable={real}|mirror={pure}");
    assert!(!real && !pure, "the out-of-range read declines in both encoders");
    // Part 2: the verdict is Inconclusive — not Verified
    // (no fabricated crown) and not Refuted (the row
    // declines too; it was not totalized into something
    // refutable either).
    let status = verdict(&spec, body);
    println!("P140|probe|oob_read|verify={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase140_oob_read_declines_under_the_guarded_spec_too() {
    // The same body judged against last_read's spec: the
    // require (len(xs) >= 1) guards the LAST element, not
    // the one past it. The body still declines and the
    // verdict is still Inconclusive — the guard set is
    // exactly the spec's hypotheses, never widened.
    let spec = one(LAST_SPEC);
    let body = idx(v("xs"), len(v("xs")));
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P140|probe|oob_under_last|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(!real && !pure);
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase140_oob_row_declines_against_a_constant_body() {
    // The ROW's read is the unguarded one here: a body
    // that plainly encodes (a constant) still cannot be
    // judged, because `output == xs[len(xs)]` declines on
    // the constraint side. Inconclusive — the row was not
    // totalized into a refutable (or provable) claim.
    let spec = one(OOB_SPEC);
    let status = verdict(&spec, i(0));
    println!("P140|probe|oob_row_const_body|verify={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase140_last_read_verifies() {
    // W2's exit standing (§4.1): the guarded sibling
    // Verifies. The require discharges both the body's
    // read and the row's read at `len(xs) - 1`.
    let spec = one(LAST_SPEC);
    let body = idx(v("xs"), bin(IRBinOp::Sub, len(v("xs")), i(1)));
    let (real, pure) = enc_pair(&spec, &body);
    println!("P140|probe|last_read|encodable={real}|mirror={pure}");
    assert!(real && pure, "the guarded read encodes on both sides");
    let status = verdict(&spec, body);
    println!("P140|probe|last_read|verify={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase140_last_read_teeth_and_guard_absence() {
    let spec = one(LAST_SPEC);
    // Teeth: the FIRST element is a guarded read too (0
    // is in range under len >= 1), it encodes — and it is
    // genuinely Refuted against the last-element row: the
    // equality of the two selects is a real claim with a
    // real countermodel, not a vacuous pass.
    let first = idx(v("xs"), i(0));
    let (real, pure) = enc_pair(&spec, &first);
    let status = verdict(&spec, first);
    println!("P140|probe|last_read_first_elem|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(real && pure);
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // Guard absence: the identical spec minus its require
    // — the read's shape is unchanged, only the guard is
    // gone, and the verdict falls to Inconclusive. The
    // guard, not the read, is what W2 added.
    let bare = one(
        r#"Probe { spec last_read_bare {
            input: xs: List<Int>
            output: Int
            constraint: output == xs[len(xs) - 1]
            test { input: [10, 20]; expect: 20 }
        } }"#,
    );
    let body = idx(v("xs"), bin(IRBinOp::Sub, len(v("xs")), i(1)));
    let (real, pure) = enc_pair(&bare, &body);
    let status = verdict(&bare, body);
    println!("P140|probe|last_read_bare|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(!real && !pure);
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Guarded index specimens ───────────────────────────

#[test]
fn phase140_require_guarded_param_index() {
    // The P137 shape, at its W2 standing: requires
    // `i >= 0` and `i < len(xs)` are the row's own guard;
    // body and row both read under them.
    let spec = one(
        r#"Probe { spec guarded_index {
            input: xs: List<Int>, i: Int
            output: Int
            require: i >= 0
            require: i < len(xs)
            constraint: output == xs[i]
            test { input: ([10, 20], 1); expect: 20 }
        } }"#,
    );
    let body = idx(v("xs"), v("i"));
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P140|index|require_guarded|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(real && pure);
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The same spec minus its requires: the read is
    // unguarded and declines on both sides.
    let bare = one(
        r#"Probe { spec unguarded_index {
            input: xs: List<Int>, i: Int
            output: Int
            constraint: output == xs[i]
            test { input: ([10, 20], 1); expect: 20 }
        } }"#,
    );
    let body = idx(v("xs"), v("i"));
    let (real, pure) = enc_pair(&bare, &body);
    let status = verdict(&bare, body);
    println!("P140|index|unguarded|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(!real && !pure);
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase140_statement_route_reads_under_the_same_guards() {
    // Guard sourcing on the statement side: the body is a
    // statement body (Alloc + Return), so the expression
    // route declines it and the statement route judges it.
    // The statement evaluation sources its guards from the
    // same require terms (its context carries them), so
    // the guarded read Verifies — while the encodability
    // predicates, which are expression-route instruments,
    // both decline the statement shape itself, as they
    // always have.
    let spec = one(
        r#"Probe { spec guarded_index {
            input: xs: List<Int>, i: Int
            output: Int
            require: i >= 0
            require: i < len(xs)
            constraint: output == xs[i]
            test { input: ([10, 20], 1); expect: 20 }
        } }"#,
    );
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "t".into(),
            ty: TypeRef::Int,
            initial: Box::new(idx(v("xs"), v("i"))),
        },
        IRNode::Return(Box::new(v("t"))),
    ]);
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P140|index|stmt_route|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(!real && !pure, "statement shapes stay outside both predicates");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // And the unguarded statement read declines in the
    // statement route too: judged against oob_read's
    // spec, the Alloc body reading xs[len(xs)] produces
    // no statement outcome, and the verdict is
    // Inconclusive — the discipline is the encoder's, not
    // the route's.
    let oob = one(OOB_SPEC);
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "t".into(),
            ty: TypeRef::Int,
            initial: Box::new(idx(v("xs"), len(v("xs")))),
        },
        IRNode::Return(Box::new(v("t"))),
    ]);
    let status = verdict(&oob, body);
    println!("P140|index|stmt_route_oob|verify={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase140_antecedent_guards_the_rows_read() {
    // Constraint-side guard sourcing beyond requires: the
    // row's own antecedent. The body is the parameter
    // itself (no read on the body side); the row's read
    // sits in the consequent, guarded by the antecedent
    // that states its in-range fact. (The equality rows
    // of list_take/list_drop are this shape's shipped
    // kin — §2.4 under an antecedent.)
    let spec = one(
        r#"Probe { spec antecedent_read {
            input: xs: List<Int>, i: Int, v: Int
            output: Int
            constraint: (i >= 0 && i < len(xs)) implies (xs[i] == v || output == 0)
            test { input: ([10, 20], 1, 20); expect: 0 }
        } }"#,
    );
    // Body 0 satisfies the row's second disjunct whenever
    // the antecedent holds: Verified.
    let status = verdict(&spec, i(0));
    println!("P140|index|antecedent|verify={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // Teeth: body 1 — under the antecedent with v != xs[i]
    // the row fails, a genuine countermodel: Refuted.
    let status = verdict(&spec, i(1));
    println!("P140|index|antecedent_teeth|verify={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── The equality arm (§2.4) ───────────────────────────

#[test]
fn phase140_equality_rows_under_antecedents() {
    // list_take's row 216 / list_drop's row 241 shape,
    // distilled: `hypothesis implies output == xs`, body
    // the parameter. The equality arm needs no index
    // guard; the antecedent is the row's own business.
    let take = one(
        r#"Probe { spec take_eq {
            input: xs: List<Int>, n: Int
            output: List<Int>
            constraint: n >= len(xs) implies output == xs
            test { input: ([1, 2], 5); expect: [1, 2] }
        } }"#,
    );
    let (real, pure) = enc_pair(&take, &v("xs"));
    let status = verdict(&take, v("xs"));
    println!("P140|eq|take_shape|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(real && pure);
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // Teeth: an appended output is a DIFFERENT list under
    // the antecedent (its length is len + 1) — the §2.4
    // form's length conjunct refutes it genuinely.
    let status = verdict(&take, bin(IRBinOp::Append, v("xs"), i(0)));
    println!("P140|eq|take_shape_teeth|verify={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    let drop = one(
        r#"Probe { spec drop_eq {
            input: xs: List<Int>, n: Int
            output: List<Int>
            constraint: n <= 0 implies output == xs
            test { input: ([1, 2], 0); expect: [1, 2] }
        } }"#,
    );
    let status = verdict(&drop, v("xs"));
    println!("P140|eq|drop_shape|verify={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase140_equality_has_teeth_and_neq_is_its_negation() {
    // Two free pairs are NOT provably equal: the pointwise
    // form has a countermodel, so the wrong body is
    // Refuted — the form is a real semantics, not a
    // reflexivity rubber stamp.
    let spec = one(
        r#"Probe { spec pair_eq {
            input: xs: List<Int>, ys: List<Int>
            output: Bool
            constraint: output == (xs == ys)
            test { input: ([1], [1]); expect: true }
        } }"#,
    );
    let body = bin(IRBinOp::Eq, v("xs"), v("ys"));
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P140|eq|pair_pair|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(real && pure);
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The swapped body (xs == xs) against the same row:
    // Refuted — xs == ys is not a tautology.
    let status = verdict(&spec, bin(IRBinOp::Eq, v("xs"), v("xs")));
    println!("P140|eq|pair_pair_teeth|verify={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // Neq is the negation of the same form, no separate
    // semantics: xs != xs is false, provably.
    let neq = one(
        r#"Probe { spec pair_neq {
            input: xs: List<Int>, ys: List<Int>
            output: Bool
            constraint: output == (xs != ys)
            test { input: ([1], [2]); expect: true }
        } }"#,
    );
    let status = verdict(&neq, bin(IRBinOp::Neq, v("xs"), v("ys")));
    println!("P140|eq|pair_neq|verify={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
    // Teeth for Neq: the always-false body (xs != xs)
    // against the xs != ys row is Refuted.
    let status = verdict(&neq, bin(IRBinOp::Neq, v("xs"), v("xs")));
    println!("P140|eq|pair_neq_teeth|verify={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase140_equality_crosses_the_bridge_pointwise() {
    // Pair vs concrete list, through the §2.5 bridge into
    // the §2.4 form. This specimen is also the R1
    // evidence: the antecedent pins the parameter's
    // elements at 0 and 1 (guarded reads under
    // `require: len(xs) == 2`) and the row concludes
    // `output == [1, 2]`. Under RAW array equality this
    // could not Verify — the parameter array's
    // beyond-length content is unconstrained, so an
    // extensional countermodel exists. Under the frozen
    // pointwise form it Verifies: beyond-length content
    // is unobservable, exactly as §2.3/§2.4 freeze it.
    let spec = one(
        r#"Probe { spec cross_eq {
            input: xs: List<Int>
            output: List<Int>
            require: len(xs) == 2
            constraint: xs[0] == 1 && xs[1] == 2 implies output == [1, 2]
            test { input: [1, 2]; expect: [1, 2] }
        } }"#,
    );
    let (real, pure) = enc_pair(&spec, &v("xs"));
    let status = verdict(&spec, v("xs"));
    println!("P140|eq|crossing|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(real && pure);
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // Teeth: pin only the first element and the row is
    // genuinely refutable at index 1 — the pointwise
    // range is [0, len), all of it, not a prefix sample.
    let weak = one(
        r#"Probe { spec cross_eq_weak {
            input: xs: List<Int>
            output: List<Int>
            require: len(xs) == 2
            constraint: xs[0] == 1 implies output == [1, 2]
            test { input: [1, 2]; expect: [1, 2] }
        } }"#,
    );
    let status = verdict(&weak, v("xs"));
    println!("P140|eq|crossing_teeth|verify={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Concat: §2.6's named answer ───────────────────────

#[test]
fn phase140_concat_binds_as_its_exact_length() {
    // Concat of two pairs declines elementwise and binds
    // as the length-only form with the exact sum length:
    // length rows over a concatenation stand (§2.6).
    let spec = one(
        r#"Probe { spec concat_len {
            input: xs: List<Int>, ys: List<Int>
            output: Int
            constraint: output == len(xs) + len(ys)
            test { input: ([1], [2, 3]); expect: 3 }
        } }"#,
    );
    let body = len(bin(IRBinOp::Concat, v("xs"), v("ys")));
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P140|concat|len_row|encodable={real}|mirror={pure}|verify={status:?}");
    assert!(real && pure);
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // Teeth: one summand short is a genuine refutation.
    let status = verdict(&spec, len(v("xs")));
    println!("P140|concat|len_row_teeth|verify={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── The mirror agreement battery ──────────────────────

#[test]
fn phase140_mirror_agreement_battery() {
    // Every W2 shape, real == pure, in both directions
    // (§2.2). The unguarded spec first: with no requires,
    // every pair read declines and every guard-free shape
    // (equality, Concat-as-length) encodes.
    let unguarded = one(
        r#"Probe { spec shapes {
            input: xs: List<Int>, ys: List<Int>, cs: List<Char>, i: Int, b: Bool
            output: Int
            constraint: output == 0
            test { input: ([1], [2], ['a'], 0, true); expect: 0 }
        } }"#,
    );
    let shapes: Vec<(&str, IRNode, bool)> = vec![
        ("idx_literal_into_param", idx(v("xs"), i(0)), false),
        ("idx_var", idx(v("xs"), v("i")), false),
        ("idx_len", idx(v("xs"), len(v("xs"))), false),
        (
            "idx_len_minus_1_unguarded",
            idx(v("xs"), bin(IRBinOp::Sub, len(v("xs")), i(1))),
            false,
        ),
        ("eq_pair_pair", bin(IRBinOp::Eq, v("xs"), v("ys")), true),
        ("neq_pair_pair", bin(IRBinOp::Neq, v("xs"), v("ys")), true),
        ("eq_pair_concrete", bin(IRBinOp::Eq, v("xs"), list(vec![i(1), i(2)])), true),
        ("eq_concrete_pair", bin(IRBinOp::Eq, list(vec![i(1), i(2)]), v("xs")), true),
        ("neq_pair_concrete", bin(IRBinOp::Neq, v("xs"), list(vec![i(1), i(2)])), true),
        (
            "eq_char_list_crossing_refused",
            bin(IRBinOp::Eq, v("xs"), list(vec![IRNode::Literal(IRLiteral::Char('a'))])),
            false,
        ),
        ("eq_pair_vs_length_only", bin(IRBinOp::Eq, v("xs"), v("cs")), false),
        (
            "eq_concrete_concrete_unmoved",
            bin(IRBinOp::Eq, list(vec![i(1), i(2)]), list(vec![i(3), i(4)])),
            false,
        ),
        ("concat_pair_pair", bin(IRBinOp::Concat, v("xs"), v("ys")), true),
        ("len_of_concat", len(bin(IRBinOp::Concat, v("xs"), v("ys"))), true),
        (
            "idx_into_concat_declines",
            idx(bin(IRBinOp::Concat, v("xs"), v("ys")), i(0)),
            false,
        ),
        (
            "eq_concat_vs_pair_declines",
            bin(IRBinOp::Eq, bin(IRBinOp::Concat, v("xs"), v("ys")), v("xs")),
            false,
        ),
        (
            "eq_appended_pair",
            bin(IRBinOp::Eq, bin(IRBinOp::Append, v("xs"), i(1)), v("ys")),
            true,
        ),
        (
            // The appended element's position is NOT one
            // of §2.3's guard sources: `len(xs)` into the
            // appended pair needs `len(xs) >= 0`, which no
            // hypothesis here states. It declines until a
            // hypothesis states it (see the append spec
            // below, where one does).
            "idx_appended_position_unguarded",
            idx(bin(IRBinOp::Append, v("xs"), i(1)), len(v("xs"))),
            false,
        ),
    ];
    for (name, body, expected) in &shapes {
        let (real, pure) = enc_pair(&unguarded, body);
        println!("P140|mirror|unguarded|{name}|real={real}|pure={pure}|expect={expected}");
        assert_eq!((real, pure), (*expected, *expected), "{name}");
    }

    // The guarded spec: requires state the in-range facts
    // for xs only. Reads the facts cover encode; reads
    // they do not cover (a different list, one past the
    // covered index, the length itself) still decline —
    // the guard set is exact, on both sides.
    let guarded = one(
        r#"Probe { spec shapes_guarded {
            input: xs: List<Int>, ys: List<Int>, i: Int
            output: Int
            require: i >= 0
            require: i < len(xs)
            require: len(xs) >= 1
            constraint: output == 0
            test { input: ([1], [2], 0); expect: 0 }
        } }"#,
    );
    let gshapes: Vec<(&str, IRNode, bool)> = vec![
        ("idx_var_guarded", idx(v("xs"), v("i")), true),
        (
            "idx_len_minus_1_guarded",
            idx(v("xs"), bin(IRBinOp::Sub, len(v("xs")), i(1))),
            true,
        ),
        ("idx_zero_guarded", idx(v("xs"), i(0)), true),
        ("idx_len_still_oob", idx(v("xs"), len(v("xs"))), false),
        ("idx_other_list_unguarded", idx(v("ys"), v("i")), false),
        (
            "idx_i_plus_1_not_covered",
            idx(v("xs"), bin(IRBinOp::Add, v("i"), i(1))),
            false,
        ),
        (
            "idx_by_other_length",
            idx(v("xs"), bin(IRBinOp::Sub, len(v("ys")), i(1))),
            false,
        ),
    ];
    for (name, body, expected) in &gshapes {
        let (real, pure) = enc_pair(&guarded, body);
        println!("P140|mirror|guarded|{name}|real={real}|pure={pure}|expect={expected}");
        assert_eq!((real, pure), (*expected, *expected), "{name}");
    }

    // The append spec: with `len(xs) >= 0` stated, the
    // appended position IS in range and the read encodes
    // on both sides; one past it still declines.
    let append_spec = one(
        r#"Probe { spec shapes_append {
            input: xs: List<Int>
            output: Int
            require: len(xs) >= 0
            constraint: output == 0
            test { input: [1]; expect: 0 }
        } }"#,
    );
    let ashapes: Vec<(&str, IRNode, bool)> = vec![
        (
            "idx_appended_position_with_len_fact",
            idx(bin(IRBinOp::Append, v("xs"), i(1)), len(v("xs"))),
            true,
        ),
        (
            "idx_past_appended",
            idx(
                bin(IRBinOp::Append, v("xs"), i(1)),
                bin(IRBinOp::Add, len(v("xs")), i(1)),
            ),
            false,
        ),
    ];
    for (name, body, expected) in &ashapes {
        let (real, pure) = enc_pair(&append_spec, body);
        println!("P140|mirror|append|{name}|real={real}|pure={pure}|expect={expected}");
        assert_eq!((real, pure), (*expected, *expected), "{name}");
    }
}
