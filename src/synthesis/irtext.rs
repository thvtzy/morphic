// ─────────────────────────────────────────────────────────────
//  IR TEXT SERIALIZATION (Phase 150, 1.4 PCC —
//  DISEBAR_SCOPE §4.2)
//
//  The one real build inside proof-carrying code: a text
//  serialization of `IRNode` that is STABLE (the same body
//  serializes byte-identically), VERSIONED (the text
//  carries its version on its first line; a reader refuses
//  a version it does not know, by name), and PARSEABLE (a
//  reader reconstructs the body the writer held — the
//  round-trip law below). Until this phase, bodies
//  travelled as Rust `Debug` identity text (the library
//  fetch's form): a fingerprint, not a format — nothing
//  could parse it back, and nothing versioned it. A
//  re-verification bundle (§4.1) needs the body in a form
//  a stranger's binary can read, so the form is specified
//  here, not incidental.
//
//  THE FORMAT — `morphic-ir/1`. Line 1 is the version
//  marker, exactly `morphic-ir/1`. The rest is one node in
//  a prefix parenthesized grammar; tokens are `(`, `)`,
//  bare atoms (tags, operator names, numbers) and strings
//  (JSON-quoted, so any name, invariant text or literal
//  survives verbatim). Grammar (n = node, t = type,
//  p = pattern, s = string, i = integer):
//
//    text    := "morphic-ir/1" node
//    node    := "(hole" i typeopt ")"
//             | "(block" n* ")"
//             | "(let" s n n ")"
//             | "(if" n n n ")"
//             | "(while" n n stropt ")"
//             | "(for" s n n n ")"
//             | "(match" n case* ")"
//             | "(call" s n* ")"
//             | "(return" n ")"
//             | "(lit" literal ")"
//             | "(var" s ")"
//             | "(binop" BINOP n n ")"
//             | "(unop" UNOP n ")"
//             | "(lambda" params n ")"
//             | "(closure" params "(" s* ")" n ")"
//             | "(collection" KIND n* ")"
//             | "(map" n n ")" | "(filter" n n ")"
//             | "(fold" n n n ")" | "(sort" n n ")"
//             | "(alloc" s t n ")"
//             | "(assign" n n ")"
//             | "(typed" n t ")"
//             | "(index" n n ")"
//             | "(loop" n stropt ")"
//             | "(break" nodeopt ")"
//             | "(structdef" s fields n ")"
//             | "(try" n ")"
//             | "(field" n s ")"
//             | "(record" s "(" ( "(" s n ")" )* ")" ")"
//    case    := "(case" p n ")"
//    params  := "(" ( "(" s t ")" )* ")"
//    fields  := "(" ( "(" s t ")" )* ")"
//    typeopt := "none" | "(some" t ")"
//    stropt  := "none" | "(some" s ")"
//    nodeopt := "none" | "(some" n ")"
//    literal := "(int" i ")" | "(float" FLOAT ")"
//             | "(bool" ("true"|"false") ")"
//             | "(string" s ")" | "(char" s ")" | "unit"
//    pattern := "wildcard" | "(pvar" s ")"
//             | "(plit" literal ")"
//             | "(ctor" s p* ")" | "(guard" p s ")"
//    type    := "Int"|"Float"|"Bool"|"String"|"Char"|"Unit"
//             | "I8"|"I16"|"I32"|"I64"|"U8"|"U16"|"U32"|"U64"
//             | "(Generic" s ")" | "(Named" s ")"
//             | "(List" t ")" | "(Set" t ")"
//             | "(Map" t t ")" | "(Option" t ")"
//             | "(Result" t t ")" | "(Tuple" t* ")"
//             | "(Array" t i ")" | "(Function" "(" t* ")" t ")"
//             | "(Ref" t ")" | "(Stream" t ")"
//
//  FLOAT is the f64's Debug rendering, which round-trips
//  through the standard parser (including `inf`, `-inf`,
//  `NaN`). A char literal is a string of exactly one code
//  point.
//
//  TOTALITY IS THE ANTI-FABRICATION PROPERTY: the writer
//  covers the whole IR inventory with an exhaustive match
//  — a node form the grammar lacked would fail to compile
//  the writer, never serialize approximately or drop
//  silently. The reader is the mirror: it declines by
//  name (unknown version, unknown tag, wrong shape,
//  trailing content), never guesses. Round-trip identity
//  is judged by the pipeline's own body fingerprint, the
//  Debug rendering: parse(write(body)) is Debug-identical
//  to body, and write(parse(write(body))) is byte-equal
//  to write(body) — pinned over every shipped body and a
//  synthetic inventory battery in tests/phase150_pcc.rs.
// ─────────────────────────────────────────────────────────────

use crate::spec::ast::TypeRef;

use super::engine::{CollectionKind, IRBinOp, IRLiteral, IRNode, IRPattern, IRUnaryOp};

/// The format's version marker: the first line of every
/// IR text, and the only version this build reads.
pub const IR_TEXT_VERSION: &str = "morphic-ir/1";

// ── The writer ──────────────────────────────────────────

