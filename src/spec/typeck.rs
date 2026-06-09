// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC TYPE CHECKER                                    │
// │  Validates and enriches the specification AST            │
// └──────────────────────────────────────────────────────────┘

use super::ast::*;
use std::collections::{HashMap, HashSet};

/// Type context tracking variables and their types
#[derive(Debug, Clone, Default)]
struct TypeEnv {
    variables: HashMap<String, TypeRef>,
    generics: HashSet<String>,
    type_defs: HashMap<String, TypeDef>,
    function_sigs: HashMap<String, FunctionSpec>,
}

pub struct TypeCheckResult {
    pub spec: Spec,
    pub warnings: Vec<String>,
}

/// Run type checking on a parsed specification
pub fn check(spec: Spec) -> Result<TypeCheckResult, TypeError> {
    let mut env = TypeEnv::default();
    let mut warnings = Vec::new();

    // Register type definitions first
    for td in &spec.types {
        env.type_defs.insert(td.name.clone(), td.clone());
    }

    // Then register function signatures
    for func in &spec.functions {
        env.function_sigs.insert(func.name.clone(), func.clone());
    }

    // Check each function
    let mut checked_functions = Vec::new();
    for func in spec.functions {
        let checked_func = check_function(&func, &mut env)?;
        checked_functions.push(checked_func);
    }

    // Check global invariants
    for inv in &spec.invariants {
        check_global_invariant(inv, &env)?;
    }

    Ok(TypeCheckResult {
        spec: Spec {
            name: spec.name,
            imports: spec.imports,
            functions: checked_functions,
            types: spec.types,
            invariants: spec.invariants,
        },
        warnings,
    })
}

fn check_function(func: &FunctionSpec, env: &mut TypeEnv) -> Result<FunctionSpec, TypeError> {
    let mut local_env = env.clone();

    // Add generic parameters to scope
    for g in &func.generics {
        local_env.generics.insert(g.name.clone());
    }

    // Add input parameters to scope
    for param in &func.params {
        local_env.variables.insert(param.name.clone(), param.ty.clone());
    }

    // Check preconditions
    for pre in &func.preconditions {
        check_constraint(pre, &local_env)
            .map_err(|e| TypeError::in_fn(&func.name, &format!("precondition: {}", e.message)))?;
    }

    // Check postconditions
    for post in &func.postconditions {
        // In postconditions, 'output' refers to the return value
        local_env.variables.insert("output".into(), func.return_type.clone());
        check_constraint(post, &local_env)
            .map_err(|e| TypeError::in_fn(&func.name, &format!("postcondition: {}", e.message)))?;
    }

    // Check invariants
    for inv in &func.invariants {
        check_constraint(&inv.constraint, &local_env)
            .map_err(|e| TypeError::in_fn(&func.name, &format!("invariant: {}", e.message)))?;
    }

    // Check tests
    for test in &func.tests {
        check_expr(&test.input, &local_env).map_err(|e| {
            let tn = test.name.as_deref().unwrap_or("<unnamed>");
            TypeError::in_fn(&func.name, &format!("test '{}' input: {}", tn, e.message))
        })?;
        check_expr(&test.expected_output, &local_env).map_err(|e| {
            let tn = test.name.as_deref().unwrap_or("<unnamed>");
            TypeError::in_fn(&func.name, &format!("test '{}' output: {}", tn, e.message))
        })?;
    }

    // Check complexity bounds make sense
    if let Some(ref cplx) = func.complexity {
        if let BigO::Custom(ref s) = cplx.bound {
            // Warn that custom complexity can't be verified
        }
    }

    Ok(func.clone())
}

fn check_global_invariant(inv: &GlobalInvariant, env: &TypeEnv) -> Result<(), TypeError> {
    check_constraint(&inv.constraint, env)
        .map_err(|e| TypeError::in_global(&inv.name, &e.message))
}

