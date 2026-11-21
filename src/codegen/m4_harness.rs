// ─────────────────────────────────────────────────────────────
//  M4-S1 COMPOSED-COMPILER HARNESS (test-only) — Phase 44:
//  the reusable build-path instrument the rest of M4 runs on.
//
//  WHAT IT IS. The Phase 43 audit defined M4's composition:
//  the original pipeline (parse → typeck →
//  synthesize_with_module → verify → select) with the M3
//  composed codegen — the mirror-completed composition of
//  src/codegen/m3_gate.rs (mode B: winners + assembly glue +
//  the mirror-pinned F5 layer) — seated in
//  `codegen::generate`'s place. This harness is that
//  composition as a callable instrument: give it a .morph
//  file and a spec name and it produces the spec's generated
//  Rust file TWICE — once from the original `generate`, once
//  from the composed codegen — and byte-compares the two
//  whole files (driver included, not per-arm renderings;
//  that whole-file unit is what makes M4 a new gate rather
//  than an M3 re-run).
//
//  THE ONE DESIGN RULE — EXTRACT ONCE, RENDER TWICE. The
//  audit measured winner selection to be DRAW-DEPENDENT at
//  corpus scale: three consecutive `morphic build` runs of
//  sort.morph crowned three different winners, so building
//  twice and comparing files compares two different programs
//  and proves nothing. This harness therefore runs the
//  pipeline for a (file, spec) pair EXACTLY ONCE, caches the
//  single selected winner, and renders THAT winner body
//  through both codegens. The cache is structural, not a
//  convention: `dual_render` can only reach the pipeline
//  through `WinnerCache::winner_for`, which invokes the
//  pipeline closure at most once per key. The mechanics
//  tests pin this (a counting closure whose second
//  invocation would return a DIFFERENT body is never invoked
//  twice), and the end-to-end test asserts the run counter
//  in situ.
//
//  PLACEMENT. A sibling of m3_gate inside the codegen
//  module, on the same precedent: test-only, declared in
//  src/codegen/mod.rs, reusing the M3 gate's machinery by
//  reference (the items it needs are `pub(super)` there)
//  rather than copying it — the winner extraction, the
//  winner executor, and the `Composed` emitter are the
//  gate's own, so S2/S3 judge the same composition the M3
//  gate judged. The original side here is `super::generate`
//  exactly as src/main.rs's Build invokes it (opt level 1:
//  the composed compiler is an opt-1 compiler, as every
//  measurement since M2 has been).
//
//  WHAT A RESULT MEANS. `DualRender` records the winner's
//  provenance and a structural key of its body — the
//  identity evidence that both renderings render THIS body —
//  plus each side's outcome (text or error) and, when both
//  sides produced text, the structured byte comparison:
//  identical, or the first divergence's byte offset, line,
//  column, and context windows from both files. A side
//  ERRORING is a recorded outcome, not a harness failure:
//  at S1 a characterized divergence (or composition error)
//  is a valid measurement; the harness's own job is to make
//  it impossible to confuse which layer spoke.
//
//  Run the end-to-end proof with:
//    cargo test -- --ignored --nocapture m4_s1
//  (report at target/m4-harness/s1-report.md)
// ─────────────────────────────────────────────────────────────

use super::generate;
use super::m3_gate::{
    bin, hand_entry, hand_set, lb, ls, make_candidate, vr, Composed, WinnerSet,
};
use crate::spec::ast::{FunctionSpec, TypeRef};
use crate::spec::{parser, typeck};
use crate::synthesis::engine::{
    CandidateImplementation, IRBinOp, IRLiteral, IRNode, IRPattern,
};
use crate::synthesis::{engine, selector};
use crate::verify;
use std::collections::HashMap;

// ── Structured byte comparison ────────────────────────────

/// The verdict of comparing two whole generated files.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum FileComparison {
    Identical,
    Divergent(Divergence),
}

/// The first byte at which two files disagree, located in
/// the original file's line/column terms, with a context
/// window from each side. `original_ended` /
/// `composed_ended` mark the prefix case: the divergence
/// offset is that side's end of file.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Divergence {
    pub byte: usize,
    pub line: usize,
    pub column: usize,
    pub original_len: usize,
    pub composed_len: usize,
    pub original_context: String,
    pub composed_context: String,
    pub original_ended: bool,
    pub composed_ended: bool,
}

impl Divergence {
    /// The report rendering of this divergence (the shape
    /// the mechanics test pins).
    pub fn describe(&self) -> String {
        let end = |ended: bool| if ended { " (end of file)" } else { "" };
        format!(
            "first diff at byte {} (line {}, column {}) — original {} bytes, composed {} bytes\n    original: …{}{}\n    composed: …{}{}",
            self.byte,
            self.line,
            self.column,
            self.original_len,
            self.composed_len,
            self.original_context,
            end(self.original_ended),
            self.composed_context,
            end(self.composed_ended),
        )
    }
}