/// Serialize a body to IR text: the version line, then
/// the node in the grammar above. Deterministic — the
/// same body always produces the same bytes.
pub fn write_ir(node: &IRNode) -> String {
    let mut out = String::new();
    out.push_str(IR_TEXT_VERSION);
    out.push('\n');
    write_node(node, &mut out);
    out.push('\n');
    out
}

fn w_str(out: &mut String, s: &str) {
    out.push_str(&serde_json::to_string(s).expect("a &str always quotes as JSON"));
}

fn w_open(out: &mut String, tag: &str) {
    out.push('(');
    out.push_str(tag);
}

fn w_sp(out: &mut String) {
    out.push(' ');
}

fn w_close(out: &mut String) {
    out.push(')');
}

fn write_node(node: &IRNode, out: &mut String) {
    match node {
        IRNode::Hole { id, expected_type } => {
            w_open(out, "hole");
            w_sp(out);
            out.push_str(&id.to_string());
            w_sp(out);
            match expected_type {
                None => out.push_str("none"),
                Some(ty) => {
                    w_open(out, "some");
                    w_sp(out);
                    write_type(ty, out);
                    w_close(out);
                }
            }
            w_close(out);
        }
        IRNode::Block(items) => {
            w_open(out, "block");
            for item in items {
                w_sp(out);
                write_node(item, out);
            }
            w_close(out);
        }
        IRNode::Let { name, value, body } => {
            w_open(out, "let");
            w_sp(out);
            w_str(out, name);
            w_sp(out);
            write_node(value, out);
            w_sp(out);
            write_node(body, out);
            w_close(out);
        }
        IRNode::If { condition, then_branch, else_branch } => {
            w_open(out, "if");
            w_sp(out);
            write_node(condition, out);
            w_sp(out);
            write_node(then_branch, out);
            w_sp(out);
            write_node(else_branch, out);
            w_close(out);
        }
        IRNode::While { condition, body, invariant } => {
            w_open(out, "while");
            w_sp(out);
            write_node(condition, out);
            w_sp(out);
            write_node(body, out);
            w_sp(out);
            write_str_opt(out, invariant.as_deref());
            w_close(out);
        }
        IRNode::For { var, start, end, body } => {
            w_open(out, "for");
            w_sp(out);
            w_str(out, var);
            w_sp(out);
            write_node(start, out);
            w_sp(out);
            write_node(end, out);
            w_sp(out);
            write_node(body, out);
            w_close(out);
        }
        IRNode::Match { scrutinee, cases } => {
            w_open(out, "match");
            w_sp(out);
            write_node(scrutinee, out);
            for (pattern, body) in cases {
                w_sp(out);
                w_open(out, "case");
                w_sp(out);
                write_pattern(pattern, out);
                w_sp(out);
                write_node(body, out);
                w_close(out);
            }
            w_close(out);
        }
        IRNode::Call { function, args } => {
            w_open(out, "call");
            w_sp(out);
            w_str(out, function);
            for arg in args {
                w_sp(out);
                write_node(arg, out);
            }
            w_close(out);
        }
        IRNode::Return(inner) => {
            w_open(out, "return");
            w_sp(out);
            write_node(inner, out);
            w_close(out);
        }
        IRNode::Literal(lit) => {
            w_open(out, "lit");
            w_sp(out);
            write_literal(lit, out);
            w_close(out);
        }
        IRNode::Var(name) => {
            w_open(out, "var");
            w_sp(out);
            w_str(out, name);
            w_close(out);
        }
        IRNode::BinOp(op, l, r) => {
            w_open(out, "binop");
            w_sp(out);
            out.push_str(binop_name(*op));
            w_sp(out);
            write_node(l, out);
            w_sp(out);
            write_node(r, out);
            w_close(out);
        }
        IRNode::UnaryOp(op, inner) => {
            w_open(out, "unop");
            w_sp(out);
            out.push_str(unop_name(*op));
            w_sp(out);
            write_node(inner, out);
            w_close(out);
        }
        IRNode::Lambda { params, body } => {
            w_open(out, "lambda");
            w_sp(out);
            write_params(params, out);
            w_sp(out);
            write_node(body, out);
            w_close(out);
        }
        IRNode::Closure { params, captures, body } => {
            w_open(out, "closure");
            w_sp(out);
            write_params(params, out);
            w_sp(out);
            out.push('(');
            for (i, capture) in captures.iter().enumerate() {
                if i > 0 {
                    w_sp(out);
                }
                w_str(out, capture);
            }
            out.push(')');
            w_sp(out);
            write_node(body, out);
            w_close(out);
        }
        IRNode::Collection { kind, elements } => {
            w_open(out, "collection");
            w_sp(out);
            out.push_str(kind_name(*kind));
            for element in elements {
                w_sp(out);
                write_node(element, out);
            }
            w_close(out);
        }
        IRNode::Map { function, over } => {
            w_open(out, "map");
            w_sp(out);
            write_node(function, out);
            w_sp(out);
            write_node(over, out);
            w_close(out);
        }
        IRNode::Filter { predicate, over } => {
            w_open(out, "filter");
            w_sp(out);
            write_node(predicate, out);
            w_sp(out);
            write_node(over, out);
            w_close(out);
        }
        IRNode::Fold { operation, initial, over } => {
            w_open(out, "fold");
            w_sp(out);
            write_node(operation, out);
            w_sp(out);
            write_node(initial, out);
            w_sp(out);
            write_node(over, out);
            w_close(out);
        }
        IRNode::Sort { comparator, collection } => {
            w_open(out, "sort");
            w_sp(out);
            write_node(comparator, out);
            w_sp(out);
            write_node(collection, out);
            w_close(out);
        }
        IRNode::Alloc { name, ty, initial } => {
            w_open(out, "alloc");
            w_sp(out);
            w_str(out, name);
            w_sp(out);
            write_type(ty, out);
            w_sp(out);
            write_node(initial, out);
            w_close(out);
        }
        IRNode::Assign { target, value } => {
            w_open(out, "assign");
            w_sp(out);
            write_node(target, out);
            w_sp(out);
            write_node(value, out);
            w_close(out);
        }
        IRNode::Typed { node, ty } => {
            w_open(out, "typed");
            w_sp(out);
            write_node(node, out);
            w_sp(out);
            write_type(ty, out);
            w_close(out);
        }
        IRNode::Index(collection, index) => {
            w_open(out, "index");
            w_sp(out);
            write_node(collection, out);
            w_sp(out);
            write_node(index, out);
            w_close(out);
        }
        IRNode::Loop { body, invariant } => {
            w_open(out, "loop");
            w_sp(out);
            write_node(body, out);
            w_sp(out);
            write_str_opt(out, invariant.as_deref());
            w_close(out);
        }
        IRNode::Break { value } => {
            w_open(out, "break");
            w_sp(out);
            match value {
                None => out.push_str("none"),
                Some(inner) => {
                    w_open(out, "some");
                    w_sp(out);
                    write_node(inner, out);
                    w_close(out);
                }
            }
            w_close(out);
        }
        IRNode::StructDef { name, fields, body } => {
            w_open(out, "structdef");
            w_sp(out);
            w_str(out, name);
            w_sp(out);
            write_params(fields, out);
            w_sp(out);
            write_node(body, out);
            w_close(out);
        }
        IRNode::TryPropagate(inner) => {
            w_open(out, "try");
            w_sp(out);
            write_node(inner, out);
            w_close(out);
        }
        IRNode::FieldAccess { base, field } => {
            w_open(out, "field");
            w_sp(out);
            write_node(base, out);
            w_sp(out);
            w_str(out, field);
            w_close(out);
        }
        IRNode::RecordLit { type_name, fields } => {
            w_open(out, "record");
            w_sp(out);
            w_str(out, type_name);
            w_sp(out);
            out.push('(');
            for (i, (name, value)) in fields.iter().enumerate() {
                if i > 0 {
                    w_sp(out);
                }
                out.push('(');
                w_str(out, name);
                w_sp(out);
                write_node(value, out);
                out.push(')');
            }
            out.push(')');
            w_close(out);
        }
    }
}

