// ─────────────────────────────────────────────────────────────
//  M3 GATE HARNESS (test-only) — Phase 42: the byte-identical
//  gate for the self-hosted codegen.
//
//  The ROAD M3 gate criterion: "Generated codegen produces
//  byte-identical Rust output", judged (ROAD slice plan,
//  item 6) "over a corpus of candidates byte-compared against
//  the original emitter". This harness is that judgment.
//
//  MECHANICS — why this is real, not a re-implementation:
//
//  * The winner set is extracted by the real pipeline
//    (parse → typecheck → synthesize_with_module at opt 1 →
//    Z3 verify_all (module-aware) → select_best), exactly as
//    the M3 probe and cross-validation harness run it, and
//    extraction is run TWICE: the gate asserts the two passes
//    produce identical winner bodies (structural keys) and
//    identical provenance classes — the Phase 41 finding
//    (winner set fixed and run-independent: 110 constraint
//    seeds + the 2 by-design pool winners) re-proven inside
//    the gate itself. Phase 56 re-record (2026-11-21): after
//    the Phase 55 spec amendments the module holds 116 specs
//    and the fixed set is 113 constraint seeds + the 3
//    by-design pool winners (rust_ident_empty,
//    rust_target_route, rust_expr_call_form).
//
//  * The synthesized-codegen ARTIFACT is the 116 winners
//    rendered through the ORIGINAL `generate` into one Rust
//    module (each winner file wrapped in its own `mod`, so
//    the per-file `mod tests` cannot collide), compiled with
//    rustc --crate-type lib — the probe's compile check at
//    module scale.
//
//  * The COMPARISON composes the winners into an emitter via
//    the assembly structure of tests/m3_proof.rs
//    (ir_expr_assembled, ir_stmt_assembled, ir_tail_assembled,
//    the ty/pattern/spec-expr/constraint assemblies, and
//    generate_rust_from_spec_assembled), transcribed here over
//    the real tree types (IRNode / IRPattern / TypeRef / spec
//    Expr / spec Constraint). Every spec call in the
//    assemblies resolves to the WINNER body of that spec,
//    executed by the real IR interpreter (synthesis::eval) —
//    never to the hand-written mirror. Everything the
//    assemblies themselves treat as glue stays glue, taken
//    from the original where the assembly hand-copies it:
//    recursion, the ", " joins, identifier sanitization
//    (super::rust_ident), numeric Display conversions,
//    char escaping (super::escape_rust_char), the pad
//    repetition, and the indexed test-name default (F2).
//    Where an assembly inlines a fixed text that a spec pins
//    as that text's own table (the *_fixed_text family,
//    rust_file_fixed_text), the gate consults the winner of
//    that spec — the composed codegen is built from the
//    winners wherever a winner exists for the piece.
//    Two assembly copies are stale against the post-Phase-29
//    original and the gate follows the ORIGINAL (the side
//    being compared): string literals in patterns and in
//    spec-Exprs are escaped there (the spec doc for
//    rust_string_literal_text names the pattern arm as one
//    of its three sites; the spec-Expr arm has no spec site
//    and its escaping is glue, as in the original).
//
//  * THE F5 QUESTION, measured two ways. The string-literal
//    specs pin the WRAP (quote … quote, plus .to_string() for
//    the owned form); the per-character escaping inside is
//    inexpressible in the spec language (finding F5) and is
//    mirror-pinned by design (see the rust_string_literal_text
//    spec doc: its equation "is exact exactly on the identity
//    subdomain … and silent off it"). The gate therefore runs
//    the whole corpus under two compositions that differ ONLY
//    at the two literal-text call sites:
//      mode A (strict): the winners receive the raw contents —
//        the composition of winners + assembly glue alone.
//      mode B (mirror-completed): the winners receive the
//        escaped contents — the mirror layer supplies exactly
//        the one recorded per-character transformation the
//        spec declares its own to be silent on, and the
//        winners still decide every wrap/ownership byte.
//    An A-mode mismatch is F5-attributable iff the same item
//    matches under B; the harness asserts that reconciliation
//    for every A mismatch, so the strict composition's ONLY
//    divergence is the recorded class, mechanically proven —
//    and the gate's verdict is read off mode B, the
//    composition as the project's own records define it
//    (winners + assembly + the mirror-pinned F5 layer).
//
//  * THE CORPUS (exact composition in the report):
//    (i)   every expression/statement/type/pattern/spec-expr/
//          constraint tree the tests/m3_proof.rs ASSEMBLY
//          golden tests exercise, transcribed into the native
//          tree types — each carries the proof file's expected
//          output, and the harness asserts the ORIGINAL side
//          reproduces it, so a transcription slip stops the
//          gate loudly instead of manufacturing a mismatch;
//          (ii)  every M3 winner body (116) and every M2 winner
//          body (14), plus every unique subtree of those
//          bodies, each rendered three ways (expression,
//          statement, tail) by both emitters;
//    (iii) the slice-6 DriverSpec end-to-end cases (full /
//          bare / empty-doc / String-param test placement),
//          converted to real candidates;
//    (iv)  every winner of (ii) as a whole candidate file:
//          composed driver vs original `generate`;
//    (v)   the `generate` dispatch battery: target names
//          routed by the composed dispatch (rust_target_route
//          + rust_target_result_form + the unsupported text)
//          vs the original's Ok/Err + error text. (The "ir"
//          route's debug text is mirror-only by the Phase 40
//          record — route + Ok-ness are compared, not bytes.)
//
//  Run with:
//    cargo test -- --ignored --nocapture m3_gate
// ─────────────────────────────────────────────────────────────

use super::{
    constraint_to_rust, escape_rust_char, escape_rust_str, expr_to_rust, generate,
    ir_pattern_to_rust, ir_to_rust, ir_to_rust_expr, ir_to_rust_tail, rust_ident,
    ty_to_rust, CodegenError,
};
use crate::spec::ast::{
    BinOp as SpecBinOp, Constraint, Expr as SpecExpr, FunctionSpec, OrderOp, TypeRef,
};
use crate::spec::{parser, typeck};
use crate::synthesis::engine::{
    CandidateImplementation, CollectionKind, IRBinOp, IRLiteral, IRNode, IRPattern,
    IRUnaryOp, Provenance,
};
use crate::synthesis::eval::{self, Value};
use crate::synthesis::{engine, selector};
use crate::verify;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::process::Command;

// ── Kind codes (declaration order, as the specs pin them) ──

fn node_kind(node: &IRNode) -> i64 {
    use IRNode::*;
    match node {
        Hole { .. } => 0,
        Block(_) => 1,
        Let { .. } => 2,
        If { .. } => 3,
        While { .. } => 4,
        For { .. } => 5,
        Match { .. } => 6,
        Call { .. } => 7,
        Return(_) => 8,
        Literal(_) => 9,
        Var(_) => 10,
        BinOp(..) => 11,
        UnaryOp(..) => 12,
        Lambda { .. } => 13,
        Closure { .. } => 14,
        Collection { .. } => 15,
        Map { .. } => 16,
        Filter { .. } => 17,
        Fold { .. } => 18,
        Sort { .. } => 19,
        Alloc { .. } => 20,
        Assign { .. } => 21,
        Typed { .. } => 22,
        Index(..) => 23,
        Loop { .. } => 24,
        Break { .. } => 25,
        StructDef { .. } => 26,
        TryPropagate(_) => 27,
    }
}

fn binop_code(op: IRBinOp) -> i64 {
    use IRBinOp::*;
    match op {
        Add => 0,
        Sub => 1,
        Mul => 2,
        Div => 3,
        Mod => 4,
        And => 5,
        Or => 6,
        Xor => 7,
        Eq => 8,
        Neq => 9,
        Lt => 10,
        Le => 11,
        Gt => 12,
        Ge => 13,
        Shl => 14,
        Shr => 15,
        Concat => 16,
        Append => 17,
        Merge => 18,
    }
}

fn unary_code(op: IRUnaryOp) -> i64 {
    use IRUnaryOp::*;
    match op {
        Neg => 0,
        Not => 1,
        Abs => 2,
        Len => 3,
        Head => 4,
        Tail => 5,
        Init => 6,
        Last => 7,
        Reverse => 8,
        Clone => 9,
    }
}

fn collection_code(kind: CollectionKind) -> i64 {
    use CollectionKind::*;
    match kind {
        List => 0,
        Set => 1,
        Map => 2,
        Array => 3,
        Tuple => 4,
    }
}

fn lit_kind(lit: &IRLiteral) -> i64 {
    use IRLiteral::*;
    match lit {
        Int(_) => 0,
        Float(_) => 1,
        Bool(_) => 2,
        String(_) => 3,
        Char(_) => 4,
        Unit => 5,
    }
}

/// TypeRef code (the ExpectedTypeCode base): declaration
/// order in spec::ast::TypeRef.
fn type_code(ty: &TypeRef) -> i64 {
    use TypeRef::*;
    match ty {
        Int => 0,
        Float => 1,
        Bool => 2,
        String => 3,
        Char => 4,
        Unit => 5,
        I8 => 6,
        I16 => 7,
        I32 => 8,
        I64 => 9,
        U8 => 10,
        U16 => 11,
        U32 => 12,
        U64 => 13,
        Generic(_) => 14,
        Named(_) => 15,
        List(_) => 16,
        Set(_) => 17,
        Map(..) => 18,
        Option(_) => 19,
        Result(..) => 20,
        Tuple(_) => 21,
        Array(..) => 22,
        Function(..) => 23,
        Ref(_) => 24,
        Stream(_) => 25,
    }
}

/// ExpectedTypeCode: a TypeRef code for Some, 26 for None
/// (the slice-3 convention).
fn expected_code(expected: Option<&TypeRef>) -> i64 {
    match expected {
        Some(ty) => type_code(ty),
        None => 26,
    }
}

fn spec_binop_code(op: SpecBinOp) -> i64 {
    use SpecBinOp::*;
    match op {
        Add => 0,
        Sub => 1,
        Mul => 2,
        Div => 3,
        Mod => 4,
        And => 5,
        Or => 6,
        Eq => 7,
        Neq => 8,
        Lt => 9,
        Le => 10,
        Gt => 11,
        Ge => 12,
        Concat => 13,
    }
}

fn order_code(op: OrderOp) -> i64 {
    match op {
        OrderOp::LessThan => 0,
        OrderOp::LessThanOrEqual => 1,
        OrderOp::GreaterThan => 2,
        OrderOp::GreaterThanOrEqual => 3,
    }
}

fn constraint_kind(cons: &Constraint) -> i64 {
    use Constraint::*;
    match cons {
        True => 0,
        Expr(_) => 1,
        Forall { .. } => 2,
        Exists { .. } => 3,
        Implies(..) => 4,
        And(_) => 5,
        Or(_) => 6,
        Not(_) => 7,
        Eq(..) => 8,
        Order(..) => 9,
        Predicate(..) => 10,
    }
}

// ── Winner extraction (the real pipeline) ─────────────────

pub(super) fn load_module(file: &str) -> Vec<FunctionSpec> {
    let root = env!("CARGO_MANIFEST_DIR");
    let path = format!("{}/examples/self_host/{}", root, file);
    let source = std::fs::read_to_string(&path).unwrap();
    let parsed = parser::parse(&source).expect("parse must succeed");
    let checked = typeck::check(parsed).expect("typeck must succeed");
    checked.spec.functions
}

fn provenance_name(p: &Provenance) -> String {
    match p {
        Provenance::LLMGenerated { .. } => "LLMGenerated".to_string(),
        Provenance::Mutation { .. } => "Mutation".to_string(),
        Provenance::Crossover { .. } => "Crossover".to_string(),
        Provenance::TestInduced { .. } => "TestInduced".to_string(),
        Provenance::ConstraintInduced { kind } => format!("ConstraintInduced({})", kind),
        Provenance::Template { .. } => "Template".to_string(),
        Provenance::HumanGuided => "HumanGuided".to_string(),
    }
}

/// One spec's extracted winner: the candidate itself (for
/// whole-file rendering), its raw body, its call-resolved
/// body (what the executor runs), and the bookkeeping the
/// gate asserts on.
pub(super) struct WinnerEntry {
    pub(super) candidate: CandidateImplementation,
    resolved: IRNode,
    pub(super) provenance: String,
    seed_kind: Option<String>,
    verified: bool,
    tpr: f64,
    /// Structural key of the raw body (its Debug text): the
    /// reproducibility evidence across extraction passes.
    pub(super) body_key: String,
}

pub(super) struct WinnerSet {
    /// Specs in module order.
    order: Vec<String>,
    entries: HashMap<String, WinnerEntry>,
}

impl WinnerSet {
    /// Number of specs in the set (module order length).
    /// Exposed for the M4 harness's extraction sanity check.
    pub(super) fn spec_count(&self) -> usize {
        self.order.len()
    }

    pub(super) fn get(&self, name: &str) -> Result<&WinnerEntry, String> {
        self.entries
            .get(name)
            .ok_or_else(|| format!("no winner extracted for spec {}", name))
    }
}

/// Run the module-aware pipeline once over every spec in
/// `functions` and extract each spec's winner.
pub(super) fn extract_winners(functions: &[FunctionSpec]) -> WinnerSet {
    let mut order = Vec::new();
    let mut raw: HashMap<String, CandidateImplementation> = HashMap::new();
    for func in functions {
        let name = func.name.clone();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine::synthesize_with_module(func, 1, functions)
        }));
        let candidates = match outcome {
            Ok(Ok(c)) if !c.is_empty() => c,
            Ok(Ok(_)) => panic!("{}: synthesis returned no candidates", name),
            Ok(Err(e)) => panic!("{}: synthesis error: {}", name, e),
            Err(_) => panic!("{}: synthesis panicked", name),
        };
        let verified = verify::verifier::Verifier::with_specs(functions)
            .verify_all(candidates, func)
            .unwrap_or_else(|e| panic!("{}: verify_all error: {}", name, e));
        let winner = selector::select_best(verified, func)
            .unwrap_or_else(|e| panic!("{}: select_best error: {}", name, e));
        order.push(name);
        raw.insert(func.name.clone(), winner);
    }
    // Call resolution over the whole winner index (a winner
    // body may call sibling specs; the executor runs resolved
    // bodies so every such call denotes the CALLEE WINNER).
    let mut resolved_cache: HashMap<String, IRNode> = HashMap::new();
    let mut entries = HashMap::new();
    for name in &order {
        let cand = raw.get(name).unwrap();
        let resolved = resolve_spec_body(name, &raw, &mut resolved_cache, &[])
            .unwrap_or_else(|e| panic!("{}: winner call resolution failed: {}", name, e));
        let seed_kind = match &cand.provenance {
            Provenance::ConstraintInduced { kind } => Some(kind.clone()),
            _ => None,
        };
        entries.insert(
            name.clone(),
            WinnerEntry {
                body_key: format!("{:?}", cand.body),
                resolved,
                provenance: provenance_name(&cand.provenance),
                seed_kind,
                verified: cand.verified,
                tpr: cand.scores.test_pass_ratio,
                candidate: cand.clone(),
            },
        );
    }
    WinnerSet { order, entries }
}

// ── Call resolution over the winner index ─────────────────
//
// A conservative analogue of SpecIndex::inline_calls, but
// resolving calls to WINNER bodies (the resolver in engine.rs
// resolves to seed bodies; the gate composes winners).
// Substitution is refused — never renamed — when a name the
// callee binds (match pattern variable, let name, lambda /
// closure parameter) occurs anywhere in an argument, the
// engine resolver's capture discipline generalized to all
// binder kinds. Depth cap 8 and a visiting set, as there.

const MAX_RESOLVE_DEPTH: usize = 8;

fn resolve_spec_body(
    name: &str,
    index: &HashMap<String, CandidateImplementation>,
    cache: &mut HashMap<String, IRNode>,
    visiting: &[String],
) -> Result<IRNode, String> {
    if let Some(body) = cache.get(name) {
        return Ok(body.clone());
    }
    if visiting.iter().any(|v| v == name) {
        return Err(format!("call cycle through {}", name));
    }
    if visiting.len() >= MAX_RESOLVE_DEPTH {
        return Err(format!("call depth cap at {}", name));
    }
    let cand = index
        .get(name)
        .ok_or_else(|| format!("spec {} not in the winner index", name))?;
    let mut visiting = visiting.to_vec();
    visiting.push(name.to_string());
    let resolved = resolve_node(&cand.body, index, cache, &visiting)?;
    cache.insert(name.to_string(), resolved.clone());
    Ok(resolved)
}

