// ─────────────────────────────────────────────────────────────
//  STRING / DTO API (Phase 66, v0.6 W1)
//
//  The library's front door for non-CLI shells: the browser
//  wrapper (crates/morphic-wasm) today, any embedding later.
//  Inputs and outputs are strings and serde DTOs — never
//  paths (the CLI's filesystem contract stays in the binary;
//  audit finding H4). The native CLI does not route through
//  this module; it calls the same stages directly, exactly
//  as before the extraction.
//
//  HONESTY CONTRACT (v0.6 D2): a build without the
//  `z3-support` feature has NO formal verifier. Reports from
//  such a build carry `verification_available: false` and a
//  winner status of "Inconclusive" — executed-test evidence
//  is reported as itself (ratios with their denominators),
//  never dressed as a verification verdict.
// ─────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

use crate::spec::ast::FunctionSpec;
use crate::synthesis::engine::{
    self, CandidateImplementation, Provenance, ScoreBreakdown, SynthesisConfig,
};
use crate::synthesis::{eval, runner, selector};
use crate::verify::verifier::Verifier;

// ── Check (the W2 surface) ─────────────────────────────────

/// The outcome of parsing + typechecking one source text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckReport {
    /// True iff parse AND typecheck both accepted the source.
    pub ok: bool,
    /// The failing stage, when `ok` is false.
    pub error: Option<StageError>,
    /// One entry per spec function in the module, in source
    /// order. Empty when parsing failed; populated from the
    /// parsed module when typechecking failed (the specs were
    /// readable even if the module did not check).
    pub specs: Vec<SpecReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageError {
    /// "parse" | "typecheck" | "synthesis" | "selection"
    pub stage: String,
    pub message: String,
    /// 1-based source line of the diagnostic, when the
    /// failing stage carries one (Phase 67, W2). Parse
    /// errors always carry line + column (the parser's
    /// ParseError tracks them). Typecheck errors carry
    /// NONE today: TypeError has a span field, but every
    /// constructor leaves it unset, so the honest value
    /// here is None — the typecheck message names the
    /// spec instead. Absent from the JSON when None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// 1-based source column; see `line`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecReport {
    pub name: String,
    pub params: Vec<ParamReport>,
    pub return_type: String,
    /// Number of postconditions the spec declares.
    pub postconditions: usize,
    /// The spec's in-spec tests, in source order — an
    /// INVENTORY with per-test bindability (could the IR
    /// interpreter judge a candidate against this test?).
    /// Listing them executes nothing: running a test needs a
    /// candidate body, which a check does not have.
    pub tests: Vec<TestReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamReport {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    pub index: usize,
    pub name: Option<String>,
    /// True iff the test's input binds against the spec's
    /// parameters AND its expected output maps to an
    /// interpreter value (synthesis::eval::test_bindability).
    pub bindable: bool,
}

fn spec_report(spec: &FunctionSpec) -> SpecReport {
    let bindability = eval::test_bindability(spec);
    SpecReport {
        name: spec.name.clone(),
        params: spec
            .params
            .iter()
            .map(|p| ParamReport {
                name: p.name.clone(),
                ty: p.ty.to_string(),
            })
            .collect(),
        return_type: spec.return_type.to_string(),
        postconditions: spec.postconditions.len(),
        tests: spec
            .tests
            .iter()
            .enumerate()
            .map(|(i, t)| TestReport {
                index: i,
                name: t.name.clone(),
                bindable: bindability[i],
            })
            .collect(),
    }
}

/// Parse + typecheck `source` and report the module's specs
/// with their test inventories. The string-level twin of the
/// CLI's `check` command.
pub fn check(source: &str) -> CheckReport {
    let parsed = match crate::spec::parser::parse(source) {
        Ok(spec) => spec,
        Err(e) => {
            return CheckReport {
                ok: false,
                error: Some(StageError {
                    stage: "parse".into(),
                    message: e.to_string(),
                    line: Some(e.line),
                    column: Some(e.column),
                }),
                specs: vec![],
            };
        }
    };
    let specs: Vec<SpecReport> = parsed.functions.iter().map(spec_report).collect();
    match crate::spec::typeck::check(parsed) {
        Ok(_) => CheckReport {
            ok: true,
            error: None,
            specs,
        },
        Err(e) => CheckReport {
            ok: false,
            error: Some(StageError {
                stage: "typecheck".into(),
                message: e.to_string(),
                // TypeError's span is never populated by any
                // constructor today — no position to report.
                line: None,
                column: None,
            }),
            specs,
        },
    }
}

// ── Render ─────────────────────────────────────────────────

/// Render an already-built candidate to a target language
/// ("rust" | "c" | "wasm" | "python" | "javascript" | "ir")
/// — `codegen::generate` with its error flattened to a
/// string for DTO transport. The "wasm" target renders
/// WebAssembly TEXT (WAT) for preview/copy (v0.6 D6); it
/// does not assemble or execute anything.
pub fn render(candidate: &CandidateImplementation, target: &str) -> Result<String, String> {
    crate::codegen::generate(candidate, target).map_err(|e| e.to_string())
}

// ── Standalone verify (Phase 149, DISEBAR_SCOPE §3) ────

/// Where a candidate body for `verify_body` comes from.
/// The two sources a caller could name at Phase 149 — a
/// shipped library body, or the body a spec's own rows
/// denote through the standing resolver — plus the third
/// that Phase 150's IR text serialization
/// (`synthesis::irtext`, DISEBAR_SCOPE §4.2) makes
/// possible: the body itself, carried as text. §3.3 named
/// this source as 150's consequence; it arrives here as
/// the freeze's recorded extension, through the same
/// parser the bundles use, declining malformed text by
/// name.
#[derive(Debug, Clone, PartialEq)]
pub enum BodySource {
    /// The shipped body of the named standard-library
    /// entry (`stdlib::shipped_body`).
    Library(String),
    /// The body the named spec's own rows denote
    /// (`SpecIndex::callee_body`'s resolver chain).
    Denoted,
    /// A body carried as IR text (Phase 150): parsed by
    /// `irtext::parse_ir`, which declines malformed or
    /// foreign-version text by name.
    IrText(String),
}

/// One postcondition row's judgment in a `VerifyReport`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRowReport {
    pub constraint: String,
    pub holds: bool,
}