fn write_str_opt(out: &mut String, value: Option<&str>) {
    match value {
        None => out.push_str("none"),
        Some(s) => {
            w_open(out, "some");
            w_sp(out);
            w_str(out, s);
            w_close(out);
        }
    }
}

fn write_params(params: &[(String, TypeRef)], out: &mut String) {
    out.push('(');
    for (i, (name, ty)) in params.iter().enumerate() {
        if i > 0 {
            w_sp(out);
        }
        out.push('(');
        w_str(out, name);
        w_sp(out);
        write_type(ty, out);
        out.push(')');
    }
    out.push(')');
}

fn write_literal(lit: &IRLiteral, out: &mut String) {
    match lit {
        IRLiteral::Int(n) => {
            w_open(out, "int");
            w_sp(out);
            out.push_str(&n.to_string());
            w_close(out);
        }
        IRLiteral::Float(x) => {
            w_open(out, "float");
            w_sp(out);
            out.push_str(&format!("{x:?}"));
            w_close(out);
        }
        IRLiteral::Bool(b) => {
            w_open(out, "bool");
            w_sp(out);
            out.push_str(if *b { "true" } else { "false" });
            w_close(out);
        }
        IRLiteral::String(s) => {
            w_open(out, "string");
            w_sp(out);
            w_str(out, s);
            w_close(out);
        }
        IRLiteral::Char(c) => {
            w_open(out, "char");
            w_sp(out);
            w_str(out, &c.to_string());
            w_close(out);
        }
        IRLiteral::Unit => out.push_str("unit"),
    }
}

fn write_pattern(pattern: &IRPattern, out: &mut String) {
    match pattern {
        IRPattern::Wildcard => out.push_str("wildcard"),
        IRPattern::Variable(name) => {
            w_open(out, "pvar");
            w_sp(out);
            w_str(out, name);
            w_close(out);
        }
        IRPattern::Literal(lit) => {
            w_open(out, "plit");
            w_sp(out);
            write_literal(lit, out);
            w_close(out);
        }
        IRPattern::Constructor(name, args) => {
            w_open(out, "ctor");
            w_sp(out);
            w_str(out, name);
            for arg in args {
                w_sp(out);
                write_pattern(arg, out);
            }
            w_close(out);
        }
        IRPattern::Guard(inner, guard) => {
            w_open(out, "guard");
            w_sp(out);
            write_pattern(inner, out);
            w_sp(out);
            w_str(out, guard);
            w_close(out);
        }
    }
}

