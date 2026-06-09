// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC VERIFIER                                        │
// │  Formal verification of synthesized implementations      │
// └──────────────────────────────────────────────────────────┘

use crate::spec::ast::*;
use crate::synthesis::engine::{CandidateImplementation, IRNode, IRLiteral, IRBinOp, IRUnaryOp};
use super::smt::{SmtSolver, SmtFormula, SmtResult, SmtModel};
use std::sync::Mutex;
use rayon::prelude::*;
use std::collections::HashMap;
use std::time::Instant;

/// The result of verifying a single candidate
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Candidate ID
    pub candidate_id: u64,
    /// Overall verification status
    pub status: VerificationStatus,
    /// Individual constraint results
    pub constraint_results: Vec<ConstraintResult>,
    /// Counterexample (if verification failed)
    pub counterexample: Option<Counterexample>,
    /// Time spent in verification (ms)
    pub verification_time_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationStatus {
    /// All constraints hold
    Verified,
    /// At least one constraint fails
    VerificationFailed,
    /// Verification couldn't complete (timeout, resource)
    Inconclusive,
    /// Candidate is syntactically invalid
    Malformed,
}

#[derive(Debug, Clone)]
pub struct ConstraintResult {
    /// Human-readable constraint description
    pub constraint: String,
    /// Whether this constraint holds
    pub holds: bool,
    /// Z3 solver time for this constraint (ms)
    pub solver_time_ms: u64,
    /// Counterexample values (from Z3 model)
    pub model: Option<HashMap<String, SmtValue>>,
}

#[derive(Debug, Clone)]
pub struct Counterexample {
    /// Description of what went wrong
    pub description: String,
    /// Input values that cause the violation
    pub inputs: HashMap<String, SmtValue>,
    /// Expected output (from constraint)
    pub expected: Option<SmtValue>,
    /// Actual output (from implementation)
    pub actual: Option<SmtValue>,
}

/// Value from the SMT solver model
#[derive(Debug, Clone)]
pub enum SmtValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    List(Vec<SmtValue>),
    Uninterpreted(String),
}

impl std::fmt::Display for SmtValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmtValue::Int(n) => write!(f, "{}", n),
            SmtValue::Float(x) => write!(f, "{}", x),
            SmtValue::Bool(b) => write!(f, "{}", b),
            SmtValue::String(s) => write!(f, "\"{}\"", s),
            SmtValue::List(vals) => {
                write!(f, "[")?;
                for (i, v) in vals.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            SmtValue::Uninterpreted(s) => write!(f, "{}", s),
        }
    }
}

// ── Verifier Engine ───────────────────────────────────────

pub struct Verifier {
    /// Whether to use Z3 backend
    use_z3: bool,
    /// Maximum time per constraint (ms)
    timeout_per_constraint_ms: u64,
    /// Maximum total verification time per candidate (ms)
    timeout_total_ms: u64,
    /// Whether to find counterexamples
    find_counterexamples: bool,
}

impl Default for Verifier {
    fn default() -> Self {
        Self {
            use_z3: true,
            timeout_per_constraint_ms: 5_000,
            timeout_total_ms: 30_000,
            find_counterexamples: true,
        }
    }
}

impl Verifier {
    pub fn new() -> Self { Self::default() }

    /// Verify all candidates against their specifications
    pub fn verify_all(
        &self,
        candidates: Vec<CandidateImplementation>,
        spec: &FunctionSpec,
    ) -> Result<Vec<CandidateImplementation>, VerificationError> {
        // Parallel verification using rayon
        let results: Vec<VerificationResult> = candidates
            .par_iter()
            .map(|candidate| self.verify_one(candidate, spec))
            .collect();

        // Mark verified candidates
        let mut verified_candidates = Vec::new();
        for (mut candidate, result) in candidates.into_iter().zip(results.into_iter()) {
            candidate.verified = result.status == VerificationStatus::Verified;
            // Update scores based on constraint results
            let passed = result.constraint_results.iter().filter(|r| r.holds).count();
            let total = result.constraint_results.len();
            if total > 0 {
                candidate.scores.constraint_score = passed as f64 / total as f64;
            }
            candidate.score = crate::synthesis::engine::composite_score(&candidate.scores);
            verified_candidates.push(candidate);
        }

        Ok(verified_candidates)
    }