/// Compare two generated files byte-for-byte; on mismatch,
/// locate the first differing byte and capture context
/// windows (28 bytes before, 36 from the divergence) from
/// both sides.
pub(super) fn compare_files(original: &str, composed: &str) -> FileComparison {
    if original == composed {
        return FileComparison::Identical;
    }
    let (a, b) = (original.as_bytes(), composed.as_bytes());
    let byte = a
        .iter()
        .zip(b.iter())
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.len().min(b.len()));
    // Line/column of the divergence in the ORIGINAL text
    // (1-based; column counts bytes within the line).
    let before = &original[..byte.min(a.len())];
    let line = before.matches('\n').count() + 1;
    let column = byte - before.rfind('\n').map(|i| i + 1).unwrap_or(0) + 1;
    let window = |bytes: &[u8]| {
        let lo = byte.saturating_sub(28);
        let hi = (byte + 36).min(bytes.len());
        String::from_utf8_lossy(&bytes[lo..hi]).into_owned()
    };
    FileComparison::Divergent(Divergence {
        byte,
        line,
        column,
        original_len: a.len(),
        composed_len: b.len(),
        original_context: window(a),
        composed_context: window(b),
        original_ended: byte >= a.len(),
        composed_ended: byte >= b.len(),
    })
}

// ── Single-extraction winner cache ────────────────────────

/// The harness's only door to the pipeline. A (file, spec)
/// key's pipeline closure runs AT MOST ONCE per cache; every
/// later request for the key returns the cached winner —
/// the same body both codegens will render. `pipeline_runs`
/// counts closure invocations so tests can prove the
/// extraction happened exactly once.
#[derive(Default)]
pub(super) struct WinnerCache {
    entries: HashMap<String, CandidateImplementation>,
    pipeline_runs: usize,
}

impl WinnerCache {
    pub(super) fn winner_for(
        &mut self,
        key: &str,
        run: impl FnOnce() -> Result<CandidateImplementation, String>,
    ) -> Result<CandidateImplementation, String> {
        if let Some(winner) = self.entries.get(key) {
            return Ok(winner.clone());
        }
        self.pipeline_runs += 1;
        let winner = run()?;
        self.entries.insert(key.to_string(), winner.clone());
        Ok(winner)
    }

    pub(super) fn pipeline_runs(&self) -> usize {
        self.pipeline_runs
    }
}

/// Structural key of a winner body (its Debug text) — the
/// identity evidence two renderings share.
pub(super) fn body_key(winner: &CandidateImplementation) -> String {
    format!("{:?}", winner.body)
}

/// A short stable hash of a body key, for report lines
/// (the full Debug text of a real winner body is far too
/// long to print).
pub(super) fn short_hash(text: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

// ── The pipeline, run once per (file, spec) ───────────────

/// Parse + typecheck a .morph file (path relative to the
/// crate root) into its module functions. Deterministic and
/// cheap — NOT part of winner extraction; it may run any
/// number of times. The expensive, draw-dependent part is
/// `run_pipeline_once`, and it runs once.
pub(super) fn load_functions(rel_path: &str) -> Result<Vec<FunctionSpec>, String> {
    let root = env!("CARGO_MANIFEST_DIR");
    let path = format!("{}/{}", root, rel_path);
    let source = std::fs::read_to_string(&path)
        .map_err(|e| format!("{}: read failed: {}", rel_path, e))?;
    let parsed = parser::parse(&source)
        .map_err(|e| format!("{}: parse failed: {}", rel_path, e))?;
    let checked = typeck::check(parsed)
        .map_err(|e| format!("{}: typeck failed: {}", rel_path, e))?;
    Ok(checked.spec.functions)
}

/// The real build pipeline for ONE spec, exactly as
/// src/main.rs's Build runs it (module-aware synthesis at
/// opt level 1 → module-aware Z3 verification → selection),
/// returning the single selected winner. Synthesis panics
/// are caught and reported, on the M3 gate's precedent.
pub(super) fn run_pipeline_once(
    functions: &[FunctionSpec],
    spec_name: &str,
) -> Result<CandidateImplementation, String> {
    let func = functions
        .iter()
        .find(|f| f.name == spec_name)
        .ok_or_else(|| {
            format!(
                "no spec named {} (module has: {})",
                spec_name,
                functions
                    .iter()
                    .map(|f| f.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine::synthesize_with_module(func, 1, functions)
    }));
    let candidates = match outcome {
        Ok(Ok(c)) if !c.is_empty() => c,
        Ok(Ok(_)) => return Err(format!("{}: synthesis returned no candidates", spec_name)),
        Ok(Err(e)) => return Err(format!("{}: synthesis error: {}", spec_name, e)),
        Err(payload) => {
            return Err(format!(
                "{}: synthesis panicked: {}",
                spec_name,
                panic_text(&payload)
            ))
        }
    };
    let verified = verify::verifier::Verifier::with_specs(functions)
        .verify_all(candidates, func)
        .map_err(|e| format!("{}: verify_all error: {}", spec_name, e))?;
    selector::select_best(verified, func)
        .map_err(|e| format!("{}: select_best error: {}", spec_name, e))
}

pub(super) fn panic_text(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

// ── The two renderings of one winner ──────────────────────

/// The original side: `codegen::generate` on the winner,
/// target "rust", exactly as `morphic build` renders it.
fn render_original(winner: &CandidateImplementation) -> Result<String, String> {
    generate(winner, "rust").map_err(|e| format!("{}", e))
}

/// The composed side: the M3 mirror-completed composition's
/// whole-file driver over the SAME winner. Execution
/// failures arrive as Err; a panic is caught and reported
/// as the composed side's outcome, never propagated — the
/// harness records which layer spoke.
fn render_composed(
    composed: &Composed,
    winner: &CandidateImplementation,
) -> Result<String, String> {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        composed.driver_file(winner)
    }));
    match outcome {
        Ok(result) => result,
        Err(payload) => Err(format!(
            "composed codegen panicked: {}",
            panic_text(&payload)
        )),
    }
}

