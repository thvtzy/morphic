// ┌────────────────────────────────────────────────────────────┐
// │  M3: Codegen — Proof Tests (slices 1–2: leaf mappings +   │
// │  small template families)                                  │
// │                                                            │
// │  Two layers, mirroring tests/m2_proof.rs:                  │
// │                                                            │
// │  A. Pure-Rust mirror implementations of every spec in      │
// │     examples/self_host/codegen.morph, with golden tests    │
// │     covering the FULL mapping range of each table plus     │
// │     off-range inputs. The specs' constraints carry the     │
// │     complete mapping; these mirrors are the executable     │
// │     cross-check (the M2 lesson: never trust a verdict      │
// │     without cross-validation).                             │
// │                                                            │
// │     The real emitter functions are private to              │
// │     src/codegen/mod.rs, so — as in M2 — each mirror is a   │
// │     faithful hand copy of the table it mirrors, with the   │
// │     source location cited. Enum codes follow declaration   │
// │     order in the Rust source, as in the .morph file.       │
// │                                                            │
// │  B. Real pipeline tests: the compiled `morphic` binary     │
// │     runs `check` (parse + typecheck) on codegen.morph.     │
// │     A negative control proves the pipeline actually        │
// │     rejects invalid specs, so a pass is meaningful.        │
// └────────────────────────────────────────────────────────────┘

use std::io::Write;
use std::process::Command;

// ═════════════════════════════════════════════════════════════
// A. MIRRORS — slice 1 leaf mappings of the Rust emitter
// ═════════════════════════════════════════════════════════════

/// Mirror of the scalar leaf table of `ty_to_rust`
/// (src/codegen/mod.rs:1093). Composite and named types are
/// recursive/name-carrying in the original and unpinned in the
/// spec; mirrored here as None.
fn rust_type_name(tag: i64) -> Option<&'static str> {
    match tag {
        0 => Some("i64"),     // Int
        1 => Some("f64"),     // Float
        2 => Some("bool"),    // Bool
        3 => Some("String"),  // String
        4 => Some("char"),    // Char
        5 => Some("()"),      // Unit
        6 => Some("i8"),      // I8
        7 => Some("i16"),     // I16
        8 => Some("i32"),     // I32
        9 => Some("i64"),     // I64 — same name as Int, as in the original
        10 => Some("u8"),     // U8
        11 => Some("u16"),    // U16
        12 => Some("u32"),    // U32
        13 => Some("u64"),    // U64
        _ => None,
    }
}

/// Mirror of the operator table inside `ir_to_rust_expr`
/// (src/codegen/mod.rs:615). Append (17) and Merge (18) are
/// bound to ".push"/".extend" in the original table but those
/// values are dead — the arms are special-cased into method
/// calls before the symbol is used — so they have no infix
/// symbol: None here, exactly as the spec leaves them unpinned.
fn rust_binop_symbol(op: i64) -> Option<&'static str> {
    match op {
        0 => Some("+"),   // Add
        1 => Some("-"),   // Sub
        2 => Some("*"),   // Mul
        3 => Some("/"),   // Div
        4 => Some("%"),   // Mod
        5 => Some("&&"),  // And
        6 => Some("||"),  // Or
        7 => Some("^"),   // Xor
        8 => Some("=="),  // Eq
        9 => Some("!="),  // Neq
        10 => Some("<"),  // Lt
        11 => Some("<="), // Le
        12 => Some(">"),  // Gt
        13 => Some(">="), // Ge
        14 => Some("<<"), // Shl
        15 => Some(">>"), // Shr
        16 => Some("+"),  // Concat — shares Add's symbol
        _ => None,
    }
}

/// Mirror of the prefix arms of the UnaryOp match in
/// `ir_to_rust_expr` (src/codegen/mod.rs:636): only Neg and
/// Not render with a prefix symbol (parenthesized).
fn rust_unaryop_symbol(op: i64) -> Option<&'static str> {
    match op {
        0 => Some("-"), // Neg
        1 => Some("!"), // Not
        _ => None,
    }
}

/// Mirror of the method-call arms of the UnaryOp match in
/// `ir_to_rust_expr` (src/codegen/mod.rs:636).
fn rust_unaryop_method(op: i64) -> Option<&'static str> {
    match op {
        2 => Some("abs"),     // Abs
        3 => Some("len"),     // Len
        8 => Some("reverse"), // Reverse
        9 => Some("clone"),   // Clone
        _ => None,
    }
}

/// Mirror of the emission-family classification of the UnaryOp
/// match in `ir_to_rust_expr` (src/codegen/mod.rs:636):
/// 0 = prefix symbol, 1 = method call, 2 = list-part
/// index/slice form. Total over the ten operators.
fn rust_unaryop_form(op: i64) -> Option<i64> {
    match op {
        0 | 1 => Some(0),         // Neg, Not
        2 | 3 | 8 | 9 => Some(1), // Abs, Len, Reverse, Clone
        4..=7 => Some(2),         // Head, Tail, Init, Last
        _ => None,
    }
}

/// Mirror of the Collection match in `ir_to_rust_expr`
/// (src/codegen/mod.rs:676), EXPRESSION position forms:
/// 0 = vec![..], 1 = HashSet::from([..]), 2 = HashMap::from([..]),
/// 3 = array literal, 4 = tuple literal. (Statement position in
/// the original collapses List/Set/Map to the vec! form; that
/// position rule belongs to the statement-walker slice.)
fn rust_collection_form(kind: i64) -> Option<i64> {
    match kind {
        0 => Some(0), // List
        1 => Some(1), // Set
        2 => Some(2), // Map
        3 => Some(3), // Array
        4 => Some(4), // Tuple
        _ => None,
    }
}

/// Mirror of `is_rust_keyword` (src/codegen/mod.rs:53): the
/// emitter's exact 51-word set, copied verbatim.
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

/// Mirror of the operator table of `expr_to_rust`
/// (src/codegen/mod.rs:1188) — the spec-expression emitter used
/// for generated test assertions. A separate, weaker table than
/// `rust_binop_symbol`: the operators it does not map fall
/// through to "?" in the original, pinned here as rows.
fn spec_binop_symbol(op: i64) -> Option<&'static str> {
    match op {
        0 => Some("+"),  // Add
        1 => Some("-"),  // Sub
        2 => Some("*"),  // Mul
        3 => Some("/"),  // Div
        4 => Some("?"),  // Mod — unmapped in the original
        5 => Some("&&"), // And
        6 => Some("||"), // Or
        7 => Some("=="), // Eq
        8 => Some("!="), // Neq
        9 => Some("?"),  // Lt — unmapped in the original
        10 => Some("?"), // Le — unmapped
        11 => Some("?"), // Gt — unmapped
        12 => Some("?"), // Ge — unmapped
        13 => Some("?"), // Concat — unmapped
        _ => None,
    }
}

// ═════════════════════════════════════════════════════════════
// A2. MIRRORS — slice 2 small template families
//
// The walkers render children first, so each composite form is
// a template over already-rendered strings. The .morph specs
// pin the classifications and every fixed affix; the assembly
// mirrors below compose those pinned pieces exactly as the
// original functions do (and are cited per function). The one
// step no spec pins is concatenation itself — the spec
// language has no concat operator — plus the numeric text
// conversions (Int -> decimal, f64 Display), which the mirrors
// perform with Rust's own to_string, as the original does.
// ═════════════════════════════════════════════════════════════

/// Mirror of the template-family classification of `ty_to_rust`
/// (src/codegen/mod.rs:1093): 0 = scalar leaf, 1 = name,
/// 2 = wrap-one, 3 = wrap-two, 4 = tuple join, 5 = array,
/// 6 = function. Total over the 26 TypeRef codes.
fn rust_type_form(tag: i64) -> Option<i64> {
    match tag {
        0..=13 => Some(0),        // scalar leaves
        14 | 15 => Some(1),       // Generic, Named
        16 | 17 | 19 | 24 | 25 => Some(2), // List, Set, Option, Ref, Stream
        18 | 20 => Some(3),       // Map, Result
        21 => Some(4),            // Tuple
        22 => Some(5),            // Array
        23 => Some(6),            // Function
        _ => None,
    }
}