/// The outcome of judging ONE candidate body against ONE
/// spec: the verifier's verdict (its own, produced by
/// `Verifier::verify_one` at call time — in a no-Z3
/// build the honest Inconclusive, `verification_available`
/// false), the per-row results, and the executed half
/// (the interpreter's counts on the same body, through
/// the index-carrying route of Phase 135).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyReport {
    pub spec_name: String,
    /// "library:<entry>" | "denoted" | "ir-text" — where
    /// the judged body came from.
    pub body_source: String,
    pub verification_available: bool,
    /// "Verified" | "VerificationFailed" | "Inconclusive"
    /// | "Malformed"
    pub status: String,
    pub rows: Vec<VerifyRowReport>,
    pub tests_passed: usize,
    pub tests_evaluable: usize,
    pub tests_total: usize,
    pub verification_time_ms: u64,
}

/// The standalone verify entry point (the audit's §1(b)
/// gap): parse + typecheck `source`, resolve the named
/// spec, obtain the candidate body from `body_source`,
/// and judge it in-process. The verdict is never a
/// caller's claim and never cached — it is `verify_one`'s
/// output for this body and this spec, now.
pub fn verify_body(
    source: &str,
    spec_name: &str,
    body_source: &BodySource,
) -> Result<VerifyReport, StageError> {
    let err = |stage: &str, e: &dyn std::fmt::Display| StageError {
        stage: stage.into(),
        message: e.to_string(),
        line: None,
        column: None,
    };
    let parsed = crate::spec::parser::parse(source).map_err(|e| StageError {
        stage: "parse".into(),
        message: e.to_string(),
        line: Some(e.line),
        column: Some(e.column),
    })?;
    let checked = crate::spec::typeck::check(parsed).map_err(|e| err("typecheck", &e))?;
    let functions = checked.spec.functions;
    let spec = functions
        .iter()
        .find(|f| f.name == spec_name)
        .ok_or_else(|| StageError {
            stage: "verify".into(),
            message: format!(
                "No spec named '{}' in this module (specs: {})",
                spec_name,
                functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")
            ),
            line: None,
            column: None,
        })?
        .clone();

    let index = engine::SpecIndex::from_specs(&functions);
    let (body, body_source_text) = match body_source {
        BodySource::Library(entry) => (
            crate::stdlib::shipped_body(entry).ok_or_else(|| StageError {
                stage: "verify".into(),
                message: format!(
                    "no shipped body for library entry '{entry}' \
                     (the library tool lists the shipped entries)"
                ),
                line: None,
                column: None,
            })?,
            format!("library:{entry}"),
        ),
        BodySource::Denoted => (
            index.callee_body(spec_name).ok_or_else(|| StageError {
                stage: "verify".into(),
                message: format!(
                    "spec '{spec_name}' denotes no body through the resolver — \
                     its rows are not in a denoting form (an equation row or a guard table)"
                ),
                line: None,
                column: None,
            })?,
            "denoted".to_string(),
        ),
        BodySource::IrText(text) => (
            crate::synthesis::irtext::parse_ir(text).map_err(|e| StageError {
                stage: "verify".into(),
                message: e,
                line: None,
                column: None,
            })?,
            "ir-text".to_string(),
        ),
    };

    let candidate = CandidateImplementation {
        id: 0,
        body: body.clone(),
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
    };
    let result = Verifier::with_specs(&functions).verify_one(&candidate, &spec);
    let run = eval::run_spec_tests_in(&body, &spec, &index);
    Ok(VerifyReport {
        spec_name: spec.name.clone(),
        body_source: body_source_text,
        verification_available: cfg!(feature = "z3-support"),
        status: format!("{:?}", result.status),
        rows: result
            .constraint_results
            .iter()
            .map(|r| VerifyRowReport { constraint: r.constraint.clone(), holds: r.holds })
            .collect(),
        tests_passed: run.passed,
        tests_evaluable: run.evaluable,
        tests_total: spec.tests.len(),
        verification_time_ms: result.verification_time_ms,
    })
}

