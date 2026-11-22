// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC CODEGEN                                         │
// │  Generate code from verified implementation candidates   │
// └──────────────────────────────────────────────────────────┘

use crate::spec::ast::*;
use crate::synthesis::engine::{CandidateImplementation, IRNode, IRLiteral, IRBinOp, IRUnaryOp, CollectionKind, IRPattern};

// ── Public API ────────────────────────────────────────────

/// Generate source code from a verified candidate implementation
pub fn generate(candidate: &CandidateImplementation, target: &str) -> Result<String, CodegenError> {
    match target {
        "rust" => generate_rust_route(candidate),
        "c" => generate_c(candidate),
        "wasm" => generate_wasm(candidate),
        "python" => generate_python(candidate),
        "javascript" | "js" => generate_javascript(candidate),
        "ir" => Ok(debug_ir_string(&candidate.body, 0)),
        other => Err(CodegenError::UnsupportedTarget(other.into())),
    }
}

/// The "rust" route of `generate` — the M5-S2 switch
/// (Phase 52). With the Cargo feature `bootstrap-codegen`
/// (owner decision D4, accepted 2026-11-21), the compiled
/// synthesized adapter renders: that build IS the bootstrap
/// binary of the M5 audit's definition (the original compiler
/// with the synthesized Codegen module seated in the Rust
/// emitter's place). Without the feature, the original
/// `generate_rust` renders — byte-for-byte the pre-M5
/// compiler. Every other route of `generate` is untouched in
/// both configurations, and the original emitter stays in the
/// tree in both (the ROAD anti-pattern rule: keep both,
/// switch by feature flag).
fn generate_rust_route(candidate: &CandidateImplementation) -> Result<String, CodegenError> {
    #[cfg(feature = "bootstrap-codegen")]
    {
        bootstrap_adapter::render_rust_synthesized(candidate)
    }
    #[cfg(not(feature = "bootstrap-codegen"))]
    {
        generate_rust(candidate)
    }
}

// ── Rust Codegen ──────────────────────────────────────────

/// Sanitize a name from spec/IR text into a valid Rust identifier.
/// Names in specs are free text (e.g. test names like "empty-list"),
/// so anything that is not `[A-Za-z0-9_]` becomes `_`; a leading digit
/// gets a `_` prefix, and a result that collides with a Rust keyword
/// gets a `_` suffix. Applied at every Rust emission point so that a
/// name's declaration and all its uses sanitize identically.
fn rust_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 1);
    for (i, ch) in name.chars().enumerate() {
        if ch == '_' || ch.is_alphanumeric() {
            if i == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    if is_rust_keyword(&out) {
        out.push('_');
    }
    out
}

fn is_rust_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "break" | "const" | "continue" | "crate" | "else" | "enum" | "extern"
            | "false" | "fn" | "for" | "if" | "impl" | "in" | "let" | "loop" | "match"
            | "mod" | "move" | "mut" | "pub" | "ref" | "return" | "self" | "Self"
            | "static" | "struct" | "super" | "trait" | "true" | "type" | "unsafe"
            | "use" | "where" | "while" | "async" | "await" | "dyn" | "abstract"
            | "become" | "box" | "do" | "final" | "macro" | "override" | "priv"
            | "typeof" | "unsized" | "virtual" | "yield" | "try"
    )
}

// Kept-but-unselected under `bootstrap-codegen` (the M5-S2
// switch routes around it there — the ROAD's keep-both
// rule); the allow fires only in that configuration.
#[cfg_attr(feature = "bootstrap-codegen", allow(dead_code))]
fn generate_rust(candidate: &CandidateImplementation) -> Result<String, CodegenError> {
    let mut out = String::new();

    // Module header
    if !candidate.spec.doc.is_none() {
        let doc = candidate.spec.doc.as_ref().unwrap();
        for line in doc.lines() {
            out.push_str(&format!("//! {}\n", line));
        }
        out.push('\n');
    }

    // Function signature. Every declared generic parameter
    // carries a `: Clone` bound (Phase 62, owner decision
    // (i)): the bound-free signature provably admits no
    // element-reading construction (Phase 61 probe: E0507 /
    // E0599 / E0631 by construction). The per-parameter
    // text is specced (rust_generic_param_text in
    // examples/self_host/codegen.morph).
    let generics = if candidate.spec.generics.is_empty() {
        String::new()
    } else {
        let g = candidate.spec.generics.iter()
            .map(|gp| format!("{}: Clone", rust_ident(&gp.name)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("<{}>", g)
    };

    let params: Vec<String> = candidate.spec.params.iter()
        .map(|p| format!("{}: {}", rust_ident(&p.name), ty_to_rust(&p.ty)))
        .collect();

    let ret = ty_to_rust(&candidate.spec.return_type);

    // Function definition
    out.push_str(&format!(
        "pub fn {}{}({}) -> {} {{\n",
        rust_ident(&candidate.spec.name), generics, params.join(", "), ret
    ));

    // Precondition asserts (debug-only), at the head of the
    // function body (Phase 55): through Phase 54 this block
    // was emitted before the signature as a free-standing
    // module-level block, which no Rust file can parse; the
    // block's text is unchanged, only its position moved
    // (rust_file_text's equation states the order).
    if !candidate.spec.preconditions.is_empty() {
        out.push_str("#[cfg(debug_assertions)]\n");
        out.push_str("{\n");
        for pre in &candidate.spec.preconditions {
            out.push_str(&format!("    debug_assert!({}, \"Precondition violated\");\n",
                constraint_to_rust(pre)));
        }
        out.push_str("}\n\n");
    }

    // Body — a tail position: its value is the function's return
    // value, so it is rendered with the tail renderer (the same
    // one match arms use). Statement-shaped bodies (Match, Let,
    // Return, loops, …) fall through to statement rendering
    // unchanged; bare expression bodies (Literal, BinOp, Var,
    // Collection, …) keep their value instead of being discarded
    // as `expr;` statements that make the function return ().
    let body_code = ir_to_rust_tail(
        &candidate.body,
        1,
        &candidate.spec.return_type,
        Some(&candidate.spec.return_type),
    );
    out.push_str(&body_code);

    out.push_str("}\n");

    // Tests
    if !candidate.spec.tests.is_empty() {
        out.push_str("\n#[cfg(test)]\n");
        out.push_str("mod tests {\n");
        out.push_str("    use super::*;\n\n");

        for (i, test) in candidate.spec.tests.iter().enumerate() {
            let test_name = test.name.clone().unwrap_or_else(|| format!("test_{}", i));
            out.push_str(&format!("    #[test]\n"));
            out.push_str(&format!("    fn {}() {{\n", rust_ident(&test_name)));
            // A tuple test input whose arity equals the
            // parameter count spreads its elements into the
            // call positionally (rust_test_case's doc states
            // the rule; the same arity rule Phase 53's
            // evaluation binding uses). Every other input
            // renders as a single expression.
            let input_text = match &test.input {
                Expr::Call(name, args)
                    if name == "__tuple__"
                        && args.len() == candidate.spec.params.len() =>
                {
                    args.iter()
                        .map(expr_to_rust)
                        .collect::<Vec<_>>()
                        .join(", ")
                }
                other => expr_to_rust(other),
            };
            out.push_str(&format!(
                "        let result = {}{}({});\n",
                rust_ident(&candidate.spec.name),
                test_call_generics_text(&candidate.spec, test),
                input_text
            ));
            out.push_str(&format!(
                "        assert_eq!(result, {});\n",
                expr_to_rust(&test.expected_output)
            ));
            out.push_str("    }\n");
        }

        out.push_str("}\n");
    }

    Ok(out)
}

/// The turbofish annotation for one generated test's call
/// (Phase 61, campaign slice G6) — the `generics_text` the
/// driver hands `rust_test_case` (its spec doc states the
/// rule). The empty string for every test except the
/// E0283 class: a generic spec with a function-typed
/// parameter, where the test passes an empty collection
/// literal for a `List<generic>` parameter and no other
/// argument of the test pins the generic — without the
/// annotation the empty collection leaves the element
/// type unpinned against the unconstrained fn-typed
/// signature and the generated test module does not
/// compile (E0283; measured on sort in Phases 56/58/61).
/// A generic is pinned by a scalar literal at a
/// bare-generic parameter position, or by a scalar
/// literal inside a non-empty list literal at a
/// `List<generic>` position; a lambda never pins. The
/// concrete type is derived from the spec's own test
/// literals (the first scalar literal kind across the
/// spec's tests — the G1 runner harness's derivation,
/// `synthesis::runner` — defaulting to i64), repeated
/// once per generic parameter. `pub(crate)`: the compiled
/// bootstrap adapter (bootstrap_adapter) shares this
/// driver rule; the proof assemblies carry their own
/// copies, pinned by the gates.
pub(crate) fn test_call_generics_text(spec: &FunctionSpec, test: &Test) -> String {
    if spec.generics.is_empty() {
        return String::new();
    }
    if !spec.params.iter().any(|p| matches!(p.ty, TypeRef::Function(..))) {
        return String::new();
    }
    // Argument positions, under the same spread rule the
    // call's input text follows (rust_test_case's doc).
    let positions: Vec<(&Expr, &TypeRef)> = match &test.input {
        Expr::Call(name, args) if name == "__tuple__" && args.len() == spec.params.len() => {
            args.iter()
                .zip(spec.params.iter().map(|p| &p.ty))
                .collect()
        }
        other if spec.params.len() == 1 => vec![(other, &spec.params[0].ty)],
        _ => return String::new(),
    };
    let generic_names: Vec<&str> = spec.generics.iter().map(|g| g.name.as_str()).collect();
    // Does a type position denote the generic `g`? (The
    // parser carries type parameters as Named; the AST's
    // Generic form is the same fact in the other spelling.)
    let is_generic = |t: &TypeRef, g: &str| -> bool {
        matches!(t, TypeRef::Generic(n) | TypeRef::Named(n) if n == g)
    };
    let is_scalar_literal = |e: &Expr| -> bool {
        matches!(
            e,
            Expr::IntLit(_)
                | Expr::FloatLit(_)
                | Expr::BoolLit(_)
                | Expr::StringLit(_)
                | Expr::CharLit(_)
        )
    };
    let unpinned_empty = generic_names.iter().any(|g| {
        let has_empty = positions.iter().any(|(e, t)| {
            matches!(t, TypeRef::List(inner) if is_generic(inner, g))
                && matches!(e, Expr::Call(n, a) if n == "List" && a.is_empty())
        });
        if !has_empty {
            return false;
        }
        let pinned = positions.iter().any(|(e, t)| {
            (is_generic(t, g) && is_scalar_literal(e))
                || (matches!(t, TypeRef::List(inner) if is_generic(inner, g))
                    && matches!(e, Expr::Call(n, a) if n == "List" && a.iter().any(|el| is_scalar_literal(el))))
        });
        !pinned
    });
    if !unpinned_empty {
        return String::new();
    }
    let ty = concrete_type_for_spec(spec);
    let types = vec![ty; spec.generics.len()].join(", ");
    format!("::<{}>", types)
}

/// The concrete type the annotation pins a spec's
/// generics to: the first scalar literal kind found in
/// the spec's own tests (DFS over inputs, then expected
/// outputs, test by test), defaulting to i64 — the G1
/// runner harness's derivation (synthesis::runner's
/// `concrete_type_for_spec`), carried here for the
/// shipped rendering.
fn concrete_type_for_spec(spec: &FunctionSpec) -> &'static str {
    for test in &spec.tests {
        if let Some(ty) = first_literal_type(&test.input) {
            return ty;
        }
        if let Some(ty) = first_literal_type(&test.expected_output) {
            return ty;
        }
    }
    "i64"
}

fn first_literal_type(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::IntLit(_) => Some("i64"),
        Expr::FloatLit(_) => Some("f64"),
        Expr::BoolLit(_) => Some("bool"),
        Expr::StringLit(_) => Some("String"),
        Expr::CharLit(_) => Some("char"),
        Expr::Var(_) => None,
        Expr::Field(inner, _) => first_literal_type(inner),
        Expr::Index(a, b) => first_literal_type(a).or_else(|| first_literal_type(b)),
        Expr::Call(_, args) => args.iter().find_map(|a| first_literal_type(a)),
        Expr::BinOp(_, l, r) => first_literal_type(l).or_else(|| first_literal_type(r)),
        Expr::UnaryOp(_, e) => first_literal_type(e),
        Expr::Lambda(_, body) => first_literal_type(body),
        Expr::Length(e) => first_literal_type(e),
        Expr::Comprehension {
            collection, body, ..
        } => first_literal_type(collection).or_else(|| first_literal_type(body)),
        // M1 features (Phase 73): the literal search walks
        // the new forms structurally — receiver before
        // arguments, field values in written order.
        Expr::MethodCall { receiver, args, .. } => first_literal_type(receiver)
            .or_else(|| args.iter().find_map(|a| first_literal_type(a))),
        Expr::RecordLit { fields, .. } => fields
            .iter()
            .find_map(|(_, v)| first_literal_type(v)),
    }
}

// ── C Codegen ─────────────────────────────────────────────

fn generate_c(candidate: &CandidateImplementation) -> Result<String, CodegenError> {
    let mut out = String::new();

    out.push_str("#include <stdbool.h>\n");
    out.push_str("#include <stddef.h>\n");
    out.push_str("#include <stdint.h>\n\n");

    // Struct types from spec
    for ty in &candidate.spec.types() {
        if let TypeKind::Record { fields } = &ty.kind {
            out.push_str(&format!("typedef struct {{\n"));
            for (fname, ftype) in fields {
                out.push_str(&format!("    {} {};\n", ty_to_c(ftype), fname));
            }
            out.push_str(&format!("}} {};\n\n", ty.name));
        }
    }

    // Function signature
    let ret = ty_to_c(&candidate.spec.return_type);
    let params: Vec<String> = candidate.spec.params.iter()
        .map(|p| format!("{} {}", ty_to_c(&p.ty), p.name))
        .collect();

    out.push_str(&format!(
        "{} {}({}) {{\n",
        ret, candidate.spec.name, params.join(", ")
    ));

    let body = ir_to_c(&candidate.body, 1);
    out.push_str(&body);
    out.push_str("}\n");

    Ok(out)
}

// ── WASM (WebAssembly Text Format) Codegen ────────────────

fn generate_wasm(candidate: &CandidateImplementation) -> Result<String, CodegenError> {
    let mut out = String::new();
    out.push_str("(module\n");

    // Export the function
    out.push_str(&format!(
        "  (export \"{}\" (func ${}))\n",
        candidate.spec.name, candidate.spec.name
    ));

    // Local types
    let mut local_count = candidate.spec.params.len();
    let params_wasm: Vec<String> = candidate.spec.params.iter()
        .map(|p| {
            local_count += 1;
            format!("(param {} {})", p.name, ty_to_wasm(&p.ty))
        })
        .collect();

    out.push_str(&format!(
        "  (func ${} {}\n    (result {})\n",
        candidate.spec.name,
        params_wasm.join(" "),
        ty_to_wasm(&candidate.spec.return_type),
    ));

    // Body in WAT
    let body = ir_to_wasm(&candidate.body);
    out.push_str(&body);

    out.push_str("  )\n");
    out.push_str(")\n");

    Ok(out)
}

// ── Python Codegen ────────────────────────────────────────

fn generate_python(candidate: &CandidateImplementation) -> Result<String, CodegenError> {
    let mut out = String::new();

    // Docstring
    if let Some(doc) = &candidate.spec.doc {
        out.push_str(&format!("\"\"\"{}\"\"\"\n", doc));
    }

    // Type hints
    let params: Vec<String> = candidate.spec.params.iter()
        .map(|p| format!("{}: {}", p.name, ty_to_python(&p.ty)))
        .collect();

    out.push_str(&format!(
        "def {}({}) -> {}:\n",
        candidate.spec.name,
        params.join(", "),
        ty_to_python(&candidate.spec.return_type),
    ));

    let body = ir_to_python(&candidate.body, 1);
    out.push_str(&body);

    // Default return
    if !body.contains("return") {
        out.push_str("    pass\n");
    }

    Ok(out)
}

// ── JavaScript Codegen ────────────────────────────────────

fn generate_javascript(candidate: &CandidateImplementation) -> Result<String, CodegenError> {
    let mut out = String::new();

    // JSDoc
    if let Some(doc) = &candidate.spec.doc {
        out.push_str(&format!("/** {} */\n", doc));
    }

    let params: Vec<String> = candidate.spec.params.iter()
        .map(|p| p.name.clone())
        .collect();

    out.push_str(&format!(
        "function {}({}) {{\n",
        candidate.spec.name,
        params.join(", "),
    ));

    let body = ir_to_javascript(&candidate.body, 1);
    out.push_str(&body);
    out.push_str("}\n");

    // Also export
    out.push_str(&format!(
        "export {{ {} }};\n",
        candidate.spec.name,
    ));

    Ok(out)
}

// ── IR → Rust Translation ─────────────────────────────────

/// Statement-level IR → Rust. `fn_ret` is the enclosing function's
/// return type, threaded through so value positions whose expected
/// type is the return type (match arms, `Return` expressions) can be
/// emitted with that expectation — see `ir_to_rust_expr_typed`.
/// A `Match` rendered here in statement position has its value
/// discarded unless it is the function tail; using `fn_ret` as the
/// arm expectation in both cases keeps the arms mutually consistent,
/// and the only conversion it drives (string literal → `String`)
/// fires solely when the function actually returns `String`.
fn ir_to_rust(node: &IRNode, indent: usize, fn_ret: &TypeRef) -> String {
    let pad = "    ".repeat(indent);
    let pad_inner = "    ".repeat(indent + 1);

    match node {
        IRNode::Hole { .. } => {
            format!("{}// TODO: synthesis incomplete\n{}panic!(\"Hole not filled\");\n", pad, pad)
        }

        IRNode::Block(stmts) => {
            let mut out = String::new();
            for stmt in stmts {
                out.push_str(&ir_to_rust(stmt, indent, fn_ret));
            }
            out
        }

        IRNode::Let { name, value, body } => {
            // The Let's body is the value position of the whole
            // Let: render it tail-shaped so a bare expression body
            // yields its value (see ir_to_rust_tail).
            format!(
                "{}let {} = {};\n{}",
                pad, rust_ident(name), ir_to_rust_expr(value),
                ir_to_rust_tail(body, indent, fn_ret, Some(fn_ret))
            )
        }

        IRNode::If { condition, then_branch, else_branch } => {
            // Branch bodies are the If's value positions: render
            // them tail-shaped, like match arms.
            format!(
                "{}if {} {{\n{}}} else {{\n{}}}\n",
                pad,
                expr_in_condition_position(condition),
                ir_to_rust_tail(then_branch, indent + 1, fn_ret, Some(fn_ret)),
                ir_to_rust_tail(else_branch, indent + 1, fn_ret, Some(fn_ret)),
            )
        }

        IRNode::While { condition, body, invariant } => {
            let inv_comment = invariant.as_ref()
                .map(|i| format!("{}// invariant: {}\n", pad, i))
                .unwrap_or_default();
            format!(
                "{}{}while {} {{\n{}{}}}\n",
                inv_comment,
                pad,
                expr_in_condition_position(condition),
                ir_to_rust(body, indent + 1, fn_ret),
                pad,
            )
        }

        IRNode::For { var, start, end, body } => {
            format!(
                "{}for {} in {}..{} {{\n{}{}}}\n",
                pad, rust_ident(var),
                ir_to_rust_expr(start),
                ir_to_rust_expr(end),
                ir_to_rust(body, indent + 1, fn_ret),
                pad,
            )
        }

        IRNode::Match { scrutinee, cases } => {
            let mut out = format!("{}match {} {{\n", pad, match_scrutinee_to_rust(scrutinee, cases));
            for (pattern, body) in cases {
                let pat = ir_pattern_to_rust(pattern);
                out.push_str(&format!("{}{} => {{\n{}{}}},\n",
                    pad_inner, pat,
                    ir_to_rust_tail(body, indent + 2, fn_ret, Some(fn_ret)),
                    pad_inner,
                ));
            }
            out.push_str(&format!("{}}}\n", pad));
            out
        }

        IRNode::Call { function, args } => {
            // Statement position: terminated like any other
            // expression statement — the unterminated form was a
            // syntax error whenever a statement followed. Value
            // positions never reach this arm: they route through
            // ir_to_rust_tail, which emits the bare call. The
            // call text itself is the expression walker's
            // (Phase 85: one rendering per call shape).
            format!("{}{};\n", pad, ir_call_text(function, args))
        }

        IRNode::Return(expr) => {
            format!("{}return {};\n", pad, ir_to_rust_expr_typed(expr, Some(fn_ret)))
        }

        IRNode::Alloc { name, ty, initial } => {
            format!(
                "{}let mut {}: {} = {};\n",
                pad, rust_ident(name), ty_to_rust(ty), ir_to_rust_expr_typed(initial, Some(ty))
            )
        }

        IRNode::Assign { target, value } => {
            format!(
                "{}{} = {};\n",
                pad, ir_to_rust_expr(target), ir_to_rust_expr(value)
            )
        }

        IRNode::Collection { kind, elements } => {
            // Statement position: a collection used as a
            // statement must be terminated with `;` like any
            // other expression statement — the unterminated form
            // was a syntax error whenever a statement followed
            // ("expected `;`, found keyword `return`" in the
            // Phase 11 probe winners). Genuine tail positions
            // (function body, match arms, If/Let value positions)
            // never reach this arm: they route through
            // ir_to_rust_tail, which emits the bare expression.
            match kind {
                CollectionKind::List => {
                    let elems: Vec<String> = elements.iter().map(|e| ir_to_rust_expr(e)).collect();
                    format!("{}vec![{}];\n", pad, elems.join(", "))
                }
                CollectionKind::Array => {
                    let elems: Vec<String> = elements.iter().map(|e| ir_to_rust_expr(e)).collect();
                    format!("{}[{}];\n", pad, elems.join(", "))
                }
                CollectionKind::Tuple => {
                    let elems: Vec<String> = elements.iter().map(|e| ir_to_rust_expr(e)).collect();
                    format!("{}({});\n", pad, elems.join(", "))
                }
                CollectionKind::Set => {
                    let elems: Vec<String> = elements.iter().map(|e| ir_to_rust_expr(e)).collect();
                    format!("{}vec![{}];\n", pad, elems.join(", "))
                }
                CollectionKind::Map => {
                    let elems: Vec<String> = elements.iter().map(|e| ir_to_rust_expr(e)).collect();
                    format!("{}vec![{}];\n", pad, elems.join(", "))
                }
            }
        }

        IRNode::Map { function, over } => {
            // Statement position: terminated (see the Call arm).
            format!(
                "{}({}).iter().map({}).collect();\n",
                pad,
                ir_to_rust_expr(over),
                ir_to_rust_expr(function),
            )
        }

        IRNode::Filter { predicate, over } => {
            format!(
                "{}({}).iter().filter({}).collect();\n",
                pad,
                ir_to_rust_expr(over),
                ir_to_rust_expr(predicate),
            )
        }

        IRNode::Fold { operation, initial, over } => {
            format!(
                "{}({}).iter().fold({}, {});\n",
                pad,
                ir_to_rust_expr(over),
                ir_to_rust_expr(initial),
                ir_to_rust_expr(operation),
            )
        }

        IRNode::Sort { comparator, collection } => {
            // The final `sorted` is the statement's discarded
            // value: terminated (see the Call arm). As a function
            // body or arm value it routes through ir_to_rust_tail
            // instead and keeps the value.
            format!(
                "{}let mut sorted = {};\nsorted.sort_by({});\nsorted;\n",
                pad,
                ir_to_rust_expr(collection),
                ir_to_rust_expr(comparator),
            )
        }

        IRNode::Typed { node, .. } => ir_to_rust(node, indent, fn_ret),

        // Expressions that shouldn't appear as statements
        _ => format!("{}{};\n", pad, ir_to_rust_expr(node)),
    }
}

