//! Phase 115 (the List family, STDLIB_SCOPE §2.6): the
//! Tier-2 pin.
//!
//! The frozen scope runs the List family at two tiers.
//! `list_is_empty` is Tier-1 and is pinned with the
//! other Tier-1 entries in tests/phase110_prelude.rs
//! (pipeline crown, Z3-Verified ∧ golden-exact). The
//! eleven element-level entries are Tier-2 (decision
//! D7a): **executed-exact on the full battery, the tier
//! printed in each entry's documentation** — because
//! the verifier binds a List parameter as its length
//! only, so no element-level entry can Verify at any
//! body quality until the §3.9 list-element encoder
//! lands post-1.0, with this library as its test
//! corpus.
//!
//! What this file pins, per Tier-2 entry:
//!
//! * the shipped body is the hand body recorded here
//!   (Debug identity against the registry) — the
//!   pipeline crowns no Verified body for any Tier-2
//!   entry and its executed crowns are unstable
//!   samples (measured below), so the hand body is
//!   the entry's crown at the Tier-2 standard, and this
//!   pin re-proves its executed-exactness on the full
//!   battery through BOTH executed judges;
//! * in-process: the production interpreter
//!   (`eval::run_spec_tests`) passes every in-spec test,
//!   with no spec-silent and no unevaluable point;
//! * compiled: the body renders through production
//!   codegen, compiles under rustc, and passes every
//!   in-spec test — the compiled-runner route, driven
//!   directly (the runner stage itself fires only on
//!   its recorded triggers; the Tier-2 standard is the
//!   route's verdict on the body, which is what is
//!   measured here);
//! * the verifier's verdict is Inconclusive — the
//!   honest §3.9 standing, pinned so a future encoder
//!   improvement moves it loudly, in the open;
//! * the entry's doc comment names its tier (the
//!   scope's "tier printed in the documentation").
//!
//! The bodies are written in the one vocabulary both
//! executed judges reach in full: For scans whose loop
//! variable is the index (the compiled form's only
//! usable index — generated Rust cannot index a Vec by
//! an Int variable, the G5 boundary), Alloc/Assign
//! accumulators, statement Append, and Int counters
//! wherever an Int is compared or produced. Phase 115
//! added the interpreter's For arm (the judging
//! machinery §2.6 priced into this phase), so the
//! in-process judge reaches these bodies exactly as
//! the compiled runner does.
//!
//! §3.4, measured (the `is_sorted` judging-coverage
//! wall — `runner::trigger` is owner-reserved and is
//! NOT touched by this phase): the wall's in-process
//! half is DISSOLVED by the For arm — the standing
//! AdjacentScan TEMPLATE body, unevaluable in-process
//! since Phase 59, now judges 5/5 through the
//! interpreter on the example spec, and the pipeline
//! usually crowns that very template body for
//! `list_is_sorted` at 9/9. But the crown is not
//! stable: one measured pipeline sample crowned a
//! constant-true body at 6/9 instead. The pipeline's
//! autonomous judging of this entry is therefore
//! unreliable in both directions, which is exactly
//! why the library ships pinned data: the shipped
//! hand body — the same adjacent scan, recorded here
//! — judges 9/9 on both executed routes on every run.
//! The owner's one-line trigger decision remains
//! theirs to take; the Tier-2 battery does not wait
//! on it, and the entry ships.

//! Configuration note: pins that judge or render the
//! NATIVE prelude (embedded library data) are gated
//! `#[cfg(not(feature = "wasm-playground"))]` — under
//! that feature the prelude is installed from the stdlib
//! asset at runtime (browser-side behaviour is pinned by
//! the Phase 112 asset pins + the playground corpus); a
//! workspace `--all-targets` build unifies the feature
//! into this package via the morphic-wasm member. In the
//! standing native configurations every pin runs.

