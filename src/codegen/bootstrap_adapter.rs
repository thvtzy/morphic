// ─────────────────────────────────────────────────────────────
//  BOOTSTRAP ADAPTER (production) — Phase 51 (M5-S1): the M3
//  gate's composed emitter, transcribed from IR-interpreter
//  calls to DIRECT calls into the compiled synthesized module
//  (super::synthesized_codegen).
//
//  WHAT THIS IS. The M5 audit (ROAD, Phase 50, item B) defines
//  the bootstrap binary's codegen interface: the tree-walking
//  layer of the M3 gate's `Composed` emitter (m3_gate.rs) plus
//  its kind-code encoders, structurally identical, with every
//  `WinnerSet::call_*` (an IR-interpreter execution of a
//  winner body) replaced by a direct call to the artifact's
//  compiled winner function of the same name, reached through
//  its `mod` wrapper so the shipped artifact stays
//  byte-identical to the gate-judged one. Wherever the gate's
//  composition applies a MIRROR COMPLETION, this adapter
//  applies the same completion as an ordinary Rust helper —
//  the audit's 9-item shim inventory, complete:
//    (1) `escape_rust_str` on string-literal contents at the
//        literal call sites (the F5 completion; the adapter
//        is the mirror-completed composition, mode B — there
//        is no mode A here, the strict composition is a
//        measurement device of the gate, not a shipped form);
//    (2) `escape_rust_char` for Char literals (the original's
//        own helper, super::escape_rust_char);
//    (3) the ", " joins at every list-rendering site;
//    (4) numeric Display — Int/Bool pattern literals, Int /
//        Float IR literals, Int/Bool spec expressions (the
//        F2/F3 items, via Rust's own to_string()/Display,
//        exactly as the mirrors do);
//    (5) `rust_ident` sanitization (super::rust_ident);
//    (6) pad repetition, "    ".repeat(indent);
//    (7) the indexed test-name default, format!("test_{}", i)
//        for unnamed tests (rust_test_name_form pins the
//        named/default CHOICE; the default's text is F2);
//    (8) the constraint catch-all's constant "true" (the
//        Phase 42 glue correction);
//    (9) the scrutinee helper predicates
//        pattern_has_string_literal / is_bare_string_literal.
//  The recursion itself — the tree walks below — is assembly
//  glue by the M3 definition, not winner business.
//
//  STATUS. Since M5-S2 (Phase 52) `generate`'s "rust" route
//  dispatches here when the Cargo feature `bootstrap-codegen`
//  is enabled (see `generate_rust_route` in super): the
//  feature-on build is the bootstrap binary. With the feature
//  off (the default), the original `generate_rust` renders and
//  nothing in production calls this module. The equivalence
//  claim is proven in src/codegen/m5_s1.rs: the compiled
//  adapter is byte-identical to the original emitter — and to
//  the interpreter composition it transcribes — over the M3
//  gate corpus and the M4 whole-file corpus; the switch and
//  the roundtrip are judged in src/codegen/m5_s2.rs.
// ─────────────────────────────────────────────────────────────

use super::{escape_rust_char, escape_rust_str, rust_ident, synthesized_codegen as syn, CodegenError};
use crate::spec::ast::{
    BinOp as SpecBinOp, Constraint, Expr as SpecExpr, OrderOp, TypeRef,
};
use crate::synthesis::engine::{
    CandidateImplementation, CollectionKind, IRBinOp, IRLiteral, IRNode, IRPattern,
    IRUnaryOp,
};

// ── The production entry points ───────────────────────────

/// The bootstrap codegen's whole-file Rust render of one
/// winner: the compiled adapter's driver, in
/// `codegen::generate`'s "rust" seat. Errors from the walk
/// (the tail-form shape errors the composition can raise)
/// map to CodegenError at this boundary, as the audit
/// specifies.
pub(crate) fn render_rust_synthesized(
    candidate: &CandidateImplementation,
) -> Result<String, CodegenError> {
    Adapter
        .driver_file(candidate)
        .map_err(CodegenError::InternalError)
}

/// `generate`'s dispatch shape over the synthesized module:
/// the compiled route/form winners decide Ok vs unsupported
/// (an unsupported target's CodegenError Display text is the
/// winner's text — the M3 gate's dispatch battery proves the
/// two byte-equal); the "rust" route renders through the
/// compiled adapter; the remaining Ok routes (c / wasm /
/// python / javascript / ir) have no synthesized counterpart
/// (M5 audit, item A) and stay with the original emitter,
/// exactly as they will with the M5-S2 switch in place.
pub(crate) fn generate_synthesized(
    candidate: &CandidateImplementation,
    target: &str,
) -> Result<String, CodegenError> {
    match Adapter.dispatch(target) {
        Err(_) => Err(CodegenError::UnsupportedTarget(target.into())),
        Ok(()) if target == "rust" => render_rust_synthesized(candidate),
        Ok(()) => super::generate(candidate, target),
    }
}

// ── Kind codes (declaration order, as the specs pin them) ──
//
// Transcribed verbatim from the M3 gate harness
// (m3_gate.rs:133–320): assembly glue shared by every
// composition of the winners.

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
        // M1 features (Phase 73): appended at the end, never
        // renumbered (audit section 4). The Codegen specs
        // gain the 28/29 rows in Phase 78; until then the
        // composed emitter's classifications are total over
        // these codes by their standing default arms.
        FieldAccess { .. } => 28,
        RecordLit { .. } => 29,
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