/// Render a node in tail position (a match arm body): its value is the
/// enclosing block's value, so expression nodes are emitted without the
/// trailing `;` that statement rendering adds — a `value;` arm would
/// evaluate to `()` and fail the function's return type (E0308).
/// Statement-shaped nodes (Let, If, Return, Hole, nested Match, …)
/// keep their normal statement rendering, which is already tail-shaped
/// where it needs to be.
fn ir_to_rust_tail(
    node: &IRNode,
    indent: usize,
    fn_ret: &TypeRef,
    expected: Option<&TypeRef>,
) -> String {
    let pad = "    ".repeat(indent);
    match node {
        IRNode::Block(stmts) => {
            let mut out = String::new();
            if let Some((last, rest)) = stmts.split_last() {
                for stmt in rest {
                    out.push_str(&ir_to_rust(stmt, indent, fn_ret));
                }
                out.push_str(&ir_to_rust_tail(last, indent, fn_ret, expected));
            }
            out
        }
        // A Typed node declares its own type: it overrides the
        // expectation inherited from the enclosing position.
        IRNode::Typed { node, ty } => ir_to_rust_tail(node, indent, fn_ret, Some(ty)),
        IRNode::Literal(_) | IRNode::Var(_) | IRNode::BinOp(..) | IRNode::UnaryOp(..)
        | IRNode::Lambda { .. } | IRNode::Closure { .. } | IRNode::Collection { .. }
        | IRNode::Call { .. } | IRNode::Map { .. } | IRNode::Filter { .. }
        | IRNode::Fold { .. } | IRNode::Sort { .. } | IRNode::Index(..)
        | IRNode::TryPropagate(..)
        // Phase 77: the new expression kinds are value-
        // shaped like Index — a tail-position projection
        // or record literal yields its value.
        | IRNode::FieldAccess { .. } | IRNode::RecordLit { .. } => {
            format!("{}{}\n", pad, ir_to_rust_expr_typed(node, expected))
        }
        _ => ir_to_rust(node, indent, fn_ret),
    }
}

/// Expression emission with an expected type, where the caller knows
/// one (function return position, match arm tail, an `Alloc`'s
/// declared type, or a `Typed` node's annotation). The only
/// expectation-driven rewrite: a string literal whose expected type
/// is `String` is emitted owned (`"s".to_string()`) — a bare `"s"`
/// is a `&str` and fails E0308 where a `String` is required
/// (defect 4d). Everywhere else the literal stays a `&str`, so
/// `&str` contexts are unchanged. All other nodes delegate to the
/// untyped emitter; a `Typed` node re-enters here with its own
/// declared type as the expectation.
fn ir_to_rust_expr_typed(node: &IRNode, expected: Option<&TypeRef>) -> String {
    match node {
        IRNode::Typed { node, ty } => ir_to_rust_expr_typed(node, Some(ty)),
        IRNode::Literal(IRLiteral::String(s)) if expected == Some(&TypeRef::String) => {
            format!("\"{}\".to_string()", escape_rust_str(s))
        }
        _ => ir_to_rust_expr(node),
    }
}

/// Render a statement-shaped node as a Rust block expression
/// for use in expression position (`Let`, `While`, `For`): the
/// statement rendering goes inside `{ … }`, where a Let's tail
/// body still yields its value and the loops yield `()`, exactly
/// as in statement position. Statement rendering needs the
/// enclosing function's return type; expression position does
/// not carry one, so a neutral `Unit` context is used — the only
/// loss is the String-ownership rewrite in return/tail positions
/// nested inside the block, which the untyped expression emitter
/// already forgoes elsewhere (If branches, Lambda bodies).
/// Escape a string's contents for emission inside a Rust
/// string literal (Phase 29, defect 3). `"` and `\` take a
/// backslash; `\n`, `\r` and `\t` use their short forms;
/// every other control character uses `\u{...}`. All other
/// characters — including non-ASCII text — are emitted
/// as-is, so a literal without special characters renders
/// byte-identically to the pre-fix emitter. The rule is a
/// per-character transformation: in the self-hosted specs
/// it is mirror-pinned (finding F5), and the spec equation
/// for the literal template pins the wrap on the
/// no-special-characters subdomain.
fn escape_rust_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Escape one character for emission inside a Rust char
/// literal (Phase 29, defect 3, the adjacent arm of the
/// string rule).
fn escape_rust_char(c: char) -> String {
    match c {
        '\'' => "\\'".to_string(),
        '\\' => "\\\\".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        c if c.is_control() => format!("\\u{{{:x}}}", c as u32),
        c => c.to_string(),
    }
}

/// Left operand of an IR Concat in `ir_to_rust_expr`
/// (Phase 29, defect 1). Rust's `Add` for text is
/// `String + &str`: the left operand must be an owned
/// String. A string literal renders as a &str, so in left
/// position it takes the same `.to_string()` ownership the
/// typed emitter applies in expectation positions; every
/// other operand shape (Call, nested Concat, …) already
/// evaluates to a fresh String value and renders as usual.
///
/// Phase 39 (E0382, Concat-operand move — the second
/// scoped unfreeze, user-approved 2026-11-21): a VARIABLE
/// is not a fresh value — rendering it bare MOVES it, so
/// a template that uses one String parameter two or more
/// times on an execution path (the statement templates'
/// `pad` / `pad_inner`, repeated in a single `++` chain)
/// failed E0382, borrow of moved value. The borrow-based
/// alternative does not exist (`&String + &str` has no
/// `Add` impl — string concatenation requires an owned
/// String on the left, verified empirically), so the Var
/// takes the literal treatment's own analogue: an owned
/// copy, `.clone()` (the emitter's Clone-op rendering).
/// The clone is value-transparent — the IR is pure — so
/// every previously compiling rendering keeps its
/// behavior; only its bytes move (single-use Var-left
/// renderings included: whether a bare move is legal
/// depends on uses anywhere later on the path, which an
/// operand renderer cannot see — see the Phase 39
/// inventory in ROAD_TO_SELF_HOSTING.md). The match is on
/// the DIRECT node only, as with the literal arm: a
/// Typed-wrapped variable delegates through the typed
/// walker and is outside this treatment (no measured
/// winner has that shape).
fn concat_owned_operand(node: &IRNode) -> String {
    match node {
        IRNode::Literal(IRLiteral::String(s)) => {
            format!("\"{}\".to_string()", escape_rust_str(s))
        }
        IRNode::Var(name) => {
            format!("{}.clone()", rust_ident(name))
        }
        _ => ir_to_rust_expr(node),
    }
}

/// Right operand of an IR Concat in `ir_to_rust_expr`
/// (Phase 29, defect 2a). `String + String` is E0308 —
/// the right operand must be borrowed. A string literal
/// is already a &str and stays bare (borrowing it would
/// produce a `&&str`, which has no `Add` impl); a nested
/// Concat is borrowed as a unit (its rendering is already
/// parenthesized); a block-like operand is parenthesized
/// first, the same set `expr_in_condition_position`
/// wraps.
fn concat_borrowed_operand(node: &IRNode) -> String {
    match node {
        IRNode::Literal(IRLiteral::String(_)) => ir_to_rust_expr(node),
        _ if is_block_like_expr(node) => {
            format!("&({})", ir_to_rust_expr(node))
        }
        _ => format!("&{}", ir_to_rust_expr(node)),
    }
}

/// The block-like node kinds whose renderings need
/// parentheses when an operator or a borrow is prefixed
/// (the set `expr_in_condition_position` wraps).
fn is_block_like_expr(node: &IRNode) -> bool {
    matches!(
        node,
        IRNode::Match { .. }
            | IRNode::If { .. }
            | IRNode::Loop { .. }
            | IRNode::While { .. }
            | IRNode::For { .. }
            | IRNode::Let { .. }
            | IRNode::Block(_)
            | IRNode::StructDef { .. }
            | IRNode::Lambda { .. }
            | IRNode::Closure { .. }
    )
}

/// Scrutinee rendering for both Match arms (Phase 29,
/// defect 2b). When any case pattern is — or contains —
/// a string literal, the patterns are `&str`, so a
/// String-valued scrutinee must be viewed through
/// `.as_str()`; without it the patterns mismatch the
/// scrutinee (E0308). The emitter has no type
/// information, so the rule is syntactic: it fires on
/// every scrutinee shape except one that is itself a
/// string literal — a literal scrutinee is already a
/// &str, its patterns already match, and `&str` has no
/// stable `as_str()` (E0658). A scrutinee variable
/// let-bound to a bare literal is the known blind spot
/// of the syntactic rule, the same class of limitation
/// as the expectation-driven ownership treatment.
fn match_scrutinee_to_rust(
    scrutinee: &IRNode,
    cases: &[(IRPattern, IRNode)],
) -> String {
    let rendered = expr_in_condition_position(scrutinee);
    if cases.iter().any(|(pat, _)| pattern_has_string_literal(pat))
        && !is_bare_string_literal(scrutinee)
    {
        format!("{}.as_str()", rendered)
    } else {
        rendered
    }
}

fn pattern_has_string_literal(pat: &IRPattern) -> bool {
    match pat {
        IRPattern::Literal(IRLiteral::String(_)) => true,
        IRPattern::Constructor(_, subs) => {
            subs.iter().any(pattern_has_string_literal)
        }
        IRPattern::Guard(inner, _) => pattern_has_string_literal(inner),
        _ => false,
    }
}

fn is_bare_string_literal(node: &IRNode) -> bool {
    match node {
        IRNode::Literal(IRLiteral::String(_)) => true,
        IRNode::Typed { node, .. } => is_bare_string_literal(node),
        _ => false,
    }
}

fn expr_block_stmt(node: &IRNode) -> String {
    let neutral = TypeRef::Unit;
    format!("{{ {} }}", ir_to_rust(node, 0, &neutral).trim_end())
}

/// The loud fallback for a shape with no honest expression form,
/// following the codebase's Hole convention: `panic!` has type
/// `!` and coerces anywhere, so the generated file still parses
/// and compiles — the gap stays loud instead of silent.
fn unsupported_expr(shape: &str) -> String {
    format!("panic!(\"unsupported IR in expression position: {}\")", shape)
}

/// Block-like expressions (match/if/loop/blocks/lambdas/…) cannot
/// appear bare where Rust expects a condition or a scrutinee —
/// `if match … { } { }` and `match match … { } { }` do not parse.
/// Wrap those in parens; everything else renders exactly as
/// before, so existing winners' bytes are unchanged.
fn expr_in_condition_position(node: &IRNode) -> String {
    if is_block_like_expr(node) {
        format!("({})", ir_to_rust_expr(node))
    } else {
        ir_to_rust_expr(node)
    }
}

/// The text of an IR Call in expression position
/// (Phase 85, M1 generation campaign Leg 1 — new forms
/// only; every pre-existing name renders byte-identically
/// to the plain call template). Three of the parser's
/// pseudo-calls reach bodies as IR Calls and are not Rust
/// functions at all: `List` is the list literal (the
/// spec-Expr walker's Phase 55 form, mirrored here —
/// `vec![...]`), `None` is the zero-argument constructor
/// (a bare value, never `None()` — E0618), and `len` is
/// the language's one pure builtin (interpreter:
/// `eval_call`), whose result is an Int. Its rendering is
/// keyed on the argument's shape, the only type evidence
/// this walker carries: over a record's field projection
/// or a string literal — the string-shaped positions, and
/// the only ones any measured seed body carries (peek's
/// and next_char's `len(state.source)`) — it is the CHAR
/// count, the interpreter's `chars().count()` semantics
/// (a byte count would misjudge every non-ASCII source:
/// the battery's "é😀x" has 3 chars and 8 bytes); over
/// every other shape it is the element count. Both forms
/// cast to i64, the language's Int. Named boundary: a
/// List-typed record field in the field-projection
/// position would take the char form and fail loudly at
/// rustc (a Vec has no `chars()`); no measured body
/// carries that shape (the tokenize rows' `len` over
/// `tokens` is a MethodCall, which lowers to a Hole and
/// never reaches a body).
fn ir_call_text(function: &str, args: &[IRNode]) -> String {
    let args_str: Vec<String> = args.iter().map(|a| ir_to_rust_expr(a)).collect();
    match function {
        "List" => format!("vec![{}]", args_str.join(", ")),
        "None" if args.is_empty() => "None".to_string(),
        "len" if args.len() == 1 => {
            let arg = &args[0];
            match arg {
                IRNode::FieldAccess { .. } | IRNode::Literal(IRLiteral::String(_)) => {
                    format!("({}).chars().count() as i64", args_str[0])
                }
                _ => format!("({}).len() as i64", args_str[0]),
            }
        }
        // F2 (Phase 100, D1a/D2a): the Int→decimal-text
        // conversion renders as Rust's own `to_string()`
        // — Display itself, so the compiled route matches
        // the interpreter and the encoder's literal fold
        // exactly, negatives and i64 extremes included.
        // Wrong arity keeps the plain call form below (the
        // standing catch-all), as an unrendered builtin
        // should: loud at rustc, never a guessed rendering.
        "int_to_str" if args.len() == 1 => {
            format!("({}).to_string()", args_str[0])
        }
        _ => format!("{}({})", rust_ident(function), args_str.join(", ")),
    }
}

/// Phase 87: the struct-update (FRU) shape test for
/// the Rust RecordLit arm. Returns the base variable
/// and the carried field names when the literal is a
/// same-record REBUILD: it carries at least two
/// fields straight from one base variable — a field
/// is carried when its value is exactly the
/// projection of the same-named field off that
/// variable — and every non-carried field's value
/// both uses the base only in screened positions
/// (fru_use_ok) and is computed from the base at all
/// (fru_reads_base). The screens are what make the
/// variant sound without field-type knowledge: Rust
/// evaluates the explicit field values first and
/// moves the rest out of the base last, so an
/// explicit value may READ any non-carried field (a
/// projection, exactly as the clone rendering reads
/// it), but it must not read a carried field (whose
/// value the update form supplies from the base,
/// after the explicit values have run) nor touch the
/// base in any other position (a bare use, a call
/// argument — positions that could move or borrow
/// the whole record). Anything outside the screened
/// shapes declines, and the standing clone rendering
/// stands.
fn fru_shape(fields: &[(String, IRNode)]) -> Option<(&str, Vec<&str>)> {
    let mut by_base: Vec<(&str, Vec<&str>)> = Vec::new();
    for (name, value) in fields {
        if let IRNode::FieldAccess { base, field } = value {
            if field == name {
                if let IRNode::Var(b) = base.as_ref() {
                    match by_base.iter_mut().find(|(bb, _)| *bb == b.as_str()) {
                        Some((_, names)) => names.push(name.as_str()),
                        None => by_base.push((b.as_str(), vec![name.as_str()])),
                    }
                }
            }
        }
    }
    let (base, carried) = by_base.into_iter().max_by_key(|(_, names)| names.len())?;
    if carried.len() < 2 {
        return None;
    }
    // A rebuild changes at least one field: a literal
    // whose fields are ALL carries is a projection
    // (it may well build a SMALLER record from the
    // base — the proof corpus's record-lit-field-clone
    // case, S { source, pos } off a wider state — and
    // the update form, whose base must be the very
    // same record, cannot express it).
    if carried.len() == fields.len() {
        return None;
    }
    for (name, value) in fields {
        if carried.iter().any(|c| c == name) {
            continue;
        }
        if !fru_use_ok(value, base, &carried) {
            return None;
        }
        // The rebuild shape's second half: a computed
        // field of a same-record rebuild is computed
        // FROM the base — it reads the base through a
        // projection (of a non-carried field, per the
        // screen above). A literal whose computed
        // fields never touch the base is a construction
        // from parts, not a rebuild — the template's
        // token record is the standing example: its
        // line/col are projections of the tokenizer
        // STATE, a different record, and updating a
        // Token literal from a state base is a type
        // error the clone rendering never risked.
        if !fru_reads_base(value, base) {
            return None;
        }
    }
    Some((base, carried))
}

/// Does this tree project the base variable anywhere
/// (a FieldAccess whose base is exactly the variable)?
/// A search, not a screen: binder shadowing is the
/// screen's concern (fru_use_ok declines binders named
/// like the base), so finding a projection under one
/// can only over-approve in combination with a screen
/// that already declined.
fn fru_reads_base(node: &IRNode, base: &str) -> bool {
    match node {
        IRNode::FieldAccess { base: fb, .. } => {
            matches!(fb.as_ref(), IRNode::Var(n) if n == base) || fru_reads_base(fb, base)
        }
        IRNode::Var(_) | IRNode::Literal(_) => false,
        IRNode::BinOp(_, l, r) => fru_reads_base(l, base) || fru_reads_base(r, base),
        IRNode::UnaryOp(_, inner) => fru_reads_base(inner, base),
        IRNode::Call { args, .. } => args.iter().any(|a| fru_reads_base(a, base)),
        IRNode::If {
            condition,
            then_branch,
            else_branch,
        } => {
            fru_reads_base(condition, base)
                || fru_reads_base(then_branch, base)
                || fru_reads_base(else_branch, base)
        }
        IRNode::Match { scrutinee, cases } => {
            fru_reads_base(scrutinee, base)
                || cases.iter().any(|(_, b)| fru_reads_base(b, base))
        }
        IRNode::Index(c, i) => fru_reads_base(c, base) || fru_reads_base(i, base),
        IRNode::RecordLit { fields, .. } => fields.iter().any(|(_, v)| fru_reads_base(v, base)),
        IRNode::Collection { elements, .. } => elements.iter().any(|e| fru_reads_base(e, base)),
        IRNode::Typed { node, .. } => fru_reads_base(node, base),
        IRNode::Let { value, body, .. } => {
            fru_reads_base(value, base) || fru_reads_base(body, base)
        }
        _ => false,
    }
}

/// The fru_shape screen: every occurrence of the base
/// variable in a computed field's value is the base
/// of a projection of a non-carried field. Binders
/// named like the base decline (their occurrences
/// would be the binder's, and telling them apart per
/// occurrence is not this screen's business), as does
/// any shape not listed.
fn fru_use_ok(node: &IRNode, base: &str, carried: &[&str]) -> bool {
    match node {
        IRNode::Var(name) => name != base,
        IRNode::Literal(_) => true,
        IRNode::FieldAccess { base: fb, field } => {
            if matches!(fb.as_ref(), IRNode::Var(n) if n == base) {
                !carried.iter().any(|c| c == field)
            } else {
                fru_use_ok(fb, base, carried)
            }
        }
        IRNode::BinOp(_, l, r) => fru_use_ok(l, base, carried) && fru_use_ok(r, base, carried),
        IRNode::UnaryOp(_, inner) => fru_use_ok(inner, base, carried),
        IRNode::Call { args, .. } => args.iter().all(|a| fru_use_ok(a, base, carried)),
        IRNode::If {
            condition,
            then_branch,
            else_branch,
        } => {
            fru_use_ok(condition, base, carried)
                && fru_use_ok(then_branch, base, carried)
                && fru_use_ok(else_branch, base, carried)
        }
        IRNode::Match { scrutinee, cases } => {
            fru_use_ok(scrutinee, base, carried)
                && cases.iter().all(|(pat, body)| {
                    !fru_pattern_binds(pat, base) && fru_use_ok(body, base, carried)
                })
        }
        IRNode::Index(c, i) => fru_use_ok(c, base, carried) && fru_use_ok(i, base, carried),
        IRNode::RecordLit { fields, .. } => {
            fields.iter().all(|(_, v)| fru_use_ok(v, base, carried))
        }
        IRNode::Collection { elements, .. } => {
            elements.iter().all(|e| fru_use_ok(e, base, carried))
        }
        IRNode::Typed { node, .. } => fru_use_ok(node, base, carried),
        IRNode::Let { name, value, body } => {
            name != base && fru_use_ok(value, base, carried) && fru_use_ok(body, base, carried)
        }
        _ => false,
    }
}

fn fru_pattern_binds(pat: &IRPattern, name: &str) -> bool {
    match pat {
        IRPattern::Variable(n) => n == name,
        IRPattern::Constructor(_, subs) => subs.iter().any(|p| fru_pattern_binds(p, name)),
        IRPattern::Guard(inner, _) => fru_pattern_binds(inner, name),
        _ => false,
    }
}