/// Mirror of the fixed opening texts of the composite arms of
/// `ty_to_rust` (src/codegen/mod.rs:1107-1122). Set is
/// UNQUALIFIED "HashSet<" in the original — pinned as-is.
fn rust_type_prefix(tag: i64) -> Option<&'static str> {
    match tag {
        16 => Some("Vec<"),
        17 => Some("HashSet<"), // unqualified in the original
        18 => Some("HashMap<"),
        19 => Some("Option<"),
        20 => Some("Result<"),
        21 => Some("("),
        22 => Some("["),
        23 => Some("fn("),
        24 => Some("&"),
        25 => Some("impl Iterator<Item = "),
        _ => None,
    }
}

/// Mirror of the fixed closing texts of the composite arms of
/// `ty_to_rust` (src/codegen/mod.rs:1107-1122). Function and
/// Ref close with the empty string.
fn rust_type_suffix(tag: i64) -> Option<&'static str> {
    match tag {
        16 | 17 | 18 | 19 | 20 | 25 => Some(">"),
        21 => Some(")"),
        22 => Some("]"),
        23 | 24 => Some(""),
        _ => None,
    }
}

/// Mirror of the fixed between-children texts of `ty_to_rust`
/// (src/codegen/mod.rs:1107-1122).
fn rust_type_separator(tag: i64) -> Option<&'static str> {
    match tag {
        18 | 20 | 21 | 23 => Some(", "),
        22 => Some("; "),
        _ => None,
    }
}

/// Mirror of the one midfix in `ty_to_rust`: the text between
/// a function type's joined arguments and its return type
/// (src/codegen/mod.rs:1119-1122).
fn rust_type_midfix(tag: i64) -> Option<&'static str> {
    match tag {
        23 => Some(") -> "),
        _ => None,
    }
}

/// Mirror of the rendering-family classification of the
/// Literal arm of `ir_to_rust_expr` (src/codegen/mod.rs:605):
/// 0 = decimal text, 1 = f64 Display text, 2 = Bool value text,
/// 3 = quoted-raw template, 4 = quoted template, 5 = fixed
/// text. Total over the six IRLiteral kinds.
fn rust_literal_form(kind: i64) -> Option<i64> {
    match kind {
        0 => Some(0), // Int
        1 => Some(1), // Float
        2 => Some(2), // Bool
        3 => Some(3), // String
        4 => Some(4), // Char
        5 => Some(5), // Unit
        _ => None,
    }
}

/// Mirror of the Bool arm shared by `ir_to_rust_expr`
/// (src/codegen/mod.rs:607) and `ir_pattern_to_rust`
/// (src/codegen/mod.rs:818): `b.to_string()`. Total — the Bool
/// domain is finite.
fn rust_literal_bool(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

/// Mirror of the one value-independent literal text: the Unit
/// arm of `ir_to_rust_expr` (src/codegen/mod.rs:611).
fn rust_literal_fixed_text(kind: i64) -> Option<&'static str> {
    match kind {
        5 => Some("()"), // Unit
        _ => None,
    }
}

/// Mirror of the rendering-family classification of
/// `ir_pattern_to_rust` (src/codegen/mod.rs:813): 0 = fixed
/// text, 1 = name, 2 = literal, 3 = constructor template,
/// 4 = guard template. Total over the five IRPattern kinds.
fn rust_pattern_form(kind: i64) -> Option<i64> {
    match kind {
        0 => Some(0), // Wildcard
        1 => Some(1), // Variable
        2 => Some(2), // Literal
        3 => Some(3), // Constructor
        4 => Some(4), // Guard
        _ => None,
    }
}

/// Mirror of the one value-independent pattern text: the
/// Wildcard arm of `ir_pattern_to_rust` (src/codegen/mod.rs:815).
fn rust_pattern_fixed_text(kind: i64) -> Option<&'static str> {
    match kind {
        0 => Some("_"), // Wildcard
        _ => None,
    }
}

/// Mirror of the family classification of the Literal arm of
/// `ir_pattern_to_rust` (src/codegen/mod.rs:817): 0 = value
/// text (Int, Bool), 1 = wildcard fallback (Float, Char,
/// Unit), 2 = quoted-raw template (String). Total over the
/// six IRLiteral kinds.
fn rust_pattern_literal_form(kind: i64) -> Option<i64> {
    match kind {
        0 | 2 => Some(0), // Int, Bool — value text
        1 | 4 | 5 => Some(1), // Float, Char, Unit — wildcard
        3 => Some(2),     // String — quoted template
        _ => None,
    }
}

/// Mirror of the fallback rows of the Literal arm of
/// `ir_pattern_to_rust` (src/codegen/mod.rs:821): Float, Char
/// and Unit literal patterns render "_".
fn rust_pattern_literal_text(kind: i64) -> Option<&'static str> {
    match kind {
        1 | 4 | 5 => Some("_"),
        _ => None,
    }
}

/// Mirror of the empty-sub-patterns branch of the Constructor
/// arm of `ir_pattern_to_rust` (src/codegen/mod.rs:823-827):
/// with no sub-patterns the output is the bare name. The
/// with-sub-patterns branch is template assembly, unpinned in
/// the spec: None here.
fn rust_pattern_ctor_name(name: &str, has_subs: bool) -> Option<String> {
    if has_subs {
        None
    } else {
        Some(name.to_string())
    }
}

/// Mirror of the rendering-family classification of
/// `expr_to_rust` (src/codegen/mod.rs:1177): 0 = name,
/// 1 = decimal text, 2 = Bool value text, 3 = quoted-raw
/// template, 4 = call template, 5 = binary template,
/// 6 = "??" fallback. Total over the thirteen Expr kinds.
/// FloatLit (2) and UnaryOp (9) have no arm in the original
/// and land in family 6.
fn rust_expr_form(kind: i64) -> Option<i64> {
    match kind {
        0 => Some(0),       // Var
        1 => Some(1),       // IntLit
        3 => Some(2),       // BoolLit
        4 => Some(3),       // StringLit
        7 => Some(4),       // Call
        8 => Some(5),       // BinOp
        2 | 5 | 6 | 9 | 10 | 11 | 12 => Some(6), // FloatLit, Field, Index, UnaryOp, Lambda, Length, Comprehension
        _ => None,
    }
}

/// Mirror of the catch-all of `expr_to_rust`
/// (src/codegen/mod.rs:1199): every kind without an arm
/// renders "??".
fn rust_expr_fixed_text(kind: i64) -> Option<&'static str> {
    match kind {
        2 | 5 | 6 | 9 | 10 | 11 | 12 => Some("??"),
        _ => None,
    }
}

/// Mirror of the rendering-family classification of
/// `constraint_to_rust` (src/codegen/mod.rs:1162): 0 = fixed
/// text "true" (True + the catch-all kinds), 1 = negation
/// template, 2 = equality template, 3 = order, 4 = conjunction
/// template. Total over the eleven Constraint kinds.
fn rust_constraint_form(kind: i64) -> Option<i64> {
    match kind {
        0 | 1 | 2 | 3 | 4 | 6 | 10 => Some(0), // True, Expr, Forall, Exists, Implies, Or, Predicate
        5 => Some(4),                          // And
        7 => Some(1),                          // Not
        8 => Some(2),                          // Eq
        9 => Some(3),                          // Order
        _ => None,
    }
}

/// Mirror of the True arm and catch-all of
/// `constraint_to_rust` (src/codegen/mod.rs:1163,1173): all
/// render "true".
fn rust_constraint_fixed_text(kind: i64) -> Option<&'static str> {
    match kind {
        0 | 1 | 2 | 3 | 4 | 6 | 10 => Some("true"),
        _ => None,
    }
}