// ── The compiled emitter ──────────────────────────────────
//
// The tests/m3_proof.rs assemblies as the M3 gate's `Composed`
// transcribed them, with every winner execution replaced by a
// direct call to the compiled winner of the same name. The
// composition is the mirror-completed one (mode B): the
// contents handed to the string-literal winners are always
// the escaped contents — shim (1).

pub(crate) struct Adapter;

impl Adapter {
    /// The contents a string literal's winner sees: always
    /// the mirror-pinned escaped contents (shim 1).
    fn literal_contents(&self, contents: &str) -> String {
        escape_rust_str(contents)
    }

    // ── Types (ty_to_rust_assembled) ──

    pub(crate) fn ty_text(&self, ty: &TypeRef) -> Result<String, String> {
        let tag = type_code(ty);
        Ok(match ty {
            TypeRef::Generic(name) | TypeRef::Named(name) => rust_ident(name),
            TypeRef::List(inner)
            | TypeRef::Set(inner)
            | TypeRef::Option(inner)
            | TypeRef::Ref(inner)
            | TypeRef::Stream(inner) => format!(
                "{}{}{}",
                syn::rust_type_prefix::rust_type_prefix(tag),
                self.ty_text(inner)?,
                syn::rust_type_suffix::rust_type_suffix(tag),
            ),
            TypeRef::Map(first, second) | TypeRef::Result(first, second) => format!(
                "{}{}{}{}{}",
                syn::rust_type_prefix::rust_type_prefix(tag),
                self.ty_text(first)?,
                syn::rust_type_separator::rust_type_separator(tag),
                self.ty_text(second)?,
                syn::rust_type_suffix::rust_type_suffix(tag),
            ),
            TypeRef::Tuple(parts) => {
                let rendered: Result<Vec<String>, String> =
                    parts.iter().map(|p| self.ty_text(p)).collect();
                format!(
                    "{}{}{}",
                    syn::rust_type_prefix::rust_type_prefix(21),
                    rendered?.join(", "),
                    syn::rust_type_suffix::rust_type_suffix(21),
                )
            }
            TypeRef::Array(inner, size) => format!(
                "{}{}{}{}{}",
                syn::rust_type_prefix::rust_type_prefix(22),
                self.ty_text(inner)?,
                syn::rust_type_separator::rust_type_separator(22),
                size,
                syn::rust_type_suffix::rust_type_suffix(22),
            ),
            TypeRef::Function(args, ret) => {
                let rendered: Result<Vec<String>, String> =
                    args.iter().map(|a| self.ty_text(a)).collect();
                format!(
                    "{}{}{}{}{}",
                    syn::rust_type_prefix::rust_type_prefix(23),
                    rendered?.join(", "),
                    syn::rust_type_midfix::rust_type_midfix(23),
                    self.ty_text(ret)?,
                    syn::rust_type_suffix::rust_type_suffix(23),
                )
            }
            // Scalar leaves: the slice-1 name table.
            _ => syn::rust_type_name::rust_type_name(tag),
        })
    }

    // ── Patterns (ir_pattern_assembled) ──