fn ir_to_rust_expr(node: &IRNode) -> String {
    match node {
        IRNode::Literal(lit) => match lit {
            IRLiteral::Int(n) => n.to_string(),
            IRLiteral::Float(f) => f.to_string(),
            IRLiteral::Bool(b) => b.to_string(),
            IRLiteral::String(s) => format!("\"{}\"", escape_rust_str(s)),
            IRLiteral::Char(c) => format!("'{}'", escape_rust_char(*c)),
            IRLiteral::Unit => "()".into(),
        },
        IRNode::Var(name) => rust_ident(name),
        IRNode::BinOp(op, lhs, rhs) => {
            let rust_op = match op {
                IRBinOp::Add => "+", IRBinOp::Sub => "-",
                IRBinOp::Mul => "*", IRBinOp::Div => "/",
                IRBinOp::Mod => "%",
                IRBinOp::And => "&&", IRBinOp::Or => "||",
                IRBinOp::Xor => "^",
                IRBinOp::Eq => "==", IRBinOp::Neq => "!=",
                IRBinOp::Lt => "<", IRBinOp::Le => "<=",
                IRBinOp::Gt => ">", IRBinOp::Ge => ">=",
                IRBinOp::Shl => "<<", IRBinOp::Shr => ">>",
                IRBinOp::Concat => "+",
                IRBinOp::Append => ".push",
                IRBinOp::Merge => ".extend",
            };
            if *op == IRBinOp::Append {
                format!("{}.push({})", ir_to_rust_expr(lhs), ir_to_rust_expr(rhs))
            } else if *op == IRBinOp::Merge {
                format!("{}.extend({})", ir_to_rust_expr(lhs), ir_to_rust_expr(rhs))
            } else if *op == IRBinOp::Concat {
                // Phase 29, defects 1+2a (E0369/E0308,
                // user-approved unfreeze): `+` needs an owned
                // left operand and a borrowed right one. The
                // left operand goes through the ownership
                // treatment (see concat_owned_operand), the
                // right through the borrow treatment (see
                // concat_borrowed_operand).
                format!(
                    "({} + {})",
                    concat_owned_operand(lhs),
                    concat_borrowed_operand(rhs)
                )
            } else {
                format!("({} {} {})", ir_to_rust_expr(lhs), rust_op, ir_to_rust_expr(rhs))
            }
        }
        IRNode::UnaryOp(op, inner) => {
            let inner = ir_to_rust_expr(inner);
            match op {
                IRUnaryOp::Neg => format!("(- {})", inner),
                IRUnaryOp::Not => format!("(! {})", inner),
                IRUnaryOp::Abs => format!("{}.abs()", inner),
                IRUnaryOp::Len => format!("{}.len()", inner),
                IRUnaryOp::Clone => format!("{}.clone()", inner),
                IRUnaryOp::Reverse => format!("{}.reverse()", inner),
                // List-part ops (the selector's verdict types
                // them: Head/Last yield the element, Tail/Init
                // the list). Indexing/slicing is the direct
                // lowering; the operand text is emitted twice
                // for Last/Init, which is sound for the IR's
                // pure expression forms.
                IRUnaryOp::Head => format!("({})[0]", inner),
                IRUnaryOp::Last => format!("({})[({}).len() - 1]", inner, inner),
                IRUnaryOp::Tail => format!("({})[1..].to_vec()", inner),
                IRUnaryOp::Init => format!("({})[..({}).len() - 1].to_vec()", inner, inner),
            }
        }
        IRNode::Call { function, args } => ir_call_text(function, args),
        IRNode::If { condition, then_branch, else_branch } => {
            format!(
                "if {} {{ {} }} else {{ {} }}",
                expr_in_condition_position(condition),
                ir_to_rust_expr(then_branch),
                ir_to_rust_expr(else_branch),
            )
        }
        IRNode::Lambda { params, body } | IRNode::Closure { params, body, .. } => {
            let params_str: Vec<String> = params.iter()
                .map(|(n, _)| rust_ident(n))
                .collect();
            format!(
                "|{}| {{ {} }}",
                params_str.join(", "),
                ir_to_rust_expr(body),
            )
        }
        IRNode::Collection { kind, elements } => {
            let elems: Vec<String> = elements.iter().map(|e| ir_to_rust_expr(e)).collect();
            match kind {
                CollectionKind::List => format!("vec![{}]", elems.join(", ")),
                CollectionKind::Array => format!("[{}]", elems.join(", ")),
                CollectionKind::Tuple => format!("({})", elems.join(", ")),
                // Fully qualified: no import is generated for
                // winner files, so a bare `HashSet::from` fails
                // with E0433.
                CollectionKind::Set => format!("std::collections::HashSet::from([{}])", elems.join(", ")),
                CollectionKind::Map => format!("std::collections::HashMap::from([{}])", elems.join(", ")),
            }
        }
        IRNode::Map { function, over } => {
            format!(
                "({}).iter().map({}).collect()",
                ir_to_rust_expr(over),
                ir_to_rust_expr(function),
            )
        }
        IRNode::Filter { predicate, over } => {
            format!(
                "({}).iter().filter({}).collect()",
                ir_to_rust_expr(over),
                ir_to_rust_expr(predicate),
            )
        }
        IRNode::Fold { operation, initial, over } => {
            format!(
                "({}).iter().fold({}, {})",
                ir_to_rust_expr(over),
                ir_to_rust_expr(initial),
                ir_to_rust_expr(operation),
            )
        }
        IRNode::Sort { comparator, collection } => {
            // The statement form (bind, sort, yield) wrapped as a
            // block expression so the sorted value is the value.
            format!(
                "{{ let mut sorted = {}; sorted.sort_by({}); sorted }}",
                ir_to_rust_expr(collection),
                ir_to_rust_expr(comparator),
            )
        }
        IRNode::Match { scrutinee, cases } => {
            let arms: Vec<String> = cases
                .iter()
                .map(|(pat, body)| {
                    format!(
                        "{} => {}",
                        ir_pattern_to_rust(pat),
                        ir_to_rust_expr(body)
                    )
                })
                .collect();
            format!(
                "match {} {{ {} }}",
                match_scrutinee_to_rust(scrutinee, cases),
                arms.join(", ")
            )
        }
        IRNode::Block(stmts) => {
            // A block is an expression in Rust: statements render
            // as statements, the last one keeps the tail position
            // (mirrors ir_to_rust_tail's Block arm, in a neutral
            // return-type context — see expr_block_stmt).
            let neutral = TypeRef::Unit;
            let mut inner = String::new();
            if let Some((last, rest)) = stmts.split_last() {
                for stmt in rest {
                    inner.push_str(&ir_to_rust(stmt, 0, &neutral));
                }
                inner.push_str(&ir_to_rust_tail(last, 0, &neutral, None));
            }
            format!("{{ {} }}", inner.trim_end())
        }
        IRNode::Let { .. } | IRNode::While { .. } | IRNode::For { .. } => {
            expr_block_stmt(node)
        }
        IRNode::Loop { body, .. } => {
            let neutral = TypeRef::Unit;
            format!(
                "loop {{\n{}}}",
                ir_to_rust(body, 0, &neutral).trim_end()
            )
        }
        IRNode::StructDef { name, fields, body } => {
            // An item definition followed by the body that uses
            // it — Rust allows item declarations inside a block.
            let fields_str: Vec<String> = fields
                .iter()
                .map(|(n, t)| format!("{}: {}", rust_ident(n), ty_to_rust(t)))
                .collect();
            format!(
                "{{ struct {} {{ {} }} {} }}",
                rust_ident(name),
                fields_str.join(", "),
                ir_to_rust_expr(body),
            )
        }
        IRNode::Index(coll, idx) => {
            // Phase 85 (M1 generation campaign, Leg 1 — a
            // new arm case, the corpus shapes below it
            // unchanged): indexing a STRING is scalar char
            // access in this language (the interpreter's
            // chars().nth(); the Z3 encoder's CharStr
            // bridge, Phase 76). The walker carries no type
            // information, so the string case is keyed on
            // the base's shape — a record's field projection
            // or a string literal, the string-shaped
            // positions (peek's `state.source[state.pos]`
            // is the measured carrier; it died E0277 under
            // the bracket form). The index is cast to
            // usize, and an out-of-bounds index totalizes
            // to the NUL sentinel — the Rust-side analogue
            // of the bridge's documented totalization (an
            // out-of-bounds indexed char denotes the empty
            // CharStr, which no real char equals in the
            // comparisons the bridge supports; NUL is the
            // sentinel a generated body can name — a
            // genuine NUL in a source at an unguarded index
            // is the recorded collision). Every other base
            // shape keeps the standing bracket rendering,
            // byte-identical (the M3 corpus pins it for Var
            // and Call bases); a List-typed record field in
            // the projection position would take the char
            // form and fail loudly at rustc, the same named
            // boundary as the len arm's.
            match coll.as_ref() {
                IRNode::FieldAccess { .. } | IRNode::Literal(IRLiteral::String(_)) => {
                    format!(
                        "({}).chars().nth(({}) as usize).unwrap_or('\\0')",
                        ir_to_rust_expr(coll),
                        ir_to_rust_expr(idx)
                    )
                }
                _ => format!("({})[{}]", ir_to_rust_expr(coll), ir_to_rust_expr(idx)),
            }
        }
        IRNode::TryPropagate(inner) => {
            format!("({})?", ir_to_rust_expr(inner))
        }
        IRNode::Assign { target, value } => {
            format!(
                "({} = {})",
                ir_to_rust_expr(target),
                ir_to_rust_expr(value)
            )
        }
        // `return`/`break` greedily consume a following
        // expression, so they are parenthesized to keep the IR's
        // operand boundary: `(return 5) + 1` must not become
        // `return (5 + 1)`.
        IRNode::Return(inner) => {
            format!("(return {})", ir_to_rust_expr(inner))
        }
        IRNode::Break { value } => match value {
            Some(v) => format!("(break {})", ir_to_rust_expr(v)),
            None => "(break)".into(),
        },
        IRNode::Hole { .. } => "panic!(\"Hole not filled\")".into(),
        // An Alloc binds a name for later statements; it has no
        // value of its own, so there is no honest expression
        // form — the loud fallback applies.
        IRNode::Alloc { .. } => unsupported_expr("Alloc"),
        // M1 features (Phase 77, decisions D2a/D3a — the
        // scoped unfreeze: these two arms replace the
        // Phase 73 loud fallback; no existing arm moves).
        // Field projection is Rust's own `.field`; the
        // base renders through the condition-position
        // discipline (block-like bases parenthesized),
        // and a record-literal base is parenthesized too
        // — a bare struct literal cannot head a projection
        // in the scrutinee/condition positions this
        // emitter feeds.
        IRNode::FieldAccess { base, field } => {
            let base_text = match base.as_ref() {
                IRNode::RecordLit { .. } => format!("({})", ir_to_rust_expr(base)),
                _ => expr_in_condition_position(base),
            };
            format!("{}.{}", base_text, rust_ident(field))
        }
        // Record construction: the Rust struct literal,
        // fields in written order, in the StructDef arm's
        // spacing convention. A bare string-literal field
        // value takes the owned form — the field's
        // declared type is the spec's (String maps to
        // String), the defect-4d rule at the position
        // this arm controls, exactly as the spec-Expr
        // RecordLit arm.
        IRNode::RecordLit { type_name, fields } => {
            let render_value = |v: &IRNode| -> String {
                match v {
                    IRNode::Literal(IRLiteral::String(s)) => {
                        format!("\"{}\".to_string()", escape_rust_str(s))
                    }
                    // Phase 85: a field projection is
                    // not a fresh value — rendered bare
                    // it MOVES the field out of the base
                    // record, and a later field of the
                    // same literal that projects the
                    // same base again (next_char's
                    // inlined peek guards re-read
                    // `state.source` after the `source`
                    // field) dies E0382. The Phase 39
                    // Concat-Var treatment, at the one
                    // position this arm controls: an
                    // owned copy. The IR is pure, so the
                    // clone is value-transparent.
                    IRNode::FieldAccess { .. } => {
                        format!("{}.clone()", ir_to_rust_expr(v))
                    }
                    _ => ir_to_rust_expr(v),
                }
            };
            // Phase 87 (scoped unfreeze — a variant
            // rendering of this arm only): the
            // struct-update form, `R { <computed>, ..base }`,
            // when the literal carries two or more fields
            // straight from one base variable and every
            // computed field's use of that base is a
            // projection of a non-carried field (see
            // fru_shape). The Phase 85 clone-per-carry
            // rendering pays a full allocation-sized
            // clone per carried field per construction —
            // for next_char, the state String + token Vec
            // on every character — and the Phase 87
            // profile priced exactly that as the
            // benchmark's largest remaining term. The
            // update form moves the carried fields
            // instead; the computed fields render exactly
            // as above, so the value is unchanged.
            if let Some((base, carried)) = fru_shape(fields) {
                let mut parts: Vec<String> = fields
                    .iter()
                    .filter(|(n, _)| !carried.iter().any(|c| c == n))
                    .map(|(n, v)| format!("{}: {}", rust_ident(n), render_value(v)))
                    .collect();
                parts.push(format!("..{}", rust_ident(base)));
                return format!("{} {{ {} }}", rust_ident(type_name), parts.join(", "));
            }
            let fields_str: Vec<String> = fields
                .iter()
                .map(|(n, v)| format!("{}: {}", rust_ident(n), render_value(v)))
                .collect();
            format!("{} {{ {} }}", rust_ident(type_name), fields_str.join(", "))
        }
        IRNode::Typed { node, ty } => ir_to_rust_expr_typed(node, Some(ty)),
    }
}

fn ir_pattern_to_rust(pattern: &IRPattern) -> String {
    match pattern {
        IRPattern::Wildcard => "_".into(),
        IRPattern::Variable(name) => rust_ident(name),
        IRPattern::Literal(lit) => match lit {
            IRLiteral::Int(n) => n.to_string(),
            IRLiteral::Bool(b) => b.to_string(),
            IRLiteral::String(s) => format!("\"{}\"", escape_rust_str(s)),
            _ => "_".into(),
        },
        IRPattern::Constructor(name, sub_patterns) => {
            let subs: Vec<String> = sub_patterns.iter().map(ir_pattern_to_rust).collect();
            if subs.is_empty() {
                rust_ident(name)
            } else {
                format!("{}({})", rust_ident(name), subs.join(", "))
            }
        }
        IRPattern::Guard(pat, guard) => {
            format!("{} if {}", ir_pattern_to_rust(pat), guard)
        }
    }
}

// ── IR → C Translation ────────────────────────────────────

fn ir_to_c(node: &IRNode, indent: usize) -> String {
    let pad = "    ".repeat(indent);
    match node {
        IRNode::Return(expr) => {
            format!("{}return {};\n", pad, ir_to_c_expr(expr))
        }
        IRNode::Block(stmts) => {
            let mut out = String::new();
            for stmt in stmts {
                out.push_str(&ir_to_c(stmt, indent));
            }
            out
        }
        IRNode::If { condition, then_branch, else_branch } => {
            format!(
                "{}if ({}) {{\n{}}} else {{\n{}}}\n",
                pad,
                ir_to_c_expr(condition),
                ir_to_c(then_branch, indent + 1),
                ir_to_c(else_branch, indent + 1),
            )
        }
        IRNode::While { condition, body, .. } => {
            format!(
                "{}while ({}) {{\n{}{}}}\n",
                pad,
                ir_to_c_expr(condition),
                ir_to_c(body, indent + 1),
                pad,
            )
        }
        IRNode::For { var, start, end, body } => {
            format!(
                "{}for (int {} = {}; {} < {}; {}++) {{\n{}{}}}\n",
                pad, var,
                ir_to_c_expr(start),
                var, ir_to_c_expr(end),
                var,
                ir_to_c(body, indent + 1),
                pad,
            )
        }
        _ => format!("{}/* {} */;\n", pad, "unsupported"),
    }
}

fn ir_to_c_expr(node: &IRNode) -> String {
    match node {
        IRNode::Literal(lit) => match lit {
            IRLiteral::Int(n) => n.to_string(),
            IRLiteral::Bool(b) => if *b { "1".into() } else { "0".into() },
            _ => "0".into(),
        },
        IRNode::Var(name) => name.clone(),
        IRNode::BinOp(op, lhs, rhs) => {
            let c_op = match op {
                IRBinOp::Add => "+", IRBinOp::Sub => "-",
                IRBinOp::Mul => "*", IRBinOp::Div => "/",
                IRBinOp::Eq => "==", IRBinOp::Neq => "!=",
                IRBinOp::Lt => "<", IRBinOp::Gt => ">",
                IRBinOp::And => "&&", IRBinOp::Or => "||",
                _ => "?",
            };
            format!("({} {} {})", ir_to_c_expr(lhs), c_op, ir_to_c_expr(rhs))
        }
        // M1 features (Phase 77): C's record concept is
        // the struct — generate_c carries the record
        // typedef rendering — so projection is member
        // access and construction is a C99 compound
        // literal with designated initializers, fields
        // in written order.
        IRNode::FieldAccess { base, field } => {
            format!("{}.{}", ir_to_c_expr(base), field)
        }
        IRNode::RecordLit { type_name, fields } => {
            let fields_str: Vec<String> = fields
                .iter()
                .map(|(n, v)| format!(".{} = {}", n, ir_to_c_expr(v)))
                .collect();
            format!("({}){{ {} }}", type_name, fields_str.join(", "))
        }
        _ => "0".into(),
    }
}

// ── IR → WAT (WebAssembly) Translation ────────────────────

fn ir_to_wasm(node: &IRNode) -> String {
    let mut out = String::new();
    match node {
        IRNode::Block(stmts) => {
            for stmt in stmts {
                out.push_str(&ir_to_wasm(stmt));
            }
        }
        IRNode::Return(expr) => {
            out.push_str(&format!("    {}\n", ir_to_wasm_expr(expr)));
            out.push_str("    return\n");
        }
        // M1 features (Phase 77): no aggregate concept
        // exists in the emitted WAT (see ir_to_wasm_expr),
        // so both nodes keep the standing statement
        // fallback — an explicit arm, byte-identical to
        // the catch-all, recording the choice at the site.
        IRNode::FieldAccess { .. } | IRNode::RecordLit { .. } => {
            out.push_str(&format!("    ;; {}\n", "node"));
        }
        _ => {
            out.push_str(&format!("    ;; {}\n", "node"));
        }
    }
    out
}

fn ir_to_wasm_expr(node: &IRNode) -> String {
    match node {
        IRNode::Literal(lit) => match lit {
            IRLiteral::Int(n) => format!("i32.const {}", n),
            IRLiteral::Bool(b) => format!("i32.const {}", if *b { 1 } else { 0 }),
            _ => "i32.const 0".into(),
        },
        IRNode::BinOp(op, lhs, rhs) => {
            let wasm_op = match op {
                IRBinOp::Add => "i32.add",
                IRBinOp::Sub => "i32.sub",
                IRBinOp::Mul => "i32.mul",
                _ => "drop",
            };
            format!(
                "{}\n    {}\n    {}",
                ir_to_wasm_expr(lhs),
                ir_to_wasm_expr(rhs),
                wasm_op,
            )
        }
        IRNode::Var(name) => format!("local.get ${}", name),
        // M1 features (Phase 77): the emitted WAT is a
        // scalar model — params/results are i32/f64, no
        // memory or struct types are ever declared — so
        // neither new node has a target concept; both
        // keep the standing expression fallback, as an
        // explicit arm byte-identical to the catch-all.
        IRNode::FieldAccess { .. } | IRNode::RecordLit { .. } => "i32.const 0".into(),
        _ => "i32.const 0".into(),
    }
}

// ── IR → Python Translation ───────────────────────────────

fn ir_to_python(node: &IRNode, indent: usize) -> String {
    let pad = "    ".repeat(indent);
    match node {
        IRNode::Return(expr) => {
            format!("{}return {}\n", pad, ir_to_python_expr(expr))
        }
        IRNode::Block(stmts) => {
            let mut out = String::new();
            for stmt in stmts {
                out.push_str(&ir_to_python(stmt, indent));
            }
            out
        }
        IRNode::If { condition, then_branch, else_branch } => {
            format!(
                "{}if {}:\n{}else:\n{}\n",
                pad,
                ir_to_python_expr(condition),
                ir_to_python(then_branch, indent + 1),
                ir_to_python(else_branch, indent + 1),
            )
        }
        IRNode::For { var, start, end, body } => {
            format!(
                "{}for {} in range({}, {}):\n{}{}\n",
                pad, var,
                ir_to_python_expr(start),
                ir_to_python_expr(end),
                ir_to_python(body, indent + 1),
                pad,
            )
        }
        _ => format!("{}pass  # {}\n", pad, "unsupported"),
    }
}

fn ir_to_python_expr(node: &IRNode) -> String {
    match node {
        IRNode::Literal(lit) => match lit {
            IRLiteral::Int(n) => n.to_string(),
            IRLiteral::Bool(b) => if *b { "True".into() } else { "False".into() },
            IRLiteral::String(s) => format!("\"{}\"", s),
            _ => "None".into(),
        },
        IRNode::Var(name) => name.clone(),
        IRNode::BinOp(op, lhs, rhs) => {
            let py_op = match op {
                IRBinOp::Add => "+", IRBinOp::Sub => "-",
                IRBinOp::Mul => "*", IRBinOp::Div => "/",
                IRBinOp::Eq => "==", IRBinOp::Neq => "!=",
                IRBinOp::And => "and", IRBinOp::Or => "or",
                _ => "?",
            };
            format!("({} {} {})", ir_to_python_expr(lhs), py_op, ir_to_python_expr(rhs))
        }
        // M1 features (Phase 77): this emitter generates
        // no class or dataclass definitions for record
        // types (ty_to_python maps a named type to Any),
        // so the declaration-free record value is a dict
        // and projection is its subscript — the
        // self-consistent pair. The literal's type name
        // has no runtime form in this model.
        IRNode::FieldAccess { base, field } => {
            format!("{}[\"{}\"]", ir_to_python_expr(base), field)
        }
        IRNode::RecordLit { fields, .. } => {
            let fields_str: Vec<String> = fields
                .iter()
                .map(|(n, v)| format!("\"{}\": {}", n, ir_to_python_expr(v)))
                .collect();
            format!("{{{}}}", fields_str.join(", "))
        }
        _ => "None".into(),
    }
}

// ── IR → JavaScript Translation ───────────────────────────

fn ir_to_javascript(node: &IRNode, indent: usize) -> String {
    let pad = "    ".repeat(indent);
    match node {
        IRNode::Return(expr) => {
            format!("{}return {};\n", pad, ir_to_js_expr(expr))
        }
        IRNode::Block(stmts) => {
            let mut out = String::new();
            for stmt in stmts {
                out.push_str(&ir_to_javascript(stmt, indent));
            }
            out
        }
        IRNode::If { condition, then_branch, else_branch } => {
            format!(
                "{}if ({}) {{\n{}}} else {{\n{}}}\n",
                pad,
                ir_to_js_expr(condition),
                ir_to_javascript(then_branch, indent + 1),
                ir_to_javascript(else_branch, indent + 1),
            )
        }
        IRNode::For { var, start, end, body } => {
            format!(
                "{}for (let {} = {}; {} < {}; {}++) {{\n{}{}}}\n",
                pad, var,
                ir_to_js_expr(start),
                var, ir_to_js_expr(end),
                var,
                ir_to_javascript(body, indent + 1),
                pad,
            )
        }
        _ => format!("{}// {}\n", pad, "unsupported"),
    }
}

