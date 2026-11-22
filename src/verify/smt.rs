// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC SMT LAYER (v0.2)                                │
// │  z3 crate v0.20 — thread-local context API               │
// │  gh-release: Z3 binary auto-downloaded, no cmake needed   │
// └──────────────────────────────────────────────────────────┘

use std::collections::HashMap;

// The solver's result VALUES are pure data and live in
// super::verdict (Phase 66) — only the session below needs
// Z3. Re-exported so `verify::smt::{SmtResult, SmtValue}`
// keeps resolving for existing callers.
pub use super::verdict::{SmtResult, SmtValue};

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
        // The timeout was previously stored but never applied;
        // with candidate bodies encoded, solver calls do real
        // work and the bound matters.
        let mut params = z3::Params::new();
        params.set_u32("timeout", timeout_ms as u32);
        solver.set_params(&params);
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

    /// Check satisfiability; on Sat, evaluate the given Int
    /// constants in the model so callers get real counterexample
    /// values for the variables they created. Constants the
    /// model does not pin to a numeral are omitted.
    pub fn check_and_eval_ints(&self, vars: &[(&str, &z3::ast::Int)]) -> SmtResult {
        match self.solver.check() {
            z3::SatResult::Sat => {
                let mut values = HashMap::new();
                if let Some(model) = self.solver.get_model() {
                    for (name, ast) in vars {
                        if let Some(v) = model.eval(*ast, true) {
                            if let Some(n) = v.as_i64() {
                                values.insert(name.to_string(), SmtValue::Int(n));
                            }
                        }
                    }
                }
                SmtResult::Sat(values)
            }
            z3::SatResult::Unsat  => SmtResult::Unsat,
            z3::SatResult::Unknown => SmtResult::Unknown,
        }
    }

    /// `check_and_eval_ints` under the F2 refutation
    /// discipline (Phase 100, decision D4a — the
    /// refutation-degradation rule).
    ///
    /// THE RULE, precisely: a Sat verdict over a judged
    /// encoding that contains applications of the
    /// `int_to_str` UNINTERPRETED function may stand as a
    /// REFUTATION only if the countermodel is still a
    /// countermodel when the UF is replaced by the real
    /// conversion. That condition is checkable on the
    /// model itself: for every UF application `(arg, app)`
    /// the caller collected from the encoding, evaluate
    /// both terms in the model; the application AGREES
    /// iff the argument evaluates to a numeral `n` and
    /// the application to the string `format!("{}", n)`
    /// — the real conversion's rendering. (Term values
    /// are compositional, so if every UF application in
    /// the encoding agrees, every term's model value is
    /// unchanged under the real conversion and the model
    /// genuinely refutes.) If any application disagrees,
    /// or cannot be evaluated to a numeral/string pair,
    /// or the caller flags an UNINSPECTABLE occurrence
    /// (a UF application under a quantifier, which the
    /// agreement walk cannot enter and whose model value
    /// is not a single point), the Sat verdict DEGRADES
    /// to `SmtResult::Unknown`: the candidate is
    /// Inconclusive, never VerificationFailed on a
    /// possibly fabricated countermodel. Proven verdicts
    /// (Unsat) are untouched — a proof under a UF holds
    /// for every interpretation of the symbol, hence for
    /// the real conversion. With no applications and no
    /// flag this method is `check_and_eval_ints` exactly.
    pub fn check_and_eval_ints_uf_guarded(
        &self,
        vars: &[(&str, &z3::ast::Int)],
        uf_apps: &[(z3::ast::Int, z3::ast::String)],
        uf_uninspectable: bool,
    ) -> SmtResult {
        match self.solver.check() {
            z3::SatResult::Sat => {
                if uf_uninspectable {
                    return SmtResult::Unknown;
                }
                let Some(model) = self.solver.get_model() else {
                    // No model to check the applications
                    // against: with applications present
                    // the verdict cannot be trusted.
                    return if uf_apps.is_empty() {
                        SmtResult::Sat(HashMap::new())
                    } else {
                        SmtResult::Unknown
                    };
                };
                for (arg, app) in uf_apps {
                    let agrees = match (model.eval(arg, true), model.eval(app, true)) {
                        (Some(av), Some(sv)) => match (av.as_i64(), sv.as_string()) {
                            (Some(n), Some(s)) => s == format!("{}", n),
                            _ => false,
                        },
                        _ => false,
                    };
                    if !agrees {
                        return SmtResult::Unknown;
                    }
                }
                let mut values = HashMap::new();
                for (name, ast) in vars {
                    if let Some(v) = model.eval(*ast, true) {
                        if let Some(n) = v.as_i64() {
                            values.insert(name.to_string(), SmtValue::Int(n));
                        }
                    }
                }
                SmtResult::Sat(values)
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

    #[test]
    fn test_uf_guarded_check() {
        use std::str::FromStr;
        // Phase 100 (F2, D4a): the UF-guarded check. With
        // no UF applications and no uninspectable flag it
        // behaves exactly like `check_and_eval_ints`.
        let s = Z3Session::new(5000);
        let x = s.int_const("x");
        s.assert(&x.gt(&s.int_val(5)));
        match s.check_and_eval_ints_uf_guarded(&[("x", &x)], &[], false) {
            SmtResult::Sat(m) => {
                assert!(matches!(m.get("x"), Some(SmtValue::Int(n)) if *n > 5))
            }
            other => panic!("expected Sat with x > 5, got {:?}", other),
        }
        // Forced disagreement: `int_to_str(40 + 2) != "42"`
        // is satisfiable ONLY by interpreting the UF as a
        // function the real conversion is not — the
        // argument evaluates to 42 and the real conversion
        // of 42 IS "42". The would-be countermodel is
        // fabricated, so the verdict degrades to Unknown.
        let s2 = Z3Session::new(5000);
        let uf = z3::FuncDecl::new(
            "int_to_str",
            &[&z3::Sort::int()],
            &z3::Sort::string(),
        );
        let arg = z3_add(&z3::ast::Int::from_i64(40), &z3::ast::Int::from_i64(2));
        let app = uf
            .apply(&[&arg as &dyn z3::ast::Ast])
            .as_string()
            .expect("UF range is String");
        let forty_two = z3::ast::String::from_str("42").unwrap();
        s2.assert(&app.eq(&forty_two).not());
        match s2.check_and_eval_ints_uf_guarded(&[], &[(arg, app)], false) {
            SmtResult::Unknown => {}
            other => panic!(
                "expected degraded Unknown for a UF-fabricated model, got {:?}",
                other
            ),
        }
        // The uninspectable flag (a UF occurrence under a
        // quantifier the agreement walk could not enter)
        // degrades even an otherwise plain Sat.
        let s3 = Z3Session::new(5000);
        let y = s3.int_const("y");
        s3.assert(&y.gt(&s3.int_val(0)));
        match s3.check_and_eval_ints_uf_guarded(&[("y", &y)], &[], true) {
            SmtResult::Unknown => {}
            other => panic!("expected degraded Unknown, got {:?}", other),
        }
    }
}
