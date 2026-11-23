//! Phase 133 (G4 D6a) — the Z3 uninterpreted-function arm.
//!
//! G4_SCOPE §4, frozen: a function-typed parameter binds as
//! a Z3 uninterpreted function of its declared signature;
//! application of that parameter encodes as UF application;
//! `encodable.rs` mirrors exactly this slice. The arm may
//! produce Verified ONLY on congruence — no extensionality,
//! no injectivity, no value at any point the spec does not
//! itself state. A row whose truth needs more is never
//! proven by the arm: it is decided only by a genuine
//! countermodel (a real function refuting it) or stays
//! Inconclusive — never a fabricated verdict.
//!
//! Signature slice (the arm's named boundary, measured
//! against the encoder's sort discipline): domain and
//! range sorts over Int-family / Bool / String — the sorts
//! whose symbolic values are plain Z3 terms carrying no
//! side conditions. Char is named OUT in both positions:
//! a UF-produced Char would owe a D5a range side condition
//! no collector asserts (a countermodel could refute
//! through a surrogate — the fabricated-countermodel
//! class), and a Char domain point is only as good as the
//! side conditions its provenance happens to carry. List /
//! Option / Result / Record / function ranges have no Z3
//! sort in this encoder at all. Signatures outside the
//! slice keep the standing binding (Opaque) and their
//! applications stay unencodable — Inconclusive, as today.
//!
//! Refutation discipline: a parameter is UNIVERSALLY
//! quantified — the spec claims its rows for every
//! function — so a UF countermodel (a finite point graph
//! plus an input) is a real function and a real refutation;
//! Sat verdicts through this arm stand. That is the exact
//! opposite of the `int_to_str` UF (Phase 100, D4a), which
//! stands for ONE specific function whose models need not
//! be the real conversion — hence its guarded check. The
//! two arms compose: a row mentioning both is still
//! scanned by `int_to_str_apps` and judged by the guarded
//! check, unchanged.
//!
//! Every test prints a `P133|...` line with the measured
//! standing; assertions pin exactly what the line reports.
//! In the no-Z3 build every verdict below is the honest
//! subset (Inconclusive throughout) — the pins branch on
//! the feature exactly as the Phase 127 probes do.

use std::collections::BTreeMap;
use std::path::PathBuf;

use morphic::spec::ast::FunctionSpec;
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, Provenance, ScoreBreakdown, SpecIndex,
};
use morphic::verify::encodable::body_is_encodable_pure;
use morphic::verify::verdict::VerificationStatus;
use morphic::verify::verifier::{self, Verifier};

fn try_checked(source: &str) -> Result<Vec<FunctionSpec>, String> {
    let module = parser::parse(source).map_err(|e| format!("parse: {e}"))?;
    let checked = typeck::check(module).map_err(|e| format!("check: {}", e.message))?;
    Ok(checked.spec.functions)
}

fn checked_spec(source: &str) -> FunctionSpec {
    try_checked(source)
        .expect("probe source typechecks")
        .into_iter()
        .next()
        .expect("probe source has one spec")
}

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 133,
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

fn var(name: &str) -> IRNode {
    IRNode::Var(name.into())
}

fn call(name: &str, args: Vec<IRNode>) -> IRNode {
    IRNode::Call {
        function: name.into(),
        args,
    }
}

fn verdict(spec: &FunctionSpec, body: IRNode) -> VerificationStatus {
    Verifier::new()
        .verify_one(&candidate(spec, body), spec)
        .status
}

/// The §3 type specimen, verbatim (G4_SCOPE §3).
const APPLY_TWICE: &str = r#"Probe { spec apply_twice {
    input: f: fn(Int) -> Int, x: Int
    output: Int
    constraint: output == f(f(x))
    test { input: (|n| n + 1, 40); expect: 42 }
    test { input: (|n| n * 2, 3); expect: 12 }
} }"#;

