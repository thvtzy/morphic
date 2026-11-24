//! Phase 139 (W1) — the paired binding: a `List<Int>`
//! value binds as the standing length term plus a fresh
//! `Array<Int, Int>` term (LIST_ENCODER_SCOPE §2.1), the
//! mirror learns the shape in lockstep (§2.2), and the
//! statement layer carries the pair as a pair.
//!
//! W1 discharges NOTHING new by itself: no index term
//! exists yet (W2), no equality form (W2), no quantifier
//! arm (W3). The pins below therefore split into
//! non-movers (they pass on the parent tree and must keep
//! passing — every length-level standing, the non-Int
//! sorts' exact `ListLen` standings per §9 R9, the
//! concrete-list machinery, the statement-mode loop
//! standings) and the phase's own new shapes (the pair's
//! mirror agreement, the §2.5 literal crossing in merges,
//! the Append store form's item discipline), which fail
//! on the parent tree.
//!
//! The sweep-identity pin for this phase is the standing
//! instrument, `phase133_corpus_verdict_drift` (201
//! specs; the counts and named buckets it pins are the
//! before/after identity this phase must preserve), and
//! the 17/17 Tier-2 Inconclusive sweep is
//! `phase137_all_tier2_entries_stand_inconclusive`
//! (renamed `phase137_tier2_standings` at Phase 141,
//! when W3 moved eight of its entries to Verified).
//! Neither is duplicated here; both must stay green.
//!
//! Every test prints a `P139|...` line with the measured
//! standing; assertions pin exactly what the line
//! reports. In the no-Z3 build the verdicts are the
//! honest subset (Inconclusive throughout) — the pins
//! branch on the feature exactly as the Phase 133/137
//! probes do. The encodability pins are configuration-
//! independent: in the no-Z3 build the real predicate IS
//! the pure one, and the pure answers are the same
//! Some/None structure.

use morphic::spec::ast::{FunctionSpec, TypeRef};
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, IRPattern, Provenance, ScoreBreakdown,
    SpecIndex,
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
        id: 139,
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

// ── The pair binds; length standings unmoved ──────────

#[test]
fn phase139_pair_binds_and_length_standings_stand() {
    // The P137 control, restated at the pair: a length-
    // only contract over a List<Int> parameter Verifies —
    // the length half of the pair is the standing term.
    let spec = one(
        r#"Probe { spec len_only {
            input: xs: List<Int>
            output: Int
            constraint: output == len(xs)
            test { input: [1, 2]; expect: 2 }
        } }"#,
    );
    let (real, pure) = enc_pair(&spec, &len(v("xs")));
    let status = verdict(&spec, len(v("xs")));
    println!("P139|bind|len_only|encodable={real}/{pure}|verify={status:?}");
    assert!(real && pure);
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // list_is_empty's shape (its Tier-1 crown is a length
    // standing): the faithful body Verifies, a wrong one
    // is Refuted — the pair did not blunt the teeth.
    let spec = one(
        r#"Probe { spec empty_check {
            input: xs: List<Int>
            output: Bool
            constraint: output == (len(xs) == 0)
            test { input: []; expect: true }
        } }"#,
    );
    let faithful = bin(IRBinOp::Eq, len(v("xs")), i(0));
    let status = verdict(&spec, faithful);
    println!("P139|bind|empty_check={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
    let wrong = bin(IRBinOp::Eq, len(v("xs")), i(1));
    let status = verdict(&spec, wrong);
    println!("P139|bind|empty_check_wrong={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // Two List<Int> parameters: their length terms are
    // independent standing terms (neither pair's array is
    // observed; the lengths compose as they always did).
    let spec = one(
        r#"Probe { spec two_lens {
            input: xs: List<Int>, ys: List<Int>
            output: Int
            constraint: output == len(xs) + len(ys)
            test { input: ([1], [2, 3]); expect: 3 }
        } }"#,
    );
    let body = bin(IRBinOp::Add, len(v("xs")), len(v("ys")));
    let status = verdict(&spec, body);
    println!("P139|bind|two_lens={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The pair binds through the standing recursion: an
    // Option<List<Int>> parameter's payload is the pair,
    // and `len` of the unwrapped payload reads its length
    // term — the Phase 76 Option standing, unmoved.
    let spec = one(
        r#"Probe { spec opt_list_len {
            input: o: Option<List<Int>>
            output: Int
            constraint: o == None() || output == len(o.unwrap())
            constraint: o != None() || output == 0
            test { input: None(); expect: 0 }
        } }"#,
    );
    let body = IRNode::Match {
        scrutinee: Box::new(v("o")),
        cases: vec![
            (
                IRPattern::Constructor("Some".into(), vec![IRPattern::Variable("w".into())]),
                len(v("w")),
            ),
            (IRPattern::Constructor("None".into(), vec![]), i(0)),
        ],
    };
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P139|bind|opt_list_len|encodable={real}/{pure}|verify={status:?}");
    assert!(real && pure);
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── R9: non-Int element sorts keep ListLen exactly ────