// ── The composed compiler ─────────────────────────────────

/// One dual rendering: the winner both sides rendered (its
/// provenance and body key are the identity evidence), each
/// side's outcome, and the byte comparison when both sides
/// produced text.
pub(super) struct DualRender {
    pub file: String,
    pub spec_name: String,
    pub winner_provenance: String,
    pub winner_body_key: String,
    pub original: Result<String, String>,
    pub composed: Result<String, String>,
    pub comparison: Option<FileComparison>,
}

impl DualRender {
    /// The one-line verdict for reports.
    pub fn verdict(&self) -> String {
        match (&self.original, &self.composed, &self.comparison) {
            (_, _, Some(FileComparison::Identical)) => "IDENTICAL".to_string(),
            (_, _, Some(FileComparison::Divergent(d))) => {
                format!("DIVERGENT — {}", d.describe().replace('\n', " "))
            }
            (Err(e), _, _) => format!("ORIGINAL ERROR — {}", e),
            (_, Err(e), _) => format!("COMPOSED ERROR — {}", e),
            _ => "NOT COMPARED".to_string(),
        }
    }
}

/// The composed-compiler harness: the M3 composed codegen
/// (borrowed) plus the single-extraction winner cache.
pub(super) struct ComposedCompiler<'a> {
    composed: Composed<'a>,
    cache: WinnerCache,
}

impl<'a> ComposedCompiler<'a> {
    pub fn new(winners: &'a WinnerSet) -> Self {
        ComposedCompiler {
            // Mode B — the mirror-completed composition, the
            // composition as the project's records define it.
            composed: Composed {
                w: winners,
                escape_literals: true,
            },
            cache: WinnerCache::default(),
        }
    }

    /// Extract (file, spec)'s winner ONCE and render that
    /// single winner through both codegens. Extraction or
    /// loading failures are Err; rendering outcomes —
    /// including a side erroring — are recorded in the
    /// returned DualRender.
    pub fn dual_render(
        &mut self,
        rel_path: &str,
        spec_name: &str,
    ) -> Result<DualRender, String> {
        let functions = load_functions(rel_path)?;
        let key = format!("{}::{}", rel_path, spec_name);
        let winner = self
            .cache
            .winner_for(&key, || run_pipeline_once(&functions, spec_name))?;
        let original = render_original(&winner);
        let composed = render_composed(&self.composed, &winner);
        let comparison = match (&original, &composed) {
            (Ok(a), Ok(b)) => Some(compare_files(a, b)),
            _ => None,
        };
        Ok(DualRender {
            file: rel_path.to_string(),
            spec_name: spec_name.to_string(),
            winner_provenance: format!("{:?}", winner.provenance),
            winner_body_key: body_key(&winner),
            original,
            composed,
            comparison,
        })
    }

    /// How many times the pipeline actually ran inside this
    /// compiler — the single-extraction evidence.
    pub fn pipeline_runs(&self) -> usize {
        self.cache.pipeline_runs()
    }
}

// ── Mechanics tests (not ignored) ─────────────────────────
//
// The harness must provably do what it says on hand-built
// inputs, without synthesis: single extraction, selection
// determinism on a fixed pool, the dual-render plumbing,
// and the divergence report's shape.