fn apply_twice_body() -> IRNode {
    call("f", vec![call("f", vec![var("x")])])
}

#[test]
fn phase133_apply_twice_verifies_under_the_uf_arm() {
    // §3's exit standing for this phase: the specimen
    // reaches Verified under the arm, and its faithful
    // body enters the encodable subset (the Phase 127
    // probe's `encodable=false` pin flips at its site).
    let spec = checked_spec(APPLY_TWICE);
    let encodable = verifier::body_is_encodable(&spec, &apply_twice_body());
    let status = verdict(&spec, apply_twice_body());
    println!("P133|apply_twice|encodable={encodable}|verify={status:?}");
    #[cfg(feature = "z3-support")]
    {
        assert!(encodable, "the apply body is encodable under the UF arm");
        assert_eq!(status, VerificationStatus::Verified);
    }
    #[cfg(not(feature = "z3-support"))]
    {
        assert_eq!(status, VerificationStatus::Inconclusive);
    }
}

#[test]
fn phase133_congruence_is_the_whole_proof() {
    // What the arm proves is congruence and nothing else:
    // equal arguments yield equal results, composed. A
    // single application against its own statement of the
    // row verifies (reflexivity through the UF)…
    let once = checked_spec(
        r#"Probe { spec apply_once {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            constraint: output == f(x)
            test { input: (|n| n + 1, 40); expect: 41 }
        } }"#,
    );
    let status = verdict(&once, call("f", vec![var("x")]));
    println!("P133|congruence|apply_once={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // …and an application under a `require:` row encodes
    // through the same arm: the row is assumable (Phase
    // 109's Declined no longer fires for it), so the
    // postcondition is judged on the stated domain. The
    // domain here pins f at the very point the body
    // applies — congruence closes it.
    let req = checked_spec(
        r#"Probe { spec apply_req_uf {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            require: f(x) == x + 1
            constraint: output == x + 1
            test { input: (|n| n + 1, 40); expect: 41 }
        } }"#,
    );
    let status = verdict(&req, call("f", vec![var("x")]));
    println!("P133|congruence|apply_req_uf={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase133_wrong_body_is_genuinely_refuted() {
    // The body applies f ONCE; the row demands the
    // composition. A countermodel is a real function
    // (f(0)=1, f(1)=2 refutes at x=0), so the refutation
    // stands as VerificationFailed — the parameter is
    // universally quantified, and this arm's Sat is never
    // degraded the way the specific-function `int_to_str`
    // UF's Sat is (see the module docs).
    let spec = checked_spec(APPLY_TWICE);
    let result = Verifier::new().verify_one(
        &candidate(&spec, call("f", vec![var("x")])),
        &spec,
    );
    println!(
        "P133|refutation|apply_once_body={:?}|counterexample={:?}",
        result.status,
        result
            .counterexample
            .as_ref()
            .map(|c| c.inputs.get("x").cloned())
    );
    #[cfg(feature = "z3-support")]
    assert_eq!(result.status, VerificationStatus::VerificationFailed);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(result.status, VerificationStatus::Inconclusive);
}