#[test]
fn phase139_non_int_element_sorts_keep_listlen() {
    // A List<Char> parameter keeps the length-only
    // binding AND its Append totalization: appending a
    // Char grows the length by exactly one and Verifies.
    // (Had the pair engaged here, the Char item could not
    // store into an Int array and the body would decline
    // — Verified is the R9 standing, stated as a verdict.)
    let spec = one(
        r#"Probe { spec char_push_len {
            input: cs: List<Char>
            output: Int
            constraint: output == len(cs) + 1
            test { input: ['a']; expect: 2 }
        } }"#,
    );
    let grown = len(bin(
        IRBinOp::Append,
        v("cs"),
        IRNode::Literal(IRLiteral::Char('a')),
    ));
    let status = verdict(&spec, grown);
    println!("P139|r9|char_append_len={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
    let status = verdict(&spec, len(v("cs")));
    println!("P139|r9|char_no_append={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // Element access on a List<Char> declines exactly as
    // today (the Phase 133 side-condition class stays
    // out): unencodable on both sides of the mirror.
    let spec = one(
        r#"Probe { spec char_index {
            input: cs: List<Char>
            output: Char
            require: len(cs) >= 1
            constraint: output == cs[0]
            test { input: ['a']; expect: 'a' }
        } }"#,
    );
    let body = IRNode::Index(Box::new(v("cs")), Box::new(i(0)));
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P139|r9|char_index|encodable={real}/{pure}|verify={status:?}");
    assert!(!real && !pure);
    assert_eq!(status, VerificationStatus::Inconclusive);

    // List<String>: the length standing, nothing more.
    let spec = one(
        r#"Probe { spec string_list_len {
            input: ss: List<String>
            output: Int
            constraint: output == len(ss)
            test { input: ["a"]; expect: 1 }
        } }"#,
    );
    let status = verdict(&spec, len(v("ss")));
    println!("P139|r9|string_list_len={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // A nested list: List<List<Int>> keeps ListLen —
    // its length Verifies, its elements decline.
    let spec = one(
        r#"Probe { spec nested_len {
            input: xss: List<List<Int>>
            output: Int
            constraint: output == len(xss)
            test { input: [[1]]; expect: 1 }
        } }"#,
    );
    let status = verdict(&spec, len(v("xss")));
    println!("P139|r9|nested_len={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
    let spec = one(
        r#"Probe { spec nested_index {
            input: xss: List<List<Int>>
            output: List<Int>
            constraint: output == xss[0]
            test { input: [[1]]; expect: [1] }
        } }"#,
    );
    let body = IRNode::Index(Box::new(v("xss")), Box::new(i(0)));
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P139|r9|nested_index|encodable={real}/{pure}|verify={status:?}");
    assert!(!real && !pure);
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Append over a pair: the §2.6 store form ───────────

