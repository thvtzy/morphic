//! Phase 118 (1.1 Embeddable scope audit): probes pinning
//! the embedding surfaces AS THEY STAND at v1.0.0 — the
//! measured evidence base of EMBED_AUDIT.md. In the
//! Phase 106 tradition these pins assert today's truth,
//! including the gaps: when a future 1.1 phase closes a
//! gap, the corresponding pin fails loudly and is
//! re-aimed at the new truth at its site.
//!
//! What is pinned here:
//!
//! * The JavaScript emitter's coverage of the shipped
//!   library bodies (the pre-generated-JS route): Tier-1
//!   expression bodies and Match bodies vanish into
//!   `// unsupported` (no tail renderer — NF-116-1), a
//!   Tier-2 For body loses its accumulator statements and
//!   returns an undeclared variable, `%` / `<=` render as
//!   a bare `?` (unparseable JS), Int literals render as
//!   bare JS numbers (precision boundary at 2^53), Int
//!   division renders as JS float division, and
//!   constructors / Index have no representation
//!   (`undefined`). Entry CALLS, by contrast, refuse
//!   loudly since Phase 116 — the asymmetry is the
//!   finding: calls refuse, bodies break.
//! * The WAT target's type map: Morphic Int renders as
//!   WAT i32 (truncation in the signature itself), and a
//!   Tier-1 body renders as a comment.
//! * The interpreter route a Rust host has today: the
//!   shipped bodies evaluate directly through
//!   `synthesis::eval::eval` with a hand-built Env
//!   (int_abs, int_clamp, list_sum measured) — the route
//!   exists at IR level; no name→call API wraps it.
//! * The Rust target as the contrast: a Tier-1 entry
//!   generates real typed code — and carries NF-116-2
//!   verbatim (`debug_assert!(true, ...)` for its
//!   precondition).
//!
//! Configuration note (Phase 116/117 precedent): pins
//! that read the NATIVE prelude (embedded library data)
//! are gated `#[cfg(not(feature = "wasm-playground"))]`
//! — under that feature the prelude is installed from
//! the stdlib asset at runtime. Hand-body pins build
//! their spec from a local parsed module and run in
//! every configuration.

use morphic::codegen::{self, CodegenError};
use morphic::spec::ast::FunctionSpec;
use morphic::spec::parser;
#[cfg(not(feature = "wasm-playground"))]
use morphic::stdlib;
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, Provenance, ScoreBreakdown,
};
#[cfg(not(feature = "wasm-playground"))]
use morphic::synthesis::eval::{self, Env, Value};
#[cfg(not(feature = "wasm-playground"))]
use std::collections::HashMap;

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 118,
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

#[cfg(not(feature = "wasm-playground"))]
fn entry(name: &str) -> (FunctionSpec, IRNode) {
    let spec = stdlib::prelude_specs()
        .into_iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("{name} in prelude"));
    let body = stdlib::shipped_body(name).unwrap_or_else(|| panic!("{name} ships a body"));
    (spec, body)
}

#[cfg(not(feature = "wasm-playground"))]
fn js_of_entry(name: &str) -> String {
    let (spec, body) = entry(name);
    codegen::generate(&candidate(&spec, body), "javascript")
        .unwrap_or_else(|e| panic!("{name} JS generation: {e}"))
}

/// A local spec for hand-built bodies (configuration-
/// independent: no prelude involved).
fn hand_spec() -> FunctionSpec {
    let module = parser::parse(
        "Probe {\nspec f {\n    input: x: Int\n    output: Int\n    constraint: output == x\n    test \"t\" { input: 1 expect: 1 }\n}\n}",
    )
    .expect("probe module parses");
    module.functions.into_iter().next().expect("one spec")
}

fn ret(b: IRNode) -> IRNode {
    IRNode::Return(Box::new(b))
}
fn var(n: &str) -> IRNode {
    IRNode::Var(n.into())
}
fn lit(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}
fn js_of_hand(body: IRNode) -> String {
    codegen::generate(&candidate(&hand_spec(), body), "javascript").expect("hand JS generation")
}

// ── The JS route: shipped bodies ───────────────────────

/// NF-116-1's tail gap, at the simplest Tier-1 body in
/// the library: `int_abs`'s If renders, its value
/// branches do not — the generated function contains no
/// `return` at all and answers `undefined` for every
/// input (node-run measured in the audit).
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn js_tier1_int_abs_has_no_return() {
    let js = js_of_entry("int_abs");
    assert!(js.contains("function int_abs(x)"), "{js}");
    assert!(js.contains("// unsupported"), "{js}");
    assert!(!js.contains("return"), "no return survives:\n{js}");
}

/// Bare-expression bodies vanish wholesale in statement
/// position — even `string_is_empty`, whose only
/// ingredient (`len`) has a correct JS expression arm.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn js_expression_bodies_vanish() {
    for name in ["string_is_empty", "list_is_empty", "int_is_even"] {
        let js = js_of_entry(name);
        assert!(js.contains("// unsupported"), "{name}:\n{js}");
        assert!(!js.contains("return"), "{name} must not return:\n{js}");
    }
}

/// Match bodies (the Result/Option/Char entries' shape)
/// have no JS arm at all.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn js_match_bodies_vanish() {
    for name in ["option_unwrap_or", "char_to_digit"] {
        let js = js_of_entry(name);
        assert!(js.contains("// unsupported"), "{name}:\n{js}");
        assert!(!js.contains("return"), "{name} must not return:\n{js}");
    }
}