// ── Seeded synthesis (the playground profile, D3) ──────────

/// The playground synthesis profile: the opt-level-1 shape
/// (the only LLM-free CLI level) with its two browser
/// properties stated rather than assumed — no LLM, ever,
/// and budgets that are iteration/population caps, not
/// wall-clock (the engine's `max_time_ms` is never consulted
/// in the run path; audit finding S6). Determinism comes
/// from the seeded engine RNG, not from this struct.
pub fn playground_config() -> SynthesisConfig {
    let mut config = SynthesisConfig::from_opt_level(1);
    config.use_llm = false;
    config.use_z3 = cfg!(feature = "z3-support");
    config
}

/// The outcome of one seeded playground-profile run over the
/// source's FIRST spec (the CLI Build arm's one-spec-at-a-
/// time discipline, module in scope).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisReport {
    pub spec_name: String,
    /// Candidates the pipeline evaluated (the engine's own
    /// counter for this run's pool, post-verification).
    pub candidates: usize,
    /// False in a build without `z3-support`: no candidate
    /// in such a build has a formal verdict, whatever its
    /// executed-test evidence says (D2).
    pub verification_available: bool,
    pub winner: WinnerReport,
}

/// The playground's pinned default seed (D3): the value
/// the playground UI pre-fills in its seed field, and the
/// seed the W3 pinned corpus is measured with. A run is
/// reproducible iff it names its seed; this is the seed a
/// first-time visitor's runs use.
pub const PLAYGROUND_DEFAULT_SEED: u64 = 42;

/// One target rendering of the crowned winner, computed
/// in-process from the actual winner candidate (never
/// re-derived by a shell). Exactly one of `text` / `error`
/// is set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderReport {
    /// "rust" | "c" | "wasm" | "python" | "javascript" | "ir"
    pub target: String,
    pub text: Option<String>,
    pub error: Option<String>,
}