fn ir_to_js_expr(node: &IRNode) -> String {
    match node {
        IRNode::Literal(lit) => match lit {
            IRLiteral::Int(n) => n.to_string(),
            IRLiteral::Float(f) => f.to_string(),
            IRLiteral::Bool(b) => b.to_string(),
            IRLiteral::String(s) => format!("'{}'", s),
            IRLiteral::Char(c) => format!("'{}'", c),
            IRLiteral::Unit => "undefined".into(),
        },
        IRNode::Var(name) => name.clone(),
        IRNode::BinOp(op, lhs, rhs) => {
            let js_op = match op {
                IRBinOp::Add => "+", IRBinOp::Sub => "-",
                IRBinOp::Mul => "*", IRBinOp::Div => "/",
                IRBinOp::Eq => "===", IRBinOp::Neq => "!==",
                IRBinOp::Lt => "<", IRBinOp::Gt => ">",
                IRBinOp::And => "&&", IRBinOp::Or => "||",
                _ => "?",
            };
            format!("({} {} {})", ir_to_js_expr(lhs), js_op, ir_to_js_expr(rhs))
        }
        // M1 features (Phase 77): object literals and
        // property access are native and
        // declaration-free — the exact record model. The
        // literal's type name has no runtime form in it.
        IRNode::FieldAccess { base, field } => {
            format!("{}.{}", ir_to_js_expr(base), field)
        }
        IRNode::RecordLit { fields, .. } => {
            let fields_str: Vec<String> = fields
                .iter()
                .map(|(n, v)| format!("{}: {}", n, ir_to_js_expr(v)))
                .collect();
            format!("{{ {} }}", fields_str.join(", "))
        }
        _ => "undefined".into(),
    }
}

// ── Auto-acquire types from spec ──────────────────────────

impl FunctionSpec {
    fn types(&self) -> Vec<TypeDef> {
        // Extract referenced type definitions from the spec context
        // In production, these come from the full Spec module
        Vec::new()
    }
}

// ── Type Translators ──────────────────────────────────────

fn ty_to_rust(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Int => "i64".into(),
        TypeRef::Float => "f64".into(),
        TypeRef::Bool => "bool".into(),
        TypeRef::String => "String".into(),
        TypeRef::Char => "char".into(),
        TypeRef::Unit => "()".into(),
        TypeRef::I8 => "i8".into(), TypeRef::I16 => "i16".into(),
        TypeRef::I32 => "i32".into(), TypeRef::I64 => "i64".into(),
        TypeRef::U8 => "u8".into(), TypeRef::U16 => "u16".into(),
        TypeRef::U32 => "u32".into(), TypeRef::U64 => "u64".into(),
        TypeRef::List(t) => format!("Vec<{}>", ty_to_rust(t)),
        TypeRef::Set(t) => format!("HashSet<{}>", ty_to_rust(t)),
        TypeRef::Map(k, v) => format!("HashMap<{}, {}>", ty_to_rust(k), ty_to_rust(v)),
        TypeRef::Option(t) => format!("Option<{}>", ty_to_rust(t)),
        TypeRef::Result(ok, err) => format!("Result<{}, {}>", ty_to_rust(ok), ty_to_rust(err)),
        TypeRef::Tuple(ts) => {
            let parts: Vec<String> = ts.iter().map(ty_to_rust).collect();
            format!("({})", parts.join(", "))
        }
        TypeRef::Array(t, n) => format!("[{}; {}]", ty_to_rust(t), n),
        TypeRef::Ref(t) => format!("&{}", ty_to_rust(t)),
        TypeRef::Named(name) | TypeRef::Generic(name) => rust_ident(name),
        TypeRef::Function(args, ret) => {
            let args_str: Vec<String> = args.iter().map(ty_to_rust).collect();
            format!("fn({}) -> {}", args_str.join(", "), ty_to_rust(ret))
        }
        TypeRef::Stream(t) => format!("impl Iterator<Item = {}>", ty_to_rust(t)),
    }
}

fn ty_to_c(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Int | TypeRef::I32 => "int".into(),
        TypeRef::I64 | TypeRef::U64 => "int64_t".into(),
        TypeRef::Float => "double".into(),
        TypeRef::Bool => "bool".into(),
        TypeRef::String => "const char*".into(),
        TypeRef::Char => "char".into(),
        TypeRef::Unit => "void".into(),
        TypeRef::List(t) => format!("{}*", ty_to_c(t)),
        TypeRef::Named(name) => name.clone(),
        _ => "void*".into(),
    }
}

fn ty_to_wasm(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Int | TypeRef::I32 | TypeRef::I64 => "i32".into(),
        TypeRef::Float => "f64".into(),
        TypeRef::Bool => "i32".into(),
        TypeRef::Unit => "".into(),
        _ => "i32".into(),
    }
}

fn ty_to_python(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Int => "int".into(),
        TypeRef::Float => "float".into(),
        TypeRef::Bool => "bool".into(),
        TypeRef::String => "str".into(),
        TypeRef::List(t) => format!("list[{}]", ty_to_python(t)),
        TypeRef::Option(t) => format!("Optional[{}]", ty_to_python(t)),
        _ => "Any".into(),
    }
}

fn constraint_to_rust(constraint: &Constraint) -> String {
    match constraint {
        Constraint::True => "true".into(),
        Constraint::Not(inner) => format!("!({})", constraint_to_rust(inner)),
        Constraint::Eq(a, b) => format!("{} == {}", expr_to_rust(a), expr_to_rust(b)),
        Constraint::Order(OrderOp::LessThan, a, b) => format!("{} < {}", expr_to_rust(a), expr_to_rust(b)),
        Constraint::Order(OrderOp::LessThanOrEqual, a, b) => format!("{} <= {}", expr_to_rust(a), expr_to_rust(b)),
        Constraint::And(parts) => {
            let p: Vec<String> = parts.iter().map(constraint_to_rust).collect();
            format!("({})", p.join(" && "))
        }
        _ => "true".into(),
    }
}

fn expr_to_rust(expr: &Expr) -> String {
    match expr {
        Expr::IntLit(n) => n.to_string(),
        Expr::BoolLit(b) => b.to_string(),
        Expr::StringLit(s) => format!("\"{}\"", escape_rust_str(s)),
        Expr::CharLit(c) => format!("'{}'", escape_rust_char(*c)),
        Expr::Var(name) => rust_ident(name),
        Expr::Call(name, args) => {
            let args_str: Vec<String> = args.iter().map(expr_to_rust).collect();
            // The parser's pseudo-calls (rust_expr_call_form,
            // Phase 55): the list literal renders as the vec
            // literal, the tuple literal as a Rust tuple, and
            // the zero-argument None as the bare constructor.
            // Every other call keeps the plain call template.
            match name.as_str() {
                "List" => format!("vec![{}]", args_str.join(", ")),
                "__tuple__" => format!("({})", args_str.join(", ")),
                "None" => "None".to_string(),
                _ => format!("{}({})", rust_ident(name), args_str.join(", ")),
            }
        }
        Expr::BinOp(op, lhs, rhs) => {
            let rust_op = match op {
                BinOp::Add => "+", BinOp::Sub => "-",
                BinOp::Mul => "*", BinOp::Div => "/",
                BinOp::Mod => "%",
                BinOp::Eq => "==", BinOp::Neq => "!=",
                BinOp::And => "&&", BinOp::Or => "||",
                BinOp::Lt => "<", BinOp::Le => "<=",
                BinOp::Gt => ">", BinOp::Ge => ">=",
                BinOp::Concat => "+",
            };
            format!("({} {} {})", expr_to_rust(lhs), rust_op, expr_to_rust(rhs))
        }
        // The closure template (rust_expr_lambda_text,
        // Phase 55): parameter names only — the generated
        // closure is unannotated, as in the IR walker's
        // Lambda arm.
        Expr::Lambda(params, body) => {
            let names: Vec<String> = params
                .iter()
                .map(|(name, _)| rust_ident(name))
                .collect();
            format!("|{}| {}", names.join(", "), expr_to_rust(body))
        }
        // Field projection (Phase 78 — the lockstep arm
        // the Phase 77 record deferred to the Codegen spec
        // amendments, which move both sides together):
        // Rust's own `.field` on the rendered receiver,
        // exactly the rust_expr_field_text template. This
        // walker carries no condition-position discipline
        // (that is the IR walker's), so the receiver
        // renders as it renders in every other position
        // here.
        Expr::Field(base, field) => {
            format!("{}.{}", expr_to_rust(base), rust_ident(field))
        }
        // Method calls (Phase 77, decision D1a — scoped
        // unfreeze, new arm only): the four closed-table
        // builtins render as the identical Rust method
        // syntax on the rendered receiver. Anything else
        // — a name outside the table, or a table name
        // carrying arguments (the builtins are
        // zero-argument; both shapes are Phase 74 type
        // errors, so a checked module never reaches this)
        // — renders the standing catch-all, never a guess.
        Expr::MethodCall { receiver, name, args } => {
            match name.as_str() {
                "is_ok" | "is_err" | "unwrap" | "len" if args.is_empty() => {
                    format!("{}.{}()", expr_to_rust(receiver), name)
                }
                _ => "??".into(),
            }
        }
        // Record literals (Phase 77, decision D3a — new
        // arm only): the Rust struct-literal form, fields
        // in written order, in the StructDef arm's spacing
        // convention. One expectation the arm itself
        // knows: a record field's declared type comes
        // from the spec (ty_to_rust maps String to
        // String), so a bare string-literal field value
        // takes the owned form — the defect-4d rule
        // (Phase 29) applied at the one position this
        // arm controls; deeper expressions keep the
        // standing rendering, as everywhere else here.
        Expr::RecordLit { type_name, fields } => {
            let fields_str: Vec<String> = fields
                .iter()
                .map(|(n, v)| {
                    let value_text = match v {
                        Expr::StringLit(s) => {
                            format!("\"{}\".to_string()", escape_rust_str(s))
                        }
                        _ => expr_to_rust(v),
                    };
                    format!("{}: {}", rust_ident(n), value_text)
                })
                .collect();
            format!("{} {{ {} }}", rust_ident(type_name), fields_str.join(", "))
        }
        _ => "??".into(),
    }
}

// ── Debug IR ──────────────────────────────────────────────

fn debug_ir_string(node: &IRNode, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match node {
        IRNode::Hole { id, .. } => format!("{}Hole({})\n", pad, id),
        IRNode::Block(stmts) => {
            let mut out = format!("{}Block [\n", pad);
            for s in stmts { out.push_str(&debug_ir_string(s, indent + 1)); }
            out.push_str(&format!("{}]\n", pad));
            out
        }
        IRNode::Return(expr) => format!("{}Return\n{}", pad, debug_ir_string(expr, indent + 1)),
        IRNode::Var(name) => format!("{}Var({})\n", pad, name),
        IRNode::Literal(lit) => format!("{}Literal({:?})\n", pad, lit),
        IRNode::Call { function, args } => {
            let mut out = format!("{}Call({})\n", pad, function);
            for a in args { out.push_str(&debug_ir_string(a, indent + 1)); }
            out
        }
        _ => format!("{}{}\n", pad, format!("{:?}", std::mem::discriminant(node))),
    }
}

// ── Error Type ────────────────────────────────────────────

#[derive(Debug)]
pub enum CodegenError {
    UnsupportedTarget(String),
    InvalidIR(String),
    InternalError(String),
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::UnsupportedTarget(t) => write!(f, "Unsupported target language: {}", t),
            CodegenError::InvalidIR(msg) => write!(f, "Invalid IR: {}", msg),
            CodegenError::InternalError(msg) => write!(f, "Codegen error: {}", msg),
        }
    }
}

impl std::error::Error for CodegenError {}

// ── Tests ─────────────────────────────────────────────────

/// The M5-S1 bootstrap artifact (Phase 51): the M3 Codegen
/// module's 112 synthesized winners, checked in as a versioned
/// file and compiled into the crate — the compiled codegen the
/// bootstrap adapter calls. GENERATED FILE (its own header
/// carries the regeneration command and provenance). Since
/// M5-S2 (Phase 52) the adapter that calls it sits in
/// `generate`'s "rust" route behind the `bootstrap-codegen`
/// Cargo feature; with the feature off, no production path
/// reaches this module.
pub(crate) mod synthesized_codegen;

/// The M5-S1 bootstrap adapter (Phase 51): the M3 gate's
/// composed emitter transcribed from IR-interpreter calls to
/// direct calls into `synthesized_codegen`'s compiled winner
/// functions, plus the audit's 9-item shim inventory as
/// ordinary Rust helpers. Since M5-S2 (Phase 52) it is wired
/// into `generate`'s "rust" route behind the Cargo feature
/// `bootstrap-codegen` (see `generate_rust_route`); with the
/// feature off, the original emitter renders and this module
/// is reached only from tests. (dead_code allowed: in a
/// feature-off production build nothing calls it — the
/// ROAD's keep-both rule.)
#[allow(dead_code)]
pub(crate) mod bootstrap_adapter;

/// M5-S1 proof (Phase 51): the checked-in bootstrap artifact's
/// byte-identity against a fresh gate-style extraction +
/// render, and the compiled adapter's equivalence with the
/// original emitter (and with the interpreter composition it
/// transcribes) over the M3 gate corpus and the M4 whole-file
/// corpus. Test-only; reuses the M3 gate's and M4 harness's
/// machinery by reference.
#[cfg(test)]
mod m5_s1;

/// M5-S2 (Phase 52): the feature-flag switch's routing,
/// pinned in both configurations, and the M5 roundtrip in
/// the Phase 50 audit's chosen form — (a) the extract-once
/// byte comparison (one pipeline extraction per standalone
/// file; that winner rendered by the original emitter and by
/// the feature-on dispatch path, byte-compared) and (b) the
/// process-level behavioral roundtrip (the binary built with
/// and without `bootstrap-codegen`; each builds sort.morph
/// repeatedly, every drawn winner function compiled and
/// executed against sort's in-spec tests). Test-only.
#[cfg(test)]
mod m5_s2;

/// M5-S4 (Phase 64): the SC3 recorded measurement for the M5
/// gate — the codegen render (the only stage that differs
/// between the configurations) timed for the original emitter
/// vs the compiled adapter over the M3 gate corpus and the M4
/// whole-file corpus, median of interleaved rounds, per the
/// Phase 50 audit's design (item E) and owner decision D3
/// (a recorded measurement with a mandatory baseline, not a
/// hard gate item). Test-only.
#[cfg(test)]
mod m5_s4;

/// The M3 gate harness (Phase 42): composes the Codegen
/// module's 112 synthesized winners through the assembly
/// structure of tests/m3_proof.rs and byte-compares the
/// composed codegen against this module's original emitter
/// functions over an adversarial corpus. Test-only; it lives
/// inside codegen so the harness can call the emitter's
/// private functions — the original side of the comparison
/// is the real thing, not a copy.
#[cfg(test)]
mod m3_gate;