#[test]
fn phase133_beyond_congruence_stays_inconclusive() {
    // (a) A value at a point the spec does not state: the
    // row claims the body's result equals f at a FIXED
    // point for every x. That is false for some real
    // functions, true for none of the arm's business —
    // the honest outcomes are a genuine refutation or
    // Inconclusive, never Verified.
    let point = checked_spec(
        r#"Probe { spec apply_point {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            constraint: output == f(0)
            test { input: (|n| n + 1, 40); expect: 1 }
        } }"#,
    );
    let status = verdict(&point, call("f", vec![var("x")]));
    println!("P133|beyond|point_value={status:?}");
    assert_ne!(status, VerificationStatus::Verified);

    // (b) A signature outside the arm's slice: the range
    // is a List — no Z3 sort, the parameter keeps its
    // Opaque binding, the application stays unencodable.
    let list_ranged = checked_spec(
        r#"Probe { spec apply_list_ranged {
            input: f: fn(Int) -> List<Int>, x: Int
            output: List<Int>
            constraint: output == f(x)
            test { input: (|n| [n], 40); expect: [40] }
        } }"#,
    );
    let body = call("f", vec![var("x")]);
    let encodable = verifier::body_is_encodable(&list_ranged, &body);
    let status = verdict(&list_ranged, body);
    println!("P133|beyond|list_ranged|encodable={encodable}|verify={status:?}");
    assert!(!encodable, "an unmappable signature stays unencodable");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // (c) A Char signature: named out of the slice (the
    // module docs state why) — same standing as (b).
    let char_sig = checked_spec(
        r#"Probe { spec apply_char {
            input: f: fn(Char) -> Int, c: Char
            output: Int
            constraint: output == f(c)
            test { input: (|ch| 1, 'a'); expect: 1 }
        } }"#,
    );
    let body = call("f", vec![var("c")]);
    let encodable = verifier::body_is_encodable(&char_sig, &body);
    let status = verdict(&char_sig, body);
    println!("P133|beyond|char_sig|encodable={encodable}|verify={status:?}");
    assert!(!encodable, "a Char signature stays unencodable in this arm");
    assert_eq!(status, VerificationStatus::Inconclusive);

    // (d) A higher-order NODE body (Map applying the
    // parameter): §4 excludes Map/Filter/Fold encodings
    // outright (they are §3.9's) — the body stays outside
    // the encodable subset whatever the constraint says.
    let spec = checked_spec(APPLY_TWICE);
    let map_body = IRNode::Map {
        function: Box::new(var("f")),
        over: Box::new(IRNode::Literal(IRLiteral::Int(0))),
    };
    let encodable = verifier::body_is_encodable(&spec, &map_body);
    let status = verdict(&spec, map_body);
    println!("P133|beyond|map_body|encodable={encodable}|verify={status:?}");
    assert!(!encodable, "a Map body stays unencodable (§3.9's, not this arm's)");
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase133_guards_ignore_f_and_same_f() {
    // Guard 1 (§3): an UNUSED fn parameter never poisons —
    // binding it as a UF leaves a spec that never applies
    // it exactly where it stood (the Phase 127 pin, held
    // now with the arm live).
    let ignore = checked_spec(
        r#"Probe { spec ignore_f {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            constraint: output == x
            test { input: (|n| n + 1, 5); expect: 5 }
        } }"#,
    );
    let status = verdict(&ignore, var("x"));
    println!("P133|guard|ignore_f={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // Guard 2 (§3): function EQUALITY is not UF reasoning.
    // `output == (f == g)` — if the comparison encoded as
    // UF-symbol equality, Z3 could prove it (same symbol)
    // or refute it (distinct symbols); the encoder
    // declines the comparison, so the frozen standing
    // holds: Inconclusive.
    let same = checked_spec(
        r#"Probe { spec same_f {
            input: f: fn(Int) -> Int, g: fn(Int) -> Int
            output: Bool
            constraint: output == (f == g)
            test { input: (|n| n, |n| n); expect: true }
        } }"#,
    );
    let status = verdict(&same, IRNode::Literal(IRLiteral::Bool(true)));
    println!("P133|guard|same_f={status:?}");
    assert_eq!(status, VerificationStatus::Inconclusive);
    // And a body that itself performs the comparison
    // declines at evaluation — unencodable, same verdict.
    let comparing = IRNode::BinOp(
        IRBinOp::Eq,
        Box::new(var("f")),
        Box::new(var("g")),
    );
    let encodable = verifier::body_is_encodable(&same, &comparing);
    let status = verdict(&same, comparing);
    println!("P133|guard|same_f_comparing_body|encodable={encodable}|verify={status:?}");
    assert!(!encodable, "comparing two function values is unencodable");
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase133_signatures_and_value_flow() {
    // Multi-argument and Bool/String signatures ride the
    // same arm.
    let multi = checked_spec(
        r#"Probe { spec apply_pair {
            input: f: fn(Int, Int) -> Int, x: Int, y: Int
            output: Int
            constraint: output == f(x, y)
            test { input: (|a, b| a + b, 20, 22); expect: 42 }
        } }"#,
    );
    let status = verdict(&multi, call("f", vec![var("x"), var("y")]));
    println!("P133|sig|apply_pair={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    let bool_string = checked_spec(
        r#"Probe { spec apply_bs {
            input: g: fn(Bool) -> String, b: Bool
            output: String
            constraint: output == g(b)
            test { input: (|v| "y", true); expect: "y" }
        } }"#,
    );
    let status = verdict(&bool_string, call("g", vec![var("b")]));
    println!("P133|sig|apply_bs={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    // The function VALUE flows through spec inlining: a
    // callee whose parameter is bound to the caller's UF
    // applies the same symbol, so the composition
    // verifies end to end.
    let module_src = r#"Probe {
        spec apply_it {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            constraint: output == f(x)
            test { input: (|n| n + 1, 41); expect: 42 }
        }
        spec use_it {
            input: g: fn(Int) -> Int, y: Int
            output: Int
            constraint: output == apply_it(g, y)
            test { input: (|n| n + 1, 41); expect: 42 }
        }
    }"#;
    let module = parser::parse(module_src).expect("module parses");
    let checked = typeck::check(module).expect("module typechecks");
    let index = SpecIndex::from_module(&checked.spec);
    let use_it = checked
        .spec
        .functions
        .iter()
        .find(|s| s.name == "use_it")
        .expect("use_it present")
        .clone();
    let body = call("apply_it", vec![var("g"), var("y")]);
    let encodable = verifier::body_is_encodable_with(&use_it, &body, &index);
    let status = Verifier::with_index(index)
        .verify_one(&candidate(&use_it, body), &use_it)
        .status;
    println!("P133|flow|use_it|encodable={encodable}|verify={status:?}");
    #[cfg(feature = "z3-support")]
    {
        assert!(encodable);
        assert_eq!(status, VerificationStatus::Verified);
    }
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}

