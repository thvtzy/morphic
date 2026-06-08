// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC SMT LAYER                                       │
// │  Interface to Z3 Theorem Prover (SMT-LIB2)               │
// └──────────────────────────────────────────────────────────┘

use crate::spec::ast::TypeRef;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::io::Write;

/// An SMT-LIB2 formula ready for the solver
#[derive(Debug, Clone)]
pub struct SmtFormula {
    /// Variable declarations
    declarations: Vec<SmtDecl>,
    /// Assertions
    assertions: Vec<String>,
    /// Options
    options: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
struct SmtDecl {
    name: String,
    sort: String,
}

impl SmtFormula {
    pub fn new() -> Self {
        Self {
            declarations: Vec::new(),
            assertions: Vec::new(),
            options: vec![
                (":produce-models".into(), "true".into()),
                (":timeout".into(), "5000".into()),
            ],
        }
    }

    /// Declare a variable with its Morphic type mapped to SMT sort
    pub fn declare_var(&mut self, name: &str, ty: &TypeRef) {
        let sort = type_to_smt_sort(ty);
        self.declarations.push(SmtDecl {
            name: name.into(),
            sort,
        });
    }

    /// Add an assertion
    pub fn assert(&mut self, formula: String) {
        self.assertions.push(formula);
    }

    /// Set a solver option
    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.push((key.into(), value.into()));
    }

    /// Serialize to SMT-LIB2 string
    pub fn to_smtlib2(&self) -> String {
        let mut out = String::new();

        // Set logic
        out.push_str("(set-logic QF_UFLIA)\n");

        // Options
        for (key, value) in &self.options {
            out.push_str(&format!("(set-option {} {})\n", key, value));
        }

        // Declarations
        for decl in &self.declarations {
            out.push_str(&format!(
                "(declare-fun {} () {})\n",
                decl.name, decl.sort
            ));
        }

        // Assertions
        for assertion in &self.assertions {
            out.push_str(&format!("(assert {})\n", assertion));
        }

        out.push_str("(check-sat)\n");

        // Only get model if SAT
        out.push_str("(get-model)\n");

        out
    }

    pub fn to_z3_string(&self) -> String {
        self.to_smtlib2()
    }
}

fn type_to_smt_sort(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Int | TypeRef::I8 | TypeRef::I16 | TypeRef::I32 | TypeRef::I64 |
        TypeRef::U8 | TypeRef::U16 | TypeRef::U32 | TypeRef::U64 => "Int".into(),
        TypeRef::Float => "Real".into(),
        TypeRef::Bool => "Bool".into(),
        TypeRef::String | TypeRef::Char => "String".into(),
        TypeRef::List(t) => format!("(List {})", type_to_smt_sort(t)),
        TypeRef::Array(t, n) => format!("(Array Int {})", type_to_smt_sort(t)),
        TypeRef::Option(t) => {
            format!("(Option {})", type_to_smt_sort(t))
        }
        TypeRef::Named(name) => name.clone(),
        TypeRef::Generic(name) => name.clone(),
        _ => "Int".into(), // Default fallback
    }
}

// ── SMT Solver ────────────────────────────────────────────

pub struct SmtSolver {
    timeout_ms: u64,
    options: HashMap<String, String>,
}

impl SmtSolver {
    pub fn new() -> Self {
        Self {
            timeout_ms: 30_000,
            options: HashMap::new(),
        }
    }

