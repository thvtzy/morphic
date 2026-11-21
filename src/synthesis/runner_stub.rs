// ─────────────────────────────────────────────────────────────
//  COMPILED-RUNNER STUB (Phase 66, v0.6 W1)
//
//  Compiled in place of runner.rs when the `compiled-runner`
//  feature is off (the browser profile). The real stage's
//  contract has a documented DEGRADATION for exactly this
//  situation — "infrastructure failure degrades the WHOLE
//  spec to today's fallback behavior (the pool is returned
//  unchanged). A build must never fail because the runner
//  failed." A build with no rustc to spawn is that failure
//  in permanent form, so the stub IS the degradation path,
//  not a simulation of scoring: candidates keep their
//  interpreter/FALLBACK test_pass_ratio, selection proceeds,
//  and the outcome reports itself Unavailable wherever the
//  real stage would report NotTriggered/Degraded/Scored.
// ─────────────────────────────────────────────────────────────

use crate::spec::ast::FunctionSpec;
use crate::synthesis::engine::CandidateImplementation;
use std::time::Duration;

/// How many finalists the runner judges per triggered spec
/// (the real stage's bound; kept so report shapes agree).
pub const RUNNER_FINALISTS: usize = 8;

/// One finalist's measured outcome (the real stage's shape;
/// the stub produces none).
#[derive(Debug, Clone)]
pub struct FinalistOutcome {
    pub candidate_id: u64,
    pub passed: usize,
    pub total: usize,
    pub new_test_pass_ratio: f64,
}

/// What the stage did. The stub's only outcome is
/// `Unavailable`: the stage does not exist in this build.
#[derive(Debug)]
pub enum RunnerOutcome {
    /// The compiled-runner stage is not compiled into this
    /// build (browser profile); the pool was returned
    /// unchanged — the stage's documented degradation.
    Unavailable,
}

/// The pipeline stage, as wired into the pipeline between
/// verification and selection. Degradation semantics: the
/// pool is returned unchanged.
pub fn rescore_finalists(
    candidates: Vec<CandidateImplementation>,
    spec: &FunctionSpec,
) -> Vec<CandidateImplementation> {
    rescore_finalists_report(candidates, spec).0
}

/// `rescore_finalists`, also reporting what it did: nothing,
/// because the stage is unavailable in this build.
pub fn rescore_finalists_report(
    candidates: Vec<CandidateImplementation>,
    _spec: &FunctionSpec,
) -> (Vec<CandidateImplementation>, RunnerOutcome) {
    (candidates, RunnerOutcome::Unavailable)
}

/// The elapsed time the stub never spends (report-shape
/// parity for callers that log stage durations).
pub fn unavailable_elapsed() -> Duration {
    Duration::ZERO
}
