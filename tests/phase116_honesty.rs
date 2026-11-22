//! Phase 116 (cross-target honesty, STDLIB_SCOPE gate
//! clause 5): the silent free-call fallback dies for
//! library entries.
//!
//! Measured BEFORE this phase (probe, Phase 116): a body
//! calling a shipped library entry rendered on every
//! target as a silent wrong answer — Rust emitted a free
//! call to a function the generated file does not define
//! (`int_abs(x)`, failing only later at the user's own
//! rustc), C/WAT reduced the call to a comment, Python to
//! `pass # unsupported` / `None`, JavaScript to
//! `undefined`; the builtins `len` / `int_to_str` were
//! correct only on Rust and silently constant-folded to
//! `0` / `None` / `undefined` in expression position on
//! the other targets (measured in `list_min`'s own
//! generated C/Python/JS: `(0 == 0)`, `(None == 0)`,
//! `(undefined === 0)`).
//!
//! The pins below fix the combinations of the clause as
//! written — bodies whose calls involve the shipped
//! library (the 29 entries, plus the builtin tier `len` /
//! `int_to_str` of scope §2.1):
//!
//! * Rust renders correctly by construction: the used
//!   entries are emitted into the generated file as real
//!   functions (their specs + shipped bodies), so the
//!   generated Rust compiles and runs (the clause's
//!   stated measurement).
//! * C and WAT cannot render any library call (no length
//!   or aggregate model exists in either target) — they
//!   refuse at generation time with an error naming the
//!   call and the target.
//! * Python and JavaScript refuse entry calls the same
//!   way (neither emitter has a call rendering at all),
//!   and render the two builtins correctly in the
//!   expression positions their walkers serve
//!   (`len(x)`, `str(x)`; `[...(x)].length`, `String(x)`).
//! * Combinations NOT involving the library keep their
//!   standing behaviour exactly (pinned below for a
//!   plain sibling call and a plain BinOp body).
//! * The closed method table (case (c)) has no IR form:
//!   a body-position method call lowers to a Hole before
//!   codegen (Rust renders it loudly — TODO + panic), and
//!   in spec position the table renders verbatim through
//!   `expr_to_rust` (pinned in-module since Phases
//!   77/113); a checked method-table spec generates
//!   working Rust with no fallback gibberish. Both
//!   standing behaviours are pinned, unchanged. (One
//!   wider dishonesty measured adjacent to (c) — the
//!   precondition block degrades every Expr-form
//!   precondition to `true` — is finding NF-116-2 in the
//!   ROAD note, not this phase's fix: it is not
//!   method-specific.)

use morphic::codegen;
use morphic::spec::ast::FunctionSpec;
use morphic::spec::parser;
use morphic::stdlib;
use morphic::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, IRUnaryOp, Provenance, ScoreBreakdown,
};