/// M4-S1 composed-compiler harness (Phase 44): generalizes
/// the M3 gate's composition into a reusable build-path
/// instrument — one winner extraction per spec through the
/// real pipeline, that single winner rendered by the
/// original `generate` AND by the composed codegen
/// (m3_gate's mirror-completed composition), byte-compared
/// as whole files. Test-only; it lives inside codegen next
/// to the M3 gate whose machinery it reuses.
#[cfg(test)]
mod m4_harness;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::ast::{Expr, Param, Test};
    use crate::synthesis::engine::{Provenance, ScoreBreakdown};
    use std::collections::HashMap;

    fn candidate(spec_name: &str, body: IRNode) -> CandidateImplementation {
        CandidateImplementation {
            id: 1,
            body,
            spec_name: spec_name.into(),
            spec: FunctionSpec {
                name: spec_name.into(),
                doc: None,
                generics: vec![],
                params: vec![Param {
                    name: "kind".into(),
                    ty: TypeRef::Int,
                    annotations: HashMap::new(),
                }],
                return_type: TypeRef::Bool,
                preconditions: vec![],
                postconditions: vec![],
                invariants: vec![],
                complexity: None,
                resource: None,
                tests: vec![],
                annotations: HashMap::new(),
            },
            score: 1.0,
            scores: ScoreBreakdown {
                test_pass_ratio: 1.0,
                constraint_score: 1.0,
                complexity_score: 1.0,
                quality_score: 1.0,
                synthesis_time_us: 0,
            },
            generation: 0,
            provenance: Provenance::HumanGuided,
            verified: true,
            verification_status: None,
        }
    }

    fn spec_test(name: &str) -> Test {
        Test {
            name: Some(name.into()),
            input: Expr::IntLit(0),
            expected_output: Expr::BoolLit(true),
            timeout_ms: None,
            property: false,
        }
    }

    // ── Phase 61 (G6): the test-call turbofish rule ──────

    fn generic_param(name: &str) -> crate::spec::ast::GenericParam {
        crate::spec::ast::GenericParam {
            name: name.into(),
            bounds: vec![],
        }
    }

    fn named(name: &str) -> TypeRef {
        TypeRef::Named(name.into())
    }

    fn sort_shaped_spec() -> FunctionSpec {
        FunctionSpec {
            name: "sort".into(),
            doc: None,
            generics: vec![generic_param("T")],
            params: vec![
                Param {
                    name: "list".into(),
                    ty: TypeRef::List(Box::new(named("T"))),
                    annotations: HashMap::new(),
                },
                Param {
                    name: "cmp".into(),
                    ty: TypeRef::Function(
                        vec![named("T"), named("T")],
                        Box::new(TypeRef::Bool),
                    ),
                    annotations: HashMap::new(),
                },
            ],
            return_type: TypeRef::List(Box::new(named("T"))),
            preconditions: vec![],
            postconditions: vec![],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: HashMap::new(),
        }
    }

    fn tuple_test(name: &str, args: Vec<Expr>, expected: Expr) -> Test {
        Test {
            name: Some(name.into()),
            input: Expr::Call("__tuple__".into(), args),
            expected_output: expected,
            timeout_ms: None,
            property: false,
        }
    }

    fn lt_lambda() -> Expr {
        Expr::Lambda(
            vec![("a".into(), named("T")), ("b".into(), named("T"))],
            Box::new(Expr::BinOp(
                crate::spec::ast::BinOp::Lt,
                Box::new(Expr::Var("a".into())),
                Box::new(Expr::Var("b".into())),
            )),
        )
    }

    fn list_lit(elems: Vec<Expr>) -> Expr {
        Expr::Call("List".into(), elems)
    }

    #[test]
    fn generics_text_annotates_only_the_unpinned_empty_collection_test() {
        let mut spec = sort_shaped_spec();
        let empty = tuple_test(
            "empty",
            vec![list_lit(vec![]), lt_lambda()],
            list_lit(vec![]),
        );
        let single = tuple_test(
            "single",
            vec![list_lit(vec![Expr::IntLit(5)]), lt_lambda()],
            list_lit(vec![Expr::IntLit(5)]),
        );
        spec.tests = vec![empty.clone(), single.clone()];
        // The empty list leaves T unpinned (the lambda
        // cannot pin it): annotate, with the concrete type
        // derived from the spec's own tests (the [5] test).
        assert_eq!(test_call_generics_text(&spec, &empty), "::<i64>");
        // A non-empty list pins T itself: no annotation,
        // byte-identical rendering.
        assert_eq!(test_call_generics_text(&spec, &single), "");
    }

    #[test]
    fn generics_text_skips_specs_the_rule_does_not_cover() {
        // binary_search's shape: the needle argument sits
        // at a bare-generic position and its literal pins
        // T even for the empty-list test — no annotation.
        let mut spec = sort_shaped_spec();
        spec.name = "binary_search".into();
        spec.params = vec![
            Param {
                name: "haystack".into(),
                ty: TypeRef::List(Box::new(named("T"))),
                annotations: HashMap::new(),
            },
            Param {
                name: "needle".into(),
                ty: named("T"),
                annotations: HashMap::new(),
            },
            Param {
                name: "cmp".into(),
                ty: TypeRef::Function(
                    vec![named("T"), named("T")],
                    Box::new(TypeRef::Int),
                ),
                annotations: HashMap::new(),
            },
        ];
        spec.return_type = TypeRef::Option(Box::new(TypeRef::I64));
        let empty = tuple_test(
            "empty",
            vec![list_lit(vec![]), Expr::IntLit(5), lt_lambda()],
            Expr::Var("None".into()),
        );
        spec.tests = vec![empty.clone()];
        assert_eq!(test_call_generics_text(&spec, &empty), "");

        // No fn-typed parameter: no annotation even with an
        // unpinned empty list.
        let mut spec2 = sort_shaped_spec();
        spec2.params.remove(1);
        let t = tuple_test("empty", vec![list_lit(vec![])], list_lit(vec![]));
        spec2.tests = vec![t.clone()];
        assert_eq!(test_call_generics_text(&spec2, &t), "");

        // No generics at all: never annotated.
        let mut spec3 = sort_shaped_spec();
        spec3.generics = vec![];
        let t3 = tuple_test(
            "empty",
            vec![list_lit(vec![]), lt_lambda()],
            list_lit(vec![]),
        );
        spec3.tests = vec![t3.clone()];
        assert_eq!(test_call_generics_text(&spec3, &t3), "");
    }

    #[test]
    fn generated_test_module_carries_the_annotation() {
        let mut cand = candidate("sort", IRNode::Var("list".into()));
        cand.spec = sort_shaped_spec();
        cand.spec.tests = vec![
            tuple_test(
                "empty",
                vec![list_lit(vec![]), lt_lambda()],
                list_lit(vec![]),
            ),
            tuple_test(
                "single",
                vec![list_lit(vec![Expr::IntLit(5)]), lt_lambda()],
                list_lit(vec![Expr::IntLit(5)]),
            ),
        ];
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("let result = sort::<i64>(vec![], |a, b| (a < b));"),
            "the empty test's call must be annotated:\n{}",
            code
        );
        assert!(
            code.contains("let result = sort(vec![5], |a, b| (a < b));"),
            "the pinned test's call must stay bare:\n{}",
            code
        );
    }

    // Regression (M2 synthesis run, defect 1): spec test names were
    // emitted verbatim as Rust fn identifiers — "empty-list" made
    // 12/14 generated winner files unparseable.
    #[test]
    fn rust_codegen_sanitizes_spec_and_test_names() {
        let mut cand = candidate("is-literal-kind", IRNode::Var("kind".into()));
        cand.spec.tests = vec![spec_test("empty-list"), spec_test("binary-not-literal")];
        let code = generate(&cand, "rust").unwrap();
        assert!(code.contains("pub fn is_literal_kind("), "spec name not sanitized:\n{}", code);
        assert!(code.contains("fn empty_list()"), "test name not sanitized:\n{}", code);
        assert!(code.contains("fn binary_not_literal()"), "test name not sanitized:\n{}", code);
        assert!(!code.contains("empty-list"), "raw hyphenated name leaked:\n{}", code);
        assert!(!code.contains("is-literal-kind"), "raw hyphenated name leaked:\n{}", code);
    }

    // Regression (M2 synthesis run, defect 2): Match arm bodies were
    // emitted as statements (`true;`), discarding the arm value — the
    // match then evaluated to `()` and the function failed with E0308.
    // This is the exact winner shape of `is_literal_kind`.
    #[test]
    fn rust_codegen_match_arms_yield_their_values() {
        let body = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(0)),
                    IRNode::Literal(IRLiteral::Bool(true)),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Bool(false)),
                ),
            ],
        };
        let cand = candidate("is_literal_kind", body);
        let code = generate(&cand, "rust").unwrap();
        assert!(!code.contains("true;"), "arm value discarded as statement:\n{}", code);
        assert!(!code.contains("false;"), "arm value discarded as statement:\n{}", code);
        assert!(code.contains("true\n"), "arm value not a tail expression:\n{}", code);
        assert!(code.contains("false\n"), "arm value not a tail expression:\n{}", code);
    }

    // Same defect, non-literal arm bodies: a BinOp arm and a Block arm
    // whose last statement is an expression must also yield values.
    #[test]
    fn rust_codegen_match_arms_yield_expression_and_block_values() {
        let body = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(0)),
                    IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(IRNode::Var("kind".into())),
                        Box::new(IRNode::Literal(IRLiteral::Int(1))),
                    ),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Block(vec![IRNode::Literal(IRLiteral::Int(7))]),
                ),
            ],
        };
        let mut cand = candidate("next_kind", body);
        cand.spec.return_type = TypeRef::Int;
        let code = generate(&cand, "rust").unwrap();
        assert!(code.contains("(kind + 1)\n"), "BinOp arm not a tail expression:\n{}", code);
        assert!(!code.contains("(kind + 1);"), "BinOp arm discarded:\n{}", code);
        assert!(code.contains("7\n"), "Block arm value not yielded:\n{}", code);
        assert!(!code.contains("7;"), "Block arm value discarded:\n{}", code);
    }

    // Regression (Phase 11 probe, runs 2–3): a Collection in
    // STATEMENT position was emitted tail-shaped (`vec![]` with
    // no `;`), so a winner whose body was Block([Collection,
    // Return, …]) — the exact `needs_parens` winner shape in two
    // of three runs — was a syntax error: "expected `;`, found
    // keyword `return`". A collection used as a statement must be
    // terminated like any other expression statement.
    #[test]
    fn rust_codegen_collection_statement_is_terminated() {
        let body = IRNode::Block(vec![
            IRNode::Collection {
                kind: CollectionKind::List,
                elements: vec![],
            },
            IRNode::Return(Box::new(IRNode::Var("kind".into()))),
        ]);
        let cand = candidate("needs_parens", body);
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("vec![];"),
            "collection statement not terminated:\n{}",
            code
        );
    }

    // The other half of the same defect family: a function BODY
    // is a tail position — its value is the return value. A bare
    // Collection body must keep its tail shape (no `;`), or a
    // List-returning winner stops compiling.
    #[test]
    fn rust_codegen_bare_collection_body_stays_tail() {
        let body = IRNode::Collection {
            kind: CollectionKind::List,
            elements: vec![IRNode::Literal(IRLiteral::Int(1))],
        };
        let mut cand = candidate("singleton", body);
        cand.spec.return_type = TypeRef::List(Box::new(TypeRef::Int));
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("vec![1]\n}"),
            "bare collection body not a tail expression:\n{}",
            code
        );
        assert!(
            !code.contains("vec![1];"),
            "bare collection body discarded as statement:\n{}",
            code
        );
    }

    // Same tail defect for a bare expression body (the Phase 10
    // `tags_compatible` winner shape, generalized): a BinOp body
    // was emitted `(actual + expected);` — a discarded statement,
    // so the function returned () and failed E0308. As the body
    // it must be the tail expression.
    #[test]
    fn rust_codegen_bare_expression_body_stays_tail() {
        let body = IRNode::BinOp(
            IRBinOp::Eq,
            Box::new(IRNode::Var("kind".into())),
            Box::new(IRNode::Literal(IRLiteral::Int(0))),
        );
        let cand = candidate("is_zero", body);
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("(kind == 0)\n"),
            "bare expression body not a tail expression:\n{}",
            code
        );
        assert!(
            !code.contains("(kind == 0);"),
            "bare expression body discarded as statement:\n{}",
            code
        );
    }

    // Regression (M2 synthesis run, defect 4d): `IRLiteral::String` was
    // emitted as a bare `&str` literal in tail position of functions
    // whose spec return type is `String` — E0308 in 4/14 winner files
    // (`binop_symbol`, `node_kind_name`, `print_node_kind`,
    // `type_tag_name`). This is the exact `type_tag_name` winner shape:
    // a Match body whose arms are string literals (direct, wrapped in a
    // Block, and wrapped in a Typed node), plus a Hole wildcard arm.
    #[test]
    fn rust_codegen_string_literals_owned_in_string_returning_match() {
        let body = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(1)),
                    IRNode::Literal(IRLiteral::String("Int".into())),
                ),
                (
                    IRPattern::Literal(IRLiteral::Int(4)),
                    IRNode::Block(vec![IRNode::Literal(IRLiteral::String(
                        "String".into(),
                    ))]),
                ),
                (
                    IRPattern::Literal(IRLiteral::Int(6)),
                    IRNode::Typed {
                        node: Box::new(IRNode::Literal(IRLiteral::String("List".into()))),
                        ty: TypeRef::String,
                    },
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Hole { id: 1, expected_type: None },
                ),
            ],
        };
        let mut cand = candidate("type_tag_name", body);
        cand.spec.return_type = TypeRef::String;
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("\"Int\".to_string()"),
            "direct literal arm not owned:\n{}", code
        );
        assert!(
            code.contains("\"String\".to_string()"),
            "block-wrapped literal arm not owned:\n{}", code
        );
        assert!(
            code.contains("\"List\".to_string()"),
            "Typed literal arm not owned:\n{}", code
        );
        assert!(
            !code.contains("\n            \"Int\"\n"),
            "bare &str literal still emitted as arm tail:\n{}", code
        );
    }

    // Same defect, `Return` position: a returned string literal in a
    // `String`-returning function must also be owned.
    #[test]
    fn rust_codegen_string_literal_owned_in_return() {
        let body = IRNode::Return(Box::new(IRNode::Literal(IRLiteral::String(
            "done".into(),
        ))));
        let mut cand = candidate("greeting", body);
        cand.spec.return_type = TypeRef::String;
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("return \"done\".to_string();"),
            "returned literal not owned:\n{}", code
        );
    }

    // No blanket conversion: where the expected type is NOT `String`,
    // string literals stay `&str` — converting everywhere would change
    // semantics for `&str` contexts.
    #[test]
    fn rust_codegen_string_literals_stay_str_without_string_expectation() {
        let body = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(1)),
                    IRNode::Literal(IRLiteral::String("Int".into())),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::String("Other".into())),
                ),
            ],
        };
        let cand = candidate("not_a_string_fn", body); // return_type: Bool
        let code = generate(&cand, "rust").unwrap();
        assert!(
            !code.contains(".to_string()"),
            "literal converted without a String expectation:\n{}", code
        );
        assert!(
            code.contains("\"Int\""),
            "literal missing entirely:\n{}", code
        );
    }

    // Sanitization must also guard Rust keywords and leading digits,
    // and apply to parameter names (declaration and use sites alike).
    #[test]
    fn rust_codegen_sanitizes_keywords_digits_and_params() {
        let mut cand = candidate("kw", IRNode::Var("type".into()));
        cand.spec.params = vec![Param {
            name: "type".into(),
            ty: TypeRef::Int,
            annotations: HashMap::new(),
        }];
        cand.spec.tests = vec![spec_test("match"), spec_test("3d case")];
        let code = generate(&cand, "rust").unwrap();
        assert!(code.contains("type_: i64"), "keyword param not sanitized:\n{}", code);
        assert!(code.contains("fn match_()"), "keyword test name not sanitized:\n{}", code);
        assert!(code.contains("fn _3d_case()"), "digit-led test name not sanitized:\n{}", code);
    }

    fn candidate_with(
        name: &str,
        params: Vec<(&str, TypeRef)>,
        return_type: TypeRef,
        body: IRNode,
    ) -> CandidateImplementation {
        let mut cand = candidate(name, body);
        cand.spec.params = params
            .into_iter()
            .map(|(n, ty)| Param {
                name: n.into(),
                ty,
                annotations: HashMap::new(),
            })
            .collect();
        cand.spec.return_type = return_type;
        cand
    }

    // ── Phase 13: emitter completeness ────────────────────
    //
    // The expression emitter's fallthrough arm emitted
    // `/* UNSUPPORTED */` for 19 of the 28 IRNode variants, and
    // two handled variants degraded to `UNKNOWN_OP` / `UNKNOWN_UNOP`
    // operands (BinOp Xor; UnaryOp Head/Tail/Init/Last). Any of
    // those in expression position (an If condition, a BinOp
    // operand, a Call argument, a Let value, …) made the whole
    // generated file unparseable — seen in a Phase 11 winner's
    // `if /* UNSUPPORTED */`. Every variant must render to a
    // parseable expression; shapes with no honest expression form
    // follow the Hole convention (`panic!(...)`, type `!`).

    fn assert_expr_renders(node: &IRNode) -> String {
        let out = ir_to_rust_expr(node);
        assert!(
            !out.trim().is_empty(),
            "empty expression emission for {:?}",
            node
        );
        assert!(
            !out.contains("UNSUPPORTED"),
            "UNSUPPORTED marker for {:?}: {}",
            node, out
        );
        assert!(
            !out.contains("UNKNOWN_OP") && !out.contains("UNKNOWN_UNOP"),
            "unknown-op marker for {:?}: {}",
            node, out
        );
        out
    }

    #[test]
    fn rust_expr_every_ir_variant_renders_without_markers() {
        let leaf = || Box::new(IRNode::Literal(IRLiteral::Int(1)));
        let bool_leaf = || Box::new(IRNode::Literal(IRLiteral::Bool(true)));
        let lambda = |name: &str| {
            IRNode::Lambda {
                params: vec![(name.into(), TypeRef::Int)],
                body: Box::new(IRNode::Var(name.into())),
            }
        };
        let variants: Vec<IRNode> = vec![
            IRNode::Hole { id: 7, expected_type: None },
            IRNode::Block(vec![IRNode::Literal(IRLiteral::Int(1))]),
            IRNode::Let {
                name: "x".into(),
                value: leaf(),
                body: Box::new(IRNode::Var("x".into())),
            },
            IRNode::If {
                condition: bool_leaf(),
                then_branch: leaf(),
                else_branch: leaf(),
            },
            IRNode::While {
                condition: Box::new(IRNode::Literal(IRLiteral::Bool(false))),
                body: Box::new(IRNode::Block(vec![])),
                invariant: None,
            },
            IRNode::For {
                var: "i".into(),
                start: leaf(),
                end: leaf(),
                body: Box::new(IRNode::Block(vec![])),
            },
            IRNode::Match {
                scrutinee: Box::new(IRNode::Var("kind".into())),
                cases: vec![(
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Int(0)),
                )],
            },
            IRNode::Call {
                function: "check".into(),
                args: vec![IRNode::Var("kind".into())],
            },
            IRNode::Return(leaf()),
            IRNode::Literal(IRLiteral::Int(3)),
            IRNode::Var("kind".into()),
            IRNode::BinOp(
                IRBinOp::Xor,
                bool_leaf(),
                Box::new(IRNode::Literal(IRLiteral::Bool(false))),
            ),
            IRNode::UnaryOp(IRUnaryOp::Head, Box::new(IRNode::Var("kind".into()))),
            IRNode::UnaryOp(IRUnaryOp::Tail, Box::new(IRNode::Var("kind".into()))),
            IRNode::UnaryOp(IRUnaryOp::Init, Box::new(IRNode::Var("kind".into()))),
            IRNode::UnaryOp(IRUnaryOp::Last, Box::new(IRNode::Var("kind".into()))),
            lambda("x"),
            IRNode::Closure {
                params: vec![("x".into(), TypeRef::Int)],
                captures: vec!["kind".into()],
                body: Box::new(IRNode::Var("x".into())),
            },
            IRNode::Collection {
                kind: CollectionKind::List,
                elements: vec![IRNode::Literal(IRLiteral::Int(1))],
            },
            IRNode::Collection {
                kind: CollectionKind::Set,
                elements: vec![IRNode::Literal(IRLiteral::Int(1))],
            },
            IRNode::Collection {
                kind: CollectionKind::Map,
                elements: vec![],
            },
            IRNode::Map {
                function: Box::new(lambda("x")),
                over: Box::new(IRNode::Var("kind".into())),
            },
            IRNode::Filter {
                predicate: Box::new(lambda("x")),
                over: Box::new(IRNode::Var("kind".into())),
            },
            IRNode::Fold {
                operation: Box::new(lambda("a")),
                initial: leaf(),
                over: Box::new(IRNode::Var("kind".into())),
            },
            IRNode::Sort {
                comparator: Box::new(lambda("a")),
                collection: Box::new(IRNode::Var("kind".into())),
            },
            IRNode::Alloc {
                name: "buf".into(),
                ty: TypeRef::Int,
                initial: leaf(),
            },
            IRNode::Assign {
                target: Box::new(IRNode::Var("kind".into())),
                value: leaf(),
            },
            IRNode::Typed {
                node: leaf(),
                ty: TypeRef::Int,
            },
            IRNode::Index(Box::new(IRNode::Var("kind".into())), leaf()),
            IRNode::Loop {
                body: Box::new(IRNode::Block(vec![])),
                invariant: None,
            },
            IRNode::Break { value: None },
            IRNode::Break { value: Some(leaf()) },
            IRNode::StructDef {
                name: "Point".into(),
                fields: vec![("x".into(), TypeRef::Int)],
                body: leaf(),
            },
            IRNode::TryPropagate(Box::new(IRNode::Var("kind".into()))),
            // Phase 77: the two M1-feature kinds render
            // for real (they were the loud unsupported
            // fallback through Phase 76).
            IRNode::FieldAccess {
                base: Box::new(IRNode::Var("kind".into())),
                field: "x".into(),
            },
            IRNode::RecordLit {
                type_name: "Point".into(),
                fields: vec![("x".into(), IRNode::Literal(IRLiteral::Int(1)))],
            },
        ];
        for node in &variants {
            assert_expr_renders(node);
        }
    }

    #[test]
    fn rust_expr_hole_is_a_panic_not_a_comment() {
        let out = ir_to_rust_expr(&IRNode::Hole {
            id: 1,
            expected_type: None,
        });
        assert_eq!(out, "panic!(\"Hole not filled\")");
    }

    #[test]
    fn rust_expr_xor_and_list_unary_ops_render() {
        let xor = ir_to_rust_expr(&IRNode::BinOp(
            IRBinOp::Xor,
            Box::new(IRNode::Literal(IRLiteral::Bool(true))),
            Box::new(IRNode::Literal(IRLiteral::Bool(false))),
        ));
        assert_eq!(xor, "(true ^ false)");
        let var = || Box::new(IRNode::Var("kind".into()));
        assert!(ir_to_rust_expr(&IRNode::UnaryOp(IRUnaryOp::Head, var())).contains("[0]"));
        assert!(ir_to_rust_expr(&IRNode::UnaryOp(IRUnaryOp::Tail, var())).contains("[1..]"));
        assert!(ir_to_rust_expr(&IRNode::UnaryOp(IRUnaryOp::Init, var())).contains("len() - 1"));
        assert!(ir_to_rust_expr(&IRNode::UnaryOp(IRUnaryOp::Last, var())).contains("len() - 1"));
    }

    #[test]
    fn rust_expr_index_try_closure_match_render() {
        let index = ir_to_rust_expr(&IRNode::Index(
            Box::new(IRNode::Var("items".into())),
            Box::new(IRNode::Literal(IRLiteral::Int(0))),
        ));
        assert_eq!(index, "(items)[0]");
        let try_prop = ir_to_rust_expr(&IRNode::TryPropagate(Box::new(IRNode::Var(
            "kind".into(),
        ))));
        assert_eq!(try_prop, "(kind)?");
        let closure = ir_to_rust_expr(&IRNode::Closure {
            params: vec![("x".into(), TypeRef::Int)],
            captures: vec!["kind".into()],
            body: Box::new(IRNode::Var("x".into())),
        });
        assert!(closure.starts_with("|x|"), "closure not lambda-shaped: {}", closure);
        let mat = ir_to_rust_expr(&IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![(
                IRPattern::Wildcard,
                IRNode::Literal(IRLiteral::Int(0)),
            )],
        });
        assert!(mat.contains("match kind {"), "match not rendered: {}", mat);
        assert!(mat.contains("_ => 0"), "match arm not rendered: {}", mat);
    }

    #[test]
    fn rust_expr_block_like_conditions_and_scrutinees_are_parenthesized() {
        // A Match rendered by the new expression arm cannot sit
        // bare in a condition or scrutinee position — `if match
        // … { } { }` does not parse.
        let body = IRNode::If {
            condition: Box::new(IRNode::Match {
                scrutinee: Box::new(IRNode::Var("kind".into())),
                cases: vec![
                    (
                        IRPattern::Literal(IRLiteral::Int(0)),
                        IRNode::Literal(IRLiteral::Bool(true)),
                    ),
                    (
                        IRPattern::Wildcard,
                        IRNode::Literal(IRLiteral::Bool(false)),
                    ),
                ],
            }),
            then_branch: Box::new(IRNode::Literal(IRLiteral::Bool(true))),
            else_branch: Box::new(IRNode::Literal(IRLiteral::Bool(false))),
        };
        let cand = candidate("match_condition", body);
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("if (match kind {"),
            "match condition not parenthesized:\n{}",
            code
        );
        let nested = ir_to_rust_expr(&IRNode::Match {
            scrutinee: Box::new(IRNode::Match {
                scrutinee: Box::new(IRNode::Var("kind".into())),
                cases: vec![(
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Int(0)),
                )],
            }),
            cases: vec![(
                IRPattern::Wildcard,
                IRNode::Literal(IRLiteral::Int(1)),
            )],
        });
        assert!(
            nested.contains("match (match kind {"),
            "match scrutinee not parenthesized: {}",
            nested
        );
    }

    #[test]
    fn rust_expr_return_and_break_are_parenthesized() {
        // `return`/`break` greedily consume a following expression,
        // so as an operand they must be parenthesized to keep the
        // IR's operand boundary: `(return 5) + 1`, not `return 5 + 1`.
        let ret = ir_to_rust_expr(&IRNode::Return(Box::new(IRNode::Literal(
            IRLiteral::Int(5),
        ))));
        assert_eq!(ret, "(return 5)");
        let brk = ir_to_rust_expr(&IRNode::Break {
            value: Some(Box::new(IRNode::Literal(IRLiteral::Int(5)))),
        });
        assert_eq!(brk, "(break 5)");
        let brk_unit = ir_to_rust_expr(&IRNode::Break { value: None });
        assert_eq!(brk_unit, "(break)");
    }

    #[test]
    fn rust_expr_set_and_map_collections_are_fully_qualified() {
        // No import is generated for winner files, so an unqualified
        // `HashSet::from(...)` fails E0433. Fully qualified paths
        // compile standalone.
        let set = ir_to_rust_expr(&IRNode::Collection {
            kind: CollectionKind::Set,
            elements: vec![IRNode::Literal(IRLiteral::Int(1))],
        });
        assert!(
            set.contains("std::collections::HashSet::from"),
            "set collection not fully qualified: {}",
            set
        );
        let map = ir_to_rust_expr(&IRNode::Collection {
            kind: CollectionKind::Map,
            elements: vec![],
        });
        assert!(
            map.contains("std::collections::HashMap::from"),
            "map collection not fully qualified: {}",
            map
        );
    }

    // The Phase 11 sighting, pinned directly: a Hole in an If
    // condition was emitted as `if /* UNSUPPORTED */` — a parse
    // error in the whole file.
    #[test]
    fn rust_codegen_hole_in_if_condition_is_a_panic() {
        let body = IRNode::If {
            condition: Box::new(IRNode::Hole {
                id: 99,
                expected_type: None,
            }),
            then_branch: Box::new(IRNode::Literal(IRLiteral::Bool(true))),
            else_branch: Box::new(IRNode::Literal(IRLiteral::Bool(false))),
        };
        let cand = candidate("hole_condition", body);
        let code = generate(&cand, "rust").unwrap();
        assert!(
            !code.contains("UNSUPPORTED"),
            "UNSUPPORTED marker in generated code:\n{}",
            code
        );
        assert!(
            code.contains("if panic!(\"Hole not filled\")"),
            "hole condition not a panic:\n{}",
            code
        );
    }

    // Statement position (Phase 13): the Call/Map/Filter/Fold/Sort
    // statement arms emitted unterminated tail shapes — the same
    // latent hazard the Collection arm carried until Phase 11.
    // A following statement turns any of these into a syntax error.

    #[test]
    fn rust_codegen_call_statement_is_terminated() {
        let body = IRNode::Block(vec![
            IRNode::Call {
                function: "check".into(),
                args: vec![IRNode::Var("kind".into())],
            },
            IRNode::Return(Box::new(IRNode::Literal(IRLiteral::Bool(true)))),
        ]);
        let cand = candidate("check", body);
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("check(kind);"),
            "call statement not terminated:\n{}",
            code
        );
    }

    #[test]
    fn rust_codegen_map_and_filter_statements_are_terminated() {
        let lambda = |body: IRNode| IRNode::Lambda {
            params: vec![("x".into(), TypeRef::Int)],
            body: Box::new(body),
        };
        let body = IRNode::Block(vec![
            IRNode::Map {
                function: Box::new(lambda(IRNode::Var("x".into()))),
                over: Box::new(IRNode::Var("kind".into())),
            },
            IRNode::Filter {
                predicate: Box::new(lambda(IRNode::Literal(IRLiteral::Bool(true)))),
                over: Box::new(IRNode::Var("kind".into())),
            },
            IRNode::Return(Box::new(IRNode::Literal(IRLiteral::Bool(true)))),
        ]);
        let cand = candidate("map_filter_stmts", body);
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains(".collect();"),
            "map/filter statement not terminated:\n{}",
            code
        );
        assert_eq!(
            code.matches(".collect();").count(),
            2,
            "both map and filter statements must be terminated:\n{}",
            code
        );
    }

    #[test]
    fn rust_codegen_fold_statement_is_terminated() {
        let body = IRNode::Block(vec![
            IRNode::Fold {
                operation: Box::new(IRNode::Lambda {
                    params: vec![("a".into(), TypeRef::Int)],
                    body: Box::new(IRNode::Var("a".into())),
                }),
                initial: Box::new(IRNode::Literal(IRLiteral::Int(0))),
                over: Box::new(IRNode::Var("kind".into())),
            },
            IRNode::Return(Box::new(IRNode::Literal(IRLiteral::Bool(true)))),
        ]);
        let cand = candidate("fold_stmt", body);
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("fold(0, |a| { a });"),
            "fold statement not terminated:\n{}",
            code
        );
    }

    #[test]
    fn rust_codegen_sort_statement_is_terminated() {
        let body = IRNode::Block(vec![
            IRNode::Sort {
                comparator: Box::new(IRNode::Lambda {
                    params: vec![("a".into(), TypeRef::Int)],
                    body: Box::new(IRNode::Literal(IRLiteral::Int(1))),
                }),
                collection: Box::new(IRNode::Var("kind".into())),
            },
            IRNode::Return(Box::new(IRNode::Literal(IRLiteral::Bool(true)))),
        ]);
        let cand = candidate("sort_stmt", body);
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("sorted;"),
            "sort statement's final value not terminated:\n{}",
            code
        );
    }

    // Tail guards: value positions must keep the bare expression —
    // terminating these would discard the value and break the
    // function's return type (the Phase 11 tail defect family).

    #[test]
    fn rust_codegen_bare_call_body_stays_tail() {
        let body = IRNode::Call {
            function: "check".into(),
            args: vec![IRNode::Var("kind".into())],
        };
        let cand = candidate("check", body);
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("check(kind)\n}"),
            "bare call body not a tail expression:\n{}",
            code
        );
        assert!(
            !code.contains("check(kind);"),
            "bare call body discarded as statement:\n{}",
            code
        );
    }

    #[test]
    fn rust_codegen_bare_map_body_stays_tail() {
        let body = IRNode::Map {
            function: Box::new(IRNode::Lambda {
                params: vec![("x".into(), TypeRef::Int)],
                body: Box::new(IRNode::Var("x".into())),
            }),
            over: Box::new(IRNode::Var("kind".into())),
        };
        let mut cand = candidate("mapped", body);
        cand.spec.return_type = TypeRef::List(Box::new(TypeRef::Int));
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains(".collect()\n}"),
            "bare map body not a tail expression:\n{}",
            code
        );
        assert!(
            !code.contains(".collect();"),
            "bare map body discarded as statement:\n{}",
            code
        );
    }

    #[test]
    fn rust_codegen_bare_fold_body_stays_tail() {
        let body = IRNode::Fold {
            operation: Box::new(IRNode::Lambda {
                params: vec![("a".into(), TypeRef::Int)],
                body: Box::new(IRNode::Var("a".into())),
            }),
            initial: Box::new(IRNode::Literal(IRLiteral::Int(0))),
            over: Box::new(IRNode::Var("kind".into())),
        };
        let mut cand = candidate("folded", body);
        cand.spec.return_type = TypeRef::Int;
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("fold(0, |a| { a })\n}"),
            "bare fold body not a tail expression:\n{}",
            code
        );
        assert!(
            !code.contains("fold(0, |a| { a });"),
            "bare fold body discarded as statement:\n{}",
            code
        );
    }

    #[test]
    fn rust_codegen_bare_sort_body_stays_tail() {
        let body = IRNode::Sort {
            comparator: Box::new(IRNode::Lambda {
                params: vec![("a".into(), TypeRef::Int)],
                body: Box::new(IRNode::Literal(IRLiteral::Int(1))),
            }),
            collection: Box::new(IRNode::Var("kind".into())),
        };
        let mut cand = candidate("sorted_body", body);
        cand.spec.return_type = TypeRef::List(Box::new(TypeRef::Int));
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("sorted\n}") || code.contains("sorted }"),
            "bare sort body does not yield the sorted value:\n{}",
            code
        );
        assert!(
            !code.contains("sorted;\n}"),
            "bare sort body discarded as statement:\n{}",
            code
        );
    }

    #[test]
    fn rust_codegen_match_arm_call_yields_value() {
        let body = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(0)),
                    IRNode::Call {
                        function: "check".into(),
                        args: vec![IRNode::Var("kind".into())],
                    },
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Bool(false)),
                ),
            ],
        };
        let cand = candidate("check", body);
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("check(kind)\n"),
            "match arm call not a tail expression:\n{}",
            code
        );
        assert!(
            !code.contains("check(kind);"),
            "match arm call discarded as statement:\n{}",
            code
        );
    }

    // Integration: a synthetic body combining the previously
    // broken shapes in both positions must compile with rustc
    // (the probe's method). Before this phase the file did not
    // even parse: unterminated Call/Fold statements and a
    // `/* UNSUPPORTED */` If condition.
    #[test]
    fn rust_codegen_previously_broken_shapes_compile() {
        let body = IRNode::Block(vec![
            // Call in statement position (the self-call passes a
            // fresh list for the second parameter so `items`
            // stays borrowable for the statements after it).
            IRNode::Call {
                function: "check".into(),
                args: vec![
                    IRNode::Var("kind".into()),
                    IRNode::Collection {
                        kind: CollectionKind::List,
                        elements: vec![],
                    },
                ],
            },
            // Fold in statement position.
            IRNode::Fold {
                operation: Box::new(IRNode::Lambda {
                    params: vec![
                        ("acc".into(), TypeRef::Int),
                        ("x".into(), TypeRef::Int),
                    ],
                    body: Box::new(IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(IRNode::Var("acc".into())),
                        Box::new(IRNode::Var("x".into())),
                    )),
                }),
                initial: Box::new(IRNode::Literal(IRLiteral::Int(0))),
                over: Box::new(IRNode::Var("items".into())),
            },
            // Collection in statement position (Phase 11 fix,
            // kept here as company).
            IRNode::Collection {
                kind: CollectionKind::List,
                elements: vec![IRNode::Literal(IRLiteral::Int(1))],
            },
            // Hole in an If condition (expression position).
            IRNode::If {
                condition: Box::new(IRNode::Hole {
                    id: 99,
                    expected_type: None,
                }),
                then_branch: Box::new(IRNode::Block(vec![IRNode::Return(Box::new(
                    IRNode::Literal(IRLiteral::Int(1)),
                ))])),
                else_branch: Box::new(IRNode::Block(vec![IRNode::Return(Box::new(
                    IRNode::Literal(IRLiteral::Int(2)),
                ))])),
            },
            // Index in expression position.
            IRNode::Return(Box::new(IRNode::BinOp(
                IRBinOp::Add,
                Box::new(IRNode::Index(
                    Box::new(IRNode::Var("items".into())),
                    Box::new(IRNode::Literal(IRLiteral::Int(0))),
                )),
                Box::new(IRNode::Literal(IRLiteral::Int(1))),
            ))),
        ]);
        let cand = candidate_with(
            "check",
            vec![
                ("kind", TypeRef::Int),
                ("items", TypeRef::List(Box::new(TypeRef::Int))),
            ],
            TypeRef::Int,
            body,
        );
        let code = generate(&cand, "rust").unwrap();
        assert!(
            !code.contains("UNSUPPORTED"),
            "UNSUPPORTED marker in generated code:\n{}",
            code
        );
        compile_rust_lib(&code);

        // A bare Map body is a tail position: it must keep its
        // value (and compile).
        let mapped = candidate_with(
            "mapped",
            vec![("items", TypeRef::List(Box::new(TypeRef::Int)))],
            TypeRef::List(Box::new(TypeRef::Int)),
            IRNode::Map {
                function: Box::new(IRNode::Lambda {
                    params: vec![("x".into(), TypeRef::Int)],
                    body: Box::new(IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(IRNode::Var("x".into())),
                        Box::new(IRNode::Literal(IRLiteral::Int(0))),
                    )),
                }),
                over: Box::new(IRNode::Var("items".into())),
            },
        );
        compile_rust_lib(&generate(&mapped, "rust").unwrap());
    }

    // ── Phase 29: rendering fixes (user-approved unfreeze) ──
    //
    // The three defects measured in Phases 25/27/28, fixed
    // under the re-pin inventory in ROAD_TO_SELF_HOSTING.md.
    // Each test below renders a shape that previously failed
    // rustc (or produced invalid Rust) and asserts the emitted
    // code compiles AND behaves when executed.

    #[test]
    fn rust_concat_literal_first_assembly_compiles_and_runs() {
        // Defect 1 (E0369): `"Vec<" ++ inner ++ ">"` rendered
        // `(("Vec<" + inner) + ">")` — a &str literal as the
        // left operand of `+`. The left operand of a Concat
        // must be owned. (The Var RHS borrow is defect 2a,
        // covered by its own test below.)
        let body = IRNode::BinOp(
            IRBinOp::Concat,
            Box::new(IRNode::BinOp(
                IRBinOp::Concat,
                Box::new(IRNode::Literal(IRLiteral::String("Vec<".into()))),
                Box::new(IRNode::Var("inner".into())),
            )),
            Box::new(IRNode::Literal(IRLiteral::String(">".into()))),
        );
        // The left-operand ownership is defect 1's part of
        // this rendering; the `&inner` borrow is defect 2a's
        // (its own test below asserts the full text again
        // through the generator).
        assert_eq!(
            ir_to_rust_expr(&body),
            "((\"Vec<\".to_string() + &inner) + \">\")"
        );
        // The compile leg isolates defect 1: a literal pair
        // (left owned, right a &str) through the full
        // generator.
        let pair_body = IRNode::BinOp(
            IRBinOp::Concat,
            Box::new(IRNode::Literal(IRLiteral::String("Vec<".into()))),
            Box::new(IRNode::Literal(IRLiteral::String(">".into()))),
        );
        let cand = candidate_with("empty_vec", vec![], TypeRef::String, pair_body);
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("(\"Vec<\".to_string() + \">\")"),
            "literal-pair concat not owned on the left:\n{}",
            code
        );
        let out = compile_and_run(
            &code,
            "assert_eq!(empty_vec(), \"Vec<>\");\nprintln!(\"empty_vec ok\");",
        );
        assert!(out.contains("empty_vec ok"), "run output: {}", out);
    }

    #[test]
    fn rust_concat_literal_pair_and_variable_first_compile() {
        // Literal ++ literal: left owned, right stays a &str.
        let pair = ir_to_rust_expr(&IRNode::BinOp(
            IRBinOp::Concat,
            Box::new(IRNode::Literal(IRLiteral::String("a".into()))),
            Box::new(IRNode::Literal(IRLiteral::String("b".into()))),
        ));
        assert_eq!(pair, "(\"a\".to_string() + \"b\")");
        // Variable-first (the Phase 25 exclaim shape): the Var
        // yields a String, but a bare rendering would MOVE it
        // (Phase 39, E0382) — in left position it takes the
        // owned-copy treatment, `.clone()`; the literal RHS
        // stays bare. (Phase 29 recorded this rendering as
        // byte-identical to pre-fix at `(inner + "!")`; the
        // Phase 39 inventory's C-1 re-pins it.)
        let var_first = ir_to_rust_expr(&IRNode::BinOp(
            IRBinOp::Concat,
            Box::new(IRNode::Var("inner".into())),
            Box::new(IRNode::Literal(IRLiteral::String("!".into()))),
        ));
        assert_eq!(var_first, "(inner.clone() + \"!\")");
    }

    /// Discover a reachable rustc (PATH first, then the cargo
    /// home fallback), shared by the compile helpers.
    fn rustc_path() -> Option<String> {
        std::process::Command::new("rustc")
            .arg("--version")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|_| "rustc".to_string())
            .or_else(|| {
                let home = std::env::var("HOME").ok()?;
                let path = format!("{}/.cargo/bin/rustc", home);
                std::process::Command::new(&path)
                    .arg("--version")
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|_| path)
            })
    }

    /// Compile generated code as a binary with an appended
    /// `main` and run it, returning stdout (Phase 29: the
    /// fixed renderings must not only parse — the emitted
    /// shapes must behave identically when executed). Panics
    /// on any compile or run failure; skips only when no
    /// rustc is reachable at all.
    fn compile_and_run(code: &str, main_body: &str) -> String {
        use std::io::Write;
        let Some(rustc) = rustc_path() else {
            eprintln!("rustc not reachable; skipping run check");
            return String::new();
        };
        let dir = std::env::temp_dir().join(format!(
            "morphic_codegen_run_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("winner_main.rs");
        let full = format!("{}\nfn main() {{\n{}\n}}\n", code, main_body);
        let mut f = std::fs::File::create(&file).unwrap();
        f.write_all(full.as_bytes()).unwrap();
        drop(f);
        let bin = dir.join("winner_main");
        let out = std::process::Command::new(&rustc)
            .args(["--edition", "2021"])
            .arg(&file)
            .arg("-o")
            .arg(&bin)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "generated code does not compile as a binary:\n{}\n--- code ---\n{}",
            String::from_utf8_lossy(&out.stderr),
            full
        );
        let run = std::process::Command::new(&bin).output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            run.status.success(),
            "generated binary failed:\n{}\n--- code ---\n{}",
            String::from_utf8_lossy(&run.stderr),
            full
        );
        String::from_utf8_lossy(&run.stdout).into_owned()
    }

    #[test]
    fn rust_concat_string_rhs_is_borrowed_and_runs() {
        // Defect 2a (E0308): the Concat RHS was never
        // borrowed, so `String + String` failed — the
        // rust_param_text shape (`name ++ ": " ++ ty`).
        let body = IRNode::BinOp(
            IRBinOp::Concat,
            Box::new(IRNode::BinOp(
                IRBinOp::Concat,
                Box::new(IRNode::Var("name".into())),
                Box::new(IRNode::Literal(IRLiteral::String(": ".into()))),
            )),
            Box::new(IRNode::Var("ty".into())),
        );
        assert_eq!(
            ir_to_rust_expr(&body),
            "((name.clone() + \": \") + &ty)"
        );
        let cand = candidate_with(
            "param_text",
            vec![("name", TypeRef::String), ("ty", TypeRef::String)],
            TypeRef::String,
            body,
        );
        let code = generate(&cand, "rust").unwrap();
        let out = compile_and_run(
            &code,
            "assert_eq!(param_text(\"count\".to_string(), \"i64\".to_string()), \"count: i64\");\n\
             println!(\"param_text ok\");",
        );
        assert!(out.contains("param_text ok"), "run output: {}", out);
        // With defect 1, the full wrap_vec assembly now
        // compiles and runs end to end.
        let wrap_body = IRNode::BinOp(
            IRBinOp::Concat,
            Box::new(IRNode::BinOp(
                IRBinOp::Concat,
                Box::new(IRNode::Literal(IRLiteral::String("Vec<".into()))),
                Box::new(IRNode::Var("inner".into())),
            )),
            Box::new(IRNode::Literal(IRLiteral::String(">".into()))),
        );
        let wrap = candidate_with(
            "wrap_vec",
            vec![("inner", TypeRef::String)],
            TypeRef::String,
            wrap_body,
        );
        let wrap_code = generate(&wrap, "rust").unwrap();
        assert!(
            wrap_code.contains(r#"(("Vec<".to_string() + &inner) + ">")"#),
            "wrap_vec rendering:\n{}",
            wrap_code
        );
        let out = compile_and_run(
            &wrap_code,
            "assert_eq!(wrap_vec(\"i64\".to_string()), \"Vec<i64>\");\n\
             assert_eq!(wrap_vec(\"Vec<u8>\".to_string()), \"Vec<Vec<u8>>\");\n\
             assert_eq!(wrap_vec(String::new()), \"Vec<>\");\n\
             println!(\"wrap_vec ok\");",
        );
        assert!(out.contains("wrap_vec ok"), "run output: {}", out);
        // A nested Concat on the right is borrowed as a unit.
        let nested = ir_to_rust_expr(&IRNode::BinOp(
            IRBinOp::Concat,
            Box::new(IRNode::Literal(IRLiteral::String("x".into()))),
            Box::new(IRNode::BinOp(
                IRBinOp::Concat,
                Box::new(IRNode::Literal(IRLiteral::String("y".into()))),
                Box::new(IRNode::Var("s".into())),
            )),
        ));
        assert_eq!(nested, "(\"x\".to_string() + &(\"y\".to_string() + &s))");
    }

    // ── Phase 39: E0382 Concat-operand move ──────────────
    //
    // The fourth rendering family (Phase 36, Finding 2):
    // Concat's rendered `+` MOVES a String variable in
    // left-operand position. Phase 29 owned a literal there
    // and borrowed right operands, but a variable used two
    // or more times on one execution path — the statement
    // templates' pad / pad_inner, repeated in a single `++`
    // chain — fails E0382 (borrow of moved value). The
    // tests below render the four measured winners' body
    // shapes through the full generator and assert the
    // emitted Rust compiles AND executes correctly (the
    // compile_and_run discipline of Phase 29).

    fn concat_chain(parts: Vec<IRNode>) -> IRNode {
        let mut it = parts.into_iter();
        let first = it.next().unwrap();
        it.fold(first, |acc, p| {
            IRNode::BinOp(IRBinOp::Concat, Box::new(acc), Box::new(p))
        })
    }

    fn lit(s: &str) -> IRNode {
        IRNode::Literal(IRLiteral::String(s.into()))
    }

    fn var(name: &str) -> IRNode {
        IRNode::Var(name.into())
    }

    #[test]
    fn rust_concat_var_left_renders_clone() {
        // The unit shape: a variable in Concat left position
        // takes the owned-copy treatment (`.clone()`, the
        // literal treatment's analogue); a variable on the
        // right stays borrowed, a literal left stays
        // `.to_string()`.
        assert_eq!(
            ir_to_rust_expr(&IRNode::BinOp(
                IRBinOp::Concat,
                Box::new(var("x")),
                Box::new(lit("a")),
            )),
            "(x.clone() + \"a\")"
        );
        assert_eq!(
            ir_to_rust_expr(&IRNode::BinOp(
                IRBinOp::Concat,
                Box::new(var("x")),
                Box::new(var("y")),
            )),
            "(x.clone() + &y)"
        );
    }

    #[test]
    fn rust_stmt_template_pad_reuse_compiles_and_runs() {
        // rust_stmt_hole_text's seed body, verbatim shape:
        // pad used twice in one chain.
        let hole_body = concat_chain(vec![
            var("pad"),
            lit("// TODO: synthesis incomplete\n"),
            var("pad"),
            lit("panic!(\"Hole not filled\");\n"),
        ]);
        assert_eq!(
            ir_to_rust_expr(&hole_body),
            "(((pad.clone() + \"// TODO: synthesis incomplete\\n\") + &pad) + \"panic!(\\\"Hole not filled\\\");\\n\")"
        );
        let cand = candidate_with(
            "stmt_hole_text",
            vec![("pad", TypeRef::String)],
            TypeRef::String,
            hole_body,
        );
        let code = generate(&cand, "rust").unwrap();
        let out = compile_and_run(
            &code,
            "assert_eq!(stmt_hole_text(\"    \".to_string()), \"    // TODO: synthesis incomplete\\n    panic!(\\\"Hole not filled\\\");\\n\");\n\
             assert_eq!(stmt_hole_text(String::new()), \"// TODO: synthesis incomplete\\npanic!(\\\"Hole not filled\\\");\\n\");\n\
             println!(\"hole ok\");",
        );
        assert!(out.contains("hole ok"), "run output: {}", out);

        // rust_stmt_for_text's shape: pad opens the line and
        // pads the closing brace.
        let for_body = concat_chain(vec![
            var("pad"),
            lit("for "),
            var("v"),
            lit(" in "),
            var("start"),
            lit(".."),
            var("end"),
            lit(" {\n"),
            var("body"),
            var("pad"),
            lit("}\n"),
        ]);
        let cand = candidate_with(
            "stmt_for_text",
            vec![
                ("pad", TypeRef::String),
                ("v", TypeRef::String),
                ("start", TypeRef::String),
                ("end", TypeRef::String),
                ("body", TypeRef::String),
            ],
            TypeRef::String,
            for_body,
        );
        let code = generate(&cand, "rust").unwrap();
        let out = compile_and_run(
            &code,
            "assert_eq!(stmt_for_text(\"  \".to_string(), \"i\".to_string(), \"0\".to_string(), \"3\".to_string(), \"  x;\\n\".to_string()), \"  for i in 0..3 {\\n  x;\\n  }\\n\");\n\
             println!(\"for ok\");",
        );
        assert!(out.contains("for ok"), "run output: {}", out);

        // rust_stmt_match_arm_text's shape: pad_inner frames
        // the arm on both sides.
        let arm_body = concat_chain(vec![
            var("pad_inner"),
            var("pat"),
            lit(" => {\n"),
            var("body"),
            var("pad_inner"),
            lit("},\n"),
        ]);
        let cand = candidate_with(
            "stmt_match_arm_text",
            vec![
                ("pad_inner", TypeRef::String),
                ("pat", TypeRef::String),
                ("body", TypeRef::String),
            ],
            TypeRef::String,
            arm_body,
        );
        let code = generate(&cand, "rust").unwrap();
        let out = compile_and_run(
            &code,
            "assert_eq!(stmt_match_arm_text(\"    \".to_string(), \"Some(x)\".to_string(), \"        x\\n\".to_string()), \"    Some(x) => {\\n        x\\n    },\\n\");\n\
             println!(\"arm ok\");",
        );
        assert!(out.contains("arm ok"), "run output: {}", out);
    }

    #[test]
    fn rust_stmt_while_invariant_pad_thrice_compiles_and_runs() {
        // rust_stmt_while_text's invariant branch: pad three
        // times in one chain (comment line, loop head,
        // closing brace), inv once on the right.
        let while_body = concat_chain(vec![
            var("pad"),
            lit("// invariant: "),
            var("inv"),
            lit("\n"),
            var("pad"),
            lit("while "),
            var("cond"),
            lit(" {\n"),
            var("body"),
            var("pad"),
            lit("}\n"),
        ]);
        let cand = candidate_with(
            "stmt_while_text",
            vec![
                ("pad", TypeRef::String),
                ("inv", TypeRef::String),
                ("cond", TypeRef::String),
                ("body", TypeRef::String),
            ],
            TypeRef::String,
            while_body,
        );
        let code = generate(&cand, "rust").unwrap();
        let out = compile_and_run(
            &code,
            "assert_eq!(stmt_while_text(\"  \".to_string(), \"x > 0\".to_string(), \"c\".to_string(), \"    s;\\n\".to_string()), \"  // invariant: x > 0\\n  while c {\\n    s;\\n  }\\n\");\n\
             println!(\"while ok\");",
        );
        assert!(out.contains("while ok"), "run output: {}", out);
    }

    #[test]
    fn rust_concat_var_reused_after_chain_compiles_and_runs() {
        // A variable moved by a chain and then used again by
        // a LATER statement: the Let value consumes x in a
        // Concat, the Let's body uses x again. Pre-fix this
        // is the same E0382, one statement later.
        let body = IRNode::Block(vec![IRNode::Let {
            name: "y".into(),
            value: Box::new(concat_chain(vec![var("x"), lit("a")])),
            body: Box::new(concat_chain(vec![var("x"), var("y")])),
        }]);
        let cand = candidate_with(
            "reuse_after",
            vec![("x", TypeRef::String)],
            TypeRef::String,
            body,
        );
        let code = generate(&cand, "rust").unwrap();
        let out = compile_and_run(
            &code,
            "assert_eq!(reuse_after(\"m\".to_string()), \"mma\");\n\
             println!(\"reuse ok\");",
        );
        assert!(out.contains("reuse ok"), "run output: {}", out);
    }

    #[test]
    fn rust_match_over_string_scrutinee_uses_as_str() {
        // Defect 2b (E0308): a Match whose scrutinee is a
        // String and whose patterns are string literals
        // rendered `match name { "type" => ... }` — &str
        // patterns against a String scrutinee (the
        // rust_ident_empty winner's shape).
        let lit_str = |s: &str| IRNode::Literal(IRLiteral::String(s.into()));
        let body = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("name".into())),
            cases: vec![
                (
                    IRPattern::Guard(
                        Box::new(IRPattern::Literal(IRLiteral::String(
                            "type".into(),
                        ))),
                        "flag".to_string(),
                    ),
                    lit_str("keyword"),
                ),
                (
                    IRPattern::Literal(IRLiteral::String("fn".into())),
                    lit_str("keyword"),
                ),
                (IRPattern::Wildcard, lit_str("plain")),
            ],
        };
        let cand = candidate_with(
            "classify",
            vec![
                ("name", TypeRef::String),
                ("flag", TypeRef::Bool),
            ],
            TypeRef::String,
            body,
        );
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("match name.as_str()"),
            "string-pattern match not rendered with .as_str():\n{}",
            code
        );
        let out = compile_and_run(
            &code,
            "assert_eq!(classify(\"type\".to_string(), true), \"keyword\");\n\
             assert_eq!(classify(\"type\".to_string(), false), \"plain\");\n\
             assert_eq!(classify(\"fn\".to_string(), false), \"keyword\");\n\
             assert_eq!(classify(\"hello\".to_string(), true), \"plain\");\n\
             println!(\"classify ok\");",
        );
        assert!(out.contains("classify ok"), "run output: {}", out);
    }

    #[test]
    fn rust_match_as_str_only_where_patterns_need_it() {
        // A literal scrutinee is already a &str (and &str
        // has no stable .as_str()): it must stay bare.
        let body = IRNode::Match {
            scrutinee: Box::new(IRNode::Literal(IRLiteral::String(
                "type".into(),
            ))),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::String("type".into())),
                    IRNode::Literal(IRLiteral::Int(1)),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Int(0)),
                ),
            ],
        };
        let rendered = ir_to_rust_expr(&body);
        assert!(
            rendered.contains("match \"type\" {"),
            "literal scrutinee must stay bare:\n{}",
            rendered
        );
        assert!(!rendered.contains(".as_str()"), "rendered: {}", rendered);
        // Expression position (a Let value) gets the same
        // treatment as statement position.
        let expr_body = IRNode::Let {
            name: "r".into(),
            value: Box::new(IRNode::Match {
                scrutinee: Box::new(IRNode::Var("name".into())),
                cases: vec![
                    (
                        IRPattern::Literal(IRLiteral::String("a".into())),
                        IRNode::Literal(IRLiteral::Int(1)),
                    ),
                    (
                        IRPattern::Wildcard,
                        IRNode::Literal(IRLiteral::Int(0)),
                    ),
                ],
            }),
            body: Box::new(IRNode::Var("r".into())),
        };
        let cand = candidate_with(
            "score",
            vec![("name", TypeRef::String)],
            TypeRef::Int,
            expr_body,
        );
        let code = generate(&cand, "rust").unwrap();
        assert!(
            code.contains("match name.as_str()"),
            "expression-position match not rendered with .as_str():\n{}",
            code
        );
        let out = compile_and_run(
            &code,
            "assert_eq!(score(\"a\".to_string()), 1);\n\
             assert_eq!(score(\"b\".to_string()), 0);\n\
             println!(\"score ok\");",
        );
        assert!(out.contains("score ok"), "run output: {}", out);
        // Int patterns against an Int scrutinee: untouched.
        let int_match = ir_to_rust_expr(&IRNode::Match {
            scrutinee: Box::new(IRNode::Var("n".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(3)),
                    IRNode::Literal(IRLiteral::Int(1)),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Int(0)),
                ),
            ],
        });
        assert!(!int_match.contains(".as_str()"), "rendered: {}", int_match);
    }

    #[test]
    fn rust_string_literals_are_escaped() {
        // Defect 3 (parse-level): string literals were
        // emitted with contents RAW, so any literal
        // containing `"`, `\` or a control character
        // produced syntactically invalid Rust (`"""` for
        // the quote templates). All five emission sites
        // escape per Rust source rules; literals without
        // special characters render byte-identically.
        let lit = |s: &str| IRNode::Literal(IRLiteral::String(s.into()));
        // Expression position.
        assert_eq!(ir_to_rust_expr(&lit("a\"b")), "\"a\\\"b\"");
        assert_eq!(ir_to_rust_expr(&lit("a\\b")), "\"a\\\\b\"");
        assert_eq!(ir_to_rust_expr(&lit("line\nbreak")), "\"line\\nbreak\"");
        assert_eq!(ir_to_rust_expr(&lit("a\tb")), "\"a\\tb\"");
        assert_eq!(ir_to_rust_expr(&lit("a\u{1}b")), "\"a\\u{1}b\"");
        // Innocent contents: identity.
        assert_eq!(ir_to_rust_expr(&lit("Int")), "\"Int\"");
        assert_eq!(ir_to_rust_expr(&lit("café")), "\"café\"");
        assert_eq!(ir_to_rust_expr(&lit("with space")), "\"with space\"");
        assert_eq!(ir_to_rust_expr(&lit("")), "\"\"");
        // Owned position (typed emitter).
        assert_eq!(
            ir_to_rust_expr_typed(&lit("a\"b"), Some(&TypeRef::String)),
            "\"a\\\"b\".to_string()"
        );
        // Concat left position (defect 1's owned arm).
        assert_eq!(
            concat_owned_operand(&lit("a\"")),
            "\"a\\\"\".to_string()"
        );
        // Pattern position.
        assert_eq!(
            ir_pattern_to_rust(&IRPattern::Literal(IRLiteral::String(
                "a\"b".into()
            ))),
            "\"a\\\"b\""
        );
        // Spec-Expr position (generated test modules and
        // precondition asserts).
        assert_eq!(
            expr_to_rust(&Expr::StringLit("a\"b".into())),
            "\"a\\\"b\""
        );
        // Char literals: the same defect class at the
        // adjacent arm.
        let chr = |c: char| IRNode::Literal(IRLiteral::Char(c));
        assert_eq!(ir_to_rust_expr(&chr('\'')), "'\\''");
        assert_eq!(ir_to_rust_expr(&chr('\\')), "'\\\\'");
        assert_eq!(ir_to_rust_expr(&chr('\n')), "'\\n'");
        assert_eq!(ir_to_rust_expr(&chr('a')), "'a'");
    }

    #[test]
    fn rust_quote_bearing_literals_compile_and_run() {
        // The quote templates' winner shape: a body whose
        // value is a quote-bearing literal (here the single
        // character `"`), plus a generated test module whose
        // expected value contains quotes — the
        // rust_precondition_block parse failure flowed
        // through the test module's spec-Expr rendering.
        let mut cand = candidate_with(
            "echo_quote",
            vec![("s", TypeRef::String)],
            TypeRef::String,
            IRNode::Literal(IRLiteral::String("say \"hi\"\nbye".into())),
        );
        cand.spec.tests = vec![Test {
            name: Some("quotes".into()),
            input: Expr::StringLit("x".into()),
            expected_output: Expr::StringLit("say \"hi\"\nbye".into()),
            timeout_ms: None,
            property: false,
        }];
        let code = generate(&cand, "rust").unwrap();
        compile_rust_lib(&code);
        let out = compile_and_run(
            &code,
            "assert_eq!(echo_quote(\"x\".to_string()), \"say \\\"hi\\\"\\nbye\");\n\
             println!(\"quote ok\");",
        );
        assert!(out.contains("quote ok"), "run output: {}", out);
    }

    /// Compile a generated file with `rustc --edition 2021
    /// --crate-type lib` (the probe's external check, in-process).
    /// Skips silently only when no rustc is reachable at all.
    fn compile_rust_lib(code: &str) {
        use std::io::Write;
        let Some(rustc) = rustc_path() else {
            eprintln!("rustc not reachable; skipping compile check");
            return;
        };
        // Unique per call: tests run on parallel threads in
        // one process, and a pid-only directory made
        // concurrent compile checks stomp on each other's
        // winner.rs/rlib (surfaced by Phase 29's added
        // compile legs as a spurious archive error in an
        // unrelated test).
        let dir = std::env::temp_dir().join(format!(
            "morphic_codegen_compile_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("winner.rs");
        let mut f = std::fs::File::create(&file).unwrap();
        f.write_all(code.as_bytes()).unwrap();
        drop(f);
        let out = std::process::Command::new(rustc)
            .args(["--edition", "2021", "--crate-type", "lib"])
            .arg(&file)
            .arg("-o")
            .arg(dir.join("libwinner.rlib"))
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            out.status.success(),
            "generated code does not compile:\n{}\n--- code ---\n{}",
            String::from_utf8_lossy(&out.stderr),
            code
        );
    }

    // ── Phase 77: native emitters for the new nodes ─────
    //
    // The scoped unfreeze (owner-ratified in the Phase 72
    // plan, the Phase 29/39/55/62 form): the Rust emitter
    // gains NEW ARMS ONLY for the Phase 73 node shapes —
    // Expr::MethodCall / Expr::RecordLit in expr_to_rust,
    // IRNode::FieldAccess / IRNode::RecordLit in
    // ir_to_rust_expr — and the C / WAT / Python /
    // JavaScript IR emitters gain the two IR nodes under
    // their own conventions. No existing arm's output
    // moves; the M3/M4 gate re-runs are the proof.

    #[test]
    fn phase77_expr_to_rust_method_calls_render_verbatim() {
        let var = |n: &str| Expr::Var(n.into());
        let mc = |receiver: Expr, name: &str| Expr::MethodCall {
            receiver: Box::new(receiver),
            name: name.into(),
            args: vec![],
        };
        // The four D1a builtins render as the identical
        // Rust method syntax on the rendered receiver.
        assert_eq!(expr_to_rust(&mc(var("output"), "is_ok")), "output.is_ok()");
        assert_eq!(expr_to_rust(&mc(var("output"), "is_err")), "output.is_err()");
        assert_eq!(expr_to_rust(&mc(var("output"), "unwrap")), "output.unwrap()");
        assert_eq!(expr_to_rust(&mc(var("tokens"), "len")), "tokens.len()");
        // Chaining composes — the restored tokenizer's own
        // `output.unwrap().len()` shape.
        assert_eq!(
            expr_to_rust(&mc(mc(var("output"), "unwrap"), "len")),
            "output.unwrap().len()"
        );
        // A method outside the closed table renders the
        // function's standing catch-all — never a guessed
        // rendering. (Unreachable for checked modules: the
        // Phase 74 typechecker rejects unknown methods and
        // arguments to a builtin alike.)
        assert_eq!(expr_to_rust(&mc(var("output"), "tokens")), "??");
        let with_arg = Expr::MethodCall {
            receiver: Box::new(var("output")),
            name: "is_ok".into(),
            args: vec![Expr::IntLit(1)],
        };
        assert_eq!(expr_to_rust(&with_arg), "??");
        // The Field boundary as Phase 77 left it —
        // Expr::Field kept its pre-existing catch-all
        // rendering ("??", and "??.len()" through a method
        // receiver) — was Phase 78 territory, and Phase 78
        // has now landed: the Field arm above renders the
        // projection for real, in lockstep with the
        // Codegen specs' kind-5 row (rust_expr_field_text),
        // so the chained shape renders whole.
        let field_recv = Expr::Field(Box::new(var("state")), "tokens".into());
        assert_eq!(expr_to_rust(&field_recv), "state.tokens");
        assert_eq!(expr_to_rust(&mc(field_recv, "len")), "state.tokens.len()");
    }

    #[test]
    fn phase77_expr_to_rust_record_literals_render_named() {
        // The restored tokenizer module's own test shape
        // (tokenize_one "keyword-spec"): a Token literal,
        // fields in written order.
        let token = Expr::RecordLit {
            type_name: "Token".into(),
            fields: vec![
                ("kind".into(), Expr::IntLit(0)),
                ("lexeme".into(), Expr::StringLit("spec".into())),
                ("line".into(), Expr::IntLit(1)),
                ("col".into(), Expr::IntLit(1)),
            ],
        };
        assert_eq!(
            expr_to_rust(&token),
            "Token { kind: 0, lexeme: \"spec\".to_string(), line: 1, col: 1 }"
        );
        // Nested inside the List pseudo-call and an Ok(...)
        // wrap — the tokenize_all "simple-spec" expected
        // shape.
        let wrapped = Expr::Call(
            "Ok".into(),
            vec![Expr::Call("List".into(), vec![token])],
        );
        assert_eq!(
            expr_to_rust(&wrapped),
            "Ok(vec![Token { kind: 0, lexeme: \"spec\".to_string(), line: 1, col: 1 }])"
        );
        // An empty record literal keeps the template's
        // braces (the StructDef arm's spacing convention).
        let empty = Expr::RecordLit {
            type_name: "Unit2".into(),
            fields: vec![],
        };
        assert_eq!(expr_to_rust(&empty), "Unit2 {  }");
    }

    #[test]
    fn phase77_ir_to_rust_field_access_and_record_literals() {
        let fa = |base: IRNode, field: &str| IRNode::FieldAccess {
            base: Box::new(base),
            field: field.into(),
        };
        let v = |n: &str| IRNode::Var(n.into());
        let int = |n: i64| IRNode::Literal(IRLiteral::Int(n));
        // Projection on a variable base — the tokenizer's
        // pervasive shape.
        assert_eq!(ir_to_rust_expr(&fa(v("state"), "pos")), "state.pos");
        // Chained projection composes bare.
        assert_eq!(
            ir_to_rust_expr(&fa(fa(v("a"), "b"), "c")),
            "a.b.c"
        );
        // A record-literal base is parenthesized: a bare
        // struct literal cannot head a projection in the
        // scrutinee/condition positions this emitter feeds.
        let lit_base = IRNode::RecordLit {
            type_name: "Token".into(),
            fields: vec![("kind".into(), int(0))],
        };
        assert_eq!(
            ir_to_rust_expr(&fa(lit_base, "kind")),
            "(Token { kind: 0 }).kind"
        );
        // Record construction, fields in written order,
        // values rendered by the same emitter.
        let state_lit = IRNode::RecordLit {
            type_name: "TokenizerState".into(),
            fields: vec![
                ("source".into(), fa(v("state"), "source")),
                (
                    "pos".into(),
                    IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(fa(v("state"), "pos")),
                        Box::new(int(1)),
                    ),
                ),
            ],
        };
        // Phase 85 re-statement: a FieldAccess field
        // value now takes the clone form (a projection
        // is not a fresh value — rendered bare it moves
        // the field out of its base, E0382 when the base
        // is read again; measured on next_char's body in
        // the Phase 81 cross-validation). Phase 77 pinned
        // the bare form.
        assert_eq!(
            ir_to_rust_expr(&state_lit),
            "TokenizerState { source: state.source.clone(), pos: (state.pos + 1) }"
        );
        // A bare string-literal field value takes the
        // owned form (the defect-4d rule — the field's
        // declared type is the spec's String).
        let token_lit = IRNode::RecordLit {
            type_name: "Token".into(),
            fields: vec![
                ("kind".into(), int(0)),
                (
                    "lexeme".into(),
                    IRNode::Literal(IRLiteral::String("spec".into())),
                ),
            ],
        };
        assert_eq!(
            ir_to_rust_expr(&token_lit),
            "Token { kind: 0, lexeme: \"spec\".to_string() }"
        );
        // Tail position keeps the value (no trailing `;`):
        // a body that is a bare projection or a bare record
        // literal returns it.
        let neutral = TypeRef::Unit;
        assert_eq!(
            ir_to_rust_tail(&fa(v("state"), "pos"), 0, &neutral, None),
            "state.pos\n"
        );
        assert_eq!(
            ir_to_rust_tail(&state_lit, 0, &neutral, None),
            "TokenizerState { source: state.source.clone(), pos: (state.pos + 1) }\n"
        );
    }

    #[test]
    fn phase77_other_emitters_field_access_and_record_literals() {
        let fa = |base: IRNode, field: &str| IRNode::FieldAccess {
            base: Box::new(base),
            field: field.into(),
        };
        let v = |n: &str| IRNode::Var(n.into());
        let int = |n: i64| IRNode::Literal(IRLiteral::Int(n));
        let token_ir = IRNode::RecordLit {
            type_name: "Token".into(),
            fields: vec![
                ("kind".into(), int(0)),
                ("line".into(), int(1)),
            ],
        };
        // C: structs are the target's record concept and
        // generate_c carries the record-typedef rendering;
        // projection is member access, construction is a
        // C99 compound literal with designated
        // initializers. (Field VALUES ride the C
        // expression emitter's standing per-kind coverage
        // — Int here; a String field value would take its
        // standing fallback, a pre-existing C limitation
        // outside this phase's two nodes.)
        assert_eq!(ir_to_c_expr(&fa(v("state"), "pos")), "state.pos");
        assert_eq!(
            ir_to_c_expr(&token_ir),
            "(Token){ .kind = 0, .line = 1 }"
        );
        // Python: the emitter generates no class or
        // dataclass definitions for record types (ty_to_python
        // maps a named type to Any), so the declaration-free
        // record value is a dict and projection is its
        // subscript — the self-consistent pair.
        assert_eq!(
            ir_to_python_expr(&fa(v("state"), "pos")),
            "state[\"pos\"]"
        );
        let token_py = IRNode::RecordLit {
            type_name: "Token".into(),
            fields: vec![
                ("kind".into(), int(0)),
                ("lexeme".into(), IRNode::Literal(IRLiteral::String("spec".into()))),
            ],
        };
        assert_eq!(
            ir_to_python_expr(&token_py),
            "{\"kind\": 0, \"lexeme\": \"spec\"}"
        );
        // JavaScript: object literals and property access
        // are native and declaration-free — the exact
        // record model.
        assert_eq!(ir_to_js_expr(&fa(v("state"), "pos")), "state.pos");
        assert_eq!(
            ir_to_js_expr(&token_py),
            "{ kind: 0, lexeme: 'spec' }"
        );
        // WAT: the emitted WAT has no aggregate concept at
        // all (scalar params/results only, no memory or
        // struct model), so both nodes keep the emitter's
        // standing fallbacks — pinned here so the choice
        // is a recorded decision, not an omission.
        assert_eq!(ir_to_wasm_expr(&fa(v("state"), "pos")), "i32.const 0");
        assert_eq!(ir_to_wasm_expr(&token_ir), "i32.const 0");
        assert_eq!(ir_to_wasm(&fa(v("state"), "pos")), "    ;; node\n");
    }

    // The compile fixture: the restored tokenizer module
    // of Phases 73–76 (verbatim copy of the eval.rs const —
    // the current example file's types, the dropped
    // tokenize_one / tokenize_all constraints restored,
    // the 9 mirror tests in the D3a named form) plus ONE
    // fixture-only spec, `step_ok`, marked inline: the
    // module's own method-call constraints are
    // postconditions (generate does not render those) and
    // contain Field receivers (whose expr_to_rust
    // rendering stays the standing catch-all until Phase
    // 78), so the MethodCall arm's generated-code exercise
    // rides a precondition — the one constraint position
    // generate_rust DOES render (debug_assert) — on a
    // Result-typed parameter, the D1a table's home ground.
    // One measured subtlety, recorded rather than worked
    // around silently: a parsed `require: a == b` lands
    // in Constraint::Expr (parse_expr consumes `==` as a
    // BinOp), which the native constraint_to_rust renders
    // through its standing "true" catch-all — so the
    // fixture hands generate step_ok's precondition in
    // the engine-constructed Constraint::Eq form, the
    // shape that renderer's Eq arm (and the M3 corpus's
    // constraint cases) exists for. The spec text below
    // keeps its `require:` line: it parses and typechecks
    // as written; only the candidate's precondition VALUE
    // is transcribed, exactly as the fixture bodies are
    // transcriptions.
    const PHASE77_TOKENIZER_MODULE: &str = r#"ScratchTokenizer {

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

