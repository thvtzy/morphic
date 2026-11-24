//! Phase 150 — proof-carrying code (DISEBAR_SCOPE.md §4):
//! the claim in a manifest row becomes evidence a stranger
//! can re-check. The mechanism is re-verification: a
//! bundle (spec + body in the new IR text serialization +
//! claimed verdict/tier/battery + crown route) that
//! `verify-bundle` re-judges from (spec, body) alone, and
//! `verify --library`, the stdlib self-check that re-runs
//! every shipped entry's crown from the shipped data.
//!
//! Pins: the IR text format (stable, versioned,
//! parseable — round-trip identity over all 35 shipped
//! bodies and a synthetic battery covering the whole node
//! inventory; named declines for unknown versions, tags
//! and malformed text); the bundle (every shipped entry's
//! bundle re-verifies; the checked-in bundle files are
//! byte-equal to fresh checked production); the tamper
//! battery (a flipped verdict claim, a coherent downgrade,
//! a swapped body, a wrong route, stale format versions,
//! a moved battery count, malformed body text — each
//! fails with its own named refusal); the no-Z3 posture
//! (Tier-1 re-verification refused by name, Tier-2
//! re-judged on executed evidence); user bundles through
//! both crown routes; and the MCP verify tool's third
//! body source (IR text, the §3.3 consequence).
//!
//! Crown-heavy tests serialize on one lock: the synthesis
//! engine's RNG is process-global and heavy crowns flip
//! under parallel load on this box (the Phase 144 gate
//! binary's discipline).

use morphic::api::BodySource;
use morphic::codegen::bindings::{self, BindingInput, CrownRoute};
use morphic::mcp::McpServer;
use morphic::pcc::{self, Bundle};
use morphic::spec::ast::TypeRef;
use morphic::synthesis::engine::{
    CollectionKind, IRBinOp, IRLiteral, IRNode, IRPattern, IRUnaryOp,
};
use morphic::synthesis::irtext::{parse_ir, write_ir, IR_TEXT_VERSION};
use serde_json::{json, Value};
use std::sync::Mutex;

/// Crown-bearing tests serialize on this lock (the
/// recorded load class of Phases 142–144: heavy crowns
/// flip Inconclusive under parallel load — the verifier
/// carries a 5 s wall-clock per-constraint timeout —
/// each passing solo). The guard is recovered from
/// poisoning: if one test's crown flips under ambient
/// load and panics, the others still stand on their own
/// assertions, not on a PoisonError. No assertion is
/// weakened; the phase's gate evidence rests on solo
/// runs regardless.
static LOCK: Mutex<()> = Mutex::new(());

fn crown_serial() -> std::sync::MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// A minimal user module: one spec whose body is `x + 1`.
const INC_MODULE: &str = r#"Inc {
spec inc {
    input: x: Int
    output: Int

    constraint: output == x + 1

    test {
        input: 1
        expect: 2
    }

    test {
        input: 41
        expect: 42
    }
}
}
"#;

fn v(name: &str) -> IRNode {
    IRNode::Var(name.into())
}

fn il(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}

fn bx(n: IRNode) -> Box<IRNode> {
    Box::new(n)
}

// ── The IR text format ──────────────────────────────────