/// A Tier-2 For body partially renders — the loop and
/// its `len` bound survive; the accumulator's Alloc and
/// Assign statements drop, leaving `return acc;` naming
/// a variable nothing declares (node: ReferenceError).
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn js_tier2_list_sum_drops_its_accumulator() {
    let js = js_of_entry("list_sum");
    assert!(js.contains("for (let i = 0;"), "{js}");
    assert!(js.contains("// unsupported"), "{js}");
    assert!(js.contains("return acc;"), "{js}");
}

/// The asymmetry Phase 116 left: a body CALLING an
/// entry refuses loudly by name on JS, while the
/// entries' own bodies (above) silently break.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn js_entry_calls_refuse_loudly() {
    let body = ret(IRNode::Call {
        function: "int_abs".into(),
        args: vec![var("x")],
    });
    let err = codegen::generate(&candidate(&hand_spec(), body), "javascript")
        .expect_err("entry calls refuse on JS");
    assert!(
        matches!(err, CodegenError::UnrenderableLibraryCall { ref name, .. } if name == "int_abs"),
        "{err}"
    );
}

// ── The JS route: expression-level losses ──────────────

/// `%` and `<=` (and `>=`) have no JS operator arm: the
/// fallback string is a bare `?`, producing
/// `return (x ? 2);` — a SyntaxError under node --check.
#[test]
fn js_mod_and_le_render_a_bare_question_mark() {
    for op in [IRBinOp::Mod, IRBinOp::Le] {
        let js = js_of_hand(ret(IRNode::BinOp(op, Box::new(var("x")), Box::new(lit(2)))));
        assert!(js.contains("return (x ? 2);"), "{js}");
    }
}

/// Int literals render as bare JS number literals: the
/// i64 maximum is emitted digit-exact, and a JS engine
/// parses it to 9223372036854775808 (2^63) — the
/// precision boundary a typed binding must decide.
#[test]
fn js_int_literal_is_a_bare_js_number() {
    let js = js_of_hand(ret(lit(i64::MAX)));
    assert!(js.contains("return 9223372036854775807;"), "{js}");
}

/// Morphic Int division truncates; the JS rendering is
/// the float-division operator, verbatim.
#[test]
fn js_int_division_renders_float_division() {
    let js = js_of_hand(ret(IRNode::BinOp(
        IRBinOp::Div,
        Box::new(var("x")),
        Box::new(lit(2)),
    )));
    assert!(js.contains("return (x / 2);"), "{js}");
}

/// Constructors and Index have no JS representation:
/// both render as the literal `undefined`.
#[test]
fn js_constructors_and_index_render_undefined() {
    let some = js_of_hand(ret(IRNode::Call {
        function: "Some".into(),
        args: vec![var("x")],
    }));
    assert!(some.contains("return undefined;"), "{some}");
    let index = js_of_hand(ret(IRNode::Index(Box::new(var("x")), Box::new(lit(0)))));
    assert!(index.contains("return undefined;"), "{index}");
}

// ── The WAT target ─────────────────────────────────────

/// The WAT type map truncates Morphic Int to i32 in the
/// generated signature itself, and the Tier-1 body is a
/// comment — the exported function has no instructions.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn wat_int_abs_is_i32_and_comment_bodied() {
    let (spec, body) = entry("int_abs");
    let wat = codegen::generate(&candidate(&spec, body), "wasm").expect("WAT generation");
    assert!(wat.contains("(param x i32)"), "{wat}");
    assert!(wat.contains("(result i32)"), "{wat}");
    assert!(wat.contains(";; node"), "{wat}");
}

// ── The interpreter route (a Rust host today) ──────────

/// The shipped bodies evaluate directly through the
/// public interpreter with a hand-built Env — Tier-1
/// expression body, composition body, and a Tier-2 For
/// body all answer correctly. This is the whole of
/// today's "call a library function from Rust": IR in,
/// Value out, no name→call wrapper, no input typing.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn eval_route_calls_shipped_entries_at_ir_level() {
    let (_, abs_body) = entry("int_abs");
    let mut env: Env = HashMap::new();
    env.insert("x".into(), Value::Int(-5));
    assert_eq!(eval::eval(&abs_body, &env), Ok(Value::Int(5)));

    let (_, clamp_body) = entry("int_clamp");
    let mut env: Env = HashMap::new();
    env.insert("x".into(), Value::Int(15));
    env.insert("lo".into(), Value::Int(0));
    env.insert("hi".into(), Value::Int(10));
    assert_eq!(eval::eval(&clamp_body, &env), Ok(Value::Int(10)));

    let (_, sum_body) = entry("list_sum");
    let mut env: Env = HashMap::new();
    env.insert(
        "xs".into(),
        Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
    );
    assert_eq!(eval::eval(&sum_body, &env), Ok(Value::Int(6)));
}

// ── The Rust contrast ──────────────────────────────────

/// Generated Rust for the same Tier-1 entry is real,
/// typed, compilable code (Phase 116 pins the compile +
/// run) — and it carries NF-116-2 verbatim: the
/// precondition block asserts `true`.
#[cfg(not(feature = "wasm-playground"))]
#[test]
fn rust_int_abs_is_real_code_with_the_nf116_2_marker() {
    let (spec, body) = entry("int_abs");
    let rust = codegen::generate(&candidate(&spec, body), "rust").expect("Rust generation");
    assert!(rust.contains("pub fn int_abs(x: i64) -> i64"), "{rust}");
    assert!(
        rust.contains("debug_assert!(true, \"Precondition violated\")"),
        "NF-116-2 in the generated file:\n{rust}"
    );
}