#[test]
fn phase133_mirror_agrees_on_the_apply_battery() {
    // §4's mirror discipline: `encodable.rs` learns
    // exactly the slice the real encoder learns — the two
    // encodability predicates agree on every shape below,
    // in both directions.
    let twice = checked_spec(APPLY_TWICE);
    let list_ranged = checked_spec(
        r#"Probe { spec apply_list_ranged {
            input: f: fn(Int) -> List<Int>, x: Int
            output: List<Int>
            constraint: output == f(x)
            test { input: (|n| [n], 40); expect: [40] }
        } }"#,
    );
    let same = checked_spec(
        r#"Probe { spec same_f {
            input: f: fn(Int) -> Int, g: fn(Int) -> Int
            output: Bool
            constraint: output == (f == g)
            test { input: (|n| n, |n| n); expect: true }
        } }"#,
    );
    let cases: Vec<(&FunctionSpec, IRNode, &str)> = vec![
        (&twice, apply_twice_body(), "apply_twice faithful body"),
        (&twice, call("f", vec![var("x")]), "apply once"),
        (&twice, var("x"), "no application"),
        (
            &list_ranged,
            call("f", vec![var("x")]),
            "unmappable signature application",
        ),
        (
            &same,
            IRNode::BinOp(IRBinOp::Eq, Box::new(var("f")), Box::new(var("g"))),
            "function comparison body",
        ),
        (
            &twice,
            IRNode::Map {
                function: Box::new(var("f")),
                over: Box::new(IRNode::Literal(IRLiteral::Int(0))),
            },
            "Map body",
        ),
    ];
    for (spec, body, label) in &cases {
        let real = verifier::body_is_encodable(spec, body);
        let pure = body_is_encodable_pure(spec, body, &SpecIndex::empty());
        println!("P133|mirror|{label}|real={real}|pure={pure}");
        assert_eq!(real, pure, "mirror disagreement on {label}");
    }
    // The battery's positive pole, stated outright: the
    // faithful apply body IS encodable on both sides.
    #[cfg(feature = "z3-support")]
    assert!(verifier::body_is_encodable(&twice, &apply_twice_body()));
    assert!(body_is_encodable_pure(
        &twice,
        &apply_twice_body(),
        &SpecIndex::empty()
    ));
}