/// One in-spec test's executed outcome against the crowned
/// winner, as the IR interpreter measured it
/// (`synthesis::eval::test_outcomes`). `outcome` is
/// "passed" | "failed" | "body_not_evaluable" |
/// "unbindable"; `reason` carries the evaluator's reason
/// (body coverage) or the reason class (binding) for the
/// two not-evaluable outcomes, and is None otherwise.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestOutcomeReport {
    pub index: usize,
    pub name: Option<String>,
    pub outcome: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinnerReport {
    /// The crowned body, in the IR's Debug rendering — the
    /// same structural fingerprint the pipeline itself uses
    /// for body identity (engine::readmit_protected_seeds).
    pub body_debug: String,
    pub score: f64,
    /// Executed-test evidence: passed / evaluable, as the
    /// interpreter measured it (the runner stage may have
    /// re-measured finalists in a native build — the ratio
    /// recorded on the winner is the pipeline's final one).
    pub test_pass_ratio: f64,
    pub constraint_score: f64,
    /// Raw executed-test counts for the crowned body,
    /// measured by the IR interpreter at report time
    /// (`eval::run_spec_tests` on the winner — NOT the
    /// hole-decayed pipeline ratio above): the winner passed
    /// `tests_passed` of the `tests_evaluable` tests the
    /// interpreter could bind, out of `tests_total` in-spec
    /// tests. The playground displays these as the exact
    /// fraction passed / evaluable / total (W3).
    pub tests_passed: usize,
    pub tests_evaluable: usize,
    pub tests_total: usize,
    /// Per-test executed outcomes, in spec order (W3).
    pub test_outcomes: Vec<TestOutcomeReport>,
    /// The winner rendered to every codegen target (W3).
    /// The "wasm" entry is WAT text for preview/copy (D6).
    pub renders: Vec<RenderReport>,
    /// "Verified" | "VerificationFailed" | "Inconclusive" |
    /// "Malformed" | "Unverified" (no verdict recorded).
    /// In a no-Z3 build this is "Inconclusive" for every
    /// well-formed winner — the honest label, not a renamed
    /// substitute (v0.6 anti-pattern).
    pub verification_status: String,
}

/// Run the full pipeline (parse → typecheck → synthesize →
/// verify → runner stage → select) over the source's first
/// spec, with the engine RNG seeded from `seed` (D3): the
/// same (source, seed) in the same build configuration
/// crowns the same winner body. The native CLI never calls
/// this — its runs stay entropy-seeded, as before.
pub fn synthesize_seeded(source: &str, seed: u64) -> Result<SynthesisReport, StageError> {
    let (functions, first) = parse_and_first(source)?;
    Ok(synthesize_spec(&functions, &first, seed)?.0)
}

/// `synthesize_seeded` for a NAMED spec of the module (the
/// playground's per-spec Synthesize control, W3). The
/// module stays in scope for the run, exactly as in the
/// first-spec form; an unknown name is a "synthesis"-stage
/// error naming the module's specs.
pub fn synthesize_seeded_spec(
    source: &str,
    spec_name: &str,
    seed: u64,
) -> Result<SynthesisReport, StageError> {
    let (functions, spec) = parse_and_find(source, spec_name)?;
    Ok(synthesize_spec(&functions, &spec, seed)?.0)
}

/// The seeded run WITH its crowned winner attached
/// (Phase 154, DISEBAR_15_SCOPE §4): the report and the
/// winning candidate from the SAME run, so a consumer
/// packaging the crown (the `build --package-from` verb)
/// holds the very body the report's verdict and battery
/// describe — never a second run's body wearing the
/// first run's measurements. The report-only entry
/// points above are this function with the candidate
/// dropped.
pub fn synthesize_seeded_spec_winner(
    source: &str,
    spec_name: &str,
    seed: u64,
) -> Result<(SynthesisReport, CandidateImplementation), StageError> {
    let (functions, spec) = parse_and_find(source, spec_name)?;
    synthesize_spec(&functions, &spec, seed)
}