fn resolve_node(
    node: &IRNode,
    index: &HashMap<String, CandidateImplementation>,
    cache: &mut HashMap<String, IRNode>,
    visiting: &[String],
) -> Result<IRNode, String> {
    match node {
        IRNode::Call { function, args } => {
            let mut resolved_args = Vec::with_capacity(args.len());
            for a in args {
                resolved_args.push(resolve_node(a, index, cache, visiting)?);
            }
            if !index.contains_key(function) {
                // Builtins (len, List) and anything else the
                // index does not hold pass through untouched;
                // judging them is the executor's business.
                return Ok(IRNode::Call {
                    function: function.clone(),
                    args: resolved_args,
                });
            }
            let callee = index.get(function).unwrap();
            let callee_body = resolve_spec_body(function, index, cache, visiting)?;
            substitute_params_winner(&callee_body, &callee.spec.params, &resolved_args)
        }
        _ => rebuild_children(node, &mut |child| {
            resolve_node(child, index, cache, visiting)
        }),
    }
}

/// Rebuild `node` with `f` applied to every child node.
fn rebuild_children(
    node: &IRNode,
    f: &mut dyn FnMut(&IRNode) -> Result<IRNode, String>,
) -> Result<IRNode, String> {
    use IRNode::*;
    Ok(match node {
        Hole { .. } | Literal(_) | Var(_) => node.clone(),
        Block(stmts) => Block(
            stmts
                .iter()
                .map(|s| f(s))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Let { name, value, body } => Let {
            name: name.clone(),
            value: Box::new(f(value)?),
            body: Box::new(f(body)?),
        },
        If {
            condition,
            then_branch,
            else_branch,
        } => If {
            condition: Box::new(f(condition)?),
            then_branch: Box::new(f(then_branch)?),
            else_branch: Box::new(f(else_branch)?),
        },
        While {
            condition,
            body,
            invariant,
        } => While {
            condition: Box::new(f(condition)?),
            body: Box::new(f(body)?),
            invariant: invariant.clone(),
        },
        For {
            var,
            start,
            end,
            body,
        } => For {
            var: var.clone(),
            start: Box::new(f(start)?),
            end: Box::new(f(end)?),
            body: Box::new(f(body)?),
        },
        Match { scrutinee, cases } => Match {
            scrutinee: Box::new(f(scrutinee)?),
            cases: cases
                .iter()
                .map(|(p, b)| Ok((p.clone(), f(b)?)))
                .collect::<Result<Vec<_>, String>>()?,
        },
        Call { function, args } => Call {
            function: function.clone(),
            args: args
                .iter()
                .map(|a| f(a))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Return(inner) => Return(Box::new(f(inner)?)),
        BinOp(op, l, r) => BinOp(*op, Box::new(f(l)?), Box::new(f(r)?)),
        UnaryOp(op, inner) => UnaryOp(*op, Box::new(f(inner)?)),
        Lambda { params, body } => Lambda {
            params: params.clone(),
            body: Box::new(f(body)?),
        },
        Closure {
            params,
            captures,
            body,
        } => Closure {
            params: params.clone(),
            captures: captures.clone(),
            body: Box::new(f(body)?),
        },
        Collection { kind, elements } => Collection {
            kind: *kind,
            elements: elements
                .iter()
                .map(|e| f(e))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Map { function, over } => Map {
            function: Box::new(f(function)?),
            over: Box::new(f(over)?),
        },
        Filter { predicate, over } => Filter {
            predicate: Box::new(f(predicate)?),
            over: Box::new(f(over)?),
        },
        Fold {
            operation,
            initial,
            over,
        } => Fold {
            operation: Box::new(f(operation)?),
            initial: Box::new(f(initial)?),
            over: Box::new(f(over)?),
        },
        Sort {
            comparator,
            collection,
        } => Sort {
            comparator: Box::new(f(comparator)?),
            collection: Box::new(f(collection)?),
        },
        Alloc { name, ty, initial } => Alloc {
            name: name.clone(),
            ty: ty.clone(),
            initial: Box::new(f(initial)?),
        },
        Assign { target, value } => Assign {
            target: Box::new(f(target)?),
            value: Box::new(f(value)?),
        },
        Typed { node, ty } => Typed {
            node: Box::new(f(node)?),
            ty: ty.clone(),
        },
        Index(c, i) => Index(Box::new(f(c)?), Box::new(f(i)?)),
        Loop { body } => Loop {
            body: Box::new(f(body)?),
        },
        Break { value } => Break {
            value: match value {
                Some(v) => Some(Box::new(f(v)?)),
                None => None,
            },
        },
        StructDef { name, fields, body } => StructDef {
            name: name.clone(),
            fields: fields.clone(),
            body: Box::new(f(body)?),
        },
        TryPropagate(inner) => TryPropagate(Box::new(f(inner)?)),
    })
}

/// Every name `node` binds anywhere: match pattern variables,
/// let names, lambda/closure parameters.
fn collect_binders(node: &IRNode, out: &mut Vec<String>) {
    match node {
        IRNode::Match { cases, .. } => {
            for (pat, _) in cases {
                collect_pattern_binders(pat, out);
            }
        }
        IRNode::Let { name, .. } => out.push(name.clone()),
        IRNode::Lambda { params, .. } | IRNode::Closure { params, .. } => {
            for (name, _) in params {
                out.push(name.clone());
            }
        }
        _ => {}
    }
    let _ = rebuild_children(node, &mut |child| {
        collect_binders(child, out);
        Ok(child.clone())
    });
}

fn collect_pattern_binders(pat: &IRPattern, out: &mut Vec<String>) {
    match pat {
        IRPattern::Variable(name) => out.push(name.clone()),
        IRPattern::Constructor(_, subs) => {
            for s in subs {
                collect_pattern_binders(s, out);
            }
        }
        IRPattern::Guard(inner, _) => collect_pattern_binders(inner, out),
        _ => {}
    }
}

/// Every variable name occurring in `node` (an over-approxima-
/// tion of the free variables — conservative for a capture
/// screen, exactly as the engine resolver's screen is).
fn collect_var_names(node: &IRNode, out: &mut Vec<String>) {
    if let IRNode::Var(name) = node {
        out.push(name.clone());
    }
    let _ = rebuild_children(node, &mut |child| {
        collect_var_names(child, out);
        Ok(child.clone())
    });
}

/// Substitute argument trees for a callee's parameters in its
/// (already call-resolved) body. Refused when any binder name
/// in the body occurs in any argument (capture), mirroring
/// the engine resolver's discipline.
fn substitute_params_winner(
    body: &IRNode,
    params: &[crate::spec::ast::Param],
    args: &[IRNode],
) -> Result<IRNode, String> {
    if params.len() != args.len() {
        return Err(format!(
            "arity mismatch: {} params, {} args",
            params.len(),
            args.len()
        ));
    }
    let mut binders = Vec::new();
    collect_binders(body, &mut binders);
    if !binders.is_empty() {
        let mut arg_names = Vec::new();
        for a in args {
            collect_var_names(a, &mut arg_names);
        }
        if binders.iter().any(|b| arg_names.contains(b)) {
            return Err(format!(
                "substitution refused: a callee binder ({}) occurs in an argument",
                binders
                    .iter()
                    .find(|b| arg_names.contains(b))
                    .unwrap()
            ));
        }
    }
    let map: HashMap<&str, &IRNode> = params
        .iter()
        .map(|p| p.name.as_str())
        .zip(args.iter())
        .collect();
    substitute_rec_winner(body, &map, &[])
}

fn substitute_rec_winner(
    node: &IRNode,
    map: &HashMap<&str, &IRNode>,
    shadowed: &[String],
) -> Result<IRNode, String> {
    match node {
        IRNode::Var(name) => {
            if shadowed.iter().any(|s| s == name) {
                return Ok(node.clone());
            }
            match map.get(name.as_str()) {
                Some(arg) => Ok((*arg).clone()),
                None => Ok(node.clone()),
            }
        }
        IRNode::Match { scrutinee, cases } => {
            let scrutinee = Box::new(substitute_rec_winner(scrutinee, map, shadowed)?);
            let mut out = Vec::with_capacity(cases.len());
            for (pat, body) in cases {
                let mut pat_binders = Vec::new();
                collect_pattern_binders(pat, &mut pat_binders);
                let body = if pat_binders.is_empty() {
                    substitute_rec_winner(body, map, shadowed)?
                } else {
                    let mut shadowed = shadowed.to_vec();
                    shadowed.extend(pat_binders);
                    substitute_rec_winner(body, map, &shadowed)?
                };
                out.push((pat.clone(), body));
            }
            Ok(IRNode::Match {
                scrutinee,
                cases: out,
            })
        }
        IRNode::Let { name, value, body } => {
            let value = Box::new(substitute_rec_winner(value, map, shadowed)?);
            let mut shadowed = shadowed.to_vec();
            shadowed.push(name.clone());
            let body = Box::new(substitute_rec_winner(body, map, &shadowed)?);
            Ok(IRNode::Let {
                name: name.clone(),
                value,
                body,
            })
        }
        IRNode::Lambda { params, body } => {
            let mut shadowed = shadowed.to_vec();
            shadowed.extend(params.iter().map(|(n, _)| n.clone()));
            Ok(IRNode::Lambda {
                params: params.clone(),
                body: Box::new(substitute_rec_winner(body, map, &shadowed)?),
            })
        }
        IRNode::Closure {
            params,
            captures,
            body,
        } => {
            let mut shadowed = shadowed.to_vec();
            shadowed.extend(params.iter().map(|(n, _)| n.clone()));
            Ok(IRNode::Closure {
                params: params.clone(),
                captures: captures.clone(),
                body: Box::new(substitute_rec_winner(body, map, &shadowed)?),
            })
        }
        _ => rebuild_children(node, &mut |child| {
            substitute_rec_winner(child, map, shadowed)
        }),
    }
}

/// Census of the Call nodes in a body: (total calls, the
/// distinct callee names). The gate reports this for every
/// winner — the resolution above is expected to be a no-op
/// on the measured winner set (Phase 41 compiled every
/// winner standalone, which no body calling a sibling spec
/// could survive), and the census proves it.
fn call_census(node: &IRNode) -> (usize, Vec<String>) {
    let mut names: Vec<String> = Vec::new();
    let mut count = 0usize;
    fn walk(node: &IRNode, count: &mut usize, names: &mut Vec<String>) {
        if let IRNode::Call { function, .. } = node {
            *count += 1;
            if !names.contains(function) {
                names.push(function.clone());
            }
        }
        let _ = rebuild_children(node, &mut |child| {
            walk(child, count, names);
            Ok(child.clone())
        });
    }
    walk(node, &mut count, &mut names);
    names.sort();
    (count, names)
}

// ── The executor: spec calls resolve to winner bodies ─────

fn v_int(n: i64) -> Value {
    Value::Int(n)
}
fn v_bool(b: bool) -> Value {
    Value::Bool(b)
}
fn v_text(s: &str) -> Value {
    Value::Str(s.to_string())
}

impl WinnerSet {
    /// Execute spec `name`'s winner body on `args` with the
    /// real IR interpreter. The body's own calls were resolved
    /// against the winner index at extraction time, so this is
    /// the composed codegen's one and only execution path.
    fn call(&self, name: &str, args: &[Value]) -> Result<Value, String> {
        let entry = self.get(name)?;
        let spec = &entry.candidate.spec;
        if spec.params.len() != args.len() {
            return Err(format!(
                "{}: arity mismatch ({} params, {} args)",
                name,
                spec.params.len(),
                args.len()
            ));
        }
        let mut env: eval::Env = HashMap::new();
        for (param, arg) in spec.params.iter().zip(args.iter()) {
            env.insert(param.name.clone(), arg.clone());
        }
        eval::eval(&entry.resolved, &env)
            .map_err(|e| format!("{}: winner execution failed: {:?}", name, e))
    }

    fn call_int(&self, name: &str, args: &[Value]) -> Result<i64, String> {
        match self.call(name, args)? {
            Value::Int(n) => Ok(n),
            other => Err(format!("{}: expected Int result, got {:?}", name, other)),
        }
    }

    fn call_bool(&self, name: &str, args: &[Value]) -> Result<bool, String> {
        match self.call(name, args)? {
            Value::Bool(b) => Ok(b),
            other => Err(format!("{}: expected Bool result, got {:?}", name, other)),
        }
    }

    fn call_text(&self, name: &str, args: &[Value]) -> Result<String, String> {
        match self.call(name, args)? {
            Value::Str(s) => Ok(s),
            other => Err(format!("{}: expected String result, got {:?}", name, other)),
        }
    }
}

// ── The composed emitter ──────────────────────────────────
//
// The tests/m3_proof.rs assemblies, transcribed over the real
// tree types, with every spec call executed against the
// winner set. `escape_literals` is the ONLY difference
// between mode A (false) and mode B (true): whether the
// contents handed to the two string-literal winners are the
// raw contents (A) or the mirror-pinned escaped contents
// (B) — see the module header.

pub(super) struct Composed<'a> {
    pub(super) w: &'a WinnerSet,
    pub(super) escape_literals: bool,
}

impl<'a> Composed<'a> {
    /// The contents a string literal's winner sees.
    fn literal_contents(&self, contents: &str) -> String {
        if self.escape_literals {
            escape_rust_str(contents)
        } else {
            contents.to_string()
        }
    }

    fn text1(&self, name: &str, a: Value) -> Result<String, String> {
        self.w.call_text(name, &[a])
    }

    // ── Types (ty_to_rust_assembled) ──

    fn ty_text(&self, ty: &TypeRef) -> Result<String, String> {
        let tag = type_code(ty);
        Ok(match ty {
            TypeRef::Generic(name) | TypeRef::Named(name) => rust_ident(name),
            TypeRef::List(inner)
            | TypeRef::Set(inner)
            | TypeRef::Option(inner)
            | TypeRef::Ref(inner)
            | TypeRef::Stream(inner) => format!(
                "{}{}{}",
                self.text1("rust_type_prefix", v_int(tag))?,
                self.ty_text(inner)?,
                self.text1("rust_type_suffix", v_int(tag))?,
            ),
            TypeRef::Map(first, second) | TypeRef::Result(first, second) => format!(
                "{}{}{}{}{}",
                self.text1("rust_type_prefix", v_int(tag))?,
                self.ty_text(first)?,
                self.text1("rust_type_separator", v_int(tag))?,
                self.ty_text(second)?,
                self.text1("rust_type_suffix", v_int(tag))?,
            ),
            TypeRef::Tuple(parts) => {
                let rendered: Result<Vec<String>, String> =
                    parts.iter().map(|p| self.ty_text(p)).collect();
                format!(
                    "{}{}{}",
                    self.text1("rust_type_prefix", v_int(21))?,
                    rendered?.join(", "),
                    self.text1("rust_type_suffix", v_int(21))?,
                )
            }
            TypeRef::Array(inner, size) => format!(
                "{}{}{}{}{}",
                self.text1("rust_type_prefix", v_int(22))?,
                self.ty_text(inner)?,
                self.text1("rust_type_separator", v_int(22))?,
                size,
                self.text1("rust_type_suffix", v_int(22))?,
            ),
            TypeRef::Function(args, ret) => {
                let rendered: Result<Vec<String>, String> =
                    args.iter().map(|a| self.ty_text(a)).collect();
                format!(
                    "{}{}{}{}{}",
                    self.text1("rust_type_prefix", v_int(23))?,
                    rendered?.join(", "),
                    self.text1("rust_type_midfix", v_int(23))?,
                    self.ty_text(ret)?,
                    self.text1("rust_type_suffix", v_int(23))?,
                )
            }
            // Scalar leaves: the slice-1 name table.
            _ => self.text1("rust_type_name", v_int(tag))?,
        })
    }

    // ── Patterns (ir_pattern_assembled) ──

    fn pattern_text(&self, pat: &IRPattern) -> Result<String, String> {
        Ok(match pat {
            IRPattern::Wildcard => self.text1("rust_pattern_fixed_text", v_int(0))?,
            IRPattern::Variable(name) => rust_ident(name),
            IRPattern::Literal(lit) => self.pattern_literal_text(lit)?,
            IRPattern::Constructor(name, subs) => {
                if subs.is_empty() {
                    // The branch rust_pattern_ctor_name pins:
                    // the winner receives the rendered name.
                    self.w.call_text(
                        "rust_pattern_ctor_name",
                        &[v_text(&rust_ident(name)), v_bool(false)],
                    )?
                } else {
                    let rendered: Result<Vec<String>, String> =
                        subs.iter().map(|p| self.pattern_text(p)).collect();
                    format!("{}({})", rust_ident(name), rendered?.join(", "))
                }
            }
            IRPattern::Guard(inner, guard) => {
                format!("{} if {}", self.pattern_text(inner)?, guard)
            }
        })
    }

    fn pattern_literal_text(&self, lit: &IRLiteral) -> Result<String, String> {
        Ok(match lit {
            IRLiteral::Int(n) => n.to_string(),
            IRLiteral::Bool(b) => b.to_string(),
            // The Literal arm of ir_pattern_to_rust is one of
            // rust_string_literal_text's three sites (its spec
            // doc): the composed codegen renders it through
            // the literal winner, as in expression position.
            IRLiteral::String(s) => self
                .text1("rust_string_literal_text", v_text(&self.literal_contents(s)))?,
            // Float / Char / Unit widen to the wildcard text
            // the rust_pattern_literal_text table pins.
            other => self.text1("rust_pattern_literal_text", v_int(lit_kind(other)))?,
        })
    }

    // ── Spec expressions (expr_to_rust_assembled) ──

    fn sexpr_text(&self, expr: &SpecExpr) -> Result<String, String> {
        Ok(match expr {
            SpecExpr::Var(name) => rust_ident(name),
            SpecExpr::IntLit(n) => n.to_string(),
            SpecExpr::BoolLit(b) => b.to_string(),
            // No spec site pins expr_to_rust's StringLit arm;
            // the original escapes (a Phase 29 site) and the
            // assembly's job is to compose as the original
            // composes — the escaping here is glue, in BOTH
            // modes (see the module header).
            SpecExpr::StringLit(s) => format!("\"{}\"", escape_rust_str(s)),
            // The original's CharLit arm (Phase 45): the
            // escaped character inside single quotes — the
            // same escape_rust_char the IR Char arm uses.
            SpecExpr::CharLit(c) => format!("'{}'", escape_rust_char(*c)),
            SpecExpr::Call(name, args) => {
                let rendered: Result<Vec<String>, String> =
                    args.iter().map(|a| self.sexpr_text(a)).collect();
                let joined = rendered?.join(", ");
                // The pseudo-call dispatch (Phase 55): the
                // classification is the rust_expr_call_form
                // winner's; the vec/tuple forms render through
                // their template winners; the bare constructor
                // (form 3) is the name itself, as in the
                // assembly; every other call keeps the plain
                // call template.
                let form = self
                    .w
                    .call_int("rust_expr_call_form", &[v_text(name)])?;
                match form {
                    1 => self.text1("rust_expr_list_text", v_text(&joined))?,
                    2 => self.text1("rust_expr_tuple_text", v_text(&joined))?,
                    3 => "None".to_string(),
                    _ => format!("{}({})", rust_ident(name), joined),
                }
            }
            SpecExpr::BinOp(op, lhs, rhs) => format!(
                "({} {} {})",
                self.sexpr_text(lhs)?,
                self.text1("spec_binop_symbol", v_int(spec_binop_code(*op)))?,
                self.sexpr_text(rhs)?,
            ),
            // The closure template (Phase 55): parameter
            // names only, the body rendered by this same
            // walker, composed through the
            // rust_expr_lambda_text winner.
            SpecExpr::Lambda(params, body) => {
                let names: Vec<String> =
                    params.iter().map(|(n, _)| rust_ident(n)).collect();
                self.w.call_text(
                    "rust_expr_lambda_text",
                    &[
                        v_text(&names.join(", ")),
                        v_text(&self.sexpr_text(body)?),
                    ],
                )?
            }
            // Every kind without an arm: the fixed text the
            // rust_expr_fixed_text table pins ("??"). (Kind
            // 10, Lambda, left this table in Phase 55 — it
            // has its own arm above.)
            other => {
                let kind = match other {
                    SpecExpr::FloatLit(_) => 2,
                    SpecExpr::Field(..) => 5,
                    SpecExpr::Index(..) => 6,
                    SpecExpr::UnaryOp(..) => 9,
                    SpecExpr::Length(_) => 11,
                    SpecExpr::Comprehension { .. } => 12,
                    _ => unreachable!(),
                };
                self.text1("rust_expr_fixed_text", v_int(kind))?
            }
        })
    }

    // ── Constraints (constraint_to_rust_assembled) ──

    fn constraint_text(&self, cons: &Constraint) -> Result<String, String> {
        Ok(match cons {
            Constraint::Not(inner) => format!("!({})", self.constraint_text(inner)?),
            Constraint::Eq(a, b) => {
                format!("{} == {}", self.sexpr_text(a)?, self.sexpr_text(b)?)
            }
            Constraint::Order(op, a, b) => {
                let sep = self.text1("rust_order_separator", v_int(order_code(*op)))?;
                if sep.is_empty() {
                    // The assembly's None branch (m3_proof.rs
                    // constraint_to_rust_assembled): orders
                    // with no separator render the constant
                    // "true" — assembly glue, exactly as the
                    // assembly writes it. (rust_order_form
                    // classifies this fallback as form 2 but
                    // returns a code, not text; the
                    // rust_constraint_fixed_text table is
                    // keyed by constraint KIND and has no
                    // Order row.)
                    "true".to_string()
                } else {
                    format!("{}{}{}", self.sexpr_text(a)?, sep, self.sexpr_text(b)?)
                }
            }
            Constraint::And(parts) => {
                let rendered: Result<Vec<String>, String> =
                    parts.iter().map(|p| self.constraint_text(p)).collect();
                format!("({})", rendered?.join(" && "))
            }
            // True and every catch-all kind: the fixed text
            // the rust_constraint_fixed_text table pins.
            _ => self.text1("rust_constraint_fixed_text", v_int(constraint_kind(cons)))?,
        })
    }

    // ── Expressions (ir_expr_assembled) ──

    fn literal_text(&self, lit: &IRLiteral) -> Result<String, String> {
        Ok(match lit {
            IRLiteral::Int(n) => n.to_string(),
            IRLiteral::Float(f) => f.to_string(),
            IRLiteral::Bool(b) => self.text1("rust_literal_bool", v_bool(*b))?,
            IRLiteral::String(s) => self
                .text1("rust_string_literal_text", v_text(&self.literal_contents(s)))?,
            IRLiteral::Char(c) => format!("'{}'", escape_rust_char(*c)),
            IRLiteral::Unit => self.text1("rust_literal_fixed_text", v_int(5))?,
        })
    }

    /// Assembly of concat_owned_operand: a direct String
    /// literal takes the owned literal template; a direct Var
    /// takes the clone text (glue, as in the assembly — the
    /// Phase 39 treatment); every other node renders as usual.
    fn concat_owned_text(&self, node: &IRNode) -> Result<String, String> {
        Ok(match node {
            IRNode::Literal(IRLiteral::String(s)) => self.text1(
                "rust_string_literal_owned_text",
                v_text(&self.literal_contents(s)),
            )?,
            IRNode::Var(name) => format!("{}.clone()", rust_ident(name)),
            _ => self.expr_text(node)?,
        })
    }

    /// Assembly of concat_borrowed_operand, composed through
    /// the rust_concat_right_form / rust_concat_right_text
    /// winners exactly as the emitter decides then renders.
    fn concat_borrowed_text(&self, node: &IRNode) -> Result<String, String> {
        let rendered = self.expr_text(node)?;
        let (kind, lit) = match node {
            IRNode::Literal(l) => (9, lit_kind(l)),
            _ => (node_kind(node), 0),
        };
        let form = self
            .w
            .call_int("rust_concat_right_form", &[v_int(kind), v_int(lit)])?;
        self.w
            .call_text("rust_concat_right_text", &[v_int(form), v_text(&rendered)])
    }

    /// Assembly of match_scrutinee_to_rust, composed through
    /// the slice-4 scrutinee winners.
    fn scrutinee_text(
        &self,
        scrutinee: &IRNode,
        cases: &[(IRPattern, IRNode)],
    ) -> Result<String, String> {
        let base = self.w.call_text(
            "rust_condition_text",
            &[
                v_int(node_kind(scrutinee)),
                v_text(&self.expr_text(scrutinee)?),
            ],
        )?;
        let has_string = cases
            .iter()
            .any(|(p, _)| pattern_has_string_literal(p));
        let form = self.w.call_int(
            "rust_match_scrutinee_form",
            &[v_bool(has_string), v_bool(is_bare_string_literal(scrutinee))],
        )?;
        self.w
            .call_text("rust_match_scrutinee_text", &[v_int(form), v_text(&base)])
    }

    fn condition_text(&self, node: &IRNode) -> Result<String, String> {
        self.w.call_text(
            "rust_condition_text",
            &[v_int(node_kind(node)), v_text(&self.expr_text(node)?)],
        )
    }

    fn expr_text(&self, node: &IRNode) -> Result<String, String> {
        Ok(match node {
            IRNode::Literal(lit) => self.literal_text(lit)?,
            IRNode::Var(name) => rust_ident(name),
            IRNode::BinOp(op, l, r) => {
                let form = self
                    .w
                    .call_int("rust_expr_binop_form", &[v_int(binop_code(*op))])?;
                match form {
                    1 => self.w.call_text(
                        "rust_binop_concat_text",
                        &[
                            v_text(&self.concat_owned_text(l)?),
                            v_text(&self.concat_borrowed_text(r)?),
                        ],
                    )?,
                    2 => self.w.call_text(
                        "rust_binop_append_text",
                        &[v_text(&self.expr_text(l)?), v_text(&self.expr_text(r)?)],
                    )?,
                    3 => self.w.call_text(
                        "rust_binop_merge_text",
                        &[v_text(&self.expr_text(l)?), v_text(&self.expr_text(r)?)],
                    )?,
                    _ => self.w.call_text(
                        "rust_binop_infix_text",
                        &[
                            v_int(binop_code(*op)),
                            v_text(&self.expr_text(l)?),
                            v_text(&self.expr_text(r)?),
                        ],
                    )?,
                }
            }
            IRNode::UnaryOp(op, inner) => self.w.call_text(
                "rust_unary_text",
                &[v_int(unary_code(*op)), v_text(&self.expr_text(inner)?)],
            )?,
            IRNode::Call { function, args } => {
                let rendered: Result<Vec<String>, String> =
                    args.iter().map(|a| self.expr_text(a)).collect();
                self.w.call_text(
                    "rust_call_text",
                    &[v_text(&rust_ident(function)), v_text(&rendered?.join(", "))],
                )?
            }
            IRNode::If {
                condition,
                then_branch,
                else_branch,
            } => self.w.call_text(
                "rust_if_text",
                &[
                    v_text(&self.condition_text(condition)?),
                    v_text(&self.expr_text(then_branch)?),
                    v_text(&self.expr_text(else_branch)?),
                ],
            )?,
            IRNode::Lambda { params, body } | IRNode::Closure { params, body, .. } => {
                // One arm in the original (Phase 30): Closure
                // captures are not rendered, parameter types
                // are dropped.
                let names: Vec<String> =
                    params.iter().map(|(n, _)| rust_ident(n)).collect();
                self.w.call_text(
                    "rust_lambda_text",
                    &[
                        v_text(&names.join(", ")),
                        v_text(&self.expr_text(body)?),
                    ],
                )?
            }
            IRNode::Collection { kind, elements } => {
                let rendered: Result<Vec<String>, String> =
                    elements.iter().map(|e| self.expr_text(e)).collect();
                format!(
                    "{}{}{}",
                    self.text1("rust_collection_prefix", v_int(collection_code(*kind)))?,
                    rendered?.join(", "),
                    self.text1("rust_collection_suffix", v_int(collection_code(*kind)))?,
                )
            }
            IRNode::Map { function, over } => self.w.call_text(
                "rust_map_text",
                &[
                    v_text(&self.expr_text(over)?),
                    v_text(&self.expr_text(function)?),
                ],
            )?,
            IRNode::Filter { predicate, over } => self.w.call_text(
                "rust_filter_text",
                &[
                    v_text(&self.expr_text(over)?),
                    v_text(&self.expr_text(predicate)?),
                ],
            )?,
            IRNode::Fold {
                operation,
                initial,
                over,
            } => self.w.call_text(
                "rust_fold_text",
                &[
                    v_text(&self.expr_text(over)?),
                    v_text(&self.expr_text(initial)?),
                    v_text(&self.expr_text(operation)?),
                ],
            )?,
            IRNode::Sort {
                comparator,
                collection,
            } => self.w.call_text(
                "rust_sort_text",
                &[
                    v_text(&self.expr_text(collection)?),
                    v_text(&self.expr_text(comparator)?),
                ],
            )?,
            IRNode::Match { scrutinee, cases } => {
                let arms: Result<Vec<String>, String> = cases
                    .iter()
                    .map(|(p, b)| {
                        self.w.call_text(
                            "rust_match_arm_text",
                            &[
                                v_text(&self.pattern_text(p)?),
                                v_text(&self.expr_text(b)?),
                            ],
                        )
                    })
                    .collect();
                self.w.call_text(
                    "rust_match_text",
                    &[
                        v_text(&self.scrutinee_text(scrutinee, cases)?),
                        v_text(&arms?.join(", ")),
                    ],
                )?
            }
            IRNode::Block(stmts) => {
                // The original's Block arm: statements render
                // as statements in a neutral context, the last
                // keeps the tail position; the whole is the
                // rust_block_text template over the inner text.
                let inner = self.block_inner_text(stmts)?;
                self.text1("rust_block_text", v_text(inner.trim_end()))?
            }
            IRNode::Let { .. } | IRNode::While { .. } | IRNode::For { .. } => {
                // expr_block_stmt: the statement rendering
                // inside the rust_block_text template.
                let inner = self.stmt_text(node, 0, &TypeRef::Unit)?;
                self.text1("rust_block_text", v_text(inner.trim_end()))?
            }
            IRNode::Loop { body } => {
                let inner = self.stmt_text(body, 0, &TypeRef::Unit)?;
                self.text1("rust_loop_text", v_text(inner.trim_end()))?
            }
            IRNode::StructDef { name, fields, body } => {
                let rendered: Result<Vec<String>, String> = fields
                    .iter()
                    .map(|(n, t)| {
                        self.w.call_text(
                            "rust_param_text",
                            &[v_text(&rust_ident(n)), v_text(&self.ty_text(t)?)],
                        )
                    })
                    .collect();
                self.w.call_text(
                    "rust_struct_def_text",
                    &[
                        v_text(&rust_ident(name)),
                        v_text(&rendered?.join(", ")),
                        v_text(&self.expr_text(body)?),
                    ],
                )?
            }
            IRNode::Index(coll, idx) => self.w.call_text(
                "rust_index_text",
                &[
                    v_text(&self.expr_text(coll)?),
                    v_text(&self.expr_text(idx)?),
                ],
            )?,
            IRNode::TryPropagate(inner) => {
                self.text1("rust_try_text", v_text(&self.expr_text(inner)?))?
            }
            IRNode::Assign { target, value } => self.w.call_text(
                "rust_assign_text",
                &[
                    v_text(&self.expr_text(target)?),
                    v_text(&self.expr_text(value)?),
                ],
            )?,
            IRNode::Return(inner) => {
                self.text1("rust_return_text", v_text(&self.expr_text(inner)?))?
            }
            IRNode::Break { value } => match value {
                Some(inner) => self.w.call_text(
                    "rust_break_text",
                    &[v_bool(true), v_text(&self.expr_text(inner)?)],
                )?,
                None => self
                    .w
                    .call_text("rust_break_text", &[v_bool(false), v_text("")])?,
            },
            IRNode::Hole { .. } => self.text1("rust_hole_text", v_int(0))?,
            IRNode::Alloc { .. } => {
                self.text1("rust_unsupported_text", v_text("Alloc"))?
            }
            IRNode::Typed { node, ty } => self.expr_typed_text(node, Some(ty))?,
        })
    }

    /// The Block arm's inner text: rest as statements, last
    /// as a tail — in the neutral (Unit) context the original
    /// uses in expression position.
    fn block_inner_text(&self, stmts: &[IRNode]) -> Result<String, String> {
        let mut inner = String::new();
        if let Some((last, rest)) = stmts.split_last() {
            for stmt in rest {
                inner.push_str(&self.stmt_text(stmt, 0, &TypeRef::Unit)?);
            }
            inner.push_str(&self.tail_text(last, 0, &TypeRef::Unit, None)?);
        }
        Ok(inner)
    }

    /// Assembly of ir_to_rust_expr_typed: the only
    /// expectation-driven rewrite is the String-literal
    /// ownership one (rust_string_ownership decides, the
    /// owned template renders); a Typed node re-enters with
    /// its own type; every other node delegates.
    fn expr_typed_text(
        &self,
        node: &IRNode,
        expected: Option<&TypeRef>,
    ) -> Result<String, String> {
        Ok(match node {
            IRNode::Typed { node, ty } => self.expr_typed_text(node, Some(ty))?,
            IRNode::Literal(IRLiteral::String(s)) => {
                let owned = self.w.call_bool(
                    "rust_string_ownership",
                    &[v_int(3), v_int(expected_code(expected))],
                )?;
                if owned {
                    self.text1(
                        "rust_string_literal_owned_text",
                        v_text(&self.literal_contents(s)),
                    )?
                } else {
                    self.expr_text(node)?
                }
            }
            _ => self.expr_text(node)?,
        })
    }

    // ── Statements (ir_stmt_assembled) ──

    fn stmt_text(
        &self,
        node: &IRNode,
        indent: usize,
        fn_ret: &TypeRef,
    ) -> Result<String, String> {
        let pad = "    ".repeat(indent);
        let pad_inner = "    ".repeat(indent + 1);
        Ok(match node {
            IRNode::Hole { .. } => self.text1("rust_stmt_hole_text", v_text(&pad))?,
            IRNode::Block(stmts) => {
                let parts: Result<Vec<String>, String> = stmts
                    .iter()
                    .map(|s| self.stmt_text(s, indent, fn_ret))
                    .collect();
                parts?.concat()
            }
            IRNode::Let { name, value, body } => self.w.call_text(
                "rust_stmt_let_text",
                &[
                    v_text(&pad),
                    v_text(&rust_ident(name)),
                    v_text(&self.expr_text(value)?),
                    v_text(&self.tail_text(body, indent, fn_ret, Some(fn_ret))?),
                ],
            )?,
            IRNode::If {
                condition,
                then_branch,
                else_branch,
            } => self.w.call_text(
                "rust_stmt_if_text",
                &[
                    v_text(&pad),
                    v_text(&self.condition_text(condition)?),
                    v_text(&self.tail_text(then_branch, indent + 1, fn_ret, Some(fn_ret))?),
                    v_text(&self.tail_text(else_branch, indent + 1, fn_ret, Some(fn_ret))?),
                ],
            )?,
            IRNode::While {
                condition,
                body,
                invariant,
            } => self.w.call_text(
                "rust_stmt_while_text",
                &[
                    v_bool(invariant.is_some()),
                    v_text(invariant.as_deref().unwrap_or("")),
                    v_text(&pad),
                    v_text(&self.condition_text(condition)?),
                    v_text(&self.stmt_text(body, indent + 1, fn_ret)?),
                ],
            )?,
            IRNode::For {
                var,
                start,
                end,
                body,
            } => self.w.call_text(
                "rust_stmt_for_text",
                &[
                    v_text(&pad),
                    v_text(&rust_ident(var)),
                    v_text(&self.expr_text(start)?),
                    v_text(&self.expr_text(end)?),
                    v_text(&self.stmt_text(body, indent + 1, fn_ret)?),
                ],
            )?,
            IRNode::Match { scrutinee, cases } => {
                let mut out = self.w.call_text(
                    "rust_stmt_match_head_text",
                    &[
                        v_text(&pad),
                        v_text(&self.stmt_scrutinee_text(scrutinee, cases)?),
                    ],
                )?;
                for (pat, body) in cases {
                    out.push_str(&self.w.call_text(
                        "rust_stmt_match_arm_text",
                        &[
                            v_text(&pad_inner),
                            v_text(&self.pattern_text(pat)?),
                            v_text(&self.tail_text(
                                body,
                                indent + 2,
                                fn_ret,
                                Some(fn_ret),
                            )?),
                        ],
                    )?);
                }
                out.push_str(
                    &self.text1("rust_stmt_match_close_text", v_text(&pad))?,
                );
                out
            }
            IRNode::Call { function, args } => {
                let rendered: Result<Vec<String>, String> =
                    args.iter().map(|a| self.expr_text(a)).collect();
                self.w.call_text(
                    "rust_stmt_call_text",
                    &[
                        v_text(&pad),
                        v_text(&rust_ident(function)),
                        v_text(&rendered?.join(", ")),
                    ],
                )?
            }
            IRNode::Return(inner) => self.w.call_text(
                "rust_stmt_return_text",
                &[
                    v_text(&pad),
                    v_text(&self.expr_typed_text(inner, Some(fn_ret))?),
                ],
            )?,
            IRNode::Alloc { name, ty, initial } => self.w.call_text(
                "rust_stmt_alloc_text",
                &[
                    v_text(&pad),
                    v_text(&rust_ident(name)),
                    v_text(&self.ty_text(ty)?),
                    v_text(&self.expr_typed_text(initial, Some(ty))?),
                ],
            )?,
            IRNode::Assign { target, value } => self.w.call_text(
                "rust_stmt_assign_text",
                &[
                    v_text(&pad),
                    v_text(&self.expr_text(target)?),
                    v_text(&self.expr_text(value)?),
                ],
            )?,
            IRNode::Collection { kind, elements } => {
                let rendered: Result<Vec<String>, String> =
                    elements.iter().map(|e| self.expr_text(e)).collect();
                self.w.call_text(
                    "rust_stmt_collection_text",
                    &[
                        v_int(collection_code(*kind)),
                        v_text(&pad),
                        v_text(&rendered?.join(", ")),
                    ],
                )?
            }
            IRNode::Map { function, over } => self.w.call_text(
                "rust_stmt_map_text",
                &[
                    v_text(&pad),
                    v_text(&self.expr_text(over)?),
                    v_text(&self.expr_text(function)?),
                ],
            )?,
            IRNode::Filter { predicate, over } => self.w.call_text(
                "rust_stmt_filter_text",
                &[
                    v_text(&pad),
                    v_text(&self.expr_text(over)?),
                    v_text(&self.expr_text(predicate)?),
                ],
            )?,
            IRNode::Fold {
                operation,
                initial,
                over,
            } => self.w.call_text(
                "rust_stmt_fold_text",
                &[
                    v_text(&pad),
                    v_text(&self.expr_text(over)?),
                    v_text(&self.expr_text(initial)?),
                    v_text(&self.expr_text(operation)?),
                ],
            )?,
            IRNode::Sort {
                comparator,
                collection,
            } => self.w.call_text(
                "rust_stmt_sort_text",
                &[
                    v_text(&pad),
                    v_text(&self.expr_text(collection)?),
                    v_text(&self.expr_text(comparator)?),
                ],
            )?,
            IRNode::Typed { node, .. } => self.stmt_text(node, indent, fn_ret)?,
            // Expressions that shouldn't appear as statements:
            // the terminated fallback.
            _ => self.w.call_text(
                "rust_stmt_expr_text",
                &[v_text(&pad), v_text(&self.expr_text(node)?)],
            )?,
        })
    }

    /// The statement Match's scrutinee (the expression
    /// assembly's scrutinee composition over statement cases).
    fn stmt_scrutinee_text(
        &self,
        scrutinee: &IRNode,
        cases: &[(IRPattern, IRNode)],
    ) -> Result<String, String> {
        self.scrutinee_text(scrutinee, cases)
    }

    // ── Tails (ir_tail_assembled) ──

    fn tail_text(
        &self,
        node: &IRNode,
        indent: usize,
        fn_ret: &TypeRef,
        expected: Option<&TypeRef>,
    ) -> Result<String, String> {
        let form = self
            .w
            .call_int("rust_tail_form", &[v_int(node_kind(node))])?;
        Ok(match form {
            2 => {
                let IRNode::Block(stmts) = node else {
                    return Err("tail form 2 on a non-Block node".to_string());
                };
                match stmts.split_last() {
                    None => String::new(),
                    Some((last, rest)) => {
                        let rest_text: Result<Vec<String>, String> = rest
                            .iter()
                            .map(|s| self.stmt_text(s, indent, fn_ret))
                            .collect();
                        self.w.call_text(
                            "rust_tail_block_text",
                            &[
                                v_text(&rest_text?.concat()),
                                v_text(&self.tail_text(last, indent, fn_ret, expected)?),
                            ],
                        )?
                    }
                }
            }
            3 => {
                let IRNode::Typed { node, ty } = node else {
                    return Err("tail form 3 on a non-Typed node".to_string());
                };
                self.tail_text(node, indent, fn_ret, Some(ty))?
            }
            1 => self.w.call_text(
                "rust_tail_expr_text",
                &[
                    v_text(&"    ".repeat(indent)),
                    v_text(&self.expr_typed_text(node, expected)?),
                ],
            )?,
            _ => self.stmt_text(node, indent, fn_ret)?,
        })
    }

    // ── The driver (generate_rust_from_spec_assembled) ──

    pub(super) fn driver_file(&self, candidate: &CandidateImplementation) -> Result<String, String> {
        let spec = &candidate.spec;

        // Module header: presence is the doc's Some-ness.
        let header = match &spec.doc {
            Some(doc) => {
                let lines: Result<Vec<String>, String> = doc
                    .lines()
                    .map(|line| self.text1("rust_doc_line", v_text(line)))
                    .collect();
                let header_text =
                    self.text1("rust_doc_header_text", v_text(&lines?.concat()))?;
                self.w.call_text(
                    "rust_driver_section_text",
                    &[v_bool(true), v_text(&header_text)],
                )?
            }
            None => self
                .w
                .call_text("rust_driver_section_text", &[v_bool(false), v_text("")])?,
        };

        // Signature parts.
        let generic_names: Vec<String> =
            spec.generics.iter().map(|g| rust_ident(&g.name)).collect();
        let generics = self.w.call_text(
            "rust_generics_text",
            &[
                v_text(&generic_names.join(", ")),
                v_bool(!spec.generics.is_empty()),
            ],
        )?;
        let params: Result<Vec<String>, String> = spec
            .params
            .iter()
            .map(|p| {
                self.w.call_text(
                    "rust_param_text",
                    &[v_text(&rust_ident(&p.name)), v_text(&self.ty_text(&p.ty)?)],
                )
            })
            .collect();
        let signature = self.w.call_text(
            "rust_signature",
            &[
                v_text(&rust_ident(&spec.name)),
                v_text(&generics),
                v_text(&params?.join(", ")),
                v_text(&self.ty_text(&spec.return_type)?),
            ],
        )?;

        // Precondition block: presence is non-emptiness,
        // placed AFTER the signature (at the head of the
        // function body) by rust_file_text — the Phase 55
        // placement; through Phase 54 it stood before the
        // signature as a free-standing block.
        let pre_block = if spec.preconditions.is_empty() {
            self.w
                .call_text("rust_driver_section_text", &[v_bool(false), v_text("")])?
        } else {
            let asserts: Result<Vec<String>, String> = spec
                .preconditions
                .iter()
                .map(|c| {
                    self.text1(
                        "rust_precondition_assert",
                        v_text(&self.constraint_text(c)?),
                    )
                })
                .collect();
            let block =
                self.text1("rust_precondition_block", v_text(&asserts?.concat()))?;
            self.w.call_text(
                "rust_driver_section_text",
                &[v_bool(true), v_text(&block)],
            )?
        };

        // Body: the tail assembly at indent 1, the return
        // type's code as both fn_ret and the Some expectation
        // (the driver's own seed).
        let body = self.tail_text(
            &candidate.body,
            1,
            &spec.return_type,
            Some(&spec.return_type),
        )?;

        // Test module: presence is non-emptiness.
        let test_module = if spec.tests.is_empty() {
            self.w
                .call_text("rust_driver_section_text", &[v_bool(false), v_text("")])?
        } else {
            let cases: Result<Vec<String>, String> = spec
                .tests
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let raw_name = t
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("test_{}", i));
                    // The tuple spread (Phase 55;
                    // rust_test_case's doc states the rule):
                    // a tuple input whose arity equals the
                    // parameter count spreads its elements
                    // into the call; every other input
                    // renders as a single expression.
                    let input_text = match &t.input {
                        SpecExpr::Call(name, args)
                            if name == "__tuple__"
                                && args.len() == spec.params.len() =>
                        {
                            let rendered: Result<Vec<String>, String> =
                                args.iter().map(|a| self.sexpr_text(a)).collect();
                            rendered?.join(", ")
                        }
                        other => self.sexpr_text(other)?,
                    };
                    self.w.call_text(
                        "rust_test_case",
                        &[
                            v_text(&rust_ident(&raw_name)),
                            v_text(&rust_ident(&spec.name)),
                            v_text(&input_text),
                            v_text(&self.sexpr_text(&t.expected_output)?),
                        ],
                    )
                })
                .collect();
            let module =
                self.text1("rust_test_module", v_text(&cases?.concat()))?;
            self.w.call_text(
                "rust_driver_section_text",
                &[v_bool(true), v_text(&module)],
            )?
        };

        self.w.call_text(
            "rust_file_text",
            &[
                v_text(&header),
                v_text(&pre_block),
                v_text(&signature),
                v_text(&body),
                v_text(&test_module),
            ],
        )
    }

    // ── Dispatch (generate) ──

    /// The composed dispatch's prediction for a target name:
    /// Ok for routes 0..=5, Err with the composed error text
    /// for route 6.
    fn dispatch(&self, target: &str) -> Result<Result<(), String>, String> {
        let route = self
            .w
            .call_int("rust_target_route", &[v_text(target)])?;
        let form = self
            .w
            .call_int("rust_target_result_form", &[v_int(route)])?;
        Ok(match form {
            0 => Ok(()),
            _ => Err(self.text1("rust_unsupported_target_text", v_text(target))?),
        })
    }
}