#[test]
fn harness_single_extraction_enforced() {
    use std::cell::Cell;
    // Two hand winners with DIFFERENT bodies: if the cache
    // ever ran the pipeline twice for one key, the second
    // result would carry the second body.
    let w_true = make_candidate(
        "pick",
        None,
        vec![],
        vec![("n", TypeRef::Int)],
        TypeRef::Bool,
        vec![],
        lb(true),
        vec![],
    );
    let w_false = make_candidate(
        "pick",
        None,
        vec![],
        vec![("n", TypeRef::Int)],
        TypeRef::Bool,
        vec![],
        lb(false),
        vec![],
    );
    let calls = Cell::new(0);
    let mut cache = WinnerCache::default();
    let first = cache
        .winner_for("file::pick", || {
            calls.set(calls.get() + 1);
            Ok(w_true.clone())
        })
        .unwrap();
    let second = cache
        .winner_for("file::pick", || {
            calls.set(calls.get() + 1);
            Ok(w_false.clone())
        })
        .unwrap();
    assert_eq!(body_key(&first), body_key(&second));
    assert_eq!(calls.get(), 1, "the pipeline closure ran more than once");
    assert_eq!(cache.pipeline_runs(), 1);
    // A different key is a different extraction.
    let third = cache
        .winner_for("other::pick", || {
            calls.set(calls.get() + 1);
            Ok(w_false.clone())
        })
        .unwrap();
    assert_ne!(body_key(&first), body_key(&third));
    assert_eq!(cache.pipeline_runs(), 2);
}

#[test]
fn harness_winner_pick_deterministic_on_same_pool() {
    // The selection step the harness's pipeline ends in
    // (selector::select_best, the exact call
    // run_pipeline_once makes) over an IDENTICAL pool state
    // crowns an identical body — extraction determinism at
    // the selection layer, complementing the cache test.
    let mut low = make_candidate(
        "pick",
        None,
        vec![],
        vec![("n", TypeRef::Int)],
        TypeRef::Bool,
        vec![],
        lb(true),
        vec![],
    );
    low.scores.test_pass_ratio = 0.5;
    let mut high = make_candidate(
        "pick",
        None,
        vec![],
        vec![("n", TypeRef::Int)],
        TypeRef::Bool,
        vec![],
        lb(false),
        vec![],
    );
    high.scores.test_pass_ratio = 1.0;
    let spec = low.spec.clone();
    let pool = vec![low, high];
    let high_key = body_key(&pool[1]);
    let a = selector::select_best(pool.clone(), &spec).unwrap();
    let b = selector::select_best(pool, &spec).unwrap();
    assert_eq!(body_key(&a), body_key(&b));
    // And the crowned body is the higher-scoring one.
    assert_eq!(body_key(&a), high_key);
}

#[test]
fn harness_dual_render_plumbing_hand_built() {
    // A hand-built winner candidate with known output, and
    // a hand-built Codegen winner set carrying exactly the
    // specs the composed driver consults for it (partial
    // tables in the M3 mechanics style — the plumbing is
    // what is pinned here; the real winner set is the
    // end-to-end test's business). Both renderings must
    // produce the known file, byte-identical.
    let concat = |l: IRNode, r: IRNode| bin(IRBinOp::Concat, l, r);
    let section = hand_entry(
        "rust_driver_section_text",
        vec![("present", TypeRef::Bool), ("text", TypeRef::String)],
        TypeRef::String,
        IRNode::Match {
            scrutinee: Box::new(vr("present")),
            cases: vec![
                (IRPattern::Literal(IRLiteral::Bool(true)), vr("text")),
                (IRPattern::Wildcard, ls("")),
            ],
        },
    );
    let generics = hand_entry(
        "rust_generics_text",
        vec![("joined", TypeRef::String), ("present", TypeRef::Bool)],
        TypeRef::String,
        IRNode::Match {
            scrutinee: Box::new(vr("present")),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Bool(true)),
                    concat(concat(ls("<"), vr("joined")), ls(">")),
                ),
                (IRPattern::Wildcard, ls("")),
            ],
        },
    );
    let type_name = hand_entry(
        "rust_type_name",
        vec![("tag", TypeRef::Int)],
        TypeRef::String,
        IRNode::Match {
            scrutinee: Box::new(vr("tag")),
            cases: vec![
                (IRPattern::Literal(IRLiteral::Int(0)), ls("i64")),
                (IRPattern::Literal(IRLiteral::Int(2)), ls("bool")),
                (IRPattern::Wildcard, ls("")),
            ],
        },
    );
    let param = hand_entry(
        "rust_param_text",
        vec![("name", TypeRef::String), ("ty", TypeRef::String)],
        TypeRef::String,
        concat(concat(vr("name"), ls(": ")), vr("ty")),
    );
    let signature = hand_entry(
        "rust_signature",
        vec![
            ("name", TypeRef::String),
            ("generics", TypeRef::String),
            ("params", TypeRef::String),
            ("ret", TypeRef::String),
        ],
        TypeRef::String,
        concat(
            concat(
                concat(
                    concat(
                        concat(concat(ls("pub fn "), vr("name")), vr("generics")),
                        ls("("),
                    ),
                    vr("params"),
                ),
                ls(") -> "),
            ),
            concat(vr("ret"), ls(" {\n")),
        ),
    );
    let tail_form = hand_entry(
        "rust_tail_form",
        vec![("kind", TypeRef::Int)],
        TypeRef::Int,
        IRNode::Match {
            scrutinee: Box::new(vr("kind")),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(9)),
                    IRNode::Literal(IRLiteral::Int(1)),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Int(0)),
                ),
            ],
        },
    );
    let literal_bool = hand_entry(
        "rust_literal_bool",
        vec![("value", TypeRef::Bool)],
        TypeRef::String,
        IRNode::Match {
            scrutinee: Box::new(vr("value")),
            cases: vec![
                (IRPattern::Literal(IRLiteral::Bool(true)), ls("true")),
                (IRPattern::Literal(IRLiteral::Bool(false)), ls("false")),
            ],
        },
    );
    let tail_expr = hand_entry(
        "rust_tail_expr_text",
        vec![("pad", TypeRef::String), ("rendered", TypeRef::String)],
        TypeRef::String,
        concat(concat(vr("pad"), vr("rendered")), ls("\n")),
    );
    let file_text = hand_entry(
        "rust_file_text",
        vec![
            ("header", TypeRef::String),
            ("pre_block", TypeRef::String),
            ("signature", TypeRef::String),
            ("body", TypeRef::String),
            ("test_module", TypeRef::String),
        ],
        TypeRef::String,
        concat(
            concat(
                concat(
                    concat(
                        concat(vr("header"), vr("pre_block")),
                        vr("signature"),
                    ),
                    vr("body"),
                ),
                ls("}\n"),
            ),
            vr("test_module"),
        ),
    );
    let set = hand_set(vec![
        section,
        generics,
        type_name,
        param,
        signature,
        tail_form,
        literal_bool,
        tail_expr,
        file_text,
    ]);
    let composed = Composed {
        w: &set,
        escape_literals: true,
    };
    let cand = make_candidate(
        "answer",
        None,
        vec![],
        vec![("n", TypeRef::Int)],
        TypeRef::Bool,
        vec![],
        lb(true),
        vec![],
    );
    let expected = "pub fn answer(n: i64) -> bool {\n    true\n}\n";
    let original = render_original(&cand).unwrap();
    assert_eq!(original, expected);
    let composed_text = render_composed(&composed, &cand).unwrap();
    assert_eq!(composed_text, expected);
    assert_eq!(
        compare_files(&original, &composed_text),
        FileComparison::Identical
    );
}

