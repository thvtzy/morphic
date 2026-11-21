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

/// Return type of a standard library function, if `name` is one.
///
/// The names and signatures are those of LANGUAGE_REFERENCE
/// Appendix A. Types are coarse, matching the precision of the
/// rest of this checker (cf. Index returning `Named("_")`).
fn stdlib_return_type(name: &str) -> Option<TypeRef> {
    match name {
        "len" | "sum" | "product" => Some(TypeRef::Int),
        "is_sorted" | "is_permutation" | "contains" | "all" | "any" => Some(TypeRef::Bool),
        "range" => Some(TypeRef::List(Box::new(TypeRef::Int))),
        "reverse" | "sort" | "filter" | "map" | "zip" | "take" | "drop" => {
            Some(TypeRef::List(Box::new(TypeRef::Named("_".into()))))
        }
        "fold" | "min" | "max" => Some(TypeRef::Named("_".into())),
        _ => None,
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
        Expr::CharLit(_) => Ok(TypeRef::Char),
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
            let mut arg_tys = Vec::new();
            for arg in args {
                arg_tys.push(check_expr(arg, env)?);
            }
            // Constructors the parser itself generates for literals:
            // "List" for [a, b, c] and "__tuple__" for (a, b, ...).
            // These are not user-called functions.
            match name.as_str() {
                "List" => {
                    return Ok(TypeRef::List(Box::new(TypeRef::Named("_".into()))));
                }
                "__tuple__" => return Ok(TypeRef::Tuple(arg_tys)),
                // Option constructors (LANGUAGE_REFERENCE section 4):
                // `Option<Int>` values are written Some(5) / None().
                "Some" => {
                    let inner = arg_tys
                        .into_iter()
                        .next()
                        .unwrap_or(TypeRef::Named("_".into()));
                    return Ok(TypeRef::Option(Box::new(inner)));
                }
                "None" => {
                    return Ok(TypeRef::Option(Box::new(TypeRef::Named("_".into()))));
                }
                _ => {}
            }
            // A spec defined in this module wins over the standard
            // library fallback below.
            if let Some(f) = env.function_sigs.get(name) {
                return Ok(f.return_type.clone());
            }
            // Standard library functions are assumed to be available
            // (LANGUAGE_REFERENCE Appendix A) — the same assumption
            // check_constraint already makes for predicate names.
            if let Some(ty) = stdlib_return_type(name) {
                return Ok(ty);
            }
            Err(TypeError {
                message: format!("Called undefined function: '{}'", name),
                function: None,
                span: None,
            })
        }
        Expr::BinOp(op, lhs, rhs) => {
            let lty = check_expr(lhs, env)?;
            let rty = check_expr(rhs, env)?;
            // `++` (Concat) is the one operator this checker
            // types precisely, because the layers below are
            // sort-strict about it: the IR interpreter evaluates
            // Concat only on two Strings or two Lists, and the
            // Z3 verifier encodes only those cases. A spec the
            // backends would reject must therefore fail HERE, at
            // check time. Every other operator keeps the coarse
            // rule below, unchanged.
            if *op == BinOp::Concat {
                return concat_result_type(&lty, &rty).ok_or_else(|| TypeError {
                    message: format!(
                        "`++` (concat) requires two String or two List operands, got {:?} and {:?}",
                        lty, rty
                    ),
                    function: None,
                    span: None,
                });
            }
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

/// Result type of `l ++ r` for already-checked operand types,
/// or None if the pair is not a concat. This mirrors, at this
/// checker's coarse precision, the rule the synthesis selector
/// applies to IR bodies (`selector`'s Concat verdict, via its
/// `provably_different`): String ++ String is String;
/// List<A> ++ List<B> is List<A> unless the element types are
/// provably different; anything else is not a concat. An
/// operand whose type is the checker's unknown marker
/// (`Named("_")`, e.g. an index expression) proves nothing
/// either way and defers to the other operand, as elsewhere
/// in this checker.
fn concat_result_type(l: &TypeRef, r: &TypeRef) -> Option<TypeRef> {
    match (l, r) {
        (TypeRef::String, TypeRef::String) => Some(TypeRef::String),
        (TypeRef::List(a), TypeRef::List(b)) => {
            if types_provably_differ(a, b) {
                None
            } else {
                Some(TypeRef::List(a.clone()))
            }
        }
        (TypeRef::Named(n), other) | (other, TypeRef::Named(n)) if n == "_" => {
            Some(other.clone())
        }
        _ => None,
    }
}

/// Coarse provably-different test between two types,
/// mirroring `selector::provably_different` at this checker's
/// precision: equal types, anything involving a generic
/// parameter, and anything involving the `_` unknown marker
/// prove nothing; unary containers recurse; two types with
/// different head constructors (or different concrete
/// primitives) differ.
fn types_provably_differ(a: &TypeRef, b: &TypeRef) -> bool {
    if a == b {
        return false;
    }
    if matches!(a, TypeRef::Generic(_)) || matches!(b, TypeRef::Generic(_)) {
        return false;
    }
    if matches!(a, TypeRef::Named(n) if n == "_")
        || matches!(b, TypeRef::Named(n) if n == "_")
    {
        return false;
    }
    match (a, b) {
        (TypeRef::List(x), TypeRef::List(y))
        | (TypeRef::Set(x), TypeRef::Set(y))
        | (TypeRef::Option(x), TypeRef::Option(y))
        | (TypeRef::Ref(x), TypeRef::Ref(y))
        | (TypeRef::Stream(x), TypeRef::Stream(y)) => types_provably_differ(x, y),
        _ => true,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::parser::parse;

    #[test]
    fn stdlib_calls_typecheck() {
        // LANGUAGE_REFERENCE Appendix A lists standard library
        // functions "assumed to be available in constraints", and
        // predicate names are already trusted unchecked — but calls
        // like len(...) inside expressions were rejected as undefined
        // functions, so no spec using len/range could pass `check`.
        let src = r#"
        T {
        spec f {
            input: list: List<Int>
            output: Bool

            constraint: len(list) >= 0
            constraint: forall i: Int in range(0, len(list)):
                list[i] > 0 implies output == true
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let result = check(spec);
        assert!(
            result.is_ok(),
            "stdlib calls must typecheck, got: {:?}",
            result.err().map(|e| e.message)
        );
    }

    #[test]
    fn option_constructors_typecheck() {
        // Some(v) / None() are the documented Option constructors
        // (LANGUAGE_REFERENCE section 4), used by binary_search in
        // constraints and test expectations. The parser lowers them
        // to Call nodes and the checker rejected them as undefined
        // functions.
        let src = r#"
        T {
        spec f {
            input: list: List<Int>, needle: Int
            output: Option<Int>

            constraint: output == None() implies len(list) == 0

            test "missing" { input: ([1, 2], 5); expect: None() }
            test "found" { input: ([1, 2], 2); expect: Some(1) }
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let result = check(spec);
        assert!(
            result.is_ok(),
            "Option constructors must typecheck, got: {:?}",
            result.err().map(|e| e.message)
        );
    }

    #[test]
    fn list_and_tuple_test_inputs_typecheck() {
        // The parser lowers list literals to Call("List", ...) and
        // tuples to Call("__tuple__", ...). The checker rejected its
        // own parser's output as undefined functions, so any spec
        // with a list or tuple test input failed `check`.
        let src = r#"
        T {
        spec f {
            input: list: List<Int>, threshold: Int
            output: Int

            constraint: output >= 0

            test "a" { input: ([10, 20, 30], 5); expect: 3 }
            test "b" { input: []; expect: 0 }
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let result = check(spec);
        assert!(
            result.is_ok(),
            "list/tuple test inputs must typecheck, got: {:?}",
            result.err().map(|e| e.message)
        );
    }

    #[test]
    fn unknown_function_call_still_rejected() {
        // Only the documented standard library names are assumed;
        // anything else must still resolve to a spec in the module.
        let src = r#"
        T {
        spec f {
            input: kind: Int
            output: Bool

            constraint: output == nosuchfn(kind)
        }
        }
        "#;

        let spec = parse(src).unwrap();
        assert!(check(spec).is_err(), "unknown function was accepted");
    }

    // ── Concat `++` typing (Phase 25, finding F1) ────────────
    //
    // The layers below the checker are sort-strict about
    // Concat: the IR interpreter evaluates it only on two
    // Strings or two Lists, and the Z3 verifier encodes only
    // those cases. So `++` is the one operator this checker
    // types precisely — a spec the backends would reject must
    // fail here, at `morphic check` time.

    #[test]
    fn concat_of_strings_typechecks() {
        // The M3 wrap-one template: a String assembled around a
        // String parameter, compared against a String output.
        let src = r#"
        T {
        spec wrap {
            input: inner: String
            output: String

            constraint: output == "Vec<" ++ inner ++ ">"
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let result = check(spec);
        assert!(
            result.is_ok(),
            "String ++ String must typecheck, got: {:?}",
            result.err().map(|e| e.message)
        );
    }

    #[test]
    fn concat_rejects_int_operands() {
        let src = r#"
        T {
        spec f {
            input: a: Int
            output: Int

            constraint: output == a ++ 2
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let result = check(spec);
        assert!(result.is_err(), "Int ++ Int must be a type error");
        let err = result.err().unwrap();
        assert!(
            err.message.contains("++"),
            "error should name the operator: {}",
            err.message
        );
    }

    #[test]
    fn concat_rejects_mixed_operands() {
        let src = r#"
        T {
        spec f {
            input: name: String
            output: String

            constraint: output == name ++ 1
        }
        }
        "#;

        let spec = parse(src).unwrap();
        assert!(
            check(spec).is_err(),
            "String ++ Int must be a type error"
        );
    }

    #[test]
    fn concat_of_compatible_lists_typechecks() {
        // The interpreter's second Concat case: List ++ List.
        let src = r#"
        T {
        spec f {
            input: xs: List<Int>, ys: List<Int>
            output: List<Int>

            constraint: output == xs ++ ys
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let result = check(spec);
        assert!(
            result.is_ok(),
            "List<Int> ++ List<Int> must typecheck, got: {:?}",
            result.err().map(|e| e.message)
        );
    }

    #[test]
    fn concat_rejects_mismatched_lists() {
        // Element types provably different (Int vs String):
        // the selector's IR verdict calls this untypable, the
        // interpreter would fail on the values, so check must
        // reject it too.
        let src = r#"
        T {
        spec f {
            input: xs: List<Int>, ys: List<String>
            output: List<Int>

            constraint: output == xs ++ ys
        }
        }
        "#;

        let spec = parse(src).unwrap();
        assert!(
            check(spec).is_err(),
            "List<Int> ++ List<String> must be a type error"
        );
    }

    // ── Char literals (Phase 45, finding F4) ────────────────

    #[test]
    fn char_literal_spec_typechecks() {
        // A Char classifier in the tokenizer's shape: the
        // literal types as Char (check_expr's CharLit arm),
        // so equality and range constraints over a Char
        // parameter pass `morphic check`.
        let src = r#"
        T {
        spec is_digit {
            input: c: Char
            output: Bool

            constraint: output == (c >= '0' && c <= '9')

            test "digit" { input: '5'; expect: true }
            test "letter" { input: 'a'; expect: false }
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let result = check(spec);
        assert!(
            result.is_ok(),
            "char literal spec must typecheck, got: {:?}",
            result.err().map(|e| e.message)
        );
    }
}