/// The original's pattern_has_string_literal (glue in the
/// scrutinee composition, as in the assemblies).
fn pattern_has_string_literal(pat: &IRPattern) -> bool {
    match pat {
        IRPattern::Literal(IRLiteral::String(_)) => true,
        IRPattern::Constructor(_, subs) => subs.iter().any(pattern_has_string_literal),
        IRPattern::Guard(inner, _) => pattern_has_string_literal(inner),
        _ => false,
    }
}

/// The original's is_bare_string_literal (through Typed).
fn is_bare_string_literal(node: &IRNode) -> bool {
    match node {
        IRNode::Literal(IRLiteral::String(_)) => true,
        IRNode::Typed { node, .. } => is_bare_string_literal(node),
        _ => false,
    }
}

// ── Corpus ────────────────────────────────────────────────

pub(super) enum CaseKind {
    Ty(TypeRef),
    Pat(IRPattern),
    SExpr(SpecExpr),
    Cons(Constraint),
    Expr(IRNode),
    Stmt(IRNode, usize, TypeRef),
    Tail(IRNode, usize, TypeRef, Option<TypeRef>),
    File(Box<CandidateImplementation>),
    Dispatch(String),
}

pub(super) struct Case {
    pub(super) class: &'static str,
    pub(super) label: String,
    /// The tests/m3_proof.rs golden expectation for the
    /// ORIGINAL side, where the proof file states one: the
    /// harness asserts the original reproduces it, so the
    /// transcription itself is verified.
    pub(super) expected: Option<&'static str>,
    pub(super) original: String,
    pub(super) kind: CaseKind,
}