use morphic::api;
use morphic::spec::ast::{FunctionSpec, TypeRef};
use morphic::spec::{parser, typeck};
use morphic::stdlib;
use morphic::synthesis::engine::{
    CandidateImplementation, CollectionKind, IRBinOp, IRLiteral, IRNode, IRUnaryOp, Provenance,
    ScoreBreakdown,
};
use morphic::synthesis::{eval, templates};
use morphic::verify::verdict::VerificationStatus;
use morphic::verify::verifier::Verifier;

const LIST_SRC: &str = include_str!("../stdlib/list.morph");
const IS_SORTED_EXAMPLE: &str = include_str!("../examples/self_host/is_sorted.morph");

/// The eleven Tier-2 entries, in module order.
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
];

/// (name, in-spec battery size) per Tier-2 entry.
const BATTERIES: &[(&str, usize)] = &[
    ("list_reverse", 6),
    ("list_contains", 7),
    ("list_sum", 8),
    ("list_product", 8),
    ("list_min", 6),
    ("list_max", 6),
    ("list_take", 7),
    ("list_drop", 7),
    ("list_is_sorted", 9),
    ("list_sort", 7),
    ("list_binary_search", 10),
];

// ── Body construction (the hand bodies, recorded) ────

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
fn len_c(x: IRNode) -> IRNode {
    IRNode::Call {
        function: "len".into(),
        args: vec![x],
    }
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
fn some(x: IRNode) -> IRNode {
    IRNode::Call {
        function: "Some".into(),
        args: vec![x],
    }
}
fn none() -> IRNode {
    IRNode::Call {
        function: "None".into(),
        args: vec![],
    }
}
fn ret(x: IRNode) -> IRNode {
    IRNode::Return(Box::new(x))
}
fn list_ty() -> TypeRef {
    TypeRef::List(Box::new(TypeRef::Int))
}

fn expected_body(name: &str) -> IRNode {
    match name {
        "list_reverse" => block(vec![
            alloc("out", list_ty(), empty_list()),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                append_stmt(
                    "out",
                    idx(
                        v("xs"),
                        bop(IRBinOp::Sub, bop(IRBinOp::Sub, len_u(v("xs")), i(1)), v("i")),
                    ),
                ),
            ),
            ret(v("out")),
        ]),
        "list_contains" => block(vec![
            alloc("found", TypeRef::Bool, boolean(false)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                assign(
                    "found",
                    bop(
                        IRBinOp::Or,
                        v("found"),
                        bop(IRBinOp::Eq, idx(v("xs"), v("i")), v("x")),
                    ),
                ),
            ),
            ret(v("found")),
        ]),
        "list_sum" => block(vec![
            alloc("acc", TypeRef::Int, i(0)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                assign("acc", bop(IRBinOp::Add, v("acc"), idx(v("xs"), v("i")))),
            ),
            ret(v("acc")),
        ]),
        "list_product" => block(vec![
            alloc("acc", TypeRef::Int, i(1)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                assign("acc", bop(IRBinOp::Mul, v("acc"), idx(v("xs"), v("i")))),
            ),
            ret(v("acc")),
        ]),
        "list_min" => if_(
            bop(IRBinOp::Eq, len_c(v("xs")), i(0)),
            none(),
            block(vec![
                alloc("m", TypeRef::Int, idx(v("xs"), i(0))),
                for_(
                    "i",
                    i(1),
                    len_u(v("xs")),
                    assign(
                        "m",
                        if_(
                            bop(IRBinOp::Lt, idx(v("xs"), v("i")), v("m")),
                            idx(v("xs"), v("i")),
                            v("m"),
                        ),
                    ),
                ),
                ret(some(v("m"))),
            ]),
        ),
        "list_max" => if_(
            bop(IRBinOp::Eq, len_c(v("xs")), i(0)),
            none(),
            block(vec![
                alloc("m", TypeRef::Int, idx(v("xs"), i(0))),
                for_(
                    "i",
                    i(1),
                    len_u(v("xs")),
                    assign(
                        "m",
                        if_(
                            bop(IRBinOp::Gt, idx(v("xs"), v("i")), v("m")),
                            idx(v("xs"), v("i")),
                            v("m"),
                        ),
                    ),
                ),
                ret(some(v("m"))),
            ]),
        ),
        "list_take" => block(vec![
            alloc("out", list_ty(), empty_list()),
            alloc("k", TypeRef::Int, i(0)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                block(vec![
                    if_(
                        bop(IRBinOp::Lt, v("k"), v("n")),
                        append_stmt("out", idx(v("xs"), v("i"))),
                        block(vec![]),
                    ),
                    assign("k", bop(IRBinOp::Add, v("k"), i(1))),
                ]),
            ),
            ret(v("out")),
        ]),
        "list_drop" => block(vec![
            alloc("out", list_ty(), empty_list()),
            alloc("k", TypeRef::Int, i(0)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                block(vec![
                    if_(
                        bop(IRBinOp::Ge, v("k"), v("n")),
                        append_stmt("out", idx(v("xs"), v("i"))),
                        block(vec![]),
                    ),
                    assign("k", bop(IRBinOp::Add, v("k"), i(1))),
                ]),
            ),
            ret(v("out")),
        ]),
        "list_is_sorted" => block(vec![
            alloc("ok", TypeRef::Bool, boolean(true)),
            for_(
                "i",
                i(1),
                len_u(v("xs")),
                assign(
                    "ok",
                    bop(
                        IRBinOp::And,
                        v("ok"),
                        bop(
                            IRBinOp::Le,
                            idx(v("xs"), bop(IRBinOp::Sub, v("i"), i(1))),
                            idx(v("xs"), v("i")),
                        ),
                    ),
                ),
            ),
            ret(v("ok")),
        ]),
        "list_sort" => block(vec![
            alloc("out", list_ty(), empty_list()),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                block(vec![
                    alloc("x", TypeRef::Int, idx(v("xs"), v("i"))),
                    alloc("pos", TypeRef::Int, len_c(v("out"))),
                    alloc("k", TypeRef::Int, i(0)),
                    for_(
                        "j",
                        i(0),
                        len_u(v("out")),
                        block(vec![
                            if_(
                                bop(
                                    IRBinOp::And,
                                    bop(IRBinOp::Eq, v("pos"), len_c(v("out"))),
                                    bop(IRBinOp::Gt, idx(v("out"), v("j")), v("x")),
                                ),
                                assign("pos", v("k")),
                                block(vec![]),
                            ),
                            assign("k", bop(IRBinOp::Add, v("k"), i(1))),
                        ]),
                    ),
                    alloc("next", list_ty(), empty_list()),
                    alloc("k2", TypeRef::Int, i(0)),
                    for_(
                        "j",
                        i(0),
                        len_u(v("out")),
                        block(vec![
                            if_(
                                bop(IRBinOp::Eq, v("k2"), v("pos")),
                                append_stmt("next", v("x")),
                                block(vec![]),
                            ),
                            append_stmt("next", idx(v("out"), v("j"))),
                            assign("k2", bop(IRBinOp::Add, v("k2"), i(1))),
                        ]),
                    ),
                    if_(
                        bop(IRBinOp::Eq, v("pos"), len_c(v("out"))),
                        append_stmt("next", v("x")),
                        block(vec![]),
                    ),
                    assign("out", v("next")),
                ]),
            ),
            ret(v("out")),
        ]),
        "list_binary_search" => block(vec![
            alloc("res", TypeRef::Option(Box::new(TypeRef::Int)), none()),
            alloc("k", TypeRef::Int, i(0)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                block(vec![
                    if_(
                        bop(
                            IRBinOp::And,
                            bop(IRBinOp::Eq, idx(v("xs"), v("i")), v("x")),
                            bop(IRBinOp::Eq, v("res"), none()),
                        ),
                        assign("res", some(v("k"))),
                        block(vec![]),
                    ),
                    assign("k", bop(IRBinOp::Add, v("k"), i(1))),
                ]),
            ),
            ret(v("res")),
        ]),
        other => panic!("no expected body for {other}"),
    }
}

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
        id: 115,
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