#[test]
fn phase139_append_over_a_pair_is_the_store_form() {
    // The Phase 91 push_len standing, restated as the
    // pair's: the grown length is exact (the store form's
    // length half is the totalization's, term for term),
    // and the teeth are unblunted.
    let spec = one(
        r#"Probe { spec push_len {
            input: xs: List<Int>
            output: Int
            constraint: output == len(xs) + 1
            test { input: [1]; expect: 2 }
        } }"#,
    );
    let grown = len(bin(IRBinOp::Append, v("xs"), i(7)));
    let status = verdict(&spec, grown);
    println!("P139|append|grown={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
    let twice = len(bin(
        IRBinOp::Append,
        bin(IRBinOp::Append, v("xs"), i(7)),
        i(8),
    ));
    let status = verdict(&spec, twice);
    println!("P139|append|twice={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The store form's item discipline: the appended
    // element must be an Int term — a Bool item cannot
    // store into an Array<Int, Int>, so the shape
    // declines on both sides of the mirror. (Under the
    // old ListLen totalization any item was accepted;
    // that acceptance was the elements-unmodelled world,
    // and the pair does not inherit it.)
    let body = bin(IRBinOp::Append, v("xs"), IRNode::Literal(IRLiteral::Bool(true)));
    let (real, pure) = enc_pair(&spec, &body);
    println!("P139|append|bool_item|encodable={real}/{pure}");
    assert!(!real && !pure, "a non-Int append item declines on both sides");
}

// ── The mirror-agreement battery ( §2.2) ──────────────

#[test]
fn phase139_mirror_agrees_on_the_binding_shapes() {
    // The Phase 133 instrument at the new shapes: the
    // real and pure encodability predicates agree in
    // both directions on every shape W1 introduces, and
    // the expected answers are pinned outright.
    let spec = one(
        r#"Probe { spec shapes {
            input: xs: List<Int>, ys: List<Int>, cs: List<Char>, b: Bool
            output: Int
            constraint: output == 0
            test { input: ([1], [2], ['a'], true); expect: 0 }
        } }"#,
    );
    let ite = |c: IRNode, a: IRNode, b: IRNode| IRNode::If {
        condition: Box::new(c),
        then_branch: Box::new(a),
        else_branch: Box::new(b),
    };
    let cases: Vec<(IRNode, bool, &str)> = vec![
        (len(v("xs")), true, "len of a pair"),
        (bin(IRBinOp::Append, v("xs"), i(7)), true, "append an Int to a pair"),
        (
            bin(IRBinOp::Append, v("xs"), IRNode::Literal(IRLiteral::Bool(true))),
            false,
            "append a Bool to a pair",
        ),
        (
            IRNode::Index(Box::new(v("xs")), Box::new(i(0))),
            false,
            // W2's arm is built (Phase 140); the shape
            // still declines HERE because this battery's
            // spec states no guard — a literal index into
            // a parameter pair is in range only under a
            // hypothesis (§2.3). The guarded standings are
            // Phase 140's own battery.
            "index a pair, unguarded in this spec (§2.3 decline)",
        ),
        (
            bin(IRBinOp::Eq, v("xs"), v("ys")),
            // Standing updated at Phase 140 (W2): the
            // equality arm is built, in §2.4's pointwise
            // form — the shape encodes on both sides, as
            // this battery's agreement discipline requires.
            // At W1 it declined, as the label then said.
            true,
            "pair equality (W2's form, built at Phase 140)",
        ),
        (
            bin(IRBinOp::Concat, v("xs"), v("ys")),
            // Standing updated at Phase 140 (W2): §2.6's
            // named answer — Concat declines ELEMENTWISE
            // and binds as its exact length (the length-
            // only form), so the shape encodes on both
            // sides as a ListLen; every element row over
            // it still declines (Phase 140's battery pins
            // that half). At W1 the value declined whole.
            true,
            "concat of pairs (§2.6: binds as its exact length since Phase 140)",
        ),
        (ite(v("b"), v("xs"), v("ys")), true, "merge two pairs"),
        (
            ite(v("b"), v("xs"), list(vec![i(1), i(2)])),
            true,
            "merge a pair with a literal (the §2.5 crossing)",
        ),
        (
            ite(v("b"), list(vec![i(1), i(2)]), v("xs")),
            true,
            "merge a literal with a pair",
        ),
        (
            ite(v("b"), v("xs"), list(vec![IRNode::Literal(IRLiteral::Char('a'))])),
            false,
            "merge a pair with a Char literal (sort mismatch declines)",
        ),
        (
            ite(v("b"), v("xs"), v("cs")),
            false,
            "merge a pair with a ListLen (different bindings decline)",
        ),
        (
            len(ite(v("b"), v("xs"), list(vec![i(1), i(2)]))),
            true,
            "len of a crossed merge",
        ),
        (
            IRNode::Map {
                function: Box::new(IRNode::Lambda {
                    params: vec![("n".into(), TypeRef::Int)],
                    body: Box::new(bin(IRBinOp::Add, v("n"), i(1))),
                }),
                over: Box::new(v("xs")),
            },
            false,
            "Map over a pair (W5's, not yet built)",
        ),
        (len(v("cs")), true, "len of a ListLen stands"),
        (
            bin(IRBinOp::Append, v("cs"), IRNode::Literal(IRLiteral::Char('a'))),
            true,
            "append to a ListLen stands (any item, as today)",
        ),
    ];
    for (body, want, label) in &cases {
        let (real, pure) = enc_pair(&spec, body);
        println!("P139|mirror|{label}|real={real}|pure={pure}");
        assert_eq!(real, pure, "mirror disagreement on {label}");
        assert_eq!(real, *want, "unexpected encodability on {label}");
    }
}