    pub(crate) fn pattern_text(&self, pat: &IRPattern) -> Result<String, String> {
        Ok(match pat {
            IRPattern::Wildcard => syn::rust_pattern_fixed_text::rust_pattern_fixed_text(0),
            IRPattern::Variable(name) => rust_ident(name),
            IRPattern::Literal(lit) => self.pattern_literal_text(lit)?,
            IRPattern::Constructor(name, subs) => {
                if subs.is_empty() {
                    // The branch rust_pattern_ctor_name pins:
                    // the winner receives the rendered name.
                    syn::rust_pattern_ctor_name::rust_pattern_ctor_name(
                        rust_ident(name),
                        false,
                    )
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
            IRLiteral::String(s) => syn::rust_string_literal_text::rust_string_literal_text(
                self.literal_contents(s),
            ),
            // Phase 124: Char literal patterns render
            // literally — the escaped character inside
            // single quotes, the expression position's
            // own glue (as in literal_text below),
            // retiring the F4 widening. Float / Unit
            // widen to the wildcard text the
            // rust_pattern_literal_text table pins.
            IRLiteral::Char(c) => format!("'{}'", escape_rust_char(*c)),
            other => syn::rust_pattern_literal_text::rust_pattern_literal_text(lit_kind(other)),
        })
    }

    // ── Spec expressions (expr_to_rust_assembled) ──

    pub(crate) fn sexpr_text(&self, expr: &SpecExpr) -> Result<String, String> {
        Ok(match expr {
            SpecExpr::Var(name) => rust_ident(name),
            SpecExpr::IntLit(n) => n.to_string(),
            SpecExpr::BoolLit(b) => b.to_string(),
            // No spec site pins expr_to_rust's StringLit arm;
            // the original escapes (a Phase 29 site) and the
            // assembly's job is to compose as the original
            // composes — the escaping here is glue, as in the
            // original (shim 1's second site).
            SpecExpr::StringLit(s) => format!("\"{}\"", escape_rust_str(s)),
            // The original's CharLit arm (Phase 45): the
            // escaped character inside single quotes — the
            // same escape_rust_char the IR Char arm uses.
            SpecExpr::CharLit(c) => format!("'{}'", escape_rust_char(*c)),
            SpecExpr::Call(name, args) => {
                let rendered: Result<Vec<String>, String> =
                    args.iter().map(|a| self.sexpr_text(a)).collect();
                let joined = rendered?.join(", ");
                // The pseudo-call dispatch (rust_expr_call_form,
                // Phase 55): 'List' renders as the vec literal,
                // '__tuple__' as a Rust tuple, 'None' as the
                // bare constructor. The classification and the
                // two templates are the artifact winners'
                // (Phase 56 regeneration landed their modules);
                // the bare constructor (form 3) is the name
                // itself, as in the assembly.
                match syn::rust_expr_call_form::rust_expr_call_form(name.clone()) {
                    1 => syn::rust_expr_list_text::rust_expr_list_text(joined),
                    2 => syn::rust_expr_tuple_text::rust_expr_tuple_text(joined),
                    3 => "None".to_string(),
                    _ => format!("{}({})", rust_ident(name), joined),
                }
            }
            SpecExpr::BinOp(op, lhs, rhs) => format!(
                "({} {} {})",
                self.sexpr_text(lhs)?,
                // The spec_binop_symbol winner (its table was
                // amended in Phase 55; the Phase 56 artifact
                // carries the amended winner).
                syn::spec_binop_symbol::spec_binop_symbol(spec_binop_code(*op)),
                self.sexpr_text(rhs)?,
            ),
            // The closure template (rust_expr_lambda_text,
            // Phase 55): parameter names only, the body
            // rendered by this same walker; composed through
            // the artifact winner (Phase 56).
            SpecExpr::Lambda(params, body) => {
                let names: Vec<String> =
                    params.iter().map(|(n, _)| rust_ident(n)).collect();
                syn::rust_expr_lambda_text::rust_expr_lambda_text(
                    names.join(", "),
                    self.sexpr_text(body)?,
                )
            }
            // Field projection (Phase 78): the template
            // winner renders the dot; the field name passes
            // through the same rust_ident glue the Var arm
            // uses, exactly as the original's Field arm.
            SpecExpr::Field(base, field) => {
                syn::rust_expr_field_text::rust_expr_field_text(
                    self.sexpr_text(base)?,
                    rust_ident(field),
                )
            }
            // Method calls (Phase 78, decision D1a): the
            // dispatch winner classifies the EFFECTIVE name
            // — the walker folds the argument count into
            // the key (a call carrying arguments passes the
            // empty name, itself off-table), exactly the
            // fold the spec's doc records and the original's
            // args.is_empty() guard makes. The closed table
            // renders through the method template winner on
            // the rendered receiver; everything else takes
            // the fixed-text table's kind-13 row.
            SpecExpr::MethodCall {
                receiver,
                name,
                args,
            } => {
                let key = if args.is_empty() { name.clone() } else { String::new() };
                match syn::rust_expr_method_form::rust_expr_method_form(key) {
                    0 => syn::rust_expr_method_text::rust_expr_method_text(
                        self.sexpr_text(receiver)?,
                        name.clone(),
                    ),
                    _ => syn::rust_expr_fixed_text::rust_expr_fixed_text(13),
                }
            }
            // Record literals (Phase 78, decision D3a):
            // fields in written order; a bare string-literal
            // field value takes the owned form through its
            // winner (the defect-4d rule, as the original's
            // arm applies it); the frame is the record
            // template winner's, the type name through the
            // rust_ident glue as in the original.
            SpecExpr::RecordLit { type_name, fields } => {
                let rendered: Result<Vec<String>, String> = fields
                    .iter()
                    .map(|(n, v)| {
                        let value_text = match v {
                            SpecExpr::StringLit(s) => {
                                syn::rust_string_literal_owned_text::rust_string_literal_owned_text(
                                    self.literal_contents(s),
                                )
                            }
                            _ => self.sexpr_text(v)?,
                        };
                        Ok(format!("{}: {}", rust_ident(n), value_text))
                    })
                    .collect();
                syn::rust_expr_record_text::rust_expr_record_text(
                    rust_ident(type_name),
                    rendered?.join(", "),
                )
            }
            // Every kind without an arm: the fixed text the
            // rust_expr_fixed_text table pins ("??").
            other => {
                let kind = match other {
                    SpecExpr::FloatLit(_) => 2,
                    SpecExpr::Index(..) => 6,
                    SpecExpr::UnaryOp(..) => 9,
                    SpecExpr::Length(_) => 11,
                    SpecExpr::Comprehension { .. } => 12,
                    _ => unreachable!(),
                };
                syn::rust_expr_fixed_text::rust_expr_fixed_text(kind)
            }
        })
    }

    // ── Constraints (constraint_to_rust_assembled) ──

    pub(crate) fn constraint_text(&self, cons: &Constraint) -> Result<String, String> {
        Ok(match cons {
            // Phase 124 (NF-116-2, global): the Expr and
            // Predicate forms render genuinely, through
            // the assert-position walker below — as the
            // original now renders them.
            Constraint::Expr(expr) => self.sexpr_assert_text(expr)?,
            Constraint::Predicate(name, args) => self.sexpr_assert_call(name, args)?,
            Constraint::Not(inner) => format!("!({})", self.constraint_text(inner)?),
            Constraint::Eq(a, b) => {
                format!(
                    "{} == {}",
                    self.sexpr_assert_text(a)?,
                    self.sexpr_assert_text(b)?
                )
            }
            Constraint::Order(op, a, b) => {
                let sep = syn::rust_order_separator::rust_order_separator(order_code(*op));
                if sep.is_empty() {
                    // The assembly's None branch (m3_proof.rs
                    // constraint_to_rust_assembled): orders
                    // with no separator render the constant
                    // "true" — assembly glue, exactly as the
                    // assembly writes it (shim 8). Since the
                    // Phase 124 amendment every OrderOp
                    // carries a separator, so this branch is
                    // the table's silence, never a rendering.
                    "true".to_string()
                } else {
                    format!(
                        "{}{}{}",
                        self.sexpr_assert_text(a)?,
                        sep,
                        self.sexpr_assert_text(b)?
                    )
                }
            }
            Constraint::And(parts) => {
                let rendered: Result<Vec<String>, String> =
                    parts.iter().map(|p| self.constraint_text(p)).collect();
                format!("({})", rendered?.join(" && "))
            }
            Constraint::Or(parts) => {
                let rendered: Result<Vec<String>, String> =
                    parts.iter().map(|p| self.constraint_text(p)).collect();
                format!("({})", rendered?.join(" || "))
            }
            Constraint::Implies(a, b) => format!(
                "(!({}) || ({}))",
                self.constraint_text(a)?,
                self.constraint_text(b)?
            ),
            // True and the two quantified kinds: the
            // fixed text the rust_constraint_fixed_text
            // table pins (the quantified pair is the
            // named fallback — no value-level rendering
            // of a quantified proposition exists).
            _ => syn::rust_constraint_fixed_text::rust_constraint_fixed_text(
                constraint_kind(cons),
            ),
        })
    }

    /// Assert-position spec expressions (Phase 124):
    /// the constraint walker's operand rendering — the
    /// original's assert_expr_to_rust, composed as the
    /// original composes it: calls and binary operations
    /// recurse through the assert walker (the clone rule
    /// reaches calls nested in a proposition); the
    /// pseudo-calls keep their forms over assert-rendered
    /// operands; every other form takes sexpr_text's
    /// rendering.
    pub(crate) fn sexpr_assert_text(&self, expr: &SpecExpr) -> Result<String, String> {
        Ok(match expr {
            SpecExpr::Call(name, args) => {
                match syn::rust_expr_call_form::rust_expr_call_form(name.clone()) {
                    1 => {
                        let rendered: Result<Vec<String>, String> =
                            args.iter().map(|a| self.sexpr_assert_text(a)).collect();
                        syn::rust_expr_list_text::rust_expr_list_text(rendered?.join(", "))
                    }
                    2 => {
                        let rendered: Result<Vec<String>, String> =
                            args.iter().map(|a| self.sexpr_assert_text(a)).collect();
                        syn::rust_expr_tuple_text::rust_expr_tuple_text(rendered?.join(", "))
                    }
                    3 => "None".to_string(),
                    _ => self.sexpr_assert_call(name, args)?,
                }
            }
            SpecExpr::BinOp(op, lhs, rhs) => format!(
                "({} {} {})",
                self.sexpr_assert_text(lhs)?,
                syn::spec_binop_symbol::spec_binop_symbol(spec_binop_code(*op)),
                self.sexpr_assert_text(rhs)?,
            ),
            _ => self.sexpr_text(expr)?,
        })
    }

    /// A call at assert position: a plain-variable
    /// argument renders cloned (the clone rule — the
    /// proposition must not consume the function's
    /// argument); every other argument renders through
    /// the assert walker.
    fn sexpr_assert_call(
        &self,
        name: &str,
        args: &[SpecExpr],
    ) -> Result<String, String> {
        let rendered: Result<Vec<String>, String> = args
            .iter()
            .map(|arg| match arg {
                SpecExpr::Var(v) => Ok(format!("{}.clone()", rust_ident(v))),
                _ => self.sexpr_assert_text(arg),
            })
            .collect();
        Ok(format!("{}({})", rust_ident(name), rendered?.join(", ")))
    }

    // ── Expressions (ir_expr_assembled) ──

    fn literal_text(&self, lit: &IRLiteral) -> Result<String, String> {
        Ok(match lit {
            IRLiteral::Int(n) => n.to_string(),
            IRLiteral::Float(f) => f.to_string(),
            IRLiteral::Bool(b) => syn::rust_literal_bool::rust_literal_bool(*b),
            IRLiteral::String(s) => syn::rust_string_literal_text::rust_string_literal_text(
                self.literal_contents(s),
            ),
            IRLiteral::Char(c) => format!("'{}'", escape_rust_char(*c)),
            IRLiteral::Unit => syn::rust_literal_fixed_text::rust_literal_fixed_text(5),
        })
    }

    /// Assembly of concat_owned_operand: a direct String
    /// literal takes the owned literal template; a direct Var
    /// takes the clone text (glue, as in the assembly — the
    /// Phase 39 treatment); every other node renders as usual.
    fn concat_owned_text(&self, node: &IRNode) -> Result<String, String> {
        Ok(match node {
            IRNode::Literal(IRLiteral::String(s)) => {
                syn::rust_string_literal_owned_text::rust_string_literal_owned_text(
                    self.literal_contents(s),
                )
            }
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
        let form = syn::rust_concat_right_form::rust_concat_right_form(kind, lit);
        Ok(syn::rust_concat_right_text::rust_concat_right_text(form, rendered))
    }

    /// Assembly of match_scrutinee_to_rust, composed through
    /// the slice-4 scrutinee winners.
    fn scrutinee_text(
        &self,
        scrutinee: &IRNode,
        cases: &[(IRPattern, IRNode)],
    ) -> Result<String, String> {
        let base = syn::rust_condition_text::rust_condition_text(
            node_kind(scrutinee),
            self.expr_text(scrutinee)?,
        );
        let has_string = cases.iter().any(|(p, _)| pattern_has_string_literal(p));
        let form = syn::rust_match_scrutinee_form::rust_match_scrutinee_form(
            has_string,
            is_bare_string_literal(scrutinee),
        );
        Ok(syn::rust_match_scrutinee_text::rust_match_scrutinee_text(form, base))
    }

    fn condition_text(&self, node: &IRNode) -> Result<String, String> {
        Ok(syn::rust_condition_text::rust_condition_text(
            node_kind(node),
            self.expr_text(node)?,
        ))
    }

    /// Assembly of ir_call_text (src/codegen/mod.rs),
    /// composed through the Phase 85 winners exactly as
    /// the emitter decides then renders — the gate
    /// assembly's helper, transcribed to direct calls.
    /// Family 4 (Phase 102: the 'int_to_str' builtin)
    /// renders through the synthesized
    /// rust_call_int_to_str_text winner over the single
    /// argument. Shared by the expression and statement
    /// walkers.
    fn call_dispatched_text(
        &self,
        function: &str,
        args: &[IRNode],
    ) -> Result<String, String> {
        let eff_name: &str = match function {
            "None" if !args.is_empty() => "",
            "len" if args.len() != 1 => "",
            "int_to_str" if args.len() != 1 => "",
            other => other,
        };
        let form = syn::rust_call_form::rust_call_form(eff_name.to_string());
        Ok(match form {
            1 => {
                let rendered: Result<Vec<String>, String> =
                    args.iter().map(|a| self.expr_text(a)).collect();
                syn::rust_expr_list_text::rust_expr_list_text(rendered?.join(", "))
            }
            2 => rust_ident(function),
            4 => syn::rust_call_int_to_str_text::rust_call_int_to_str_text(
                self.expr_text(&args[0])?,
            ),
            3 => {
                let argform: i64 = match &args[0] {
                    IRNode::FieldAccess { .. } => 1,
                    IRNode::Literal(IRLiteral::String(_)) => 1,
                    _ => 0,
                };
                let arg_text = self.expr_text(&args[0])?;
                let len_form = syn::rust_call_len_form::rust_call_len_form(argform);
                if len_form == 1 {
                    syn::rust_call_len_char_text::rust_call_len_char_text(arg_text)
                } else {
                    syn::rust_call_len_text::rust_call_len_text(arg_text)
                }
            }
            _ => {
                let rendered: Result<Vec<String>, String> =
                    args.iter().map(|a| self.expr_text(a)).collect();
                syn::rust_call_text::rust_call_text(
                    rust_ident(function),
                    rendered?.join(", "),
                )
            }
        })
    }

    pub(crate) fn expr_text(&self, node: &IRNode) -> Result<String, String> {
        Ok(match node {
            IRNode::Literal(lit) => self.literal_text(lit)?,
            IRNode::Var(name) => rust_ident(name),
            IRNode::BinOp(op, l, r) => {
                let form = syn::rust_expr_binop_form::rust_expr_binop_form(binop_code(*op));
                match form {
                    1 => syn::rust_binop_concat_text::rust_binop_concat_text(
                        self.concat_owned_text(l)?,
                        self.concat_borrowed_text(r)?,
                    ),
                    2 => syn::rust_binop_append_text::rust_binop_append_text(
                        self.expr_text(l)?,
                        self.expr_text(r)?,
                    ),
                    3 => syn::rust_binop_merge_text::rust_binop_merge_text(
                        self.expr_text(l)?,
                        self.expr_text(r)?,
                    ),
                    _ => syn::rust_binop_infix_text::rust_binop_infix_text(
                        binop_code(*op),
                        self.expr_text(l)?,
                        self.expr_text(r)?,
                    ),
                }
            }
            IRNode::UnaryOp(op, inner) => syn::rust_unary_text::rust_unary_text(
                unary_code(*op),
                self.expr_text(inner)?,
            ),
            IRNode::Call { function, args } => self.call_dispatched_text(function, args)?,
            IRNode::If {
                condition,
                then_branch,
                else_branch,
            } => syn::rust_if_text::rust_if_text(
                self.condition_text(condition)?,
                self.expr_text(then_branch)?,
                self.expr_text(else_branch)?,
            ),
            IRNode::Lambda { params, body } | IRNode::Closure { params, body, .. } => {
                // One arm in the original (Phase 30): Closure
                // captures are not rendered, parameter types
                // are dropped.
                let names: Vec<String> = params.iter().map(|(n, _)| rust_ident(n)).collect();
                syn::rust_lambda_text::rust_lambda_text(
                    names.join(", "),
                    self.expr_text(body)?,
                )
            }
            IRNode::Collection { kind, elements } => {
                let rendered: Result<Vec<String>, String> =
                    elements.iter().map(|e| self.expr_text(e)).collect();
                format!(
                    "{}{}{}",
                    syn::rust_collection_prefix::rust_collection_prefix(
                        collection_code(*kind)
                    ),
                    rendered?.join(", "),
                    syn::rust_collection_suffix::rust_collection_suffix(
                        collection_code(*kind)
                    ),
                )
            }
            IRNode::Map { function, over } => syn::rust_map_text::rust_map_text(
                self.expr_text(over)?,
                self.expr_text(function)?,
            ),
            IRNode::Filter { predicate, over } => syn::rust_filter_text::rust_filter_text(
                self.expr_text(over)?,
                self.expr_text(predicate)?,
            ),
            IRNode::Fold {
                operation,
                initial,
                over,
            } => syn::rust_fold_text::rust_fold_text(
                self.expr_text(over)?,
                self.expr_text(initial)?,
                self.expr_text(operation)?,
            ),
            IRNode::Sort {
                comparator,
                collection,
            } => syn::rust_sort_text::rust_sort_text(
                self.expr_text(collection)?,
                self.expr_text(comparator)?,
            ),
            IRNode::Match { scrutinee, cases } => {
                let arms: Result<Vec<String>, String> = cases
                    .iter()
                    .map(|(p, b)| {
                        Ok(syn::rust_match_arm_text::rust_match_arm_text(
                            self.pattern_text(p)?,
                            self.expr_text(b)?,
                        ))
                    })
                    .collect();
                syn::rust_match_text::rust_match_text(
                    self.scrutinee_text(scrutinee, cases)?,
                    arms?.join(", "),
                )
            }
            IRNode::Block(stmts) => {
                // The original's Block arm: statements render
                // as statements in a neutral context, the last
                // keeps the tail position; the whole is the
                // rust_block_text template over the inner text.
                let inner = self.block_inner_text(stmts)?;
                syn::rust_block_text::rust_block_text(inner.trim_end().to_string())
            }
            IRNode::Let { .. } | IRNode::While { .. } | IRNode::For { .. } => {
                // expr_block_stmt: the statement rendering
                // inside the rust_block_text template.
                let inner = self.stmt_text(node, 0, &TypeRef::Unit)?;
                syn::rust_block_text::rust_block_text(inner.trim_end().to_string())
            }
            IRNode::Loop { body, .. } => {
                let inner = self.stmt_text(body, 0, &TypeRef::Unit)?;
                syn::rust_loop_text::rust_loop_text(inner.trim_end().to_string())
            }
            IRNode::StructDef { name, fields, body } => {
                let rendered: Result<Vec<String>, String> = fields
                    .iter()
                    .map(|(n, t)| {
                        Ok(syn::rust_param_text::rust_param_text(
                            rust_ident(n),
                            self.ty_text(t)?,
                        ))
                    })
                    .collect();
                syn::rust_struct_def_text::rust_struct_def_text(
                    rust_ident(name),
                    rendered?.join(", "),
                    self.expr_text(body)?,
                )
            }
            IRNode::Index(coll, idx) => {
                // Phase 85 dispatch (rust_index_form),
                // exactly the gate assembly's decision.
                let (kind, lit) = match coll.as_ref() {
                    IRNode::Literal(l) => (9, lit_kind(l)),
                    _ => (node_kind(coll), 0),
                };
                let form = syn::rust_index_form::rust_index_form(kind, lit);
                let coll_text = self.expr_text(coll)?;
                let idx_text = self.expr_text(idx)?;
                if form == 1 {
                    syn::rust_index_char_text::rust_index_char_text(coll_text, idx_text)
                } else {
                    syn::rust_index_text::rust_index_text(coll_text, idx_text)
                }
            }
            IRNode::TryPropagate(inner) => {
                syn::rust_try_text::rust_try_text(self.expr_text(inner)?)
            }
            IRNode::Assign { target, value } => syn::rust_assign_text::rust_assign_text(
                self.expr_text(target)?,
                self.expr_text(value)?,
            ),
            IRNode::Return(inner) => {
                syn::rust_return_text::rust_return_text(self.expr_text(inner)?)
            }
            IRNode::Break { value } => match value {
                Some(inner) => {
                    syn::rust_break_text::rust_break_text(true, self.expr_text(inner)?)
                }
                None => syn::rust_break_text::rust_break_text(false, String::new()),
            },
            IRNode::Hole { .. } => syn::rust_hole_text::rust_hole_text(0),
            IRNode::Alloc { .. } => {
                syn::rust_unsupported_text::rust_unsupported_text("Alloc".to_string())
            }
            // M1 features (Phase 78, decisions D2a/D3a):
            // field projection renders through its template
            // winner — the base through the condition
            // position, a record-literal base parenthesized
            // first, exactly the original's discipline; the
            // field name through the rust_ident glue.
            IRNode::FieldAccess { base, field } => {
                let base_text = match base.as_ref() {
                    IRNode::RecordLit { .. } => {
                        format!("({})", self.expr_text(base)?)
                    }
                    _ => self.condition_text(base)?,
                };
                syn::rust_field_access_text::rust_field_access_text(
                    base_text,
                    rust_ident(field),
                )
            }
            // Record construction: fields in written order,
            // a bare string-literal field value owned
            // through its winner (the defect-4d rule), the
            // frame the record template winner's.
            IRNode::RecordLit { type_name, fields } => {
                let rendered: Result<Vec<String>, String> = fields
                    .iter()
                    .map(|(n, v)| {
                        let value_text = match v {
                            IRNode::Literal(IRLiteral::String(s)) => {
                                syn::rust_string_literal_owned_text::rust_string_literal_owned_text(
                                    self.literal_contents(s),
                                )
                            }
                            // Phase 85: a FieldAccess value
                            // takes the clone form through
                            // rust_unary_text's Clone family.
                            IRNode::FieldAccess { .. } => {
                                syn::rust_unary_text::rust_unary_text(9, self.expr_text(v)?)
                            }
                            _ => self.expr_text(v)?,
                        };
                        Ok(format!("{}: {}", rust_ident(n), value_text))
                    })
                    .collect();
                syn::rust_record_lit_text::rust_record_lit_text(
                    rust_ident(type_name),
                    rendered?.join(", "),
                )
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
                let owned = syn::rust_string_ownership::rust_string_ownership(
                    3,
                    expected_code(expected),
                );
                if owned {
                    syn::rust_string_literal_owned_text::rust_string_literal_owned_text(
                        self.literal_contents(s),
                    )
                } else {
                    self.expr_text(node)?
                }
            }
            _ => self.expr_text(node)?,
        })
    }

    // ── Statements (ir_stmt_assembled) ──

    pub(crate) fn stmt_text(
        &self,
        node: &IRNode,
        indent: usize,
        fn_ret: &TypeRef,
    ) -> Result<String, String> {
        let pad = "    ".repeat(indent);
        let pad_inner = "    ".repeat(indent + 1);
        Ok(match node {
            IRNode::Hole { .. } => syn::rust_stmt_hole_text::rust_stmt_hole_text(pad),
            IRNode::Block(stmts) => {
                let parts: Result<Vec<String>, String> = stmts
                    .iter()
                    .map(|s| self.stmt_text(s, indent, fn_ret))
                    .collect();
                parts?.concat()
            }
            IRNode::Let { name, value, body } => {
                syn::rust_stmt_let_text::rust_stmt_let_text(
                    pad,
                    rust_ident(name),
                    self.expr_text(value)?,
                    self.tail_text(body, indent, fn_ret, Some(fn_ret))?,
                )
            }
            IRNode::If {
                condition,
                then_branch,
                else_branch,
            } => syn::rust_stmt_if_text::rust_stmt_if_text(
                pad,
                self.condition_text(condition)?,
                self.tail_text(then_branch, indent + 1, fn_ret, Some(fn_ret))?,
                self.tail_text(else_branch, indent + 1, fn_ret, Some(fn_ret))?,
            ),
            IRNode::While {
                condition,
                body,
                invariant,
            } => syn::rust_stmt_while_text::rust_stmt_while_text(
                invariant.is_some(),
                invariant.as_deref().unwrap_or("").to_string(),
                pad,
                self.condition_text(condition)?,
                self.stmt_text(body, indent + 1, fn_ret)?,
            ),
            IRNode::For {
                var,
                start,
                end,
                body,
            } => syn::rust_stmt_for_text::rust_stmt_for_text(
                pad,
                rust_ident(var),
                self.expr_text(start)?,
                self.expr_text(end)?,
                self.stmt_text(body, indent + 1, fn_ret)?,
            ),
            IRNode::Match { scrutinee, cases } => {
                let mut out = syn::rust_stmt_match_head_text::rust_stmt_match_head_text(
                    pad.clone(),
                    self.stmt_scrutinee_text(scrutinee, cases)?,
                );
                for (pat, body) in cases {
                    out.push_str(
                        &syn::rust_stmt_match_arm_text::rust_stmt_match_arm_text(
                            pad_inner.clone(),
                            self.pattern_text(pat)?,
                            self.tail_text(body, indent + 2, fn_ret, Some(fn_ret))?,
                        ),
                    );
                }
                out.push_str(&syn::rust_stmt_match_close_text::rust_stmt_match_close_text(
                    pad,
                ));
                out
            }
            IRNode::Call { function, args } => {
                // Phase 85: family 0 keeps the standing
                // statement template; the dispatched
                // families terminate through
                // rust_stmt_expr_text — the gate
                // assembly's composition exactly.
                let eff_name: &str = match function.as_str() {
                    "None" if !args.is_empty() => "",
                    "len" if args.len() != 1 => "",
                    "int_to_str" if args.len() != 1 => "",
                    other => other,
                };
                let form = syn::rust_call_form::rust_call_form(eff_name.to_string());
                if form == 0 {
                    let rendered: Result<Vec<String>, String> =
                        args.iter().map(|a| self.expr_text(a)).collect();
                    syn::rust_stmt_call_text::rust_stmt_call_text(
                        pad,
                        rust_ident(function),
                        rendered?.join(", "),
                    )
                } else {
                    let rendered = self.call_dispatched_text(function, args)?;
                    syn::rust_stmt_expr_text::rust_stmt_expr_text(pad, rendered)
                }
            }
            IRNode::Return(inner) => syn::rust_stmt_return_text::rust_stmt_return_text(
                pad,
                self.expr_typed_text(inner, Some(fn_ret))?,
            ),
            IRNode::Alloc { name, ty, initial } => {
                syn::rust_stmt_alloc_text::rust_stmt_alloc_text(
                    pad,
                    rust_ident(name),
                    self.ty_text(ty)?,
                    self.expr_typed_text(initial, Some(ty))?,
                )
            }
            IRNode::Assign { target, value } => {
                syn::rust_stmt_assign_text::rust_stmt_assign_text(
                    pad,
                    self.expr_text(target)?,
                    self.expr_text(value)?,
                )
            }
            IRNode::Collection { kind, elements } => {
                let rendered: Result<Vec<String>, String> =
                    elements.iter().map(|e| self.expr_text(e)).collect();
                syn::rust_stmt_collection_text::rust_stmt_collection_text(
                    collection_code(*kind),
                    pad,
                    rendered?.join(", "),
                )
            }
            IRNode::Map { function, over } => syn::rust_stmt_map_text::rust_stmt_map_text(
                pad,
                self.expr_text(over)?,
                self.expr_text(function)?,
            ),
            IRNode::Filter { predicate, over } => {
                syn::rust_stmt_filter_text::rust_stmt_filter_text(
                    pad,
                    self.expr_text(over)?,
                    self.expr_text(predicate)?,
                )
            }
            IRNode::Fold {
                operation,
                initial,
                over,
            } => syn::rust_stmt_fold_text::rust_stmt_fold_text(
                pad,
                self.expr_text(over)?,
                self.expr_text(initial)?,
                self.expr_text(operation)?,
            ),
            IRNode::Sort {
                comparator,
                collection,
            } => syn::rust_stmt_sort_text::rust_stmt_sort_text(
                pad,
                self.expr_text(collection)?,
                self.expr_text(comparator)?,
            ),
            IRNode::Typed { node, .. } => self.stmt_text(node, indent, fn_ret)?,
            // Expressions that shouldn't appear as statements:
            // the terminated fallback.
            _ => syn::rust_stmt_expr_text::rust_stmt_expr_text(
                pad,
                self.expr_text(node)?,
            ),
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

    pub(crate) fn tail_text(
        &self,
        node: &IRNode,
        indent: usize,
        fn_ret: &TypeRef,
        expected: Option<&TypeRef>,
    ) -> Result<String, String> {
        let form = syn::rust_tail_form::rust_tail_form(node_kind(node));
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
                        syn::rust_tail_block_text::rust_tail_block_text(
                            rest_text?.concat(),
                            self.tail_text(last, indent, fn_ret, expected)?,
                        )
                    }
                }
            }
            3 => {
                let IRNode::Typed { node, ty } = node else {
                    return Err("tail form 3 on a non-Typed node".to_string());
                };
                self.tail_text(node, indent, fn_ret, Some(ty))?
            }
            1 => syn::rust_tail_expr_text::rust_tail_expr_text(
                "    ".repeat(indent),
                self.expr_typed_text(node, expected)?,
            ),
            _ => self.stmt_text(node, indent, fn_ret)?,
        })
    }

    // ── The driver (generate_rust_from_spec_assembled) ──

    pub(crate) fn driver_file(
        &self,
        candidate: &CandidateImplementation,
    ) -> Result<String, String> {
        let spec = &candidate.spec;

        // Module header: presence is the doc's Some-ness.
        let header = match &spec.doc {
            Some(doc) => {
                let lines: Result<Vec<String>, String> = doc
                    .lines()
                    .map(|line| {
                        Ok(syn::rust_doc_line::rust_doc_line(line.to_string()))
                    })
                    .collect();
                let header_text =
                    syn::rust_doc_header_text::rust_doc_header_text(lines?.concat());
                syn::rust_driver_section_text::rust_driver_section_text(
                    true,
                    header_text,
                )
            }
            None => {
                syn::rust_driver_section_text::rust_driver_section_text(false, String::new())
            }
        };

        // Signature parts. Each generic parameter's text
        // is the artifact's rust_generic_param_text winner
        // (Phase 62: the name + its Clone bound).
        let generic_names: Vec<String> = spec
            .generics
            .iter()
            .map(|g| {
                syn::rust_generic_param_text::rust_generic_param_text(
                    rust_ident(&g.name),
                )
            })
            .collect();
        let generics = syn::rust_generics_text::rust_generics_text(
            generic_names.join(", "),
            !spec.generics.is_empty(),
        );
        let params: Result<Vec<String>, String> = spec
            .params
            .iter()
            .map(|p| {
                Ok(syn::rust_param_text::rust_param_text(
                    rust_ident(&p.name),
                    self.ty_text(&p.ty)?,
                ))
            })
            .collect();
        let signature = syn::rust_signature::rust_signature(
            rust_ident(&spec.name),
            generics,
            params?.join(", "),
            self.ty_text(&spec.return_type)?,
        );

        // Precondition block: presence is non-emptiness.
        // Its position in the file is rust_file_text's
        // composition — the AMENDED equation places it
        // after the signature, and the Phase 56 artifact's
        // winner composes the amended order.
        let pre_block = if spec.preconditions.is_empty() {
            syn::rust_driver_section_text::rust_driver_section_text(false, String::new())
        } else {
            let asserts: Result<Vec<String>, String> = spec
                .preconditions
                .iter()
                .map(|c| {
                    Ok(syn::rust_precondition_assert::rust_precondition_assert(
                        self.constraint_text(c)?,
                    ))
                })
                .collect();
            let block =
                syn::rust_precondition_block::rust_precondition_block(asserts?.concat());
            syn::rust_driver_section_text::rust_driver_section_text(true, block)
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
            syn::rust_driver_section_text::rust_driver_section_text(false, String::new())
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
                    Ok(syn::rust_test_case::rust_test_case(
                        rust_ident(&raw_name),
                        rust_ident(&spec.name),
                        // The call's generics text
                        // (Phase 61): the driver's rule,
                        // shared with the original
                        // (super::test_call_generics_text).
                        super::test_call_generics_text(spec, t),
                        input_text,
                        self.sexpr_text(&t.expected_output)?,
                    ))
                })
                .collect();
            let module = syn::rust_test_module::rust_test_module(cases?.concat());
            syn::rust_driver_section_text::rust_driver_section_text(true, module)
        };

        Ok(syn::rust_file_text::rust_file_text(
            header,
            pre_block,
            signature,
            body,
            test_module,
        ))
    }

