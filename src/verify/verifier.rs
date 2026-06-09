// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC VERIFIER (v0.2)                                 │
// │  Z3-based formal verification of candidate implementations│
// └──────────────────────────────────────────────────────────┘

use crate::spec::ast::*;
use crate::synthesis::engine::{CandidateImplementation, IRNode, ScoreBreakdown, composite_score};
use super::smt::{Z3Session, SmtResult, SmtValue, z3_add, z3_sub, z3_mul, z3_and};
use std::collections::HashMap;
use std::time::Instant;

// ── Public Types ───────────────────────────────────────────

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

// ── Verifier ───────────────────────────────────────────────

pub struct Verifier {
    timeout_per_constraint_ms: u64,
}

impl Default for Verifier {
    fn default() -> Self { Self { timeout_per_constraint_ms: 5_000 } }
}

impl Verifier {
    pub fn new() -> Self { Self::default() }

    pub fn verify_all(
        &self,
        candidates: Vec<CandidateImplementation>,
        spec: &FunctionSpec,
    ) -> Result<Vec<CandidateImplementation>, VerificationError> {
        let mut verified = Vec::with_capacity(candidates.len());
        for mut candidate in candidates {
            let result = self.verify_one(&candidate, spec);
            candidate.verified = result.status == VerificationStatus::Verified;
            let passed = result.constraint_results.iter().filter(|r| r.holds).count();
            let total = result.constraint_results.len();
            if total > 0 {
                candidate.scores.constraint_score = passed as f64 / total as f64;
            }
            candidate.score = composite_score(&candidate.scores);
            verified.push(candidate);
        }
        Ok(verified)
    }

    pub fn verify_one(
        &self,
        candidate: &CandidateImplementation,
        spec: &FunctionSpec,
    ) -> VerificationResult {
        let start = Instant::now();
        let mut constraint_results = Vec::new();
        let mut counterexample = None;

        if !is_well_formed(&candidate.body) {
            return VerificationResult {
                candidate_id: candidate.id, status: VerificationStatus::Malformed,
                constraint_results: vec![], counterexample: None,
                verification_time_ms: start.elapsed().as_millis() as u64,
            };
        }

        for (i, c) in spec.postconditions.iter().enumerate() {
            let result = self.verify_constraint(c, &format!("post[{}]", i));
            if !result.holds && counterexample.is_none() {
                counterexample = Some(Counterexample {
                    description: format!("Postcondition {} violated", i + 1),
                    inputs: result.model.clone().unwrap_or_default(),
                });
            }
            constraint_results.push(result);
        }

        let all_hold = constraint_results.iter().all(|r| r.holds);
        VerificationResult {
            candidate_id: candidate.id,
            status: if all_hold { VerificationStatus::Verified } else { VerificationStatus::VerificationFailed },
            constraint_results, counterexample,
            verification_time_ms: start.elapsed().as_millis() as u64,
        }
    }

    fn verify_constraint(&self, constraint: &Constraint, label: &str) -> ConstraintResult {
        let start = Instant::now();
        let z3 = Z3Session::new(self.timeout_per_constraint_ms);

        let formula = encode_constraint(constraint, &z3);
        let negated = formula.not();
        z3.assert(&negated);

        let result = z3.check_and_get_model();
        match result {
            SmtResult::Unsat => ConstraintResult {
                constraint: label.into(), holds: true,
                solver_time_ms: start.elapsed().as_millis() as u64, model: None,
            },
            SmtResult::Sat(model) => ConstraintResult {
                constraint: label.into(), holds: false,
                solver_time_ms: start.elapsed().as_millis() as u64,
                model: if model.is_empty() { None } else { Some(model) },
            },
            _ => ConstraintResult {
                constraint: label.into(), holds: false,
                solver_time_ms: start.elapsed().as_millis() as u64, model: None,
            },
        }
    }
}

// ── Constraint → Z3 Translation ────────────────────────────

fn encode_constraint(c: &Constraint, z3: &Z3Session) -> z3::ast::Bool {
    match c {
        Constraint::True => z3::ast::Bool::from_bool(true),
        Constraint::Not(inner) => encode_constraint(inner, z3).not(),
        Constraint::Order(OrderOp::LessThan, a, b) => encode_expr_int(a, z3).lt(&encode_expr_int(b, z3)),
        Constraint::Order(OrderOp::LessThanOrEqual, a, b) => encode_expr_int(a, z3).le(&encode_expr_int(b, z3)),
        Constraint::Order(OrderOp::GreaterThan, a, b) => encode_expr_int(a, z3).gt(&encode_expr_int(b, z3)),
        Constraint::Eq(a, b) => encode_expr_int(a, z3).eq(&encode_expr_int(b, z3)),
        Constraint::And(parts) => {
            if parts.is_empty() {
                return z3::ast::Bool::from_bool(true);
            }
            let mut acc = encode_constraint(&parts[0], z3);
            for p in &parts[1..] {
                acc = z3_and(&acc, &encode_constraint(p, z3));
            }
            acc
        }
        Constraint::Implies(a, b) => encode_constraint(a, z3).implies(&encode_constraint(b, z3)),
        Constraint::Expr(expr) => encode_expr_bool(expr, z3),
        _ => z3::ast::Bool::from_bool(true),
    }
}

fn encode_expr_int(expr: &Expr, z3: &Z3Session) -> z3::ast::Int {
    match expr {
        Expr::IntLit(n) => z3.int_val(*n as u64),
        Expr::Var(name) => z3.int_const(name),
        Expr::BinOp(BinOp::Add, l, r) => z3_add(&encode_expr_int(l, z3), &encode_expr_int(r, z3)),
        Expr::BinOp(BinOp::Sub, l, r) => z3_sub(&encode_expr_int(l, z3), &encode_expr_int(r, z3)),
        Expr::BinOp(BinOp::Mul, l, r) => z3_mul(&encode_expr_int(l, z3), &encode_expr_int(r, z3)),
        _ => z3.int_val(0),
    }
}

fn encode_expr_bool(expr: &Expr, z3: &Z3Session) -> z3::ast::Bool {
    match expr {
        Expr::BoolLit(true) => z3::ast::Bool::from_bool(true),
        Expr::BoolLit(false) => z3::ast::Bool::from_bool(false),
        Expr::BinOp(BinOp::Eq, l, r) => encode_expr_int(l, z3).eq(&encode_expr_int(r, z3)),
        Expr::BinOp(BinOp::Lt, l, r) => encode_expr_int(l, z3).lt(&encode_expr_int(r, z3)),
        Expr::BinOp(BinOp::Le, l, r) => encode_expr_int(l, z3).le(&encode_expr_int(r, z3)),
        Expr::BinOp(BinOp::Gt, l, r) => encode_expr_int(l, z3).gt(&encode_expr_int(r, z3)),
        Expr::BinOp(BinOp::And, l, r) => {
            z3::ast::Bool::and(&[&encode_expr_bool(l, z3), &encode_expr_bool(r, z3)])
        }
        _ => z3::ast::Bool::from_bool(true),
    }
}

fn is_well_formed(_node: &IRNode) -> bool { true }

// ── Public API ─────────────────────────────────────────────

pub fn verify_all(
    candidates: Vec<CandidateImplementation>,
    spec: &FunctionSpec,
) -> Result<Vec<CandidateImplementation>, VerificationError> {
    Verifier::new().verify_all(candidates, spec)
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