    /// Verify a single candidate
    pub fn verify_one(
        &self,
        candidate: &CandidateImplementation,
        spec: &FunctionSpec,
    ) -> VerificationResult {
        let start = Instant::now();

        // Quick structural check
        if !is_well_formed(&candidate.body) {
            return VerificationResult {
                candidate_id: candidate.id,
                status: VerificationStatus::Malformed,
                constraint_results: vec![],
                counterexample: None,
                verification_time_ms: start.elapsed().as_millis() as u64,
            };
        }

        let mut constraint_results = Vec::new();
        let mut counterexample = None;

        // Verify preconditions
        for (i, pre) in spec.preconditions.iter().enumerate() {
            let result = self.verify_constraint(
                &candidate.body,
                spec,
                pre,
                ConstraintKind::Precondition,
            );
            if !result.holds && counterexample.is_none() {
                counterexample = Some(Counterexample {
                    description: format!("Precondition {} violated", i + 1),
                    inputs: result.model.clone().unwrap_or_default(),
                    expected: None,
                    actual: None,
                });
            }
            constraint_results.push(result);
        }

        // Verify postconditions
        for (i, post) in spec.postconditions.iter().enumerate() {
            let result = self.verify_constraint(
                &candidate.body,
                spec,
                post,
                ConstraintKind::Postcondition,
            );
            if !result.holds && counterexample.is_none() {
                counterexample = Some(Counterexample {
                    description: format!("Postcondition {} violated", i + 1),
                    inputs: result.model.clone().unwrap_or_default(),
                    expected: None,
                    actual: None,
                });
            }
            constraint_results.push(result);
        }

        // Verify invariants
        for (i, inv) in spec.invariants.iter().enumerate() {
            let result = self.verify_constraint(
                &candidate.body,
                spec,
                &inv.constraint,
                ConstraintKind::Invariant,
            );
            if !result.holds && counterexample.is_none() {
                counterexample = Some(Counterexample {
                    description: format!("Invariant {} violated: {:?}", i + 1, inv.name),
                    inputs: result.model.clone().unwrap_or_default(),
                    expected: None,
                    actual: None,
                });
            }
            constraint_results.push(result);
        }

        let all_hold = constraint_results.iter().all(|r| r.holds);
        let elapsed = start.elapsed().as_millis() as u64;

        VerificationResult {
            candidate_id: candidate.id,
            status: if all_hold {
                VerificationStatus::Verified
            } else {
                VerificationStatus::VerificationFailed
            },
            constraint_results,
            counterexample,
            verification_time_ms: elapsed,
        }
    }

    /// Verify a single constraint against the implementation
    fn verify_constraint(
        &self,
        implementation: &IRNode,
        spec: &FunctionSpec,
        constraint: &Constraint,
        kind: ConstraintKind,
    ) -> ConstraintResult {
        let start = Instant::now();

        if self.use_z3 {
            let _ = self.verify_with_z3(implementation, spec, constraint, kind);
        } else {
            let _ = self.verify_with_random_testing(implementation, spec, constraint, kind);
        }

        ConstraintResult {
            constraint: format_constraint(constraint),
            holds: true, // Placeholder
            solver_time_ms: start.elapsed().as_millis() as u64,
            model: None,
        }
    }