/// Parse + typecheck `source` and resolve the named
/// spec, the shared front half of the named-spec entry
/// points.
fn parse_and_find(
    source: &str,
    spec_name: &str,
) -> Result<(Vec<FunctionSpec>, FunctionSpec), StageError> {
    let err = |stage: &str, e: &dyn std::fmt::Display| StageError {
        stage: stage.into(),
        message: e.to_string(),
        line: None,
        column: None,
    };
    let parsed = crate::spec::parser::parse(source).map_err(|e| StageError {
        stage: "parse".into(),
        message: e.to_string(),
        line: Some(e.line),
        column: Some(e.column),
    })?;
    let checked = crate::spec::typeck::check(parsed).map_err(|e| err("typecheck", &e))?;
    let functions = checked.spec.functions;
    let spec = functions
        .iter()
        .find(|f| f.name == spec_name)
        .ok_or_else(|| StageError {
            stage: "synthesis".into(),
            message: format!(
                "No spec named '{}' in this module (specs: {})",
                spec_name,
                functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")
            ),
            line: None,
            column: None,
        })?
        .clone();
    Ok((functions, spec))
}

/// The seeded run over an already-checked module given
/// as ASTs — `synthesize_seeded_spec` without the text
/// round-trip. Phase 150's bundle re-verification uses
/// it: the consumer holds a bundle's spec as an AST and
/// reconstructs the module it was crowned in, so the
/// pipeline crown is re-run over ASTs, not re-serialized
/// text.
pub fn synthesize_seeded_functions(
    functions: &[FunctionSpec],
    spec_name: &str,
    seed: u64,
) -> Result<SynthesisReport, StageError> {
    let spec = functions
        .iter()
        .find(|f| f.name == spec_name)
        .ok_or_else(|| StageError {
            stage: "synthesis".into(),
            message: format!(
                "No spec named '{}' in this module (specs: {})",
                spec_name,
                functions.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")
            ),
            line: None,
            column: None,
        })?
        .clone();
    Ok(synthesize_spec(functions, &spec, seed)?.0)
}

fn parse_and_first(
    source: &str,
) -> Result<(Vec<FunctionSpec>, FunctionSpec), StageError> {
    let err = |stage: &str, e: &dyn std::fmt::Display| StageError {
        stage: stage.into(),
        message: e.to_string(),
        line: None,
        column: None,
    };
    let parsed = crate::spec::parser::parse(source).map_err(|e| StageError {
        stage: "parse".into(),
        message: e.to_string(),
        line: Some(e.line),
        column: Some(e.column),
    })?;
    let checked = crate::spec::typeck::check(parsed).map_err(|e| err("typecheck", &e))?;
    let functions = checked.spec.functions;
    let first = functions
        .first()
        .ok_or_else(|| StageError {
            stage: "synthesis".into(),
            message: "No spec functions found".into(),
            line: None,
            column: None,
        })?
        .clone();
    Ok((functions, first))
}