// ── The verdict-drift sweep (gate clause 3's discipline) ──

fn corpus_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in [
        "examples",
        "examples/self_host",
        "stdlib",
        "playground/examples",
        "playground/corpus",
        "held_out",
    ] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "morph") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// The verdict table over the standing corpus: every spec
/// with a denotation body (`SpecIndex::callee_body` — the
/// body the pipeline itself would judge), verified against
/// its own module's index. Returns (spec name → status)
/// plus the count of specs with no denotation body.
fn verdict_table() -> (BTreeMap<String, String>, usize) {
    let mut table: BTreeMap<String, String> = BTreeMap::new();
    let mut no_body = 0usize;
    for path in corpus_files() {
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(module) = parser::parse(&source) else {
            continue; // the playground's intentional negative fixtures
        };
        let Ok(checked) = typeck::check(module) else {
            continue;
        };
        let index = SpecIndex::from_module(&checked.spec);
        let verifier = Verifier::with_index(index.clone());
        for spec in &checked.spec.functions {
            if table.contains_key(&spec.name) {
                continue;
            }
            match index.callee_body(&spec.name) {
                Some(body) => {
                    let status = verifier
                        .verify_one(&candidate(spec, body), spec)
                        .status;
                    table.insert(spec.name.clone(), format!("{status:?}"));
                }
                None => {
                    no_body += 1;
                    table.insert(spec.name.clone(), "NO_BODY".to_string());
                }
            }
        }
    }
    (table, no_body)
}

#[test]
fn phase133_corpus_verdict_table() {
    let (table, no_body) = verdict_table();
    for (name, status) in &table {
        println!("P133|verdict|{name}={status}");
    }
    println!(
        "P133|verdict|TOTAL specs={} no_body={}",
        table.len(),
        no_body
    );
    // The pinned after-table is asserted in
    // `phase133_corpus_verdict_drift` below; this
    // instrument only prints.
}