#[test]
fn harness_divergence_report_shape() {
    assert_eq!(compare_files("abc", "abc"), FileComparison::Identical);
    // Mid-file divergence: byte, line, column, contexts.
    let FileComparison::Divergent(d) = compare_files("abcXdef", "abcYdef") else {
        panic!("expected a divergence");
    };
    assert_eq!(d.byte, 3);
    assert_eq!(d.line, 1);
    assert_eq!(d.column, 4);
    assert!(d.original_context.contains('X'));
    assert!(d.composed_context.contains('Y'));
    assert!(!d.original_ended && !d.composed_ended);
    // Line/column track newlines in the original.
    let FileComparison::Divergent(d) = compare_files("l1\nl2\nl3X", "l1\nl2\nl3Y") else {
        panic!("expected a divergence");
    };
    assert_eq!((d.byte, d.line, d.column), (8, 3, 3));
    // Prefix case: the shorter side ended at the divergence.
    let FileComparison::Divergent(d) = compare_files("abc", "abcd") else {
        panic!("expected a divergence");
    };
    assert_eq!(d.byte, 3);
    assert!(d.original_ended);
    assert!(!d.composed_ended);
    assert!(d.describe().contains("end of file"));
    // Empty vs non-empty diverges at byte 0.
    let FileComparison::Divergent(d) = compare_files("", "x") else {
        panic!("expected a divergence");
    };
    assert_eq!((d.byte, d.line, d.column), (0, 1, 1));
    assert!(d.original_ended);
}

// ── End-to-end proof (ignored) ────────────────────────────
//
// S1's proof on a small real set: the four standalone
// warm-up/example specs. The Codegen winner set is
// extracted once (the M3 gate's own extraction), each
// target's winner is extracted once, and the single winner
// is dual-rendered and byte-compared as a whole file.
// Divergences and side errors are REPORTED per file with
// their divergence detail — S1 characterizes; it does not
// silently require a pass. What this test asserts is the
// instrument's own contract: every file produced a record,
// and the pipeline ran exactly once per file (sort is
// rendered twice; the second render must hit the cache).