/// The shared pipeline run behind both seeded entry points:
/// synthesize → verify → runner stage → select over `spec`,
/// with the module (`functions`) in scope and the engine
/// RNG seeded from `seed`.
fn synthesize_spec(
    functions: &[FunctionSpec],
    spec: &FunctionSpec,
    seed: u64,
) -> Result<(SynthesisReport, CandidateImplementation), StageError> {
    let err = |stage: &str, e: &dyn std::fmt::Display| StageError {
        stage: stage.into(),
        message: e.to_string(),
        line: None,
        column: None,
    };
    let spec = spec.clone();

    engine::seed_engine_rng(seed);
    let mut engine = engine::SynthesisEngine::with_module(playground_config(), functions);
    let candidates = engine.run(&spec).map_err(|e| err("synthesis", &e))?;
    let verified = Verifier::with_specs(functions)
        .verify_all(candidates, &spec)
        .map_err(|e| err("synthesis", &e))?;
    // The compiled-runner stage, or — in a build without the
    // `compiled-runner` feature — its documented degradation:
    // the pool passes through unchanged (runner stub).
    let pool = runner::rescore_finalists(verified, &spec);
    let candidates = pool.len();
    let winner = selector::select_best(pool, &spec).map_err(|e| err("selection", &e))?;

    // Executed evidence for the crowned body, measured by
    // the interpreter NOW (the raw counts behind the W3
    // fraction; the pipeline's own test_pass_ratio on the
    // winner may be hole-decayed or runner-rescored, and is
    // reported separately, as itself).
    let run = eval::run_spec_tests(&winner.body, &spec);
    let outcomes = eval::test_outcomes(&winner.body, &spec);
    let renders = ["rust", "c", "wasm", "python", "javascript", "ir"]
        .iter()
        .map(|target| match render(&winner, target) {
            Ok(text) => RenderReport { target: target.to_string(), text: Some(text), error: None },
            Err(e) => RenderReport { target: target.to_string(), text: None, error: Some(e) },
        })
        .collect();

    let crowned = winner.clone();
    Ok((
        SynthesisReport {
            spec_name: spec.name.clone(),
            candidates,
            verification_available: cfg!(feature = "z3-support"),
            winner: WinnerReport {
                body_debug: format!("{:?}", winner.body),
                score: winner.score,
                test_pass_ratio: winner.scores.test_pass_ratio,
                constraint_score: winner.scores.constraint_score,
                tests_passed: run.passed,
                tests_evaluable: run.evaluable,
                tests_total: spec.tests.len(),
                test_outcomes: outcomes
                    .iter()
                    .enumerate()
                    .map(|(i, o)| {
                        let (outcome, reason) = match o {
                            eval::TestOutcome::Passed => ("passed", None),
                            eval::TestOutcome::Failed => ("failed", None),
                            eval::TestOutcome::BodyNotEvaluable(r) => {
                                ("body_not_evaluable", Some(r.clone()))
                            }
                            eval::TestOutcome::Unbindable(r) => {
                                ("unbindable", Some(r.clone()))
                            }
                            eval::TestOutcome::SpecSilent(r) => {
                                ("spec_silent", Some(r.clone()))
                            }
                        };
                        TestOutcomeReport {
                            index: i,
                            name: spec.tests[i].name.clone(),
                            outcome: outcome.into(),
                            reason,
                        }
                    })
                    .collect(),
                renders,
                verification_status: match &winner.verification_status {
                    Some(s) => format!("{:?}", s),
                    None => "Unverified".into(),
                },
            },
        },
        crowned,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = include_str!("../examples/self_host/validator.morph");
    const INVALID: &str = "spec broken { input: x: Int output: Int constraint: ";

    #[test]
    fn check_reports_specs_and_bindability() {
        let report = check(VALID);
        assert!(report.ok, "validator.morph must check: {:?}", report.error);
        assert!(!report.specs.is_empty());
        let total_tests: usize = report.specs.iter().map(|s| s.tests.len()).sum();
        assert!(total_tests > 0, "the module carries in-spec tests");
        // Every reported test index is its position in the
        // spec's own test list (inventory integrity).
        for spec in &report.specs {
            for (i, t) in spec.tests.iter().enumerate() {
                assert_eq!(t.index, i);
            }
        }
    }

    #[test]
    fn check_rejects_invalid_source_at_parse() {
        let report = check(INVALID);
        assert!(!report.ok);
        let error = report.error.expect("a failing stage is named");
        assert_eq!(error.stage, "parse");
        assert!(report.specs.is_empty());
    }

    /// W2's positioning contract (Phase 67): parse
    /// diagnostics carry the parser's 1-based line/column
    /// through the DTO; typecheck diagnostics carry none
    /// (TypeError populates no span today) — the UI must
    /// render the first kind in the source and show the
    /// second as a stage message only.
    #[test]
    fn check_parse_error_carries_position_typecheck_carries_none() {
        // Truncated mid-constraint: the parser stops at the
        // end of the single-line source.
        let report = check(INVALID);
        let error = report.error.expect("parse error");
        assert_eq!(error.line, Some(1));
        assert!(error.column.is_some_and(|c| c > 1));

        // Parses fine, fails typecheck: `++` (concat)
        // applied to Int operands — the typeck suite's own
        // pinned rejection shape.
        let type_err = "Typed {\n\
        spec f {\n\
            input: a: Int\n\
            output: Int\n\
            constraint: output == a ++ 2\n\
        }\n\
        }";
        let report = check(type_err);
        assert!(!report.ok);
        let error = report.error.expect("typecheck error");
        assert_eq!(error.stage, "typecheck");
        assert_eq!(error.line, None);
        assert_eq!(error.column, None);
        // The parsed specs are still reported (readable
        // module, failed check) — W2 renders them.
        assert_eq!(report.specs.len(), 1);
    }

    /// W3 (Phase 68): the named-spec entry point crowns a
    /// winner for the named spec, carries the raw executed
    /// counts + per-test outcomes + all six renders, and
    /// rejects an unknown name at the synthesis stage.
    #[test]
    fn seeded_spec_report_carries_counts_outcomes_and_renders() {
        let report = synthesize_seeded_spec(VALID, "is_known_type_tag", PLAYGROUND_DEFAULT_SEED)
            .expect("named-spec run");
        assert_eq!(report.spec_name, "is_known_type_tag");
        let w = &report.winner;
        assert_eq!(w.tests_total, 5);
        assert_eq!(w.test_outcomes.len(), w.tests_total);
        assert!(w.tests_passed <= w.tests_evaluable && w.tests_evaluable <= w.tests_total);
        // The counts are the fold of the per-test outcomes.
        let passed = w.test_outcomes.iter().filter(|o| o.outcome == "passed").count();
        let evaluable = w.test_outcomes.iter().filter(|o| o.outcome != "unbindable").count();
        assert_eq!(w.tests_passed, passed);
        assert_eq!(w.tests_evaluable, evaluable);
        // All six codegen targets are rendered (or errored).
        let targets: Vec<&str> = w.renders.iter().map(|r| r.target.as_str()).collect();
        assert_eq!(targets, ["rust", "c", "wasm", "python", "javascript", "ir"]);
        assert!(w.renders.iter().all(|r| r.text.is_some() != r.error.is_some()));

        let err = synthesize_seeded_spec(VALID, "no_such_spec", PLAYGROUND_DEFAULT_SEED)
            .expect_err("unknown spec name must fail");
        assert_eq!(err.stage, "synthesis");
        assert!(err.message.contains("no_such_spec"));
    }

    /// W3 GATE HARNESS (Phase 68) — compiled ONLY in the
    /// browser configuration (no Z3, no compiled runner):
    /// the configuration whose crowns the WASM build
    /// produces. For every pinned case it (1) runs the API,
    /// (2) re-runs the pipeline stages independently and
    /// asserts the crowned body is IDENTICAL to the API's
    /// (body identity — the seeded determinism makes the
    /// body a pure function of source + seed + config), and
    /// (3) asserts `eval::run_spec_tests` on that identical
    /// body reproduces the API's reported fraction AND the
    /// literal pin. The pins were measured from this harness
    /// (the native interpreter) BEFORE any browser run;
    /// playground/corpus/cases.json carries the same pins
    /// for the WASM side of the gate.
    #[cfg(all(not(feature = "z3-support"), not(feature = "compiled-runner")))]
    #[test]
    fn w3_gate_fractions_match_native_interpreter() {
        use crate::synthesis::{engine, runner, selector};
        use crate::verify::verifier::Verifier;

        const SORT: &str = include_str!("../examples/sort.morph");
        const IS_SORTED: &str = include_str!("../examples/self_host/is_sorted.morph");
        const COUNT_MATCHING: &str =
            include_str!("../examples/self_host/count_matching.morph");

        // (label, source, spec, seed, passed, evaluable, total)
        // Phase 115 re-measurement: the interpreter's For
        // arm made the For-shaped template bodies
        // evaluable in this profile, so is_sorted's crown
        // is now the AdjacentScan body at 5/5 and
        // count_matching's crown passes 3/3 — the Phase
        // 68 pins (4/5, 1/3) recorded the pre-For-arm
        // wall, where those bodies never reached a judge.
        // Phase 130 re-measurement: function values made
        // sort's lambda-carrying tests bind, so its crown
        // is measured for real in this profile — 3 passed
        // of 6 evaluable — where the Phase 68 pin (0, 0, 6)
        // recorded the function-value wall, at which no
        // test could bind at all.
        // Phase 131 re-measurement: higher-order judging
        // (Assign-to-Index, the Let mutation delta, the
        // bare-`None` idiom) put sort's template at 6/6
        // in-process, and this profile crowns the template
        // itself — the case is renamed sort-full, its
        // "partial" name retired with the standing it
        // described.
        let cases: [(&str, &str, &str, u64, usize, usize, usize); 4] = [
            ("validator-fully-evaluable", VALID, "is_known_type_tag", PLAYGROUND_DEFAULT_SEED, 5, 5, 5),
            ("is-sorted-full", IS_SORTED, "is_sorted", PLAYGROUND_DEFAULT_SEED, 5, 5, 5),
            ("count-matching-full", COUNT_MATCHING, "count_matching", PLAYGROUND_DEFAULT_SEED, 3, 3, 3),
            ("sort-full", SORT, "sort", PLAYGROUND_DEFAULT_SEED, 6, 6, 6),
        ];

        for (label, source, spec_name, seed, pin_passed, pin_evaluable, pin_total) in cases {
            let report = synthesize_seeded_spec(source, spec_name, seed)
                .unwrap_or_else(|e| panic!("{label}: API run failed: {e:?}"));
            assert_eq!(report.spec_name, spec_name, "{label}");
            assert!(!report.verification_available, "{label}: browser config has no verifier");
            assert_eq!(report.winner.verification_status, "Inconclusive", "{label}");

            // Independent pipeline re-run → body identity.
            let parsed = crate::spec::parser::parse(source).expect("parse");
            let checked = crate::spec::typeck::check(parsed).expect("typeck");
            let functions = &checked.spec.functions;
            let spec = functions.iter().find(|f| f.name == spec_name).expect("spec").clone();
            engine::seed_engine_rng(seed);
            let mut eng = engine::SynthesisEngine::with_module(playground_config(), functions);
            let cands = eng.run(&spec).expect("engine run");
            let verified = Verifier::with_specs(functions)
                .verify_all(cands, &spec)
                .expect("verify_all");
            let pool = runner::rescore_finalists(verified, &spec);
            let winner = selector::select_best(pool, &spec).expect("select_best");
            assert_eq!(
                format!("{:?}", winner.body),
                report.winner.body_debug,
                "{label}: API winner body must be the pipeline's crowned body"
            );

            // The interpreter's ratio for the identical body.
            let run = eval::run_spec_tests(&winner.body, &spec);
            assert_eq!(
                (run.passed, run.evaluable, spec.tests.len()),
                (report.winner.tests_passed, report.winner.tests_evaluable, report.winner.tests_total),
                "{label}: API fraction must be the interpreter's measurement of the crowned body"
            );
            assert_eq!(
                (run.passed, run.evaluable, spec.tests.len()),
                (pin_passed, pin_evaluable, pin_total),
                "{label}: measured fraction must equal the pin"
            );
            println!(
                "W3PIN {label} passed={} evaluable={} total={} status={} outcomes={}",
                run.passed,
                run.evaluable,
                spec.tests.len(),
                report.winner.verification_status,
                report
                    .winner
                    .test_outcomes
                    .iter()
                    .map(|o| o.outcome.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
    }

    /// D3's pin: same seed + same source → same crowned
    /// winner body, in whatever configuration this test
    /// binary was built with (the W1 gate runs it in the
    /// no-Z3/no-runner configuration, where the pipeline is
    /// the browser's). The spec is a single-spec source whose
    /// tests all bind and whose constructed seeds the
    /// interpreter evaluates, so no stage's coverage gap —
    /// only the RNG — could make the two runs differ.
    #[test]
    fn seeded_synthesis_is_deterministic() {
        let source = "Playground {\n\
        spec is_known_type_tag {\n\
            input: tag: Int\n\
            output: Bool\n\
            constraint: output == (tag>=0&&tag<=8)\n\
            test \"unknown-tag\" { input: 0 expect: true }\n\
            test \"result-tag\" { input: 8 expect: true }\n\
            test \"out-of-range\" { input: 9 expect: false }\n\
            test \"far-out\" { input: 42 expect: false }\n\
        }\n\
        }";
        let first = synthesize_seeded(source, 0x5EED).expect("first run");
        let second = synthesize_seeded(source, 0x5EED).expect("second run");
        assert_eq!(
            first.winner.body_debug, second.winner.body_debug,
            "same seed must crown the same winner body"
        );
        assert_eq!(first.winner.score, second.winner.score);
        assert_eq!(first.candidates, second.candidates);
    }
}