pub(super) fn render(composed: &Composed, case: &Case) -> Result<String, String> {
    Ok(match &case.kind {
        CaseKind::Ty(ty) => composed.ty_text(ty)?,
        CaseKind::Pat(p) => composed.pattern_text(p)?,
        CaseKind::SExpr(e) => composed.sexpr_text(e)?,
        CaseKind::Cons(c) => composed.constraint_text(c)?,
        CaseKind::Expr(n) => composed.expr_text(n)?,
        CaseKind::Stmt(n, indent, ret) => composed.stmt_text(n, *indent, ret)?,
        CaseKind::Tail(n, indent, ret, expected) => {
            composed.tail_text(n, *indent, ret, expected.as_ref())?
        }
        CaseKind::File(c) => composed.driver_file(c)?,
        CaseKind::Dispatch(t) => match composed.dispatch(t)? {
            Ok(()) => "Ok".to_string(),
            Err(text) => format!("Err:{}", text),
        },
    })
}

// Tree-building shorthands for the transcriptions below.
pub(super) fn li(n: i64) -> IRNode {
    IRNode::Literal(IRLiteral::Int(n))
}
pub(super) fn ls(s: &str) -> IRNode {
    IRNode::Literal(IRLiteral::String(s.to_string()))
}
pub(super) fn lb(b: bool) -> IRNode {
    IRNode::Literal(IRLiteral::Bool(b))
}
pub(super) fn vr(n: &str) -> IRNode {
    IRNode::Var(n.to_string())
}
pub(super) fn bin(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
    IRNode::BinOp(op, Box::new(l), Box::new(r))
}
pub(super) fn call(f: &str, args: Vec<IRNode>) -> IRNode {
    IRNode::Call {
        function: f.to_string(),
        args,
    }
}
fn lam(params: &[&str], body: IRNode) -> IRNode {
    IRNode::Lambda {
        params: params
            .iter()
            .map(|n| (n.to_string(), TypeRef::Unit))
            .collect(),
        body: Box::new(body),
    }
}