#[test]
#[ignore = "M4-S1: Codegen winner extraction (123 specs) + 4 example pipelines; run explicitly with --ignored --nocapture"]
fn m4_s1_examples_dual_render() {
    let root = env!("CARGO_MANIFEST_DIR");
    let out_dir = format!("{}/target/m4-harness", root);
    std::fs::create_dir_all(&out_dir).unwrap();

    let codegen_functions =
        load_functions("examples/self_host/codegen.morph").expect("codegen module loads");
    let winners = super::m3_gate::extract_winners(&codegen_functions);
    assert_eq!(
        winners.spec_count(),
        123,
        "the composed side must be the M3 winner set (123 Codegen specs)"
    );
    let mut compiler = ComposedCompiler::new(&winners);

    let targets: [(&str, &str); 4] = [
        ("examples/sort.morph", "sort"),
        ("examples/binary_search.morph", "binary_search"),
        ("examples/self_host/is_sorted.morph", "is_sorted"),
        ("examples/self_host/count_matching.morph", "count_matching"),
    ];
    let mut report = String::new();
    report.push_str("# M4-S1 dual-render report (Phase 44)\n\n");
    let mut records = 0;
    let mut sort_body_key: Option<String> = None;
    for (file, spec) in targets {
        match compiler.dual_render(file, spec) {
            Ok(render) => {
                records += 1;
                if spec == "sort" {
                    sort_body_key = Some(render.winner_body_key.clone());
                }
                let line = format!(
                    "- `{}` spec `{}` — winner {} [body {}] — {}",
                    render.file,
                    render.spec_name,
                    render.winner_provenance,
                    short_hash(&render.winner_body_key),
                    render.verdict(),
                );
                println!("{}", line);
                report.push_str(&line);
                report.push('\n');
                if let Some(FileComparison::Divergent(d)) = &render.comparison {
                    report.push_str(&format!("  {}\n", d.describe().replace('\n', "\n  ")));
                }
            }
            Err(e) => {
                records += 1;
                let line = format!("- `{}` spec `{}` — EXTRACTION ERROR — {}", file, spec, e);
                println!("{}", line);
                report.push_str(&line);
                report.push('\n');
            }
        }
    }
    // The in-situ single-extraction proof: rendering sort a
    // second time must return the cached winner — same body
    // key, no new pipeline run.
    let again = compiler
        .dual_render("examples/sort.morph", "sort")
        .expect("second sort render");
    assert_eq!(
        Some(again.winner_body_key),
        sort_body_key,
        "the second render of sort must carry the SAME winner body"
    );
    assert_eq!(
        compiler.pipeline_runs(),
        4,
        "the pipeline must run exactly once per file (4 files)"
    );
    report.push_str(&format!(
        "\nPipeline runs: {} for {} files (sort rendered twice; second render served from the cache).\n",
        compiler.pipeline_runs(),
        records,
    ));
    std::fs::write(format!("{}/s1-report.md", out_dir), &report).unwrap();
    assert_eq!(records, 4, "every target file must produce a record");
}

// ── The M4 gate (ignored) ─────────────────────────────────
//
// S3: the corpus scale-up. The standing gate test for M4,
// on the M3 gate precedent (kept ignored, explicit run
// command, report artifact — this is also the permanent
// shape S2's self-compile item takes: the S1 proof above
// stays as the small standing test, and sort.morph is a
// corpus member here).
//
// THE CORPUS, stated before any run (Phase 46 policies):
// every spec in the repo's .morph inventory whose file
// passes `morphic check` (parse + typeck — exactly
// `load_functions` below). The inventory is DERIVED by
// scanning examples/ at run time, never hardcoded; the
// enumeration is asserted as counts, so an inventory
// change fails this test loudly instead of silently
// re-scoping the gate. As closed in Phase 46 the corpus
// was 130 specs in 7 files, with
// examples/functional_tokenizer.morph (8 specs) the one
// recorded exclusion — it failed check, and repairing it
// was S4's business. Phase 47 (S4 part 1) re-authored that
// file into the accepted dialect; it passes check now, so
// the corpus is the full inventory — 138 specs in 8
// files — and the exclusion set is empty. Phase 56
// re-record (2026-11-21): the Phase 55 amendments grew
// codegen.morph from 112 to 116 specs, so the corpus is
// now 142 specs in 8 files (derivation: 138 + 4 new
// Codegen specs; per-file counts in EXPECTED_CORPUS).
// Phase 62 re-record (2026-11-21): the Clone-bound
// amendment adds rust_generic_param_text — codegen.morph
// holds 117 specs and the corpus is 143 (derivation:
// 142 + 1 new Codegen spec). Phase 78 re-record
// (2026-11-22): the new-kind amendment adds six specs —
// codegen.morph holds 123 specs and the corpus is 149
// (derivation: 143 + 6 new Codegen specs). The tokenizer's
// inclusion is sound on the harness's own terms: the dual
// render judges rendering identity of whatever winner the
// pipeline crowns for each spec, per extracted winner.
// The Codegen module's own 123 specs ARE corpus members:
// the gate demands 100+ byte-identical specs and the
// non-Codegen corpus is only 26, so the arithmetic forces
// inclusion; rendering a Codegen spec's winner through
// the composed codegen is the M3 gate's own self-reference
// (its item (iv) rendered all 123 winner files end-to-end
// through the composed driver), already proven there.
// The composition judged is the mirror-completed one
// (mode B), per the Phase 43 ledger: a strict-composition
// reading would inherit M3's recorded F5 divergences.
//
// Per (file, spec): ONE winner extraction through the real
// pipeline, that single winner rendered by the original
// `generate` and by the composed codegen, whole files
// byte-compared (the harness's design rule — winner
// selection is draw-dependent, so verdicts are per
// extracted winner, never per double build).
//
// Run the gate with:
//   cargo test -- --ignored --nocapture m4_gate
// (report at target/m4-harness/gate-report.md, rewritten
// after each file so a partial run stays informative)