// ── Concrete lists + the §2.5 crossing ────────────────

#[test]
fn phase139_concrete_lists_and_the_crossing() {
    // The audit's concrete control, unbroken in the
    // paired world: a body building a concrete list and
    // indexing it encodes and Verifies (Sym::List is not
    // replaced — §2.5).
    let spec = one(
        r#"Probe { spec probe_concrete {
            input: x: Int
            output: Int
            constraint: output == x
            test { input: 5; expect: 5 }
        } }"#,
    );
    let body = IRNode::Index(Box::new(list(vec![v("x"), i(9)])), Box::new(i(0)));
    let (real, pure) = enc_pair(&spec, &body);
    let status = verdict(&spec, body);
    println!("P139|bridge|concrete|encodable={real}/{pure}|verify={status:?}");
    assert!(real && pure);
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The crossing at work: a body merging the parameter
    // with a literal, observed only through its length,
    // Verifies through the paired form — the literal
    // crosses as the store-chain over the constant base
    // (§2.5), so the merged value IS a pair and its
    // length is the ite of the two lengths.
    let spec = one(
        r#"Probe { spec pick_len {
            input: xs: List<Int>, b: Bool
            output: Int
            constraint: b == false || output == len(xs)
            constraint: b == true || output == 2
            test { input: ([9], true); expect: 1 }
        } }"#,
    );
    let ite = |a: IRNode, b: IRNode| IRNode::If {
        condition: Box::new(v("b")),
        then_branch: Box::new(a),
        else_branch: Box::new(b),
    };
    let status = verdict(&spec, len(ite(v("xs"), list(vec![i(1), i(2)]))));
    println!("P139|bridge|pick_len={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The other branch order, its rows restated.
    let spec = one(
        r#"Probe { spec pick_len_rev {
            input: xs: List<Int>, b: Bool
            output: Int
            constraint: b == false || output == 2
            constraint: b == true || output == len(xs)
            test { input: ([9], false); expect: 1 }
        } }"#,
    );
    let status = verdict(&spec, len(ite(list(vec![i(1), i(2)]), v("xs"))));
    println!("P139|bridge|pick_len_rev={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // Teeth: the crossed literal's length is EXACT — a
    // three-element literal against the two-element rows
    // is Refuted, not absorbed.
    let spec = one(
        r#"Probe { spec pick_len {
            input: xs: List<Int>, b: Bool
            output: Int
            constraint: b == false || output == len(xs)
            constraint: b == true || output == 2
            test { input: ([9], true); expect: 1 }
        } }"#,
    );
    let status = verdict(&spec, len(ite(v("xs"), list(vec![i(1), i(2), i(3)]))));
    println!("P139|bridge|pick_len_teeth={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

// ── Statement mode carries the pair as a pair ─────────

#[test]
fn phase139_statement_mode_carries_the_pair() {
    // A loop that APPENDS to a List<Int> local seeded
    // from the parameter: the invariant route havocs the
    // written local at the loop head — the havoc of a
    // pair is a fresh pair, the body's Append is the
    // store form through the statement layer's own append
    // path, and the length facts the invariant states
    // discharge exactly as they did when the binding was
    // ListLen. This specimen Verifies on the parent tree
    // (ListLen throughout) and must stand.
    let spec = one(
        r#"Probe { spec grow_count {
            input: xs: List<Int>
            output: Int
            require: len(xs) >= 0
            constraint: output == len(xs)
            test { input: [1, 2]; expect: 2 }
        } }"#,
    );
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "ys".into(),
            ty: TypeRef::List(Box::new(TypeRef::Int)),
            initial: Box::new(v("xs")),
        },
        IRNode::Alloc {
            name: "i".into(),
            ty: TypeRef::Int,
            initial: Box::new(i(0)),
        },
        IRNode::While {
            condition: Box::new(bin(IRBinOp::Lt, v("i"), len(v("xs")))),
            invariant: Some("len(ys) == len(xs) + i; i <= len(xs)".into()),
            body: Box::new(IRNode::Block(vec![
                IRNode::Assign {
                    target: Box::new(v("ys")),
                    value: Box::new(bin(IRBinOp::Append, v("ys"), i(0))),
                },
                IRNode::Assign {
                    target: Box::new(v("i")),
                    value: Box::new(bin(IRBinOp::Add, v("i"), i(1))),
                },
            ])),
        },
        IRNode::Return(Box::new(bin(IRBinOp::Sub, len(v("ys")), v("i")))),
    ]);
    let status = verdict(&spec, body);
    println!("P139|stmt|grow_count={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // A statement-mode branch merge over pairs: the two
    // branches assign different pair values (one the
    // parameter, one its store-form growth) and the join
    // merges them as a pair — lengths by the standing ite,
    // arrays by the same ite (§2.11) — and the rows read
    // the merged length exactly.
    let spec = one(
        r#"Probe { spec pick_stmt {
            input: xs: List<Int>, b: Bool
            output: Int
            constraint: b == false || output == len(xs) + 1
            constraint: b == true || output == len(xs)
            test { input: ([1], true); expect: 2 }
        } }"#,
    );
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "ys".into(),
            ty: TypeRef::List(Box::new(TypeRef::Int)),
            initial: Box::new(v("xs")),
        },
        IRNode::If {
            condition: Box::new(v("b")),
            then_branch: Box::new(IRNode::Assign {
                target: Box::new(v("ys")),
                value: Box::new(bin(IRBinOp::Append, v("ys"), i(3))),
            }),
            else_branch: Box::new(IRNode::Assign {
                target: Box::new(v("ys")),
                value: Box::new(v("xs")),
            }),
        },
        IRNode::Return(Box::new(len(v("ys")))),
    ]);
    let status = verdict(&spec, body);
    println!("P139|stmt|pick_stmt={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The merge fast path: both branches assign the SAME
    // pair — `stmt_sym_identical` sees the pair as
    // identical (both terms node-identical) and the merge
    // is free, exactly the ListLen fast path's standing
    // (R2: an identity test, not an equality semantics).
    let spec = one(
        r#"Probe { spec same_stmt {
            input: xs: List<Int>, b: Bool
            output: Int
            constraint: output == len(xs)
            test { input: ([1], true); expect: 1 }
        } }"#,
    );
    let assign_xs = IRNode::Assign {
        target: Box::new(v("ys")),
        value: Box::new(v("xs")),
    };
    let body = IRNode::Block(vec![
        IRNode::Alloc {
            name: "ys".into(),
            ty: TypeRef::List(Box::new(TypeRef::Int)),
            initial: Box::new(v("xs")),
        },
        IRNode::If {
            condition: Box::new(v("b")),
            then_branch: Box::new(assign_xs.clone()),
            else_branch: Box::new(assign_xs),
        },
        IRNode::Return(Box::new(len(v("ys")))),
    ]);
    let status = verdict(&spec, body);
    println!("P139|stmt|same_stmt={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}
