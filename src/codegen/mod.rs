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
        "rust" => generate_rust(candidate),
        "c" => generate_c(candidate),
        "wasm" => generate_wasm(candidate),
        "python" => generate_python(candidate),
        "javascript" | "js" => generate_javascript(candidate),
        "ir" => Ok(debug_ir_string(&candidate.body, 0)),
        other => Err(CodegenError::UnsupportedTarget(other.into())),
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

    // Function signature
    let generics = if candidate.spec.generics.is_empty() {
        String::new()
    } else {
        let g = candidate.spec.generics.iter()
            .map(|gp| rust_ident(&gp.name))
            .collect::<Vec<_>>()
            .join(", ");
        format!("<{}>", g)
    };

    let params: Vec<String> = candidate.spec.params.iter()
        .map(|p| format!("{}: {}", rust_ident(&p.name), ty_to_rust(&p.ty)))
        .collect();

    let ret = ty_to_rust(&candidate.spec.return_type);

    // Precondition asserts (debug-only)
    if !candidate.spec.preconditions.is_empty() {
        out.push_str("#[cfg(debug_assertions)]\n");
        out.push_str("{\n");
        for pre in &candidate.spec.preconditions {
            out.push_str(&format!("    debug_assert!({}, \"Precondition violated\");\n",
                constraint_to_rust(pre)));
        }
        out.push_str("}\n\n");
    }

    // Function definition
    out.push_str(&format!(
        "pub fn {}{}({}) -> {} {{\n",
        rust_ident(&candidate.spec.name), generics, params.join(", "), ret
    ));

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
            out.push_str(&format!(
                "        let result = {}({});\n",
                rust_ident(&candidate.spec.name),
                expr_to_rust(&test.input)
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
            let mut out = format!("{}match {} {{\n", pad, expr_in_condition_position(scrutinee));
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
            let args_str: Vec<String> = args.iter().map(|a| ir_to_rust_expr(a)).collect();
            format!("{}{}({})\n", pad, rust_ident(function), args_str.join(", "))
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
            format!(
                "{}({}).iter().map({}).collect()\n",
                pad,
                ir_to_rust_expr(over),
                ir_to_rust_expr(function),
            )
        }

        IRNode::Filter { predicate, over } => {
            format!(
                "{}({}).iter().filter({}).collect()\n",
                pad,
                ir_to_rust_expr(over),
                ir_to_rust_expr(predicate),
            )
        }

        IRNode::Fold { operation, initial, over } => {
            format!(
                "{}({}).iter().fold({}, {})\n",
                pad,
                ir_to_rust_expr(over),
                ir_to_rust_expr(initial),
                ir_to_rust_expr(operation),
            )
        }

        IRNode::Sort { comparator, collection } => {
            format!(
                "{}let mut sorted = {};\nsorted.sort_by({});\nsorted\n",
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
        | IRNode::Lambda { .. } | IRNode::Collection { .. } => {
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
            format!("\"{}\".to_string()", s)
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
    match node {
        IRNode::Match { .. }
        | IRNode::If { .. }
        | IRNode::Loop { .. }
        | IRNode::While { .. }
        | IRNode::For { .. }
        | IRNode::Let { .. }
        | IRNode::Block(_)
        | IRNode::StructDef { .. }
        | IRNode::Lambda { .. }
        | IRNode::Closure { .. } => format!("({})", ir_to_rust_expr(node)),
        _ => ir_to_rust_expr(node),
    }
}

fn ir_to_rust_expr(node: &IRNode) -> String {
    match node {
        IRNode::Literal(lit) => match lit {
            IRLiteral::Int(n) => n.to_string(),
            IRLiteral::Float(f) => f.to_string(),
            IRLiteral::Bool(b) => b.to_string(),
            IRLiteral::String(s) => format!("\"{}\"", s),
            IRLiteral::Char(c) => format!("'{}'", c),
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
        IRNode::Call { function, args } => {
            let args_str: Vec<String> = args.iter().map(|a| ir_to_rust_expr(a)).collect();
            format!("{}({})", rust_ident(function), args_str.join(", "))
        }
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
                expr_in_condition_position(scrutinee),
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
        IRNode::Loop { body } => {
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
            format!("({})[{}]", ir_to_rust_expr(coll), ir_to_rust_expr(idx))
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
            IRLiteral::String(s) => format!("\"{}\"", s),
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
        Expr::StringLit(s) => format!("\"{}\"", s),
        Expr::Var(name) => rust_ident(name),
        Expr::Call(name, args) => {
            let args_str: Vec<String> = args.iter().map(expr_to_rust).collect();
            format!("{}({})", rust_ident(name), args_str.join(", "))
        }
        Expr::BinOp(op, lhs, rhs) => {
            let rust_op = match op {
                BinOp::Add => "+", BinOp::Sub => "-",
                BinOp::Mul => "*", BinOp::Div => "/",
                BinOp::Eq => "==", BinOp::Neq => "!=",
                BinOp::And => "&&", BinOp::Or => "||",
                _ => "?",
            };
            format!("({} {} {})", expr_to_rust(lhs), rust_op, expr_to_rust(rhs))
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
            },
            IRNode::Break { value: None },
            IRNode::Break { value: Some(leaf()) },
            IRNode::StructDef {
                name: "Point".into(),
                fields: vec![("x".into(), TypeRef::Int)],
                body: leaf(),
            },
            IRNode::TryPropagate(Box::new(IRNode::Var("kind".into()))),
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
            // Call in statement position.
            IRNode::Call {
                function: "check".into(),
                args: vec![IRNode::Var("kind".into())],
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

    /// Compile a generated file with `rustc --edition 2021
    /// --crate-type lib` (the probe's external check, in-process).
    /// Skips silently only when no rustc is reachable at all.
    fn compile_rust_lib(code: &str) {
        use std::io::Write;
        let rustc = std::process::Command::new("rustc")
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
            });
        let Some(rustc) = rustc else {
            eprintln!("rustc not reachable; skipping compile check");
            return;
        };
        let dir = std::env::temp_dir().join(format!(
            "morphic_codegen_compile_{}",
            std::process::id()
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
}