fn dbg(node: &IRNode) -> String {
    format!("{:?}", node)
}

// ── The pin ──────────────────────────────────────────

#[test]
fn tier2_docs_name_the_tier() {
    let specs = list_specs();
    for name in TIER2 {
        let spec = spec_of(&specs, name);
        let doc = spec.doc.clone().unwrap_or_default();
        assert!(
            doc.contains("Tier-2"),
            "{name}: the tier must be printed in the entry's documentation, got: {doc}"
        );
    }
    let empty = spec_of(&specs, "list_is_empty");
    assert!(
        empty.doc.clone().unwrap_or_default().contains("Tier-1"),
        "list_is_empty: the Tier-1 standard is printed in its documentation"
    );
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn tier2_shipped_bodies_are_the_hand_bodies() {
    for name in TIER2 {
        let shipped = stdlib::shipped_body(name)
            .unwrap_or_else(|| panic!("{name} ships a body"));
        assert_eq!(
            dbg(&shipped),
            dbg(&expected_body(name)),
            "{name}: the shipped body must be the recorded hand body"
        );
    }
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn tier2_bodies_are_executed_exact_in_process() {
    let specs = list_specs();
    for (name, total) in BATTERIES {
        let spec = spec_of(&specs, name);
        let body = stdlib::shipped_body(name).unwrap();
        let outcomes = eval::test_outcomes(&body, &spec);
        assert_eq!(outcomes.len(), *total, "{name}: battery size");
        assert!(
            outcomes.iter().all(|o| matches!(o, eval::TestOutcome::Passed)),
            "{name}: every in-spec point must pass in-process (no spec-silent, \
             no unevaluable point), got {outcomes:?}"
        );
        let run = eval::run_spec_tests(&body, &spec);
        assert_eq!(
            (run.passed, run.evaluable),
            (*total, *total),
            "{name}: executed-exact in-process"
        );
        println!("P115PIN|{name}|in-process {total}/{total}");
    }
}

/// The compiled route for every List entry (Tier-1
/// included): production codegen renders the shipped
/// body, rustc compiles the generated module, and its
/// own test module — one generated test per in-spec
/// test — passes in full.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn list_bodies_are_executed_exact_through_the_compiled_route() {
    let specs = list_specs();
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase115-compiled-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut all: Vec<(&str, usize)> = vec![("list_is_empty", 4)];
    all.extend(BATTERIES.iter().copied());
    for (name, total) in all {
        let spec = spec_of(&specs, name);
        let body = stdlib::shipped_body(name).unwrap();
        let generated = morphic::codegen::generate(&candidate(&spec, body), "rust")
            .unwrap_or_else(|e| panic!("{name}: the shipped body must render to Rust: {e}"));
        let src_path = dir.join(format!("{name}.rs"));
        std::fs::write(&src_path, &generated).unwrap();
        let bin_path = dir.join(format!("{name}_test"));
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
            "{name}: the generated Rust must compile:\n{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let run = std::process::Command::new(&bin_path)
            .output()
            .expect("the compiled test binary runs");
        let stdout = String::from_utf8_lossy(&run.stdout);
        assert!(
            run.status.success()
                && stdout.contains(&format!("test result: ok. {total} passed")),
            "{name}: the compiled route must pass {total}/{total}, got: {stdout}"
        );
        println!("P115PIN|{name}|compiled {total}/{total}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(feature = "z3-support")]
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn tier2_verdicts_are_inconclusive_the_section_3_9_standing() {
    let specs = list_specs();
    for name in TIER2 {
        let spec = spec_of(&specs, name);
        let body = stdlib::shipped_body(name).unwrap();
        let verdict = Verifier::with_specs(&specs)
            .verify_one(&candidate(&spec, body), &spec)
            .status;
        assert_eq!(
            verdict,
            VerificationStatus::Inconclusive,
            "{name}: element-level verdicts wait on the §3.9 encoder — \
             Inconclusive is the honest standing until it lands"
        );
    }
}

/// Why hand bodies ship: the production pipeline
/// (seeded profile, seed 42) crowns NO Verified body
/// for any Tier-2 entry — element-level rows are
/// unencodable, so no body can Verify (the §3.9
/// standing, a property of the encoder, not a sample).
/// The executed standings of its crowns are moreover
/// unstable samples: `list_is_sorted`'s crown flips
/// between the AdjacentScan template body at 9/9 and a
/// constant-true body at 6/9 across runs. Pinned data
/// is the deterministic answer: the shipped hand body,
/// whose executed-exactness the pins above re-prove on
/// every run. If a crown ever Verifies, the entry is
/// re-crowned and the data updated, in the open (the
/// Tier-1 discipline).
#[cfg(feature = "z3-support")]
#[test]
fn the_pipeline_crowns_no_verified_tier2_body() {
    for (name, total) in BATTERIES {
        let report = api::synthesize_seeded_spec(LIST_SRC, name, 42)
            .unwrap_or_else(|e| panic!("{name}: pipeline run: {e:?}"));
        let w = &report.winner;
        assert_eq!(w.tests_total, *total, "{name}: battery size");
        assert_ne!(
            w.verification_status, "Verified",
            "{name}: no Tier-2 crown can Verify while list elements are \
             unencodable — if one ever does, ship the crown instead"
        );
        println!(
            "P115CROWN|{name}|pipeline crown: {} {}/{}",
            w.verification_status, w.tests_passed, w.tests_evaluable
        );
    }
}

/// §3.4 measured, both halves (see the file header):
/// the pipeline's `list_is_sorted` crown is the wall's
/// constant-true body — NOT the shipped body — while
/// the standing AdjacentScan template body now judges
/// in-process at 5/5 on the example spec (the For arm
/// dissolved the wall's in-process half). Nothing here
/// touches `runner::trigger`.
#[test]
fn section_3_4_measured() {
    // The template half: the AdjacentScan body the
    // pipeline builds for the example is_sorted spec
    // evaluates in-process now, and passes its tests.
    let module = parser::parse(IS_SORTED_EXAMPLE).expect("example parses");
    let checked = typeck::check(module).expect("example checks");
    let example = &checked.spec.functions[0];
    let templates = templates::generate_templates(example);
    assert_eq!(templates.len(), 1, "the AdjacentScan template fires");
    assert_eq!(templates[0].0, templates::TemplateKind::AdjacentScan);
    let run = eval::run_spec_tests(&templates[0].1, example);
    assert_eq!(
        (run.passed, run.evaluable),
        (5, 5),
        "the template body must judge in-process at 5/5 after the For arm"
    );
}

/// §3.4 measured, the pipeline half (the in-process
/// half is `section_3_4_measured` above): the crown
/// for `list_is_sorted` is never Verified, and across
/// measured samples it flips between the AdjacentScan
/// template body at 9/9 and a constant-true body at
/// 6/9 — the instability is the finding, so this pin
/// asserts only what every sample shares (Inconclusive,
/// never the Tier-1 standard) and records the sample
/// it drew. Nothing here touches `runner::trigger`.
#[cfg(feature = "z3-support")]
#[test]
fn section_3_4_the_pipeline_crown_for_is_sorted_is_unstable() {
    let report = api::synthesize_seeded_spec(LIST_SRC, "list_is_sorted", 42)
        .expect("pipeline run over list_is_sorted");
    assert_eq!(
        report.winner.verification_status, "Inconclusive",
        "the §3.9 standing: no element-level crown Verifies"
    );
    println!(
        "P115WALL|list_is_sorted pipeline crown sample: {} {}/{} (measured samples flip \
         between the template body at 9/9 and a constant-true body at 6/9; the shipped \
         hand body judges 9/9 on both executed routes deterministically)",
        report.winner.verification_status,
        report.winner.tests_passed,
        report.winner.tests_total
    );
}