fn write_type(ty: &TypeRef, out: &mut String) {
    match ty {
        TypeRef::Int => out.push_str("Int"),
        TypeRef::Float => out.push_str("Float"),
        TypeRef::Bool => out.push_str("Bool"),
        TypeRef::String => out.push_str("String"),
        TypeRef::Char => out.push_str("Char"),
        TypeRef::Unit => out.push_str("Unit"),
        TypeRef::I8 => out.push_str("I8"),
        TypeRef::I16 => out.push_str("I16"),
        TypeRef::I32 => out.push_str("I32"),
        TypeRef::I64 => out.push_str("I64"),
        TypeRef::U8 => out.push_str("U8"),
        TypeRef::U16 => out.push_str("U16"),
        TypeRef::U32 => out.push_str("U32"),
        TypeRef::U64 => out.push_str("U64"),
        TypeRef::Generic(name) => {
            w_open(out, "Generic");
            w_sp(out);
            w_str(out, name);
            w_close(out);
        }
        TypeRef::Named(name) => {
            w_open(out, "Named");
            w_sp(out);
            w_str(out, name);
            w_close(out);
        }
        TypeRef::List(inner) => write_type_unary(out, "List", inner),
        TypeRef::Set(inner) => write_type_unary(out, "Set", inner),
        TypeRef::Option(inner) => write_type_unary(out, "Option", inner),
        TypeRef::Ref(inner) => write_type_unary(out, "Ref", inner),
        TypeRef::Stream(inner) => write_type_unary(out, "Stream", inner),
        TypeRef::Map(k, v) => {
            w_open(out, "Map");
            w_sp(out);
            write_type(k, out);
            w_sp(out);
            write_type(v, out);
            w_close(out);
        }
        TypeRef::Result(o, e) => {
            w_open(out, "Result");
            w_sp(out);
            write_type(o, out);
            w_sp(out);
            write_type(e, out);
            w_close(out);
        }
        TypeRef::Tuple(items) => {
            w_open(out, "Tuple");
            for item in items {
                w_sp(out);
                write_type(item, out);
            }
            w_close(out);
        }
        TypeRef::Array(inner, size) => {
            w_open(out, "Array");
            w_sp(out);
            write_type(inner, out);
            w_sp(out);
            out.push_str(&size.to_string());
            w_close(out);
        }
        TypeRef::Function(params, ret) => {
            w_open(out, "Function");
            w_sp(out);
            out.push('(');
            for (i, param) in params.iter().enumerate() {
                if i > 0 {
                    w_sp(out);
                }
                write_type(param, out);
            }
            out.push(')');
            w_sp(out);
            write_type(ret, out);
            w_close(out);
        }
    }
}

fn write_type_unary(out: &mut String, tag: &str, inner: &TypeRef) {
    w_open(out, tag);
    w_sp(out);
    write_type(inner, out);
    w_close(out);
}

fn binop_name(op: IRBinOp) -> &'static str {
    match op {
        IRBinOp::Add => "Add",
        IRBinOp::Sub => "Sub",
        IRBinOp::Mul => "Mul",
        IRBinOp::Div => "Div",
        IRBinOp::Mod => "Mod",
        IRBinOp::And => "And",
        IRBinOp::Or => "Or",
        IRBinOp::Xor => "Xor",
        IRBinOp::Eq => "Eq",
        IRBinOp::Neq => "Neq",
        IRBinOp::Lt => "Lt",
        IRBinOp::Le => "Le",
        IRBinOp::Gt => "Gt",
        IRBinOp::Ge => "Ge",
        IRBinOp::Shl => "Shl",
        IRBinOp::Shr => "Shr",
        IRBinOp::Concat => "Concat",
        IRBinOp::Append => "Append",
        IRBinOp::Merge => "Merge",
    }
}

fn unop_name(op: IRUnaryOp) -> &'static str {
    match op {
        IRUnaryOp::Neg => "Neg",
        IRUnaryOp::Not => "Not",
        IRUnaryOp::Abs => "Abs",
        IRUnaryOp::Len => "Len",
        IRUnaryOp::Head => "Head",
        IRUnaryOp::Tail => "Tail",
        IRUnaryOp::Init => "Init",
        IRUnaryOp::Last => "Last",
        IRUnaryOp::Reverse => "Reverse",
        IRUnaryOp::Clone => "Clone",
    }
}

fn kind_name(kind: CollectionKind) -> &'static str {
    match kind {
        CollectionKind::List => "List",
        CollectionKind::Set => "Set",
        CollectionKind::Map => "Map",
        CollectionKind::Array => "Array",
        CollectionKind::Tuple => "Tuple",
    }
}

// ── The reader ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    LParen,
    RParen,
    Atom(String),
    Str(String),
}