fn check_constraint(constraint: &Constraint, env: &TypeEnv) -> Result<(), TypeError> {
    match constraint {
        Constraint::True => Ok(()),
        Constraint::Expr(expr) => { check_expr(expr, env)?; Ok(()) }
        Constraint::Forall { vars, body } => {
            let mut local = env.clone();
            for (name, ty) in vars {
                local.variables.insert(name.clone(), ty.clone());
            }
            check_constraint(body, &local)
        }
        Constraint::Exists { vars, body } => {
            let mut local = env.clone();
            for (name, ty) in vars {
                local.variables.insert(name.clone(), ty.clone());
            }
            check_constraint(body, &local)
        }
        Constraint::Implies(lhs, rhs) => {
            check_constraint(lhs, env)?;
            check_constraint(rhs, env)?;
            Ok(())
        }
        Constraint::And(parts) | Constraint::Or(parts) => {
            for p in parts {
                check_constraint(p, env)?;
            }
            Ok(())
        }
        Constraint::Not(inner) => check_constraint(inner, env),
        Constraint::Eq(lhs, rhs) | Constraint::Order(_, lhs, rhs) => {
            check_expr(lhs, env)?;
            check_expr(rhs, env)?;
            Ok(())
        }
        Constraint::Predicate(name, args) => {
            for arg in args {
                check_expr(arg, env)?;
            }
            // Predicate names are not checked — they're assumed to be
            // provided by the synthesis backend
            Ok(())
        }
    }
}

fn check_expr(expr: &Expr, env: &TypeEnv) -> Result<TypeRef, TypeError> {
    match expr {
        Expr::Var(name) => {
            if let Some(ty) = env.variables.get(name) {
                return Ok(ty.clone());
            }
            if let Some(f) = env.function_sigs.get(name) {
                let args: Vec<TypeRef> = f.params.iter().map(|p| p.ty.clone()).collect();
                return Ok(TypeRef::Function(args, Box::new(f.return_type.clone())));
            }
            Err(TypeError {
                message: format!("Undefined variable: '{}'", name),
                function: None,
                span: None,
            })
        }
        Expr::IntLit(_) => Ok(TypeRef::Int),
        Expr::FloatLit(_) => Ok(TypeRef::Float),
        Expr::BoolLit(_) => Ok(TypeRef::Bool),
        Expr::StringLit(_) => Ok(TypeRef::String),
        Expr::Field(base, _field) => {
            // Structural typing — we check base exists, field is deferred to runtime
            check_expr(base, env)
        }
        Expr::Index(base, idx) => {
            let _base_ty = check_expr(base, env)?;
            let _idx_ty = check_expr(idx, env)?;
            // Index returns element type of collection
            Ok(TypeRef::Named("_".into()))
        }
        Expr::Call(name, args) => {
            for arg in args {
                check_expr(arg, env)?;
            }
            // Return type from function sig if known
            env.function_sigs.get(name)
                .map(|f| f.return_type.clone())
                .ok_or_else(|| TypeError {
                    message: format!("Called undefined function: '{}'", name),
                    function: None,
                    span: None,
                })
        }
        Expr::BinOp(_op, lhs, rhs) => {
            let _lty = check_expr(lhs, env)?;
            let _rty = check_expr(rhs, env)?;
            // Coarse typing: result type is the join of both operand types
            Ok(TypeRef::Int) // Simplified
        }
        Expr::UnaryOp(_op, inner) => check_expr(inner, env),
        Expr::Lambda(params, body) => {
            let mut local = env.clone();
            for (name, ty) in params {
                local.variables.insert(name.clone(), ty.clone());
            }
            let ret = check_expr(body, &local)?;
            let args: Vec<TypeRef> = params.iter().map(|(_, t)| t.clone()).collect();
            Ok(TypeRef::Function(args, Box::new(ret)))
        }
        Expr::Length(_) => Ok(TypeRef::Int),
        Expr::Comprehension { binder, collection, body } => {
            let _coll_ty = check_expr(collection, env)?;
            let mut local = env.clone();
            local.variables.insert(binder.clone(), TypeRef::Named("_".into()));
            let body_ty = check_expr(body, &local)?;
            Ok(TypeRef::List(Box::new(body_ty)))
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub function: Option<String>,
    pub span: Option<(usize, usize)>,
}

impl TypeError {
    fn in_fn(fn_name: &str, msg: &str) -> Self {
        Self {
            message: format!("In spec '{}': {}", fn_name, msg),
            function: Some(fn_name.into()),
            span: None,
        }
    }
    fn in_global(inv_name: &str, msg: &str) -> Self {
        Self {
            message: format!("In invariant '{}': {}", inv_name, msg),
            function: None,
            span: None,
        }
    }
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Type error: {}", self.message)
    }
}

impl std::error::Error for TypeError {}