/// Corpus (i): the tests/m3_proof.rs assembly golden trees,
/// transcribed into the native tree types. Every case
/// carries the proof file's expected output; the gate
/// asserts the original side reproduces it before judging
/// the composed side.
pub(super) fn proof_corpus() -> Vec<Case> {
    let mut cases: Vec<Case> = Vec::new();
    let mut push = |class: &'static str,
                    label: &str,
                    expected: &'static str,
                    original: String,
                    kind: CaseKind| {
        cases.push(Case {
            class,
            label: label.to_string(),
            expected: Some(expected),
            original,
            kind,
        });
    };

    // ── Types (proof: type_assembly_nested) ──
    let tys: Vec<(&str, TypeRef, &str)> = vec![
        ("list-int", TypeRef::List(Box::new(TypeRef::Int)), "Vec<i64>"),
        ("set-int", TypeRef::Set(Box::new(TypeRef::Int)), "HashSet<i64>"),
        ("ref-string", TypeRef::Ref(Box::new(TypeRef::String)), "&String"),
        (
            "stream-int",
            TypeRef::Stream(Box::new(TypeRef::Int)),
            "impl Iterator<Item = i64>",
        ),
        (
            "list-list-int",
            TypeRef::List(Box::new(TypeRef::List(Box::new(TypeRef::Int)))),
            "Vec<Vec<i64>>",
        ),
        (
            "option-result",
            TypeRef::Option(Box::new(TypeRef::Result(
                Box::new(TypeRef::Int),
                Box::new(TypeRef::String),
            ))),
            "Option<Result<i64, String>>",
        ),
        (
            "map-string-list-bool",
            TypeRef::Map(
                Box::new(TypeRef::String),
                Box::new(TypeRef::List(Box::new(TypeRef::Bool))),
            ),
            "HashMap<String, Vec<bool>>",
        ),
        (
            "tuple-int-string",
            TypeRef::Tuple(vec![TypeRef::Int, TypeRef::String]),
            "(i64, String)",
        ),
        ("tuple-empty", TypeRef::Tuple(vec![]), "()"),
        ("tuple-one", TypeRef::Tuple(vec![TypeRef::Int]), "(i64)"),
        (
            "array-int-3",
            TypeRef::Array(Box::new(TypeRef::Int), 3),
            "[i64; 3]",
        ),
        (
            "array-u8-256",
            TypeRef::Array(Box::new(TypeRef::U8), 256),
            "[u8; 256]",
        ),
        (
            "func-int-bool-unit",
            TypeRef::Function(vec![TypeRef::Int, TypeRef::Bool], Box::new(TypeRef::Unit)),
            "fn(i64, bool) -> ()",
        ),
        (
            "func-none-option-string",
            TypeRef::Function(
                vec![],
                Box::new(TypeRef::Option(Box::new(TypeRef::String))),
            ),
            "fn() -> Option<String>",
        ),
        ("named", TypeRef::Named("MyStruct".to_string()), "MyStruct"),
        (
            "list-named-t",
            TypeRef::List(Box::new(TypeRef::Named("T".to_string()))),
            "Vec<T>",
        ),
    ];
    for (label, ty, expected) in tys {
        let original = ty_to_rust(&ty);
        push("proof-ty", label, expected, original, CaseKind::Ty(ty));
    }

    // ── Patterns (proof: pattern_assembly) ──
    let pats: Vec<(&str, IRPattern, &str)> = vec![
        ("wildcard", IRPattern::Wildcard, "_"),
        ("variable", IRPattern::Variable("x".to_string()), "x"),
        ("lit-int", IRPattern::Literal(IRLiteral::Int(3)), "3"),
        ("lit-int-neg", IRPattern::Literal(IRLiteral::Int(-1)), "-1"),
        (
            "lit-bool",
            IRPattern::Literal(IRLiteral::Bool(false)),
            "false",
        ),
        (
            "lit-str",
            IRPattern::Literal(IRLiteral::String("hi".to_string())),
            "\"hi\"",
        ),
        ("lit-float", IRPattern::Literal(IRLiteral::Float(1.5)), "_"),
        ("lit-char", IRPattern::Literal(IRLiteral::Char('a')), "_"),
        ("lit-unit", IRPattern::Literal(IRLiteral::Unit), "_"),
        (
            "ctor-none",
            IRPattern::Constructor("None".to_string(), vec![]),
            "None",
        ),
        (
            "ctor-some-var",
            IRPattern::Constructor(
                "Some".to_string(),
                vec![IRPattern::Variable("x".to_string())],
            ),
            "Some(x)",
        ),
        (
            "ctor-pair",
            IRPattern::Constructor(
                "Pair".to_string(),
                vec![
                    IRPattern::Literal(IRLiteral::Int(1)),
                    IRPattern::Wildcard,
                ],
            ),
            "Pair(1, _)",
        ),
        (
            "ctor-nested",
            IRPattern::Constructor(
                "Some".to_string(),
                vec![IRPattern::Constructor(
                    "Ok".to_string(),
                    vec![IRPattern::Variable("v".to_string())],
                )],
            ),
            "Some(Ok(v))",
        ),
        (
            "guard",
            IRPattern::Guard(
                Box::new(IRPattern::Variable("x".to_string())),
                "x > 0".to_string(),
            ),
            "x if x > 0",
        ),
    ];
    for (label, pat, expected) in pats {
        let original = ir_pattern_to_rust(&pat);
        push("proof-pattern", label, expected, original, CaseKind::Pat(pat));
    }

    // ── Spec expressions (proof: expr_assembly) ──
    let sexprs: Vec<(&str, SpecExpr, &str)> = vec![
        ("var", SpecExpr::Var("x".to_string()), "x"),
        ("int-neg", SpecExpr::IntLit(-7), "-7"),
        ("bool", SpecExpr::BoolLit(true), "true"),
        (
            "string",
            SpecExpr::StringLit("s".to_string()),
            "\"s\"",
        ),
        ("float-fallback", SpecExpr::FloatLit(1.5), "??"),
        (
            "unary-fallback",
            SpecExpr::UnaryOp(crate::spec::ast::UnaryOp::Neg, Box::new(SpecExpr::Var("x".to_string()))),
            "??",
        ),
        (
            "field-fallback",
            SpecExpr::Field(Box::new(SpecExpr::Var("x".to_string())), "f".to_string()),
            "??",
        ),
        (
            "comprehension-fallback",
            SpecExpr::Comprehension {
                binder: "x".to_string(),
                collection: Box::new(SpecExpr::Var("xs".to_string())),
                body: Box::new(SpecExpr::Var("x".to_string())),
            },
            "??",
        ),
        (
            "call",
            SpecExpr::Call(
                "f".to_string(),
                vec![SpecExpr::IntLit(1), SpecExpr::Var("x".to_string())],
            ),
            "f(1, x)",
        ),
        ("call-empty", SpecExpr::Call("g".to_string(), vec![]), "g()"),
        (
            "binop-add",
            SpecExpr::BinOp(
                SpecBinOp::Add,
                Box::new(SpecExpr::IntLit(1)),
                Box::new(SpecExpr::IntLit(2)),
            ),
            "(1 + 2)",
        ),
        (
            "binop-mod-symbol",
            SpecExpr::BinOp(
                SpecBinOp::Mod,
                Box::new(SpecExpr::IntLit(1)),
                Box::new(SpecExpr::IntLit(2)),
            ),
            "(1 % 2)",
        ),
        (
            "binop-concat-symbol",
            SpecExpr::BinOp(
                SpecBinOp::Concat,
                Box::new(SpecExpr::StringLit("a".to_string())),
                Box::new(SpecExpr::StringLit("b".to_string())),
            ),
            "(\"a\" + \"b\")",
        ),
        (
            "call-list",
            SpecExpr::Call(
                "List".to_string(),
                vec![SpecExpr::IntLit(1), SpecExpr::IntLit(2)],
            ),
            "vec![1, 2]",
        ),
        ("call-list-empty", SpecExpr::Call("List".to_string(), vec![]), "vec![]"),
        (
            "call-tuple",
            SpecExpr::Call(
                "__tuple__".to_string(),
                vec![SpecExpr::IntLit(1), SpecExpr::Var("x".to_string())],
            ),
            "(1, x)",
        ),
        ("call-none", SpecExpr::Call("None".to_string(), vec![]), "None"),
        (
            "call-some",
            SpecExpr::Call("Some".to_string(), vec![SpecExpr::IntLit(1)]),
            "Some(1)",
        ),
        (
            "lambda-lt",
            SpecExpr::Lambda(
                vec![
                    ("a".to_string(), TypeRef::Int),
                    ("b".to_string(), TypeRef::Int),
                ],
                Box::new(SpecExpr::BinOp(
                    SpecBinOp::Lt,
                    Box::new(SpecExpr::Var("a".to_string())),
                    Box::new(SpecExpr::Var("b".to_string())),
                )),
            ),
            "|a, b| (a < b)",
        ),
        (
            "binop-nested",
            SpecExpr::BinOp(
                SpecBinOp::And,
                Box::new(SpecExpr::BinOp(
                    SpecBinOp::Eq,
                    Box::new(SpecExpr::Var("a".to_string())),
                    Box::new(SpecExpr::IntLit(1)),
                )),
                Box::new(SpecExpr::BoolLit(true)),
            ),
            "((a == 1) && true)",
        ),
    ];
    for (label, expr, expected) in sexprs {
        let original = expr_to_rust(&expr);
        push("proof-sexpr", label, expected, original, CaseKind::SExpr(expr));
    }

    // ── Constraints (proof: constraint_assembly) ──
    let vx = || SpecExpr::Var("x".to_string());
    let cons: Vec<(&str, Constraint, &str)> = vec![
        ("true", Constraint::True, "true"),
        ("or-catchall", Constraint::Or(vec![Constraint::True]), "true"),
        (
            "predicate-catchall",
            Constraint::Predicate("p".to_string(), vec![]),
            "true",
        ),
        (
            "forall-catchall",
            Constraint::Forall {
                vars: vec![],
                body: Box::new(Constraint::True),
            },
            "true",
        ),
        (
            "not",
            Constraint::Not(Box::new(Constraint::True)),
            "!(true)",
        ),
        (
            "not-not-eq",
            Constraint::Not(Box::new(Constraint::Not(Box::new(Constraint::Eq(
                vx(),
                SpecExpr::IntLit(3),
            ))))),
            "!(!(x == 3))",
        ),
        ("eq", Constraint::Eq(vx(), SpecExpr::IntLit(3)), "x == 3"),
        (
            "order-lt",
            Constraint::Order(OrderOp::LessThan, vx(), SpecExpr::IntLit(3)),
            "x < 3",
        ),
        (
            "order-le",
            Constraint::Order(OrderOp::LessThanOrEqual, vx(), SpecExpr::IntLit(3)),
            "x <= 3",
        ),
        (
            "order-gt-silent",
            Constraint::Order(OrderOp::GreaterThan, vx(), SpecExpr::IntLit(3)),
            "true",
        ),
        (
            "order-ge-silent",
            Constraint::Order(OrderOp::GreaterThanOrEqual, vx(), SpecExpr::IntLit(3)),
            "true",
        ),
        (
            "and-two",
            Constraint::And(vec![
                Constraint::Eq(vx(), SpecExpr::IntLit(3)),
                Constraint::Order(OrderOp::LessThan, vx(), SpecExpr::IntLit(4)),
            ]),
            "(x == 3 && x < 4)",
        ),
        (
            "and-one",
            Constraint::And(vec![Constraint::Eq(vx(), SpecExpr::IntLit(3))]),
            "(x == 3)",
        ),
        ("and-empty", Constraint::And(vec![]), "()"),
    ];
    for (label, cons, expected) in cons {
        let original = constraint_to_rust(&cons);
        push("proof-constraint", label, expected, original, CaseKind::Cons(cons));
    }

    // ── Expression walker (proof: expr_walker_*) ──
    let exprs: Vec<(&str, IRNode, &str)> = vec![
        (
            "binop-call-binop",
            bin(
                IRBinOp::Mul,
                call(
                    "add",
                    vec![bin(IRBinOp::Add, vr("a"), li(1)), vr("b")],
                ),
                li(2),
            ),
            "(add((a + 1), b) * 2)",
        ),
        (
            "append",
            bin(IRBinOp::Append, vr("items"), call("next", vec![])),
            "items.push(next())",
        ),
        (
            "merge",
            bin(IRBinOp::Merge, vr("a"), vr("b")),
            "a.extend(b)",
        ),
        (
            "last",
            IRNode::UnaryOp(IRUnaryOp::Last, Box::new(vr("xs"))),
            "(xs)[(xs).len() - 1]",
        ),
        (
            "init",
            IRNode::UnaryOp(IRUnaryOp::Init, Box::new(vr("xs"))),
            "(xs)[..(xs).len() - 1].to_vec()",
        ),
        (
            "concat-chain",
            bin(
                IRBinOp::Concat,
                bin(IRBinOp::Concat, ls("a"), vr("x")),
                ls("b"),
            ),
            "((\"a\".to_string() + &x) + \"b\")",
        ),
        (
            "concat-blocklike-right",
            bin(
                IRBinOp::Concat,
                vr("s"),
                IRNode::If {
                    condition: Box::new(vr("c")),
                    then_branch: Box::new(ls("y")),
                    else_branch: Box::new(ls("n")),
                },
            ),
            "(s.clone() + &(if c { \"y\" } else { \"n\" }))",
        ),
        (
            "concat-escaped-owned",
            bin(IRBinOp::Concat, ls("a\"b"), vr("x")),
            "(\"a\\\"b\".to_string() + &x)",
        ),
        (
            "if-match-cond",
            IRNode::If {
                condition: Box::new(IRNode::Match {
                    scrutinee: Box::new(vr("y")),
                    cases: vec![(IRPattern::Wildcard, li(1))],
                }),
                then_branch: Box::new(li(2)),
                else_branch: Box::new(li(3)),
            },
            "if (match y { _ => 1 }) { 2 } else { 3 }",
        ),
        (
            "if-binop-cond",
            IRNode::If {
                condition: Box::new(bin(IRBinOp::Or, vr("a"), vr("b"))),
                then_branch: Box::new(vr("t")),
                else_branch: Box::new(vr("e")),
            },
            "if (a || b) { t } else { e }",
        ),
        (
            "match-string-pat",
            IRNode::Match {
                scrutinee: Box::new(vr("s")),
                cases: vec![
                    (
                        IRPattern::Literal(IRLiteral::String("go".to_string())),
                        li(1),
                    ),
                    (IRPattern::Wildcard, li(0)),
                ],
            },
            "match s.as_str() { \"go\" => 1, _ => 0 }",
        ),
        (
            "match-nested-string-pat",
            IRNode::Match {
                scrutinee: Box::new(vr("s")),
                cases: vec![(
                    IRPattern::Constructor(
                        "Some".to_string(),
                        vec![IRPattern::Literal(IRLiteral::String("x".to_string()))],
                    ),
                    li(1),
                )],
            },
            "match s.as_str() { Some(\"x\") => 1 }",
        ),
        (
            "match-bare-literal-scrutinee",
            IRNode::Match {
                scrutinee: Box::new(ls("go")),
                cases: vec![(
                    IRPattern::Literal(IRLiteral::String("go".to_string())),
                    li(1),
                )],
            },
            "match \"go\" { \"go\" => 1 }",
        ),
        (
            "match-in-match",
            IRNode::Match {
                scrutinee: Box::new(vr("a")),
                cases: vec![(
                    IRPattern::Wildcard,
                    IRNode::Match {
                        scrutinee: Box::new(vr("b")),
                        cases: vec![(IRPattern::Wildcard, li(0))],
                    },
                )],
            },
            "match a { _ => match b { _ => 0 } }",
        ),
        (
            "lambda-binop",
            lam(&["x"], bin(IRBinOp::Add, vr("x"), li(1))),
            "|x| { (x + 1) }",
        ),
        (
            "if-hole-cond",
            IRNode::If {
                condition: Box::new(IRNode::Hole {
                    id: 0,
                    expected_type: None,
                }),
                then_branch: Box::new(li(1)),
                else_branch: Box::new(li(2)),
            },
            "if panic!(\"Hole not filled\") { 1 } else { 2 }",
        ),
        (
            "collection-set",
            IRNode::Collection {
                kind: CollectionKind::Set,
                elements: vec![li(1), li(2)],
            },
            "std::collections::HashSet::from([1, 2])",
        ),
        (
            "collection-map-tuple",
            IRNode::Collection {
                kind: CollectionKind::Map,
                elements: vec![IRNode::Collection {
                    kind: CollectionKind::Tuple,
                    elements: vec![ls("k"), li(1)],
                }],
            },
            "std::collections::HashMap::from([(\"k\", 1)])",
        ),
        (
            "collection-list-binop",
            IRNode::Collection {
                kind: CollectionKind::List,
                elements: vec![bin(IRBinOp::Add, vr("a"), li(1))],
            },
            "vec![(a + 1)]",
        ),
        (
            "map-lambda",
            IRNode::Map {
                function: Box::new(lam(&["x"], bin(IRBinOp::Mul, vr("x"), li(2)))),
                over: Box::new(vr("xs")),
            },
            "(xs).iter().map(|x| { (x * 2) }).collect()",
        ),
        (
            "filter",
            IRNode::Filter {
                predicate: Box::new(vr("ok")),
                over: Box::new(vr("xs")),
            },
            "(xs).iter().filter(ok).collect()",
        ),
        (
            "fold",
            IRNode::Fold {
                operation: Box::new(vr("add")),
                initial: Box::new(li(0)),
                over: Box::new(vr("xs")),
            },
            "(xs).iter().fold(0, add)",
        ),
        (
            "sort",
            IRNode::Sort {
                comparator: Box::new(vr("cmp")),
                collection: Box::new(vr("xs")),
            },
            "{ let mut sorted = xs; sorted.sort_by(cmp); sorted }",
        ),
        (
            "map-over-filter",
            IRNode::Map {
                function: Box::new(vr("f")),
                over: Box::new(IRNode::Filter {
                    predicate: Box::new(vr("p")),
                    over: Box::new(vr("xs")),
                }),
            },
            "((xs).iter().filter(p).collect()).iter().map(f).collect()",
        ),
        (
            "return-in-binop",
            bin(
                IRBinOp::Add,
                IRNode::Return(Box::new(li(5))),
                li(1),
            ),
            "((return 5) + 1)",
        ),
        (
            "alloc-fallback",
            IRNode::Alloc {
                name: "x".to_string(),
                ty: TypeRef::Unit,
                initial: Box::new(li(0)),
            },
            "panic!(\"unsupported IR in expression position: Alloc\")",
        ),
        (
            "hole",
            IRNode::Hole {
                id: 0,
                expected_type: None,
            },
            "panic!(\"Hole not filled\")",
        ),
        (
            "struct-def",
            IRNode::StructDef {
                name: "Point".to_string(),
                fields: vec![
                    ("x".to_string(), TypeRef::Int),
                    ("y".to_string(), TypeRef::Int),
                ],
                body: Box::new(vr("origin")),
            },
            "{ struct Point { x: i64, y: i64 } origin }",
        ),
        (
            "loop-call",
            IRNode::Loop {
                body: Box::new(call("tick", vec![])),
            },
            "loop {\ntick();}",
        ),
        (
            "try-index",
            IRNode::TryPropagate(Box::new(IRNode::Index(
                Box::new(vr("xs")),
                Box::new(li(0)),
            ))),
            "((xs)[0])?",
        ),
        (
            "index-call",
            IRNode::Index(Box::new(call("get", vec![vr("i")])), Box::new(li(0))),
            "(get(i))[0]",
        ),
        (
            "assign-binop",
            IRNode::Assign {
                target: Box::new(vr("x")),
                value: Box::new(bin(IRBinOp::Add, vr("x"), li(1))),
            },
            "(x = (x + 1))",
        ),
        (
            "break-value",
            IRNode::Break {
                value: Some(Box::new(vr("v"))),
            },
            "(break v)",
        ),
        ("break-none", IRNode::Break { value: None }, "(break)"),
        (
            "char-quote",
            IRNode::Literal(IRLiteral::Char('\'')),
            "'\\''",
        ),
        (
            "char-dquote",
            IRNode::Literal(IRLiteral::Char('"')),
            "'\"'",
        ),
    ];
    for (label, node, expected) in exprs {
        let original = ir_to_rust_expr(&node);
        push("proof-expr", label, expected, original, CaseKind::Expr(node));
    }

    // ── Statement walker (proof: stmt_walker_*) ──
    let stmts: Vec<(&str, IRNode, usize, TypeRef, &str)> = vec![
        (
            "block-alloc-call-return",
            IRNode::Block(vec![
                IRNode::Alloc {
                    name: "buf".to_string(),
                    ty: TypeRef::String,
                    initial: Box::new(ls("hi")),
                },
                call("log", vec![vr("buf")]),
                IRNode::Return(Box::new(ls("done"))),
            ]),
            1,
            TypeRef::String,
            "    let mut buf: String = \"hi\".to_string();\n    log(buf);\n    return \"done\".to_string();\n",
        ),
        (
            "block-hole",
            IRNode::Block(vec![IRNode::Hole {
                id: 0,
                expected_type: None,
            }]),
            1,
            TypeRef::Int,
            "    // TODO: synthesis incomplete\n    panic!(\"Hole not filled\");\n",
        ),
        (
            "block-two-calls",
            IRNode::Block(vec![call("f", vec![li(1)]), call("g", vec![])]),
            0,
            TypeRef::Int,
            "f(1);\ng();\n",
        ),
        (
            "block-if-nested",
            IRNode::Block(vec![IRNode::If {
                condition: Box::new(vr("c")),
                then_branch: Box::new(IRNode::Block(vec![IRNode::Assign {
                    target: Box::new(vr("x")),
                    value: Box::new(li(1)),
                }])),
                else_branch: Box::new(IRNode::Block(vec![call("f", vec![])])),
            }]),
            1,
            TypeRef::Int,
            "    if c {\n        x = 1;\n} else {\n        f()\n}\n",
        ),
        (
            "match-string-pat",
            IRNode::Match {
                scrutinee: Box::new(vr("v")),
                cases: vec![
                    (
                        IRPattern::Literal(IRLiteral::String("a".to_string())),
                        li(1),
                    ),
                    (IRPattern::Wildcard, call("g", vec![])),
                ],
            },
            1,
            TypeRef::Int,
            "    match v.as_str() {\n        \"a\" => {\n            1\n        },\n        _ => {\n            g()\n        },\n    }\n",
        ),
        (
            "block-while-invariant",
            IRNode::Block(vec![IRNode::While {
                condition: Box::new(vr("c")),
                body: Box::new(IRNode::Block(vec![IRNode::Assign {
                    target: Box::new(vr("x")),
                    value: Box::new(bin(IRBinOp::Add, vr("x"), li(1))),
                }])),
                invariant: Some("x > 0".to_string()),
            }]),
            1,
            TypeRef::Int,
            "    // invariant: x > 0\n    while c {\n        x = (x + 1);\n    }\n",
        ),
        // tail-vs-statement, statement side (indent 1, Int)
        (
            "stmt-list",
            IRNode::Collection {
                kind: CollectionKind::List,
                elements: vec![li(1), li(2)],
            },
            1,
            TypeRef::Int,
            "    vec![1, 2];\n",
        ),
        (
            "stmt-set-collapsed",
            IRNode::Collection {
                kind: CollectionKind::Set,
                elements: vec![li(1)],
            },
            1,
            TypeRef::Int,
            "    vec![1];\n",
        ),
        ("stmt-call", call("f", vec![li(1)]), 1, TypeRef::Int, "    f(1);\n"),
        (
            "stmt-map",
            IRNode::Map {
                function: Box::new(vr("f")),
                over: Box::new(vr("xs")),
            },
            1,
            TypeRef::Int,
            "    (xs).iter().map(f).collect();\n",
        ),
        (
            "stmt-sort",
            IRNode::Sort {
                comparator: Box::new(vr("cmp")),
                collection: Box::new(vr("xs")),
            },
            1,
            TypeRef::Int,
            "    let mut sorted = xs;\nsorted.sort_by(cmp);\nsorted;\n",
        ),
        // return ownership (indent 0)
        (
            "return-owned",
            IRNode::Return(Box::new(ls("s"))),
            0,
            TypeRef::String,
            "return \"s\".to_string();\n",
        ),
        (
            "return-bare",
            IRNode::Return(Box::new(ls("s"))),
            0,
            TypeRef::Int,
            "return \"s\";\n",
        ),
        (
            "let-owned-body",
            IRNode::Let {
                name: "x".to_string(),
                value: Box::new(li(1)),
                body: Box::new(ls("s")),
            },
            0,
            TypeRef::String,
            "let x = 1;\n\"s\".to_string()\n",
        ),
        (
            "let-bare-body",
            IRNode::Let {
                name: "x".to_string(),
                value: Box::new(li(1)),
                body: Box::new(ls("s")),
            },
            0,
            TypeRef::Int,
            "let x = 1;\n\"s\"\n",
        ),
        (
            "if-owned-branches",
            IRNode::If {
                condition: Box::new(vr("c")),
                then_branch: Box::new(ls("a")),
                else_branch: Box::new(ls("b")),
            },
            0,
            TypeRef::String,
            "if c {\n    \"a\".to_string()\n} else {\n    \"b\".to_string()\n}\n",
        ),
        (
            "alloc-string-owned",
            IRNode::Alloc {
                name: "s".to_string(),
                ty: TypeRef::String,
                initial: Box::new(ls("v")),
            },
            0,
            TypeRef::Int,
            "let mut s: String = \"v\".to_string();\n",
        ),
        (
            "alloc-int-bare",
            IRNode::Alloc {
                name: "n".to_string(),
                ty: TypeRef::Int,
                initial: Box::new(ls("v")),
            },
            0,
            TypeRef::String,
            "let mut n: i64 = \"v\";\n",
        ),
        (
            "typed-stmt-transparent",
            IRNode::Typed {
                node: Box::new(ls("s")),
                ty: TypeRef::String,
            },
            0,
            TypeRef::Int,
            "\"s\";\n",
        ),
    ];
    for (label, node, indent, ret, expected) in stmts {
        let original = ir_to_rust(&node, indent, &ret);
        push(
            "proof-stmt",
            label,
            expected,
            original,
            CaseKind::Stmt(node, indent, ret),
        );
    }

    // ── Tails (proof: stmt_walker_tail_vs_statement +
    //    stmt_walker_return_ownership), indent 1 / fn_ret Int
    //    / expected None unless stated ──
    let tails: Vec<(&str, IRNode, usize, TypeRef, Option<TypeRef>, &str)> = vec![
        (
            "tail-list",
            IRNode::Collection {
                kind: CollectionKind::List,
                elements: vec![li(1), li(2)],
            },
            1,
            TypeRef::Int,
            None,
            "    vec![1, 2]\n",
        ),
        (
            "tail-set",
            IRNode::Collection {
                kind: CollectionKind::Set,
                elements: vec![li(1)],
            },
            1,
            TypeRef::Int,
            None,
            "    std::collections::HashSet::from([1])\n",
        ),
        (
            "tail-call",
            call("f", vec![li(1)]),
            1,
            TypeRef::Int,
            None,
            "    f(1)\n",
        ),
        (
            "tail-map",
            IRNode::Map {
                function: Box::new(vr("f")),
                over: Box::new(vr("xs")),
            },
            1,
            TypeRef::Int,
            None,
            "    (xs).iter().map(f).collect()\n",
        ),
        (
            "tail-sort",
            IRNode::Sort {
                comparator: Box::new(vr("cmp")),
                collection: Box::new(vr("xs")),
            },
            1,
            TypeRef::Int,
            None,
            "    { let mut sorted = xs; sorted.sort_by(cmp); sorted }\n",
        ),
        (
            "tail-block",
            IRNode::Block(vec![call("f", vec![li(1)]), li(5)]),
            1,
            TypeRef::Int,
            None,
            "    f(1);\n    5\n",
        ),
        (
            "tail-block-empty",
            IRNode::Block(vec![]),
            1,
            TypeRef::Int,
            None,
            "",
        ),
        (
            "tail-typed-owned",
            IRNode::Typed {
                node: Box::new(ls("s")),
                ty: TypeRef::String,
            },
            0,
            TypeRef::Int,
            None,
            "\"s\".to_string()\n",
        ),
    ];
    for (label, node, indent, ret, expected_ty, expected) in tails {
        let original = ir_to_rust_tail(&node, indent, &ret, expected_ty.as_ref());
        push(
            "proof-tail",
            label,
            expected,
            original,
            CaseKind::Tail(node, indent, ret, expected_ty),
        );
    }

    cases
}