/// The audited inventory of check-passing files, as
/// (path, spec count) — asserted against the derived
/// enumeration so the gate's corpus cannot drift silently.
/// (Phase 47: functional_tokenizer.morph joined the corpus
/// when its re-authoring made it pass check.)
const EXPECTED_CORPUS: [(&str, usize); 8] = [
    ("examples/binary_search.morph", 1),
    ("examples/functional_tokenizer.morph", 8),
    ("examples/self_host/codegen.morph", 123),
    ("examples/self_host/count_matching.morph", 1),
    ("examples/self_host/is_sorted.morph", 1),
    ("examples/self_host/printer.morph", 7),
    ("examples/self_host/validator.morph", 7),
    ("examples/sort.morph", 1),
];

/// Files the scan must find but `morphic check` must
/// reject (the recorded exclusion). Empty since Phase 47:
/// every file in the inventory passes check.
const EXPECTED_EXCLUDED: [&str; 0] = [];

/// Recursively collect the repo's .morph files under
/// examples/, as crate-relative paths in sorted order.
fn scan_morph_files() -> Vec<String> {
    fn walk(dir: &std::path::Path, prefix: &str, out: &mut Vec<String>) {
        let mut entries: Vec<std::fs::DirEntry> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("cannot scan {}: {}", dir.display(), e))
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = format!("{}/{}", prefix, name);
            let path = entry.path();
            if path.is_dir() {
                walk(&path, &rel, out);
            } else if name.ends_with(".morph") {
                out.push(rel);
            }
        }
    }
    let root = env!("CARGO_MANIFEST_DIR");
    let mut found = Vec::new();
    walk(
        std::path::Path::new(&format!("{}/examples", root)),
        "examples",
        &mut found,
    );
    found.sort();
    found
}

/// One check-passing corpus file and its spec names, in
/// module order.
pub(super) struct CorpusFile {
    pub(super) rel_path: String,
    pub(super) spec_names: Vec<String>,
}

/// Derive the corpus: scan the inventory, keep the files
/// that pass check (parse + typeck, `load_functions`),
/// record the rest as excluded. Asserts the derived
/// corpus IS the audited one — paths, per-file spec
/// counts, the exclusion, and the 149-spec total.
pub(super) fn enumerate_corpus() -> (Vec<CorpusFile>, Vec<String>) {
    let scanned = scan_morph_files();
    assert_eq!(
        scanned.len(),
        EXPECTED_CORPUS.len() + EXPECTED_EXCLUDED.len(),
        "the .morph inventory changed (found {:?}); re-audit and re-record before running the gate",
        scanned,
    );
    let mut files: Vec<CorpusFile> = Vec::new();
    let mut excluded: Vec<String> = Vec::new();
    for rel in &scanned {
        match load_functions(rel) {
            Ok(functions) => files.push(CorpusFile {
                rel_path: rel.clone(),
                spec_names: functions.iter().map(|f| f.name.clone()).collect(),
            }),
            Err(_) => excluded.push(rel.clone()),
        }
    }
    assert_eq!(
        excluded,
        EXPECTED_EXCLUDED
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        "the set of check-failing files changed (excluded: {:?}); re-audit and re-record",
        excluded,
    );
    let mut total = 0;
    for (path, count) in EXPECTED_CORPUS {
        let file = files
            .iter()
            .find(|f| f.rel_path == path)
            .unwrap_or_else(|| panic!("expected corpus file {} missing", path));
        assert_eq!(
            file.spec_names.len(),
            count,
            "{}: spec count changed ({} != {}); re-audit and re-record",
            path,
            file.spec_names.len(),
            count,
        );
        total += count;
    }
    assert_eq!(total, 149, "the gate corpus must hold 149 specs");
    (files, excluded)
}

/// Per-file gate tallies.
#[derive(Default)]
struct FileTally {
    attempted: usize,
    identical: usize,
    divergent: usize,
    original_error: usize,
    composed_error: usize,
    extraction_error: usize,
}