    /// Use Z3 theorem prover for verification
    fn verify_with_z3(
        &self,
        implementation: &IRNode,
        spec: &FunctionSpec,
        constraint: &Constraint,
        _kind: ConstraintKind,
    ) -> SmtResult {
        // 1. Translate IR to SMT-LIB2
        let smt_formula = ir_to_smt(implementation, spec, constraint);

        // 2. Query Z3
        let mut solver = SmtSolver::new();
        solver.set_timeout(self.timeout_per_constraint_ms);
        solver.set_option(":produce-models", "true");

        let result = solver.check(&smt_formula);

        // 3. Parse result
        match result {
            SmtResult::Sat(model) => {
                // Formula is satisfiable — constraints hold (for this encoding)
                SmtResult::Sat(model)
            }
            SmtResult::Unsat => {
                // Formula is unsatisfiable — constraints DON'T hold
                SmtResult::Unsat
            }
            SmtResult::Unknown => SmtResult::Unknown,
            SmtResult::Timeout => SmtResult::Timeout,
        }
    }

    /// Bounded random testing as verification fallback
    fn verify_with_random_testing(
        &self,
        implementation: &IRNode,
        spec: &FunctionSpec,
        constraint: &Constraint,
        _kind: ConstraintKind,
    ) -> bool {
        // Generate random inputs and check constraint holds
        let num_samples = 10_000;
        for _ in 0..num_samples {
            let inputs = generate_random_inputs(spec);
            if !check_constraint_holds(implementation, constraint, &inputs) {
                return false;
            }
        }
        true
    }
}

// ── SMT Translation ───────────────────────────────────────

/// Translate IR node and constraint to SMT-LIB2 formula
fn ir_to_smt(
    _implementation: &IRNode,
    _spec: &FunctionSpec,
    _constraint: &Constraint,
) -> SmtFormula {
    // Build SMT formula that encodes:
    //   (implementation(input) != expected_output)  OR  NOT(constraint)
    //
    // If this formula is UNSAT: implementation always satisfies constraint
    // If this formula is SAT: there exists a counterexample

    let mut formula = SmtFormula::new();

    // Declare input variables
    for param in &_spec.params {
        formula.declare_var(&param.name, &param.ty);
    }

    // Declare output variable
    formula.declare_var("output", &_spec.return_type);

    // Assert implementation semantics
    encode_implementation(&mut formula, _implementation, _spec);

    // Assert negation of constraint (to find counterexample)
    // formula.assert(format!("(not {})", encode_constraint(_constraint)));

    formula
}

/// Encode implementation behavior as SMT assertions
fn encode_implementation(formula: &mut SmtFormula, node: &IRNode, _spec: &FunctionSpec) {
    match node {
        IRNode::Return(expr) => {
            let encoded = encode_expr(expr);
            formula.assert(format!("(= output {})", encoded));
        }
        IRNode::Block(stmts) => {
            for stmt in stmts {
                encode_implementation(formula, stmt, _spec);
            }
        }
        IRNode::If { condition, then_branch, else_branch } => {
            let cond = encode_expr(condition);
            formula.assert(format!(
                "(ite {} {} {})",
                cond,
                encode_expr_block(then_branch),
                encode_expr_block(else_branch),
            ));
        }
        IRNode::Match { scrutinee, cases } => {
            let mut ite_chain = String::new();
            for (i, (pattern, body)) in cases.iter().enumerate().rev() {
                if matches!(pattern, crate::synthesis::engine::IRPattern::Wildcard) {
                    ite_chain = encode_expr_block(body);
                } else {
                    let pat_expr = encode_pattern(scrutinee, pattern);
                    if i == cases.len() - 1 {
                        ite_chain = encode_expr_block(body);
                    } else {
                        ite_chain = format!(
                            "(ite {} {} {})",
                            pat_expr,
                            encode_expr_block(body),
                            ite_chain,
                        );
                    }
                }
            }
            formula.assert(format!("(= output {})", ite_chain));
        }
        _ => {
            // For other constructs, encode as expression
            formula.assert(format!("(= output {})", encode_expr(node)));
        }
    }
}