fn tokenize(text: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            '"' => {
                // Scan the raw JSON string token (escape
                // aware), then let serde_json unquote it —
                // one string discipline, shared with the
                // writer.
                let start = i;
                i += 1;
                let mut closed = false;
                while i < chars.len() {
                    match chars[i] {
                        '\\' => i += 2,
                        '"' => {
                            i += 1;
                            closed = true;
                            break;
                        }
                        _ => i += 1,
                    }
                }
                if !closed {
                    return Err("ir text: unterminated string literal".to_string());
                }
                let raw: String = chars[start..i].iter().collect();
                let value: String = serde_json::from_str(&raw)
                    .map_err(|e| format!("ir text: malformed string literal: {e}"))?;
                toks.push(Tok::Str(value));
            }
            _ => {
                let start = i;
                while i < chars.len()
                    && !chars[i].is_whitespace()
                    && chars[i] != '('
                    && chars[i] != ')'
                    && chars[i] != '"'
                {
                    i += 1;
                }
                toks.push(Tok::Atom(chars[start..i].iter().collect()));
            }
        }
    }
    Ok(toks)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Result<Tok, String> {
        let tok = self
            .toks
            .get(self.pos)
            .cloned()
            .ok_or_else(|| "ir text: unexpected end of input".to_string())?;
        self.pos += 1;
        Ok(tok)
    }

    fn expect_lparen(&mut self) -> Result<(), String> {
        match self.next()? {
            Tok::LParen => Ok(()),
            other => Err(format!("ir text: expected '(', found {}", tok_desc(&other))),
        }
    }

    fn expect_rparen(&mut self) -> Result<(), String> {
        match self.next()? {
            Tok::RParen => Ok(()),
            other => Err(format!("ir text: expected ')', found {}", tok_desc(&other))),
        }
    }

    fn at_rparen(&self) -> bool {
        matches!(self.peek(), Some(Tok::RParen))
    }

    fn expect_atom(&mut self, want: &str) -> Result<(), String> {
        match self.next()? {
            Tok::Atom(a) if a == want => Ok(()),
            other => Err(format!(
                "ir text: expected '{want}', found {}",
                tok_desc(&other)
            )),
        }
    }

    fn next_atom(&mut self) -> Result<String, String> {
        match self.next()? {
            Tok::Atom(a) => Ok(a),
            other => Err(format!("ir text: expected an atom, found {}", tok_desc(&other))),
        }
    }

    fn next_string(&mut self) -> Result<String, String> {
        match self.next()? {
            Tok::Str(s) => Ok(s),
            other => Err(format!(
                "ir text: expected a quoted string, found {}",
                tok_desc(&other)
            )),
        }
    }

    fn next_i64(&mut self) -> Result<i64, String> {
        let a = self.next_atom()?;
        a.parse::<i64>()
            .map_err(|_| format!("ir text: expected an integer, found '{a}'"))
    }

    fn next_u64(&mut self) -> Result<u64, String> {
        let a = self.next_atom()?;
        a.parse::<u64>()
            .map_err(|_| format!("ir text: expected a non-negative integer, found '{a}'"))
    }

    fn next_usize(&mut self) -> Result<usize, String> {
        let a = self.next_atom()?;
        a.parse::<usize>()
            .map_err(|_| format!("ir text: expected a non-negative integer, found '{a}'"))
    }

    /// An `Option` in the grammar's uniform shape:
    /// `none` or `(some X)` — `inner` parses the X after
    /// the `some` atom and this consumes the close.
    fn parse_opt<T>(
        &mut self,
        mut inner: impl FnMut(&mut Parser) -> Result<T, String>,
    ) -> Result<Option<T>, String> {
        match self.peek().cloned() {
            Some(Tok::Atom(a)) if a == "none" => {
                self.pos += 1;
                Ok(None)
            }
            Some(Tok::LParen) => {
                self.expect_lparen()?;
                self.expect_atom("some")?;
                let value = inner(self)?;
                self.expect_rparen()?;
                Ok(Some(value))
            }
            other => Err(format!(
                "ir text: expected 'none' or '(some ...)', found {}",
                other.map(|t| tok_desc(&t)).unwrap_or_else(|| "end of input".into())
            )),
        }
    }

    fn parse_node(&mut self) -> Result<IRNode, String> {
        self.expect_lparen()?;
        let tag = self.next_atom()?;
        let node = match tag.as_str() {
            "hole" => {
                let id = self.next_u64()?;
                let expected_type = self.parse_opt(|p| p.parse_type())?;
                IRNode::Hole { id, expected_type }
            }
            "block" => {
                let mut items = Vec::new();
                while !self.at_rparen() {
                    items.push(self.parse_node()?);
                }
                IRNode::Block(items)
            }
            "let" => {
                let name = self.next_string()?;
                let value = self.parse_node()?;
                let body = self.parse_node()?;
                IRNode::Let { name, value: Box::new(value), body: Box::new(body) }
            }
            "if" => {
                let condition = self.parse_node()?;
                let then_branch = self.parse_node()?;
                let else_branch = self.parse_node()?;
                IRNode::If {
                    condition: Box::new(condition),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(else_branch),
                }
            }
            "while" => {
                let condition = self.parse_node()?;
                let body = self.parse_node()?;
                let invariant = self.parse_opt(|p| p.next_string())?;
                IRNode::While {
                    condition: Box::new(condition),
                    body: Box::new(body),
                    invariant,
                }
            }
            "for" => {
                let var = self.next_string()?;
                let start = self.parse_node()?;
                let end = self.parse_node()?;
                let body = self.parse_node()?;
                IRNode::For { var, start: Box::new(start), end: Box::new(end), body: Box::new(body) }
            }
            "match" => {
                let scrutinee = self.parse_node()?;
                let mut cases = Vec::new();
                while !self.at_rparen() {
                    self.expect_lparen()?;
                    self.expect_atom("case")?;
                    let pattern = self.parse_pattern()?;
                    let body = self.parse_node()?;
                    self.expect_rparen()?;
                    cases.push((pattern, body));
                }
                IRNode::Match { scrutinee: Box::new(scrutinee), cases }
            }
            "call" => {
                let function = self.next_string()?;
                let mut args = Vec::new();
                while !self.at_rparen() {
                    args.push(self.parse_node()?);
                }
                IRNode::Call { function, args }
            }
            "return" => IRNode::Return(Box::new(self.parse_node()?)),
            "lit" => IRNode::Literal(self.parse_literal()?),
            "var" => IRNode::Var(self.next_string()?),
            "binop" => {
                let op = self.parse_binop()?;
                let l = self.parse_node()?;
                let r = self.parse_node()?;
                IRNode::BinOp(op, Box::new(l), Box::new(r))
            }
            "unop" => {
                let op = self.parse_unop()?;
                let inner = self.parse_node()?;
                IRNode::UnaryOp(op, Box::new(inner))
            }
            "lambda" => {
                let params = self.parse_params()?;
                let body = self.parse_node()?;
                IRNode::Lambda { params, body: Box::new(body) }
            }
            "closure" => {
                let params = self.parse_params()?;
                self.expect_lparen()?;
                let mut captures = Vec::new();
                while !self.at_rparen() {
                    captures.push(self.next_string()?);
                }
                self.expect_rparen()?;
                let body = self.parse_node()?;
                IRNode::Closure { params, captures, body: Box::new(body) }
            }
            "collection" => {
                let kind = self.parse_kind()?;
                let mut elements = Vec::new();
                while !self.at_rparen() {
                    elements.push(self.parse_node()?);
                }
                IRNode::Collection { kind, elements }
            }
            "map" => {
                let function = self.parse_node()?;
                let over = self.parse_node()?;
                IRNode::Map { function: Box::new(function), over: Box::new(over) }
            }
            "filter" => {
                let predicate = self.parse_node()?;
                let over = self.parse_node()?;
                IRNode::Filter { predicate: Box::new(predicate), over: Box::new(over) }
            }
            "fold" => {
                let operation = self.parse_node()?;
                let initial = self.parse_node()?;
                let over = self.parse_node()?;
                IRNode::Fold {
                    operation: Box::new(operation),
                    initial: Box::new(initial),
                    over: Box::new(over),
                }
            }
            "sort" => {
                let comparator = self.parse_node()?;
                let collection = self.parse_node()?;
                IRNode::Sort { comparator: Box::new(comparator), collection: Box::new(collection) }
            }
            "alloc" => {
                let name = self.next_string()?;
                let ty = self.parse_type()?;
                let initial = self.parse_node()?;
                IRNode::Alloc { name, ty, initial: Box::new(initial) }
            }
            "assign" => {
                let target = self.parse_node()?;
                let value = self.parse_node()?;
                IRNode::Assign { target: Box::new(target), value: Box::new(value) }
            }
            "typed" => {
                let node = self.parse_node()?;
                let ty = self.parse_type()?;
                IRNode::Typed { node: Box::new(node), ty }
            }
            "index" => {
                let collection = self.parse_node()?;
                let index = self.parse_node()?;
                IRNode::Index(Box::new(collection), Box::new(index))
            }
            "loop" => {
                let body = self.parse_node()?;
                let invariant = self.parse_opt(|p| p.next_string())?;
                IRNode::Loop { body: Box::new(body), invariant }
            }
            "break" => {
                let value = self.parse_opt(|p| p.parse_node())?;
                IRNode::Break { value: value.map(Box::new) }
            }
            "structdef" => {
                let name = self.next_string()?;
                let fields = self.parse_params()?;
                let body = self.parse_node()?;
                IRNode::StructDef { name, fields, body: Box::new(body) }
            }
            "try" => IRNode::TryPropagate(Box::new(self.parse_node()?)),
            "field" => {
                let base = self.parse_node()?;
                let field = self.next_string()?;
                IRNode::FieldAccess { base: Box::new(base), field }
            }
            "record" => {
                let type_name = self.next_string()?;
                self.expect_lparen()?;
                let mut fields = Vec::new();
                while !self.at_rparen() {
                    self.expect_lparen()?;
                    let name = self.next_string()?;
                    let value = self.parse_node()?;
                    self.expect_rparen()?;
                    fields.push((name, value));
                }
                self.expect_rparen()?;
                IRNode::RecordLit { type_name, fields }
            }
            other => {
                return Err(format!("ir text: unknown node tag '{other}'"));
            }
        };
        self.expect_rparen()?;
        Ok(node)
    }

    fn parse_params(&mut self) -> Result<Vec<(String, TypeRef)>, String> {
        self.expect_lparen()?;
        let mut params = Vec::new();
        while !self.at_rparen() {
            self.expect_lparen()?;
            let name = self.next_string()?;
            let ty = self.parse_type()?;
            self.expect_rparen()?;
            params.push((name, ty));
        }
        self.expect_rparen()?;
        Ok(params)
    }

    fn parse_literal(&mut self) -> Result<IRLiteral, String> {
        match self.peek().cloned() {
            Some(Tok::Atom(a)) if a == "unit" => {
                self.pos += 1;
                Ok(IRLiteral::Unit)
            }
            Some(Tok::LParen) => {
                self.expect_lparen()?;
                let tag = self.next_atom()?;
                let lit = match tag.as_str() {
                    "int" => IRLiteral::Int(self.next_i64()?),
                    "float" => {
                        let a = self.next_atom()?;
                        let x: f64 = a
                            .parse()
                            .map_err(|_| format!("ir text: expected a float, found '{a}'"))?;
                        IRLiteral::Float(x)
                    }
                    "bool" => match self.next_atom()?.as_str() {
                        "true" => IRLiteral::Bool(true),
                        "false" => IRLiteral::Bool(false),
                        other => {
                            return Err(format!(
                                "ir text: expected 'true' or 'false', found '{other}'"
                            ))
                        }
                    },
                    "string" => IRLiteral::String(self.next_string()?),
                    "char" => {
                        let s = self.next_string()?;
                        let mut chars = s.chars();
                        match (chars.next(), chars.next()) {
                            (Some(c), None) => IRLiteral::Char(c),
                            _ => {
                                return Err(format!(
                                    "ir text: a char literal is exactly one code point, found '{s}'"
                                ))
                            }
                        }
                    }
                    other => {
                        return Err(format!("ir text: unknown literal tag '{other}'"));
                    }
                };
                self.expect_rparen()?;
                Ok(lit)
            }
            other => Err(format!(
                "ir text: expected a literal, found {}",
                other.map(|t| tok_desc(&t)).unwrap_or_else(|| "end of input".into())
            )),
        }
    }

    fn parse_pattern(&mut self) -> Result<IRPattern, String> {
        match self.peek().cloned() {
            Some(Tok::Atom(a)) if a == "wildcard" => {
                self.pos += 1;
                Ok(IRPattern::Wildcard)
            }
            Some(Tok::LParen) => {
                self.expect_lparen()?;
                let tag = self.next_atom()?;
                let pattern = match tag.as_str() {
                    "pvar" => IRPattern::Variable(self.next_string()?),
                    "plit" => IRPattern::Literal(self.parse_literal()?),
                    "ctor" => {
                        let name = self.next_string()?;
                        let mut args = Vec::new();
                        while !self.at_rparen() {
                            args.push(self.parse_pattern()?);
                        }
                        IRPattern::Constructor(name, args)
                    }
                    "guard" => {
                        let inner = self.parse_pattern()?;
                        let guard = self.next_string()?;
                        IRPattern::Guard(Box::new(inner), guard)
                    }
                    other => {
                        return Err(format!("ir text: unknown pattern tag '{other}'"));
                    }
                };
                self.expect_rparen()?;
                Ok(pattern)
            }
            other => Err(format!(
                "ir text: expected a pattern, found {}",
                other.map(|t| tok_desc(&t)).unwrap_or_else(|| "end of input".into())
            )),
        }
    }

    fn parse_type(&mut self) -> Result<TypeRef, String> {
        match self.peek().cloned() {
            Some(Tok::Atom(a)) => {
                let ty = match a.as_str() {
                    "Int" => TypeRef::Int,
                    "Float" => TypeRef::Float,
                    "Bool" => TypeRef::Bool,
                    "String" => TypeRef::String,
                    "Char" => TypeRef::Char,
                    "Unit" => TypeRef::Unit,
                    "I8" => TypeRef::I8,
                    "I16" => TypeRef::I16,
                    "I32" => TypeRef::I32,
                    "I64" => TypeRef::I64,
                    "U8" => TypeRef::U8,
                    "U16" => TypeRef::U16,
                    "U32" => TypeRef::U32,
                    "U64" => TypeRef::U64,
                    other => {
                        return Err(format!("ir text: unknown type '{other}'"));
                    }
                };
                self.pos += 1;
                Ok(ty)
            }
            Some(Tok::LParen) => {
                self.expect_lparen()?;
                let tag = self.next_atom()?;
                let ty = match tag.as_str() {
                    "Generic" => TypeRef::Generic(self.next_string()?),
                    "Named" => TypeRef::Named(self.next_string()?),
                    "List" => TypeRef::List(Box::new(self.parse_type()?)),
                    "Set" => TypeRef::Set(Box::new(self.parse_type()?)),
                    "Option" => TypeRef::Option(Box::new(self.parse_type()?)),
                    "Ref" => TypeRef::Ref(Box::new(self.parse_type()?)),
                    "Stream" => TypeRef::Stream(Box::new(self.parse_type()?)),
                    "Map" => {
                        let k = self.parse_type()?;
                        let v = self.parse_type()?;
                        TypeRef::Map(Box::new(k), Box::new(v))
                    }
                    "Result" => {
                        let o = self.parse_type()?;
                        let e = self.parse_type()?;
                        TypeRef::Result(Box::new(o), Box::new(e))
                    }
                    "Tuple" => {
                        let mut items = Vec::new();
                        while !self.at_rparen() {
                            items.push(self.parse_type()?);
                        }
                        TypeRef::Tuple(items)
                    }
                    "Array" => {
                        let inner = self.parse_type()?;
                        let size = self.next_usize()?;
                        TypeRef::Array(Box::new(inner), size)
                    }
                    "Function" => {
                        self.expect_lparen()?;
                        let mut params = Vec::new();
                        while !self.at_rparen() {
                            params.push(self.parse_type()?);
                        }
                        self.expect_rparen()?;
                        let ret = self.parse_type()?;
                        TypeRef::Function(params, Box::new(ret))
                    }
                    other => {
                        return Err(format!("ir text: unknown type tag '{other}'"));
                    }
                };
                self.expect_rparen()?;
                Ok(ty)
            }
            other => Err(format!(
                "ir text: expected a type, found {}",
                other.map(|t| tok_desc(&t)).unwrap_or_else(|| "end of input".into())
            )),
        }
    }

    fn parse_binop(&mut self) -> Result<IRBinOp, String> {
        let a = self.next_atom()?;
        match a.as_str() {
            "Add" => Ok(IRBinOp::Add),
            "Sub" => Ok(IRBinOp::Sub),
            "Mul" => Ok(IRBinOp::Mul),
            "Div" => Ok(IRBinOp::Div),
            "Mod" => Ok(IRBinOp::Mod),
            "And" => Ok(IRBinOp::And),
            "Or" => Ok(IRBinOp::Or),
            "Xor" => Ok(IRBinOp::Xor),
            "Eq" => Ok(IRBinOp::Eq),
            "Neq" => Ok(IRBinOp::Neq),
            "Lt" => Ok(IRBinOp::Lt),
            "Le" => Ok(IRBinOp::Le),
            "Gt" => Ok(IRBinOp::Gt),
            "Ge" => Ok(IRBinOp::Ge),
            "Shl" => Ok(IRBinOp::Shl),
            "Shr" => Ok(IRBinOp::Shr),
            "Concat" => Ok(IRBinOp::Concat),
            "Append" => Ok(IRBinOp::Append),
            "Merge" => Ok(IRBinOp::Merge),
            _ => Err(format!("ir text: unknown binary operator '{a}'")),
        }
    }

    fn parse_unop(&mut self) -> Result<IRUnaryOp, String> {
        let a = self.next_atom()?;
        match a.as_str() {
            "Neg" => Ok(IRUnaryOp::Neg),
            "Not" => Ok(IRUnaryOp::Not),
            "Abs" => Ok(IRUnaryOp::Abs),
            "Len" => Ok(IRUnaryOp::Len),
            "Head" => Ok(IRUnaryOp::Head),
            "Tail" => Ok(IRUnaryOp::Tail),
            "Init" => Ok(IRUnaryOp::Init),
            "Last" => Ok(IRUnaryOp::Last),
            "Reverse" => Ok(IRUnaryOp::Reverse),
            "Clone" => Ok(IRUnaryOp::Clone),
            _ => Err(format!("ir text: unknown unary operator '{a}'")),
        }
    }

    fn parse_kind(&mut self) -> Result<CollectionKind, String> {
        let a = self.next_atom()?;
        match a.as_str() {
            "List" => Ok(CollectionKind::List),
            "Set" => Ok(CollectionKind::Set),
            "Map" => Ok(CollectionKind::Map),
            "Array" => Ok(CollectionKind::Array),
            "Tuple" => Ok(CollectionKind::Tuple),
            _ => Err(format!("ir text: unknown collection kind '{a}'")),
        }
    }
}

