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
use crate::synthesis::engine::{self, CandidateImplementation, SynthesisConfig};
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
    let err = |stage: &str, e: &dyn std::fmt::Display| StageError {
        stage: stage.into(),
        message: e.to_string(),
    };
    let parsed = crate::spec::parser::parse(source).map_err(|e| err("parse", &e))?;
    let checked = crate::spec::typeck::check(parsed).map_err(|e| err("typecheck", &e))?;
    let functions = &checked.spec.functions;
    let spec = functions
        .first()
        .ok_or_else(|| StageError {
            stage: "synthesis".into(),
            message: "No spec functions found".into(),
        })?
        .clone();

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

    Ok(SynthesisReport {
        spec_name: spec.name.clone(),
        candidates,
        verification_available: cfg!(feature = "z3-support"),
        winner: WinnerReport {
            body_debug: format!("{:?}", winner.body),
            score: winner.score,
            test_pass_ratio: winner.scores.test_pass_ratio,
            constraint_score: winner.scores.constraint_score,
            verification_status: match &winner.verification_status {
                Some(s) => format!("{:?}", s),
                None => "Unverified".into(),
            },
        },
    })
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