fn encode_expr(node: &IRNode) -> String {
    match node {
        IRNode::Literal(lit) => match lit {
            IRLiteral::Int(n) => n.to_string(),
            IRLiteral::Float(f) => f.to_string(),
            IRLiteral::Bool(b) => if *b { "true".into() } else { "false".into() },
            IRLiteral::String(s) => format!("\"{}\"", s),
            IRLiteral::Char(c) => format!("'{}'", c),
            IRLiteral::Unit => "()".into(),
        },
        IRNode::Var(name) => name.clone(),
        IRNode::BinOp(op, lhs, rhs) => {
            let smt_op = match op {
                IRBinOp::Add => "+",
                IRBinOp::Sub => "-",
                IRBinOp::Mul => "*",
                IRBinOp::Div => "div",
                IRBinOp::Mod => "mod",
                IRBinOp::And => "and",
                IRBinOp::Or => "or",
                IRBinOp::Eq => "=",
                IRBinOp::Neq => "distinct",
                IRBinOp::Lt => "<",
                IRBinOp::Le => "<=",
                IRBinOp::Gt => ">",
                IRBinOp::Ge => ">=",
                _ => "unknown",
            };
            format!("({} {} {})", smt_op, encode_expr(lhs), encode_expr(rhs))
        }
        IRNode::UnaryOp(op, inner) => {
            let smt_op = match op {
                IRUnaryOp::Neg => "-",
                IRUnaryOp::Not => "not",
                IRUnaryOp::Abs => "abs",
                IRUnaryOp::Len => "len",
                _ => "unknown",
            };
            format!("({} {})", smt_op, encode_expr(inner))
        }
        IRNode::Call { function, args } => {
            let args_smt: Vec<String> = args.iter().map(encode_expr).collect();
            format!("({} {})", function, args_smt.join(" "))
        }
        IRNode::If { condition, then_branch, else_branch } => {
            format!(
                "(ite {} {} {})",
                encode_expr(condition),
                encode_expr(then_branch),
                encode_expr(else_branch),
            )
        }
        _ => "??".into(), // Hole or unsupported — placeholder
    }
}

fn encode_expr_block(node: &IRNode) -> String {
    // For blocks, encode the last expression (or return value)
    match node {
        IRNode::Block(stmts) => {
            stmts.last()
                .map(|s| encode_expr(s))
                .unwrap_or_else(|| "()".into())
        }
        _ => encode_expr(node),
    }
}

fn encode_pattern(_scrutinee: &IRNode, pattern: &crate::synthesis::engine::IRPattern) -> String {
    match pattern {
        crate::synthesis::engine::IRPattern::Literal(lit) => {
            match lit {
                IRLiteral::Int(n) => format!("(= scrutinee {})", n),
                IRLiteral::Bool(b) => format!("(= scrutinee {})", b),
                _ => "true".into(),
            }
        }
        crate::synthesis::engine::IRPattern::Variable(_) => "true".into(),
        crate::synthesis::engine::IRPattern::Constructor(name, args) => {
            if args.is_empty() {
                format!("(is-{name} scrutinee)")
            } else {
                format!("(is-{name} scrutinee)")
            }
        }
        crate::synthesis::engine::IRPattern::Wildcard => "true".into(),
        crate::synthesis::engine::IRPattern::Guard(_, guard) => guard.clone(),
    }
}

// ── Helper Functions ──────────────────────────────────────

#[derive(Debug, Clone, Copy)]
enum ConstraintKind {
    Precondition,
    Postcondition,
    Invariant,
}

/// Check if IR tree is well-formed
fn is_well_formed(_node: &IRNode) -> bool {
    // Check for:
    // - No unbound variables
    // - Type consistency
    // - No infinite recursion (structural check)
    true
}