fn tok_desc(tok: &Tok) -> String {
    match tok {
        Tok::LParen => "'('".to_string(),
        Tok::RParen => "')'".to_string(),
        Tok::Atom(a) => format!("'{a}'"),
        Tok::Str(s) => format!("string \"{s}\""),
    }
}

/// Parse IR text back to the body the writer held. The
/// first line must be the version marker: a text naming
/// another `morphic-ir/*` version is refused by name, and
/// a text with no marker is refused as a version problem.
/// Every other failure is likewise a named refusal — this
/// reader never panics and never guesses.
pub fn parse_ir(text: &str) -> Result<IRNode, String> {
    let (first_line, rest) = match text.split_once('\n') {
        Some((head, tail)) => (head.trim_end_matches('\r'), tail),
        None => (text.trim_end_matches('\r'), ""),
    };
    if first_line != IR_TEXT_VERSION {
        if first_line.starts_with("morphic-ir/") {
            return Err(format!(
                "ir text: unsupported IR text version '{first_line}' — \
                 this build reads {IR_TEXT_VERSION}"
            ));
        }
        return Err(format!(
            "ir text: missing IR text version marker — the text must begin \
             with '{IR_TEXT_VERSION}' on its own line"
        ));
    }
    let toks = tokenize(rest)?;
    let mut parser = Parser { toks, pos: 0 };
    let node = parser.parse_node()?;
    if parser.pos != parser.toks.len() {
        return Err("ir text: trailing content after the root node".to_string());
    }
    Ok(node)
}