    pub fn set_timeout(&mut self, ms: u64) {
        self.timeout_ms = ms;
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.into(), value.into());
    }

    /// Run the SMT formula through Z3 and return the result
    pub fn check(&self, formula: &SmtFormula) -> SmtResult {
        // Try Z3 subprocess first
        match self.check_with_z3_process(formula) {
            Ok(result) => result,
            Err(_) => {
                // Fall back to internal Z3 FFI if available
                #[cfg(feature = "z3")]
                {
                    self.check_with_z3_ffi(formula)
                        .unwrap_or(SmtResult::Unknown)
                }
                #[cfg(not(feature = "z3"))]
                {
                    SmtResult::Unknown
                }
            }
        }
    }

    /// Call Z3 as a subprocess via stdin/stdout
    fn check_with_z3_process(&self, formula: &SmtFormula) -> Result<SmtResult, std::io::Error> {
        let smtlib2 = formula.to_smtlib2();

        // Spawn Z3
        let mut child = Command::new("z3")
            .arg("-in")
            .arg("-T:{}".to_string().replace("{}", &self.timeout_ms.to_string()))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Write SMT-LIB2 to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(smtlib2.as_bytes())?;
        }

        // Read result
        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse output
        if stdout.contains("sat") {
            let model = parse_model(&stdout);
            Ok(SmtResult::Sat(model))
        } else if stdout.contains("unsat") {
            Ok(SmtResult::Unsat)
        } else if stdout.contains("timeout") {
            Ok(SmtResult::Timeout)
        } else {
            Ok(SmtResult::Unknown)
        }
    }

    /// Use Z3 through Rust FFI bindings (z3 crate)
    #[cfg(feature = "z3")]
    fn check_with_z3_ffi(&self, formula: &SmtFormula) -> Result<SmtResult, z3::Error> {
        let mut config = z3::Config::new();
        config.set_timeout_msec(self.timeout_ms as u64);

        let ctx = z3::Context::new(&config);
        let solver = z3::Solver::new(&ctx);

        // Parse SMT-LIB2 string into Z3 context
        let smtlib2 = formula.to_smtlib2();

        // Use Z3's SMT-LIB2 parser
        let ast = z3::ast::Ast::from_smtlib2_str(&ctx, &smtlib2)?;

        match solver.check() {
            z3::SatResult::Sat => {
                let model = solver.get_model()?;
                // Convert Z3 model to our SmtValue map
                let mut values = HashMap::new();
                for decl in &formula.declarations {
                    if let Ok(val) = model.eval(&z3::ast::Int::from_u32(&ctx, 0), true) {
                        values.insert(decl.name.clone(), SmtValue::Int(
                            val.as_i64().unwrap_or(0)
                        ));
                    }
                }
                Ok(SmtResult::Sat(values))
            }
            z3::SatResult::Unsat => Ok(SmtResult::Unsat),
            z3::SatResult::Unknown => Ok(SmtResult::Unknown),
        }
    }
}

// ── SMT Result Types ──────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SmtResult {
    /// Formula is satisfiable (constraint holds — with model)
    Sat(HashMap<String, SmtValue>),
    /// Formula is unsatisfiable (constraint violated)
    Unsat,
    /// Solver couldn't determine
    Unknown,
    /// Solver timed out
    Timeout,
}

#[derive(Debug, Clone)]
pub struct SmtModel {
    pub values: HashMap<String, SmtValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SmtValue {
    Int(i64),
    Real(f64),
    Bool(bool),
    String(String),
    Array(Vec<i64>),
    Uninterpreted(String),
}

/// Parse Z3 model output
fn parse_model(stdout: &str) -> HashMap<String, SmtValue> {
    let mut values = HashMap::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("(define-fun ") {
            // Parse: (define-fun name () Sort value)
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                let name = parts[1].to_string();
                let value_str = parts[4].trim_end_matches(')');

                let value = if value_str == "true" {
                    SmtValue::Bool(true)
                } else if value_str == "false" {
                    SmtValue::Bool(false)
                } else if let Ok(n) = value_str.parse::<i64>() {
                    SmtValue::Int(n)
                } else if let Ok(f) = value_str.parse::<f64>() {
                    SmtValue::Real(f)
                } else if value_str.starts_with('"') {
                    SmtValue::String(value_str.trim_matches('"').to_string())
                } else {
                    SmtValue::Uninterpreted(value_str.to_string())
                };

                values.insert(name, value);
            }
        }
    }

    values
}

impl std::fmt::Display for SmtValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmtValue::Int(n) => write!(f, "{}", n),
            SmtValue::Real(x) => write!(f, "{}", x),
            SmtValue::Bool(b) => write!(f, "{}", b),
            SmtValue::String(s) => write!(f, "\"{}\"", s),
            SmtValue::Array(vals) => {
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

impl SmtModel {
    pub fn get(&self, var: &str) -> Option<&SmtValue> {
        self.values.get(var)
    }

    pub fn get_int(&self, var: &str) -> Option<i64> {
        self.values.get(var).and_then(|v| {
            if let SmtValue::Int(n) = v { Some(*n) } else { None }
        })
    }

    pub fn get_bool(&self, var: &str) -> Option<bool> {
        self.values.get(var).and_then(|v| {
            if let SmtValue::Bool(b) = v { Some(*b) } else { None }
        })
    }
}