#[test]
fn ir_text_round_trips_the_whole_node_inventory() {
    let int = TypeRef::Int;
    let params = vec![
        ("x".to_string(), int.clone()),
        (
            "f".to_string(),
            TypeRef::Function(vec![TypeRef::Int], Box::new(TypeRef::Bool)),
        ),
    ];
    let nodes: Vec<IRNode> = vec![
        IRNode::Hole { id: 7, expected_type: None },
        IRNode::Hole { id: u64::MAX, expected_type: Some(TypeRef::List(Box::new(int.clone()))) },
        IRNode::Block(vec![v("x"), il(1)]),
        IRNode::Block(vec![]),
        IRNode::Let { name: "y".into(), value: bx(il(3)), body: bx(v("y")) },
        IRNode::If { condition: bx(v("b")), then_branch: bx(il(1)), else_branch: bx(il(0)) },
        IRNode::While { condition: bx(v("b")), body: bx(v("x")), invariant: None },
        IRNode::While {
            condition: bx(v("b")),
            body: bx(v("x")),
            invariant: Some("x >= 0 && y > 0".into()),
        },
        IRNode::For { var: "i".into(), start: bx(il(0)), end: bx(il(10)), body: bx(v("i")) },
        IRNode::Match {
            scrutinee: bx(v("o")),
            cases: vec![
                (IRPattern::Wildcard, il(0)),
                (IRPattern::Variable("n".into()), v("n")),
                (IRPattern::Literal(IRLiteral::Int(-5)), il(5)),
                (IRPattern::Literal(IRLiteral::Char('𝄞')), il(6)),
                (IRPattern::Literal(IRLiteral::String("a\"b\\c\nd".into())), il(7)),
                (
                    IRPattern::Constructor(
                        "Some".into(),
                        vec![IRPattern::Variable("w".into()), IRPattern::Wildcard],
                    ),
                    v("w"),
                ),
                (
                    IRPattern::Guard(
                        Box::new(IRPattern::Variable("g".into())),
                        "g > 0 && g < 9".into(),
                    ),
                    il(8),
                ),
            ],
        },
        IRNode::Call { function: "int_abs".into(), args: vec![v("x"), il(-1)] },
        IRNode::Return(bx(v("x"))),
        IRNode::Literal(IRLiteral::Int(i64::MIN)),
        IRNode::Literal(IRLiteral::Int(i64::MAX)),
        IRNode::Literal(IRLiteral::Float(0.5)),
        IRNode::Literal(IRLiteral::Float(-0.0)),
        IRNode::Literal(IRLiteral::Float(1e300)),
        IRNode::Literal(IRLiteral::Float(f64::INFINITY)),
        IRNode::Literal(IRLiteral::Float(f64::NAN)),
        IRNode::Literal(IRLiteral::Bool(true)),
        IRNode::Literal(IRLiteral::Bool(false)),
        IRNode::Literal(IRLiteral::String(String::new())),
        IRNode::Literal(IRLiteral::String("ünïcode 𝄞 \"quoted\" \\ \n\t".into())),
        IRNode::Literal(IRLiteral::Char('a')),
        IRNode::Literal(IRLiteral::Char('\'')),
        IRNode::Literal(IRLiteral::Char('\\')),
        IRNode::Literal(IRLiteral::Char('\n')),
        IRNode::Literal(IRLiteral::Unit),
        IRNode::Var("output".into()),
        IRNode::Lambda { params: params.clone(), body: bx(v("x")) },
        IRNode::Closure {
            params: params.clone(),
            captures: vec!["a".into(), "b".into()],
            body: bx(v("x")),
        },
        IRNode::Map { function: bx(v("f")), over: bx(v("xs")) },
        IRNode::Filter { predicate: bx(v("p")), over: bx(v("xs")) },
        IRNode::Fold { operation: bx(v("op")), initial: bx(il(0)), over: bx(v("xs")) },
        IRNode::Sort { comparator: bx(v("cmp")), collection: bx(v("xs")) },
        IRNode::Alloc { name: "acc".into(), ty: TypeRef::Int, initial: bx(il(0)) },
        IRNode::Alloc {
            name: "m".into(),
            ty: TypeRef::Map(Box::new(TypeRef::String), Box::new(TypeRef::Int)),
            initial: bx(IRNode::Collection { kind: CollectionKind::Map, elements: vec![] }),
        },
        IRNode::Alloc {
            name: "arr".into(),
            ty: TypeRef::Array(Box::new(TypeRef::Int), 3),
            initial: bx(il(0)),
        },
        IRNode::Alloc {
            name: "r".into(),
            ty: TypeRef::Result(Box::new(TypeRef::Int), Box::new(TypeRef::String)),
            initial: bx(il(0)),
        },
        IRNode::Assign { target: bx(v("acc")), value: bx(il(9)) },
        IRNode::Typed { node: bx(v("x")), ty: TypeRef::Tuple(vec![TypeRef::Int, TypeRef::Bool]) },
        IRNode::Typed { node: bx(v("x")), ty: TypeRef::Ref(Box::new(TypeRef::Int)) },
        IRNode::Typed { node: bx(v("x")), ty: TypeRef::Stream(Box::new(TypeRef::Char)) },
        IRNode::Typed { node: bx(v("x")), ty: TypeRef::Generic("T".into()) },
        IRNode::Typed { node: bx(v("x")), ty: TypeRef::Named("TypeTag".into()) },
        IRNode::Typed { node: bx(v("x")), ty: TypeRef::Set(Box::new(TypeRef::U8)) },
        IRNode::Typed { node: bx(v("x")), ty: TypeRef::Option(Box::new(TypeRef::I32)) },
        IRNode::Index(bx(v("xs")), bx(il(0))),
        IRNode::Loop { body: bx(IRNode::Break { value: None }), invariant: None },
        IRNode::Loop { body: bx(v("x")), invariant: Some("true".into()) },
        IRNode::Break { value: None },
        IRNode::Break { value: Some(bx(il(4))) },
        IRNode::StructDef {
            name: "Point".into(),
            fields: vec![("x".into(), TypeRef::Int), ("y".into(), TypeRef::Float)],
            body: bx(v("x")),
        },
        IRNode::TryPropagate(bx(v("r"))),
        IRNode::FieldAccess { base: bx(v("p")), field: "x".into() },
        IRNode::RecordLit {
            type_name: "Point".into(),
            fields: vec![("x".into(), il(1)), ("y".into(), v("q"))],
        },
    ];
    let mut covered = nodes;
    for op in [
        IRBinOp::Add, IRBinOp::Sub, IRBinOp::Mul, IRBinOp::Div, IRBinOp::Mod,
        IRBinOp::And, IRBinOp::Or, IRBinOp::Xor, IRBinOp::Eq, IRBinOp::Neq,
        IRBinOp::Lt, IRBinOp::Le, IRBinOp::Gt, IRBinOp::Ge, IRBinOp::Shl,
        IRBinOp::Shr, IRBinOp::Concat, IRBinOp::Append, IRBinOp::Merge,
    ] {
        covered.push(IRNode::BinOp(op, bx(v("a")), bx(v("b"))));
    }
    for op in [
        IRUnaryOp::Neg, IRUnaryOp::Not, IRUnaryOp::Abs, IRUnaryOp::Len,
        IRUnaryOp::Head, IRUnaryOp::Tail, IRUnaryOp::Init, IRUnaryOp::Last,
        IRUnaryOp::Reverse, IRUnaryOp::Clone,
    ] {
        covered.push(IRNode::UnaryOp(op, bx(v("a"))));
    }
    for kind in [
        CollectionKind::List, CollectionKind::Set, CollectionKind::Map,
        CollectionKind::Array, CollectionKind::Tuple,
    ] {
        covered.push(IRNode::Collection { kind, elements: vec![il(1), il(2)] });
    }
    for node in &covered {
        let text = write_ir(node);
        assert!(
            text.starts_with(IR_TEXT_VERSION),
            "the text carries its version marker: {text}"
        );
        let parsed = parse_ir(&text)
            .unwrap_or_else(|e| panic!("the written form parses ({e}): {text}"));
        assert_eq!(
            format!("{parsed:?}"),
            format!("{node:?}"),
            "round-trip identity (the pipeline's own fingerprint): {text}"
        );
        assert_eq!(write_ir(&parsed), text, "the written form is stable: {text}");
    }
}

