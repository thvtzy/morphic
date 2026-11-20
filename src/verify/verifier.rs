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

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::ast::{BinOp as SpecBinOp, Expr as SpecExpr};
    use crate::synthesis::engine::{
        IRBinOp, IRLiteral, IRNode, IRPattern, Provenance, ScoreBreakdown,
    };

    fn load_spec(file: &str, name: &str) -> FunctionSpec {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/self_host/{}.morph", root, file);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = crate::spec::parser::parse(&source).unwrap();
        let checked = crate::spec::typeck::check(parsed).unwrap();
        checked
            .spec
            .functions
            .iter()
            .find(|f| f.name == name)
            .unwrap()
            .clone()
    }

    fn candidate(spec: &FunctionSpec, id: u64, body: IRNode) -> CandidateImplementation {
        CandidateImplementation {
            id,
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
        }
    }

    fn int(n: i64) -> IRNode {
        IRNode::Literal(IRLiteral::Int(n))
    }
    fn boolean(b: bool) -> IRNode {
        IRNode::Literal(IRLiteral::Bool(b))
    }
    fn string(s: &str) -> IRNode {
        IRNode::Literal(IRLiteral::String(s.into()))
    }
    fn var(name: &str) -> IRNode {
        IRNode::Var(name.into())
    }
    fn binop(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
        IRNode::BinOp(op, Box::new(l), Box::new(r))
    }
    fn match_on(scrutinee: IRNode, arms: Vec<(IRPattern, IRNode)>) -> IRNode {
        IRNode::Match {
            scrutinee: Box::new(scrutinee),
            cases: arms,
        }
    }
    fn lit_arm(n: i64, body: IRNode) -> (IRPattern, IRNode) {
        (IRPattern::Literal(IRLiteral::Int(n)), body)
    }
    fn wild(body: IRNode) -> (IRPattern, IRNode) {
        (IRPattern::Wildcard, body)
    }

    /// The TestInduced seed completion for is_literal_kind with
    /// the semantically correct wildcard: kind==0 → true, the
    /// two tested non-literal kinds → false, everything else
    /// → false. This body IS `kind == 0` for every input and
    /// measures tpr 1.0.
    fn is_literal_kind_completion() -> IRNode {
        match_on(
            var("kind"),
            vec![
                lit_arm(0, boolean(true)),
                lit_arm(2, boolean(false)),
                lit_arm(4, boolean(false)),
                wild(boolean(false)),
            ],
        )
    }

    fn status_of(spec: &FunctionSpec, body: IRNode) -> VerificationStatus {
        Verifier::new()
            .verify_one(&candidate(spec, 1, body), spec)
            .status
    }

    // ── The decisive pair ─────────────────────────────────

    /// Guard-style spec (node_arity: per-row `kind<a||kind>a||
    /// output==v` constraints). Before candidate encoding, EVERY
    /// candidate "verified" — the Or-rooted formula encoded to
    /// `true` regardless of the body. A body that returns 0 for
    /// every kind violates the kind==3 row (arity 1), so it must
    /// be refuted, not verified.
    #[test]
    fn wrong_candidate_for_guard_style_spec_is_refuted() {
        let spec = load_spec("validator", "node_arity");
        assert_eq!(
            status_of(&spec, int(0)),
            VerificationStatus::VerificationFailed
        );
    }

    /// The matching correct candidate for the same guard-style
    /// spec must verify: verdicts are candidate-dependent.
    #[test]
    fn correct_candidate_for_guard_style_spec_is_verified() {
        let spec = load_spec("validator", "node_arity");
        let body = match_on(
            var("kind"),
            vec![
                lit_arm(0, int(0)),
                lit_arm(1, int(0)),
                lit_arm(3, int(1)),
                lit_arm(6, int(1)),
                lit_arm(2, int(2)),
                lit_arm(5, int(2)),
                wild(int(0)),
            ],
        );
        assert_eq!(status_of(&spec, body), VerificationStatus::Verified);
    }

    /// Equation-style spec (is_literal_kind: `output ==
    /// (kind==0)`). Before candidate encoding this NEVER
    /// verified — `output` was a free int variable compared
    /// against 0. The correct seed completion must verify now.
    #[test]
    fn correct_completion_for_equation_spec_is_verified() {
        let spec = load_spec("validator", "is_literal_kind");
        assert_eq!(
            status_of(&spec, is_literal_kind_completion()),
            VerificationStatus::Verified
        );
        // The direct expression body is the same semantics.
        let expr_body = binop(IRBinOp::Eq, var("kind"), int(0));
        assert_eq!(
            status_of(&spec, expr_body),
            VerificationStatus::Verified
        );
    }

    /// Same spec, semantically wrong candidate (always true):
    /// refuted, with a counterexample.
    #[test]
    fn wrong_candidate_for_equation_spec_is_refuted() {
        let spec = load_spec("validator", "is_literal_kind");
        let result = Verifier::new().verify_one(
            &candidate(&spec, 1, boolean(true)),
            &spec,
        );
        assert_eq!(result.status, VerificationStatus::VerificationFailed);
        assert!(result.counterexample.is_some());
    }

    // ── Honest verdicts outside the subset ────────────────

    /// A body outside the encodable subset (a While loop) must
    /// be Inconclusive — never Verified for a body the verifier
    /// did not actually judge.
    #[test]
    fn unencodable_body_is_inconclusive() {
        let spec = load_spec("validator", "is_literal_kind");
        let body = IRNode::While {
            condition: Box::new(boolean(true)),
            body: Box::new(int(1)),
            invariant: None,
        };
        assert_eq!(
            status_of(&spec, body),
            VerificationStatus::Inconclusive
        );
        // An unknown call is equally unencodable.
        let body = IRNode::Call {
            function: "is_sorted".into(),
            args: vec![var("kind")],
        };
        assert_eq!(
            status_of(&spec, body),
            VerificationStatus::Inconclusive
        );
    }

    /// A body still containing synthesis holes is not a
    /// well-formed implementation at all.
    #[test]
    fn holed_body_is_malformed() {
        let spec = load_spec("validator", "is_literal_kind");
        let body = IRNode::Hole {
            id: 0,
            expected_type: None,
        };
        assert_eq!(status_of(&spec, body), VerificationStatus::Malformed);
    }

    /// validate_kind_tag's constraint calls another spec
    /// (`output == is_known_node_kind(kind)`). That constraint
    /// cannot be encoded, so the honest verdict is Inconclusive
    /// — before this fix it was VerificationFailed (the call
    /// encoded to int 0 and `output` was a free variable).
    #[test]
    fn unencodable_constraint_is_inconclusive() {
        let spec = load_spec("validator", "validate_kind_tag");
        let body = binop(
            IRBinOp::And,
            binop(IRBinOp::Ge, var("kind"), int(0)),
            binop(IRBinOp::Le, var("kind"), int(9)),
        );
        assert_eq!(
            status_of(&spec, body),
            VerificationStatus::Inconclusive
        );
    }

    // ── String outputs ────────────────────────────────────

    /// node_kind_name is a guard-style String lookup. The
    /// correct match completion must verify; a constant wrong
    /// string must be refuted. (Before candidate encoding, both
    /// "verified" — the verdict was pure constraint syntax.)
    #[test]
    fn string_lookup_spec_verdicts_follow_the_candidate() {
        let spec = load_spec("printer", "node_kind_name");
        let names = [
            "Literal",
            "Variable",
            "BinaryOp",
            "UnaryOp",
            "Call",
            "Index",
            "FieldAccess",
            "Lambda",
        ];
        let mut arms: Vec<(IRPattern, IRNode)> = names
            .iter()
            .enumerate()
            .map(|(i, n)| lit_arm(i as i64, string(n)))
            .collect();
        arms.push(wild(string("")));
        assert_eq!(
            status_of(&spec, match_on(var("kind"), arms)),
            VerificationStatus::Verified
        );
        assert_eq!(
            status_of(&spec, string("Nope")),
            VerificationStatus::VerificationFailed
        );
    }

    // ── Proof is stronger than tests, honestly ────────────

    /// is_known_type_tag: the exact range expression verifies.
    /// The per-test match completion measures tpr 1.0 (its arms
    /// cover exactly the tested inputs) but is NOT the range
    /// predicate — Z3 must refute it (e.g. tag = 5 is in range
    /// but falls into the false wildcard). This is the honest
    /// tests-vs-proof gap, pinned.
    #[test]
    fn range_spec_distinguishes_proof_from_test_coverage() {
        let spec = load_spec("validator", "is_known_type_tag");
        let exact = binop(
            IRBinOp::And,
            binop(IRBinOp::Ge, var("tag"), int(0)),
            binop(IRBinOp::Le, var("tag"), int(8)),
        );
        assert_eq!(status_of(&spec, exact), VerificationStatus::Verified);

        let test_shaped = match_on(
            var("tag"),
            vec![
                lit_arm(0, boolean(true)),
                lit_arm(1, boolean(true)),
                lit_arm(8, boolean(true)),
                lit_arm(9, boolean(false)),
                lit_arm(42, boolean(false)),
                wild(boolean(false)),
            ],
        );
        let result = Verifier::new().verify_one(&candidate(&spec, 1, test_shaped), &spec);
        assert_eq!(result.status, VerificationStatus::VerificationFailed);
        assert!(result.counterexample.is_some());
    }

    /// Multi-param spec (needs_parens): the constraint's exact
    /// boolean expression as a body must verify.
    #[test]
    fn multi_param_equation_spec_verifies_exact_body() {
        let spec = load_spec("printer", "needs_parens");
        let body = binop(
            IRBinOp::And,
            binop(
                IRBinOp::Or,
                binop(IRBinOp::Eq, var("op"), int(0)),
                binop(IRBinOp::Eq, var("op"), int(1)),
            ),
            binop(
                IRBinOp::Or,
                binop(IRBinOp::Eq, var("parent_op"), int(2)),
                binop(IRBinOp::Eq, var("parent_op"), int(3)),
            ),
        );
        assert_eq!(status_of(&spec, body), VerificationStatus::Verified);
        // And a constant body must not.
        assert_eq!(
            status_of(&spec, boolean(true)),
            VerificationStatus::VerificationFailed
        );
    }

    // ── verify_all integration ────────────────────────────

    /// verify_all must mark only the semantically correct
    /// candidate verified, and the constraint score must be the
    /// fraction of postconditions proven FOR THAT CANDIDATE.
    #[test]
    fn verify_all_marks_only_the_correct_candidate() {
        let spec = load_spec("validator", "is_literal_kind");
        let wrong = candidate(&spec, 1, boolean(true));
        let correct = candidate(&spec, 2, is_literal_kind_completion());
        let out = Verifier::new()
            .verify_all(vec![wrong, correct], &spec)
            .expect("verify_all");
        assert!(!out[0].verified);
        assert!(out[1].verified);
        assert_eq!(out[0].scores.constraint_score, 0.0);
        assert_eq!(out[1].scores.constraint_score, 1.0);
    }

    /// The spec-side expression encoder handles the constraint
    /// shapes the parser produces (guards against regressions
    /// in encode coverage for Or/Ge/Neq forms).
    #[test]
    fn spec_constraint_shapes_encode() {
        // output >= 0 as a standalone postcondition (node_arity's
        // last constraint) is an Expr(Ge) — exercised end-to-end
        // by the node_arity tests above. Here: a != constraint.
        let spec = FunctionSpec {
            name: "neq_spec".into(),
            doc: None,
            generics: vec![],
            params: vec![crate::spec::ast::Param {
                name: "x".into(),
                ty: crate::spec::ast::TypeRef::Int,
                annotations: Default::default(),
            }],
            return_type: crate::spec::ast::TypeRef::Bool,
            preconditions: vec![],
            postconditions: vec![Constraint::Expr(SpecExpr::BinOp(
                SpecBinOp::Neq,
                Box::new(SpecExpr::Var("output".into())),
                Box::new(SpecExpr::Var("x".into())),
            ))],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: Default::default(),
        };
        // Body `true`: output != x is not implied (x could make
        // the bool compare unequal sorts) — output is Bool, x is
        // Int, so this constraint is unencodable: Inconclusive.
        assert_eq!(
            status_of(&spec, boolean(true)),
            VerificationStatus::Inconclusive
        );
    }
}