/// Mirror of the operator classification of the Order arm of
/// `constraint_to_rust` (src/codegen/mod.rs:1166-1167):
/// 0 = " < " template, 1 = " <= " template, 2 = "true"
/// fallback (GreaterThan, GreaterThanOrEqual have no arm).
/// Total over the four OrderOp codes.
fn rust_order_form(op: i64) -> Option<i64> {
    match op {
        0 => Some(0), // LessThan
        1 => Some(1), // LessThanOrEqual
        2 | 3 => Some(2), // GreaterThan, GreaterThanOrEqual
        _ => None,
    }
}

/// Mirror of the separators of the Order arm of
/// `constraint_to_rust` (src/codegen/mod.rs:1166-1167),
/// spaces included.
fn rust_order_separator(op: i64) -> Option<&'static str> {
    match op {
        0 => Some(" < "),
        1 => Some(" <= "),
        _ => None,
    }
}

/// Mirror of the wrap classification of
/// `expr_in_condition_position` (src/codegen/mod.rs:587):
/// block-like kinds render parenthesized in condition /
/// scrutinee position. Total over the 28 IRNode kinds
/// (declaration order in src/synthesis/engine.rs).
fn rust_condition_wrap(kind: i64) -> Option<bool> {
    match kind {
        1 | 2 | 3 | 4 | 5 | 6 | 13 | 14 | 24 | 26 => Some(true),
        0 | 7 | 8 | 9 | 10 | 11 | 12 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 | 25 | 27 => {
            Some(false)
        }
        _ => None,
    }
}

// ── Assembly mirrors: the recursive walkers, composed from ──
// ── the pinned pieces exactly as the originals compose them ──