spec peek {
    input: state: TokenizerState
    output: Option<Char>

    constraint: state.pos<len(state.source)||output==None()
    constraint: state.pos>=len(state.source)||output==Some(state.source[state.pos])

    test "non-empty" { input: TokenizerState { source: "hi", pos: 0, line: 1, col: 1, tokens: [] } expect: Some('h') }
    test "at-end" { input: TokenizerState { source: "", pos: 0, line: 1, col: 1, tokens: [] } expect: None() }
}

spec next_char {
    input: state: TokenizerState
    output: TokenizerState

    constraint: output.pos==state.pos+1
    constraint: peek(state)!=Some('\n')||output.line==state.line+1
    constraint: peek(state)!=Some('\n')||output.col==1
    constraint: peek(state)==Some('\n')||output.col==state.col+1

    test "normal" {
        input: TokenizerState { source: "ab", pos: 0, line: 1, col: 1, tokens: [] };
        expect: TokenizerState { source: "ab", pos: 1, line: 1, col: 2, tokens: [] }
    }
}

spec skip_whitespace {
    input: state: TokenizerState
    output: TokenizerState

    constraint: output.pos>=state.pos
    constraint: len(output.source)==len(state.source)

    test "no-whitespace" {
        input: TokenizerState { source: "hi", pos: 0, line: 1, col: 1, tokens: [] };
        expect: TokenizerState { source: "hi", pos: 0, line: 1, col: 1, tokens: [] }
    }

    test "skip-spaces" {
        input: TokenizerState { source: "  hi", pos: 0, line: 1, col: 1, tokens: [] };
        expect: TokenizerState { source: "  hi", pos: 2, line: 1, col: 3, tokens: [] }
    }
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

    test "end-of-input" {
        input: TokenizerState { source: "", pos: 0, line: 1, col: 1, tokens: [] };
        expect: Ok(TokenizerState { source: "", pos: 0, line: 1, col: 1, tokens: [] })
    }
}

