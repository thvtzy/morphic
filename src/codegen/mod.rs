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
            .map(|gp| gp.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        format!("<{}>", g)
    };

    let params: Vec<String> = candidate.spec.params.iter()
        .map(|p| format!("{}: {}", p.name, ty_to_rust(&p.ty)))
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
        candidate.spec.name, generics, params.join(", "), ret
    ));

    // Body
    let body_code = ir_to_rust(&candidate.body, 1);
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
            out.push_str(&format!("    fn {}() {{\n", test_name));
            out.push_str(&format!(
                "        let result = {}({});\n",
                candidate.spec.name,
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

fn ir_to_rust(node: &IRNode, indent: usize) -> String {
    let pad = "    ".repeat(indent);
    let pad_inner = "    ".repeat(indent + 1);

    match node {
        IRNode::Hole { .. } => {
            format!("{}// TODO: synthesis incomplete\n{}panic!(\"Hole not filled\");\n", pad, pad)
        }

        IRNode::Block(stmts) => {
            let mut out = String::new();
            for stmt in stmts {
                out.push_str(&ir_to_rust(stmt, indent));
            }
            out
        }

        IRNode::Let { name, value, body } => {
            format!(
                "{}let {} = {};\n{}",
                pad, name, ir_to_rust_expr(value), ir_to_rust(body, indent)
            )
        }

        IRNode::If { condition, then_branch, else_branch } => {
            format!(
                "{}if {} {{\n{}}} else {{\n{}}}\n",
                pad,
                ir_to_rust_expr(condition),
                ir_to_rust(then_branch, indent + 1),
                ir_to_rust(else_branch, indent + 1),
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
                ir_to_rust_expr(condition),
                ir_to_rust(body, indent + 1),
                pad,
            )
        }

        IRNode::For { var, start, end, body } => {
            format!(
                "{}for {} in {}..{} {{\n{}{}}}\n",
                pad, var,
                ir_to_rust_expr(start),
                ir_to_rust_expr(end),
                ir_to_rust(body, indent + 1),
                pad,
            )
        }

        IRNode::Match { scrutinee, cases } => {
            let mut out = format!("{}match {} {{\n", pad, ir_to_rust_expr(scrutinee));
            for (pattern, body) in cases {
                let pat = ir_pattern_to_rust(pattern);
                out.push_str(&format!("{}{} => {{\n{}{}}},\n",
                    pad_inner, pat,
                    ir_to_rust(body, indent + 2),
                    pad_inner,
                ));
            }
            out.push_str(&format!("{}}}\n", pad));
            out
        }

        IRNode::Call { function, args } => {
            let args_str: Vec<String> = args.iter().map(|a| ir_to_rust_expr(a)).collect();
            format!("{}{}({})\n", pad, function, args_str.join(", "))
        }

        IRNode::Return(expr) => {
            format!("{}return {};\n", pad, ir_to_rust_expr(expr))
        }

        IRNode::Alloc { name, ty, initial } => {
            format!(
                "{}let mut {}: {} = {};\n",
                pad, name, ty_to_rust(ty), ir_to_rust_expr(initial)
            )
        }

        IRNode::Assign { target, value } => {
            format!(
                "{}{} = {};\n",
                pad, ir_to_rust_expr(target), ir_to_rust_expr(value)
            )
        }

        IRNode::Collection { kind, elements } => {
            match kind {
                CollectionKind::List => {
                    let elems: Vec<String> = elements.iter().map(|e| ir_to_rust_expr(e)).collect();
                    format!("{}vec![{}]\n", pad, elems.join(", "))
                }
                CollectionKind::Array => {
                    let elems: Vec<String> = elements.iter().map(|e| ir_to_rust_expr(e)).collect();
                    format!("{}[{}]\n", pad, elems.join(", "))
                }
                CollectionKind::Tuple => {
                    let elems: Vec<String> = elements.iter().map(|e| ir_to_rust_expr(e)).collect();
                    format!("{}({})\n", pad, elems.join(", "))
                }
                CollectionKind::Set => {
                    let elems: Vec<String> = elements.iter().map(|e| ir_to_rust_expr(e)).collect();
                    format!("{}vec![{}]\n", pad, elems.join(", "))
                }
                CollectionKind::Map => {
                    let elems: Vec<String> = elements.iter().map(|e| ir_to_rust_expr(e)).collect();
                    format!("{}vec![{}]\n", pad, elems.join(", "))
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

        IRNode::Typed { node, .. } => ir_to_rust(node, indent),

        // Expressions that shouldn't appear as statements
        _ => format!("{}{};\n", pad, ir_to_rust_expr(node)),
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
        IRNode::Var(name) => name.clone(),
        IRNode::BinOp(op, lhs, rhs) => {
            let rust_op = match op {
                IRBinOp::Add => "+", IRBinOp::Sub => "-",
                IRBinOp::Mul => "*", IRBinOp::Div => "/",
                IRBinOp::Mod => "%",
                IRBinOp::And => "&&", IRBinOp::Or => "||",
                IRBinOp::Eq => "==", IRBinOp::Neq => "!=",
                IRBinOp::Lt => "<", IRBinOp::Le => "<=",
                IRBinOp::Gt => ">", IRBinOp::Ge => ">=",
                IRBinOp::Shl => "<<", IRBinOp::Shr => ">>",
                IRBinOp::Concat => "+",
                IRBinOp::Append => ".push",
                IRBinOp::Merge => ".extend",
                _ => "UNKNOWN_OP",
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
            let rust_op = match op {
                IRUnaryOp::Neg => "-", IRUnaryOp::Not => "!",
                IRUnaryOp::Abs => ".abs()", IRUnaryOp::Len => ".len()",
                IRUnaryOp::Clone => ".clone()",
                IRUnaryOp::Reverse => ".reverse()",
                _ => "UNKNOWN_UNOP",
            };
            if *op == IRUnaryOp::Len || *op == IRUnaryOp::Abs ||
               *op == IRUnaryOp::Clone || *op == IRUnaryOp::Reverse {
                format!("{}{}", ir_to_rust_expr(inner), rust_op)
            } else {
                format!("({} {})", rust_op, ir_to_rust_expr(inner))
            }
        }
        IRNode::Call { function, args } => {
            let args_str: Vec<String> = args.iter().map(|a| ir_to_rust_expr(a)).collect();
            format!("{}({})", function, args_str.join(", "))
        }
        IRNode::If { condition, then_branch, else_branch } => {
            format!(
                "if {} {{ {} }} else {{ {} }}",
                ir_to_rust_expr(condition),
                ir_to_rust_expr(then_branch),
                ir_to_rust_expr(else_branch),
            )
        }
        IRNode::Lambda { params, body } => {
            let params_str: Vec<String> = params.iter()
                .map(|(n, _)| n.clone())
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
                CollectionKind::Set => format!("HashSet::from([{}])", elems.join(", ")),
                CollectionKind::Map => format!("HashMap::from([{}])", elems.join(", ")),
            }
        }
        IRNode::Typed { node, .. } => ir_to_rust_expr(node),
        _ => "/* UNSUPPORTED */".into(),
    }
}

fn ir_pattern_to_rust(pattern: &IRPattern) -> String {
    match pattern {
        IRPattern::Wildcard => "_".into(),
        IRPattern::Variable(name) => name.clone(),
        IRPattern::Literal(lit) => match lit {
            IRLiteral::Int(n) => n.to_string(),
            IRLiteral::Bool(b) => b.to_string(),
            IRLiteral::String(s) => format!("\"{}\"", s),
            _ => "_".into(),
        },
        IRPattern::Constructor(name, sub_patterns) => {
            let subs: Vec<String> = sub_patterns.iter().map(ir_pattern_to_rust).collect();
            if subs.is_empty() {
                name.clone()
            } else {
                format!("{}({})", name, subs.join(", "))
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
        TypeRef::Named(name) | TypeRef::Generic(name) => name.clone(),
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
        Expr::Var(name) => name.clone(),
        Expr::Call(name, args) => {
            let args_str: Vec<String> = args.iter().map(expr_to_rust).collect();
            format!("{}({})", name, args_str.join(", "))
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