    // ── Dispatch (generate) ──

    /// The compiled dispatch's prediction for a target name:
    /// Ok for routes 0..=5, Err with the composed error text
    /// for route 6. (The interpreter composition's outer
    /// execution-error layer does not exist here: the compiled
    /// winners are total.)
    pub(crate) fn dispatch(&self, target: &str) -> Result<(), String> {
        let route = syn::rust_target_route::rust_target_route(target.to_string());
        let form = syn::rust_target_result_form::rust_target_result_form(route);
        match form {
            0 => Ok(()),
            _ => Err(syn::rust_unsupported_target_text::rust_unsupported_target_text(
                target.to_string(),
            )),
        }
    }
}

/// The original's pattern_has_string_literal (glue in the
/// scrutinee composition, as in the assemblies) — shim (9).
fn pattern_has_string_literal(pat: &IRPattern) -> bool {
    match pat {
        IRPattern::Literal(IRLiteral::String(_)) => true,
        IRPattern::Constructor(_, subs) => subs.iter().any(pattern_has_string_literal),
        IRPattern::Guard(inner, _) => pattern_has_string_literal(inner),
        _ => false,
    }
}

/// The original's is_bare_string_literal (through Typed) —
/// shim (9).
fn is_bare_string_literal(node: &IRNode) -> bool {
    match node {
        IRNode::Literal(IRLiteral::String(_)) => true,
        IRNode::Typed { node, .. } => is_bare_string_literal(node),
        _ => false,
    }
}