/// Build a candidate from raw parts (the DriverSpec cases
/// of corpus (iii), transcribed from tests/m3_proof.rs's
/// slice-6 golden tests into a real spec + body).
pub(super) fn make_candidate(
    name: &str,
    doc: Option<&str>,
    generics: Vec<&str>,
    params: Vec<(&str, TypeRef)>,
    ret: TypeRef,
    preconditions: Vec<Constraint>,
    body: IRNode,
    tests: Vec<(Option<&str>, SpecExpr, SpecExpr)>,
) -> CandidateImplementation {
    use crate::spec::ast::{GenericParam, Param, Test};
    let spec = FunctionSpec {
        name: name.to_string(),
        doc: doc.map(|d| d.to_string()),
        generics: generics
            .iter()
            .map(|g| GenericParam {
                name: g.to_string(),
                bounds: vec![],
            })
            .collect(),
        params: params
            .iter()
            .map(|(n, t)| Param {
                name: n.to_string(),
                ty: t.clone(),
                annotations: HashMap::new(),
            })
            .collect(),
        return_type: ret,
        preconditions,
        postconditions: vec![],
        invariants: vec![],
        complexity: None,
        resource: None,
        tests: tests
            .into_iter()
            .map(|(name, input, expected_output)| Test {
                name: name.map(|n| n.to_string()),
                input,
                expected_output,
                timeout_ms: None,
                property: false,
            })
            .collect(),
        annotations: HashMap::new(),
    };
    CandidateImplementation {
        id: 0,
        body,
        spec_name: name.to_string(),
        spec,
        score: 0.0,
        scores: engine::ScoreBreakdown {
            test_pass_ratio: 0.0,
            constraint_score: 0.0,
            complexity_score: 0.0,
            quality_score: 0.0,
            synthesis_time_us: 0,
        },
        generation: 0,
        provenance: Provenance::HumanGuided,
        verified: true,
        verification_status: None,
    }
}