#[test]
fn phase133_corpus_verdict_drift() {
    // Gate clause 3's discipline, applied at the phase:
    // the BEFORE table (measured on the parent tree,
    // 0841dcf, with this same instrument) and the AFTER
    // table are IDENTICAL — 195 specs: 166 Verified, 25
    // NO_BODY (no denotation body in the module index —
    // the stdlib hand bodies among them are crowned by
    // the pipeline's prelude-merged index, pinned by the
    // stdlib suites, not by this sweep), 3 Inconclusive,
    // 1 VerificationFailed. The counts plus the named
    // non-Verified buckets pin the table completely: a
    // spec leaving any bucket changes a count or a set.
    // The arm's verdict movement is confined to the
    // frozen §3 probe exhibits (apply_twice, apply_req,
    // apply_fa — see `phase133_named_exhibit_movements`),
    // which live in probe sources, not in this corpus.
    //
    // Phase 135 amendment, recorded the same way: the
    // corpus grew by Amendment 2's six specs (195 → 201)
    // and every standing verdict is identical — 166
    // Verified, the failed set unchanged. Of the six,
    // five join NO_BODY (their hand bodies are crowned
    // through the prelude-merged route, like the
    // standing Tier-2 entries); `list_fold`'s single
    // defining equation (`output == fold(xs, init, f)`)
    // gives it a denotation in this sweep's route and it
    // stands Inconclusive — the same verdict its shipped
    // body carries (pinned in
    // tests/phase135_amendment_build.rs).
    let (table, no_body) = verdict_table();
    let count = |want: &str| table.values().filter(|s| s.as_str() == want).count();
    let named = |want: &str| -> Vec<String> {
        table
            .iter()
            .filter(|(_, s)| s.as_str() == want)
            .map(|(n, _)| n.clone())
            .collect()
    };
    println!(
        "P133|drift|total={} verified={} no_body={} inconclusive={:?} failed={:?}",
        table.len(),
        count("Verified"),
        no_body,
        named("Inconclusive"),
        named("VerificationFailed")
    );
    assert_eq!(table.len(), 201);
    assert_eq!(no_body, 30);
    assert_eq!(count("NO_BODY"), 30);
    #[cfg(feature = "z3-support")]
    {
        assert_eq!(count("Verified"), 166);
        assert_eq!(
            named("Inconclusive"),
            vec!["list_fold", "smallest", "sorted_probe", "total_or_empty"]
        );
        assert_eq!(named("VerificationFailed"), vec!["chained_fallback"]);
    }
    // The no-Z3 build's honest subset: no verdict is
    // available, so every judged spec stands Inconclusive
    // and the table's shape (201 / 30 NO_BODY) is the
    // drift content.
    #[cfg(not(feature = "z3-support"))]
    {
        assert_eq!(count("Verified"), 0);
        assert_eq!(count("Inconclusive"), 171);
        assert_eq!(count("VerificationFailed"), 0);
    }
}

#[test]
fn phase133_named_exhibit_movements() {
    // The arm's verdict movement inside the frozen §3
    // corpus, measured before/after on the two exhibits
    // whose rows the arm newly encodes (their §3
    // standings froze their CHECK outcome at Phase 129;
    // no verdict was frozen for either, and §4's letter
    // — application encodes as UF application — carries
    // no row-kind carve-out):
    //
    // * apply_req: Inconclusive → Verified. Its require
    //   row `f(x) > 0` was unencodable, so Phase 109's
    //   discipline declined the whole verification; the
    //   row now encodes and is assumed, and the
    //   postcondition `output == x` is an identity. The
    //   UF content sits only in the ASSUMED row — the
    //   proof itself uses no function fact at all.
    // * apply_fa: Inconclusive → Verified. The faithful
    //   body was unencodable; it now encodes, and the
    //   forall row (Phase 94's envelope; the binder is
    //   unused) closes by reflexivity over the UF terms.
    //
    // The remaining §3 exhibits measured identically on
    // both trees: pass_f and apply_atom stay Inconclusive
    // (Predicate form — §3.10's, untouched by the arm),
    // ignore_f stays Verified, same_f stays Inconclusive
    // (both pinned in `phase133_guards_ignore_f_and_same_f`).
    let req = checked_spec(
        r#"Probe { spec apply_req {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            require: f(x) > 0
            constraint: output == x
            test { input: (|n| n, 5); expect: 5 }
        } }"#,
    );
    let status = verdict(&req, var("x"));
    println!("P133|exhibit|apply_req={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);

    let fa = checked_spec(
        r#"Probe { spec apply_fa {
            input: f: fn(Int) -> Int, x: Int
            output: Int
            constraint: forall i: Int in range(1): output == f(f(x))
            test { input: (|n| n + 1, 40); expect: 42 }
        } }"#,
    );
    let status = verdict(&fa, apply_twice_body());
    println!("P133|exhibit|apply_fa={status:?}");
    #[cfg(feature = "z3-support")]
    assert_eq!(status, VerificationStatus::Verified);
    #[cfg(not(feature = "z3-support"))]
    assert_eq!(status, VerificationStatus::Inconclusive);
}