fn candidate(spec: &FunctionSpec, body: IRNode) -> CandidateImplementation {
    CandidateImplementation {
        id: 116,
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

fn v(n: &str) -> IRNode {
    IRNode::Var(n.into())
}
fn i(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}
fn call(f: &str, args: Vec<IRNode>) -> IRNode {
    IRNode::Call {
        function: f.into(),
        args,
    }
}

const MODULE: &str = r#"
Probe {
spec call_abs {
    input: x: Int
    output: Int
    constraint: output == int_abs(x) + 1
    test "t" { input: (0 - 5) expect: 6 }
}
spec call_sum {
    input: xs: List<Int>
    output: Int
    constraint: output == list_sum(xs)
    test "t" { input: List(1, 2, 3) expect: 6 }
}
spec call_unwrap_or {
    input: o: Option<Int>
    output: Int
    constraint: output == option_unwrap_or(o, 7)
    test "none" { input: None() expect: 7 }
    test "some" { input: Some(3) expect: 3 }
}
spec min_abs {
    input: x: Int
    output: Int
    constraint: output == int_min(int_abs(x), 3)
    test "neg" { input: (0 - 5) expect: 3 }
    test "pos" { input: 2 expect: 2 }
}
spec stmt_abs {
    input: x: Int
    output: Int
    constraint: output == x
    test "t" { input: (0 - 5) expect: (0 - 5) }
}
spec call_search {
    input: xs: List<Int>
    input: x: Int
    output: Option<Int>
    constraint: output == list_binary_search(xs, x)
    test "t" { input: List(1, 2, 3) expect: Some(1) }
}
spec len_call {
    input: xs: List<Int>
    output: Int
    constraint: output == len(xs)
    test "t" { input: List(1, 2) expect: 2 }
}
spec len_str {
    input: s: String
    output: Bool
    constraint: output == (len(s) == 0)
    test "t" { input: "abc" expect: false }
}
spec to_str {
    input: x: Int
    output: String
    constraint: output == int_to_str(x)
    test "t" { input: 42 expect: "42" }
}
spec needs_ok {
    input: r: Result<Int, String>
    output: Bool
    require: r.is_ok() == true
    constraint: output == true
    test "t" { input: Ok(1) expect: true }
}
spec needs_some {
    input: o: Option<Int>
    output: Bool
    require: o.is_some() == true
    constraint: output == true
    test "t" { input: Some(1) expect: true }
}
}
"#;

fn spec_of(name: &str) -> FunctionSpec {
    let module = parser::parse(MODULE).expect("probe module parses");
    module
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("spec {name}"))
        .clone()
}

/// Generate Rust, compile it with rustc --test, run it,
/// and require every generated test to pass (the
/// Phase 115 compiled-route pattern).
fn rust_compiles_and_runs(label: &str, cand: &CandidateImplementation, tests: usize) -> String {
    let generated = codegen::generate(cand, "rust")
        .unwrap_or_else(|e| panic!("{label}: must render to Rust: {e}"));
    let dir = std::env::temp_dir().join(format!(
        "morphic-phase116-{label}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let src_path = dir.join(format!("{label}.rs"));
    std::fs::write(&src_path, &generated).unwrap();
    let bin_path = dir.join(format!("{label}_test"));
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
        "{label}: the generated Rust must compile:\n{}\n--- generated ---\n{generated}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = std::process::Command::new(&bin_path)
        .output()
        .expect("the compiled test binary runs");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success() && stdout.contains(&format!("test result: ok. {tests} passed")),
        "{label}: the compiled route must pass {tests}/{tests}, got: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
    generated
}

// ── (a) Rust: entry calls render correctly ──────────────

#[test]
fn rust_caller_of_tier1_entry_compiles_and_runs() {
    let cand = candidate(
        &spec_of("call_abs"),
        IRNode::BinOp(
            IRBinOp::Add,
            Box::new(call("int_abs", vec![v("x")])),
            Box::new(i(1)),
        ),
    );
    let generated = rust_compiles_and_runs("call_abs", &cand, 1);
    assert_eq!(
        generated.matches("pub fn int_abs(").count(),
        1,
        "the used entry is emitted exactly once as a real function:\n{generated}"
    );
}

#[test]
fn rust_caller_of_tier2_entry_compiles_and_runs() {
    let cand = candidate(&spec_of("call_sum"), call("list_sum", vec![v("xs")]));
    let generated = rust_compiles_and_runs("call_sum", &cand, 1);
    assert_eq!(generated.matches("pub fn list_sum(").count(), 1);
}

#[test]
fn rust_caller_of_option_entry_compiles_and_runs() {
    let cand = candidate(
        &spec_of("call_unwrap_or"),
        call("option_unwrap_or", vec![v("o"), i(7)]),
    );
    let generated = rust_compiles_and_runs("call_unwrap_or", &cand, 2);
    assert_eq!(generated.matches("pub fn option_unwrap_or(").count(), 1);
}

#[test]
fn rust_caller_of_two_entries_gets_each_helper_once() {
    let cand = candidate(
        &spec_of("min_abs"),
        call("int_min", vec![call("int_abs", vec![v("x")]), i(3)]),
    );
    let generated = rust_compiles_and_runs("min_abs", &cand, 2);
    assert_eq!(generated.matches("pub fn int_abs(").count(), 1);
    assert_eq!(generated.matches("pub fn int_min(").count(), 1);
}

#[test]
fn rust_statement_position_entry_call_compiles() {
    let cand = candidate(
        &spec_of("stmt_abs"),
        IRNode::Block(vec![
            call("int_abs", vec![v("x")]),
            IRNode::Return(Box::new(v("x"))),
        ]),
    );
    let generated = rust_compiles_and_runs("stmt_abs", &cand, 1);
    assert_eq!(generated.matches("pub fn int_abs(").count(), 1);
}

#[test]
fn rust_entry_candidate_gets_no_self_helper() {
    let spec = stdlib::prelude_specs()
        .into_iter()
        .find(|s| s.name == "string_is_empty")
        .unwrap();
    let body = stdlib::shipped_body("string_is_empty").unwrap();
    let generated = codegen::generate(&candidate(&spec, body), "rust").unwrap();
    assert_eq!(
        generated.matches("pub fn string_is_empty(").count(),
        1,
        "an entry generated for itself gains no duplicate helper"
    );
}

// ── (b) Rust: the builtins' standing correct arms ───────

#[test]
fn rust_builtins_render_correctly() {
    let len_call = candidate(&spec_of("len_call"), call("len", vec![v("xs")]));
    let out = codegen::generate(&len_call, "rust").unwrap();
    assert!(out.contains("(xs).len() as i64"), "{out}");

    let len_unary = candidate(
        &spec_of("len_call"),
        IRNode::UnaryOp(IRUnaryOp::Len, Box::new(v("xs"))),
    );
    let out = codegen::generate(&len_unary, "rust").unwrap();
    assert!(out.contains("xs.len()"), "{out}");

    let to_str = candidate(&spec_of("to_str"), call("int_to_str", vec![v("x")]));
    let out = codegen::generate(&to_str, "rust").unwrap();
    assert!(out.contains("(x).to_string()"), "{out}");
}

// ── (a)+(b) C / WAT: loud named refusal ─────────────────

#[test]
fn c_and_wasm_reject_library_calls_loudly() {
    let cases: Vec<(FunctionSpec, IRNode, &str)> = vec![
        (
            spec_of("call_abs"),
            call("int_abs", vec![v("x")]),
            "int_abs",
        ),
        (
            spec_of("call_search"),
            IRNode::Return(Box::new(call(
                "list_binary_search",
                vec![v("xs"), v("x")],
            ))),
            "list_binary_search",
        ),
        (spec_of("len_call"), call("len", vec![v("xs")]), "len"),
        (
            spec_of("len_call"),
            IRNode::UnaryOp(IRUnaryOp::Len, Box::new(v("xs"))),
            "len",
        ),
        (
            spec_of("to_str"),
            call("int_to_str", vec![v("x")]),
            "int_to_str",
        ),
    ];
    for (spec, body, name) in &cases {
        for target in ["c", "wasm"] {
            let err = codegen::generate(&candidate(spec, body.clone()), target)
                .err()
                .unwrap_or_else(|| {
                    panic!("{target} must refuse the library call {name} at generation time")
                });
            let msg = format!("{err}");
            assert!(
                msg.contains(name) && msg.contains(target),
                "the refusal names the call and the target: {msg}"
            );
        }
    }
}

#[test]
fn c_and_wasm_keep_standing_behaviour_without_library() {
    // A plain sibling call is not a library combination:
    // the standing renderings stand, byte class included.
    let sibling = candidate(
        &spec_of("call_abs"),
        IRNode::Return(Box::new(call("my_helper", vec![v("x")]))),
    );
    assert!(codegen::generate(&sibling, "c").is_ok());
    assert!(codegen::generate(&sibling, "wasm").is_ok());
    // A plain BinOp body likewise.
    let binop = candidate(
        &spec_of("call_abs"),
        IRNode::BinOp(IRBinOp::Add, Box::new(v("x")), Box::new(i(1))),
    );
    assert!(codegen::generate(&binop, "c").is_ok());
    assert!(codegen::generate(&binop, "wasm").is_ok());
}

// ── (a) Python / JS: loud named refusal for entries ─────

#[test]
fn python_and_js_reject_entry_calls_loudly() {
    let cand = candidate(&spec_of("call_abs"), call("int_abs", vec![v("x")]));
    for target in ["python", "javascript", "js"] {
        let err = codegen::generate(&cand, target)
            .err()
            .unwrap_or_else(|| panic!("{target} must refuse the entry call at generation time"));
        let msg = format!("{err}");
        assert!(
            msg.contains("int_abs") && msg.contains(target),
            "the refusal names the entry and the target: {msg}"
        );
    }
    // Nested inside a Return / Block, the refusal fires too.
    let nested = candidate(
        &spec_of("call_sum"),
        IRNode::Block(vec![IRNode::Return(Box::new(call(
            "list_sum",
            vec![v("xs")],
        )))]),
    );
    for target in ["python", "javascript"] {
        let err = codegen::generate(&nested, target).err().unwrap();
        assert!(format!("{err}").contains("list_sum"));
    }
}

// ── (b) Python / JS: builtins render in expression position

#[test]
fn python_and_js_render_builtins_in_expression_position() {
    // len over a String, in a Return'd equality — the
    // string_is_empty shape.
    let len_ret = candidate(
        &spec_of("len_str"),
        IRNode::Block(vec![IRNode::Return(Box::new(IRNode::BinOp(
            IRBinOp::Eq,
            Box::new(call("len", vec![v("s")])),
            Box::new(i(0)),
        )))]),
    );
    let py = codegen::generate(&len_ret, "python").unwrap();
    assert!(py.contains("len(s)"), "{py}");
    let js = codegen::generate(&len_ret, "javascript").unwrap();
    assert!(js.contains("[...(s)].length"), "{js}");

    // The UnaryOp form of len.
    let len_unary = candidate(
        &spec_of("len_call"),
        IRNode::Block(vec![IRNode::Return(Box::new(IRNode::UnaryOp(
            IRUnaryOp::Len,
            Box::new(v("xs")),
        )))]),
    );
    let py = codegen::generate(&len_unary, "python").unwrap();
    assert!(py.contains("len(xs)"), "{py}");
    let js = codegen::generate(&len_unary, "javascript").unwrap();
    assert!(js.contains("[...(xs)].length"), "{js}");

    // int_to_str.
    let to_str = candidate(
        &spec_of("to_str"),
        IRNode::Block(vec![IRNode::Return(Box::new(call(
            "int_to_str",
            vec![v("x")],
        )))]),
    );
    let py = codegen::generate(&to_str, "python").unwrap();
    assert!(py.contains("str(x)"), "{py}");
    let js = codegen::generate(&to_str, "javascript").unwrap();
    assert!(js.contains("String(x)"), "{js}");
}

#[test]
fn python_and_js_keep_standing_behaviour_for_nonlibrary_calls() {
    let cand = candidate(
        &spec_of("call_abs"),
        IRNode::Block(vec![IRNode::Return(Box::new(call(
            "my_helper",
            vec![v("x")],
        )))]),
    );
    let py = codegen::generate(&cand, "python").unwrap();
    assert!(py.contains("return None"), "{py}");
    let js = codegen::generate(&cand, "javascript").unwrap();
    assert!(js.contains("return undefined"), "{js}");
}

// ── (c) The closed method table ─────────────────────────

#[test]
fn method_table_specs_generate_working_rust() {
    // In spec position the closed table's rendering
    // surface inside generate() is expr_to_rust (test
    // inputs/expected outputs, precondition asserts):
    // a checked spec using the table generates Rust with
    // no fallback gibberish (the '??' catch-all never
    // fires for a table name), and the file compiles and
    // runs. (The precondition block's separate standing
    // behaviour — every Expr-form precondition degrades
    // to `true` in constraint_to_rust, method-bearing or
    // not — is recorded as finding NF-116-2 in the ROAD
    // Phase 116 note; it is not method-specific and its
    // fix belongs to a spec-position rendering pass.)
    for name in ["needs_ok", "needs_some"] {
        let cand = candidate(
            &spec_of(name),
            IRNode::Literal(IRLiteral::Bool(true)),
        );
        let generated = rust_compiles_and_runs(name, &cand, 1);
        assert!(
            !generated.contains("??"),
            "no fallback gibberish for a closed-table spec:\n{generated}"
        );
    }
}

#[test]
fn hole_body_is_loud_in_rust() {
    // A body-position method call has no IR kind; what
    // reaches codegen is the Hole the lowering leaves,
    // and Rust renders it loudly — never a silent value.
    let cand = candidate(
        &spec_of("needs_ok"),
        IRNode::Hole {
            id: 1,
            expected_type: None,
        },
    );
    let out = codegen::generate(&cand, "rust").unwrap();
    assert!(out.contains("panic!(\"Hole not filled\")"), "{out}");
}

// ── The IR debug target names what it renders ───────────

#[test]
fn ir_target_names_library_calls() {
    let cand = candidate(&spec_of("call_abs"), call("int_abs", vec![v("x")]));
    let out = codegen::generate(&cand, "ir").unwrap();
    assert!(out.contains("int_abs"), "{out}");
}