/// Corpus (iii): the slice-6 DriverSpec end-to-end cases as
/// real candidates (original side: the real `generate`).
pub(super) fn driver_corpus() -> Vec<Case> {
    let mut cases = Vec::new();
    let mut push = |label: &str,
                    expected: &'static str,
                    cand: CandidateImplementation| {
        let original = generate(&cand, "rust").expect("generate must succeed");
        cases.push(Case {
            class: "proof-driver",
            label: label.to_string(),
            expected: Some(expected),
            original,
            kind: CaseKind::File(Box::new(cand)),
        });
    };
    push(
        "full",
        concat!(
            "//! Sorts things.\n",
            "//! Does it well.\n",
            "\n",
            "pub fn clamp_value<T>(x: i64, limit: i64) -> i64 {\n",
            "#[cfg(debug_assertions)]\n",
            "{\n",
            "    debug_assert!(x == 3, \"Precondition violated\");\n",
            "}\n",
            "\n",
            "    x\n",
            "}\n",
            "\n#[cfg(test)]\n",
            "mod tests {\n",
            "    use super::*;\n",
            "\n",
            "    #[test]\n",
            "    fn x_is_three() {\n",
            "        let result = clamp_value(3);\n",
            "        assert_eq!(result, 3);\n",
            "    }\n",
            "    #[test]\n",
            "    fn test_1() {\n",
            "        let result = clamp_value(0);\n",
            "        assert_eq!(result, 0);\n",
            "    }\n",
            "}\n",
        ),
        make_candidate(
            "clamp_value",
            Some("Sorts things.\nDoes it well."),
            vec!["T"],
            vec![("x", TypeRef::Int), ("limit", TypeRef::Int)],
            TypeRef::Int,
            vec![Constraint::Eq(
                SpecExpr::Var("x".to_string()),
                SpecExpr::IntLit(3),
            )],
            vr("x"),
            vec![
                (
                    Some("x-is-three"),
                    SpecExpr::IntLit(3),
                    SpecExpr::IntLit(3),
                ),
                (None, SpecExpr::IntLit(0), SpecExpr::IntLit(0)),
            ],
        ),
    );
    push(
        "test-value-shapes",
        concat!(
            "pub fn sort_by(xs: i64, cmp: i64) -> i64 {\n",
            "    xs\n",
            "}\n",
            "\n#[cfg(test)]\n",
            "mod tests {\n",
            "    use super::*;\n",
            "\n",
            "    #[test]\n",
            "    fn spread() {\n",
            "        let result = sort_by(vec![2, 1], |a, b| (a < b));\n",
            "        assert_eq!(result, vec![1, 2]);\n",
            "    }\n",
            "    #[test]\n",
            "    fn pair_out() {\n",
            "        let result = sort_by(1, None);\n",
            "        assert_eq!(result, (1, None));\n",
            "    }\n",
            "}\n",
        ),
        make_candidate(
            "sort_by",
            None,
            vec![],
            vec![("xs", TypeRef::Int), ("cmp", TypeRef::Int)],
            TypeRef::Int,
            vec![],
            vr("xs"),
            vec![
                (
                    Some("spread"),
                    SpecExpr::Call(
                        "__tuple__".to_string(),
                        vec![
                            SpecExpr::Call(
                                "List".to_string(),
                                vec![SpecExpr::IntLit(2), SpecExpr::IntLit(1)],
                            ),
                            SpecExpr::Lambda(
                                vec![
                                    ("a".to_string(), TypeRef::Int),
                                    ("b".to_string(), TypeRef::Int),
                                ],
                                Box::new(SpecExpr::BinOp(
                                    SpecBinOp::Lt,
                                    Box::new(SpecExpr::Var("a".to_string())),
                                    Box::new(SpecExpr::Var("b".to_string())),
                                )),
                            ),
                        ],
                    ),
                    SpecExpr::Call(
                        "List".to_string(),
                        vec![SpecExpr::IntLit(1), SpecExpr::IntLit(2)],
                    ),
                ),
                (
                    Some("pair-out"),
                    SpecExpr::Call(
                        "__tuple__".to_string(),
                        vec![
                            SpecExpr::IntLit(1),
                            SpecExpr::Call("None".to_string(), vec![]),
                        ],
                    ),
                    SpecExpr::Call(
                        "__tuple__".to_string(),
                        vec![
                            SpecExpr::IntLit(1),
                            SpecExpr::Call("None".to_string(), vec![]),
                        ],
                    ),
                ),
            ],
        ),
    );
    push(
        "bare",
        "pub fn answer(n: i64) -> bool {\n    true\n}\n",
        make_candidate(
            "answer",
            None,
            vec![],
            vec![("n", TypeRef::Int)],
            TypeRef::Bool,
            vec![],
            lb(true),
            vec![],
        ),
    );
    push(
        "empty-doc",
        "\npub fn answer(n: i64) -> bool {\n    true\n}\n",
        make_candidate(
            "answer",
            Some(""),
            vec![],
            vec![("n", TypeRef::Int)],
            TypeRef::Bool,
            vec![],
            lb(true),
            vec![],
        ),
    );
    push(
        "string-param-placement",
        concat!(
            "pub fn shout(s: String) -> String {\n",
            "    s\n",
            "}\n",
            "\n#[cfg(test)]\n",
            "mod tests {\n",
            "    use super::*;\n",
            "\n",
            "    #[test]\n",
            "    fn test_0() {\n",
            "        let result = shout(\"hey\");\n",
            "        assert_eq!(result, \"hey\");\n",
            "    }\n",
            "}\n",
        ),
        make_candidate(
            "shout",
            None,
            vec![],
            vec![("s", TypeRef::String)],
            TypeRef::String,
            vec![],
            vr("s"),
            vec![(
                None,
                SpecExpr::StringLit("hey".to_string()),
                SpecExpr::StringLit("hey".to_string()),
            )],
        ),
    );
    cases
}

/// Corpus (v): the dispatch battery.
pub(super) fn dispatch_corpus(dummy: &CandidateImplementation) -> Vec<Case> {
    [
        "rust",
        "c",
        "wasm",
        "python",
        "javascript",
        "js",
        "ir",
        "java",
        "Rust",
        "JS",
        "rs",
        "",
        "rust ",
        " rust",
        "c++",
        "j",
        "my lang!",
    ]
    .iter()
    .map(|target| {
        let original = match generate(dummy, target) {
            Ok(_) => "Ok".to_string(),
            Err(e) => format!("Err:{}", e),
        };
        Case {
            class: "dispatch",
            label: format!("target {:?}", target),
            expected: None,
            original,
            kind: CaseKind::Dispatch(target.to_string()),
        }
    })
    .collect()
}

/// Every subtree of `node`, including itself.
fn collect_subtrees(node: &IRNode, out: &mut Vec<IRNode>) {
    out.push(node.clone());
    let _ = rebuild_children(node, &mut |child| {
        collect_subtrees(child, out);
        Ok(child.clone())
    });
}

/// Corpus (ii) + (iv): the winner bodies and their unique
/// subtrees (three renderings each), and every winner as a
/// whole candidate file.
pub(super) fn winner_corpus(sets: &[(&str, &WinnerSet)]) -> Vec<Case> {
    let mut cases = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (module, set) in sets {
        for name in &set.order {
            let entry = set.get(name).unwrap();
            let cand = &entry.candidate;
            let ret = cand.spec.return_type.clone();
            // (iv) the whole file.
            let original = generate(cand, "rust").expect("generate must succeed");
            cases.push(Case {
                class: "winner-file",
                label: format!("{}::{}", module, name),
                expected: None,
                original,
                kind: CaseKind::File(Box::new(cand.clone())),
            });
            // (ii) the body and every unique subtree.
            let mut trees = Vec::new();
            collect_subtrees(&cand.body, &mut trees);
            for (i, tree) in trees.iter().enumerate() {
                let key = format!("{:?}", tree);
                if !seen.insert(key) {
                    continue;
                }
                let label = format!("{}::{}#sub{}", module, name, i);
                cases.push(Case {
                    class: "winner-tree-expr",
                    label: label.clone(),
                    expected: None,
                    original: ir_to_rust_expr(tree),
                    kind: CaseKind::Expr(tree.clone()),
                });
                cases.push(Case {
                    class: "winner-tree-stmt",
                    label: label.clone(),
                    expected: None,
                    original: ir_to_rust(tree, 1, &ret),
                    kind: CaseKind::Stmt(tree.clone(), 1, ret.clone()),
                });
                cases.push(Case {
                    class: "winner-tree-tail",
                    label,
                    expected: None,
                    original: ir_to_rust_tail(tree, 1, &ret, Some(&ret)),
                    kind: CaseKind::Tail(tree.clone(), 1, ret.clone(), Some(ret.clone())),
                });
            }
        }
    }
    cases
}

// ── The comparison runner ─────────────────────────────────

#[derive(Default)]
struct ClassStats {
    total: usize,
    a_match: usize,
    b_match: usize,
    exec_fail: usize,
    a_details: Vec<String>,
    b_details: Vec<String>,
    a_unreconciled: Vec<String>,
}

pub(super) fn diff_detail(label: &str, original: &str, composed: &str) -> String {
    let (a, b) = (original.as_bytes(), composed.as_bytes());
    let i = a
        .iter()
        .zip(b.iter())
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| a.len().min(b.len()));
    let lo = i.saturating_sub(28);
    let a_hi = (i + 36).min(a.len());
    let b_hi = (i + 36).min(b.len());
    format!(
        "{}: first diff at byte {} (original {} bytes, composed {} bytes)\n    original: …{}\n    composed: …{}",
        label,
        i,
        a.len(),
        b.len(),
        String::from_utf8_lossy(&a[lo..a_hi]),
        String::from_utf8_lossy(&b[lo..b_hi]),
    )
}

fn run_corpus(
    winners: &WinnerSet,
    cases: &[Case],
) -> (Vec<(String, ClassStats)>, Vec<String>) {
    let composed_a = Composed {
        w: winners,
        escape_literals: false,
    };
    let composed_b = Composed {
        w: winners,
        escape_literals: true,
    };
    let mut classes: Vec<(String, ClassStats)> = Vec::new();
    let mut transcription_failures: Vec<String> = Vec::new();
    for case in cases {
        if let Some(expected) = case.expected {
            if case.original != expected {
                transcription_failures.push(format!(
                    "{} ({}): ORIGINAL does not reproduce the proof golden\n    proof:    {:?}\n    original: {:?}",
                    case.class, case.label, expected, case.original
                ));
            }
        }
        let idx = match classes.iter().position(|(c, _)| c == case.class) {
            Some(i) => i,
            None => {
                classes.push((case.class.to_string(), ClassStats::default()));
                classes.len() - 1
            }
        };
        let stats = &mut classes[idx].1;
        stats.total += 1;
        let res_a = render(&composed_a, case);
        let res_b = render(&composed_b, case);
        let (a_ok, b_ok) = match (&res_a, &res_b) {
            (Err(e), _) | (_, Err(e)) => {
                stats.exec_fail += 1;
                if stats.a_details.len() < 5 {
                    stats
                        .a_details
                        .push(format!("{}: EXECUTION FAILURE: {}", case.label, e));
                }
                (false, false)
            }
            (Ok(a), Ok(b)) => {
                let a_ok = *a == case.original;
                let b_ok = *b == case.original;
                if a_ok {
                    stats.a_match += 1;
                } else if stats.a_details.len() < 5 {
                    stats.a_details.push(diff_detail(&case.label, &case.original, a));
                }
                if b_ok {
                    stats.b_match += 1;
                } else if stats.b_details.len() < 5 {
                    stats.b_details.push(diff_detail(&case.label, &case.original, b));
                }
                if !a_ok && !b_ok {
                    stats.a_unreconciled.push(case.label.clone());
                }
                (a_ok, b_ok)
            }
        };
        let _ = (a_ok, b_ok);
    }
    (classes, transcription_failures)
}

// ── The gate ──────────────────────────────────────────────

