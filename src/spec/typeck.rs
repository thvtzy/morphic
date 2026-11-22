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
        // F2 (Phase 100, D1a/D2a): the Int→decimal-text
        // conversion. Its argument is additionally
        // VALIDATED at the call site in `check_expr` —
        // see the `int_to_str` arm there.
        "int_to_str" => Some(TypeRef::String),
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
        Expr::Field(base, field) => {
            // Phase 74 (decision D2a): when the base's type
            // resolves to a record declared in this module,
            // the field is resolved against the declaration —
            // its declared type is the result, and an unknown
            // field is an error. A base provably not a record
            // is an error too. Bases the checker cannot
            // resolve keep the standing coarse deferral and
            // yield the base type, as this arm always did.
            let base_ty = check_expr(base, env)?;
            field_result_type(&base_ty, field, env)
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
                // Result constructors (LANGUAGE_REFERENCE
                // sections 4/14): `Result<T, E>` values are
                // written Ok(v) / Err(e). The payload side is
                // typed from the argument; the other side is
                // the unknown marker, at the same coarse
                // precision as None() above.
                "Ok" => {
                    let inner = arg_tys
                        .into_iter()
                        .next()
                        .unwrap_or(TypeRef::Named("_".into()));
                    return Ok(TypeRef::Result(
                        Box::new(inner),
                        Box::new(TypeRef::Named("_".into())),
                    ));
                }
                "Err" => {
                    let inner = arg_tys
                        .into_iter()
                        .next()
                        .unwrap_or(TypeRef::Named("_".into()));
                    return Ok(TypeRef::Result(
                        Box::new(TypeRef::Named("_".into())),
                        Box::new(inner),
                    ));
                }
                _ => {}
            }
            // A spec defined in this module wins over the standard
            // library fallback below.
            if let Some(f) = env.function_sigs.get(name) {
                return Ok(f.return_type.clone());
            }
            // F2 (Phase 100, D1a/D2a/D3a): `int_to_str` is
            // the one stdlib entry whose argument this
            // checker validates. The table's standing
            // coarseness (no argument inspection, inherited
            // by `len` and friends) would let
            // `int_to_str("x")` through to layers that can
            // only decline it as unencodable/unevaluable —
            // but a conversion's argument type is the whole
            // content of the builtin, so a wrong arity or a
            // non-integer argument is a type error HERE.
            // The integer family is accepted (Int and the
            // fixed widths): every layer below carries all
            // of them as the same i64 value, so the checker
            // invents no distinction they would not honor.
            if name == "int_to_str" {
                let is_integer = |t: &TypeRef| {
                    matches!(
                        t,
                        TypeRef::Int
                            | TypeRef::I8
                            | TypeRef::I16
                            | TypeRef::I32
                            | TypeRef::I64
                            | TypeRef::U8
                            | TypeRef::U16
                            | TypeRef::U32
                            | TypeRef::U64
                    )
                };
                return match arg_tys.as_slice() {
                    [t] if is_integer(t) => Ok(TypeRef::String),
                    _ => Err(TypeError {
                        message: format!(
                            "`int_to_str` expects exactly one Int argument, got {} argument(s) of type ({})",
                            arg_tys.len(),
                            arg_tys
                                .iter()
                                .map(|t| t.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        function: None,
                        span: None,
                    }),
                };
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
        Expr::MethodCall { receiver, name, args } => {
            // Phase 74 (decision D1a): resolution against the
            // closed builtin table, keyed by the receiver's
            // type — see method_call_result_type.
            let recv_ty = check_expr(receiver, env)?;
            for arg in args {
                check_expr(arg, env)?;
            }
            method_call_result_type(&recv_ty, name, args.len(), env)
        }
        Expr::RecordLit { type_name, fields } => {
            // Phase 74 (decision D3a): the literal names its
            // type; the field set must match the declared
            // record exactly and every field value must match
            // its declared type — see check_record_literal.
            check_record_literal(type_name, fields, env)?;
            Ok(TypeRef::Named(type_name.clone()))
        }
        Expr::Comprehension { binder, collection, body } => {
            let _coll_ty = check_expr(collection, env)?;
            let mut local = env.clone();
            local.variables.insert(binder.clone(), TypeRef::Named("_".into()));
            let body_ty = check_expr(body, &local)?;
            Ok(TypeRef::List(Box::new(body_ty)))
        }
    }
}

fn terr(message: String) -> TypeError {
    TypeError {
        message,
        function: None,
        span: None,
    }
}

/// Expand type aliases (and newtypes) throughout a type:
/// a `Named` type whose declaration is an alias is replaced
/// by the alias target, recursively, containers included.
/// Record and ADT names are not aliases and stay `Named`;
/// undeclared names stay as written. Alias cycles stop at
/// the repeated name. This is what lets the checker see
/// that `StepResult` is a `Result<...>` and that a field
/// declared `kind: TokenKind` expects an `Int`.
fn expand_aliases(ty: &TypeRef, env: &TypeEnv) -> TypeRef {
    expand_aliases_seen(ty, env, &HashSet::new())
}

fn expand_aliases_seen(ty: &TypeRef, env: &TypeEnv, seen: &HashSet<String>) -> TypeRef {
    match ty {
        TypeRef::Named(name) => {
            if seen.contains(name) {
                return ty.clone();
            }
            match env.type_defs.get(name).map(|d| &d.kind) {
                Some(TypeKind::Alias(inner)) | Some(TypeKind::Newtype(inner)) => {
                    let mut seen = seen.clone();
                    seen.insert(name.clone());
                    expand_aliases_seen(inner, env, &seen)
                }
                _ => ty.clone(),
            }
        }
        TypeRef::List(inner) => {
            TypeRef::List(Box::new(expand_aliases_seen(inner, env, seen)))
        }
        TypeRef::Set(inner) => {
            TypeRef::Set(Box::new(expand_aliases_seen(inner, env, seen)))
        }
        TypeRef::Map(k, v) => TypeRef::Map(
            Box::new(expand_aliases_seen(k, env, seen)),
            Box::new(expand_aliases_seen(v, env, seen)),
        ),
        TypeRef::Option(inner) => {
            TypeRef::Option(Box::new(expand_aliases_seen(inner, env, seen)))
        }
        TypeRef::Result(t, e) => TypeRef::Result(
            Box::new(expand_aliases_seen(t, env, seen)),
            Box::new(expand_aliases_seen(e, env, seen)),
        ),
        TypeRef::Tuple(ts) => TypeRef::Tuple(
            ts.iter()
                .map(|t| expand_aliases_seen(t, env, seen))
                .collect(),
        ),
        TypeRef::Array(inner, n) => {
            TypeRef::Array(Box::new(expand_aliases_seen(inner, env, seen)), *n)
        }
        TypeRef::Function(args, ret) => TypeRef::Function(
            args.iter()
                .map(|t| expand_aliases_seen(t, env, seen))
                .collect(),
            Box::new(expand_aliases_seen(ret, env, seen)),
        ),
        TypeRef::Ref(inner) => {
            TypeRef::Ref(Box::new(expand_aliases_seen(inner, env, seen)))
        }
        TypeRef::Stream(inner) => {
            TypeRef::Stream(Box::new(expand_aliases_seen(inner, env, seen)))
        }
        _ => ty.clone(),
    }
}

/// The closed builtin method table (Phase 74, decision D1a;
/// LANGUAGE_REFERENCE section 11). Resolution is keyed by
/// the receiver's type after alias expansion:
///
///   Result<T, E>:  is_ok() -> Bool, is_err() -> Bool,
///                  unwrap() -> T
///   List<T>:       len() -> Int
///
/// Every builtin is zero-argument. An unknown method name,
/// a method of the wrong family, a method on any other
/// receiver type, and any argument to a builtin are each
/// type errors naming the method and the receiver type.
/// There are no user-defined methods — UFCS is a recorded
/// future extension, deliberately not hooked here. A
/// receiver whose type the checker cannot resolve (the `_`
/// unknown marker, a generic parameter, an undeclared named
/// type) defers, as elsewhere in this checker, and yields
/// the unknown marker.
fn method_call_result_type(
    recv_ty: &TypeRef,
    name: &str,
    arg_count: usize,
    env: &TypeEnv,
) -> Result<TypeRef, TypeError> {
    const KNOWN: [&str; 4] = ["is_ok", "is_err", "unwrap", "len"];
    if !KNOWN.contains(&name) {
        return Err(terr(format!(
            "Unknown method '.{}' on {}: the builtin methods are is_ok(), is_err(), \
             unwrap() on Result<T, E> and len() on List<T>",
            name, recv_ty
        )));
    }
    let expanded = expand_aliases(recv_ty, env);
    match &expanded {
        TypeRef::Result(ok, _) => match name {
            "is_ok" | "is_err" => {
                zero_args(name, recv_ty, arg_count)?;
                Ok(TypeRef::Bool)
            }
            "unwrap" => {
                zero_args(name, recv_ty, arg_count)?;
                Ok((**ok).clone())
            }
            _ => Err(terr(format!(
                "Method '.{}' is not a method of {}: len() is the List<T> method — \
                 Result<T, E> has is_ok(), is_err(), unwrap()",
                name, recv_ty
            ))),
        },
        TypeRef::List(_) => match name {
            "len" => {
                zero_args(name, recv_ty, arg_count)?;
                Ok(TypeRef::Int)
            }
            _ => Err(terr(format!(
                "Method '.{}' is not a method of {}: {}() is a Result<T, E> method — \
                 List<T> has len()",
                name, recv_ty, name
            ))),
        },
        TypeRef::Named(n) if n == "_" || !env.type_defs.contains_key(n) => {
            Ok(TypeRef::Named("_".into()))
        }
        TypeRef::Generic(_) => Ok(TypeRef::Named("_".into())),
        _ => Err(terr(format!(
            "Method '.{}' is not defined on {}: methods exist only on \
             Result<T, E> (is_ok(), is_err(), unwrap()) and List<T> (len())",
            name, recv_ty
        ))),
    }
}

fn zero_args(name: &str, recv_ty: &TypeRef, arg_count: usize) -> Result<(), TypeError> {
    if arg_count == 0 {
        Ok(())
    } else {
        Err(terr(format!(
            "Method '.{}' on {} takes no arguments, but {} given",
            name, recv_ty, arg_count
        )))
    }
}

/// Type of `base.field` (Phase 74, decision D2a). When the
/// base's type resolves to a record declared in this
/// module, the declared field type is the result and an
/// unknown field is an error naming the field and the
/// record. A base provably not a record (a primitive, a
/// container, a name declared as a non-record) is an error.
/// An unresolvable base — the `_` unknown marker, a generic
/// parameter, an undeclared named type — keeps the standing
/// coarse deferral and yields the base type unchanged,
/// exactly as this arm behaved before Phase 74.
fn field_result_type(
    base_ty: &TypeRef,
    field: &str,
    env: &TypeEnv,
) -> Result<TypeRef, TypeError> {
    let expanded = expand_aliases(base_ty, env);
    match &expanded {
        TypeRef::Named(name) => match env.type_defs.get(name) {
            Some(td) => match &td.kind {
                TypeKind::Record { fields } => fields
                    .iter()
                    .find(|(n, _)| n == field)
                    .map(|(_, ty)| ty.clone())
                    .ok_or_else(|| {
                        terr(format!(
                            "Type '{}' has no field '{}' (declared fields: {})",
                            name,
                            field,
                            fields
                                .iter()
                                .map(|(n, _)| n.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                    }),
                _ => Err(terr(format!(
                    "Field access '.{}' on non-record type '{}'",
                    field, base_ty
                ))),
            },
            None => Ok(base_ty.clone()),
        },
        TypeRef::Generic(_) => Ok(base_ty.clone()),
        _ => Err(terr(format!(
            "Field access '.{}' on non-record type '{}'",
            field, base_ty
        ))),
    }
}

/// Validate a record literal against its named type
/// (Phase 74, decision D3a; LANGUAGE_REFERENCE section 11):
/// the name must resolve to a record declared in this
/// module (through alias/newtype names), the field set
/// must match the declaration EXACTLY — a duplicated, an
/// unknown, or a missing field is each an error naming the
/// field — and every field value's type must match the
/// declared field type at this checker's coarse precision.
fn check_record_literal(
    type_name: &str,
    fields: &[(String, Expr)],
    env: &TypeEnv,
) -> Result<(), TypeError> {
    let declared = declared_record_fields(type_name, env)?;
    let mut seen: HashSet<&str> = HashSet::new();
    for (fname, value) in fields {
        if !seen.insert(fname.as_str()) {
            return Err(terr(format!(
                "Duplicate field '{}' in record literal for '{}'",
                fname, type_name
            )));
        }
        let Some((_, declared_ty)) = declared.iter().find(|(n, _)| n == fname) else {
            return Err(terr(format!(
                "Unknown field '{}' in record literal for '{}' (declared fields: {})",
                fname,
                type_name,
                declared
                    .iter()
                    .map(|(n, _)| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        };
        let value_ty = check_expr(value, env)?;
        if !record_field_types_compatible(declared_ty, &value_ty, env) {
            return Err(terr(format!(
                "Field '{}' of record '{}' has type {}, expected {}",
                fname, type_name, value_ty, declared_ty
            )));
        }
    }
    for (dname, _) in &declared {
        if !seen.contains(dname.as_str()) {
            return Err(terr(format!(
                "Missing field '{}' in record literal for '{}'",
                dname, type_name
            )));
        }
    }
    Ok(())
}

/// The declared fields of the record a literal names,
/// following alias/newtype names to the declaration. An
/// unknown name and a name declared as a non-record are
/// distinct errors.
fn declared_record_fields(
    type_name: &str,
    env: &TypeEnv,
) -> Result<Vec<(String, TypeRef)>, TypeError> {
    let mut name = type_name;
    let mut seen: HashSet<&str> = HashSet::new();
    loop {
        let Some(td) = env.type_defs.get(name) else {
            return Err(terr(format!(
                "Record literal names undeclared type '{}'",
                type_name
            )));
        };
        match &td.kind {
            TypeKind::Record { fields } => return Ok(fields.clone()),
            TypeKind::Alias(TypeRef::Named(next)) | TypeKind::Newtype(TypeRef::Named(next)) => {
                if !seen.insert(name) {
                    return Err(terr(format!(
                        "Type '{}' is not a record type",
                        type_name
                    )));
                }
                name = next;
            }
            _ => {
                return Err(terr(format!(
                    "Type '{}' is not a record type",
                    type_name
                )));
            }
        }
    }
}

/// Per-field type agreement at this checker's standing
/// coarse precision: aliases are expanded on both sides,
/// then the types must not provably differ (the
/// `types_provably_differ` discipline, extended to recurse
/// through Result / Map / Tuple, which that helper does
/// not descend into). The `_` unknown marker and generic
/// parameters prove nothing and agree with anything.
fn record_field_types_compatible(declared: &TypeRef, value: &TypeRef, env: &TypeEnv) -> bool {
    let declared = expand_aliases(declared, env);
    let value = expand_aliases(value, env);
    types_match_coarse(&declared, &value)
}

fn types_match_coarse(a: &TypeRef, b: &TypeRef) -> bool {
    if a == b {
        return true;
    }
    if matches!(a, TypeRef::Named(n) if n == "_")
        || matches!(b, TypeRef::Named(n) if n == "_")
    {
        return true;
    }
    if matches!(a, TypeRef::Generic(_)) || matches!(b, TypeRef::Generic(_)) {
        return true;
    }
    match (a, b) {
        (TypeRef::List(x), TypeRef::List(y))
        | (TypeRef::Set(x), TypeRef::Set(y))
        | (TypeRef::Option(x), TypeRef::Option(y))
        | (TypeRef::Ref(x), TypeRef::Ref(y))
        | (TypeRef::Stream(x), TypeRef::Stream(y))
        | (TypeRef::Array(x, _), TypeRef::Array(y, _)) => types_match_coarse(x, y),
        (TypeRef::Map(k1, v1), TypeRef::Map(k2, v2)) => {
            types_match_coarse(k1, k2) && types_match_coarse(v1, v2)
        }
        (TypeRef::Result(t1, e1), TypeRef::Result(t2, e2)) => {
            types_match_coarse(t1, t2) && types_match_coarse(e1, e2)
        }
        (TypeRef::Tuple(xs), TypeRef::Tuple(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys.iter())
                    .all(|(x, y)| types_match_coarse(x, y))
        }
        _ => false,
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

    // ── Method calls, record literals, field access ────────
    //    (Phase 74, decisions D1a / D3a / D2a)
    //
    // Method resolution is a closed builtin table keyed by
    // the receiver's type (aliases expanded): Result<T, E>
    // has is_ok() / is_err() / unwrap(); List<T> has len().
    // Record literals validate against the declared record
    // (exact field set, per-field types). Field access on a
    // module-declared record resolves the declared field
    // type and rejects unknown fields and non-record bases;
    // unresolvable bases keep the standing coarse deferral.

    /// The tokenizer's type block, shared by the tests below.
    const TOKENIZER_TYPES: &str = r#"
        type TokenKind = Int;

        type Token = {
            kind: TokenKind,
            lexeme: String,
            line: Int,
            col: Int
        }

        type TokenizerState = {
            source: String,
            pos: Int,
            line: Int,
            col: Int,
            tokens: List<Token>
        }

        type StepResult = Result<TokenizerState, String>;
    "#;

    fn tokenizer_module(body: &str) -> String {
        format!("Tokenizer {{\n{}\n{}\n}}", TOKENIZER_TYPES, body)
    }

    fn check_src(src: &str) -> Result<TypeCheckResult, TypeError> {
        check(parse(src).unwrap())
    }

    #[test]
    fn method_calls_on_result_typecheck() {
        // The dropped tokenize_one constraints, verbatim: the
        // receiver is the alias StepResult, which must expand
        // to Result<TokenizerState, String> for resolution.
        let src = tokenizer_module(
            r#"
        spec tokenize_one {
            input: state: TokenizerState
            output: StepResult

            constraint: output.is_err() || len(output.unwrap().tokens) == len(state.tokens) + 1
            constraint: output.is_ok() || state.pos >= len(state.source)
        }
        "#,
        );
        let result = check_src(&src);
        assert!(
            result.is_ok(),
            "Result method calls must typecheck, got: {:?}",
            result.err().map(|e| e.message)
        );
    }

    #[test]
    fn method_len_on_list_typechecks() {
        // The dropped tokenize_all constraint (unwrap() then
        // .len() on the List payload), plus len() directly on
        // a List parameter.
        let src = tokenizer_module(
            r#"
        spec tokenize_all {
            input: state: TokenizerState
            output: Result<List<Token>, String>

            constraint: output.is_err() || output.unwrap().len() >= len(state.tokens)
        }

        spec lens {
            input: xs: List<Int>
            output: Int

            constraint: output == xs.len()
        }
        "#,
        );
        let result = check_src(&src);
        assert!(
            result.is_ok(),
            "List len() must typecheck, got: {:?}",
            result.err().map(|e| e.message)
        );
    }

    #[test]
    fn result_constructors_typecheck() {
        // Ok(v) / Err(e) are the Result constructors
        // (LANGUAGE_REFERENCE sections 4/14), the exact
        // analogue of the Option constructors Some / None
        // this checker already accepts. The restored
        // tokenizer tests are written with them.
        let src = r#"
        T {
        spec f {
            input: n: Int
            output: Result<Int, String>

            constraint: n >= 0

            test "ok" { input: 1; expect: Ok(2) }
            test "err" { input: 0; expect: Err("nope") }
        }
        }
        "#;
        let result = check_src(src);
        assert!(
            result.is_ok(),
            "Ok/Err constructors must typecheck, got: {:?}",
            result.err().map(|e| e.message)
        );
    }

    #[test]
    fn method_result_method_on_list_rejected() {
        // Wrong family: is_ok() belongs to Result, not List.
        let src = r#"
        T {
        spec f {
            input: xs: List<Int>
            output: Bool

            constraint: output == xs.is_ok()
        }
        }
        "#;
        let err = check_src(src).err().unwrap();
        assert!(
            err.message.contains("is_ok") && err.message.contains("List<Int>"),
            "error must name the method and the receiver type: {}",
            err.message
        );
    }

    #[test]
    fn method_list_method_on_result_rejected() {
        // Wrong family: len() belongs to List, not Result.
        let src = r#"
        T {
        spec f {
            input: n: Int
            output: Result<Int, String>

            constraint: output.len() >= 0
        }
        }
        "#;
        let err = check_src(src).err().unwrap();
        assert!(
            err.message.contains("len") && err.message.contains("Result<Int, String>"),
            "error must name the method and the receiver type: {}",
            err.message
        );
    }

    #[test]
    fn method_unknown_name_rejected() {
        let src = r#"
        T {
        spec f {
            input: n: Int
            output: Result<Int, String>

            constraint: output.frobnicate() == true
        }
        }
        "#;
        let err = check_src(src).err().unwrap();
        assert!(
            err.message.contains("frobnicate"),
            "error must name the unknown method: {}",
            err.message
        );
    }

    #[test]
    fn method_on_non_method_receiver_rejected() {
        // Int carries no methods at all in the closed table.
        let src = r#"
        T {
        spec f {
            input: n: Int
            output: Bool

            constraint: output == n.is_ok()
        }
        }
        "#;
        let err = check_src(src).err().unwrap();
        assert!(
            err.message.contains("is_ok") && err.message.contains("Int"),
            "error must name the method and the receiver type: {}",
            err.message
        );
    }

    #[test]
    fn method_with_arguments_rejected() {
        // Every builtin in the table is zero-argument; any
        // argument is a type error naming the method.
        let src = r#"
        T {
        spec f {
            input: n: Int
            output: Result<Int, String>

            constraint: output.is_ok(1) == true
        }
        }
        "#;
        let err = check_src(src).err().unwrap();
        assert!(
            err.message.contains("is_ok") && err.message.contains("no arguments"),
            "error must name the method and the arity rule: {}",
            err.message
        );

        let src = r#"
        T {
        spec f {
            input: xs: List<Int>
            output: Int

            constraint: output == xs.len(2)
        }
        }
        "#;
        let err = check_src(src).err().unwrap();
        assert!(
            err.message.contains("len") && err.message.contains("no arguments"),
            "error must name the method and the arity rule: {}",
            err.message
        );
    }

    #[test]
    fn record_literals_validate_against_declaration() {
        // The restored tokenizer test shapes: literals as
        // inputs and expected outputs, nested inside a list
        // literal and inside Ok(...), with an alias-typed
        // field (Token.kind: TokenKind = Int) given an Int.
        let src = tokenizer_module(
            r#"
        spec peek {
            input: state: TokenizerState
            output: Option<Char>

            constraint: state.pos<len(state.source)||output==None()
            constraint: state.pos>=len(state.source)||output==Some(state.source[state.pos])

            test "non-empty" { input: TokenizerState { source: "hi", pos: 0, line: 1, col: 1, tokens: [] } expect: Some('h') }
            test "at-end" { input: TokenizerState { source: "", pos: 0, line: 1, col: 1, tokens: [] } expect: None() }
        }

        spec tokenize_one {
            input: state: TokenizerState
            output: StepResult

            constraint: output.is_err() || len(output.unwrap().tokens) == len(state.tokens) + 1
            constraint: output.is_ok() || state.pos >= len(state.source)

            test "keyword-spec" {
                input: TokenizerState { source: "spec", pos: 0, line: 1, col: 1, tokens: [] };
                expect: Ok(TokenizerState { source: "spec", pos: 4, line: 1, col: 5, tokens: [
                    Token { kind: 0, lexeme: "spec", line: 1, col: 1 }
                ] })
            }
        }
        "#,
        );
        let result = check_src(&src);
        assert!(
            result.is_ok(),
            "record literals must validate, got: {:?}",
            result.err().map(|e| e.message)
        );
    }

    #[test]
    fn record_literal_missing_field_rejected() {
        let src = tokenizer_module(
            r#"
        spec f {
            input: state: TokenizerState
            output: Bool

            constraint: output == true

            test "a" { input: TokenizerState { source: "hi", pos: 0, line: 1, tokens: [] } expect: true }
        }
        "#,
        );
        let err = check_src(&src).err().unwrap();
        assert!(
            err.message.contains("Missing field 'col'"),
            "error must name the missing field: {}",
            err.message
        );
    }

    #[test]
    fn record_literal_unknown_field_rejected() {
        let src = tokenizer_module(
            r#"
        spec f {
            input: state: TokenizerState
            output: Bool

            constraint: output == true

            test "a" { input: Token { kind: 0, lexeme: "x", line: 1, col: 1, bogus: 2 } expect: true }
        }
        "#,
        );
        let err = check_src(&src).err().unwrap();
        assert!(
            err.message.contains("Unknown field 'bogus'"),
            "error must name the unknown field: {}",
            err.message
        );
    }

    #[test]
    fn record_literal_duplicate_field_rejected() {
        let src = tokenizer_module(
            r#"
        spec f {
            input: state: TokenizerState
            output: Bool

            constraint: output == true

            test "a" { input: Token { kind: 0, lexeme: "x", line: 1, line: 2, col: 1 } expect: true }
        }
        "#,
        );
        let err = check_src(&src).err().unwrap();
        assert!(
            err.message.contains("Duplicate field 'line'"),
            "error must name the duplicated field: {}",
            err.message
        );
    }

    #[test]
    fn record_literal_undeclared_type_rejected() {
        let src = tokenizer_module(
            r#"
        spec f {
            input: state: TokenizerState
            output: Bool

            constraint: output == true

            test "a" { input: Nope { a: 1 } expect: true }
        }
        "#,
        );
        let err = check_src(&src).err().unwrap();
        assert!(
            err.message.contains("undeclared type 'Nope'"),
            "error must name the undeclared type: {}",
            err.message
        );
    }

    #[test]
    fn record_literal_non_record_type_rejected() {
        // TokenKind is declared, but as an alias for Int —
        // naming it in a literal is a distinct error from
        // the undeclared case.
        let src = tokenizer_module(
            r#"
        spec f {
            input: state: TokenizerState
            output: Bool

            constraint: output == true

            test "a" { input: TokenKind { value: 1 } expect: true }
        }
        "#,
        );
        let err = check_src(&src).err().unwrap();
        assert!(
            err.message.contains("TokenKind") && err.message.contains("not a record"),
            "error must name the type and the rule: {}",
            err.message
        );
    }

    #[test]
    fn record_literal_field_type_mismatch_rejected() {
        let src = tokenizer_module(
            r#"
        spec f {
            input: state: TokenizerState
            output: Bool

            constraint: output == true

            test "a" { input: TokenizerState { source: "hi", pos: "zero", line: 1, col: 1, tokens: [] } expect: true }
        }
        "#,
        );
        let err = check_src(&src).err().unwrap();
        assert!(
            err.message.contains("'pos'") && err.message.contains("Int"),
            "error must name the field and the expected type: {}",
            err.message
        );
    }

    #[test]
    fn field_access_resolves_declared_field_types() {
        // Direct check_expr pins (D2a): a declared record's
        // field yields its DECLARED type (before Phase 74
        // this arm returned the base's type), an alias-typed
        // field yields the declared Named type as written,
        // and the method table's result types are exact.
        let mut env = TypeEnv::default();
        env.type_defs.insert(
            "TokenKind".into(),
            TypeDef {
                name: "TokenKind".into(),
                generics: vec![],
                kind: TypeKind::Alias(TypeRef::Int),
            },
        );
        env.type_defs.insert(
            "Token".into(),
            TypeDef {
                name: "Token".into(),
                generics: vec![],
                kind: TypeKind::Record {
                    fields: vec![
                        ("kind".into(), TypeRef::Named("TokenKind".into())),
                        ("lexeme".into(), TypeRef::String),
                        ("line".into(), TypeRef::Int),
                        ("col".into(), TypeRef::Int),
                    ],
                },
            },
        );
        env.type_defs.insert(
            "TokenizerState".into(),
            TypeDef {
                name: "TokenizerState".into(),
                generics: vec![],
                kind: TypeKind::Record {
                    fields: vec![
                        ("source".into(), TypeRef::String),
                        ("pos".into(), TypeRef::Int),
                        (
                            "tokens".into(),
                            TypeRef::List(Box::new(TypeRef::Named("Token".into()))),
                        ),
                    ],
                },
            },
        );
        env.type_defs.insert(
            "StepResult".into(),
            TypeDef {
                name: "StepResult".into(),
                generics: vec![],
                kind: TypeKind::Alias(TypeRef::Result(
                    Box::new(TypeRef::Named("TokenizerState".into())),
                    Box::new(TypeRef::String),
                )),
            },
        );
        env.variables
            .insert("state".into(), TypeRef::Named("TokenizerState".into()));
        env.variables
            .insert("output".into(), TypeRef::Named("StepResult".into()));

        let field = |base: Expr, name: &str| Expr::Field(Box::new(base), name.into());
        let var = |n: &str| Expr::Var(n.into());

        assert_eq!(
            check_expr(&field(var("state"), "pos"), &env).unwrap(),
            TypeRef::Int,
            "state.pos must resolve to the declared Int"
        );
        assert_eq!(
            check_expr(&field(var("state"), "source"), &env).unwrap(),
            TypeRef::String
        );
        assert_eq!(
            check_expr(&field(var("state"), "tokens"), &env).unwrap(),
            TypeRef::List(Box::new(TypeRef::Named("Token".into())))
        );

        let unwrap = Expr::MethodCall {
            receiver: Box::new(var("output")),
            name: "unwrap".into(),
            args: vec![],
        };
        assert_eq!(
            check_expr(&unwrap, &env).unwrap(),
            TypeRef::Named("TokenizerState".into()),
            "unwrap() on StepResult must yield the Ok payload type"
        );
        assert_eq!(
            check_expr(&field(unwrap.clone(), "tokens"), &env).unwrap(),
            TypeRef::List(Box::new(TypeRef::Named("Token".into()))),
            "output.unwrap().tokens must chain through the payload's fields"
        );
        let is_err = Expr::MethodCall {
            receiver: Box::new(var("output")),
            name: "is_err".into(),
            args: vec![],
        };
        assert_eq!(check_expr(&is_err, &env).unwrap(), TypeRef::Bool);
        let len = Expr::MethodCall {
            receiver: Box::new(field(var("state"), "tokens")),
            name: "len".into(),
            args: vec![],
        };
        assert_eq!(check_expr(&len, &env).unwrap(), TypeRef::Int);
    }

    #[test]
    fn field_access_unknown_field_rejected() {
        let src = tokenizer_module(
            r#"
        spec f {
            input: state: TokenizerState
            output: Bool

            constraint: output == (state.bogus == 1)
        }
        "#,
        );
        let err = check_src(&src).err().unwrap();
        assert!(
            err.message.contains("'bogus'") && err.message.contains("TokenizerState"),
            "error must name the field and the record type: {}",
            err.message
        );
    }

    #[test]
    fn field_access_on_non_record_rejected() {
        let src = r#"
        T {
        spec f {
            input: n: Int
            output: Bool

            constraint: output == (n.pos == 1)
        }
        }
        "#;
        let err = check_src(src).err().unwrap();
        assert!(
            err.message.contains("non-record") && err.message.contains("Int"),
            "error must name the rule and the base type: {}",
            err.message
        );
    }

    #[test]
    fn field_access_on_undeclared_type_still_defers() {
        // The standing coarse deferral (audit section 2.3):
        // a base whose named type is not declared in this
        // module cannot be resolved, so the arm keeps its
        // pre-Phase-74 behavior instead of guessing.
        let src = r#"
        T {
        spec f {
            input: x: ExternalThing
            output: Bool

            constraint: output == (x.whatever == 1)
        }
        }
        "#;
        let result = check_src(src);
        assert!(
            result.is_ok(),
            "field access on an undeclared type must still defer, got: {:?}",
            result.err().map(|e| e.message)
        );
    }

    // ── Phase 100 (F2): the `int_to_str` builtin ────────

    #[test]
    fn int_to_str_call_typechecks_as_string() {
        // F2 (Phase 100, D1a/D2a/D3a): `int_to_str` is a
        // builtin `Int -> String` in the Appendix A call
        // tradition. A well-formed use types as String in
        // constraints and in test expectations alike.
        let src = r#"
        T {
        spec f {
            input: n: Int
            output: String

            constraint: output == int_to_str(n)

            test "five" { input: 5; expect: int_to_str(5) }
        }
        }
        "#;
        let result = check_src(src);
        assert!(
            result.is_ok(),
            "int_to_str(n) must typecheck as String, got: {:?}",
            result.err().map(|e| e.message)
        );
    }

    #[test]
    fn int_to_str_wrong_argument_type_is_a_type_error() {
        // The conversion's argument IS its type: a String
        // argument is a type error at check time, not a
        // downstream surprise (Phase 100 teeth).
        let src = r#"
        T {
        spec f {
            input: s: String
            output: String

            constraint: output == int_to_str(s)
        }
        }
        "#;
        let err = check_src(src).err().unwrap();
        assert!(
            err.message.contains("int_to_str"),
            "error must name the builtin: {}",
            err.message
        );
    }

    #[test]
    fn int_to_str_wrong_arity_is_a_type_error() {
        // Exactly one argument (Phase 100 teeth): two
        // arguments and zero arguments both fail here.
        let two = r#"
        T {
        spec f {
            input: n: Int
            output: String

            constraint: output == int_to_str(n, n)
        }
        }
        "#;
        assert!(
            check_src(two).is_err(),
            "int_to_str(1, 2) must be a type error"
        );
        let zero = r#"
        T {
        spec f {
            input: n: Int
            output: String

            constraint: output == int_to_str()
        }
        }
        "#;
        assert!(
            check_src(zero).is_err(),
            "int_to_str() must be a type error"
        );
    }
}