/// Tree form of TypeRef for the assembled `ty_to_rust` mirror.
/// Names arrive already rendered (the original applies
/// rust_ident, which is slice 3).
#[allow(dead_code)]
enum Ty {
    Leaf(i64),
    Name(&'static str),
    Wrap1(i64, Box<Ty>),
    Wrap2(i64, Box<Ty>, Box<Ty>),
    Tuple(Vec<Ty>),
    Array(Box<Ty>, usize),
    Func(Vec<Ty>, Box<Ty>),
}

/// Faithful assembly of `ty_to_rust` (src/codegen/mod.rs:1093)
/// from the slice-1 name table and the slice-2 affix mirrors.
/// The array size renders via usize's Display — the decimal
/// conversion no spec pins (see the slice-2 notes).
fn ty_to_rust_assembled(ty: &Ty) -> String {
    match ty {
        Ty::Leaf(tag) => rust_type_name(*tag).unwrap().to_string(),
        Ty::Name(name) => name.to_string(),
        Ty::Wrap1(tag, inner) => format!(
            "{}{}{}",
            rust_type_prefix(*tag).unwrap(),
            ty_to_rust_assembled(inner),
            rust_type_suffix(*tag).unwrap()
        ),
        Ty::Wrap2(tag, first, second) => format!(
            "{}{}{}{}{}",
            rust_type_prefix(*tag).unwrap(),
            ty_to_rust_assembled(first),
            rust_type_separator(*tag).unwrap(),
            ty_to_rust_assembled(second),
            rust_type_suffix(*tag).unwrap()
        ),
        Ty::Tuple(parts) => {
            let rendered: Vec<String> = parts.iter().map(ty_to_rust_assembled).collect();
            format!(
                "{}{}{}",
                rust_type_prefix(21).unwrap(),
                rendered.join(rust_type_separator(21).unwrap()),
                rust_type_suffix(21).unwrap()
            )
        }
        Ty::Array(inner, size) => format!(
            "{}{}{}{}{}",
            rust_type_prefix(22).unwrap(),
            ty_to_rust_assembled(inner),
            rust_type_separator(22).unwrap(),
            size,
            rust_type_suffix(22).unwrap()
        ),
        Ty::Func(args, ret) => {
            let rendered: Vec<String> = args.iter().map(ty_to_rust_assembled).collect();
            format!(
                "{}{}{}{}{}",
                rust_type_prefix(23).unwrap(),
                rendered.join(rust_type_separator(23).unwrap()),
                rust_type_midfix(23).unwrap(),
                ty_to_rust_assembled(ret),
                rust_type_suffix(23).unwrap()
            )
        }
    }
}

/// Tree form of IRLiteral for the literal/pattern mirrors.
#[allow(dead_code)]
enum Lit {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(&'static str),
    Char(char),
    Unit,
}

/// Faithful copy of the Literal arm of `ir_to_rust_expr`
/// (src/codegen/mod.rs:605-612): Int/Float/Bool via
/// to_string (f64 Display: 1.0 renders "1"), String quoted
/// RAW (no escaping), Char single-quoted, Unit "()".
fn ir_literal_text(lit: &Lit) -> String {
    match lit {
        Lit::Int(n) => n.to_string(),
        Lit::Float(f) => f.to_string(),
        Lit::Bool(b) => b.to_string(),
        Lit::Str(s) => format!("\"{}\"", s),
        Lit::Char(c) => format!("'{}'", c),
        Lit::Unit => "()".to_string(),
    }
}

/// Tree form of IRPattern for the assembled pattern mirror.
/// Names arrive already rendered (rust_ident is slice 3).
#[allow(dead_code)]
enum Pat {
    Wildcard,
    Variable(&'static str),
    Literal(Lit),
    Constructor(&'static str, Vec<Pat>),
    Guard(Box<Pat>, &'static str),
}

/// The Literal arm of `ir_pattern_to_rust`
/// (src/codegen/mod.rs:817-822): Int/Bool/String render their
/// value (String quoted raw); Float/Char/Unit fall back to
/// the wildcard text pinned by rust_pattern_literal_text.
fn ir_pattern_literal_rendered(lit: &Lit) -> String {
    match lit {
        Lit::Int(n) => n.to_string(),
        Lit::Bool(b) => b.to_string(),
        Lit::Str(s) => format!("\"{}\"", s),
        _ => "_".to_string(),
    }
}

/// Faithful assembly of `ir_pattern_to_rust`
/// (src/codegen/mod.rs:813-831).
fn ir_pattern_assembled(pat: &Pat) -> String {
    match pat {
        Pat::Wildcard => "_".to_string(),
        Pat::Variable(name) => name.to_string(),
        Pat::Literal(lit) => ir_pattern_literal_rendered(lit),
        Pat::Constructor(name, subs) => {
            if subs.is_empty() {
                // The branch rust_pattern_ctor_name pins.
                rust_pattern_ctor_name(name, false).unwrap()
            } else {
                let rendered: Vec<String> = subs.iter().map(ir_pattern_assembled).collect();
                format!("{}({})", name, rendered.join(", "))
            }
        }
        Pat::Guard(inner, guard) => format!("{} if {}", ir_pattern_assembled(inner), guard),
    }
}

/// Tree form of the spec `Expr` for the assembled
/// `expr_to_rust` mirror. The fallback variants carry no
/// payload: their content is irrelevant, all render "??".
#[allow(dead_code)]
enum SExpr {
    Var(&'static str),
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    StringLit(&'static str),
    Field,
    Index,
    Call(&'static str, Vec<SExpr>),
    BinOp(i64, Box<SExpr>, Box<SExpr>),
    UnaryOp,
    Lambda,
    Length,
    Comprehension,
}

/// Faithful assembly of `expr_to_rust`
/// (src/codegen/mod.rs:1177-1200), with the operator symbol
/// from the slice-1 spec_binop_symbol mirror and "??" for
/// every kind without an arm.
fn expr_to_rust_assembled(expr: &SExpr) -> String {
    match expr {
        SExpr::Var(name) => name.to_string(),
        SExpr::IntLit(n) => n.to_string(),
        SExpr::FloatLit(_) => "??".to_string(),
        SExpr::BoolLit(b) => b.to_string(),
        SExpr::StringLit(s) => format!("\"{}\"", s),
        SExpr::Call(name, args) => {
            let rendered: Vec<String> = args.iter().map(expr_to_rust_assembled).collect();
            format!("{}({})", name, rendered.join(", "))
        }
        SExpr::BinOp(op, lhs, rhs) => format!(
            "({} {} {})",
            expr_to_rust_assembled(lhs),
            spec_binop_symbol(*op).unwrap(),
            expr_to_rust_assembled(rhs)
        ),
        _ => "??".to_string(),
    }
}

/// Tree form of `Constraint` for the assembled
/// `constraint_to_rust` mirror. The catch-all variants carry
/// no payload: all render "true".
#[allow(dead_code)]
enum Cons {
    True,
    Expr,
    Forall,
    Exists,
    Implies,
    And(Vec<Cons>),
    Or,
    Not(Box<Cons>),
    Eq(SExpr, SExpr),
    Order(i64, SExpr, SExpr),
    Predicate,
}

/// Faithful assembly of `constraint_to_rust`
/// (src/codegen/mod.rs:1162-1174): Not wraps in "!(..)",
/// Eq/Order render their operands via expr_to_rust (Order
/// only for LessThan/LessThanOrEqual — the others fall into
/// the catch-all), And joins with " && " inside parens, and
/// everything else is "true".
fn constraint_to_rust_assembled(cons: &Cons) -> String {
    match cons {
        Cons::Not(inner) => format!("!({})", constraint_to_rust_assembled(inner)),
        Cons::Eq(a, b) => format!(
            "{} == {}",
            expr_to_rust_assembled(a),
            expr_to_rust_assembled(b)
        ),
        Cons::Order(op, a, b) => match rust_order_separator(*op) {
            Some(sep) => format!(
                "{}{}{}",
                expr_to_rust_assembled(a),
                sep,
                expr_to_rust_assembled(b)
            ),
            None => "true".to_string(),
        },
        Cons::And(parts) => {
            let rendered: Vec<String> = parts.iter().map(constraint_to_rust_assembled).collect();
            format!("({})", rendered.join(" && "))
        }
        _ => "true".to_string(),
    }
}

/// Assembly of `expr_in_condition_position`
/// (src/codegen/mod.rs:587-601) over an already-rendered
/// child: parenthesized exactly when rust_condition_wrap
/// says the kind wraps.
fn condition_text(kind: i64, rendered: &str) -> String {
    match rust_condition_wrap(kind) {
        Some(true) => format!("({})", rendered),
        _ => rendered.to_string(),
    }
}

// ═════════════════════════════════════════════════════════════
// A. GOLDEN TESTS — full mapping range + off-range inputs
// ═════════════════════════════════════════════════════════════

#[cfg(test)]
mod golden_tests {
    use super::*;

    #[test]
    fn type_names_full_table() {
        // .morph tests: int-name, string-name, unit-name, u64-name
        assert_eq!(rust_type_name(0), Some("i64"));
        assert_eq!(rust_type_name(3), Some("String"));
        assert_eq!(rust_type_name(5), Some("()"));
        assert_eq!(rust_type_name(13), Some("u64"));
        // Remaining rows of the table
        assert_eq!(rust_type_name(1), Some("f64"));
        assert_eq!(rust_type_name(2), Some("bool"));
        assert_eq!(rust_type_name(4), Some("char"));
        assert_eq!(rust_type_name(6), Some("i8"));
        assert_eq!(rust_type_name(7), Some("i16"));
        assert_eq!(rust_type_name(8), Some("i32"));
        assert_eq!(rust_type_name(9), Some("i64"));
        assert_eq!(rust_type_name(10), Some("u8"));
        assert_eq!(rust_type_name(11), Some("u16"));
        assert_eq!(rust_type_name(12), Some("u32"));
        // Composite / named codes are unpinned by the spec
        for tag in 14..=25 {
            assert_eq!(rust_type_name(tag), None, "tag {} must be unpinned", tag);
        }
        // Off-range
        assert_eq!(rust_type_name(-1), None);
        assert_eq!(rust_type_name(26), None);
        assert_eq!(rust_type_name(100), None);
    }

    #[test]
    fn binop_symbols_full_table() {
        // .morph tests: add, mod, xor, shl, concat symbols
        assert_eq!(rust_binop_symbol(0), Some("+"));
        assert_eq!(rust_binop_symbol(4), Some("%"));
        assert_eq!(rust_binop_symbol(7), Some("^"));
        assert_eq!(rust_binop_symbol(14), Some("<<"));
        assert_eq!(rust_binop_symbol(16), Some("+"));
        // Remaining rows of the table
        assert_eq!(rust_binop_symbol(1), Some("-"));
        assert_eq!(rust_binop_symbol(2), Some("*"));
        assert_eq!(rust_binop_symbol(3), Some("/"));
        assert_eq!(rust_binop_symbol(5), Some("&&"));
        assert_eq!(rust_binop_symbol(6), Some("||"));
        assert_eq!(rust_binop_symbol(8), Some("=="));
        assert_eq!(rust_binop_symbol(9), Some("!="));
        assert_eq!(rust_binop_symbol(10), Some("<"));
        assert_eq!(rust_binop_symbol(11), Some("<="));
        assert_eq!(rust_binop_symbol(12), Some(">"));
        assert_eq!(rust_binop_symbol(13), Some(">="));
        assert_eq!(rust_binop_symbol(15), Some(">>"));
        // Append / Merge have no infix symbol
        assert_eq!(rust_binop_symbol(17), None);
        assert_eq!(rust_binop_symbol(18), None);
        // Off-range
        assert_eq!(rust_binop_symbol(-1), None);
        assert_eq!(rust_binop_symbol(19), None);
    }

    #[test]
    fn unaryop_symbols() {
        // .morph tests: neg-symbol, not-symbol
        assert_eq!(rust_unaryop_symbol(0), Some("-"));
        assert_eq!(rust_unaryop_symbol(1), Some("!"));
        // Every other operator is unpinned in this table
        for op in 2..=9 {
            assert_eq!(rust_unaryop_symbol(op), None, "op {} must be unpinned", op);
        }
        assert_eq!(rust_unaryop_symbol(-1), None);
        assert_eq!(rust_unaryop_symbol(10), None);
    }

    #[test]
    fn unaryop_methods() {
        // .morph tests: abs-method, len-method, clone-method
        assert_eq!(rust_unaryop_method(2), Some("abs"));
        assert_eq!(rust_unaryop_method(3), Some("len"));
        assert_eq!(rust_unaryop_method(9), Some("clone"));
        // Remaining row of the table
        assert_eq!(rust_unaryop_method(8), Some("reverse"));
        // Prefix and list-part operators are unpinned here
        for op in [0, 1, 4, 5, 6, 7] {
            assert_eq!(rust_unaryop_method(op), None, "op {} must be unpinned", op);
        }
        assert_eq!(rust_unaryop_method(-1), None);
        assert_eq!(rust_unaryop_method(10), None);
    }

    #[test]
    fn unaryop_forms_total() {
        // .morph tests: neg-form, abs-form, head-form, clone-form
        assert_eq!(rust_unaryop_form(0), Some(0));
        assert_eq!(rust_unaryop_form(2), Some(1));
        assert_eq!(rust_unaryop_form(4), Some(2));
        assert_eq!(rust_unaryop_form(9), Some(1));
        // Remaining rows — the classification is total
        assert_eq!(rust_unaryop_form(1), Some(0));
        assert_eq!(rust_unaryop_form(3), Some(1));
        assert_eq!(rust_unaryop_form(5), Some(2));
        assert_eq!(rust_unaryop_form(6), Some(2));
        assert_eq!(rust_unaryop_form(7), Some(2));
        assert_eq!(rust_unaryop_form(8), Some(1));
        // Off-range
        assert_eq!(rust_unaryop_form(-1), None);
        assert_eq!(rust_unaryop_form(10), None);
    }

    #[test]
    fn collection_forms_total() {
        // .morph tests: list-form, set-form, tuple-form
        assert_eq!(rust_collection_form(0), Some(0));
        assert_eq!(rust_collection_form(1), Some(1));
        assert_eq!(rust_collection_form(4), Some(4));
        // Remaining rows — the classification is total
        assert_eq!(rust_collection_form(2), Some(2));
        assert_eq!(rust_collection_form(3), Some(3));
        // Off-range
        assert_eq!(rust_collection_form(-1), None);
        assert_eq!(rust_collection_form(5), None);
    }

    #[test]
    fn keyword_set_complete() {
        // Every word of the emitter's 51-word set (source order).
        let keywords = [
            "as", "break", "const", "continue", "crate", "else", "enum", "extern",
            "false", "fn", "for", "if", "impl", "in", "let", "loop", "match",
            "mod", "move", "mut", "pub", "ref", "return", "self", "Self",
            "static", "struct", "super", "trait", "true", "type", "unsafe",
            "use", "where", "while", "async", "await", "dyn", "abstract",
            "become", "box", "do", "final", "macro", "override", "priv",
            "typeof", "unsized", "virtual", "yield", "try",
        ];
        assert_eq!(keywords.len(), 51);
        for kw in keywords {
            assert!(is_rust_keyword(kw), "{} must be a keyword", kw);
        }
        // .morph tests: fn-keyword, self-keyword
        assert!(is_rust_keyword("fn"));
        assert!(is_rust_keyword("Self"));
    }

    #[test]
    fn keyword_set_negative_space() {
        // .morph tests: plain-name, prefix-name, case-name
        assert!(!is_rust_keyword("sort"));
        assert!(!is_rust_keyword("selfish"));
        assert!(!is_rust_keyword("Fn"));
        // Exact, case-sensitive matching — near misses stay names
        assert!(!is_rust_keyword(""));
        assert!(!is_rust_keyword("SELF"));
        assert!(!is_rust_keyword("fn2"));
        assert!(!is_rust_keyword("as "));
        assert!(!is_rust_keyword(" async"));
        assert!(!is_rust_keyword("crate_"));
        assert!(!is_rust_keyword("main"));
        // Real Rust words the emitter's set does NOT carry
        assert!(!is_rust_keyword("gen")); // 2024-edition keyword, absent here
        assert!(!is_rust_keyword("union")); // contextual keyword, absent here
    }

    #[test]
    fn spec_binop_symbols_full_table() {
        // .morph tests: add-symbol, neq-symbol, mod-fallback, lt-fallback
        assert_eq!(spec_binop_symbol(0), Some("+"));
        assert_eq!(spec_binop_symbol(8), Some("!="));
        assert_eq!(spec_binop_symbol(4), Some("?"));
        assert_eq!(spec_binop_symbol(9), Some("?"));
        // Remaining rows of the table
        assert_eq!(spec_binop_symbol(1), Some("-"));
        assert_eq!(spec_binop_symbol(2), Some("*"));
        assert_eq!(spec_binop_symbol(3), Some("/"));
        assert_eq!(spec_binop_symbol(5), Some("&&"));
        assert_eq!(spec_binop_symbol(6), Some("||"));
        assert_eq!(spec_binop_symbol(7), Some("=="));
        assert_eq!(spec_binop_symbol(10), Some("?"));
        assert_eq!(spec_binop_symbol(11), Some("?"));
        assert_eq!(spec_binop_symbol(12), Some("?"));
        assert_eq!(spec_binop_symbol(13), Some("?"));
        // Off-range
        assert_eq!(spec_binop_symbol(-1), None);
        assert_eq!(spec_binop_symbol(14), None);
    }

    #[test]
    fn cross_table_consistency() {
        // The unary tables agree: symbol-pinned ops are exactly
        // the prefix-form ops, method-pinned ops are exactly the
        // method-form ops, and list-part ops have neither.
        for op in 0..=9 {
            let form = rust_unaryop_form(op).expect("form is total");
            assert_eq!(rust_unaryop_symbol(op).is_some(), form == 0, "op {}", op);
            assert_eq!(rust_unaryop_method(op).is_some(), form == 1, "op {}", op);
        }
        // Collection forms are a permutation of 0..=4.
        let mut forms: Vec<i64> = (0..=4).map(|k| rust_collection_form(k).unwrap()).collect();
        forms.sort_unstable();
        assert_eq!(forms, vec![0, 1, 2, 3, 4]);
        // The type table's one collision: Int and I64 share a name.
        assert_eq!(rust_type_name(0), rust_type_name(9));
        // Concat shares Add's symbol in the IR table.
        assert_eq!(rust_binop_symbol(16), rust_binop_symbol(0));
        // The two operator tables DIVERGE by name (codes differ
        // between the enums; compared here by operator name):
        // Mod is spec-code 4 / IR-code 4, Lt is 9 / 10,
        // Concat is 13 / 16.
        assert_eq!(spec_binop_symbol(4), Some("?")); // spec Mod
        assert_eq!(rust_binop_symbol(4), Some("%")); // IR Mod
        assert_eq!(spec_binop_symbol(9), Some("?")); // spec Lt
        assert_eq!(rust_binop_symbol(10), Some("<")); // IR Lt
        assert_eq!(spec_binop_symbol(13), Some("?")); // spec Concat
        assert_eq!(rust_binop_symbol(16), Some("+")); // IR Concat
    }

    // ═════════════════════════════════════════════════════════
    // Slice 2 golden tests
    // ═════════════════════════════════════════════════════════

    #[test]
    fn type_forms_total() {
        // .morph tests: one per family
        assert_eq!(rust_type_form(0), Some(0));
        assert_eq!(rust_type_form(14), Some(1));
        assert_eq!(rust_type_form(16), Some(2));
        assert_eq!(rust_type_form(18), Some(3));
        assert_eq!(rust_type_form(21), Some(4));
        assert_eq!(rust_type_form(22), Some(5));
        assert_eq!(rust_type_form(23), Some(6));
        // Full domain
        for tag in 0..=13 {
            assert_eq!(rust_type_form(tag), Some(0), "tag {}", tag);
        }
        assert_eq!(rust_type_form(15), Some(1));
        for tag in [16, 17, 19, 24, 25] {
            assert_eq!(rust_type_form(tag), Some(2), "tag {}", tag);
        }
        assert_eq!(rust_type_form(20), Some(3));
        // Off-range
        assert_eq!(rust_type_form(-1), None);
        assert_eq!(rust_type_form(26), None);
        assert_eq!(rust_type_form(100), None);
    }

    #[test]
    fn type_affixes_full() {
        // .morph tests: vec/set/ref/stream prefixes, vec/tuple/
        // function/ref suffixes, map/array/function separators,
        // function midfix
        assert_eq!(rust_type_prefix(16), Some("Vec<"));
        assert_eq!(rust_type_prefix(17), Some("HashSet<"));
        assert_eq!(rust_type_prefix(24), Some("&"));
        assert_eq!(rust_type_prefix(25), Some("impl Iterator<Item = "));
        assert_eq!(rust_type_suffix(16), Some(">"));
        assert_eq!(rust_type_suffix(21), Some(")"));
        assert_eq!(rust_type_suffix(23), Some(""));
        assert_eq!(rust_type_suffix(24), Some(""));
        assert_eq!(rust_type_separator(18), Some(", "));
        assert_eq!(rust_type_separator(22), Some("; "));
        assert_eq!(rust_type_separator(23), Some(", "));
        assert_eq!(rust_type_midfix(23), Some(") -> "));
        // Remaining rows
        assert_eq!(rust_type_prefix(18), Some("HashMap<"));
        assert_eq!(rust_type_prefix(19), Some("Option<"));
        assert_eq!(rust_type_prefix(20), Some("Result<"));
        assert_eq!(rust_type_prefix(21), Some("("));
        assert_eq!(rust_type_prefix(22), Some("["));
        assert_eq!(rust_type_prefix(23), Some("fn("));
        for tag in [17, 18, 19, 20, 25] {
            assert_eq!(rust_type_suffix(tag), Some(">"), "tag {}", tag);
        }
        assert_eq!(rust_type_suffix(22), Some("]"));
        assert_eq!(rust_type_separator(20), Some(", "));
        assert_eq!(rust_type_separator(21), Some(", "));
        // Unpinned space: scalar and name codes have no affixes
        for tag in 0..=15 {
            assert_eq!(rust_type_prefix(tag), None, "prefix tag {}", tag);
            assert_eq!(rust_type_suffix(tag), None, "suffix tag {}", tag);
            assert_eq!(rust_type_separator(tag), None, "sep tag {}", tag);
            assert_eq!(rust_type_midfix(tag), None, "midfix tag {}", tag);
        }
        // Separators exist exactly for the two-child / join forms
        for tag in [16, 17, 19, 24, 25] {
            assert_eq!(rust_type_separator(tag), None, "sep tag {}", tag);
        }
        // Midfix exists only for Function
        for tag in [16, 17, 18, 19, 20, 21, 22, 24, 25] {
            assert_eq!(rust_type_midfix(tag), None, "midfix tag {}", tag);
        }
        // Off-range
        assert_eq!(rust_type_prefix(26), None);
        assert_eq!(rust_type_suffix(-1), None);
    }

    #[test]
    fn type_assembly_nested() {
        // One level
        assert_eq!(ty_to_rust_assembled(&Ty::Wrap1(16, Box::new(Ty::Leaf(0)))), "Vec<i64>");
        // The Set quirk: unqualified HashSet, no import
        assert_eq!(ty_to_rust_assembled(&Ty::Wrap1(17, Box::new(Ty::Leaf(0)))), "HashSet<i64>");
        assert_eq!(ty_to_rust_assembled(&Ty::Wrap1(24, Box::new(Ty::Leaf(3)))), "&String");
        assert_eq!(
            ty_to_rust_assembled(&Ty::Wrap1(25, Box::new(Ty::Leaf(0)))),
            "impl Iterator<Item = i64>"
        );
        // Two levels deep, composed mirror calls
        assert_eq!(
            ty_to_rust_assembled(&Ty::Wrap1(16, Box::new(Ty::Wrap1(16, Box::new(Ty::Leaf(0)))))),
            "Vec<Vec<i64>>"
        );
        assert_eq!(
            ty_to_rust_assembled(&Ty::Wrap1(
                19,
                Box::new(Ty::Wrap2(20, Box::new(Ty::Leaf(0)), Box::new(Ty::Leaf(3))))
            )),
            "Option<Result<i64, String>>"
        );
        assert_eq!(
            ty_to_rust_assembled(&Ty::Wrap2(
                18,
                Box::new(Ty::Leaf(3)),
                Box::new(Ty::Wrap1(16, Box::new(Ty::Leaf(2))))
            )),
            "HashMap<String, Vec<bool>>"
        );
        // Tuple: joined, empty (collides with Unit's text), and
        // the one-element form the emitter produces (no comma)
        assert_eq!(
            ty_to_rust_assembled(&Ty::Tuple(vec![Ty::Leaf(0), Ty::Leaf(3)])),
            "(i64, String)"
        );
        assert_eq!(ty_to_rust_assembled(&Ty::Tuple(vec![])), "()");
        assert_eq!(ty_to_rust_assembled(&Ty::Tuple(vec![Ty::Leaf(0)])), "(i64)");
        // Array with decimal size
        assert_eq!(ty_to_rust_assembled(&Ty::Array(Box::new(Ty::Leaf(0)), 3)), "[i64; 3]");
        assert_eq!(ty_to_rust_assembled(&Ty::Array(Box::new(Ty::Leaf(10)), 256)), "[u8; 256]");
        // Function: joined args, midfix, return type
        assert_eq!(
            ty_to_rust_assembled(&Ty::Func(vec![Ty::Leaf(0), Ty::Leaf(2)], Box::new(Ty::Leaf(5)))),
            "fn(i64, bool) -> ()"
        );
        assert_eq!(
            ty_to_rust_assembled(&Ty::Func(vec![], Box::new(Ty::Wrap1(19, Box::new(Ty::Leaf(3)))))),
            "fn() -> Option<String>"
        );
        // Name forms render the (pre-rendered) name itself
        assert_eq!(ty_to_rust_assembled(&Ty::Name("MyStruct")), "MyStruct");
        assert_eq!(
            ty_to_rust_assembled(&Ty::Wrap1(16, Box::new(Ty::Name("T")))),
            "Vec<T>"
        );
    }

    #[test]
    fn type_form_affix_consistency() {
        // The classification determines exactly which affix
        // tables pin a composite code.
        for tag in 14..=25 {
            let form = rust_type_form(tag).expect("composite form");
            assert_eq!(rust_type_prefix(tag).is_some(), form != 1, "prefix tag {}", tag);
            assert_eq!(rust_type_suffix(tag).is_some(), form != 1, "suffix tag {}", tag);
            assert_eq!(
                rust_type_separator(tag).is_some(),
                matches!(form, 3 | 4 | 5 | 6),
                "separator tag {}",
                tag
            );
            assert_eq!(rust_type_midfix(tag).is_some(), form == 6, "midfix tag {}", tag);
        }
    }

    #[test]
    fn literal_forms_and_fixed_texts() {
        // .morph tests: int/float/string/unit forms, unit text,
        // both Bool texts
        assert_eq!(rust_literal_form(0), Some(0));
        assert_eq!(rust_literal_form(1), Some(1));
        assert_eq!(rust_literal_form(3), Some(3));
        assert_eq!(rust_literal_form(5), Some(5));
        assert_eq!(rust_literal_form(2), Some(2));
        assert_eq!(rust_literal_form(4), Some(4));
        assert_eq!(rust_literal_fixed_text(5), Some("()"));
        assert_eq!(rust_literal_bool(true), "true");
        assert_eq!(rust_literal_bool(false), "false");
        // Only Unit has fixed text
        for kind in 0..=4 {
            assert_eq!(rust_literal_fixed_text(kind), None, "kind {}", kind);
        }
        // Off-range
        assert_eq!(rust_literal_form(-1), None);
        assert_eq!(rust_literal_form(6), None);
        assert_eq!(rust_literal_fixed_text(6), None);
    }

    #[test]
    fn literal_text_assembly() {
        // Int -> decimal, incl. negative and large values (the
        // conversion no spec can express; the mirror performs
        // it with to_string exactly as the emitter does)
        assert_eq!(ir_literal_text(&Lit::Int(0)), "0");
        assert_eq!(ir_literal_text(&Lit::Int(-42)), "-42");
        assert_eq!(ir_literal_text(&Lit::Int(9223372036854775807)), "9223372036854775807");
        assert_eq!(ir_literal_text(&Lit::Int(-9223372036854775808)), "-9223372036854775808");
        // f64 Display semantics: 1.0 renders "1", not "1.0"
        assert_eq!(ir_literal_text(&Lit::Float(1.0)), "1");
        assert_eq!(ir_literal_text(&Lit::Float(100.0)), "100");
        assert_eq!(ir_literal_text(&Lit::Float(0.5)), "0.5");
        assert_eq!(ir_literal_text(&Lit::Float(-2.25)), "-2.25");
        assert_eq!(ir_literal_text(&Lit::Float(0.1)), "0.1");
        // Bool via the pinned table
        assert_eq!(ir_literal_text(&Lit::Bool(true)), rust_literal_bool(true));
        assert_eq!(ir_literal_text(&Lit::Bool(false)), rust_literal_bool(false));
        // String: quoted RAW — empty, plain, and the breaking
        // cases (embedded quote / newline render as-is)
        assert_eq!(ir_literal_text(&Lit::Str("")), "\"\"");
        assert_eq!(ir_literal_text(&Lit::Str("hello")), "\"hello\"");
        assert_eq!(ir_literal_text(&Lit::Str("a\"b")), "\"a\"b\"");
        assert_eq!(ir_literal_text(&Lit::Str("a\nb")), "\"a\nb\"");
        // Char: single-quoted
        assert_eq!(ir_literal_text(&Lit::Char('x')), "'x'");
        assert_eq!(ir_literal_text(&Lit::Char(' ')), "' '");
        // Unit
        assert_eq!(ir_literal_text(&Lit::Unit), "()");
    }

    #[test]
    fn pattern_tables_full() {
        // .morph tests: wildcard/literal/guard forms, wildcard
        // text, literal family rows, fallback texts
        assert_eq!(rust_pattern_form(0), Some(0));
        assert_eq!(rust_pattern_form(2), Some(2));
        assert_eq!(rust_pattern_form(4), Some(4));
        assert_eq!(rust_pattern_form(1), Some(1));
        assert_eq!(rust_pattern_form(3), Some(3));
        assert_eq!(rust_pattern_fixed_text(0), Some("_"));
        assert_eq!(rust_pattern_literal_form(0), Some(0));
        assert_eq!(rust_pattern_literal_form(1), Some(1));
        assert_eq!(rust_pattern_literal_form(3), Some(2));
        assert_eq!(rust_pattern_literal_form(5), Some(1));
        assert_eq!(rust_pattern_literal_form(2), Some(0));
        assert_eq!(rust_pattern_literal_form(4), Some(1));
        assert_eq!(rust_pattern_literal_text(1), Some("_"));
        assert_eq!(rust_pattern_literal_text(4), Some("_"));
        assert_eq!(rust_pattern_literal_text(5), Some("_"));
        // Unpinned space
        for kind in 1..=4 {
            assert_eq!(rust_pattern_fixed_text(kind), None, "kind {}", kind);
        }
        for kind in [0, 2, 3] {
            assert_eq!(rust_pattern_literal_text(kind), None, "kind {}", kind);
        }
        // Off-range
        assert_eq!(rust_pattern_form(5), None);
        assert_eq!(rust_pattern_form(-1), None);
        assert_eq!(rust_pattern_literal_form(6), None);
    }

    #[test]
    fn pattern_assembly() {
        assert_eq!(ir_pattern_assembled(&Pat::Wildcard), "_");
        assert_eq!(ir_pattern_assembled(&Pat::Variable("x")), "x");
        // Literal patterns: value kinds render, the rest widen
        // to the wildcard
        assert_eq!(ir_pattern_assembled(&Pat::Literal(Lit::Int(3))), "3");
        assert_eq!(ir_pattern_assembled(&Pat::Literal(Lit::Int(-1))), "-1");
        assert_eq!(ir_pattern_assembled(&Pat::Literal(Lit::Bool(false))), "false");
        assert_eq!(ir_pattern_assembled(&Pat::Literal(Lit::Str("hi"))), "\"hi\"");
        assert_eq!(ir_pattern_assembled(&Pat::Literal(Lit::Float(1.5))), "_");
        assert_eq!(ir_pattern_assembled(&Pat::Literal(Lit::Char('a'))), "_");
        assert_eq!(ir_pattern_assembled(&Pat::Literal(Lit::Unit)), "_");
        // Constructor: bare name without sub-patterns (the
        // branch rust_pattern_ctor_name pins), template with
        assert_eq!(ir_pattern_assembled(&Pat::Constructor("None", vec![])), "None");
        assert_eq!(
            ir_pattern_assembled(&Pat::Constructor("Some", vec![Pat::Variable("x")])),
            "Some(x)"
        );
        assert_eq!(
            ir_pattern_assembled(&Pat::Constructor(
                "Pair",
                vec![Pat::Literal(Lit::Int(1)), Pat::Wildcard]
            )),
            "Pair(1, _)"
        );
        // Nested constructors, two levels
        assert_eq!(
            ir_pattern_assembled(&Pat::Constructor(
                "Some",
                vec![Pat::Constructor("Ok", vec![Pat::Variable("v")])]
            )),
            "Some(Ok(v))"
        );
        // Guard: pattern + raw guard text
        assert_eq!(
            ir_pattern_assembled(&Pat::Guard(Box::new(Pat::Variable("x")), "x > 0")),
            "x if x > 0"
        );
        // The ctor-name mirror agrees with the assembly on the
        // empty branch and abstains on the assembly branch
        assert_eq!(rust_pattern_ctor_name("Some", false), Some("Some".to_string()));
        assert_eq!(rust_pattern_ctor_name("", false), Some("".to_string()));
        assert_eq!(rust_pattern_ctor_name("Some", true), None);
    }

    #[test]
    fn expr_tables_full() {
        // .morph tests: var/floatlit/call/binop/comprehension
        // forms, floatlit/unaryop fixed texts
        assert_eq!(rust_expr_form(0), Some(0));
        assert_eq!(rust_expr_form(2), Some(6));
        assert_eq!(rust_expr_form(7), Some(4));
        assert_eq!(rust_expr_form(8), Some(5));
        assert_eq!(rust_expr_form(12), Some(6));
        assert_eq!(rust_expr_form(1), Some(1));
        assert_eq!(rust_expr_form(3), Some(2));
        assert_eq!(rust_expr_form(4), Some(3));
        assert_eq!(rust_expr_fixed_text(2), Some("??"));
        assert_eq!(rust_expr_fixed_text(9), Some("??"));
        // The fallback family and the fixed-text table coincide
        for kind in 0..=12 {
            assert_eq!(
                rust_expr_form(kind) == Some(6),
                rust_expr_fixed_text(kind) == Some("??"),
                "kind {}",
                kind
            );
        }
        // Off-range
        assert_eq!(rust_expr_form(13), None);
        assert_eq!(rust_expr_form(-1), None);
        assert_eq!(rust_expr_fixed_text(13), None);
    }

    #[test]
    fn expr_assembly() {
        assert_eq!(expr_to_rust_assembled(&SExpr::Var("x")), "x");
        assert_eq!(expr_to_rust_assembled(&SExpr::IntLit(-7)), "-7");
        assert_eq!(expr_to_rust_assembled(&SExpr::BoolLit(true)), "true");
        assert_eq!(expr_to_rust_assembled(&SExpr::StringLit("s")), "\"s\"");
        // No arm: FloatLit and UnaryOp render "??", as do the
        // structural kinds
        assert_eq!(expr_to_rust_assembled(&SExpr::FloatLit(1.5)), "??");
        assert_eq!(expr_to_rust_assembled(&SExpr::UnaryOp), "??");
        assert_eq!(expr_to_rust_assembled(&SExpr::Field), "??");
        assert_eq!(expr_to_rust_assembled(&SExpr::Comprehension), "??");
        // Call template: joined args, empty args
        assert_eq!(
            expr_to_rust_assembled(&SExpr::Call("f", vec![SExpr::IntLit(1), SExpr::Var("x")])),
            "f(1, x)"
        );
        assert_eq!(expr_to_rust_assembled(&SExpr::Call("g", vec![])), "g()");
        // Binary template with the slice-1 symbol table,
        // including its "?" rows
        assert_eq!(
            expr_to_rust_assembled(&SExpr::BinOp(
                0,
                Box::new(SExpr::IntLit(1)),
                Box::new(SExpr::IntLit(2))
            )),
            "(1 + 2)"
        );
        assert_eq!(
            expr_to_rust_assembled(&SExpr::BinOp(
                4,
                Box::new(SExpr::IntLit(1)),
                Box::new(SExpr::IntLit(2))
            )),
            "(1 ? 2)"
        );
        assert_eq!(
            expr_to_rust_assembled(&SExpr::BinOp(
                13,
                Box::new(SExpr::StringLit("a")),
                Box::new(SExpr::StringLit("b"))
            )),
            "(\"a\" ? \"b\")"
        );
        // Nested binary, two levels
        assert_eq!(
            expr_to_rust_assembled(&SExpr::BinOp(
                5,
                Box::new(SExpr::BinOp(
                    7,
                    Box::new(SExpr::Var("a")),
                    Box::new(SExpr::IntLit(1))
                )),
                Box::new(SExpr::BoolLit(true))
            )),
            "((a == 1) && true)"
        );
    }

    #[test]
    fn constraint_tables_full() {
        // .morph tests: true/and/or/not/order forms, true/or/
        // predicate texts, all four order rows, both separators
        assert_eq!(rust_constraint_form(0), Some(0));
        assert_eq!(rust_constraint_form(5), Some(4));
        assert_eq!(rust_constraint_form(6), Some(0));
        assert_eq!(rust_constraint_form(7), Some(1));
        assert_eq!(rust_constraint_form(9), Some(3));
        assert_eq!(rust_constraint_form(8), Some(2));
        assert_eq!(rust_constraint_fixed_text(0), Some("true"));
        assert_eq!(rust_constraint_fixed_text(6), Some("true"));
        assert_eq!(rust_constraint_fixed_text(10), Some("true"));
        assert_eq!(rust_order_form(0), Some(0));
        assert_eq!(rust_order_form(1), Some(1));
        assert_eq!(rust_order_form(2), Some(2));
        assert_eq!(rust_order_form(3), Some(2));
        assert_eq!(rust_order_separator(0), Some(" < "));
        assert_eq!(rust_order_separator(1), Some(" <= "));
        // Family 0 and the fixed-text table coincide
        for kind in 0..=10 {
            assert_eq!(
                rust_constraint_form(kind) == Some(0),
                rust_constraint_fixed_text(kind) == Some("true"),
                "kind {}",
                kind
            );
        }
        // Order: the fallback family is exactly where no
        // separator exists
        for op in 0..=3 {
            assert_eq!(
                rust_order_form(op) == Some(2),
                rust_order_separator(op).is_none(),
                "op {}",
                op
            );
        }
        // Off-range
        assert_eq!(rust_constraint_form(11), None);
        assert_eq!(rust_order_form(4), None);
        assert_eq!(rust_order_separator(-1), None);
    }

    #[test]
    fn constraint_assembly() {
        // True and every catch-all kind render "true"
        assert_eq!(constraint_to_rust_assembled(&Cons::True), "true");
        assert_eq!(constraint_to_rust_assembled(&Cons::Or), "true");
        assert_eq!(constraint_to_rust_assembled(&Cons::Predicate), "true");
        assert_eq!(constraint_to_rust_assembled(&Cons::Forall), "true");
        // Negation template, nested
        assert_eq!(constraint_to_rust_assembled(&Cons::Not(Box::new(Cons::True))), "!(true)");
        assert_eq!(
            constraint_to_rust_assembled(&Cons::Not(Box::new(Cons::Not(Box::new(Cons::Eq(
                SExpr::Var("x"),
                SExpr::IntLit(3)
            )))))),
            "!(!(x == 3))"
        );
        // Equality over expr_to_rust operands
        assert_eq!(
            constraint_to_rust_assembled(&Cons::Eq(SExpr::Var("x"), SExpr::IntLit(3))),
            "x == 3"
        );
        // Order: LT/LTE render, GT/GTE assert nothing
        assert_eq!(
            constraint_to_rust_assembled(&Cons::Order(0, SExpr::Var("x"), SExpr::IntLit(3))),
            "x < 3"
        );
        assert_eq!(
            constraint_to_rust_assembled(&Cons::Order(1, SExpr::Var("x"), SExpr::IntLit(3))),
            "x <= 3"
        );
        assert_eq!(
            constraint_to_rust_assembled(&Cons::Order(2, SExpr::Var("x"), SExpr::IntLit(3))),
            "true"
        );
        assert_eq!(
            constraint_to_rust_assembled(&Cons::Order(3, SExpr::Var("x"), SExpr::IntLit(3))),
            "true"
        );
        // Conjunction: joined in parens; one part still
        // parenthesized; zero parts render "()"
        assert_eq!(
            constraint_to_rust_assembled(&Cons::And(vec![
                Cons::Eq(SExpr::Var("x"), SExpr::IntLit(3)),
                Cons::Order(0, SExpr::Var("x"), SExpr::IntLit(4)),
            ])),
            "(x == 3 && x < 4)"
        );
        assert_eq!(
            constraint_to_rust_assembled(&Cons::And(vec![Cons::Eq(
                SExpr::Var("x"),
                SExpr::IntLit(3)
            )])),
            "(x == 3)"
        );
        assert_eq!(constraint_to_rust_assembled(&Cons::And(vec![])), "()");
    }

    #[test]
    fn condition_wrap_total() {
        // .morph tests: match/loop wrap, literal/hole bare
        assert_eq!(rust_condition_wrap(6), Some(true));
        assert_eq!(rust_condition_wrap(24), Some(true));
        assert_eq!(rust_condition_wrap(9), Some(false));
        assert_eq!(rust_condition_wrap(0), Some(false));
        // The full 28-kind domain: exactly the ten block-like
        // kinds wrap
        let wrapping = [1, 2, 3, 4, 5, 6, 13, 14, 24, 26];
        for kind in 0..=27 {
            assert_eq!(
                rust_condition_wrap(kind),
                Some(wrapping.contains(&kind)),
                "kind {}",
                kind
            );
        }
        // Off-range
        assert_eq!(rust_condition_wrap(28), None);
        assert_eq!(rust_condition_wrap(-1), None);
    }

    #[test]
    fn condition_assembly() {
        // Block-like renderings are parenthesized
        assert_eq!(
            condition_text(6, "match y { _ => 1 }"),
            "(match y { _ => 1 })"
        );
        assert_eq!(condition_text(13, "|x| { x }"), "(|x| { x })");
        assert_eq!(condition_text(1, "{ x }"), "({ x })");
        // A BinOp condition is already parenthesized by its own
        // template and is NOT wrapped again
        assert_eq!(condition_text(11, "(a && b)"), "(a && b)");
        assert_eq!(condition_text(10, "flag"), "flag");
    }
}

// ═════════════════════════════════════════════════════════════
// B. REAL PIPELINE — `morphic check` (parse + typecheck)
// ═════════════════════════════════════════════════════════════

#[cfg(test)]
mod pipeline_tests {
    use super::*;

    fn morphic_bin() -> &'static str {
        env!("CARGO_BIN_EXE_morphic")
    }

    fn run_check_source(source: &str, label: &str) -> std::process::Output {
        let mut path = std::env::temp_dir();
        path.push(format!("morphic_m3_{}_{}.morph", label, std::process::id()));
        {
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(source.as_bytes()).unwrap();
        }
        let out = Command::new(morphic_bin())
            .arg("check")
            .arg(&path)
            .output()
            .expect("failed to run morphic binary");
        let _ = std::fs::remove_file(&path);
        out
    }

    #[test]
    fn check_codegen_morph() {
        let out = Command::new(morphic_bin())
            .args(["check", "examples/self_host/codegen.morph"])
            .output()
            .expect("failed to run morphic binary");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(out.status.success(), "codegen.morph failed check:\n{}", stdout);
        assert!(stdout.contains("Specification is valid"), "unexpected output:\n{}", stdout);
    }

    #[test]
    fn check_rejects_undefined_function_call() {
        // Negative control: calls must resolve to a spec in the module.
        let src = r#"
T {
spec bad {
    input: op: Int
    output: String
    constraint: output == nosuchfn(op)
}
}
"#;
        let out = run_check_source(src, "undef_fn");
        assert!(!out.status.success(), "undefined function call was accepted");
    }

    #[test]
    fn spec_file_has_expected_shape() {
        // Supplementary shape check (the pipeline test above is the
        // real validation): the file declares the 8 slice-1 specs
        // plus the 20 slice-2 specs.
        let src = std::fs::read_to_string("examples/self_host/codegen.morph").unwrap();
        assert!(src.contains("Codegen"), "missing module Codegen");
        let spec_decls = src.lines().filter(|l| l.starts_with("spec ")).count();
        assert_eq!(spec_decls, 28, "codegen.morph should declare 28 specs");
        assert!(src.matches("test \"").count() >= 90, "codegen.morph should carry in-spec tests");
    }
}