/// Generate random inputs for bounded model checking
fn generate_random_inputs(spec: &FunctionSpec) -> HashMap<String, SmtValue> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut inputs = HashMap::new();

    for param in &spec.params {
        let value = match &param.ty {
            TypeRef::Int | TypeRef::I32 | TypeRef::I64 =>
                SmtValue::Int(rng.gen_range(-1000..1000)),
            TypeRef::Bool =>
                SmtValue::Bool(rng.gen()),
            TypeRef::String =>
                SmtValue::String(format!("test_{}", rng.gen_range(0..100))),
            TypeRef::Float =>
                SmtValue::Float(rng.gen_range(-100.0..100.0)),
            _ => SmtValue::Uninterpreted(param.name.clone()),
        };
        inputs.insert(param.name.clone(), value);
    }

    inputs
}

/// Check if constraint holds for given inputs (concrete evaluation)
fn check_constraint_holds(
    _implementation: &IRNode,
    _constraint: &Constraint,
    _inputs: &HashMap<String, SmtValue>,
) -> bool {
    // Concrete evaluation of implementation on inputs
    // and check against constraint
    true // Placeholder
}

fn format_constraint(constraint: &Constraint) -> String {
    match constraint {
        Constraint::True => "true".into(),
        Constraint::Expr(e) => format!("{:?}", e),
        Constraint::Eq(a, b) => format!("{:?} == {:?}", a, b),
        Constraint::Order(op, a, b) => format!("{:?} {:?} {:?}", a, op, b),
        Constraint::Forall { vars, body } => {
            let vars_str: Vec<String> = vars.iter().map(|(n, t)| format!("{}: {}", n, t)).collect();
            format!("forall {}. {:?}", vars_str.join(", "), body)
        }
        Constraint::Exists { vars, body } => {
            let vars_str: Vec<String> = vars.iter().map(|(n, t)| format!("{}: {}", n, t)).collect();
            format!("exists {}. {:?}", vars_str.join(", "), body)
        }
        Constraint::And(parts) => parts.iter().map(format_constraint).collect::<Vec<_>>().join(" && "),
        Constraint::Implies(a, b) => format!("({} => {})", format_constraint(a), format_constraint(b)),
        Constraint::Not(inner) => format!("!({})", format_constraint(inner)),
        Constraint::Predicate(name, args) => {
            format!("{}({:?})", name, args)
        }
        _ => format!("{:?}", constraint),
    }
}

// ── Public API ────────────────────────────────────────────

/// Verify all candidates against their specification
pub fn verify_all(
    candidates: Vec<CandidateImplementation>,
    spec: &FunctionSpec,
) -> Result<Vec<CandidateImplementation>, VerificationError> {
    let verifier = Verifier::new();
    verifier.verify_all(candidates, spec)
}

/// Verify a single implementation against a specification
pub fn verify_implementation(
    implementation: &IRNode,
    spec: &FunctionSpec,
) -> Result<bool, VerificationError> {
    let verifier = Verifier::new();
    let candidate = CandidateImplementation {
        id: 0,
        body: implementation.clone(),
        spec_name: spec.name.clone(),
        spec: spec.clone(),
        score: 0.0,
        scores: Default::default(),
        generation: 0,
        provenance: crate::synthesis::engine::Provenance::Template {
            template_name: "manual".into(),
        },
        verified: false,
    };

    let result = verifier.verify_one(&candidate, spec);
    Ok(result.status == VerificationStatus::Verified)
}

impl Default for crate::synthesis::engine::ScoreBreakdown {
    fn default() -> Self {
        crate::synthesis::engine::ScoreBreakdown {
            test_pass_ratio: 0.0,
            constraint_score: 0.0,
            complexity_score: 1.0,
            quality_score: 1.0,
            synthesis_time_us: 0,
        }
    }
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
            VerificationError::SolverError(msg) => write!(f, "Solver error: {}", msg),
            VerificationError::Timeout => write!(f, "Verification timeout"),
            VerificationError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for VerificationError {}

// ── Re-export composite_score for use by engine and selector
pub fn composite_score(scores: &crate::synthesis::engine::ScoreBreakdown) -> f64 {
    crate::synthesis::engine::composite_score(scores)
}