#[test]
#[ignore = "M3 gate: 2 randomized synthesis passes x 116 specs + corpus execution; run explicitly with --ignored --nocapture"]
fn m3_gate() {
    let root = env!("CARGO_MANIFEST_DIR");
    let out_dir = format!("{}/target/m3-gate", root);
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut report = String::new();
    let mut writelnr = |s: &str| {
        println!("{}", s);
        report.push_str(s);
        report.push('\n');
    };

    // ── 1. Winner extraction, twice (reproducibility) ──
    let functions3 = load_module("codegen.morph");
    assert_eq!(functions3.len(), 116, "Codegen module spec count");
    let pass1 = extract_winners(&functions3);
    let pass2 = extract_winners(&functions3);
    for name in &pass1.order {
        let (a, b) = (pass1.get(name).unwrap(), pass2.get(name).unwrap());
        assert_eq!(
            a.body_key, b.body_key,
            "{}: winner body differs across extraction passes",
            name
        );
        assert_eq!(
            a.provenance, b.provenance,
            "{}: winner provenance differs across extraction passes",
            name
        );
        assert!(a.verified, "{}: winner is not Z3-Verified", name);
    }
    let winners = pass1;
    let seed_count = winners
        .order
        .iter()
        .filter(|n| winners.get(n).unwrap().seed_kind.is_some())
        .count();
    let pool: Vec<&str> = winners
        .order
        .iter()
        .map(|n| n.as_str())
        .filter(|n| winners.get(n).unwrap().seed_kind.is_none())
        .collect();
    writelnr(&format!(
        "GATE3|winners|seeds={}|pool={:?}",
        seed_count, pool
    ));
    assert_eq!(seed_count, 113, "Phase 55 measured 113 constraint seeds");
    assert_eq!(
        pool,
        vec!["rust_expr_call_form", "rust_ident_empty", "rust_target_route"],
        "the by-design seedless trio"
    );
    for name in &pool {
        assert_eq!(
            winners.get(name).unwrap().provenance,
            "Mutation",
            "{}: pool winner provenance",
            name
        );
    }
    let mut kind_tally: Vec<(String, usize)> = Vec::new();
    for name in &winners.order {
        if let Some(kind) = &winners.get(name).unwrap().seed_kind {
            match kind_tally.iter_mut().find(|(k, _)| k == kind) {
                Some((_, c)) => *c += 1,
                None => kind_tally.push((kind.clone(), 1)),
            }
        }
    }
    writelnr(&format!("GATE3|seed-kinds|{:?}", kind_tally));

    // Call census + resolution effect.
    let (mut total_calls, mut changed) = (0usize, 0usize);
    let mut callee_names: Vec<String> = Vec::new();
    for name in &winners.order {
        let entry = winners.get(name).unwrap();
        let (count, names) = call_census(&entry.candidate.body);
        total_calls += count;
        for n in names {
            if !callee_names.contains(&n) {
                callee_names.push(n);
            }
        }
        if format!("{:?}", entry.resolved) != entry.body_key {
            changed += 1;
        }
    }
    writelnr(&format!(
        "GATE3|call-census|total-calls={}|callees={:?}|bodies-changed-by-resolution={}",
        total_calls, callee_names, changed
    ));

    // Executor smoke: every winner executes on canonical
    // inputs and returns the declared result kind.
    for name in &winners.order {
        let entry = winners.get(name).unwrap();
        let spec = &entry.candidate.spec;
        let mut args = Vec::new();
        for param in &spec.params {
            args.push(match param.ty {
                TypeRef::Int => Value::Int(0),
                TypeRef::Bool => Value::Bool(false),
                TypeRef::String => Value::Str(String::new()),
                TypeRef::Float => Value::Float(0.0),
                TypeRef::Char => Value::Char('x'),
                TypeRef::Unit => Value::Unit,
                _ => Value::List(vec![]),
            });
        }
        let out = winners
            .call(name, &args)
            .unwrap_or_else(|e| panic!("executor smoke: {}", e));
        let kind_ok = match (&spec.return_type, &out) {
            (TypeRef::Int, Value::Int(_))
            | (TypeRef::Bool, Value::Bool(_))
            | (TypeRef::String, Value::Str(_))
            | (TypeRef::Float, Value::Float(_))
            | (TypeRef::Char, Value::Char(_))
            | (TypeRef::Unit, Value::Unit) => true,
            _ => false,
        };
        assert!(
            kind_ok,
            "{}: executor smoke returned {:?} for return type {}",
            name, out, spec.return_type
        );
    }
    writelnr("GATE3|executor-smoke|116/116 winners execute on canonical inputs");

    // ── 2. M2 winners (corpus sources) ──
    let mut m2_sets: Vec<(&str, WinnerSet)> = Vec::new();
    let mut m2_total = 0usize;
    for file in ["validator.morph", "printer.morph"] {
        let functions = load_module(file);
        let set = extract_winners(&functions);
        for name in &set.order {
            let entry = set.get(name).unwrap();
            assert!(entry.verified, "M2 {}: winner not Verified", name);
            assert!(
                entry.seed_kind.is_some(),
                "M2 {}: winner is not a constraint seed (Phase 19 measured 14/14)",
                name
            );
        }
        m2_total += set.order.len();
        m2_sets.push((file, set));
    }
    assert_eq!(m2_total, 14, "M2 module spec count");
    writelnr("GATE3|m2-winners|14/14 constraint seeds, Verified");

    // ── 3. The synthesized-codegen artifact ──
    let mut module_text = String::new();
    module_text.push_str(
        "// The synthesized codegen: the Codegen module's 116 winners,\n\
         // each rendered by the original `generate` and wrapped in its\n\
         // own module (so the per-file `mod tests` cannot collide).\n\
         // Judged by the M3 gate harness (src/codegen/m3_gate.rs).\n\n",
    );
    for name in &winners.order {
        let entry = winners.get(name).unwrap();
        let file = generate(&entry.candidate, "rust").expect("generate winner file");
        let _ = writeln!(module_text, "mod {} {{", name);
        module_text.push_str(&file);
        module_text.push_str("}\n\n");
    }
    let artifact_path = format!("{}/synthesized_codegen.rs", out_dir);
    std::fs::write(&artifact_path, &module_text).unwrap();
    let rlib_path = format!("{}/synthesized_codegen.rlib", out_dir);
    let compile = Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("lib")
        .arg(&artifact_path)
        .arg("-o")
        .arg(&rlib_path)
        .output()
        .expect("spawn rustc");
    let artifact_compiles = compile.status.success();
    writelnr(&format!(
        "GATE3|artifact|path={}|bytes={}|compiles={}",
        artifact_path,
        module_text.len(),
        artifact_compiles
    ));
    if !artifact_compiles {
        let stderr = String::from_utf8_lossy(&compile.stderr);
        for line in stderr.lines().filter(|l| l.starts_with("error")).take(5) {
            writelnr(&format!("GATE3|artifact-error|{}", line));
        }
    }

    // ── 4. The corpus ──
    let dummy = make_candidate(
        "dummy",
        None,
        vec![],
        vec![],
        TypeRef::Unit,
        vec![],
        IRNode::Literal(IRLiteral::Unit),
        vec![],
    );
    let mut cases = proof_corpus();
    cases.extend(driver_corpus());
    cases.extend(dispatch_corpus(&dummy));
    let mut sets: Vec<(&str, &WinnerSet)> = vec![("codegen", &winners)];
    for (file, set) in &m2_sets {
        sets.push((file, set));
    }
    cases.extend(winner_corpus(&sets));
    let total_cases = cases.len();
    writelnr(&format!("GATE3|corpus|total={}", total_cases));

    let (classes, transcription_failures) = run_corpus(&winners, &cases);
    let mut grand_total = 0usize;
    let mut grand_b = 0usize;
    let mut grand_a = 0usize;
    let mut grand_exec_fail = 0usize;
    let mut grand_unreconciled = 0usize;
    for (class, stats) in &classes {
        grand_total += stats.total;
        grand_b += stats.b_match;
        grand_a += stats.a_match;
        grand_exec_fail += stats.exec_fail;
        grand_unreconciled += stats.a_unreconciled.len();
        writelnr(&format!(
            "GATE3|class={}|total={}|A-match={}|B-match={}|exec-fail={}|A-unreconciled={}",
            class,
            stats.total,
            stats.a_match,
            stats.b_match,
            stats.exec_fail,
            stats.a_unreconciled.len()
        ));
        for d in &stats.b_details {
            writelnr(&format!("GATE3|B-MISMATCH|{}", d));
        }
        for d in &stats.a_details {
            writelnr(&format!("GATE3|A-note|{}", d));
        }
        for u in &stats.a_unreconciled {
            writelnr(&format!("GATE3|A-UNRECONCILED|{}", u));
        }
    }
    for f in &transcription_failures {
        writelnr(&format!("GATE3|TRANSCRIPTION|{}", f));
    }
    writelnr(&format!(
        "GATE3|totals|cases={}|A-match={}|B-match={}|exec-fail={}|A-unreconciled={}|transcription-failures={}",
        grand_total,
        grand_a,
        grand_b,
        grand_exec_fail,
        grand_unreconciled,
        transcription_failures.len()
    ));

    // ── 5. Verdict ──
    assert!(
        transcription_failures.is_empty(),
        "corpus transcription does not reproduce the proof golden on the original side"
    );
    assert!(artifact_compiles, "the synthesized codegen module does not compile");
    assert_eq!(grand_exec_fail, 0, "composed codegen failed to execute on corpus items");
    assert_eq!(
        grand_b, grand_total,
        "mode B (winners + assembly + mirror-pinned F5 layer) is not byte-identical over the corpus"
    );
    assert_eq!(
        grand_unreconciled, 0,
        "a strict-composition (mode A) mismatch is NOT reconciled by the F5 layer"
    );
    writelnr(&format!(
        "GATE3-VERDICT|PASS|byte-identical over {} cases (mode B); strict composition (mode A) matches {} — every A divergence is the recorded F5 escaping class, reconciled mechanically",
        grand_total, grand_a
    ));
    std::fs::write(format!("{}/gate-report.md", out_dir), &report).unwrap();
}

// ── Mechanics tests (not ignored) ─────────────────────────
//
// The harness must provably do what it says on hand-built
// inputs, without synthesis: kind codes, winner-call
// resolution + execution, the capture refusal, and exactly
// what the A/B literal modes change.

pub(super) fn hand_entry(
    name: &str,
    params: Vec<(&str, TypeRef)>,
    ret: TypeRef,
    body: IRNode,
) -> CandidateImplementation {
    make_candidate(name, None, vec![], params, ret, vec![], body, vec![])
}

pub(super) fn hand_set(cands: Vec<CandidateImplementation>) -> WinnerSet {
    let raw: HashMap<String, CandidateImplementation> = cands
        .iter()
        .map(|c| (c.spec_name.clone(), c.clone()))
        .collect();
    let mut cache = HashMap::new();
    let mut entries = HashMap::new();
    let mut order = Vec::new();
    for cand in &cands {
        let resolved = resolve_spec_body(&cand.spec_name, &raw, &mut cache, &[])
            .expect("hand-built resolution");
        order.push(cand.spec_name.clone());
        entries.insert(
            cand.spec_name.clone(),
            WinnerEntry {
                body_key: format!("{:?}", cand.body),
                resolved,
                provenance: "ConstraintInduced(equation)".to_string(),
                seed_kind: Some("equation".to_string()),
                verified: true,
                tpr: 1.0,
                candidate: cand.clone(),
            },
        );
    }
    WinnerSet { order, entries }
}

#[test]
fn gate_kind_codes() {
    assert_eq!(
        node_kind(&IRNode::Loop {
            body: Box::new(vr("x"))
        }),
        24
    );
    assert_eq!(
        node_kind(&IRNode::Closure {
            params: vec![],
            captures: vec![],
            body: Box::new(vr("x")),
        }),
        14
    );
    assert_eq!(node_kind(&IRNode::TryPropagate(Box::new(vr("x")))), 27);
    assert_eq!(node_kind(&IRNode::Typed {
        node: Box::new(vr("x")),
        ty: TypeRef::Int,
    }), 22);
    assert_eq!(binop_code(IRBinOp::Concat), 16);
    assert_eq!(binop_code(IRBinOp::Merge), 18);
    assert_eq!(unary_code(IRUnaryOp::Clone), 9);
    assert_eq!(collection_code(CollectionKind::Tuple), 4);
    assert_eq!(lit_kind(&IRLiteral::String("s".to_string())), 3);
    assert_eq!(type_code(&TypeRef::Stream(Box::new(TypeRef::Int))), 25);
    assert_eq!(expected_code(None), 26);
    assert_eq!(expected_code(Some(&TypeRef::String)), 3);
    assert_eq!(spec_binop_code(SpecBinOp::Concat), 13);
    assert_eq!(order_code(OrderOp::GreaterThanOrEqual), 3);
    assert_eq!(
        constraint_kind(&Constraint::Predicate("p".to_string(), vec![])),
        10
    );
}

#[test]
fn gate_executor_resolves_winner_calls() {
    let wrap_body = bin(
        IRBinOp::Concat,
        bin(
            IRBinOp::Concat,
            IRNode::Literal(IRLiteral::String("\"".to_string())),
            vr("s"),
        ),
        IRNode::Literal(IRLiteral::String("\"".to_string())),
    );
    let set = hand_set(vec![
        hand_entry("w_wrap", vec![("s", TypeRef::String)], TypeRef::String, wrap_body),
        hand_entry(
            "w_call",
            vec![("t", TypeRef::String)],
            TypeRef::String,
            call("w_wrap", vec![vr("t")]),
        ),
        hand_entry(
            "w_len",
            vec![("s", TypeRef::String)],
            TypeRef::Int,
            call("len", vec![vr("s")]),
        ),
        hand_entry(
            "w_nope",
            vec![("s", TypeRef::String)],
            TypeRef::String,
            call("nosuchfn", vec![vr("s")]),
        ),
    ]);
    // A call to a sibling spec resolves to the CALLEE WINNER.
    assert_eq!(
        set.call("w_call", &[v_text("x")]).unwrap(),
        Value::Str("\"x\"".to_string())
    );
    // Builtins pass resolution untouched and execute.
    assert_eq!(set.call("w_len", &[v_text("abc")]).unwrap(), Value::Int(3));
    // An unknown call is an execution failure, never a guess.
    assert!(set.call("w_nope", &[v_text("x")]).is_err());
    // The census sees the calls for what they are.
    let (count, names) = call_census(&set.get("w_call").unwrap().candidate.body);
    assert_eq!(count, 1);
    assert_eq!(names, vec!["w_wrap".to_string()]);
}

#[test]
fn gate_substitution_capture_refused() {
    // The callee binds `y` as a match pattern variable; the
    // caller's argument mentions `y` — substitution would
    // capture, so resolution refuses (the engine resolver's
    // discipline).
    let callee = hand_entry(
        "w_cap",
        vec![("s", TypeRef::String)],
        TypeRef::String,
        IRNode::Match {
            scrutinee: Box::new(vr("s")),
            cases: vec![
                (IRPattern::Variable("y".to_string()), vr("y")),
                (IRPattern::Wildcard, ls("")),
            ],
        },
    );
    let caller = hand_entry(
        "w_usey",
        vec![("y", TypeRef::String)],
        TypeRef::String,
        call("w_cap", vec![vr("y")]),
    );
    let raw: HashMap<String, CandidateImplementation> = [
        (callee.spec_name.clone(), callee),
        (caller.spec_name.clone(), caller),
    ]
    .into_iter()
    .collect();
    let mut cache = HashMap::new();
    let err = resolve_spec_body("w_usey", &raw, &mut cache, &[])
        .expect_err("capture must be refused");
    assert!(err.contains("binder"), "unexpected refusal: {}", err);
}

#[test]
fn gate_literal_modes_differ_only_by_escaping() {
    let wrap_body = bin(
        IRBinOp::Concat,
        bin(
            IRBinOp::Concat,
            IRNode::Literal(IRLiteral::String("\"".to_string())),
            vr("contents"),
        ),
        IRNode::Literal(IRLiteral::String("\"".to_string())),
    );
    let set = hand_set(vec![hand_entry(
        "rust_string_literal_text",
        vec![("contents", TypeRef::String)],
        TypeRef::String,
        wrap_body,
    )]);
    let a = Composed {
        w: &set,
        escape_literals: false,
    };
    let b = Composed {
        w: &set,
        escape_literals: true,
    };
    let lit = IRLiteral::String("a\"b".to_string());
    // Mode A: the winner wraps the raw contents.
    assert_eq!(a.literal_text(&lit).unwrap(), "\"a\"b\"");
    // Mode B: the mirror layer supplies the escaping; the
    // winner still decides the wrap.
    assert_eq!(b.literal_text(&lit).unwrap(), "\"a\\\"b\"");
    // The spec-Expr StringLit arm is glue in BOTH modes (no
    // spec site pins it): escaped, as the original renders.
    assert_eq!(
        a.sexpr_text(&SpecExpr::StringLit("a\"b".to_string())).unwrap(),
        "\"a\\\"b\""
    );
}

#[test]
fn gate_constraint_order_fallback_is_true() {
    // The assembly's Order arm: a separator for </<=, the
    // constant "true" otherwise (the original's catch-all).
    // The fixed-text table has no Order row — the fallback
    // is assembly glue, and this test pins the glue.
    let sep_body = IRNode::Match {
        scrutinee: Box::new(vr("op")),
        cases: vec![
            (
                IRPattern::Literal(IRLiteral::Int(0)),
                ls(" < "),
            ),
            (
                IRPattern::Literal(IRLiteral::Int(1)),
                ls(" <= "),
            ),
            (IRPattern::Wildcard, ls("")),
        ],
    };
    let fixed_body = IRNode::Match {
        scrutinee: Box::new(vr("kind")),
        cases: vec![
            (
                IRPattern::Literal(IRLiteral::Int(0)),
                ls("true"),
            ),
            (IRPattern::Wildcard, ls("")),
        ],
    };
    let set = hand_set(vec![
        hand_entry(
            "rust_order_separator",
            vec![("op", TypeRef::Int)],
            TypeRef::String,
            sep_body,
        ),
        hand_entry(
            "rust_constraint_fixed_text",
            vec![("kind", TypeRef::Int)],
            TypeRef::String,
            fixed_body,
        ),
    ]);
    let a = Composed {
        w: &set,
        escape_literals: false,
    };
    let vx = SpecExpr::Var("x".to_string());
    assert_eq!(
        a.constraint_text(&Constraint::Order(
            OrderOp::LessThan,
            vx.clone(),
            SpecExpr::IntLit(3),
        ))
        .unwrap(),
        "x < 3"
    );
    assert_eq!(
        a.constraint_text(&Constraint::Order(
            OrderOp::GreaterThan,
            vx.clone(),
            SpecExpr::IntLit(3),
        ))
        .unwrap(),
        "true"
    );
    assert_eq!(
        a.constraint_text(&Constraint::Order(
            OrderOp::GreaterThanOrEqual,
            vx,
            SpecExpr::IntLit(3),
        ))
        .unwrap(),
        "true"
    );
    assert_eq!(a.constraint_text(&Constraint::True).unwrap(), "true");
}

#[test]
fn gate_transcription_spot_checks() {
    // Corpus transcriptions agree with the original emitter
    // on the nose — no synthesis involved.
    let chain = bin(
        IRBinOp::Concat,
        bin(IRBinOp::Concat, ls("a"), vr("x")),
        ls("b"),
    );
    assert_eq!(
        ir_to_rust_expr(&chain),
        "((\"a\".to_string() + &x) + \"b\")"
    );
    assert_eq!(
        ty_to_rust(&TypeRef::Map(
            Box::new(TypeRef::String),
            Box::new(TypeRef::List(Box::new(TypeRef::Bool))),
        )),
        "HashMap<String, Vec<bool>>"
    );
    let bare = make_candidate(
        "answer",
        None,
        vec![],
        vec![("n", TypeRef::Int)],
        TypeRef::Bool,
        vec![],
        lb(true),
        vec![],
    );
    assert_eq!(
        generate(&bare, "rust").unwrap(),
        "pub fn answer(n: i64) -> bool {\n    true\n}\n"
    );
    // The dispatch originals: route 6 is the verbatim error.
    match generate(&bare, "java") {
        Err(e) => assert_eq!(
            format!("{}", e),
            "Unsupported target language: java"
        ),
        Ok(_) => panic!("java must be an unsupported target"),
    }
    let _ = CodegenError::UnsupportedTarget("x".to_string());
}