spec tokenize_all {
    input: state: TokenizerState
    output: Result<List<Token>, String>

    constraint: output.is_err() || output.unwrap().len() >= len(state.tokens)
    optimize: time < O(n)

    test "simple-spec" {
        input: TokenizerState { source: "spec sort", pos: 0, line: 1, col: 1, tokens: [] };
        expect: Ok([
            Token { kind: 0, lexeme: "spec", line: 1, col: 1 },
            Token { kind: 0, lexeme: "sort", line: 1, col: 6 }
        ])
    }

    test "empty" {
        input: TokenizerState { source: "", pos: 0, line: 1, col: 1, tokens: [] };
        expect: Ok([])
    }
}

spec step_ok {
    input: result: StepResult
    output: Bool

    require: result.is_ok() == true

    test "ok-state" { input: Ok(TokenizerState { source: "", pos: 0, line: 1, col: 1, tokens: [] }) expect: true }
}

}"#;

    // The Phase 48 runner prelude's shapes (the standing
    // harness-side supply): generate renders named types
    // verbatim and carries no module type declarations —
    // FunctionSpec::types() is empty by design — so the
    // record declarations a generated tokenizer file
    // needs ride the prelude, exactly as the Phase 48
    // compiled runner supplied them. (Promoting the
    // declarations into the emitter would need the
    // module context Phase 76 built for the verifier's
    // SpecIndex; generate's candidate API does not carry
    // it. Recorded in the Phase 77 ROAD note.)
    const PHASE77_PRELUDE: &str = r#"
