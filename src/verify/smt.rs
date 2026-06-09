// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC SMT LAYER (v0.2)                                │
// │  z3 crate v0.20 — thread-local context API               │
// │  gh-release: Z3 binary auto-downloaded, no cmake needed   │
// └──────────────────────────────────────────────────────────┘

use std::collections::HashMap;

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

// ── Z3 Session (v0.20 thread-local API) ───────────────────

/// A Z3 verification session.
/// v0.20 uses implicit thread-local context — no explicit Context passing.
pub struct Z3Session {
    solver: z3::Solver,
    timeout_ms: u64,
}

impl Z3Session {
    pub fn new(timeout_ms: u64) -> Self {
        let solver = z3::Solver::new();
        Self { solver, timeout_ms }
    }

    /// Create integer constant
    pub fn int_const(&self, name: &str) -> z3::ast::Int {
        z3::ast::Int::fresh_const(name)
    }

    /// Create boolean constant
    pub fn bool_const(&self, name: &str) -> z3::ast::Bool {
        z3::ast::Bool::fresh_const(name)
    }

    /// Integer literal
    pub fn int_val(&self, n: u64) -> z3::ast::Int {
        z3::ast::Int::from_u64(n)
    }

    /// Assert formula into solver
    pub fn assert(&self, formula: &z3::ast::Bool) {
        self.solver.assert(formula);
    }

    /// Check satisfiability (no model)
    pub fn check(&self) -> SmtResult {
        match self.solver.check() {
            z3::SatResult::Sat    => SmtResult::Sat(HashMap::new()),
            z3::SatResult::Unsat  => SmtResult::Unsat,
            z3::SatResult::Unknown => SmtResult::Unknown,
        }
    }

    /// Check satisfiability + extract model
    pub fn check_and_get_model(&self) -> SmtResult {
        match self.solver.check() {
            z3::SatResult::Sat => {
                if let Some(model) = self.solver.get_model() {
                    SmtResult::Sat(extract_model(&model))
                } else {
                    SmtResult::Sat(HashMap::new())
                }
            }
            z3::SatResult::Unsat  => SmtResult::Unsat,
            z3::SatResult::Unknown => SmtResult::Unknown,
        }
    }
}

// ── Arithmetic helpers ─────────────────────────────────────

pub fn z3_add(a: &z3::ast::Int, b: &z3::ast::Int) -> z3::ast::Int {
    z3::ast::Int::add(&[a, b])
}

pub fn z3_sub(a: &z3::ast::Int, b: &z3::ast::Int) -> z3::ast::Int {
    z3::ast::Int::sub(&[a, b])
}

pub fn z3_mul(a: &z3::ast::Int, b: &z3::ast::Int) -> z3::ast::Int {
    z3::ast::Int::mul(&[a, b])
}

pub fn z3_and(a: &z3::ast::Bool, b: &z3::ast::Bool) -> z3::ast::Bool {
    z3::ast::Bool::and(&[a, b])
}

fn extract_model(_model: &z3::Model) -> HashMap<String, SmtValue> {
    // v0.20: model.iter() returns FuncDecl objects.
    // Model extraction requires tracking created vars — simplified for v0.2.
    // Full counterexample extraction will be added when we track var bindings.
    HashMap::new()
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_z3_sat() {
        let s = Z3Session::new(5000);
        let x = s.int_const("x");
        let y = s.int_const("y");
        let five = s.int_val(5);
        s.assert(&x.gt(&five));
        s.assert(&y.lt(&x));
        match s.check() {
            SmtResult::Sat(_) => {} // x=6, y=5 works
            other => panic!("Expected SAT, got {:?}", other),
        }
    }

    #[test]
    fn test_z3_unsat() {
        let s = Z3Session::new(5000);
        let x = s.int_const("x");
        s.assert(&x.gt(&s.int_val(5)));
        s.assert(&x.lt(&s.int_val(0)));
        match s.check() {
            SmtResult::Unsat => {}
            other => panic!("Expected UNSAT, got {:?}", other),
        }
    }

    #[test]
    fn test_arithmetic() {
        let s = Z3Session::new(5000);
        let x = s.int_const("x");
        let three = s.int_val(3);
        let ten = s.int_val(10);
        let sum = z3_add(&x, &three);
        s.assert(&sum.eq(&ten));
        match s.check() {
            SmtResult::Sat(_) => {} // x = 7 works
            other => panic!("Expected SAT, got {:?}", other),
        }
    }
}