#[test]
#[ignore = "M4 gate: corpus-scale dual render (149 specs, minutes); run explicitly with --ignored --nocapture m4_gate"]
fn m4_gate_corpus_dual_render() {
    let started = std::time::Instant::now();
    let root = env!("CARGO_MANIFEST_DIR");
    let out_dir = format!("{}/target/m4-harness", root);
    std::fs::create_dir_all(&out_dir).unwrap();
    let report_path = format!("{}/gate-report.md", out_dir);

    // The corpus, derived and asserted against the audit.
    let (corpus, excluded) = enumerate_corpus();
    let corpus_specs: usize = corpus.iter().map(|f| f.spec_names.len()).sum();

    // The composed side: the M3 winner set, extracted once.
    let codegen_functions =
        load_functions("examples/self_host/codegen.morph").expect("codegen module loads");
    let winners = super::m3_gate::extract_winners(&codegen_functions);
    assert_eq!(
        winners.spec_count(),
        123,
        "the composed side must be the M3 winner set (123 Codegen specs)"
    );
    let mut compiler = ComposedCompiler::new(&winners);

    let mut report = String::new();
    report.push_str("# M4 gate report (Phase 46; corpus enlarged to the full 138-spec inventory in Phase 47; re-derived to 142 specs in Phase 56 after the Phase 55 codegen amendments; re-derived to 143 specs in Phase 62 after the Clone-bound amendment; re-derived to 149 specs in Phase 78 after the new-kind amendment)\n\n");
    report.push_str(
        "Composition judged: original pipeline (parse → typeck → synthesize → verify → \
         select) with the M3 mirror-completed composed codegen (mode B) in \
         `codegen::generate`'s seat. Per spec: one winner extraction, that single \
         winner rendered by both codegens, whole files byte-compared.\n\n",
    );
    report.push_str(&format!(
        "Corpus: {} specs in {} check-passing files; excluded (fails `morphic check`): {}.\n\n",
        corpus_specs,
        corpus.len(),
        if excluded.is_empty() {
            "none".to_string()
        } else {
            excluded.join(", ")
        },
    ));

    let mut grand = FileTally::default();
    for file in &corpus {
        let mut tally = FileTally::default();
        report.push_str(&format!(
            "## {} ({} specs)\n\n",
            file.rel_path,
            file.spec_names.len()
        ));
        for spec_name in &file.spec_names {
            tally.attempted += 1;
            match compiler.dual_render(&file.rel_path, spec_name) {
                Ok(render) => {
                    let line = format!(
                        "- `{}` spec `{}` — winner {} [body {}] — {}",
                        render.file,
                        render.spec_name,
                        render.winner_provenance,
                        short_hash(&render.winner_body_key),
                        render.verdict(),
                    );
                    println!("{}", line);
                    report.push_str(&line);
                    report.push('\n');
                    match &render.comparison {
                        Some(FileComparison::Identical) => tally.identical += 1,
                        Some(FileComparison::Divergent(d)) => {
                            tally.divergent += 1;
                            report.push_str(&format!(
                                "  {}\n",
                                d.describe().replace('\n', "\n  ")
                            ));
                        }
                        None => {
                            // A side errored; verdict() already
                            // named which one.
                            if render.original.is_err() {
                                tally.original_error += 1;
                            } else {
                                tally.composed_error += 1;
                            }
                        }
                    }
                }
                Err(e) => {
                    tally.extraction_error += 1;
                    let line = format!(
                        "- `{}` spec `{}` — EXTRACTION ERROR — {}",
                        file.rel_path, spec_name, e
                    );
                    println!("{}", line);
                    report.push_str(&line);
                    report.push('\n');
                }
            }
        }
        let file_line = format!(
            "\nFile tally — attempted {}, identical {}, divergent {}, original errors {}, composed errors {}, extraction errors {}.\n\n",
            tally.attempted,
            tally.identical,
            tally.divergent,
            tally.original_error,
            tally.composed_error,
            tally.extraction_error,
        );
        println!("{}", file_line);
        report.push_str(&file_line);
        grand.attempted += tally.attempted;
        grand.identical += tally.identical;
        grand.divergent += tally.divergent;
        grand.original_error += tally.original_error;
        grand.composed_error += tally.composed_error;
        grand.extraction_error += tally.extraction_error;
        // Rewrite after each file: a partial run stays
        // informative.
        std::fs::write(&report_path, &report).unwrap();
    }

    let elapsed = started.elapsed();
    let summary = format!(
        "## Total\n\nAttempted {}, identical {}, divergent {}, original errors {}, \
         composed errors {}, extraction errors {}. Pipeline runs: {} (one per spec). \
         Elapsed: {:.1}s.\n",
        grand.attempted,
        grand.identical,
        grand.divergent,
        grand.original_error,
        grand.composed_error,
        grand.extraction_error,
        compiler.pipeline_runs(),
        elapsed.as_secs_f64(),
    );
    println!("{}", summary);
    report.push_str(&summary);
    std::fs::write(&report_path, &report).unwrap();

    // The instrument contract at scale, then the gate's
    // numeric floor. Divergence characterization is the
    // gate record's business (ROAD), not an assertion here:
    // a divergent record is evidence, not a harness failure.
    assert_eq!(
        grand.attempted, 149,
        "every corpus spec must produce a record"
    );
    assert_eq!(
        compiler.pipeline_runs(),
        149,
        "the pipeline must run exactly once per corpus spec"
    );
    assert!(
        grand.identical >= 100,
        "the M4 gate demands byte-identical output for 100+ specs; got {}",
        grand.identical,
    );
}