#[test]
fn ir_text_declines_by_name() {
    // A version this build does not read: refused, naming it.
    let err = parse_ir("morphic-ir/2\n(var \"x\")").unwrap_err();
    assert!(err.contains("morphic-ir/2"), "the refusal names the version: {err}");
    assert!(err.contains("version"), "the refusal is about the version: {err}");
    // No version marker at all.
    let err = parse_ir("(var \"x\")").unwrap_err();
    assert!(err.contains("version"), "a missing marker declines as a version problem: {err}");
    // Empty text.
    assert!(parse_ir("").is_err(), "empty text declines");
    // An unknown node tag: refused, naming it.
    let err = parse_ir("morphic-ir/1\n(frobnicate (var \"x\"))").unwrap_err();
    assert!(err.contains("frobnicate"), "the refusal names the tag: {err}");
    // Truncated text declines rather than panicking.
    assert!(parse_ir("morphic-ir/1\n(block (var \"x\")").is_err());
    // A name in a non-string shape declines.
    assert!(parse_ir("morphic-ir/1\n(var 42)").is_err());
    // Trailing junk declines.
    assert!(parse_ir("morphic-ir/1\n(var \"x\") (var \"y\")").is_err());
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn ir_text_round_trips_every_shipped_body() {
    let inputs = bindings::stdlib_inputs();
    assert_eq!(inputs.len(), 35, "the shipped library surface");
    for input in &inputs {
        let text = write_ir(&input.body);
        let parsed = parse_ir(&text).unwrap_or_else(|e| {
            panic!("{}: the shipped body's written form parses ({e})", input.spec.name)
        });
        assert_eq!(
            format!("{parsed:?}"),
            format!("{:?}", input.body),
            "{}: round-trip identity on the shipped body",
            input.spec.name
        );
        assert_eq!(write_ir(&parsed), text, "{}: stable bytes", input.spec.name);
    }
}

// ── verify --library: the stdlib self-check ─────────────

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn verify_library_reports_every_crown() {
    let _guard = crown_serial();
    let standings = pcc::verify_library().expect("the self-check runs in a native build");
    assert_eq!(standings.len(), 35, "one standing per shipped entry");
    for s in &standings {
        if s.tier == "tier-1" && !cfg!(feature = "z3-support") {
            assert!(
                !s.reproduced,
                "{}: a Tier-1 crown cannot reproduce in a no-Z3 build",
                s.name
            );
            assert!(
                s.detail.contains("Z3-enabled native build"),
                "{}: the standing names the refusal: {}",
                s.name,
                s.detail
            );
        } else {
            assert!(
                s.reproduced,
                "{}: the crown reproduces from the shipped data ({})",
                s.name, s.detail
            );
        }
    }
}

// ── The bundles ─────────────────────────────────────────

#[cfg(all(not(feature = "wasm-playground"), feature = "z3-support"))]
#[test]
fn shipped_bundles_re_verify_and_match_the_checked_in_files() {
    let _guard = crown_serial();
    let produced = pcc::produce_all_bundles().expect("checked production in the Z3 build");
    assert_eq!(produced.len(), 35, "one bundle per shipped entry");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(pcc::BUNDLES_DIR);
    for (file_name, text) in &produced {
        let summary = pcc::verify_bundle_text(text)
            .unwrap_or_else(|e| panic!("{file_name}: the honest bundle re-verifies ({e})"));
        assert!(summary.contains("reproduces"), "{file_name}: {summary}");
        let on_disk = std::fs::read_to_string(dir.join(file_name))
            .unwrap_or_else(|e| panic!("{file_name}: the checked-in bundle exists ({e})"));
        assert_eq!(
            &on_disk, text,
            "{file_name}: the checked-in bundle is byte-equal to fresh production"
        );
    }
}

#[cfg(all(not(feature = "wasm-playground"), not(feature = "z3-support")))]
#[test]
fn no_z3_build_refuses_tier1_bundles_by_name() {
    let _guard = crown_serial();
    // A Tier-1 bundle, constructed by hand (production
    // itself needs the crown, so a no-Z3 producer cannot
    // mint one — the consumer refusal is what is pinned).
    let input = bindings::stdlib_inputs()
        .into_iter()
        .find(|i| i.spec.name == "int_abs")
        .expect("int_abs ships");
    let bundle = Bundle {
        format: pcc::BUNDLE_FORMAT.to_string(),
        bundle_version: pcc::BUNDLE_VERSION,
        entry: "int_abs".to_string(),
        tier: "tier-1".to_string(),
        verdict_basis: "z3-verified".to_string(),
        verdict: "Verified".to_string(),
        crown_route: "pipeline".to_string(),
        battery: pcc::BundleBattery { total: 6, evaluable: 6, passed: 6 },
        spec: input.spec.clone(),
        body: write_ir(&input.body),
    };
    let text = serde_json::to_string(&bundle).expect("the bundle serializes");
    let err = pcc::verify_bundle_text(&text).unwrap_err();
    assert!(
        err.contains("Z3-enabled native build"),
        "the Tier-1 refusal names the build it needs: {err}"
    );
    assert!(
        !err.contains("reproduces"),
        "the claim stands unexamined, never passed: {err}"
    );

    // A Tier-2 bundle is re-judged on executed evidence,
    // which this build has: it produces and re-verifies.
    let sort = bindings::stdlib_inputs()
        .into_iter()
        .find(|i| i.spec.name == "list_sort")
        .expect("list_sort ships");
    let bundle = pcc::produce_bundle(&sort).expect("Tier-2 production is executed-only");
    let text = serde_json::to_string(&bundle).expect("the bundle serializes");
    pcc::verify_bundle_text(&text).expect("the Tier-2 bundle re-verifies without Z3");
}

/// One tamper shape's refusal, for the distinctness pin.
#[cfg(all(not(feature = "wasm-playground"), feature = "z3-support"))]
fn tamper(bundle_text: &str, mutate: impl Fn(&mut Value)) -> String {
    let mut value: Value = serde_json::from_str(bundle_text).expect("the bundle is JSON");
    mutate(&mut value);
    let text = serde_json::to_string(&value).expect("the mutation serializes");
    pcc::verify_bundle_text(&text).expect_err("a tampered bundle never passes")
}

#[cfg(all(not(feature = "wasm-playground"), feature = "z3-support"))]
#[test]
fn tampered_bundles_fail_by_name() {
    let _guard = crown_serial();
    let inputs = bindings::stdlib_inputs();
    let bundle_of = |name: &str| -> String {
        let input = inputs.iter().find(|i| i.spec.name == name).unwrap();
        serde_json::to_string(&pcc::produce_bundle(input).expect("production"))
            .expect("the bundle serializes")
    };
    let int_abs = bundle_of("int_abs");
    let list_map = bundle_of("list_map");
    let sign_body = write_ir(
        &inputs.iter().find(|i| i.spec.name == "int_sign").unwrap().body,
    );

    let mut refusals: Vec<(&str, String)> = Vec::new();

    // A flipped verdict claim (the triple no longer coheres).
    refusals.push(("flipped verdict", tamper(&int_abs, |b| {
        b["verdict"] = json!("ExecutedExact");
    })));
    assert!(refusals[0].1.contains("incoherent claim"), "{}", refusals[0].1);

    // A coherent downgrade of a shipped entry (the whole
    // triple flipped consistently): the shipped row
    // contradicts it.
    refusals.push(("downgrade", tamper(&int_abs, |b| {
        b["tier"] = json!("tier-2");
        b["verdictBasis"] = json!("executed-exact");
        b["verdict"] = json!("ExecutedExact");
    })));
    assert!(refusals[1].1.contains("claim mismatch for shipped entry"), "{}", refusals[1].1);
    assert!(refusals[1].1.contains("tier"), "{}", refusals[1].1);

    // A swapped body: int_sign's body under int_abs's spec.
    refusals.push(("swapped body", tamper(&int_abs, |b| {
        b["body"] = json!(sign_body);
    })));
    assert!(refusals[2].1.contains("different body"), "{}", refusals[2].1);

    // A wrong crown route for a shipped entry.
    refusals.push(("wrong route", tamper(&list_map, |b| {
        b["crownRoute"] = json!("pipeline");
    })));
    assert!(refusals[3].1.contains("claim mismatch for shipped entry"), "{}", refusals[3].1);
    assert!(refusals[3].1.contains("crown route"), "{}", refusals[3].1);

    // A stale bundle version.
    refusals.push(("stale bundle version", tamper(&int_abs, |b| {
        b["bundleVersion"] = json!(2);
    })));
    assert!(refusals[4].1.contains("bundle version"), "{}", refusals[4].1);

    // A stale IR text version inside the body.
    refusals.push(("stale ir version", tamper(&int_abs, |b| {
        let body = b["body"].as_str().unwrap().replacen(IR_TEXT_VERSION, "morphic-ir/2", 1);
        b["body"] = json!(body);
    })));
    assert!(refusals[5].1.contains("morphic-ir/2"), "{}", refusals[5].1);

    // A moved battery count.
    refusals.push(("battery count", tamper(&int_abs, |b| {
        let passed = b["battery"]["passed"].as_u64().unwrap();
        b["battery"]["passed"] = json!(passed + 1);
    })));
    assert!(refusals[6].1.contains("battery"), "{}", refusals[6].1);
    assert!(refusals[6].1.contains("passed"), "{}", refusals[6].1);

    // Malformed body text.
    refusals.push(("malformed body", tamper(&int_abs, |b| {
        b["body"] = json!("(block");
    })));
    assert!(refusals[7].1.contains("ir text"), "{}", refusals[7].1);

    // Every shape's refusal is its own: no two coincide.
    let mut texts: Vec<&str> = refusals.iter().map(|(_, t)| t.as_str()).collect();
    texts.sort_unstable();
    texts.dedup();
    assert_eq!(texts.len(), refusals.len(), "distinct named refusals: {refusals:?}");
}

// ── User bundles (neither shipped nor prelude data) ─────

#[cfg(all(not(feature = "wasm-playground"), feature = "z3-support"))]
#[test]
fn a_user_bundle_re_verifies_through_the_pipeline_route() {
    let _guard = crown_serial();
    use morphic::spec::{parser, typeck};
    use morphic::synthesis::{engine, runner, selector};
    use morphic::verify::verifier::Verifier;

    // Crown the body the way the pipeline does (the
    // seeded run over the checked module), then bundle
    // THAT body: production re-runs the crown, so the
    // bundle's claim is measured, not stated.
    let parsed = parser::parse(INC_MODULE).expect("the module parses");
    let checked = typeck::check(parsed).expect("the module checks");
    let functions = checked.spec.functions;
    let spec = functions.iter().find(|f| f.name == "inc").unwrap().clone();
    engine::seed_engine_rng(42);
    let mut eng = engine::SynthesisEngine::with_module(morphic::api::playground_config(), &functions);
    let candidates = eng.run(&spec).expect("synthesis runs");
    let verified = Verifier::with_specs(&functions)
        .verify_all(candidates, &spec)
        .expect("verification runs");
    let pool = runner::rescore_finalists(verified, &spec);
    let winner = selector::select_best(pool, &spec).expect("a winner is selected");
    assert_eq!(
        winner.verification_status,
        Some(morphic::verify::verdict::VerificationStatus::Verified),
        "the pipeline crowns a Verified body for inc"
    );

    let input = BindingInput {
        spec,
        body: winner.body,
        tier: "tier-1".to_string(),
        verdict_basis: "z3-verified".to_string(),
        battery_pinned: 0,
        battery_silent: 0,
        crown_route: CrownRoute::Pipeline,
    };
    let bundle = pcc::produce_bundle(&input).expect("production re-runs the crown");
    let text = serde_json::to_string(&bundle).expect("the bundle serializes");
    let summary = pcc::verify_bundle_text(&text).expect("the user bundle re-verifies");
    assert!(summary.contains("inc"), "{summary}");
}

#[cfg(all(not(feature = "wasm-playground"), feature = "z3-support"))]
#[test]
fn a_user_bundle_re_verifies_through_the_encoder_route() {
    let _guard = crown_serial();
    use morphic::spec::parser;
    let parsed = parser::parse(INC_MODULE).expect("the module parses");
    let spec = parsed
        .functions
        .iter()
        .find(|f| f.name == "inc")
        .unwrap()
        .clone();
    let body = IRNode::BinOp(
        IRBinOp::Add,
        bx(v("x")),
        bx(IRNode::Literal(IRLiteral::Int(1))),
    );
    let input = BindingInput {
        spec,
        body,
        tier: "tier-1".to_string(),
        verdict_basis: "z3-verified".to_string(),
        battery_pinned: 0,
        battery_silent: 0,
        crown_route: CrownRoute::Encoder,
    };
    let bundle = pcc::produce_bundle(&input).expect("the encoder route crowns x + 1");
    assert_eq!(bundle.verdict, "Verified");
    let text = serde_json::to_string(&bundle).expect("the bundle serializes");
    pcc::verify_bundle_text(&text).expect("the user bundle re-verifies");

    // And a wrong body under the same spec fails through
    // the route — the bundle is evidence, not a label.
    let wrong = BindingInput {
        body: IRNode::BinOp(IRBinOp::Sub, bx(v("x")), bx(il(1))),
        ..input
    };
    assert!(
        pcc::produce_bundle(&wrong).is_err(),
        "production refuses a body the route does not crown"
    );
}

// ── The MCP consequence: the verify tool's third source ──

#[test]
fn mcp_verify_accepts_ir_text_bodies() {
    let _guard = crown_serial();
    let mut server = McpServer::new();
    let body = IRNode::BinOp(IRBinOp::Add, bx(v("x")), bx(il(1)));
    let call = |server: &mut McpServer, id: u64, arguments: Value| -> (bool, Value) {
        let line = json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
                          "params": {"name": "verify", "arguments": arguments}})
            .to_string();
        let response = server.handle_line(&line).expect("a response");
        let response: Value = serde_json::from_str(&response).expect("JSON");
        let result = &response["result"];
        let text = result["content"][0]["text"].as_str().expect("text content");
        (
            result["isError"].as_bool().expect("isError"),
            serde_json::from_str(text).expect("DTO JSON"),
        )
    };

    // The third source, named in the tool's own schema.
    let list = {
        let line = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}})
            .to_string();
        let response: Value =
            serde_json::from_str(&server.handle_line(&line).expect("a response")).unwrap();
        response
    };
    let verify_tool = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "verify")
        .expect("the verify tool")
        .clone();
    let kinds = verify_tool["inputSchema"]["properties"]["body"]["properties"]["kind"]["enum"]
        .as_array()
        .expect("the kind enum");
    assert!(
        kinds.iter().any(|k| k == "ir-text"),
        "the schema names the third source: {kinds:?}"
    );

    // A body carried as IR text is judged in-process.
    let (is_error, payload) = call(
        &mut server,
        2,
        json!({
            "source": INC_MODULE,
            "spec": "inc",
            "body": {"kind": "ir-text", "text": write_ir(&body)},
        }),
    );
    assert!(!is_error, "the ir-text call succeeds: {payload}");
    assert_eq!(payload["body_source"], "ir-text");
    if cfg!(feature = "z3-support") {
        assert_eq!(payload["status"], "Verified", "{payload}");
    } else {
        assert_eq!(payload["verification_available"], false, "{payload}");
    }

    // The same body through the api, by the same name.
    let report = morphic::api::verify_body(
        INC_MODULE,
        "inc",
        &BodySource::IrText(write_ir(&body)),
    )
    .expect("the api judges the ir-text body");
    assert_eq!(report.body_source, "ir-text");

    // Malformed IR text declines by name — a tool error,
    // never a panic, never a dressed verdict.
    let (is_error, payload) = call(
        &mut server,
        3,
        json!({
            "source": INC_MODULE,
            "spec": "inc",
            "body": {"kind": "ir-text", "text": "morphic-ir/1\n(frobnicate)"},
        }),
    );
    assert!(is_error, "malformed text is a tool error: {payload}");
    assert!(
        payload["message"].as_str().is_some_and(|m| m.contains("frobnicate")),
        "the decline names the shape: {payload}"
    );
}