#[derive(Debug, Clone, PartialEq)]
pub struct Token { pub kind: i64, pub lexeme: String, pub line: i64, pub col: i64 }
#[derive(Debug, Clone, PartialEq)]
pub struct TokenizerState { pub source: String, pub pos: i64, pub line: i64, pub col: i64, pub tokens: Vec<Token> }
pub type StepResult = Result<TokenizerState, String>;
"#;

    fn phase77_candidate(spec: FunctionSpec, body: IRNode) -> CandidateImplementation {
        CandidateImplementation {
            id: 1,
            body,
            spec_name: spec.name.clone(),
            spec,
            score: 1.0,
            scores: ScoreBreakdown {
                test_pass_ratio: 1.0,
                constraint_score: 1.0,
                complexity_score: 1.0,
                quality_score: 1.0,
                synthesis_time_us: 0,
            },
            generation: 0,
            provenance: Provenance::HumanGuided,
            verified: true,
            verification_status: None,
        }
    }

    #[test]
    fn phase77_tokenizer_generated_rust_compiles_and_runs() {
        use std::io::Write;
        let Some(rustc) = rustc_path() else {
            eprintln!("rustc not reachable; skipping Phase 77 fixture");
            return;
        };
        // The restored module checks whole (the Phase 74
        // headline, re-measured on this phase's copy).
        let parsed = crate::spec::parser::parse(PHASE77_TOKENIZER_MODULE)
            .expect("fixture module must parse");
        let checked = crate::spec::typeck::check(parsed)
            .expect("fixture module must typecheck");
        let specs = checked.spec.functions;
        let spec = |name: &str| {
            specs
                .iter()
                .find(|f| f.name == name)
                .unwrap_or_else(|| panic!("spec {name} must exist"))
                .clone()
        };
        let fa = |base: IRNode, field: &str| IRNode::FieldAccess {
            base: Box::new(base),
            field: field.into(),
        };
        let v = |n: &str| IRNode::Var(n.into());
        let int = |n: i64| IRNode::Literal(IRLiteral::Int(n));

        // Fixture bodies. next_char's is a REDUCED
        // transcription of the mirror body (the Phase 75
        // eval.rs transcription minus its newline branch):
        // the branch's condition indexes a String, and the
        // IR Index arm's standing rendering has no String
        // form in Rust — a pre-existing emitter limit
        // outside this phase's two nodes (peek is excluded
        // from the fixture for the same reason; Phases
        // 75/76 proved both on the interpreter/Z3 routes).
        // The reduced body still constructs the record
        // from field projections — the shapes under test.
        let next_char_body = IRNode::RecordLit {
            type_name: "TokenizerState".into(),
            fields: vec![
                ("source".into(), fa(v("state"), "source")),
                (
                    "pos".into(),
                    IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(fa(v("state"), "pos")),
                        Box::new(int(1)),
                    ),
                ),
                ("line".into(), fa(v("state"), "line")),
                (
                    "col".into(),
                    IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(fa(v("state"), "col")),
                        Box::new(int(1)),
                    ),
                ),
                ("tokens".into(), fa(v("state"), "tokens")),
            ],
        };
        // The Phase 75/76 partial bodies: they compile and
        // their generated tests judge them honestly —
        // each passes the degenerate test and fails the
        // token-producing one, as measured there.
        let tokenize_one_body = IRNode::Call {
            function: "Ok".into(),
            args: vec![v("state")],
        };
        let tokenize_all_body = IRNode::Call {
            function: "Ok".into(),
            args: vec![fa(v("state"), "tokens")],
        };
        // step_ok's body: the manual transcription of
        // is_ok as a match (method calls have no IR kind —
        // the Phase 76 design fact).
        let step_ok_body = IRNode::Match {
            scrutinee: Box::new(v("result")),
            cases: vec![
                (
                    IRPattern::Constructor("Ok".into(), vec![IRPattern::Wildcard]),
                    IRNode::Literal(IRLiteral::Bool(true)),
                ),
                (
                    IRPattern::Constructor("Err".into(), vec![IRPattern::Wildcard]),
                    IRNode::Literal(IRLiteral::Bool(false)),
                ),
            ],
        };

        // generate_rust directly (not the `generate`
        // route): this phase's subject is the native
        // emitter in both configurations; the compiled
        // adapter behind bootstrap-codegen gains its arms
        // in Phase 78.
        let mut file = String::from(PHASE77_PRELUDE);
        for (name, body) in [
            ("next_char", next_char_body),
            ("tokenize_one", tokenize_one_body),
            ("tokenize_all", tokenize_all_body),
            ("step_ok", step_ok_body),
        ] {
            let mut spec = spec(name);
            if name == "step_ok" {
                // The Eq-form transcription of the
                // spec's parsed `require:` (see the
                // module const's comment): the same
                // precondition as the engine-constructed
                // Constraint::Eq the native renderer's
                // Eq arm serves.
                spec.preconditions = vec![Constraint::Eq(
                    Expr::MethodCall {
                        receiver: Box::new(Expr::Var("result".into())),
                        name: "is_ok".into(),
                        args: vec![],
                    },
                    Expr::BoolLit(true),
                )];
            }
            let cand = phase77_candidate(spec, body);
            let generated = generate_rust(&cand).expect("generate fixture winner");
            // The m3_gate wrap: one module per winner so
            // the per-file `mod tests` cannot collide.
            file.push_str(&format!("mod gen_{} {{\nuse super::*;\n", name));
            file.push_str(&generated);
            file.push_str("}\n\n");
        }
        // The method-call arm's rendering is IN the
        // generated code (step_ok's precondition assert).
        assert!(
            file.contains("debug_assert!(result.is_ok() == true"),
            "generated file must carry the rendered method call:\n{}",
            file
        );
        // And the record literal + field projection
        // renderings are in the generated test code / body.
        assert!(
            file.contains("Token { kind: 0, lexeme: \"spec\".to_string(), line: 1, col: 1 }"),
            "generated file must carry the rendered record literal:\n{}",
            file
        );
        assert!(
            file.contains("state.tokens"),
            "generated file must carry the rendered field access:\n{}",
            file
        );

        let dir = std::env::temp_dir().join(format!(
            "morphic_phase77_fixture_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("tokenizer_fixture.rs");
        let mut f = std::fs::File::create(&src).unwrap();
        f.write_all(file.as_bytes()).unwrap();
        drop(f);
        let bin = dir.join("tokenizer_fixture_test");
        let compile = std::process::Command::new(&rustc)
            .args(["--edition", "2021", "--test"])
            .arg(&src)
            .arg("-o")
            .arg(&bin)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "Phase 77 fixture does not compile:\n{}\n--- file ---\n{}",
            String::from_utf8_lossy(&compile.stderr),
            file
        );
        let run = std::process::Command::new(&bin).output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
        // Measured outcomes: next_char 1/1 and step_ok 1/1
        // pass; the two partial bodies each pass their
        // degenerate test and fail the token-producing
        // one — 4 passed, 2 failed, the failures named.
        assert!(
            stdout.contains("4 passed; 2 failed"),
            "fixture test outcomes differ from the measured expectation:\n{}",
            stdout
        );
        assert!(
            stdout.contains("keyword_spec") && stdout.contains("simple_spec"),
            "the failing tests must be the two token-producing ones:\n{}",
            stdout
        );
    }

    // ── Phase 85 (M1 generation campaign, Leg 1): the IR
    // walker's renderings for the shapes the record-era
    // seed bodies carry — the parser's pseudo-calls as IR
    // Calls (List / None / len) and String indexing. Until
    // this phase these rendered verbatim (`List(...)`,
    // `None()`, `len(...)`, `(state.source)[state.pos]`)
    // and every generated peek / next_char / tokenize body
    // died at rustc (E0425 / E0618 / E0277 in the compiled
    // cross-validation runner, Phases 77/81/84). The List
    // and None forms mirror the spec-Expr walker's Phase
    // 55 pseudo-call forms exactly; the len and Index
    // forms carry the language's own semantics (the
    // interpreter's chars().count() length; the Z3 CharStr
    // bridge's scalar index, out-of-bounds totalized to
    // the NUL sentinel — see the arms' comments).

    #[test]
    fn phase85_ir_call_and_index_renderings() {
        let v = |n: &str| IRNode::Var(n.into());
        let fa = |base: IRNode, field: &str| IRNode::FieldAccess {
            base: Box::new(base),
            field: field.into(),
        };
        let call = |name: &str, args: Vec<IRNode>| IRNode::Call {
            function: name.into(),
            args,
        };
        // List: the vec literal, as in the spec walker.
        assert_eq!(
            ir_to_rust_expr(&call(
                "List",
                vec![
                    IRNode::Literal(IRLiteral::Int(1)),
                    IRNode::Literal(IRLiteral::Int(2))
                ]
            )),
            "vec![1, 2]"
        );
        assert_eq!(ir_to_rust_expr(&call("List", vec![])), "vec![]");
        // None: the bare constructor value, never a call.
        assert_eq!(ir_to_rust_expr(&call("None", vec![])), "None");
        // Some / Ok / Err keep the plain call form (real
        // Rust constructors).
        assert_eq!(
            ir_to_rust_expr(&call("Some", vec![IRNode::Literal(IRLiteral::Int(1))])),
            "Some(1)"
        );
        // len over a record's String field: the char count
        // as i64 (the interpreter's semantics — a byte
        // count would misjudge every non-ASCII source).
        assert_eq!(
            ir_to_rust_expr(&call("len", vec![fa(v("state"), "source")])),
            "(state.source).chars().count() as i64"
        );
        // len over a plain list variable: the element
        // count as i64.
        assert_eq!(
            ir_to_rust_expr(&call("len", vec![v("items")])),
            "(items).len() as i64"
        );
        // String indexing over a record field: scalar
        // char access, index cast, NUL totalization.
        assert_eq!(
            ir_to_rust_expr(&IRNode::Index(
                Box::new(fa(v("state"), "source")),
                Box::new(fa(v("state"), "pos"))
            )),
            "(state.source).chars().nth((state.pos) as usize).unwrap_or('\\0')"
        );
        // The corpus-pinned list form is unchanged: a Var
        // base keeps the bracket rendering.
        assert_eq!(
            ir_to_rust_expr(&IRNode::Index(
                Box::new(v("items")),
                Box::new(IRNode::Literal(IRLiteral::Int(0)))
            )),
            "(items)[0]"
        );
        // An ordinary spec-to-spec call is untouched.
        assert_eq!(
            ir_to_rust_expr(&call("peek", vec![v("state")])),
            "peek(state)"
        );
    }

    #[test]
    fn phase100_int_to_str_rendering() {
        // F2 (Phase 100): the Rust emitter's `int_to_str`
        // arm — `(n).to_string()`, Rust's own Display, so
        // the compiled route is exact by construction
        // (negatives and i64 extremes included).
        let v = |n: &str| IRNode::Var(n.into());
        let call = |name: &str, args: Vec<IRNode>| IRNode::Call {
            function: name.into(),
            args,
        };
        assert_eq!(
            ir_to_rust_expr(&call("int_to_str", vec![v("n")])),
            "(n).to_string()"
        );
        assert_eq!(
            ir_to_rust_expr(&call(
                "int_to_str",
                vec![IRNode::Literal(IRLiteral::Int(-42))]
            )),
            "(-42).to_string()"
        );
        // Wrong arity keeps the plain call form — the
        // standing catch-all, never a guessed rendering.
        assert_eq!(
            ir_to_rust_expr(&call("int_to_str", vec![v("n"), v("n")])),
            "int_to_str(n, n)"
        );
    }

    #[test]
    fn phase100_int_to_str_generated_rust_compiles_and_runs() {
        // The conversion battery through the COMPILED
        // route: a body emitted by `generate_rust`,
        // compiled with rustc and executed — its outputs
        // must equal Rust's `Display` for every battery
        // value, the same table the interpreter battery
        // and the encoder fold battery judge. (This calls
        // `generate_rust` directly, not the feature-routed
        // `generate`: the bootstrap route is the
        // synthesized emitter, whose spec corpus predates
        // the builtin — F2's recorded no-cascade stance.)
        use std::io::Write;
        let Some(rustc) = rustc_path() else {
            eprintln!("rustc not reachable; skipping Phase 100 fixture");
            return;
        };
        const BATTERY: &[(&str, i64)] = &[
            ("0i64", 0),
            ("1i64", 1),
            ("2i64", 2),
            ("3i64", 3),
            ("9i64", 9),
            ("10i64", 10),
            ("42i64", 42),
            ("-1i64", -1),
            ("-42i64", -42),
            ("1000000i64", 1_000_000),
            ("i64::MAX", i64::MAX),
            ("i64::MIN", i64::MIN),
        ];
        let spec = FunctionSpec {
            name: "render_int".into(),
            doc: None,
            generics: vec![],
            params: vec![Param {
                name: "n".into(),
                ty: TypeRef::Int,
                annotations: HashMap::new(),
            }],
            return_type: TypeRef::String,
            preconditions: vec![],
            postconditions: vec![],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: HashMap::new(),
        };
        let cand = CandidateImplementation {
            id: 1,
            body: IRNode::Call {
                function: "int_to_str".into(),
                args: vec![IRNode::Var("n".into())],
            },
            spec_name: "render_int".into(),
            spec,
            score: 1.0,
            scores: ScoreBreakdown {
                test_pass_ratio: 1.0,
                constraint_score: 1.0,
                complexity_score: 1.0,
                quality_score: 1.0,
                synthesis_time_us: 0,
            },
            generation: 0,
            provenance: Provenance::HumanGuided,
            verified: true,
            verification_status: None,
        };
        let generated = generate_rust(&cand).expect("generate Phase 100 fixture");
        assert!(
            generated.contains("(n).to_string()"),
            "generated body must carry the int_to_str rendering:\n{}",
            generated
        );
        let mut file = generated;
        file.push_str("\nfn main() {\n");
        for (expr, _) in BATTERY {
            file.push_str(&format!("    println!(\"{{}}\", render_int({}));\n", expr));
        }
        file.push_str("}\n");
        let dir = std::env::temp_dir().join(format!(
            "morphic_phase100_fixture_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("render_int_fixture100.rs");
        let mut f = std::fs::File::create(&src).unwrap();
        f.write_all(file.as_bytes()).unwrap();
        drop(f);
        let bin = dir.join("render_int_fixture100");
        let compile = std::process::Command::new(&rustc)
            .args(["--edition", "2021"])
            .arg(&src)
            .arg("-o")
            .arg(&bin)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "Phase 100 fixture does not compile:\n{}\n--- file ---\n{}",
            String::from_utf8_lossy(&compile.stderr),
            file
        );
        let run = std::process::Command::new(&bin).output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(run.status.success(), "Phase 100 fixture runner failed");
        let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
        let lines: Vec<&str> = stdout.lines().collect();
        let expected: Vec<String> = BATTERY.iter().map(|(_, n)| format!("{}", n)).collect();
        assert_eq!(
            lines,
            expected.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            "compiled int_to_str outputs must equal Rust Display on the battery"
        );
    }

    #[test]
    fn phase85_peek_next_char_generated_rust_compiles_and_runs() {
        use std::io::Write;
        let Some(rustc) = rustc_path() else {
            eprintln!("rustc not reachable; skipping Phase 85 fixture");
            return;
        };
        let parsed = crate::spec::parser::parse(PHASE77_TOKENIZER_MODULE)
            .expect("fixture module must parse");
        let checked = crate::spec::typeck::check(parsed)
            .expect("fixture module must typecheck");
        let specs = checked.spec.functions;
        let spec = |name: &str| {
            specs
                .iter()
                .find(|f| f.name == name)
                .unwrap_or_else(|| panic!("spec {name} must exist"))
                .clone()
        };
        let fa = |base: IRNode, field: &str| IRNode::FieldAccess {
            base: Box::new(base),
            field: field.into(),
        };
        let v = |n: &str| IRNode::Var(n.into());
        let int = |n: i64| IRNode::Literal(IRLiteral::Int(n));
        let call = |name: &str, args: Vec<IRNode>| IRNode::Call {
            function: name.into(),
            args,
        };
        // The Phase 81 complementary-guard seed body for
        // peek, node for node as the cross-validation dump
        // records it.
        let peek_body = || IRNode::If {
            condition: Box::new(IRNode::BinOp(
                IRBinOp::Lt,
                Box::new(fa(v("state"), "pos")),
                Box::new(call("len", vec![fa(v("state"), "source")])),
            )),
            then_branch: Box::new(call(
                "Some",
                vec![IRNode::Index(
                    Box::new(fa(v("state"), "source")),
                    Box::new(fa(v("state"), "pos")),
                )],
            )),
            else_branch: Box::new(call("None", vec![])),
        };
        // The Phase 81 record-completion seed body for
        // next_char: peek's body inlined at both guard
        // positions, as the seed constructs it.
        let next_char_body = IRNode::RecordLit {
            type_name: "TokenizerState".into(),
            fields: vec![
                ("source".into(), fa(v("state"), "source")),
                (
                    "pos".into(),
                    IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(fa(v("state"), "pos")),
                        Box::new(int(1)),
                    ),
                ),
                (
                    "line".into(),
                    IRNode::If {
                        condition: Box::new(IRNode::BinOp(
                            IRBinOp::Neq,
                            Box::new(peek_body()),
                            Box::new(call(
                                "Some",
                                vec![IRNode::Literal(IRLiteral::Char('\n'))],
                            )),
                        )),
                        then_branch: Box::new(fa(v("state"), "line")),
                        else_branch: Box::new(IRNode::BinOp(
                            IRBinOp::Add,
                            Box::new(fa(v("state"), "line")),
                            Box::new(int(1)),
                        )),
                    },
                ),
                (
                    "col".into(),
                    IRNode::If {
                        condition: Box::new(IRNode::BinOp(
                            IRBinOp::Neq,
                            Box::new(peek_body()),
                            Box::new(call(
                                "Some",
                                vec![IRNode::Literal(IRLiteral::Char('\n'))],
                            )),
                        )),
                        then_branch: Box::new(IRNode::BinOp(
                            IRBinOp::Add,
                            Box::new(fa(v("state"), "col")),
                            Box::new(int(1)),
                        )),
                        else_branch: Box::new(int(1)),
                    },
                ),
                ("tokens".into(), fa(v("state"), "tokens")),
            ],
        };
        let mut file = String::from(PHASE77_PRELUDE);
        for (name, body) in [("peek", peek_body()), ("next_char", next_char_body)] {
            let cand = phase77_candidate(spec(name), body);
            let generated = generate_rust(&cand).expect("generate fixture winner");
            file.push_str(&format!("mod gen_{} {{\nuse super::*;\n", name));
            file.push_str(&generated);
            file.push_str("}\n\n");
        }
        assert!(
            file.contains(".chars().nth(") && file.contains(".chars().count() as i64"),
            "generated file must carry the Phase 85 renderings:\n{}",
            file
        );
        assert!(
            !file.contains("None()") && !file.contains("len(state.source)"),
            "generated file must not carry the pre-Phase-85 forms:\n{}",
            file
        );
        let dir = std::env::temp_dir().join(format!(
            "morphic_phase85_fixture_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("tokenizer_fixture85.rs");
        let mut f = std::fs::File::create(&src).unwrap();
        f.write_all(file.as_bytes()).unwrap();
        drop(f);
        let bin = dir.join("tokenizer_fixture85_test");
        let compile = std::process::Command::new(&rustc)
            .args(["--edition", "2021", "--test"])
            .arg(&src)
            .arg("-o")
            .arg(&bin)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "Phase 85 fixture does not compile:\n{}\n--- file ---\n{}",
            String::from_utf8_lossy(&compile.stderr),
            file
        );
        let run = std::process::Command::new(&bin).output().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
        // peek's two in-spec tests and next_char's one all
        // pass against the seed bodies — the Phase 81
        // bodies, executed end-to-end for the first time.
        assert!(
            stdout.contains("3 passed; 0 failed"),
            "fixture test outcomes differ from the measured expectation:\n{}",
            stdout
        );
    }
}

