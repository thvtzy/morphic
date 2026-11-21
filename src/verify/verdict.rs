// ─────────────────────────────────────────────────────────────
//  VERIFICATION VERDICT TYPES (pure — Phase 66, v0.6 W1)
//
//  The verdict vocabulary of the verification pipeline,
//  split from the Z3 session code (audit finding S3: the
//  `z3-support` feature could remove the dependency but no
//  source seam let the verifier EXIST without it). These
//  types are pure data — the engine and selector import
//  `VerificationStatus`, seed admission imports the
//  encodability predicate, and neither may depend on a
//  solver being linked. The Z3 backend (verifier.rs + smt.rs,
//  behind `z3-support`) produces these values; the no-Z3
//  backend produces the honest subset: Malformed for holed
//  bodies, Inconclusive for everything else — never Verified,
//  never a renamed substitute (v0.6 anti-pattern).
// ─────────────────────────────────────────────────────────────

use std::collections::HashMap;

// ── Solver result vocabulary ───────────────────────────────
//
// Moved from smt.rs (Phase 66): the VALUES a solver session
// returns are pure data; only the session that produces
// them needs Z3. ConstraintResult models and counterexamples
// are stated in these terms in every build.

#[derive(Debug, Clone)]
pub enum SmtResult {
    Sat(HashMap<String, SmtValue>),
    Unsat,
    Unknown,
    Timeout,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SmtValue {
    Int(i64),
    Bool(bool),
    Uninterpreted(String),
}

impl std::fmt::Display for SmtValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmtValue::Int(n) => write!(f, "{}", n),
            SmtValue::Bool(b) => write!(f, "{}", b),
            SmtValue::Uninterpreted(s) => write!(f, "{}", s),
        }
    }
}

// ── Verdict types ──────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub candidate_id: u64,
    pub status: VerificationStatus,
    pub constraint_results: Vec<ConstraintResult>,
    pub counterexample: Option<Counterexample>,
    pub verification_time_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationStatus { Verified, VerificationFailed, Inconclusive, Malformed }

#[derive(Debug, Clone)]
pub struct ConstraintResult {
    pub constraint: String,
    pub holds: bool,
    pub solver_time_ms: u64,
    pub model: Option<HashMap<String, SmtValue>>,
}

#[derive(Debug, Clone)]
pub struct Counterexample {
    pub description: String,
    pub inputs: HashMap<String, SmtValue>,
}

#[derive(Debug)]
pub enum VerificationError {
    SolverError(String),
    Timeout,
    InternalError(String),
}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationError::SolverError(m) => write!(f, "Solver error: {}", m),
            VerificationError::Timeout => write!(f, "Verification timeout"),
            VerificationError::InternalError(m) => write!(f, "Internal: {}", m),
        }
    }
}
impl std::error::Error for VerificationError {}
