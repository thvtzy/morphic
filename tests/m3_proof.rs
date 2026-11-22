// ┌────────────────────────────────────────────────────────────┐
// │  M3: Codegen — Proof Tests (slices 1–6: leaf mappings,   │
// │  template families, name/text assembly, the expression  │
// │  walker, the statement walker, the driver)               │
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
/// `rust_binop_symbol`. Through Phase 54 the operators it did
/// not map fell through to "?" in the original, pinned here as
/// rows; the Phase 55 amendment (2026-11-21) gives them their
/// real Rust symbols — Concat shares Add's "+", exactly as the
/// IR table's Concat row does.
fn spec_binop_symbol(op: i64) -> Option<&'static str> {
    match op {
        0 => Some("+"),  // Add
        1 => Some("-"),  // Sub
        2 => Some("*"),  // Mul
        3 => Some("/"),  // Div
        4 => Some("%"),  // Mod
        5 => Some("&&"), // And
        6 => Some("||"), // Or
        7 => Some("=="), // Eq
        8 => Some("!="), // Neq
        9 => Some("<"),  // Lt
        10 => Some("<="), // Le
        11 => Some(">"), // Gt
        12 => Some(">="), // Ge
        13 => Some("+"), // Concat — shares Add's symbol
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
/// 6 = "??" fallback, 7 = closure template. Total over the
/// thirteen Expr kinds. FloatLit (2) and UnaryOp (9) have no
/// arm in the original and land in family 6; Lambda (10)
/// joins family 7 by the Phase 55 amendment (2026-11-21).
fn rust_expr_form(kind: i64) -> Option<i64> {
    match kind {
        0 => Some(0),       // Var
        1 => Some(1),       // IntLit
        3 => Some(2),       // BoolLit
        4 => Some(3),       // StringLit
        7 => Some(4),       // Call
        8 => Some(5),       // BinOp
        10 => Some(7),      // Lambda — closure template (Phase 55)
        5 => Some(8),       // Field — projection template (Phase 78)
        13 => Some(9),      // MethodCall — method template (Phase 78)
        14 => Some(10),     // RecordLit — record template (Phase 78)
        2 | 6 | 9 | 11 | 12 => Some(6), // FloatLit, Index, UnaryOp, Length, Comprehension
        _ => None,
    }
}

/// Mirror of the catch-all of `expr_to_rust`
/// (src/codegen/mod.rs:1199): every kind without an arm
/// renders "??". Lambda (10) left this table in the
/// Phase 55 amendment (2026-11-21); Field (5) left it in
/// the Phase 78 amendment (2026-11-22), whose amendment
/// also added the MethodCall fallback row (13).
fn rust_expr_fixed_text(kind: i64) -> Option<&'static str> {
    match kind {
        2 | 6 | 9 | 11 | 12 | 13 => Some("??"),
        _ => None,
    }
}

/// Mirror of `rust_expr_call_form` (Phase 55): which
/// rendering a Call takes, keyed by the call's name —
/// 0 = plain call template, 1 = vec literal ("List"),
/// 2 = tuple ("__tuple__"), 3 = bare constructor
/// ("None"). Total: every other name is family 0.
fn rust_expr_call_form(name: &str) -> i64 {
    match name {
        "List" => 1,
        "__tuple__" => 2,
        "None" => 3,
        _ => 0,
    }
}

/// Mirror of `rust_expr_list_text` (Phase 55): the vec
/// literal over pre-rendered, pre-joined elements.
fn rust_expr_list_text(args: &str) -> String {
    format!("vec![{}]", args)
}

/// Mirror of `rust_expr_tuple_text` (Phase 55): the Rust
/// tuple over pre-rendered, pre-joined elements.
fn rust_expr_tuple_text(args: &str) -> String {
    format!("({})", args)
}

/// Mirror of `rust_expr_lambda_text` (Phase 55): the
/// closure template over pre-rendered, pre-joined
/// parameter names and the walker's body rendering.
fn rust_expr_lambda_text(params: &str, body: &str) -> String {
    format!("|{}| {}", params, body)
}

/// Mirror of `rust_expr_field_text` (Phase 78): the
/// projection template over the rendered receiver and
/// the field name.
fn rust_expr_field_text(receiver: &str, field: &str) -> String {
    format!("{}.{}", receiver, field)
}

/// Mirror of `rust_expr_method_form` (Phase 78): which
/// rendering a MethodCall takes, keyed by the EFFECTIVE
/// method name — 0 = the verbatim template (the closed
/// D1a table), 1 = the fallback. The walker folds the
/// argument count into the key (a call carrying
/// arguments passes the empty name), exactly as the
/// spec's doc records. Total.
fn rust_expr_method_form(key: &str) -> i64 {
    match key {
        "is_ok" | "is_err" | "unwrap" | "len" => 0,
        _ => 1,
    }
}

/// Mirror of `rust_expr_method_text` (Phase 78): the
/// verbatim method template over the rendered receiver
/// and the method name.
fn rust_expr_method_text(receiver: &str, name: &str) -> String {
    format!("{}.{}()", receiver, name)
}

/// Mirror of `rust_expr_record_text` (Phase 78): the
/// struct-literal frame over the type name and the
/// pre-rendered, pre-joined fields.
fn rust_expr_record_text(name: &str, fields: &str) -> String {
    format!("{} {{ {} }}", name, fields)
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

// ═════════════════════════════════════════════════════════════
// A3. MIRRORS — slice 3 name/text assembly
//
// rust_ident, the String-ownership rule of
// ir_to_rust_expr_typed, and generate_rust's assembly
// pieces. With F1 resolved (the `++` operator), the .morph
// specs pin this slice's templates as equations over
// already-rendered parts; the mirrors below state the same
// behavior in Rust, plus the parts no spec can state:
// rust_ident's per-char loop (finding F5) and the ", "
// joins (iteration), which the assembly mirrors perform
// exactly as the originals do.
// ═════════════════════════════════════════════════════════════

/// Mirror of `rust_ident` whole (src/codegen/mod.rs:32-52),
/// copied line for line: the per-char loop, the empty
/// fallback, the keyword suffix.
fn rust_ident_verbatim(name: &str) -> String {
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

/// Mirror of the per-char step of `rust_ident`
/// (src/codegen/mod.rs:33-48): keep `_` and alphanumeric
/// chars — `char::is_alphanumeric`, which is UNICODE-aware,
/// despite the original's doc comment claiming the kept set
/// is `[A-Za-z0-9_]` — prefix `_` when the FIRST char is an
/// ASCII digit (only ASCII: a leading Unicode digit is kept
/// with no prefix), replace every other char with `_`; the
/// empty name becomes "_". No spec pins this loop (finding
/// F5); this mirror is its only executable statement.
fn rust_ident_sanitize(name: &str) -> String {
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
    out
}

/// Mirror of the keyword-suffix step of `rust_ident`
/// (src/codegen/mod.rs:49-51) applied to the sanitized
/// string: append `_` iff it is a keyword (the slice-1
/// `is_rust_keyword` mirror above). This is the step the
/// spec `rust_ident_suffix` pins as a composition over
/// `rust_keyword`.
fn rust_ident_suffix(name: &str) -> String {
    if is_rust_keyword(name) {
        format!("{}_", name)
    } else {
        name.to_string()
    }
}

/// Mirror of the spec `rust_ident_empty`: the per-char
/// step's empty fallback is the one case the spec pins —
/// Some exactly for the empty name.
fn rust_ident_empty(name: &str) -> Option<String> {
    if name.is_empty() {
        Some("_".to_string())
    } else {
        None
    }
}

/// Mirror of the dispatch classification of
/// `ir_to_rust_expr_typed` (src/codegen/mod.rs:549-558):
/// 0 = delegate to the untyped emitter, 1 = Literal (the
/// ownership check decides), 2 = Typed (re-enter with the
/// declared type as expectation). Domain: the 28 IRNode
/// kinds, as with `rust_condition_wrap`.
fn rust_typed_form(kind: i64) -> Option<i64> {
    match kind {
        9 => Some(1),
        22 => Some(2),
        0..=27 => Some(0),
        _ => None,
    }
}

/// Mirror of the ownership leaf check of
/// `ir_to_rust_expr_typed` (src/codegen/mod.rs:552-554):
/// owned exactly for a String literal (kind 3) under a
/// String expectation (ExpectedTypeCode 3); code 26 is
/// None, and every other code is Some(other type).
fn rust_string_ownership(lit: i64, expected: i64) -> bool {
    lit == 3 && expected == 3
}

/// The emitter's string escaping, hand-mirrored (Phase
/// 29): `"` and `\` take a backslash; `\n`, `\r`, `\t`
/// use their short forms; other control characters use
/// `\u{...}`; everything else (including non-ASCII text)
/// is emitted as-is. The rule is per-character (F5), so
/// it lives here and in the emitter, not in the spec —
/// the spec pins the wrap on the identity subdomain.
fn escape_rust_str_mirror(s: &str) -> String {
    let mut out = String::new();
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

/// Mirror of the bare String literal rendering
/// (src/codegen/mod.rs; the same template in pattern
/// position): quote + ESCAPED contents + quote (Phase
/// 29 — before the unfreeze the contents rendered RAW).
fn rust_string_literal_text(contents: &str) -> String {
    format!("\"{}\"", escape_rust_str_mirror(contents))
}

/// Mirror of the owned String literal rendering
/// (src/codegen/mod.rs): the bare template with
/// `.to_string()` appended, composed exactly as the
/// emitter's typed arm composes the literal arm.
fn rust_string_literal_owned_text(contents: &str) -> String {
    format!("{}.to_string()", rust_string_literal_text(contents))
}

/// Mirror of one doc-header line (src/codegen/mod.rs:73).
fn rust_doc_line(line: &str) -> String {
    format!("//! {}\n", line)
}

/// Mirror of one signature parameter's text
/// (src/codegen/mod.rs:91): rendered name, ": ", rendered
/// type.
fn rust_param_text(name: &str, ty: &str) -> String {
    format!("{}: {}", name, ty)
}

/// Mirror of one generic parameter's text inside the
/// generics bracket (Phase 62, owner decision (i)):
/// rendered name + ": Clone" — every declared generic
/// parameter of a generated signature carries the bound.
fn rust_generic_param_text(name: &str) -> String {
    format!("{}: Clone", name)
}

/// Mirror of the signature's generics bracket
/// (src/codegen/mod.rs:80-88) over the pre-joined rendered
/// generic names.
fn rust_generics_text(joined: &str, present: bool) -> String {
    if present {
        format!("<{}>", joined)
    } else {
        String::new()
    }
}

/// Mirror of the signature line (src/codegen/mod.rs:107-110)
/// over pre-rendered parts.
fn rust_signature(name: &str, generics: &str, params: &str, ret: &str) -> String {
    format!("pub fn {}{}({}) -> {} {{\n", name, generics, params, ret)
}

/// Mirror of one precondition assert line
/// (src/codegen/mod.rs:100-101) over the pre-rendered
/// constraint text.
fn rust_precondition_assert(rendered: &str) -> String {
    format!("    debug_assert!({}, \"Precondition violated\");\n", rendered)
}

/// Mirror of the precondition block
/// (src/codegen/mod.rs:96-104) over the pre-joined assert
/// lines, push for push: attribute line, "{", the asserts,
/// "}" and a blank line. The block is emitted BEFORE the
/// signature in the original (surprise 13).
fn rust_precondition_block(asserts: &str) -> String {
    let mut out = String::new();
    out.push_str("#[cfg(debug_assertions)]\n");
    out.push_str("{\n");
    out.push_str(asserts);
    out.push_str("}\n\n");
    out
}

/// Mirror of the test-name choice (src/codegen/mod.rs:136):
/// 0 = the test's own name, 1 = the indexed default. Only
/// the choice is spec-pinned (rust_test_name_form).
fn rust_test_name_form(has_name: bool) -> i64 {
    if has_name {
        0
    } else {
        1
    }
}

/// Mirror of the default test name's TEXT
/// (src/codegen/mod.rs:136): `test_{i}` with the index in
/// decimal — mirror-only, because the spec language has
/// no Int -> decimal conversion (finding F2).
fn rust_test_name_default(index: usize) -> String {
    format!("test_{}", index)
}

/// Mirror of one generated test's block
/// (src/codegen/mod.rs:139-150), push for push, over
/// pre-rendered parts. The generics text (Phase 61) is
/// the call's turbofish annotation, placed between the
/// function name and the argument list — empty unless
/// the driver's E0283-class rule supplies it (see
/// test_call_generics_text below).
fn rust_test_case(
    test_name: &str,
    fn_name: &str,
    generics_text: &str,
    input_text: &str,
    expected_text: &str,
) -> String {
    let mut out = String::new();
    out.push_str("    #[test]\n");
    out.push_str(&format!("    fn {}() {{\n", test_name));
    out.push_str(&format!(
        "        let result = {}{}({});\n",
        fn_name, generics_text, input_text
    ));
    out.push_str(&format!(
        "        assert_eq!(result, {});\n",
        expected_text
    ));
    out.push_str("    }\n");
    out
}

/// Mirror of the test-module wrapper
/// (src/codegen/mod.rs:131-133,151), push for push, over
/// the pre-joined test blocks.
fn rust_test_module(tests: &str) -> String {
    let mut out = String::new();
    out.push_str("\n#[cfg(test)]\n");
    out.push_str("mod tests {\n");
    out.push_str("    use super::*;\n\n");
    out.push_str(tests);
    out.push_str("}\n");
    out
}

/// Mirror of `generate_rust`'s two standalone fixed texts:
/// piece 0 = the function body's closing brace
/// (src/codegen/mod.rs:127), piece 1 = the blank line
/// after the doc header (src/codegen/mod.rs:75).
fn rust_file_fixed_text(piece: i64) -> Option<&'static str> {
    match piece {
        0 => Some("}\n"),
        1 => Some("\n"),
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
/// Lambda carries its parameter names and body since the
/// Phase 55 amendment gave it the closure template;
/// Field, MethodCall and RecordLit carry theirs since
/// the Phase 78 amendment gave them their templates.
#[allow(dead_code)]
enum SExpr {
    Var(&'static str),
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),
    StringLit(&'static str),
    Field(Box<SExpr>, &'static str),
    Index,
    Call(&'static str, Vec<SExpr>),
    BinOp(i64, Box<SExpr>, Box<SExpr>),
    UnaryOp,
    Lambda(Vec<&'static str>, Box<SExpr>),
    Length,
    Comprehension,
    MethodCall(Box<SExpr>, &'static str, Vec<SExpr>),
    RecordLit(&'static str, Vec<(&'static str, SExpr)>),
}

/// Faithful assembly of `expr_to_rust`
/// (src/codegen/mod.rs:1177-1200), with the operator symbol
/// from the slice-1 spec_binop_symbol mirror, the Call
/// dispatch from the rust_expr_call_form mirror (Phase 55:
/// 'List' renders as the vec literal, '__tuple__' as a Rust
/// tuple, 'None' as the bare constructor), the Lambda arm
/// as the rust_expr_lambda_text template, the Phase 78
/// arms (Field as the rust_expr_field_text template,
/// MethodCall through the rust_expr_method_form dispatch
/// into the rust_expr_method_text template or the "??"
/// fallback, RecordLit as the rust_expr_record_text frame
/// over the walker's per-field renderings — a bare string
/// literal taking the owned form, as the native arm
/// applies it), and "??" for every kind without an arm.
fn expr_to_rust_assembled(expr: &SExpr) -> String {
    match expr {
        SExpr::Var(name) => name.to_string(),
        SExpr::IntLit(n) => n.to_string(),
        SExpr::FloatLit(_) => "??".to_string(),
        SExpr::BoolLit(b) => b.to_string(),
        SExpr::StringLit(s) => format!("\"{}\"", s),
        SExpr::Call(name, args) => {
            let rendered: Vec<String> = args.iter().map(expr_to_rust_assembled).collect();
            let joined = rendered.join(", ");
            match rust_expr_call_form(name) {
                1 => rust_expr_list_text(&joined),
                2 => rust_expr_tuple_text(&joined),
                3 => "None".to_string(),
                _ => format!("{}({})", name, joined),
            }
        }
        SExpr::BinOp(op, lhs, rhs) => format!(
            "({} {} {})",
            expr_to_rust_assembled(lhs),
            spec_binop_symbol(*op).unwrap(),
            expr_to_rust_assembled(rhs)
        ),
        SExpr::Lambda(params, body) => {
            rust_expr_lambda_text(&params.join(", "), &expr_to_rust_assembled(body))
        }
        SExpr::Field(base, field) => {
            rust_expr_field_text(&expr_to_rust_assembled(base), field)
        }
        SExpr::MethodCall(receiver, name, args) => {
            // The effective-key fold (the spec's doc): a
            // call carrying arguments classifies under the
            // empty name and falls back.
            let key = if args.is_empty() { *name } else { "" };
            match rust_expr_method_form(key) {
                0 => rust_expr_method_text(&expr_to_rust_assembled(receiver), name),
                _ => "??".to_string(),
            }
        }
        SExpr::RecordLit(name, fields) => {
            let rendered: Vec<String> = fields
                .iter()
                .map(|(n, v)| {
                    let value_text = match v {
                        SExpr::StringLit(s) => format!("\"{}\".to_string()", s),
                        _ => expr_to_rust_assembled(v),
                    };
                    format!("{}: {}", n, value_text)
                })
                .collect();
            rust_expr_record_text(name, &rendered.join(", "))
        }
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

/// Faithful assembly of `generate_rust`'s fixed structure
/// (src/codegen/mod.rs:66-156) from the slice-3 mirrors.
/// The parts arrive as the original holds them at each
/// step: doc as its split lines, the signature already
/// assembled by `rust_signature` from pre-rendered pieces,
/// preconditions as their constraint_to_rust renderings,
/// the body pre-rendered (the walker slices own it), and
/// the test blocks pre-joined. The presence branches are
/// the original's own: header iff a doc exists, block iff
/// preconditions are non-empty (emitted AFTER the
/// signature, at the head of the function body — the
/// Phase 55 amendment moved it inside; through Phase 54
/// it stood before the signature at module level,
/// surprise 13), test module iff test blocks exist.
fn generate_rust_assembled(
    doc_lines: Option<&[&str]>,
    signature: &str,
    precondition_renderings: &[&str],
    body: &str,
    test_blocks: Option<&str>,
) -> String {
    let mut out = String::new();
    if let Some(lines) = doc_lines {
        for line in lines {
            out.push_str(&rust_doc_line(line));
        }
        out.push_str(rust_file_fixed_text(1).unwrap());
    }
    out.push_str(signature);
    if !precondition_renderings.is_empty() {
        let asserts: String = precondition_renderings
            .iter()
            .map(|r| rust_precondition_assert(r))
            .collect();
        out.push_str(&rust_precondition_block(&asserts));
    }
    out.push_str(body);
    out.push_str(rust_file_fixed_text(0).unwrap());
    if let Some(blocks) = test_blocks {
        out.push_str(&rust_test_module(blocks));
    }
    out
}

// ═════════════════════════════════════════════════════════════
// A4. MIRRORS — slice 4 expression walker
//
// ir_to_rust_expr (src/codegen/mod.rs:742-971), arm by arm.
// The per-arm mirrors state each template over pre-rendered
// children exactly as the .morph specs pin them; the
// assembly mirror further below performs the recursion the
// specs cannot state, composing the per-arm mirrors exactly
// as the walker composes the real helpers. Mirror-only
// machinery: the ", " joins (iteration), the statement
// rendering inside Block/Loop (slice 5 — those nodes arrive
// pre-rendered here), pattern_has_string_literal's
// recursion over the pattern tree, and
// is_bare_string_literal's recursion through Typed (the
// flat kind code cannot carry it; the spec
// rust_match_scrutinee_bare leaves kind 22 unpinned for
// exactly this reason).
// ═════════════════════════════════════════════════════════════

/// Mirror of the BinOp branch classification of
/// `ir_to_rust_expr` (src/codegen/mod.rs:753-781): 0 =
/// general infix, 1 = Concat, 2 = Append, 3 = Merge.
/// Total over the nineteen IRBinOp codes.
fn rust_expr_binop_form(op: i64) -> Option<i64> {
    match op {
        0..=15 => Some(0),
        16 => Some(1),
        17 => Some(2),
        18 => Some(3),
        _ => None,
    }
}

/// Mirror of the general infix rendering
/// (src/codegen/mod.rs:778), composed over the slice-1
/// symbol table exactly as the walker consults it.
fn rust_binop_infix_text(op: i64, l: &str, r: &str) -> String {
    format!("({} {} {})", l, rust_binop_symbol(op).unwrap(), r)
}

/// Mirror of the Append rendering (src/codegen/mod.rs:762-763).
fn rust_binop_append_text(l: &str, r: &str) -> String {
    format!("{}.push({})", l, r)
}

/// Mirror of the Merge rendering (src/codegen/mod.rs:764-765).
fn rust_binop_merge_text(l: &str, r: &str) -> String {
    format!("{}.extend({})", l, r)
}

/// Mirror of the Concat rendering (src/codegen/mod.rs:766-777)
/// over the two treated operand texts.
fn rust_binop_concat_text(owned: &str, borrowed: &str) -> String {
    format!("({} + {})", owned, borrowed)
}

/// Mirror of the Concat left-operand leaf check
/// (concat_owned_operand, src/codegen/mod.rs):
/// owned exactly for a direct String literal node
/// (Phase 29) or a direct Var node (Phase 39 — a bare
/// variable in left position moves, E0382, so it takes
/// the owned-copy treatment).
fn rust_concat_left_owned(kind: i64, lit: i64) -> bool {
    kind == 10 || (kind == 9 && lit == 3)
}

/// Mirror of the Concat left operand's final text: the
/// owned template when the leaf check fires, else the
/// operand's own rendering.
fn rust_concat_left_text(owned: bool, rendered: &str, owned_text: &str) -> String {
    if owned {
        owned_text.to_string()
    } else {
        rendered.to_string()
    }
}

/// Mirror of the Concat right-operand classification
/// (concat_borrowed_operand, src/codegen/mod.rs:640-651):
/// 0 = bare (a String literal), 1 = parenthesized borrow
/// (the block-like kinds — the rust_condition_wrap set),
/// 2 = plain borrow. Total over 28 kinds x 6 literal kinds.
fn rust_concat_right_form(kind: i64, lit: i64) -> Option<i64> {
    match kind {
        9 if lit == 3 => Some(0),
        1 | 2 | 3 | 4 | 5 | 6 | 13 | 14 | 24 | 26 => Some(1),
        0..=27 => Some(2),
        _ => None,
    }
}

/// Mirror of the Concat right operand's final text, by
/// rust_concat_right_form.
fn rust_concat_right_text(form: i64, rendered: &str) -> Option<String> {
    match form {
        0 => Some(rendered.to_string()),
        1 => Some(format!("&({})", rendered)),
        2 => Some(format!("&{}", rendered)),
        _ => None,
    }
}

/// Mirror of the UnaryOp rendering
/// (src/codegen/mod.rs:789-809) over the operand's own
/// rendering: prefix forms for Neg/Not, method forms for
/// Abs/Len/Reverse/Clone, indexing/slicing lowerings for
/// the list parts (Init and Last emit the operand twice).
fn rust_unary_text(op: i64, inner: &str) -> Option<String> {
    match op {
        0 => Some(format!("(- {})", inner)),
        1 => Some(format!("(! {})", inner)),
        2 => Some(format!("{}.abs()", inner)),
        3 => Some(format!("{}.len()", inner)),
        4 => Some(format!("({})[0]", inner)),
        5 => Some(format!("({})[1..].to_vec()", inner)),
        6 => Some(format!("({})[..({}).len() - 1].to_vec()", inner, inner)),
        7 => Some(format!("({})[({}).len() - 1]", inner, inner)),
        8 => Some(format!("{}.reverse()", inner)),
        9 => Some(format!("{}.clone()", inner)),
        _ => None,
    }
}

/// Mirror of the Collection arm's opening text in
/// expression position (src/codegen/mod.rs:838-852).
fn rust_collection_prefix(kind: i64) -> Option<&'static str> {
    match kind {
        0 => Some("vec!["),
        1 => Some("std::collections::HashSet::from(["),
        2 => Some("std::collections::HashMap::from(["),
        3 => Some("["),
        4 => Some("("),
        _ => None,
    }
}

/// Mirror of the Collection arm's closing text in
/// expression position (src/codegen/mod.rs:838-852).
fn rust_collection_suffix(kind: i64) -> Option<&'static str> {
    match kind {
        0 => Some("]"),
        1 => Some("])"),
        2 => Some("])"),
        3 => Some("]"),
        4 => Some(")"),
        _ => None,
    }
}

/// Mirror of the Map rendering (src/codegen/mod.rs:853-859).
fn rust_map_text(over: &str, func: &str) -> String {
    format!("({}).iter().map({}).collect()", over, func)
}

/// Mirror of the Filter rendering (src/codegen/mod.rs:860-866).
fn rust_filter_text(over: &str, pred: &str) -> String {
    format!("({}).iter().filter({}).collect()", over, pred)
}

/// Mirror of the Fold rendering (src/codegen/mod.rs:867-873).
fn rust_fold_text(over: &str, initial: &str, operation: &str) -> String {
    format!("({}).iter().fold({}, {})", over, initial, operation)
}

/// Mirror of the Sort rendering (src/codegen/mod.rs:874-882).
fn rust_sort_text(coll: &str, cmp: &str) -> String {
    format!(
        "{{ let mut sorted = {}; sorted.sort_by({}); sorted }}",
        coll, cmp
    )
}

/// Mirror of the Call rendering (src/codegen/mod.rs:810-813)
/// over the rendered name and the pre-joined arguments.
/// Mirror of `rust_call_form` (Phase 85): which
/// rendering an IR Call takes, keyed by the EFFECTIVE
/// name — 0 = plain call template, 1 = vec literal
/// ("List"), 2 = bare constructor ("None"), 3 = the
/// "len" builtin (sub-dispatched by
/// rust_call_len_form). Total: every other name is
/// family 0.
fn rust_call_form(name: &str) -> i64 {
    match name {
        "List" => 1,
        "None" => 2,
        "len" => 3,
        _ => 0,
    }
}

/// Mirror of `rust_call_len_form` (Phase 85): the len
/// sub-dispatch over the argument's shape — 1 = string
/// projection / string literal (character count),
/// 0 = every other shape (element count).
fn rust_call_len_form(argform: i64) -> i64 {
    if argform == 1 {
        1
    } else {
        0
    }
}

/// Mirror of `rust_call_len_text` (Phase 85): the
/// element-count len rendering.
fn rust_call_len_text(arg: &str) -> String {
    format!("({}).len() as i64", arg)
}

/// Mirror of `rust_call_len_char_text` (Phase 85):
/// the character-count len rendering.
fn rust_call_len_char_text(arg: &str) -> String {
    format!("({}).chars().count() as i64", arg)
}

/// Mirror of `rust_index_form` (Phase 85): which
/// rendering an IR Index takes, keyed by the
/// collection's kind code and literal kind — 1 = the
/// char-access template (FieldAccess base, kind 28;
/// String literal base, kind 9 / lit 3), 0 = the
/// bracket template.
fn rust_index_form(kind: i64, lit: i64) -> i64 {
    if kind == 28 || (kind == 9 && lit == 3) {
        1
    } else {
        0
    }
}

/// Mirror of `rust_index_char_text` (Phase 85): the
/// char-access rendering over a string base,
/// totalized past the end to NUL.
fn rust_index_char_text(coll: &str, idx: &str) -> String {
    format!(
        "({}).chars().nth(({}) as usize).unwrap_or('\\0')",
        coll, idx
    )
}

/// The assembled Call dispatch (Phase 85): the
/// effective name (the argument-count fold) classifies
/// through rust_call_form, family 3 sub-dispatches
/// through rust_call_len_form over the argument's
/// shape — the native ir_call_text's decision,
/// composed from the mirrors exactly as the composed
/// assembly composes it from the winners.
fn ir_call_dispatched_mirror(name: &str, args: &[Expr]) -> String {
    let eff = match name {
        "None" if !args.is_empty() => "",
        "len" if args.len() != 1 => "",
        other => other,
    };
    match rust_call_form(eff) {
        1 => rust_expr_list_text(
            &args
                .iter()
                .map(ir_expr_assembled)
                .collect::<Vec<_>>()
                .join(", "),
        ),
        2 => rust_ident_verbatim(name),
        3 => {
            let argform = match &args[0] {
                Expr::FieldAccess(..) => 1,
                Expr::Lit(Lit::Str(_)) => 1,
                _ => 0,
            };
            if rust_call_len_form(argform) == 1 {
                rust_call_len_char_text(&ir_expr_assembled(&args[0]))
            } else {
                rust_call_len_text(&ir_expr_assembled(&args[0]))
            }
        }
        _ => rust_call_text(
            &rust_ident_verbatim(name),
            &args
                .iter()
                .map(ir_expr_assembled)
                .collect::<Vec<_>>()
                .join(", "),
        ),
    }
}

fn rust_call_text(func: &str, args: &str) -> String {
    format!("{}({})", func, args)
}

/// Mirror of `expr_in_condition_position`
/// (src/codegen/mod.rs:734-740) as specced in slice 4: the
/// same assembly the slice-2 `condition_text` states,
/// restated for the walker's own spec.
fn rust_condition_text(kind: i64, rendered: &str) -> String {
    match rust_condition_wrap(kind) {
        Some(true) => format!("({})", rendered),
        _ => rendered.to_string(),
    }
}

/// Mirror of the If rendering (src/codegen/mod.rs:814-822).
fn rust_if_text(cond: &str, then_text: &str, else_text: &str) -> String {
    format!("if {} {{ {} }} else {{ {} }}", cond, then_text, else_text)
}

/// Mirror of the Lambda/Closure rendering
/// (src/codegen/mod.rs:823-832) over the pre-joined
/// parameter names.
fn rust_lambda_text(params: &str, body: &str) -> String {
    format!("|{}| {{ {} }}", params, body)
}

/// Mirror of one Match arm (src/codegen/mod.rs:883-890).
fn rust_match_arm_text(pat: &str, body: &str) -> String {
    format!("{} => {}", pat, body)
}

/// Mirror of the Match rendering (src/codegen/mod.rs:883-898)
/// over the treated scrutinee and the pre-joined arms.
fn rust_match_text(scrutinee: &str, arms: &str) -> String {
    format!("match {} {{ {} }}", scrutinee, arms)
}

/// Mirror of the bare-literal scrutinee check over the
/// flat codes (is_bare_string_literal,
/// src/codegen/mod.rs:708-714): None for kind 22 (Typed),
/// whose bareness is its inner node's — the recursion the
/// codes cannot carry, which the spec leaves unpinned.
fn rust_match_scrutinee_bare(kind: i64, lit: i64) -> Option<bool> {
    match kind {
        22 => None,
        9 => Some(lit == 3),
        0..=27 => Some(false),
        _ => None,
    }
}

/// Mirror of the scrutinee treatment decision
/// (match_scrutinee_to_rust, src/codegen/mod.rs:683-695):
/// 1 exactly when a string-literal pattern is present and
/// the scrutinee is not itself a bare String literal.
fn rust_match_scrutinee_form(has_string_pat: bool, bare: bool) -> i64 {
    if has_string_pat && !bare {
        1
    } else {
        0
    }
}

/// Mirror of the scrutinee's final text: the
/// condition-position rendering, with ".as_str()"
/// appended exactly when the form is 1.
fn rust_match_scrutinee_text(form: i64, rendered: &str) -> Option<String> {
    match form {
        0 => Some(rendered.to_string()),
        1 => Some(format!("{}.as_str()", rendered)),
        _ => None,
    }
}

/// Mirror of the block-expression wrap shared by the
/// Block arm (src/codegen/mod.rs:900-913) and
/// expr_block_stmt (src/codegen/mod.rs:716-719).
fn rust_block_text(inner: &str) -> String {
    format!("{{ {} }}", inner)
}

/// Mirror of the Loop rendering (src/codegen/mod.rs:917-922).
fn rust_loop_text(body: &str) -> String {
    format!("loop {{\n{}}}", body)
}

/// Mirror of the StructDef rendering
/// (src/codegen/mod.rs:923-935) over the rendered name, the
/// pre-joined fields and the rendered body.
fn rust_struct_def_text(name: &str, fields: &str, body: &str) -> String {
    format!("{{ struct {} {{ {} }} {} }}", name, fields, body)
}

/// Mirror of the Index rendering (src/codegen/mod.rs:936-938).
fn rust_index_text(coll: &str, idx: &str) -> String {
    format!("({})[{}]", coll, idx)
}

/// Mirror of the FieldAccess rendering (Phase 78):
/// the projection over the condition-rendered base
/// and the field name.
fn rust_field_access_text(base: &str, field: &str) -> String {
    format!("{}.{}", base, field)
}

/// Mirror of the RecordLit rendering (Phase 78): the
/// struct-literal frame over the type name and the
/// pre-rendered, pre-joined fields.
fn rust_record_lit_text(name: &str, fields: &str) -> String {
    format!("{} {{ {} }}", name, fields)
}

/// Mirror of the TryPropagate rendering
/// (src/codegen/mod.rs:939-941).
fn rust_try_text(inner: &str) -> String {
    format!("({})?", inner)
}

/// Mirror of the Assign rendering (src/codegen/mod.rs:942-948).
fn rust_assign_text(target: &str, value: &str) -> String {
    format!("({} = {})", target, value)
}

/// Mirror of the Return rendering (src/codegen/mod.rs:949-954).
fn rust_return_text(inner: &str) -> String {
    format!("(return {})", inner)
}

/// Mirror of the Break rendering (src/codegen/mod.rs:955-958).
fn rust_break_text(has_value: bool, value: &str) -> String {
    if has_value {
        format!("(break {})", value)
    } else {
        "(break)".to_string()
    }
}

/// Mirror of the Hole arm's fixed text
/// (src/codegen/mod.rs:959).
fn rust_hole_text(kind: i64) -> Option<String> {
    match kind {
        0 => Some("panic!(\"Hole not filled\")".to_string()),
        _ => None,
    }
}

/// Mirror of the loud fallback (unsupported_expr,
/// src/codegen/mod.rs:725-727).
fn rust_unsupported_text(shape: &str) -> String {
    format!(
        "panic!(\"unsupported IR in expression position: {}\")",
        shape
    )
}

// ── Slice 5 mirrors: the statement walker ──
// ── (ir_to_rust + ir_to_rust_tail, src/codegen/mod.rs:310-541) ──

/// Mirror of the dispatch of `ir_to_rust`
/// (src/codegen/mod.rs:310-498): arm codes in the
/// original's source order — 0 Hole, 1 Block, 2 Let,
/// 3 If, 4 While, 5 For, 6 Match, 7 Call, 8 Return,
/// 9 Alloc, 10 Assign, 11 Collection, 12 Map, 13 Filter,
/// 14 Fold, 15 Sort, 16 Typed, 17 the expression fallback.
/// Total over the 30 kinds (the fallback arm catches the
/// rest, exactly the original's `_`); None off-domain.
fn rust_stmt_form(kind: i64) -> Option<i64> {
    match kind {
        0 => Some(0),   // Hole
        1 => Some(1),   // Block
        2 => Some(2),   // Let
        3 => Some(3),   // If
        4 => Some(4),   // While
        5 => Some(5),   // For
        6 => Some(6),   // Match
        7 => Some(7),   // Call
        8 => Some(8),   // Return
        15 => Some(11), // Collection
        16 => Some(12), // Map
        17 => Some(13), // Filter
        18 => Some(14), // Fold
        19 => Some(15), // Sort
        20 => Some(9),  // Alloc
        21 => Some(10), // Assign
        22 => Some(16), // Typed
        0..=29 => Some(17), // Literal, Var, BinOp, UnaryOp, Lambda,
                        // Closure, Index, Loop, Break, StructDef,
                        // TryPropagate, FieldAccess, RecordLit
                        // — the fallback
        _ => None,
    }
}

/// Mirror of the dispatch of `ir_to_rust_tail`
/// (src/codegen/mod.rs:507-541): 0 = statement
/// fallthrough, 1 = bare expression, 2 = Block split,
/// 3 = Typed re-entry. Total over the 30 kinds; None
/// off-domain.
fn rust_tail_form(kind: i64) -> Option<i64> {
    match kind {
        1 => Some(2),  // Block
        22 => Some(3), // Typed
        7 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 23 | 27 | 28 | 29 => {
            Some(1) // Call, Literal, Var, BinOp, UnaryOp, Lambda,
            // Closure, Collection, Map, Filter, Fold, Sort,
            // Index, TryPropagate, FieldAccess, RecordLit
            // (Phase 78: the spec rows for the Phase 77
            // native value list)
        }
        0..=27 => Some(0), // Hole, Let, If, While, For, Match,
                           // Return, Alloc, Assign, Loop, Break,
                           // StructDef
        _ => None,
    }
}

/// Mirror of the statement-position collection prefix
/// (the Collection arm of `ir_to_rust`,
/// src/codegen/mod.rs:418-451): Set and Map collapse to
/// the List form.
fn rust_stmt_collection_prefix(kind: i64) -> Option<&'static str> {
    match kind {
        0 => Some("vec!["), // List
        1 => Some("vec!["), // Set — collapsed
        2 => Some("vec!["), // Map — collapsed
        3 => Some("["),     // Array
        4 => Some("("),     // Tuple
        _ => None,
    }
}

/// Mirror of the statement-position collection suffix
/// (src/codegen/mod.rs:418-451): the collapse is
/// complete — Set/Map lose the `])` of expression
/// position too.
fn rust_stmt_collection_suffix(kind: i64) -> Option<&'static str> {
    match kind {
        0 => Some("]"), // List
        1 => Some("]"), // Set — collapsed
        2 => Some("]"), // Map — collapsed
        3 => Some("]"), // Array
        4 => Some(")"), // Tuple
        _ => None,
    }
}

/// Mirror of the expectation provenance of the two
/// statement walkers: 0 = Some(fn_ret), 1 = Some(the
/// node's own declared type), 2 = inherited unchanged.
/// Sites: 0 Let body, 1 If then, 2 If else, 3 Match arm
/// body, 4 Return expression, 5 Alloc initial, 6 tail
/// Typed re-entry, 7 tail Block last statement.
fn rust_expectation_source(site: i64) -> Option<i64> {
    match site {
        0..=4 => Some(0),
        5..=6 => Some(1),
        7 => Some(2),
        _ => None,
    }
}

/// Mirror of the relative indent of each placement
/// site in the statement walkers (src/codegen/mod.rs:
/// 310-541). Sites: 0 Block statement, 1 Let tail body,
/// 2 If branch, 3 While body, 4 For body, 5 Match arm
/// header line, 6 Match arm body, 7 tail Block non-last,
/// 8 tail Block last.
fn rust_stmt_child_indent(site: i64) -> Option<i64> {
    match site {
        0 | 1 | 7 | 8 => Some(0),
        2..=5 => Some(1),
        6 => Some(2),
        _ => None,
    }
}

/// Mirror of the per-statement renderer choice for a
/// Block (ir_to_rust's Block arm; ir_to_rust_tail's
/// Block arm): 0 = statement renderer, 1 = tail
/// renderer. Only the tail position's last statement
/// takes the tail renderer.
fn rust_stmt_seq_form(position: i64, last: bool) -> Option<i64> {
    match (position, last) {
        (1, true) => Some(1),
        (0, _) | (1, false) => Some(0),
        _ => None,
    }
}

/// Mirror of the Hole arm of `ir_to_rust`
/// (src/codegen/mod.rs:316-319): the TODO comment line
/// and the panic line, both at pad.
fn rust_stmt_hole_text(pad: &str) -> String {
    format!(
        "{}// TODO: synthesis incomplete\n{}panic!(\"Hole not filled\");\n",
        pad, pad
    )
}

/// Mirror of the Let arm of `ir_to_rust`
/// (src/codegen/mod.rs:328-337).
fn rust_stmt_let_text(pad: &str, name: &str, value: &str, body: &str) -> String {
    format!("{}let {} = {};\n{}", pad, name, value, body)
}

/// Mirror of the If arm of `ir_to_rust`
/// (src/codegen/mod.rs:339-349): the closing braces
/// carry NO pad — the original's format string
/// interpolates none.
fn rust_stmt_if_text(pad: &str, cond: &str, then_text: &str, else_text: &str) -> String {
    format!(
        "{}if {} {{\n{}}} else {{\n{}}}\n",
        pad, cond, then_text, else_text
    )
}

/// Mirror of the While arm of `ir_to_rust`
/// (src/codegen/mod.rs:352-362): the invariant comment
/// line precedes the loop when present.
fn rust_stmt_while_text(
    has_inv: bool,
    inv: &str,
    pad: &str,
    cond: &str,
    body: &str,
) -> String {
    let inv_comment = if has_inv {
        format!("{}// invariant: {}\n", pad, inv)
    } else {
        String::new()
    };
    format!(
        "{}{}while {} {{\n{}{}}}\n",
        inv_comment, pad, cond, body, pad
    )
}

/// Mirror of the For arm of `ir_to_rust`
/// (src/codegen/mod.rs:364-373).
fn rust_stmt_for_text(pad: &str, var: &str, start: &str, end: &str, body: &str) -> String {
    format!(
        "{}for {} in {}..{} {{\n{}{}}}\n",
        pad, var, start, end, body, pad
    )
}

/// Mirror of the Match arm's head line in `ir_to_rust`
/// (src/codegen/mod.rs:376).
fn rust_stmt_match_head_text(pad: &str, scrutinee: &str) -> String {
    format!("{}match {} {{\n", pad, scrutinee)
}

/// Mirror of one case line-group of the Match arm of
/// `ir_to_rust` (src/codegen/mod.rs:378-384).
fn rust_stmt_match_arm_text(pad_inner: &str, pat: &str, body: &str) -> String {
    format!("{}{} => {{\n{}{}}},\n", pad_inner, pat, body, pad_inner)
}

/// Mirror of the Match arm's closing line in
/// `ir_to_rust` (src/codegen/mod.rs:386).
fn rust_stmt_match_close_text(pad: &str) -> String {
    format!("{}}}\n", pad)
}

/// Mirror of the Call arm of `ir_to_rust`
/// (src/codegen/mod.rs:389-398): the call terminated.
fn rust_stmt_call_text(pad: &str, func: &str, args: &str) -> String {
    format!("{}{}({});\n", pad, func, args)
}

/// Mirror of the Return arm of `ir_to_rust`
/// (src/codegen/mod.rs:400-402): the operand arrives
/// typed-rendered (the assembly supplies it).
fn rust_stmt_return_text(pad: &str, rendered: &str) -> String {
    format!("{}return {};\n", pad, rendered)
}

/// Mirror of the Alloc arm of `ir_to_rust`
/// (src/codegen/mod.rs:404-409).
fn rust_stmt_alloc_text(pad: &str, name: &str, ty: &str, initial: &str) -> String {
    format!("{}let mut {}: {} = {};\n", pad, name, ty, initial)
}

/// Mirror of the Assign arm of `ir_to_rust`
/// (src/codegen/mod.rs:411-416).
fn rust_stmt_assign_text(pad: &str, target: &str, value: &str) -> String {
    format!("{}{} = {};\n", pad, target, value)
}

/// Mirror of the Collection arm of `ir_to_rust`
/// (src/codegen/mod.rs:418-451), composed through the
/// statement affix mirrors exactly as the spec composes
/// them by call.
fn rust_stmt_collection_text(kind: i64, pad: &str, elems: &str) -> Option<String> {
    Some(format!(
        "{}{}{}{};\n",
        pad,
        rust_stmt_collection_prefix(kind)?,
        elems,
        rust_stmt_collection_suffix(kind)?
    ))
}

/// Mirror of the Map arm of `ir_to_rust`
/// (src/codegen/mod.rs:453-460).
fn rust_stmt_map_text(pad: &str, over: &str, func: &str) -> String {
    format!("{}({}).iter().map({}).collect();\n", pad, over, func)
}

/// Mirror of the Filter arm of `ir_to_rust`
/// (src/codegen/mod.rs:462-469).
fn rust_stmt_filter_text(pad: &str, over: &str, pred: &str) -> String {
    format!("{}({}).iter().filter({}).collect();\n", pad, over, pred)
}

/// Mirror of the Fold arm of `ir_to_rust`
/// (src/codegen/mod.rs:471-479): text order is
/// (initial, operation), as in the original.
fn rust_stmt_fold_text(pad: &str, over: &str, initial: &str, operation: &str) -> String {
    format!(
        "{}({}).iter().fold({}, {});\n",
        pad, over, initial, operation
    )
}

/// Mirror of the Sort arm of `ir_to_rust`
/// (src/codegen/mod.rs:481-491): only the first line
/// carries the pad — the original's format string has
/// no pad for the sort_by / sorted lines.
fn rust_stmt_sort_text(pad: &str, coll: &str, cmp: &str) -> String {
    format!(
        "{}let mut sorted = {};\nsorted.sort_by({});\nsorted;\n",
        pad, coll, cmp
    )
}

/// Mirror of the fallback arm of `ir_to_rust`
/// (src/codegen/mod.rs:496): the expression rendering
/// terminated.
fn rust_stmt_expr_text(pad: &str, rendered: &str) -> String {
    format!("{}{};\n", pad, rendered)
}

/// Mirror of the bare-expression arm of
/// `ir_to_rust_tail` (src/codegen/mod.rs:529-538):
/// pad + the typed rendering + a newline, NO `;`.
fn rust_tail_expr_text(pad: &str, rendered: &str) -> String {
    format!("{}{}\n", pad, rendered)
}

/// Mirror of the Block arm assembly of
/// `ir_to_rust_tail` (src/codegen/mod.rs:514-524):
/// the non-last statements' renderings concatenated
/// with the last statement's tail rendering.
fn rust_tail_block_text(rest: &str, last: &str) -> String {
    format!("{}{}", rest, last)
}

/// The emitter's char escaping, hand-mirrored (Phase 29):
/// like escape_rust_str_mirror, plus `'` escaped, and `"`
/// NOT escaped (a double quote is a fine char literal).
fn escape_rust_char_mirror(c: char) -> String {
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

/// IRLiteralKindCode of a Lit (slices 1-2's table).
fn lit_kind(lit: &Lit) -> i64 {
    match lit {
        Lit::Int(_) => 0,
        Lit::Float(_) => 1,
        Lit::Bool(_) => 2,
        Lit::Str(_) => 3,
        Lit::Char(_) => 4,
        Lit::Unit => 5,
    }
}

/// The Literal arm of `ir_to_rust_expr`
/// (src/codegen/mod.rs:744-751), composed from the slice
/// 1-3 literal mirrors — with the Phase 29 escaping,
/// unlike the older `ir_literal_text` above (which
/// predates the unfreeze and renders strings raw; it is
/// exercised only on Int/Float/Bool).
fn ir_expr_literal_text(lit: &Lit) -> String {
    match lit {
        Lit::Int(n) => n.to_string(),
        Lit::Float(f) => f.to_string(),
        Lit::Bool(b) => rust_literal_bool(*b).to_string(),
        Lit::Str(s) => rust_string_literal_text(s),
        Lit::Char(c) => format!("'{}'", escape_rust_char_mirror(*c)),
        Lit::Unit => "()".to_string(),
    }
}

/// Mirror of `pattern_has_string_literal`
/// (src/codegen/mod.rs:697-706): the recursion over the
/// pattern tree that the spec representation cannot carry
/// (patterns arrive pre-rendered), so the spec
/// rust_match_scrutinee_form takes the Bool as an input.
fn pattern_has_string_literal_mirror(pat: &Pat) -> bool {
    match pat {
        Pat::Literal(Lit::Str(_)) => true,
        Pat::Constructor(_, subs) => subs.iter().any(pattern_has_string_literal_mirror),
        Pat::Guard(inner, _) => pattern_has_string_literal_mirror(inner),
        _ => false,
    }
}

/// Tree form of IRNode for the assembled `ir_to_rust_expr`
/// mirror. Names arrive raw (the mirror applies
/// rust_ident_verbatim, the full rust_ident). BlockText
/// carries a pre-rendered, end-trimmed statement rendering:
/// it stands for Block and, through expr_block_stmt, for
/// Let/While/For — all block-like, so every classification
/// the walker consults agrees. LoopText likewise carries
/// its pre-rendered body. StructDef field types arrive
/// pre-rendered (ty_to_rust is slices 1-2). There is no
/// Typed variant: the Typed arm delegates to
/// ir_to_rust_expr_typed (slice 3 owns it), and its absence
/// is exactly why is_bare_string_literal's Typed recursion
/// is decided here only for direct literals.
#[allow(dead_code)]
enum Expr {
    Lit(Lit),
    Var(&'static str),
    BinOp(i64, Box<Expr>, Box<Expr>),
    UnaryOp(i64, Box<Expr>),
    Call(&'static str, Vec<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Lambda(Vec<&'static str>, Box<Expr>),
    Collection(i64, Vec<Expr>),
    Map(Box<Expr>, Box<Expr>),
    Filter(Box<Expr>, Box<Expr>),
    Fold(Box<Expr>, Box<Expr>, Box<Expr>),
    Sort(Box<Expr>, Box<Expr>),
    Match(Box<Expr>, Vec<(Pat, Expr)>),
    BlockText(&'static str),
    LoopText(&'static str),
    StructDef(&'static str, Vec<(&'static str, &'static str)>, Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
    FieldAccess(Box<Expr>, &'static str),
    RecordLit(&'static str, Vec<(&'static str, Expr)>),
    Try(Box<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    Return(Box<Expr>),
    Break(Option<Box<Expr>>),
    Hole,
    Alloc,
}

/// IRNodeKindCode of an Expr (declaration order, slices 1-2).
fn expr_kind(e: &Expr) -> i64 {
    match e {
        Expr::Hole => 0,
        Expr::BlockText(_) => 1,
        Expr::If(..) => 3,
        Expr::Match(..) => 6,
        Expr::Call(..) => 7,
        Expr::Return(_) => 8,
        Expr::Lit(_) => 9,
        Expr::Var(_) => 10,
        Expr::BinOp(..) => 11,
        Expr::UnaryOp(..) => 12,
        Expr::Lambda(..) => 13,
        Expr::Collection(..) => 15,
        Expr::Map(..) => 16,
        Expr::Filter(..) => 17,
        Expr::Fold(..) => 18,
        Expr::Sort(..) => 19,
        Expr::Alloc => 20,
        Expr::Assign(..) => 21,
        Expr::Index(..) => 23,
        Expr::FieldAccess(..) => 28,
        Expr::RecordLit(..) => 29,
        Expr::LoopText(_) => 24,
        Expr::Break(_) => 25,
        Expr::StructDef(..) => 26,
        Expr::Try(_) => 27,
    }
}

/// Assembly of `concat_owned_operand`
/// (src/codegen/mod.rs): a direct String literal
/// takes the owned template (the decision
/// rust_concat_left_owned pins); a direct Var takes
/// the clone text (Phase 39: a bare variable in left
/// position moves); every other node renders as usual.
fn concat_owned_mirror(e: &Expr) -> String {
    match e {
        Expr::Lit(Lit::Str(s)) => rust_string_literal_owned_text(s),
        Expr::Var(name) => format!("{}.clone()", rust_ident_verbatim(name)),
        _ => ir_expr_assembled(e),
    }
}

/// Assembly of `concat_borrowed_operand`
/// (src/codegen/mod.rs:640-651), composed through the
/// rust_concat_right_form / rust_concat_right_text
/// mirrors exactly as the emitter decides then renders.
fn concat_borrowed_mirror(e: &Expr) -> String {
    let rendered = ir_expr_assembled(e);
    let (kind, lit) = match e {
        Expr::Lit(l) => (9, lit_kind(l)),
        _ => (expr_kind(e), 0),
    };
    let form = rust_concat_right_form(kind, lit).unwrap();
    rust_concat_right_text(form, &rendered).unwrap()
}

/// Assembly of `is_bare_string_literal`
/// (src/codegen/mod.rs:708-714) over the Expr tree: a
/// direct String literal only — the enum has no Typed
/// variant, so the original's recursion through Typed has
/// no counterpart here (kind 22 is covered at code level
/// by the rust_match_scrutinee_bare golden).
fn is_bare_string_literal_mirror(e: &Expr) -> bool {
    matches!(e, Expr::Lit(Lit::Str(_)))
}

/// Assembly of `match_scrutinee_to_rust`
/// (src/codegen/mod.rs:683-695), composed through the
/// slice-4 scrutinee mirrors.
fn match_scrutinee_mirror(scrutinee: &Expr, cases: &[(Pat, Expr)]) -> String {
    let base = rust_condition_text(expr_kind(scrutinee), &ir_expr_assembled(scrutinee));
    let has_string = cases
        .iter()
        .any(|(p, _)| pattern_has_string_literal_mirror(p));
    let form = rust_match_scrutinee_form(has_string, is_bare_string_literal_mirror(scrutinee));
    rust_match_scrutinee_text(form, &base).unwrap()
}

/// Faithful assembly of `ir_to_rust_expr`
/// (src/codegen/mod.rs:742-971) from the slice 1-4 mirrors:
/// every arm dispatches through its per-arm mirror, and
/// children recurse exactly as the walker recurses. The
/// Literal and Var arms are slices 1-3's (the literal
/// mirrors; rust_ident_verbatim).
fn ir_expr_assembled(e: &Expr) -> String {
    match e {
        Expr::Lit(lit) => ir_expr_literal_text(lit),
        Expr::Var(name) => rust_ident_verbatim(name),
        Expr::BinOp(op, l, r) => match rust_expr_binop_form(*op).unwrap() {
            1 => rust_binop_concat_text(&concat_owned_mirror(l), &concat_borrowed_mirror(r)),
            2 => rust_binop_append_text(&ir_expr_assembled(l), &ir_expr_assembled(r)),
            3 => rust_binop_merge_text(&ir_expr_assembled(l), &ir_expr_assembled(r)),
            _ => rust_binop_infix_text(*op, &ir_expr_assembled(l), &ir_expr_assembled(r)),
        },
        Expr::UnaryOp(op, inner) => rust_unary_text(*op, &ir_expr_assembled(inner)).unwrap(),
        Expr::Call(name, args) => ir_call_dispatched_mirror(name, args),
        Expr::If(c, t, el) => rust_if_text(
            &rust_condition_text(expr_kind(c), &ir_expr_assembled(c)),
            &ir_expr_assembled(t),
            &ir_expr_assembled(el),
        ),
        Expr::Lambda(params, body) => {
            let names: Vec<String> = params
                .iter()
                .map(|p| rust_ident_verbatim(p))
                .collect();
            rust_lambda_text(&names.join(", "), &ir_expr_assembled(body))
        }
        Expr::Collection(kind, elems) => {
            let rendered: Vec<String> = elems.iter().map(ir_expr_assembled).collect();
            format!(
                "{}{}{}",
                rust_collection_prefix(*kind).unwrap(),
                rendered.join(", "),
                rust_collection_suffix(*kind).unwrap()
            )
        }
        Expr::Map(f, over) => rust_map_text(&ir_expr_assembled(over), &ir_expr_assembled(f)),
        Expr::Filter(p, over) => {
            rust_filter_text(&ir_expr_assembled(over), &ir_expr_assembled(p))
        }
        Expr::Fold(op, init, over) => rust_fold_text(
            &ir_expr_assembled(over),
            &ir_expr_assembled(init),
            &ir_expr_assembled(op),
        ),
        Expr::Sort(cmp, coll) => rust_sort_text(&ir_expr_assembled(coll), &ir_expr_assembled(cmp)),
        Expr::Match(scrutinee, cases) => {
            let arms: Vec<String> = cases
                .iter()
                .map(|(p, b)| {
                    rust_match_arm_text(&ir_pattern_assembled(p), &ir_expr_assembled(b))
                })
                .collect();
            rust_match_text(&match_scrutinee_mirror(scrutinee, cases), &arms.join(", "))
        }
        Expr::BlockText(inner) => rust_block_text(inner),
        Expr::LoopText(body) => rust_loop_text(body),
        Expr::StructDef(name, fields, body) => {
            let rendered: Vec<String> = fields
                .iter()
                .map(|(n, t)| rust_param_text(&rust_ident_verbatim(n), t))
                .collect();
            rust_struct_def_text(
                &rust_ident_verbatim(name),
                &rendered.join(", "),
                &ir_expr_assembled(body),
            )
        }
        Expr::Index(coll, idx) => {
            // Phase 85 dispatch (rust_index_form): a
            // string-shaped base takes the char-access
            // template; every other base the bracket
            // template.
            let (kind, lit) = match coll.as_ref() {
                Expr::Lit(l) => (9, lit_kind(l)),
                other => (expr_kind(other), 0),
            };
            if rust_index_form(kind, lit) == 1 {
                rust_index_char_text(&ir_expr_assembled(coll), &ir_expr_assembled(idx))
            } else {
                rust_index_text(&ir_expr_assembled(coll), &ir_expr_assembled(idx))
            }
        }
        Expr::FieldAccess(base, field) => {
            // The native arm: a record-literal base is
            // parenthesized by the walker itself; every
            // other base renders through the condition-
            // position discipline.
            let base_text = match base.as_ref() {
                Expr::RecordLit(..) => format!("({})", ir_expr_assembled(base)),
                _ => condition_mirror(base),
            };
            rust_field_access_text(&base_text, &rust_ident_verbatim(field))
        }
        Expr::RecordLit(name, fields) => {
            let rendered: Vec<String> = fields
                .iter()
                .map(|(n, v)| {
                    let value_text = match v {
                        Expr::Lit(Lit::Str(s)) => rust_string_literal_owned_text(s),
                        // Phase 85: a FieldAccess value
                        // takes the clone form
                        // (rust_unary_text family Clone).
                        Expr::FieldAccess(..) => {
                            rust_unary_text(9, &ir_expr_assembled(v)).unwrap()
                        }
                        _ => ir_expr_assembled(v),
                    };
                    format!("{}: {}", rust_ident_verbatim(n), value_text)
                })
                .collect();
            rust_record_lit_text(&rust_ident_verbatim(name), &rendered.join(", "))
        }
        Expr::Try(inner) => rust_try_text(&ir_expr_assembled(inner)),
        Expr::Assign(t, v) => {
            rust_assign_text(&ir_expr_assembled(t), &ir_expr_assembled(v))
        }
        Expr::Return(inner) => rust_return_text(&ir_expr_assembled(inner)),
        Expr::Break(v) => match v {
            Some(inner) => rust_break_text(true, &ir_expr_assembled(inner)),
            None => rust_break_text(false, ""),
        },
        Expr::Hole => rust_hole_text(0).unwrap(),
        Expr::Alloc => rust_unsupported_text("Alloc"),
    }
}

// ── Slice 5 assembly mirrors: the statement walkers, ──
// ── composed from the pinned pieces exactly as the ──
// ── originals compose them ──

/// Tree form of IRNode for the assembled statement
/// walkers. Expression children reuse the `Expr` tree;
/// names arrive raw (the mirrors apply
/// rust_ident_verbatim, as the expression assembly
/// does). `Expr(e)` stands for every kind the statement
/// walker routes to its fallback arm (rust_stmt_form
/// family 17) — and, in tail position, for the
/// expression kinds whose dispatch rust_tail_form
/// decides from the kind code. `Typed` carries the
/// annotation's TypeCode: the statement walker drops
/// it, the tail walker re-enters with it as the
/// expectation. Alloc's declared type is a `Ty` tree
/// (ty_to_rust is slices 1-2); its TypeCode for the
/// expectation is `ty_code`.
#[allow(dead_code)]
enum Stmt {
    Hole,
    Block(Vec<Stmt>),
    Let(&'static str, Expr, Box<Stmt>),
    If(Expr, Box<Stmt>, Box<Stmt>),
    While(Expr, Box<Stmt>, Option<&'static str>),
    For(&'static str, Expr, Expr, Box<Stmt>),
    Match(Expr, Vec<(Pat, Stmt)>),
    Call(&'static str, Vec<Expr>),
    Return(Expr),
    Alloc(&'static str, Ty, Expr),
    Assign(Expr, Expr),
    Collection(i64, Vec<Expr>),
    Map(Expr, Expr),
    Filter(Expr, Expr),
    Fold(Expr, Expr, Expr),
    Sort(Expr, Expr),
    Typed(Box<Stmt>, i64),
    Expr(Expr),
}

/// IRNodeKindCode of a Stmt (declaration order,
/// slices 1-2).
fn stmt_kind(s: &Stmt) -> i64 {
    match s {
        Stmt::Hole => 0,
        Stmt::Block(_) => 1,
        Stmt::Let(..) => 2,
        Stmt::If(..) => 3,
        Stmt::While(..) => 4,
        Stmt::For(..) => 5,
        Stmt::Match(..) => 6,
        Stmt::Call(..) => 7,
        Stmt::Return(_) => 8,
        Stmt::Collection(..) => 15,
        Stmt::Map(..) => 16,
        Stmt::Filter(..) => 17,
        Stmt::Fold(..) => 18,
        Stmt::Sort(..) => 19,
        Stmt::Alloc(..) => 20,
        Stmt::Assign(..) => 21,
        Stmt::Typed(..) => 22,
        Stmt::Expr(e) => expr_kind(e),
    }
}

/// The TypeCode of a `Ty` tree, for Alloc's
/// expectation: the tag the type was built from.
fn ty_code(ty: &Ty) -> i64 {
    match ty {
        Ty::Leaf(tag) => *tag,
        Ty::Name(_) => 15,
        Ty::Wrap1(tag, ..) => *tag,
        Ty::Wrap2(tag, ..) => *tag,
        Ty::Tuple(_) => 21,
        Ty::Array(..) => 22,
        Ty::Func(..) => 23,
    }
}

/// The statement walker's pad: four spaces per level.
/// The repetition is the mirror-only half of the
/// slice's indentation answer (the specs pin the
/// arithmetic, rust_stmt_child_indent).
fn stmt_pad(indent: usize) -> String {
    "    ".repeat(indent)
}

/// Assembly of `expr_in_condition_position` over the
/// Expr tree (the base both the If/While conditions
/// and the Match scrutinee build on).
fn condition_mirror(e: &Expr) -> String {
    rust_condition_text(expr_kind(e), &ir_expr_assembled(e))
}

/// Assembly of `ir_to_rust_expr_typed` over the Expr
/// tree: the only expectation-driven rewrite is the
/// String-literal ownership one (rust_string_ownership
/// decides, the owned template renders); every other
/// node delegates to the untyped assembly. The Expr
/// tree has no Typed variant — the original's re-entry
/// through a Typed node with its own type is covered
/// at code level by the rust_typed_form /
/// rust_string_ownership golden.
fn ir_expr_typed_mirror(e: &Expr, expected: i64) -> String {
    match e {
        Expr::Lit(Lit::Str(s)) if rust_string_ownership(3, expected) => {
            rust_string_literal_owned_text(s)
        }
        _ => ir_expr_assembled(e),
    }
}

/// Assembly of `match_scrutinee_to_rust` for statement
/// Match cases (the expression assembly's
/// match_scrutinee_mirror over (Pat, Stmt) cases).
fn stmt_match_scrutinee_mirror(scrutinee: &Expr, cases: &[(Pat, Stmt)]) -> String {
    let base = condition_mirror(scrutinee);
    let has_string = cases
        .iter()
        .any(|(p, _)| pattern_has_string_literal_mirror(p));
    let form = rust_match_scrutinee_form(has_string, is_bare_string_literal_mirror(scrutinee));
    rust_match_scrutinee_text(form, &base).unwrap()
}

/// Faithful assembly of `ir_to_rust`
/// (src/codegen/mod.rs:310-498) from the slice 1-5
/// mirrors: every arm dispatches through its per-arm
/// mirror, children recurse exactly as the walker
/// recurses, and `fn_ret` (a TypeCode) is threaded as
/// the expectation source the original threads.
/// Value positions route through `ir_tail_assembled`,
/// exactly as the original routes them through
/// ir_to_rust_tail.
fn ir_stmt_assembled(s: &Stmt, indent: usize, fn_ret: i64) -> String {
    let pad = stmt_pad(indent);
    match s {
        Stmt::Hole => rust_stmt_hole_text(&pad),
        Stmt::Block(stmts) => stmts
            .iter()
            .map(|st| ir_stmt_assembled(st, indent, fn_ret))
            .collect(),
        Stmt::Let(name, value, body) => rust_stmt_let_text(
            &pad,
            &rust_ident_verbatim(name),
            &ir_expr_assembled(value),
            &ir_tail_assembled(body, indent, fn_ret, fn_ret),
        ),
        Stmt::If(c, t, e) => rust_stmt_if_text(
            &pad,
            &condition_mirror(c),
            &ir_tail_assembled(t, indent + 1, fn_ret, fn_ret),
            &ir_tail_assembled(e, indent + 1, fn_ret, fn_ret),
        ),
        Stmt::While(c, body, inv) => rust_stmt_while_text(
            inv.is_some(),
            inv.unwrap_or(""),
            &pad,
            &condition_mirror(c),
            &ir_stmt_assembled(body, indent + 1, fn_ret),
        ),
        Stmt::For(var, start, end, body) => rust_stmt_for_text(
            &pad,
            &rust_ident_verbatim(var),
            &ir_expr_assembled(start),
            &ir_expr_assembled(end),
            &ir_stmt_assembled(body, indent + 1, fn_ret),
        ),
        Stmt::Match(scrutinee, cases) => {
            let mut out =
                rust_stmt_match_head_text(&pad, &stmt_match_scrutinee_mirror(scrutinee, cases));
            let pad_inner = stmt_pad(indent + 1);
            for (pat, body) in cases {
                out.push_str(&rust_stmt_match_arm_text(
                    &pad_inner,
                    &ir_pattern_assembled(pat),
                    &ir_tail_assembled(body, indent + 2, fn_ret, fn_ret),
                ));
            }
            out.push_str(&rust_stmt_match_close_text(&pad));
            out
        }
        Stmt::Call(name, args) => {
            // Phase 85: family 0 of rust_call_form
            // keeps the standing statement template;
            // the dispatched families terminate the
            // expression rendering through
            // rust_stmt_expr_text.
            let eff = match *name {
                "None" if !args.is_empty() => "",
                "len" if args.len() != 1 => "",
                other => other,
            };
            if rust_call_form(eff) == 0 {
                let rendered: Vec<String> = args.iter().map(ir_expr_assembled).collect();
                rust_stmt_call_text(&pad, &rust_ident_verbatim(name), &rendered.join(", "))
            } else {
                rust_stmt_expr_text(&pad, &ir_call_dispatched_mirror(name, args))
            }
        }
        Stmt::Return(e) => rust_stmt_return_text(&pad, &ir_expr_typed_mirror(e, fn_ret)),
        Stmt::Alloc(name, ty, initial) => rust_stmt_alloc_text(
            &pad,
            &rust_ident_verbatim(name),
            &ty_to_rust_assembled(ty),
            &ir_expr_typed_mirror(initial, ty_code(ty)),
        ),
        Stmt::Assign(t, v) => {
            rust_stmt_assign_text(&pad, &ir_expr_assembled(t), &ir_expr_assembled(v))
        }
        Stmt::Collection(kind, elems) => {
            let rendered: Vec<String> = elems.iter().map(ir_expr_assembled).collect();
            rust_stmt_collection_text(*kind, &pad, &rendered.join(", ")).unwrap()
        }
        Stmt::Map(f, over) => {
            rust_stmt_map_text(&pad, &ir_expr_assembled(over), &ir_expr_assembled(f))
        }
        Stmt::Filter(p, over) => {
            rust_stmt_filter_text(&pad, &ir_expr_assembled(over), &ir_expr_assembled(p))
        }
        Stmt::Fold(op, init, over) => rust_stmt_fold_text(
            &pad,
            &ir_expr_assembled(over),
            &ir_expr_assembled(init),
            &ir_expr_assembled(op),
        ),
        Stmt::Sort(cmp, coll) => {
            rust_stmt_sort_text(&pad, &ir_expr_assembled(coll), &ir_expr_assembled(cmp))
        }
        Stmt::Typed(inner, _) => ir_stmt_assembled(inner, indent, fn_ret),
        Stmt::Expr(e) => rust_stmt_expr_text(&pad, &ir_expr_assembled(e)),
    }
}

/// The typed rendering of a tail-position expression
/// node, for `ir_tail_assembled`'s bare-expression
/// arm: the typed walker delegates every composite to
/// the untyped emitter, so the slice-4 expression
/// templates render them (with `ir_expr_typed_mirror`
/// for the nodes the Stmt tree carries as `Expr`).
fn tail_expr_typed_text(s: &Stmt, expected: i64) -> String {
    match s {
        Stmt::Expr(e) => ir_expr_typed_mirror(e, expected),
        Stmt::Call(name, args) => ir_call_dispatched_mirror(name, args),
        Stmt::Collection(kind, elems) => {
            let rendered: Vec<String> = elems.iter().map(ir_expr_assembled).collect();
            format!(
                "{}{}{}",
                rust_collection_prefix(*kind).unwrap(),
                rendered.join(", "),
                rust_collection_suffix(*kind).unwrap()
            )
        }
        Stmt::Map(f, over) => rust_map_text(&ir_expr_assembled(over), &ir_expr_assembled(f)),
        Stmt::Filter(p, over) => {
            rust_filter_text(&ir_expr_assembled(over), &ir_expr_assembled(p))
        }
        Stmt::Fold(op, init, over) => rust_fold_text(
            &ir_expr_assembled(over),
            &ir_expr_assembled(init),
            &ir_expr_assembled(op),
        ),
        Stmt::Sort(cmp, coll) => {
            rust_sort_text(&ir_expr_assembled(coll), &ir_expr_assembled(cmp))
        }
        _ => unreachable!("tail_expr_typed_text on a statement-shaped kind"),
    }
}

/// Faithful assembly of `ir_to_rust_tail`
/// (src/codegen/mod.rs:507-541): the dispatch goes
/// through the rust_tail_form mirror, as the original
/// dispatches on the node kind. `expected` is a
/// TypeCode (26 stands for None, the ExpectedTypeCode
/// convention of slice 3).
fn ir_tail_assembled(s: &Stmt, indent: usize, fn_ret: i64, expected: i64) -> String {
    match rust_tail_form(stmt_kind(s)).unwrap() {
        2 => {
            let Stmt::Block(stmts) = s else {
                unreachable!("tail form 2 is Block only")
            };
            match stmts.split_last() {
                None => String::new(),
                Some((last, rest)) => {
                    let rest_text: String = rest
                        .iter()
                        .map(|st| ir_stmt_assembled(st, indent, fn_ret))
                        .collect();
                    rust_tail_block_text(
                        &rest_text,
                        &ir_tail_assembled(last, indent, fn_ret, expected),
                    )
                }
            }
        }
        3 => {
            let Stmt::Typed(inner, code) = s else {
                unreachable!("tail form 3 is Typed only")
            };
            ir_tail_assembled(inner, indent, fn_ret, *code)
        }
        1 => rust_tail_expr_text(&stmt_pad(indent), &tail_expr_typed_text(s, expected)),
        _ => ir_stmt_assembled(s, indent, fn_ret),
    }
}

// ═════════════════════════════════════════════════════════════
// A7. MIRRORS — slice 6: the driver (generate + generate_rust)
// ═════════════════════════════════════════════════════════════

/// Mirror of `generate`'s dispatch on the target name
/// (src/codegen/mod.rs:12-22): the exact, case-sensitive
/// &str match. Total — every name routes somewhere, the
/// fallback being route 6 (unsupported).
fn rust_target_route(target: &str) -> i64 {
    match target {
        "rust" => 0,
        "c" => 1,
        "wasm" => 2,
        "python" => 3,
        "javascript" | "js" => 4,
        "ir" => 5,
        _ => 6,
    }
}

/// Mirror of the result form of a `generate` route:
/// 0 = Ok (routes 0..=5), 1 = Err (route 6, the
/// UnsupportedTarget error). Route codes outside 0..=6
/// never occur in the original and are unpinned in the
/// spec: None here.
fn rust_target_result_form(route: i64) -> Option<i64> {
    match route {
        0..=5 => Some(0),
        6 => Some(1),
        _ => None,
    }
}

/// Mirror of the Display text of
/// `CodegenError::UnsupportedTarget`
/// (src/codegen/mod.rs:1415): the target name carried
/// verbatim — no sanitization, no case folding.
fn rust_unsupported_target_text(target: &str) -> String {
    format!("Unsupported target language: {}", target)
}

/// Mirror of `generate_rust`'s doc header, assembled
/// (src/codegen/mod.rs:70-76) over the pre-joined
/// `rust_doc_line` outputs: the lines plus the blank
/// line (rust_file_fixed_text piece 1, composed here
/// exactly as the original pushes it inside the doc
/// branch).
fn rust_doc_header_text(lines: &str) -> String {
    format!("{}{}", lines, rust_file_fixed_text(1).unwrap())
}

/// Mirror of the driver's conditional-section rule: the
/// section's assembled text when its presence flag
/// holds, the empty string otherwise. One mirror serves
/// the three call sites (doc header, precondition
/// block, test module), as the original's three branches
/// share the shape.
fn rust_driver_section_text(present: bool, text: &str) -> String {
    if present {
        text.to_string()
    } else {
        String::new()
    }
}

/// Mirror of `generate_rust`'s whole-file assembly
/// (src/codegen/mod.rs:66-155) over pre-rendered
/// sections, in emission order: doc header, signature,
/// precondition block (at the head of the function
/// body — moved there by the Phase 55 amendment;
/// through Phase 54 it stood before the signature),
/// body, the function's closing brace
/// (rust_file_fixed_text piece 0), test module.
fn rust_file_text(
    header: &str,
    pre_block: &str,
    signature: &str,
    body: &str,
    test_module: &str,
) -> String {
    format!(
        "{}{}{}{}{}{}",
        header,
        signature,
        pre_block,
        body,
        rust_file_fixed_text(0).unwrap(),
        test_module
    )
}

/// A generated test at the driver's level: an optional
/// RAW name, and the input / expected spec expressions
/// (`expr_to_rust` renders them — slice 2's SExpr tree).
struct DriverTest {
    name: Option<&'static str>,
    input: SExpr,
    expected: SExpr,
}

/// A function spec at the driver's level of
/// abstraction: everything `generate_rust` reads from a
/// candidate + its spec. Names are RAW — the assembly
/// applies `rust_ident_verbatim` at exactly the points
/// the original applies `rust_ident`.
struct DriverSpec {
    doc: Option<&'static str>,
    name: &'static str,
    generics: Vec<&'static str>,
    params: Vec<(&'static str, Ty)>,
    ret: Ty,
    preconditions: Vec<Cons>,
    body: Stmt,
    tests: Vec<DriverTest>,
}

/// Mirror of the driver's test-call generics rule
/// (src/codegen/mod.rs, `test_call_generics_text`): the
/// turbofish text for one generated test's call — the
/// empty string except the E0283 class (a generic spec
/// with a function-typed parameter, a test passing an
/// empty List literal at a List<generic> position, and
/// no argument pinning the generic: a scalar literal at
/// a bare-generic position, or a scalar literal inside
/// a non-empty List literal at a List<generic> position;
/// a lambda never pins). The concrete type is derived
/// from the spec's own tests (first scalar literal kind,
/// Int when none), repeated per generic. The mirror
/// walks the Ty/SExpr trees it has; payload-less SExpr
/// forms carry no literal and cannot pin — the mirror's
/// usual partial view.
fn test_call_generics_text(spec: &DriverSpec, test: &DriverTest) -> String {
    if spec.generics.is_empty() {
        return String::new();
    }
    if !spec.params.iter().any(|(_, t)| matches!(t, Ty::Func(..))) {
        return String::new();
    }
    let positions: Vec<(&SExpr, &Ty)> = match &test.input {
        SExpr::Call(name, args) if *name == "__tuple__" && args.len() == spec.params.len() => {
            args.iter().zip(spec.params.iter().map(|(_, t)| t)).collect()
        }
        other if spec.params.len() == 1 => vec![(other, &spec.params[0].1)],
        _ => return String::new(),
    };
    let is_generic = |t: &Ty, g: &str| matches!(t, Ty::Name(n) if *n == g);
    let is_list_of = |t: &Ty, g: &str| {
        matches!(t, Ty::Wrap1(16, inner) if is_generic(inner, g))
    };
    let is_scalar = |e: &SExpr| {
        matches!(
            e,
            SExpr::IntLit(_) | SExpr::FloatLit(_) | SExpr::BoolLit(_) | SExpr::StringLit(_)
        )
    };
    let unpinned_empty = spec.generics.iter().any(|&g| {
        let has_empty = positions.iter().any(|(e, t)| {
            is_list_of(t, g) && matches!(e, SExpr::Call(n, a) if *n == "List" && a.is_empty())
        });
        if !has_empty {
            return false;
        }
        let pinned = positions.iter().any(|(e, t)| {
            (is_generic(t, g) && is_scalar(e))
                || (is_list_of(t, g)
                    && matches!(e, SExpr::Call(n, a) if *n == "List" && a.iter().any(|el| is_scalar(el))))
        });
        !pinned
    });
    if !unpinned_empty {
        return String::new();
    }
    let ty = driver_concrete_type(spec);
    let types = vec![ty; spec.generics.len()].join(", ");
    format!("::<{}>", types)
}

/// The concrete type the mirror's annotation derivation
/// pins (mirror of `concrete_type_for_spec` in
/// src/codegen/mod.rs): the first scalar literal kind
/// across the spec's tests, i64 when none.
fn driver_concrete_type(spec: &DriverSpec) -> &'static str {
    for test in &spec.tests {
        if let Some(ty) = sexpr_literal_type(&test.input) {
            return ty;
        }
        if let Some(ty) = sexpr_literal_type(&test.expected) {
            return ty;
        }
    }
    "i64"
}

fn sexpr_literal_type(expr: &SExpr) -> Option<&'static str> {
    match expr {
        SExpr::IntLit(_) => Some("i64"),
        SExpr::FloatLit(_) => Some("f64"),
        SExpr::BoolLit(_) => Some("bool"),
        SExpr::StringLit(_) => Some("String"),
        SExpr::Call(_, args) => args.iter().find_map(sexpr_literal_type),
        SExpr::BinOp(_, l, r) => sexpr_literal_type(l).or_else(|| sexpr_literal_type(r)),
        SExpr::Lambda(_, body) => sexpr_literal_type(body),
        _ => None,
    }
}

/// Faithful assembly of `generate_rust`
/// (src/codegen/mod.rs:66-155) from the slice 1-5
/// mirrors, at the SPEC level: where the slice-3
/// assembly above (`generate_rust_assembled`) composes
/// pre-rendered parts, this one starts from the raw
/// spec — names, type trees, constraint trees, the IR
/// body tree, spec-test trees — and renders every part
/// itself. The sections are composed through the
/// slice-6 mirrors exactly as the original composes its
/// pushes — the header and the test module gated by
/// `rust_driver_section_text`, the precondition block
/// placed AFTER the signature (at the head of the
/// function body) by `rust_file_text`'s
/// order, the body rendered by the tail assembly at
/// indent 1 with the return type's code as both fn_ret
/// and the Some expectation (the driver's own seed —
/// the one expectation site outside
/// `rust_expectation_source`'s table), and no import
/// anywhere at file level.
fn generate_rust_from_spec_assembled(spec: &DriverSpec) -> String {
    // Module header (mod.rs:70-76): presence is the
    // doc's Some-ness; the lines split is str::lines.
    let header = match spec.doc {
        Some(doc) => {
            let lines: Vec<String> = doc.lines().map(rust_doc_line).collect();
            rust_driver_section_text(true, &rust_doc_header_text(&lines.concat()))
        }
        None => rust_driver_section_text(false, ""),
    };

    // Signature parts (mod.rs:79-94): generics and
    // params are rust_ident-rendered names / name-type
    // pairs joined ", " (the joins are the mirror-only
    // iteration).
    let generics = {
        let rendered: Vec<String> = spec
            .generics
            .iter()
            .map(|g| rust_generic_param_text(&rust_ident_verbatim(g)))
            .collect();
        rust_generics_text(&rendered.join(", "), !spec.generics.is_empty())
    };
    let params: Vec<String> = spec
        .params
        .iter()
        .map(|(n, t)| rust_param_text(&rust_ident_verbatim(n), &ty_to_rust_assembled(t)))
        .collect();
    let signature = rust_signature(
        &rust_ident_verbatim(spec.name),
        &generics,
        &params.join(", "),
        &ty_to_rust_assembled(&spec.ret),
    );

    // Precondition block (mod.rs:96-104): presence is
    // the list's non-emptiness.
    let pre_block = if spec.preconditions.is_empty() {
        rust_driver_section_text(false, "")
    } else {
        let asserts: Vec<String> = spec
            .preconditions
            .iter()
            .map(|c| rust_precondition_assert(&constraint_to_rust_assembled(c)))
            .collect();
        rust_driver_section_text(true, &rust_precondition_block(&asserts.concat()))
    };

    // Body (mod.rs:113-127): the tail assembly at
    // indent 1, fn_ret and expectation both the return
    // type's code.
    let body = ir_tail_assembled(&spec.body, 1, ty_code(&spec.ret), ty_code(&spec.ret));

    // Test module (mod.rs:131-152): a test's name is
    // its own (rust_ident-rendered) or the indexed
    // default (rust_test_name_form's choice; the
    // default's text is F2, mirror-only). Input and
    // expected arrive as expr_to_rust renderings placed
    // BARE in call / assert position — the recorded
    // E0308 placement for String-typed positions.
    // A tuple input whose arity equals the parameter
    // count spreads its elements into the call instead
    // (Phase 55; rust_test_case's doc states the rule).
    let test_module = if spec.tests.is_empty() {
        rust_driver_section_text(false, "")
    } else {
        let cases: Vec<String> = spec
            .tests
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let name = match t.name {
                    Some(n) => rust_ident_verbatim(n),
                    None => rust_ident_verbatim(&rust_test_name_default(i)),
                };
                let input_text = match &t.input {
                    SExpr::Call("__tuple__", args)
                        if args.len() == spec.params.len() =>
                    {
                        args.iter()
                            .map(expr_to_rust_assembled)
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                    other => expr_to_rust_assembled(other),
                };
                rust_test_case(
                    &name,
                    &rust_ident_verbatim(spec.name),
                    &test_call_generics_text(spec, t),
                    &input_text,
                    &expr_to_rust_assembled(&t.expected),
                )
            })
            .collect();
        rust_driver_section_text(true, &rust_test_module(&cases.concat()))
    };

    rust_file_text(&header, &pre_block, &signature, &body, &test_module)
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
        // .morph tests: add-symbol, neq-symbol, mod-symbol,
        // lt-symbol, concat-symbol
        assert_eq!(spec_binop_symbol(0), Some("+"));
        assert_eq!(spec_binop_symbol(8), Some("!="));
        assert_eq!(spec_binop_symbol(4), Some("%"));
        assert_eq!(spec_binop_symbol(9), Some("<"));
        // Remaining rows of the table
        assert_eq!(spec_binop_symbol(1), Some("-"));
        assert_eq!(spec_binop_symbol(2), Some("*"));
        assert_eq!(spec_binop_symbol(3), Some("/"));
        assert_eq!(spec_binop_symbol(5), Some("&&"));
        assert_eq!(spec_binop_symbol(6), Some("||"));
        assert_eq!(spec_binop_symbol(7), Some("=="));
        assert_eq!(spec_binop_symbol(10), Some("<="));
        assert_eq!(spec_binop_symbol(11), Some(">"));
        assert_eq!(spec_binop_symbol(12), Some(">="));
        assert_eq!(spec_binop_symbol(13), Some("+"));
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
        // The two operator tables AGREE on the shared
        // operators by name since the Phase 55 amendment
        // (codes differ between the enums; compared here
        // by operator name): Mod is spec-code 4 / IR-code
        // 4, Lt is 9 / 10, Concat is 13 / 16. Through
        // Phase 54 the spec table's "?" rows diverged.
        assert_eq!(spec_binop_symbol(4), Some("%")); // spec Mod
        assert_eq!(rust_binop_symbol(4), Some("%")); // IR Mod
        assert_eq!(spec_binop_symbol(9), Some("<")); // spec Lt
        assert_eq!(rust_binop_symbol(10), Some("<")); // IR Lt
        assert_eq!(spec_binop_symbol(13), Some("+")); // spec Concat
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
        // .morph tests: var/floatlit/call/binop/lambda/
        // comprehension forms, floatlit/unaryop fixed texts
        assert_eq!(rust_expr_form(0), Some(0));
        assert_eq!(rust_expr_form(2), Some(6));
        assert_eq!(rust_expr_form(7), Some(4));
        assert_eq!(rust_expr_form(8), Some(5));
        assert_eq!(rust_expr_form(10), Some(7));
        assert_eq!(rust_expr_form(12), Some(6));
        assert_eq!(rust_expr_form(1), Some(1));
        assert_eq!(rust_expr_form(3), Some(2));
        assert_eq!(rust_expr_form(4), Some(3));
        assert_eq!(rust_expr_fixed_text(2), Some("??"));
        assert_eq!(rust_expr_fixed_text(9), Some("??"));
        // Lambda left the fixed-text table in Phase 55
        assert_eq!(rust_expr_fixed_text(10), None);
        // Phase 78: Field takes the projection template
        // (family 8) and leaves the fixed-text table;
        // MethodCall takes family 9 with its fallback row
        // (13) IN the table; RecordLit takes family 10 and
        // never falls back
        assert_eq!(rust_expr_form(5), Some(8));
        assert_eq!(rust_expr_form(13), Some(9));
        assert_eq!(rust_expr_form(14), Some(10));
        assert_eq!(rust_expr_fixed_text(5), None);
        assert_eq!(rust_expr_fixed_text(13), Some("??"));
        assert_eq!(rust_expr_fixed_text(14), None);
        // The fallback family and the fixed-text table
        // coincide on the pre-Phase-73 kinds except Field,
        // which the Phase 78 amendment moved out of both
        for kind in 0..=12 {
            assert_eq!(
                rust_expr_form(kind) == Some(6),
                rust_expr_fixed_text(kind) == Some("??") && kind != 5,
                "kind {}",
                kind
            );
        }
        // Off-range
        assert_eq!(rust_expr_form(15), None);
        assert_eq!(rust_expr_form(-1), None);
        assert_eq!(rust_expr_fixed_text(15), None);
    }

    #[test]
    fn expr_call_forms_full() {
        // .morph tests: list/tuple/none forms, some/plain/
        // case/empty calls
        assert_eq!(rust_expr_call_form("List"), 1);
        assert_eq!(rust_expr_call_form("__tuple__"), 2);
        assert_eq!(rust_expr_call_form("None"), 3);
        assert_eq!(rust_expr_call_form("Some"), 0);
        assert_eq!(rust_expr_call_form("max"), 0);
        assert_eq!(rust_expr_call_form("list"), 0);
        assert_eq!(rust_expr_call_form(""), 0);
        // The templates over pre-rendered, pre-joined parts
        // (.morph tests: list-text/list-empty, tuple-text/
        // tuple-three; the lambda template's golden)
        assert_eq!(rust_expr_list_text("1, 2"), "vec![1, 2]");
        assert_eq!(rust_expr_list_text(""), "vec![]");
        assert_eq!(rust_expr_tuple_text("1, 2"), "(1, 2)");
        assert_eq!(rust_expr_tuple_text("a, b, c"), "(a, b, c)");
        assert_eq!(rust_expr_lambda_text("a, b", "(a < b)"), "|a, b| (a < b)");
        assert_eq!(rust_expr_lambda_text("x", "x"), "|x| x");
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
        assert_eq!(expr_to_rust_assembled(&SExpr::Comprehension), "??");
        // Call template: joined args, empty args
        assert_eq!(
            expr_to_rust_assembled(&SExpr::Call("f", vec![SExpr::IntLit(1), SExpr::Var("x")])),
            "f(1, x)"
        );
        assert_eq!(expr_to_rust_assembled(&SExpr::Call("g", vec![])), "g()");
        // Binary template with the slice-1 symbol table,
        // including the rows the Phase 55 amendment filled
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
            "(1 % 2)"
        );
        assert_eq!(
            expr_to_rust_assembled(&SExpr::BinOp(
                13,
                Box::new(SExpr::StringLit("a")),
                Box::new(SExpr::StringLit("b"))
            )),
            "(\"a\" + \"b\")"
        );
        // The pseudo-call forms (Phase 55): List renders as
        // the vec literal, __tuple__ as a Rust tuple, None
        // as the bare constructor; Some keeps the plain
        // call template
        assert_eq!(
            expr_to_rust_assembled(&SExpr::Call(
                "List",
                vec![SExpr::IntLit(1), SExpr::IntLit(2)]
            )),
            "vec![1, 2]"
        );
        assert_eq!(expr_to_rust_assembled(&SExpr::Call("List", vec![])), "vec![]");
        assert_eq!(
            expr_to_rust_assembled(&SExpr::Call(
                "__tuple__",
                vec![SExpr::IntLit(1), SExpr::Var("x")]
            )),
            "(1, x)"
        );
        assert_eq!(expr_to_rust_assembled(&SExpr::Call("None", vec![])), "None");
        assert_eq!(
            expr_to_rust_assembled(&SExpr::Call("Some", vec![SExpr::IntLit(1)])),
            "Some(1)"
        );
        // The closure template (Phase 55): sort's comparator
        assert_eq!(
            expr_to_rust_assembled(&SExpr::Lambda(
                vec!["a", "b"],
                Box::new(SExpr::BinOp(
                    9,
                    Box::new(SExpr::Var("a")),
                    Box::new(SExpr::Var("b"))
                ))
            )),
            "|a, b| (a < b)"
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
        // The Phase 78 arms: Field projects, MethodCall
        // renders the closed table verbatim (and falls
        // back off it), RecordLit builds the struct
        // literal with the owned-string rule
        assert_eq!(
            expr_to_rust_assembled(&SExpr::Field(Box::new(SExpr::Var("state")), "tokens")),
            "state.tokens"
        );
        assert_eq!(
            expr_to_rust_assembled(&SExpr::MethodCall(
                Box::new(SExpr::Field(Box::new(SExpr::Var("state")), "tokens")),
                "len",
                vec![]
            )),
            "state.tokens.len()"
        );
        assert_eq!(
            expr_to_rust_assembled(&SExpr::MethodCall(
                Box::new(SExpr::MethodCall(
                    Box::new(SExpr::Var("output")),
                    "unwrap",
                    vec![]
                )),
                "len",
                vec![]
            )),
            "output.unwrap().len()"
        );
        assert_eq!(
            expr_to_rust_assembled(&SExpr::MethodCall(
                Box::new(SExpr::Var("output")),
                "is_err",
                vec![]
            )),
            "output.is_err()"
        );
        assert_eq!(
            expr_to_rust_assembled(&SExpr::MethodCall(
                Box::new(SExpr::Var("output")),
                "bogus",
                vec![]
            )),
            "??"
        );
        assert_eq!(
            expr_to_rust_assembled(&SExpr::MethodCall(
                Box::new(SExpr::Var("output")),
                "is_ok",
                vec![SExpr::IntLit(1)]
            )),
            "??"
        );
        assert_eq!(
            expr_to_rust_assembled(&SExpr::RecordLit(
                "Token",
                vec![
                    ("kind", SExpr::IntLit(0)),
                    ("lexeme", SExpr::StringLit("spec")),
                    ("line", SExpr::IntLit(1)),
                ]
            )),
            "Token { kind: 0, lexeme: \"spec\".to_string(), line: 1 }"
        );
        assert_eq!(
            expr_to_rust_assembled(&SExpr::RecordLit("Empty", vec![])),
            "Empty {  }"
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

    // ── Slice 3: name/text assembly ──

    #[test]
    fn ident_sanitize_step() {
        // Mirror-only coverage (finding F5): the per-char
        // step is a loop no spec can state; the expected
        // values are the original's behavior, char for char.
        // Kept chars: letters, digits, underscore
        assert_eq!(rust_ident_sanitize("sort"), "sort");
        assert_eq!(rust_ident_sanitize("_x"), "_x");
        assert_eq!(rust_ident_sanitize("a1_b"), "a1_b");
        // Every other char becomes '_'
        assert_eq!(rust_ident_sanitize("empty-list"), "empty_list");
        assert_eq!(rust_ident_sanitize("a b"), "a_b");
        assert_eq!(rust_ident_sanitize("a.b"), "a_b");
        assert_eq!(rust_ident_sanitize("---"), "___");
        // A leading ASCII digit gets a '_' prefix
        assert_eq!(rust_ident_sanitize("9lives"), "_9lives");
        assert_eq!(rust_ident_sanitize("9"), "_9");
        // The empty name becomes "_"
        assert_eq!(rust_ident_sanitize(""), "_");
        // The kept set is char::is_alphanumeric — UNICODE —
        // despite the original's doc comment claiming
        // [A-Za-z0-9_]: accented letters pass through, and
        // a leading Unicode digit is kept with NO prefix
        // (the prefix check is is_ascii_digit only)
        assert_eq!(rust_ident_sanitize("café"), "café");
        assert_eq!(rust_ident_sanitize("٣x"), "٣x");
    }

    #[test]
    fn ident_suffix_and_composition() {
        // .morph tests: type/Self/try suffix; sort, type_
        // and _ pass through
        assert_eq!(rust_ident_suffix("type"), "type_");
        assert_eq!(rust_ident_suffix("Self"), "Self_");
        assert_eq!(rust_ident_suffix("try"), "try_");
        assert_eq!(rust_ident_suffix("sort"), "sort");
        assert_eq!(rust_ident_suffix("type_"), "type_");
        assert_eq!(rust_ident_suffix("_"), "_");
        // More of the keyword set: lowercase self, fn, loop
        assert_eq!(rust_ident_suffix("self"), "self_");
        assert_eq!(rust_ident_suffix("fn"), "fn_");
        assert_eq!(rust_ident_suffix("loop"), "loop_");
        // The decomposition is exact: the verbatim mirror
        // equals suffix-after-sanitize on every name,
        // including names whose chars are replaced
        let names = [
            "type", "Self", "fn", "sort", "empty-list", "9lives", "", "---",
            "type ", "ma tch", "café", "a.b", "_x", "while", "loop",
        ];
        for n in names {
            assert_eq!(
                rust_ident_verbatim(n),
                rust_ident_suffix(&rust_ident_sanitize(n)),
                "composition mismatch for {:?}",
                n
            );
        }
        // Full-function goldens through the verbatim mirror
        assert_eq!(rust_ident_verbatim("type"), "type_");
        assert_eq!(rust_ident_verbatim("empty-list"), "empty_list");
        assert_eq!(rust_ident_verbatim("9lives"), "_9lives");
        assert_eq!(rust_ident_verbatim(""), "_");
        assert_eq!(rust_ident_verbatim("while"), "while_");
        // A trailing space sanitizes to '_', so the result
        // is NOT suffixed a second time
        assert_eq!(rust_ident_verbatim("type "), "type_");
    }

    #[test]
    fn ident_empty_spec_mirror() {
        // The one case the spec pins
        assert_eq!(rust_ident_empty(""), Some("_".to_string()));
        // Every non-empty name is unpinned by the spec
        assert_eq!(rust_ident_empty("x"), None);
        assert_eq!(rust_ident_empty("type"), None);
    }

    #[test]
    fn typed_form_total() {
        // .morph tests: literal-check, typed-reenter,
        // binop/var delegate
        assert_eq!(rust_typed_form(9), Some(1));
        assert_eq!(rust_typed_form(22), Some(2));
        assert_eq!(rust_typed_form(11), Some(0));
        assert_eq!(rust_typed_form(10), Some(0));
        // The full 28-kind domain: exactly Literal routes
        // to the ownership check, exactly Typed re-enters
        for kind in 0..=27 {
            let want = if kind == 9 {
                1
            } else if kind == 22 {
                2
            } else {
                0
            };
            assert_eq!(rust_typed_form(kind), Some(want), "kind {}", kind);
        }
        // Off-range
        assert_eq!(rust_typed_form(28), None);
        assert_eq!(rust_typed_form(-1), None);
    }

    #[test]
    fn string_ownership_grid() {
        // Owned exactly at (String literal, String
        // expectation); expectation 26 is None, the other
        // codes are Some(type)
        for lit in 0..=5 {
            for expected in [0, 1, 2, 3, 4, 5, 13, 25, 26] {
                assert_eq!(
                    rust_string_ownership(lit, expected),
                    lit == 3 && expected == 3,
                    "lit {} expected {}",
                    lit,
                    expected
                );
            }
        }
        assert!(rust_string_ownership(3, 3));
        assert!(!rust_string_ownership(3, 26));
        assert!(!rust_string_ownership(0, 3));
    }

    #[test]
    fn string_literal_templates() {
        // .morph tests: plain, empty, symbol text
        assert_eq!(rust_string_literal_text("Int"), "\"Int\"");
        assert_eq!(rust_string_literal_text(""), "\"\"");
        assert_eq!(rust_string_literal_text("Vec<i64>"), "\"Vec<i64>\"");
        // The contents are ESCAPED (Phase 29): a quote, a
        // backslash and a newline inside the contents take
        // their Rust escape forms — this is where the
        // escaping rule is pinned (the spec equation pins
        // only the wrap, on the identity subdomain)
        assert_eq!(rust_string_literal_text("a\"b"), "\"a\\\"b\"");
        assert_eq!(rust_string_literal_text("a\\b"), "\"a\\\\b\"");
        assert_eq!(
            rust_string_literal_text("line\nbreak"),
            "\"line\\nbreak\""
        );
        // .morph tests: owned forms
        assert_eq!(
            rust_string_literal_owned_text("Int"),
            "\"Int\".to_string()"
        );
        assert_eq!(rust_string_literal_owned_text(""), "\"\".to_string()");
        // The owned form is exactly the bare form +
        // ".to_string()" on every content
        for c in ["", "x", "Vec<i64>", "a\"b"] {
            assert_eq!(
                rust_string_literal_owned_text(c),
                format!("{}.to_string()", rust_string_literal_text(c))
            );
        }
    }

    #[test]
    fn doc_and_fixed_text() {
        // .morph tests: a doc line, an empty doc line
        assert_eq!(rust_doc_line("Sorts a list"), "//! Sorts a list\n");
        assert_eq!(rust_doc_line(""), "//! \n");
        assert_eq!(rust_doc_line("  indented"), "//!   indented\n");
        // .morph tests: both fixed-text pieces
        assert_eq!(rust_file_fixed_text(0), Some("}\n"));
        assert_eq!(rust_file_fixed_text(1), Some("\n"));
        assert_eq!(rust_file_fixed_text(2), None);
        assert_eq!(rust_file_fixed_text(-1), None);
    }

    #[test]
    fn signature_pieces() {
        // Parameters: rendered name, ": ", rendered type
        assert_eq!(rust_param_text("a", "i64"), "a: i64");
        assert_eq!(rust_param_text("list", "Vec<String>"), "list: Vec<String>");
        // Generic parameters (Phase 62): rendered name +
        // the Clone bound every declared generic carries
        assert_eq!(rust_generic_param_text("T"), "T: Clone");
        assert_eq!(rust_generic_param_text("Elem"), "Elem: Clone");
        // Generics: absent is empty, present is bracketed
        assert_eq!(rust_generics_text("", false), "");
        assert_eq!(rust_generics_text("T", true), "<T>");
        assert_eq!(rust_generics_text("T, U", true), "<T, U>");
        // Signature lines over pre-rendered parts
        assert_eq!(
            rust_signature("add", "", "a: i64, b: i64", "i64"),
            "pub fn add(a: i64, b: i64) -> i64 {\n"
        );
        assert_eq!(
            rust_signature("sort", "<T>", "list: Vec<T>", "Vec<T>"),
            "pub fn sort<T>(list: Vec<T>) -> Vec<T> {\n"
        );
        assert_eq!(rust_signature("f", "", "", "()"), "pub fn f() -> () {\n");
    }

    #[test]
    fn precondition_pieces() {
        // .morph tests: a real rendering, the swallowed
        // 'true' rendering
        assert_eq!(
            rust_precondition_assert("x > 0"),
            "    debug_assert!(x > 0, \"Precondition violated\");\n"
        );
        assert_eq!(
            rust_precondition_assert("true"),
            "    debug_assert!(true, \"Precondition violated\");\n"
        );
        // .morph test: the block over one assert
        assert_eq!(
            rust_precondition_block(
                "    debug_assert!(true, \"Precondition violated\");\n"
            ),
            "#[cfg(debug_assertions)]\n{\n    debug_assert!(true, \"Precondition violated\");\n}\n\n"
        );
        // Two asserts concatenate in order, nothing between
        let two = format!(
            "{}{}",
            rust_precondition_assert("a"),
            rust_precondition_assert("b")
        );
        assert_eq!(
            rust_precondition_block(&two),
            "#[cfg(debug_assertions)]\n{\n    debug_assert!(a, \"Precondition violated\");\n    debug_assert!(b, \"Precondition violated\");\n}\n\n"
        );
    }

    #[test]
    fn test_module_pieces() {
        // .morph tests: the name choice both ways
        assert_eq!(rust_test_name_form(true), 0);
        assert_eq!(rust_test_name_form(false), 1);
        // The default's text is mirror-only (finding F2)
        assert_eq!(rust_test_name_default(0), "test_0");
        assert_eq!(rust_test_name_default(12), "test_12");
        // One test block over pre-rendered parts
        assert_eq!(
            rust_test_case("adds", "add", "", "3", "7"),
            "    #[test]\n    fn adds() {\n        let result = add(3);\n        assert_eq!(result, 7);\n    }\n"
        );
        // A String test input sits BARE in call position —
        // the recorded E0308 property of this path
        assert_eq!(
            rust_test_case("names", "f", "", "\"Int\"", "\"Int\""),
            "    #[test]\n    fn names() {\n        let result = f(\"Int\");\n        assert_eq!(result, \"Int\");\n    }\n"
        );
        // The Phase 61 generics text: the turbofish sits
        // between the function name and the argument list
        assert_eq!(
            rust_test_case("empty_list", "sort", "::<i64>", "vec![], |a, b| (a < b)", "vec![]"),
            "    #[test]\n    fn empty_list() {\n        let result = sort::<i64>(vec![], |a, b| (a < b));\n        assert_eq!(result, vec![]);\n    }\n"
        );
        // .morph test: the module wrapper over a marker
        assert_eq!(
            rust_test_module("X"),
            "\n#[cfg(test)]\nmod tests {\n    use super::*;\n\nX}\n"
        );
        // The wrapper over one real block
        let block = rust_test_case("adds", "add", "", "3", "7");
        assert_eq!(
            rust_test_module(&block),
            "\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn adds() {\n        let result = add(3);\n        assert_eq!(result, 7);\n    }\n}\n"
        );
    }

    #[test]
    fn generate_rust_assembly() {
        // Full fixed structure: doc header, signature,
        // precondition block (at the head of the function
        // body — the Phase 55 amendment moved it inside;
        // through Phase 54 it stood BEFORE the signature,
        // surprise 13), body, closing brace, test module
        let sig = rust_signature("add", "", "a: i64, b: i64", "i64");
        let case = rust_test_case("adds", "add", "", "3", "7");
        let out = generate_rust_assembled(
            Some(&["Adds things"]),
            &sig,
            &["a > 0"],
            "    a + b\n",
            Some(&case),
        );
        assert_eq!(
            out,
            "//! Adds things\n\npub fn add(a: i64, b: i64) -> i64 {\n#[cfg(debug_assertions)]\n{\n    debug_assert!(a > 0, \"Precondition violated\");\n}\n\n    a + b\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn adds() {\n        let result = add(3);\n        assert_eq!(result, 7);\n    }\n}\n"
        );
        // Minimal: no doc, no preconditions, no tests —
        // signature, body, closing brace only
        let out = generate_rust_assembled(None, &sig, &[], "    a + b\n", None);
        assert_eq!(out, "pub fn add(a: i64, b: i64) -> i64 {\n    a + b\n}\n");
        // A multi-line doc: every line gets the prefix,
        // then exactly one blank line
        let out = generate_rust_assembled(
            Some(&["one", "two"]),
            &sig,
            &[],
            "    a + b\n",
            None,
        );
        assert!(out.starts_with("//! one\n//! two\n\npub fn add"));
    }

    // ── Slice 4: the expression walker ──

    #[test]
    fn binop_forms_total() {
        // .morph tests: add/shr general, concat, append, merge
        assert_eq!(rust_expr_binop_form(0), Some(0));
        assert_eq!(rust_expr_binop_form(15), Some(0));
        assert_eq!(rust_expr_binop_form(16), Some(1));
        assert_eq!(rust_expr_binop_form(17), Some(2));
        assert_eq!(rust_expr_binop_form(18), Some(3));
        for op in 0..=18 {
            let want = match op {
                16 => 1,
                17 => 2,
                18 => 3,
                _ => 0,
            };
            assert_eq!(rust_expr_binop_form(op), Some(want), "op {}", op);
        }
        assert_eq!(rust_expr_binop_form(19), None);
        assert_eq!(rust_expr_binop_form(-1), None);
    }

    #[test]
    fn binop_texts() {
        // The infix template composes the slice-1 symbol table
        assert_eq!(rust_binop_infix_text(0, "a", "b"), "(a + b)");
        assert_eq!(rust_binop_infix_text(8, "x", "y"), "(x == y)");
        assert_eq!(rust_binop_infix_text(14, "a", "b"), "(a << b)");
        // Append/Merge: the left operand is interpolated bare
        assert_eq!(rust_binop_append_text("items", "x"), "items.push(x)");
        assert_eq!(rust_binop_merge_text("a", "b"), "a.extend(b)");
        // Concat over the two treated operand texts
        assert_eq!(
            rust_binop_concat_text("\"a\".to_string()", "&b"),
            "(\"a\".to_string() + &b)"
        );
    }

    #[test]
    fn concat_left_owned_grid() {
        // Owned exactly at (Literal kind, String literal
        // kind) and at every Var-kind cell (Phase 39)
        for kind in 0..=27 {
            for lit in 0..=5 {
                assert_eq!(
                    rust_concat_left_owned(kind, lit),
                    kind == 10 || (kind == 9 && lit == 3),
                    "kind {} lit {}",
                    kind,
                    lit
                );
            }
        }
    }

    #[test]
    fn concat_right_form_grid() {
        // The full 28 x 6 grid: bare only for a String
        // literal, parenthesized borrow for the block-like
        // kinds, plain borrow everywhere else
        for kind in 0..=27 {
            for lit in 0..=5 {
                let want = if kind == 9 && lit == 3 {
                    0
                } else if matches!(kind, 1 | 2 | 3 | 4 | 5 | 6 | 13 | 14 | 24 | 26) {
                    1
                } else {
                    2
                };
                assert_eq!(
                    rust_concat_right_form(kind, lit),
                    Some(want),
                    "kind {} lit {}",
                    kind,
                    lit
                );
            }
        }
        assert_eq!(rust_concat_right_form(28, 0), None);
        // The three text forms
        assert_eq!(rust_concat_right_text(0, "\"a\""), Some("\"a\"".to_string()));
        assert_eq!(
            rust_concat_right_text(1, "if c { 1 }"),
            Some("&(if c { 1 })".to_string())
        );
        assert_eq!(rust_concat_right_text(2, "x"), Some("&x".to_string()));
        assert_eq!(rust_concat_right_text(3, "x"), None);
    }

    #[test]
    fn unary_texts_total() {
        let cases = [
            "(- xs)",
            "(! xs)",
            "xs.abs()",
            "xs.len()",
            "(xs)[0]",
            "(xs)[1..].to_vec()",
            "(xs)[..(xs).len() - 1].to_vec()",
            "(xs)[(xs).len() - 1]",
            "xs.reverse()",
            "xs.clone()",
        ];
        for (op, want) in cases.iter().enumerate() {
            assert_eq!(
                rust_unary_text(op as i64, "xs"),
                Some(want.to_string()),
                "op {}",
                op
            );
        }
        assert_eq!(rust_unary_text(10, "xs"), None);
        assert_eq!(rust_unary_text(-1, "xs"), None);
    }

    #[test]
    fn collection_affixes_total() {
        // .morph tests: list/set prefixes, tuple suffix
        assert_eq!(rust_collection_prefix(0), Some("vec!["));
        assert_eq!(
            rust_collection_prefix(1),
            Some("std::collections::HashSet::from([")
        );
        assert_eq!(
            rust_collection_prefix(2),
            Some("std::collections::HashMap::from([")
        );
        assert_eq!(rust_collection_prefix(3), Some("["));
        assert_eq!(rust_collection_prefix(4), Some("("));
        assert_eq!(rust_collection_prefix(5), None);
        assert_eq!(rust_collection_suffix(0), Some("]"));
        assert_eq!(rust_collection_suffix(1), Some("])"));
        assert_eq!(rust_collection_suffix(2), Some("])"));
        assert_eq!(rust_collection_suffix(3), Some("]"));
        assert_eq!(rust_collection_suffix(4), Some(")"));
        assert_eq!(rust_collection_suffix(5), None);
    }

    #[test]
    fn iterator_templates() {
        assert_eq!(
            rust_map_text("xs", "|x| { x }"),
            "(xs).iter().map(|x| { x }).collect()"
        );
        assert_eq!(
            rust_filter_text("xs", "|x| { ok }"),
            "(xs).iter().filter(|x| { ok }).collect()"
        );
        assert_eq!(rust_fold_text("xs", "0", "add"), "(xs).iter().fold(0, add)");
        assert_eq!(
            rust_sort_text("xs", "cmp"),
            "{ let mut sorted = xs; sorted.sort_by(cmp); sorted }"
        );
    }

    #[test]
    fn control_templates() {
        assert_eq!(rust_call_text("add", "1, 2"), "add(1, 2)");
        assert_eq!(rust_call_text("f", ""), "f()");
        // Condition position: block-like kinds wrap, others not
        assert_eq!(
            rust_condition_text(6, "match y { _ => 1 }"),
            "(match y { _ => 1 })"
        );
        assert_eq!(rust_condition_text(3, "if a { b }"), "(if a { b })");
        assert_eq!(rust_condition_text(11, "(a && b)"), "(a && b)");
        assert_eq!(rust_condition_text(10, "flag"), "flag");
        assert_eq!(
            rust_if_text("(a && b)", "1", "2"),
            "if (a && b) { 1 } else { 2 }"
        );
        assert_eq!(rust_lambda_text("x, y", "(x + y)"), "|x, y| { (x + y) }");
        assert_eq!(rust_match_arm_text("_", "0"), "_ => 0");
        assert_eq!(rust_match_text("y", "_ => 0"), "match y { _ => 0 }");
        assert_eq!(rust_block_text("let x = 1;"), "{ let x = 1; }");
        assert_eq!(rust_loop_text("tick();"), "loop {\ntick();}");
        assert_eq!(
            rust_struct_def_text("Point", "x: i64, y: i64", "origin"),
            "{ struct Point { x: i64, y: i64 } origin }"
        );
    }

    #[test]
    fn scrutinee_decisions() {
        // Bare exactly for a direct String literal; kind 22
        // (Typed) is unpinned — the recursion the codes
        // cannot carry
        assert_eq!(rust_match_scrutinee_bare(9, 3), Some(true));
        assert_eq!(rust_match_scrutinee_bare(9, 0), Some(false));
        assert_eq!(rust_match_scrutinee_bare(10, 3), Some(false));
        assert_eq!(rust_match_scrutinee_bare(22, 3), None);
        assert_eq!(rust_match_scrutinee_bare(22, 0), None);
        assert_eq!(rust_match_scrutinee_bare(28, 3), None);
        // The .as_str() decision: string pattern present AND
        // scrutinee not itself a bare literal
        assert_eq!(rust_match_scrutinee_form(true, false), 1);
        assert_eq!(rust_match_scrutinee_form(true, true), 0);
        assert_eq!(rust_match_scrutinee_form(false, false), 0);
        assert_eq!(rust_match_scrutinee_form(false, true), 0);
        assert_eq!(rust_match_scrutinee_text(0, "y"), Some("y".to_string()));
        assert_eq!(
            rust_match_scrutinee_text(1, "y"),
            Some("y.as_str()".to_string())
        );
        assert_eq!(rust_match_scrutinee_text(2, "y"), None);
    }

    #[test]
    fn leaf_templates() {
        assert_eq!(rust_index_text("xs", "i"), "(xs)[i]");
        assert_eq!(rust_try_text("load()"), "(load())?");
        assert_eq!(rust_assign_text("x", "5"), "(x = 5)");
        assert_eq!(rust_return_text("5"), "(return 5)");
        assert_eq!(rust_break_text(true, "x"), "(break x)");
        assert_eq!(rust_break_text(false, ""), "(break)");
        // .morph tests: the Hole fixed text, the Alloc fallback
        assert_eq!(
            rust_hole_text(0),
            Some("panic!(\"Hole not filled\")".to_string())
        );
        assert_eq!(rust_hole_text(1), None);
        assert_eq!(
            rust_unsupported_text("Alloc"),
            "panic!(\"unsupported IR in expression position: Alloc\")"
        );
    }

    #[test]
    fn expr_walker_operators() {
        // Two levels deep: BinOp inside Call inside BinOp —
        // each level's own template supplies its parentheses
        let e = Expr::BinOp(
            2,
            Box::new(Expr::Call(
                "add",
                vec![
                    Expr::BinOp(0, Box::new(Expr::Var("a")), Box::new(Expr::Lit(Lit::Int(1)))),
                    Expr::Var("b"),
                ],
            )),
            Box::new(Expr::Lit(Lit::Int(2))),
        );
        assert_eq!(ir_expr_assembled(&e), "(add((a + 1), b) * 2)");
        // Append/Merge through the assembly
        let e = Expr::BinOp(
            17,
            Box::new(Expr::Var("items")),
            Box::new(Expr::Call("next", vec![])),
        );
        assert_eq!(ir_expr_assembled(&e), "items.push(next())");
        let e = Expr::BinOp(18, Box::new(Expr::Var("a")), Box::new(Expr::Var("b")));
        assert_eq!(ir_expr_assembled(&e), "a.extend(b)");
        // Init/Last emit the operand text twice
        let e = Expr::UnaryOp(7, Box::new(Expr::Var("xs")));
        assert_eq!(ir_expr_assembled(&e), "(xs)[(xs).len() - 1]");
        let e = Expr::UnaryOp(6, Box::new(Expr::Var("xs")));
        assert_eq!(ir_expr_assembled(&e), "(xs)[..(xs).len() - 1].to_vec()");
        // A Concat chain: the inner Concat renders owned on
        // the left as-is; the trailing literal stays bare on
        // the right; the leading literal takes .to_string()
        let e = Expr::BinOp(
            16,
            Box::new(Expr::BinOp(
                16,
                Box::new(Expr::Lit(Lit::Str("a"))),
                Box::new(Expr::Var("x")),
            )),
            Box::new(Expr::Lit(Lit::Str("b"))),
        );
        assert_eq!(ir_expr_assembled(&e), "((\"a\".to_string() + &x) + \"b\")");
        // A block-like right operand is borrowed parenthesized
        let e = Expr::BinOp(
            16,
            Box::new(Expr::Var("s")),
            Box::new(Expr::If(
                Box::new(Expr::Var("c")),
                Box::new(Expr::Lit(Lit::Str("y"))),
                Box::new(Expr::Lit(Lit::Str("n"))),
            )),
        );
        assert_eq!(
            ir_expr_assembled(&e),
            "(s.clone() + &(if c { \"y\" } else { \"n\" }))"
        );
        // The owned literal is escaped before .to_string()
        let e = Expr::BinOp(
            16,
            Box::new(Expr::Lit(Lit::Str("a\"b"))),
            Box::new(Expr::Var("x")),
        );
        assert_eq!(ir_expr_assembled(&e), "(\"a\\\"b\".to_string() + &x)");
    }

    #[test]
    fn expr_walker_control() {
        // A Match condition is block-like: wrapped
        let e = Expr::If(
            Box::new(Expr::Match(
                Box::new(Expr::Var("y")),
                vec![(Pat::Wildcard, Expr::Lit(Lit::Int(1)))],
            )),
            Box::new(Expr::Lit(Lit::Int(2))),
            Box::new(Expr::Lit(Lit::Int(3))),
        );
        assert_eq!(
            ir_expr_assembled(&e),
            "if (match y { _ => 1 }) { 2 } else { 3 }"
        );
        // A BinOp condition is NOT re-wrapped: its own
        // template already parenthesized it
        let e = Expr::If(
            Box::new(Expr::BinOp(
                6,
                Box::new(Expr::Var("a")),
                Box::new(Expr::Var("b")),
            )),
            Box::new(Expr::Var("t")),
            Box::new(Expr::Var("e")),
        );
        assert_eq!(ir_expr_assembled(&e), "if (a || b) { t } else { e }");
        // A string-literal pattern over a variable scrutinee:
        // .as_str() is appended
        let e = Expr::Match(
            Box::new(Expr::Var("s")),
            vec![
                (Pat::Literal(Lit::Str("go")), Expr::Lit(Lit::Int(1))),
                (Pat::Wildcard, Expr::Lit(Lit::Int(0))),
            ],
        );
        assert_eq!(
            ir_expr_assembled(&e),
            "match s.as_str() { \"go\" => 1, _ => 0 }"
        );
        // The same pattern NESTED inside a constructor
        // pattern still triggers it (the tree recursion)
        let e = Expr::Match(
            Box::new(Expr::Var("s")),
            vec![(
                Pat::Constructor("Some", vec![Pat::Literal(Lit::Str("x"))]),
                Expr::Lit(Lit::Int(1)),
            )],
        );
        assert_eq!(
            ir_expr_assembled(&e),
            "match s.as_str() { Some(\"x\") => 1 }"
        );
        // A bare literal scrutinee: already a &str, no
        // .as_str() (and &str has no stable as_str — E0658)
        let e = Expr::Match(
            Box::new(Expr::Lit(Lit::Str("go"))),
            vec![(Pat::Literal(Lit::Str("go")), Expr::Lit(Lit::Int(1)))],
        );
        assert_eq!(ir_expr_assembled(&e), "match \"go\" { \"go\" => 1 }");
        // A Match inside a Match arm body (two levels)
        let e = Expr::Match(
            Box::new(Expr::Var("a")),
            vec![(
                Pat::Wildcard,
                Expr::Match(
                    Box::new(Expr::Var("b")),
                    vec![(Pat::Wildcard, Expr::Lit(Lit::Int(0)))],
                ),
            )],
        );
        assert_eq!(
            ir_expr_assembled(&e),
            "match a { _ => match b { _ => 0 } }"
        );
        // A Lambda whose body is a BinOp
        let e = Expr::Lambda(
            vec!["x"],
            Box::new(Expr::BinOp(
                0,
                Box::new(Expr::Var("x")),
                Box::new(Expr::Lit(Lit::Int(1))),
            )),
        );
        assert_eq!(ir_expr_assembled(&e), "|x| { (x + 1) }");
        // A Hole in condition position: the panic! text,
        // unwrapped (Hole is not block-like)
        let e = Expr::If(
            Box::new(Expr::Hole),
            Box::new(Expr::Lit(Lit::Int(1))),
            Box::new(Expr::Lit(Lit::Int(2))),
        );
        assert_eq!(
            ir_expr_assembled(&e),
            "if panic!(\"Hole not filled\") { 1 } else { 2 }"
        );
    }

    #[test]
    fn expr_walker_collections() {
        // Set/Map collections are fully qualified
        let e = Expr::Collection(1, vec![Expr::Lit(Lit::Int(1)), Expr::Lit(Lit::Int(2))]);
        assert_eq!(
            ir_expr_assembled(&e),
            "std::collections::HashSet::from([1, 2])"
        );
        let e = Expr::Collection(
            2,
            vec![Expr::Collection(
                4,
                vec![Expr::Lit(Lit::Str("k")), Expr::Lit(Lit::Int(1))],
            )],
        );
        assert_eq!(
            ir_expr_assembled(&e),
            "std::collections::HashMap::from([(\"k\", 1)])"
        );
        let e = Expr::Collection(
            0,
            vec![Expr::BinOp(
                0,
                Box::new(Expr::Var("a")),
                Box::new(Expr::Lit(Lit::Int(1))),
            )],
        );
        assert_eq!(ir_expr_assembled(&e), "vec![(a + 1)]");
        // The iterator combinators through the assembly
        let e = Expr::Map(
            Box::new(Expr::Lambda(
                vec!["x"],
                Box::new(Expr::BinOp(
                    2,
                    Box::new(Expr::Var("x")),
                    Box::new(Expr::Lit(Lit::Int(2))),
                )),
            )),
            Box::new(Expr::Var("xs")),
        );
        assert_eq!(
            ir_expr_assembled(&e),
            "(xs).iter().map(|x| { (x * 2) }).collect()"
        );
        let e = Expr::Filter(Box::new(Expr::Var("ok")), Box::new(Expr::Var("xs")));
        assert_eq!(ir_expr_assembled(&e), "(xs).iter().filter(ok).collect()");
        let e = Expr::Fold(
            Box::new(Expr::Var("add")),
            Box::new(Expr::Lit(Lit::Int(0))),
            Box::new(Expr::Var("xs")),
        );
        assert_eq!(ir_expr_assembled(&e), "(xs).iter().fold(0, add)");
        let e = Expr::Sort(Box::new(Expr::Var("cmp")), Box::new(Expr::Var("xs")));
        assert_eq!(
            ir_expr_assembled(&e),
            "{ let mut sorted = xs; sorted.sort_by(cmp); sorted }"
        );
        // Two levels: a Map over a Filter result — the inner
        // collect() text is parenthesized by the outer template
        let e = Expr::Map(
            Box::new(Expr::Var("f")),
            Box::new(Expr::Filter(
                Box::new(Expr::Var("p")),
                Box::new(Expr::Var("xs")),
            )),
        );
        assert_eq!(
            ir_expr_assembled(&e),
            "((xs).iter().filter(p).collect()).iter().map(f).collect()"
        );
    }

    #[test]
    fn expr_walker_leaves() {
        // (return 5) + 1: the Return template's parentheses
        // keep the + outside the return
        let e = Expr::BinOp(
            0,
            Box::new(Expr::Return(Box::new(Expr::Lit(Lit::Int(5))))),
            Box::new(Expr::Lit(Lit::Int(1))),
        );
        assert_eq!(ir_expr_assembled(&e), "((return 5) + 1)");
        // The two loud fallbacks
        assert_eq!(
            ir_expr_assembled(&Expr::Alloc),
            "panic!(\"unsupported IR in expression position: Alloc\")"
        );
        assert_eq!(
            ir_expr_assembled(&Expr::Hole),
            "panic!(\"Hole not filled\")"
        );
        // StructDef: item + body in one block expression
        let e = Expr::StructDef(
            "Point",
            vec![("x", "i64"), ("y", "i64")],
            Box::new(Expr::Var("origin")),
        );
        assert_eq!(
            ir_expr_assembled(&e),
            "{ struct Point { x: i64, y: i64 } origin }"
        );
        // Pre-rendered statement forms
        assert_eq!(ir_expr_assembled(&Expr::LoopText("tick();")), "loop {\ntick();}");
        assert_eq!(
            ir_expr_assembled(&Expr::BlockText("let x = 1; x")),
            "{ let x = 1; x }"
        );
        // Index/Try nesting: each template parenthesizes its
        // own operand
        let e = Expr::Try(Box::new(Expr::Index(
            Box::new(Expr::Var("xs")),
            Box::new(Expr::Lit(Lit::Int(0))),
        )));
        assert_eq!(ir_expr_assembled(&e), "((xs)[0])?");
        let e = Expr::Index(
            Box::new(Expr::Call("get", vec![Expr::Var("i")])),
            Box::new(Expr::Lit(Lit::Int(0))),
        );
        assert_eq!(ir_expr_assembled(&e), "(get(i))[0]");
        // Assign with a composite value
        let e = Expr::Assign(
            Box::new(Expr::Var("x")),
            Box::new(Expr::BinOp(
                0,
                Box::new(Expr::Var("x")),
                Box::new(Expr::Lit(Lit::Int(1))),
            )),
        );
        assert_eq!(ir_expr_assembled(&e), "(x = (x + 1))");
        // Break, both forms
        assert_eq!(
            ir_expr_assembled(&Expr::Break(Some(Box::new(Expr::Var("v"))))),
            "(break v)"
        );
        assert_eq!(ir_expr_assembled(&Expr::Break(None)), "(break)");
        // Char literals through the walker: a quote escapes,
        // a double quote does not
        assert_eq!(ir_expr_assembled(&Expr::Lit(Lit::Char('\''))), "'\\''");
        assert_eq!(ir_expr_assembled(&Expr::Lit(Lit::Char('"'))), "'\"'");
    }

    // ── Phase 85 golden: the call/index form dispatch ──

    #[test]
    fn phase85_call_index_form_goldens() {
        // The classifications, complete over their keys.
        assert_eq!(rust_call_form("List"), 1);
        assert_eq!(rust_call_form("None"), 2);
        assert_eq!(rust_call_form("len"), 3);
        assert_eq!(rust_call_form("Some"), 0);
        assert_eq!(rust_call_form("peek"), 0);
        assert_eq!(rust_call_form("Len"), 0);
        assert_eq!(rust_call_form(""), 0);
        assert_eq!(rust_call_len_form(1), 1);
        assert_eq!(rust_call_len_form(0), 0);
        assert_eq!(rust_call_len_form(7), 0);
        assert_eq!(rust_index_form(28, 0), 1);
        assert_eq!(rust_index_form(9, 3), 1);
        assert_eq!(rust_index_form(9, 0), 0);
        assert_eq!(rust_index_form(0, 0), 0);
        assert_eq!(rust_index_form(4, 0), 0);
        // The templates.
        assert_eq!(rust_call_len_text("xs"), "(xs).len() as i64");
        assert_eq!(
            rust_call_len_char_text("state.source"),
            "(state.source).chars().count() as i64"
        );
        assert_eq!(
            rust_index_char_text("state.source", "state.pos"),
            "(state.source).chars().nth((state.pos) as usize).unwrap_or('\\0')"
        );
        // The assembled dispatch over mirror trees.
        let fa = |field: &'static str| {
            Expr::FieldAccess(Box::new(Expr::Var("state")), field)
        };
        assert_eq!(
            ir_expr_assembled(&Expr::Call(
                "List",
                vec![Expr::Lit(Lit::Int(1)), Expr::Lit(Lit::Int(2))],
            )),
            "vec![1, 2]"
        );
        assert_eq!(
            ir_expr_assembled(&Expr::Call("None", vec![])),
            "None"
        );
        assert_eq!(
            ir_expr_assembled(&Expr::Call("len", vec![fa("source")])),
            "(state.source).chars().count() as i64"
        );
        assert_eq!(
            ir_expr_assembled(&Expr::Call("len", vec![Expr::Var("xs")])),
            "(xs).len() as i64"
        );
        assert_eq!(
            ir_expr_assembled(&Expr::Index(Box::new(fa("source")), Box::new(fa("pos")))),
            "(state.source).chars().nth((state.pos) as usize).unwrap_or('\\0')"
        );
        assert_eq!(
            ir_expr_assembled(&Expr::Index(
                Box::new(Expr::Var("xs")),
                Box::new(Expr::Lit(Lit::Int(0))),
            )),
            "(xs)[0]"
        );
        assert_eq!(
            ir_expr_assembled(&Expr::RecordLit(
                "S",
                vec![("source", fa("source"))],
            )),
            "S { source: state.source.clone() }"
        );
    }

    // ── Slice 5 golden: the statement walker ──

    #[test]
    fn stmt_form_total() {
        // The full dispatch of ir_to_rust over the 30 kinds
        // (28/29 = FieldAccess/RecordLit, in the fallback
        // family since Phase 73; named here in Phase 78).
        let expected: [(i64, i64); 30] = [
            (0, 0), (1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6), (7, 7),
            (8, 8), (9, 17), (10, 17), (11, 17), (12, 17), (13, 17),
            (14, 17), (15, 11), (16, 12), (17, 13), (18, 14), (19, 15),
            (20, 9), (21, 10), (22, 16), (23, 17), (24, 17), (25, 17),
            (26, 17), (27, 17), (28, 17), (29, 17),
        ];
        for (kind, arm) in expected {
            assert_eq!(rust_stmt_form(kind), Some(arm), "kind {}", kind);
        }
        assert_eq!(rust_stmt_form(-1), None);
        assert_eq!(rust_stmt_form(30), None);
    }

    #[test]
    fn tail_form_total() {
        // The full dispatch of ir_to_rust_tail over the 30
        // kinds: 0 statement fallthrough, 1 bare expression,
        // 2 Block split, 3 Typed re-entry. Kinds 28/29
        // (FieldAccess/RecordLit) are bare expressions —
        // the Phase 77 native value list, specced in
        // Phase 78.
        let expected: [(i64, i64); 30] = [
            (0, 0), (1, 2), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 1),
            (8, 0), (9, 1), (10, 1), (11, 1), (12, 1), (13, 1), (14, 1),
            (15, 1), (16, 1), (17, 1), (18, 1), (19, 1), (20, 0), (21, 0),
            (22, 3), (23, 1), (24, 0), (25, 0), (26, 0), (27, 1),
            (28, 1), (29, 1),
        ];
        for (kind, form) in expected {
            assert_eq!(rust_tail_form(kind), Some(form), "kind {}", kind);
        }
        assert_eq!(rust_tail_form(-1), None);
        assert_eq!(rust_tail_form(30), None);
    }

    #[test]
    fn expr_new_kind_templates() {
        // The Phase 78 templates and the method dispatch,
        // at the mirrors' own level (.morph side: the
        // multi-input specs carry no in-spec tests; these
        // are their goldens).
        assert_eq!(rust_expr_field_text("state", "tokens"), "state.tokens");
        assert_eq!(rust_expr_field_text("a.b", "c"), "a.b.c");
        assert_eq!(rust_field_access_text("state", "pos"), "state.pos");
        assert_eq!(rust_expr_method_text("output", "is_err"), "output.is_err()");
        assert_eq!(rust_expr_method_text("output.unwrap()", "len"), "output.unwrap().len()");
        assert_eq!(
            rust_expr_record_text("Token", "kind: 0, lexeme: \"spec\".to_string()"),
            "Token { kind: 0, lexeme: \"spec\".to_string() }"
        );
        assert_eq!(rust_expr_record_text("Empty", ""), "Empty {  }");
        assert_eq!(rust_record_lit_text("Token", "kind: 0"), "Token { kind: 0 }");
        assert_eq!(rust_record_lit_text("Empty", ""), "Empty {  }");
        // The method dispatch over effective keys: the
        // closed table is the verbatim form; everything
        // else falls back (case-sensitive, as the native
        // match). The empty key — where the walker folds
        // every call that carries arguments — falls back
        // too, which is what makes the fold exact.
        for name in ["is_ok", "is_err", "unwrap", "len"] {
            assert_eq!(rust_expr_method_form(name), 0, "{}", name);
        }
        assert_eq!(rust_expr_method_form("tokens"), 1);
        assert_eq!(rust_expr_method_form("Len"), 1);
        assert_eq!(rust_expr_method_form(""), 1);
    }

    #[test]
    fn expr_walker_new_kinds() {
        // FieldAccess over a plain base: no wrapping
        let e = Expr::FieldAccess(Box::new(Expr::Var("state")), "pos");
        assert_eq!(ir_expr_assembled(&e), "state.pos");
        // Nested projections compose
        let e = Expr::FieldAccess(
            Box::new(Expr::FieldAccess(Box::new(Expr::Var("a")), "b")),
            "c",
        );
        assert_eq!(ir_expr_assembled(&e), "a.b.c");
        // A record-literal base is parenthesized by the
        // walker itself
        let e = Expr::FieldAccess(
            Box::new(Expr::RecordLit(
                "Token",
                vec![("kind", Expr::Lit(Lit::Int(0)))],
            )),
            "kind",
        );
        assert_eq!(ir_expr_assembled(&e), "(Token { kind: 0 }).kind");
        // A block-like base goes through the condition
        // discipline and wraps
        let e = Expr::FieldAccess(
            Box::new(Expr::If(
                Box::new(Expr::Var("c")),
                Box::new(Expr::Lit(Lit::Int(1))),
                Box::new(Expr::Lit(Lit::Int(2))),
            )),
            "x",
        );
        assert_eq!(ir_expr_assembled(&e), "(if c { 1 } else { 2 }).x");
        // RecordLit: written order, the owned-string rule
        // on a bare string field, nesting, the empty form
        let e = Expr::RecordLit(
            "Token",
            vec![
                ("kind", Expr::Lit(Lit::Int(0))),
                ("lexeme", Expr::Lit(Lit::Str("spec"))),
                ("line", Expr::Lit(Lit::Int(1))),
            ],
        );
        assert_eq!(
            ir_expr_assembled(&e),
            "Token { kind: 0, lexeme: \"spec\".to_string(), line: 1 }"
        );
        let e = Expr::RecordLit("Empty", vec![]);
        assert_eq!(ir_expr_assembled(&e), "Empty {  }");
        let e = Expr::RecordLit(
            "Outer",
            vec![(
                "inner",
                Expr::RecordLit("Inner", vec![("x", Expr::Lit(Lit::Int(1)))]),
            )],
        );
        assert_eq!(ir_expr_assembled(&e), "Outer { inner: Inner { x: 1 } }");
        // Tail position: both kinds yield their value
        // (rust_tail_form family 1 — no semicolon)
        let s = Stmt::Expr(Expr::FieldAccess(Box::new(Expr::Var("state")), "pos"));
        assert_eq!(ir_tail_assembled(&s, 1, 0, 26), "    state.pos\n");
        let s = Stmt::Expr(Expr::RecordLit(
            "Token",
            vec![("kind", Expr::Lit(Lit::Int(0)))],
        ));
        assert_eq!(ir_tail_assembled(&s, 1, 0, 26), "    Token { kind: 0 }\n");
        // Statement position: the fallback arm terminates
        // the expression rendering with ';'
        let s = Stmt::Expr(Expr::FieldAccess(Box::new(Expr::Var("state")), "pos"));
        assert_eq!(ir_stmt_assembled(&s, 1, 0), "    state.pos;\n");
    }

    #[test]
    fn stmt_collection_affixes_total() {
        // Statement position: Set/Map collapse to the List
        // form, prefix AND suffix.
        let prefixes: [&str; 5] = ["vec![", "vec![", "vec![", "[", "("];
        let suffixes: [&str; 5] = ["]", "]", "]", "]", ")"];
        for kind in 0..=4i64 {
            assert_eq!(
                rust_stmt_collection_prefix(kind),
                Some(prefixes[kind as usize]),
                "prefix kind {}",
                kind
            );
            assert_eq!(
                rust_stmt_collection_suffix(kind),
                Some(suffixes[kind as usize]),
                "suffix kind {}",
                kind
            );
        }
        assert_eq!(rust_stmt_collection_prefix(5), None);
        assert_eq!(rust_stmt_collection_suffix(-1), None);
        // The assembled statement text, per kind.
        assert_eq!(
            rust_stmt_collection_text(0, "", "a, b").unwrap(),
            "vec![a, b];\n"
        );
        assert_eq!(
            rust_stmt_collection_text(1, "", "a, b").unwrap(),
            "vec![a, b];\n"
        );
        assert_eq!(
            rust_stmt_collection_text(2, "    ", "a, b").unwrap(),
            "    vec![a, b];\n"
        );
        assert_eq!(
            rust_stmt_collection_text(3, "", "a, b").unwrap(),
            "[a, b];\n"
        );
        assert_eq!(
            rust_stmt_collection_text(4, "", "a, b").unwrap(),
            "(a, b);\n"
        );
        assert_eq!(rust_stmt_collection_text(5, "", "a"), None);
    }

    #[test]
    fn expectation_source_total() {
        let expected: [i64; 8] = [0, 0, 0, 0, 0, 1, 1, 2];
        for site in 0..=7i64 {
            assert_eq!(
                rust_expectation_source(site),
                Some(expected[site as usize]),
                "site {}",
                site
            );
        }
        assert_eq!(rust_expectation_source(8), None);
        assert_eq!(rust_expectation_source(-1), None);
    }

    #[test]
    fn stmt_child_indent_total() {
        let expected: [i64; 9] = [0, 0, 1, 1, 1, 1, 2, 0, 0];
        for site in 0..=8i64 {
            assert_eq!(
                rust_stmt_child_indent(site),
                Some(expected[site as usize]),
                "site {}",
                site
            );
        }
        assert_eq!(rust_stmt_child_indent(9), None);
    }

    #[test]
    fn stmt_seq_form_table() {
        assert_eq!(rust_stmt_seq_form(0, false), Some(0));
        assert_eq!(rust_stmt_seq_form(0, true), Some(0));
        assert_eq!(rust_stmt_seq_form(1, false), Some(0));
        assert_eq!(rust_stmt_seq_form(1, true), Some(1));
        assert_eq!(rust_stmt_seq_form(2, true), None);
        assert_eq!(rust_stmt_seq_form(-1, false), None);
    }

    #[test]
    fn stmt_templates() {
        // Hole: two lines, both padded.
        assert_eq!(
            rust_stmt_hole_text(""),
            "// TODO: synthesis incomplete\npanic!(\"Hole not filled\");\n"
        );
        assert_eq!(
            rust_stmt_hole_text("    "),
            "    // TODO: synthesis incomplete\n    panic!(\"Hole not filled\");\n"
        );
        // Let: header line, then the tail body immediately.
        assert_eq!(
            rust_stmt_let_text("    ", "x", "1", "    x\n"),
            "    let x = 1;\n    x\n"
        );
        // If: the closing braces carry NO pad (pinned quirk).
        assert_eq!(
            rust_stmt_if_text("    ", "c", "        5\n", "        6\n"),
            "    if c {\n        5\n} else {\n        6\n}\n"
        );
        // While, with and without the invariant comment.
        assert_eq!(
            rust_stmt_while_text(false, "", "    ", "c", "        body;\n"),
            "    while c {\n        body;\n    }\n"
        );
        assert_eq!(
            rust_stmt_while_text(true, "x > 0", "    ", "c", "        body;\n"),
            "    // invariant: x > 0\n    while c {\n        body;\n    }\n"
        );
        // For.
        assert_eq!(
            rust_stmt_for_text("    ", "i", "0", "10", "        body;\n"),
            "    for i in 0..10 {\n        body;\n    }\n"
        );
        // Match pieces: head, one arm group, close.
        assert_eq!(rust_stmt_match_head_text("    ", "v"), "    match v {\n");
        assert_eq!(
            rust_stmt_match_arm_text("        ", "Some(x)", "            x\n"),
            "        Some(x) => {\n            x\n        },\n"
        );
        assert_eq!(rust_stmt_match_close_text("    "), "    }\n");
        // Call / Return / Alloc / Assign statements.
        assert_eq!(rust_stmt_call_text("    ", "f", "a, b"), "    f(a, b);\n");
        assert_eq!(
            rust_stmt_return_text("    ", "\"s\".to_string()"),
            "    return \"s\".to_string();\n"
        );
        assert_eq!(
            rust_stmt_alloc_text("    ", "buf", "String", "\"s\".to_string()"),
            "    let mut buf: String = \"s\".to_string();\n"
        );
        assert_eq!(
            rust_stmt_assign_text("    ", "x", "(x + 1)"),
            "    x = (x + 1);\n"
        );
        // Iterator statements: the expression templates,
        // terminated.
        assert_eq!(
            rust_stmt_map_text("    ", "xs", "f"),
            "    (xs).iter().map(f).collect();\n"
        );
        assert_eq!(
            rust_stmt_filter_text("    ", "xs", "p"),
            "    (xs).iter().filter(p).collect();\n"
        );
        assert_eq!(
            rust_stmt_fold_text("    ", "xs", "0", "add"),
            "    (xs).iter().fold(0, add);\n"
        );
        // Sort: only the first line is padded (pinned quirk).
        assert_eq!(
            rust_stmt_sort_text("    ", "xs", "cmp"),
            "    let mut sorted = xs;\nsorted.sort_by(cmp);\nsorted;\n"
        );
        // Fallback: the expression rendering, terminated —
        // even a block form takes the ';'.
        assert_eq!(
            rust_stmt_expr_text("    ", "loop {\n}"),
            "    loop {\n};\n"
        );
        // Tail pieces: bare expression (no ';'), block
        // assembly (plain concatenation).
        assert_eq!(rust_tail_expr_text("    ", "x"), "    x\n");
        assert_eq!(rust_tail_block_text("a;\n", "b\n"), "a;\nb\n");
    }

    #[test]
    fn stmt_walker_sequences() {
        // A multi-statement Block at indent 1 in a
        // String-returning function: the Alloc's initial
        // and the Return's operand both take the owned
        // form through the typed positions.
        let body = Stmt::Block(vec![
            Stmt::Alloc("buf", Ty::Leaf(3), Expr::Lit(Lit::Str("hi"))),
            Stmt::Call("log", vec![Expr::Var("buf")]),
            Stmt::Return(Expr::Lit(Lit::Str("done"))),
        ]);
        assert_eq!(
            ir_stmt_assembled(&body, 1, 3),
            "    let mut buf: String = \"hi\".to_string();\n    log(buf);\n    return \"done\".to_string();\n"
        );
        // A Hole statement inside a Block.
        let body = Stmt::Block(vec![Stmt::Hole]);
        assert_eq!(
            ir_stmt_assembled(&body, 1, 0),
            "    // TODO: synthesis incomplete\n    panic!(\"Hole not filled\");\n"
        );
        // Statement position renders the LAST statement
        // as a statement too (seq position 0): the Call
        // keeps its ';'.
        let body = Stmt::Block(vec![
            Stmt::Call("f", vec![Expr::Lit(Lit::Int(1))]),
            Stmt::Call("g", vec![]),
        ]);
        assert_eq!(ir_stmt_assembled(&body, 0, 0), "f(1);\ng();\n");
    }

    #[test]
    fn stmt_walker_nesting() {
        // Depth >= 2: an If inside a Block at indent 1 —
        // branches render at indent 2, the If's closing
        // braces land at column 0 (the pinned quirk).
        let body = Stmt::Block(vec![Stmt::If(
            Expr::Var("c"),
            Box::new(Stmt::Block(vec![Stmt::Assign(
                Expr::Var("x"),
                Expr::Lit(Lit::Int(1)),
            )])),
            Box::new(Stmt::Block(vec![Stmt::Call("f", vec![])])),
        )]);
        assert_eq!(
            ir_stmt_assembled(&body, 1, 0),
            "    if c {\n        x = 1;\n} else {\n        f()\n}\n"
        );
        // A statement Match at indent 1: arm bodies tail
        // at indent 3, arm closings at indent 2, the
        // match closing at indent 1; a string pattern
        // puts the scrutinee through .as_str().
        let m = Stmt::Match(
            Expr::Var("v"),
            vec![
                (
                    Pat::Literal(Lit::Str("a")),
                    Stmt::Expr(Expr::Lit(Lit::Int(1))),
                ),
                (Pat::Wildcard, Stmt::Call("g", vec![])),
            ],
        );
        assert_eq!(
            ir_stmt_assembled(&m, 1, 0),
            "    match v.as_str() {\n        \"a\" => {\n            1\n        },\n        _ => {\n            g()\n        },\n    }\n"
        );
        // A While with an invariant inside a Block: the
        // comment line at the While's indent, the body
        // one level deeper, the closing at the While's
        // indent (padded — unlike the If).
        let body = Stmt::Block(vec![Stmt::While(
            Expr::Var("c"),
            Box::new(Stmt::Block(vec![Stmt::Assign(
                Expr::Var("x"),
                Expr::BinOp(0, Box::new(Expr::Var("x")), Box::new(Expr::Lit(Lit::Int(1)))),
            )])),
            Some("x > 0"),
        )]);
        assert_eq!(
            ir_stmt_assembled(&body, 1, 0),
            "    // invariant: x > 0\n    while c {\n        x = (x + 1);\n    }\n"
        );
    }

    #[test]
    fn stmt_walker_tail_vs_statement() {
        // The same node, both positions (indent 1,
        // expectation None = code 26 in tail position).
        let list = Stmt::Collection(0, vec![Expr::Lit(Lit::Int(1)), Expr::Lit(Lit::Int(2))]);
        assert_eq!(ir_stmt_assembled(&list, 1, 0), "    vec![1, 2];\n");
        assert_eq!(ir_tail_assembled(&list, 1, 0, 26), "    vec![1, 2]\n");
        // A Set: collapsed as a statement, fully
        // qualified as a tail value.
        let set = Stmt::Collection(1, vec![Expr::Lit(Lit::Int(1))]);
        assert_eq!(ir_stmt_assembled(&set, 1, 0), "    vec![1];\n");
        assert_eq!(
            ir_tail_assembled(&set, 1, 0, 26),
            "    std::collections::HashSet::from([1])\n"
        );
        // A Call: terminated as a statement, bare as a
        // tail.
        let call = Stmt::Call("f", vec![Expr::Lit(Lit::Int(1))]);
        assert_eq!(ir_stmt_assembled(&call, 1, 0), "    f(1);\n");
        assert_eq!(ir_tail_assembled(&call, 1, 0, 26), "    f(1)\n");
        // A Map likewise.
        let map = Stmt::Map(Expr::Var("f"), Expr::Var("xs"));
        assert_eq!(
            ir_stmt_assembled(&map, 1, 0),
            "    (xs).iter().map(f).collect();\n"
        );
        assert_eq!(
            ir_tail_assembled(&map, 1, 0, 26),
            "    (xs).iter().map(f).collect()\n"
        );
        // A Sort: the three-line discarded-value form as
        // a statement, the value-keeping block expression
        // as a tail.
        let sort = Stmt::Sort(Expr::Var("cmp"), Expr::Var("xs"));
        assert_eq!(
            ir_stmt_assembled(&sort, 1, 0),
            "    let mut sorted = xs;\nsorted.sort_by(cmp);\nsorted;\n"
        );
        assert_eq!(
            ir_tail_assembled(&sort, 1, 0, 26),
            "    { let mut sorted = xs; sorted.sort_by(cmp); sorted }\n"
        );
        // A Block in tail position: non-last statements
        // as statements, the last as a bare tail; an
        // empty Block renders nothing.
        let block = Stmt::Block(vec![
            Stmt::Call("f", vec![Expr::Lit(Lit::Int(1))]),
            Stmt::Expr(Expr::Lit(Lit::Int(5))),
        ]);
        assert_eq!(ir_tail_assembled(&block, 1, 0, 26), "    f(1);\n    5\n");
        assert_eq!(
            ir_tail_assembled(&Stmt::Block(vec![]), 1, 0, 26),
            ""
        );
    }

    #[test]
    fn stmt_walker_return_ownership() {
        // The Return arm's typed call: a String literal
        // is owned exactly in a String-returning
        // function (fn_ret = TypeCode 3).
        let ret = Stmt::Return(Expr::Lit(Lit::Str("s")));
        assert_eq!(
            ir_stmt_assembled(&ret, 0, 3),
            "return \"s\".to_string();\n"
        );
        assert_eq!(ir_stmt_assembled(&ret, 0, 0), "return \"s\";\n");
        // A Let's body is a tail with expectation
        // Some(fn_ret): a bare-literal body is owned in
        // a String function, bare otherwise. (The Let's
        // VALUE position is untyped — the literal 1 is
        // beside the point; a String value would stay
        // bare there too.)
        let let_stmt = Stmt::Let(
            "x",
            Expr::Lit(Lit::Int(1)),
            Box::new(Stmt::Expr(Expr::Lit(Lit::Str("s")))),
        );
        assert_eq!(
            ir_stmt_assembled(&let_stmt, 0, 3),
            "let x = 1;\n\"s\".to_string()\n"
        );
        assert_eq!(ir_stmt_assembled(&let_stmt, 0, 0), "let x = 1;\n\"s\"\n");
        // If branches are tails on the same expectation.
        let if_stmt = Stmt::If(
            Expr::Var("c"),
            Box::new(Stmt::Expr(Expr::Lit(Lit::Str("a")))),
            Box::new(Stmt::Expr(Expr::Lit(Lit::Str("b")))),
        );
        assert_eq!(
            ir_stmt_assembled(&if_stmt, 0, 3),
            "if c {\n    \"a\".to_string()\n} else {\n    \"b\".to_string()\n}\n"
        );
        // An Alloc's initial is typed by the Alloc's OWN
        // declared type, not fn_ret.
        let alloc_str = Stmt::Alloc("s", Ty::Leaf(3), Expr::Lit(Lit::Str("v")));
        assert_eq!(
            ir_stmt_assembled(&alloc_str, 0, 0),
            "let mut s: String = \"v\".to_string();\n"
        );
        let alloc_int = Stmt::Alloc("n", Ty::Leaf(0), Expr::Lit(Lit::Str("v")));
        assert_eq!(
            ir_stmt_assembled(&alloc_int, 0, 3),
            "let mut n: i64 = \"v\";\n"
        );
        // A tail Typed node re-enters with its own
        // annotation as the expectation (tail form 3);
        // in statement position the same node is
        // transparent and the literal falls to the
        // untyped fallback.
        let typed = Stmt::Typed(
            Box::new(Stmt::Expr(Expr::Lit(Lit::Str("s")))),
            3,
        );
        assert_eq!(ir_tail_assembled(&typed, 0, 0, 26), "\"s\".to_string()\n");
        assert_eq!(ir_stmt_assembled(&typed, 0, 0), "\"s\";\n");
    }

    // ── Slice 6: the driver ──────────────────────────

    #[test]
    fn target_route_full_table() {
        assert_eq!(rust_target_route("rust"), 0);
        assert_eq!(rust_target_route("c"), 1);
        assert_eq!(rust_target_route("wasm"), 2);
        assert_eq!(rust_target_route("python"), 3);
        assert_eq!(rust_target_route("javascript"), 4);
        assert_eq!(rust_target_route("js"), 4);
        assert_eq!(rust_target_route("ir"), 5);
        // The fallback is an exact match's fallback:
        // case variants, whitespace, near names and the
        // empty name are all unsupported.
        for name in ["java", "Rust", "JS", "rs", "", "rust ", " rust", "c++", "j"] {
            assert_eq!(rust_target_route(name), 6, "{}", name);
        }
    }

    #[test]
    fn target_result_form_table() {
        for route in 0..=5 {
            assert_eq!(rust_target_result_form(route), Some(0));
        }
        assert_eq!(rust_target_result_form(6), Some(1));
        // Off-table route codes never occur: unpinned.
        assert_eq!(rust_target_result_form(-1), None);
        assert_eq!(rust_target_result_form(7), None);
        // The composition generate exposes: every named
        // target is Ok; an unknown name is the
        // UnsupportedTarget error with its verbatim text.
        assert_eq!(rust_target_result_form(rust_target_route("java")), Some(1));
        assert_eq!(
            rust_unsupported_target_text("java"),
            "Unsupported target language: java"
        );
    }

    #[test]
    fn unsupported_target_text_golden() {
        assert_eq!(
            rust_unsupported_target_text("java"),
            "Unsupported target language: java"
        );
        assert_eq!(rust_unsupported_target_text(""), "Unsupported target language: ");
        // Verbatim: spaces and punctuation pass through
        // unsanitized.
        assert_eq!(
            rust_unsupported_target_text("my lang!"),
            "Unsupported target language: my lang!"
        );
    }

    #[test]
    fn doc_header_and_section_golden() {
        assert_eq!(rust_doc_header_text("//! Sorts a list\n"), "//! Sorts a list\n\n");
        assert_eq!(rust_doc_header_text(""), "\n");
        assert_eq!(rust_driver_section_text(true, "X"), "X");
        assert_eq!(rust_driver_section_text(false, "X"), "");
        assert_eq!(rust_driver_section_text(true, ""), "");
        assert_eq!(rust_driver_section_text(false, ""), "");
        // The whole-file equation over marker sections:
        // order is header, signature, precondition block,
        // body, closing brace, test module.
        assert_eq!(rust_file_text("H", "P", "S", "B", "T"), "HSPB}\nT");
        assert_eq!(rust_file_text("", "", "S", "B", ""), "SB}\n");
    }

    #[test]
    fn generate_rust_end_to_end_full() {
        // Doc + generics + preconditions + tests: every
        // section present. The precondition block lands
        // at the head of the function body, after the
        // signature (the Phase 55 placement); the
        // named test's hyphen sanitizes; the unnamed test
        // takes its indexed default.
        let spec = DriverSpec {
            doc: Some("Sorts things.\nDoes it well."),
            name: "clamp_value",
            generics: vec!["T"],
            params: vec![("x", Ty::Leaf(0)), ("limit", Ty::Leaf(0))],
            ret: Ty::Leaf(0),
            preconditions: vec![Cons::Eq(SExpr::Var("x"), SExpr::IntLit(3))],
            body: Stmt::Expr(Expr::Var("x")),
            tests: vec![
                DriverTest {
                    name: Some("x-is-three"),
                    input: SExpr::IntLit(3),
                    expected: SExpr::IntLit(3),
                },
                DriverTest {
                    name: None,
                    input: SExpr::IntLit(0),
                    expected: SExpr::IntLit(0),
                },
            ],
        };
        assert_eq!(
            generate_rust_from_spec_assembled(&spec),
            concat!(
                "//! Sorts things.\n",
                "//! Does it well.\n",
                "\n",
                "pub fn clamp_value<T: Clone>(x: i64, limit: i64) -> i64 {\n",
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
            )
        );
    }

    #[test]
    fn generate_rust_end_to_end_test_value_shapes() {
        // The Phase 55 test-value shapes through the
        // whole driver: a two-parameter function whose
        // test input is a tuple SPREADS into the call
        // positionally; a tuple in expected position
        // renders as a Rust tuple; a lambda test input
        // renders as a closure; a List renders as the
        // vec literal; None renders bare. The spec is
        // sort-shaped: (xs, cmp) -> xs.
        let spec = DriverSpec {
            doc: None,
            name: "sort_by",
            generics: vec![],
            params: vec![("xs", Ty::Leaf(0)), ("cmp", Ty::Leaf(0))],
            ret: Ty::Leaf(0),
            preconditions: vec![],
            body: Stmt::Expr(Expr::Var("xs")),
            tests: vec![
                DriverTest {
                    name: Some("spread"),
                    input: SExpr::Call(
                        "__tuple__",
                        vec![
                            SExpr::Call(
                                "List",
                                vec![SExpr::IntLit(2), SExpr::IntLit(1)],
                            ),
                            SExpr::Lambda(
                                vec!["a", "b"],
                                Box::new(SExpr::BinOp(
                                    9,
                                    Box::new(SExpr::Var("a")),
                                    Box::new(SExpr::Var("b")),
                                )),
                            ),
                        ],
                    ),
                    expected: SExpr::Call(
                        "List",
                        vec![SExpr::IntLit(1), SExpr::IntLit(2)],
                    ),
                },
                DriverTest {
                    name: Some("pair-out"),
                    input: SExpr::Call(
                        "__tuple__",
                        vec![SExpr::IntLit(1), SExpr::Call("None", vec![])],
                    ),
                    expected: SExpr::Call(
                        "__tuple__",
                        vec![SExpr::IntLit(1), SExpr::Call("None", vec![])],
                    ),
                },
            ],
        };
        assert_eq!(
            generate_rust_from_spec_assembled(&spec),
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
            )
        );
    }

    #[test]
    fn generate_rust_end_to_end_generic_annotation() {
        // The Phase 61 (G6) annotation through the whole
        // driver: a genuinely generic sort-shaped spec
        // whose empty-list test leaves T unpinned against
        // the fn-typed comparator parameter — that test's
        // call carries the turbofish (concrete type from
        // the spec's own tests), the pinned test's call
        // stays bare.
        let lt = || {
            SExpr::Lambda(
                vec!["a", "b"],
                Box::new(SExpr::BinOp(
                    9,
                    Box::new(SExpr::Var("a")),
                    Box::new(SExpr::Var("b")),
                )),
            )
        };
        let spec = DriverSpec {
            doc: None,
            name: "sort",
            generics: vec!["T"],
            params: vec![
                ("list", Ty::Wrap1(16, Box::new(Ty::Name("T")))),
                (
                    "cmp",
                    Ty::Func(
                        vec![Ty::Name("T"), Ty::Name("T")],
                        Box::new(Ty::Leaf(2)),
                    ),
                ),
            ],
            ret: Ty::Wrap1(16, Box::new(Ty::Name("T"))),
            preconditions: vec![],
            body: Stmt::Expr(Expr::Var("list")),
            tests: vec![
                DriverTest {
                    name: Some("empty"),
                    input: SExpr::Call(
                        "__tuple__",
                        vec![SExpr::Call("List", vec![]), lt()],
                    ),
                    expected: SExpr::Call("List", vec![]),
                },
                DriverTest {
                    name: Some("single"),
                    input: SExpr::Call(
                        "__tuple__",
                        vec![
                            SExpr::Call("List", vec![SExpr::IntLit(5)]),
                            lt(),
                        ],
                    ),
                    expected: SExpr::Call("List", vec![SExpr::IntLit(5)]),
                },
            ],
        };
        assert_eq!(
            generate_rust_from_spec_assembled(&spec),
            concat!(
                "pub fn sort<T: Clone>(list: Vec<T>, cmp: fn(T, T) -> bool) -> Vec<T> {\n",
                "    list\n",
                "}\n",
                "\n#[cfg(test)]\n",
                "mod tests {\n",
                "    use super::*;\n",
                "\n",
                "    #[test]\n",
                "    fn empty() {\n",
                "        let result = sort::<i64>(vec![], |a, b| (a < b));\n",
                "        assert_eq!(result, vec![]);\n",
                "    }\n",
                "    #[test]\n",
                "    fn single() {\n",
                "        let result = sort(vec![5], |a, b| (a < b));\n",
                "        assert_eq!(result, vec![5]);\n",
                "    }\n",
                "}\n",
            )
        );
    }

    #[test]
    fn generate_rust_end_to_end_bare() {
        // No doc, no generics, no preconditions, no
        // tests: the file is the signature, the body and
        // the closing brace — nothing else, no import.
        let spec = DriverSpec {
            doc: None,
            name: "answer",
            generics: vec![],
            params: vec![("n", Ty::Leaf(0))],
            ret: Ty::Leaf(2),
            preconditions: vec![],
            body: Stmt::Expr(Expr::Lit(Lit::Bool(true))),
            tests: vec![],
        };
        assert_eq!(
            generate_rust_from_spec_assembled(&spec),
            "pub fn answer(n: i64) -> bool {\n    true\n}\n"
        );
    }

    #[test]
    fn generate_rust_empty_doc_still_emits_blank_line() {
        // doc = Some(""): str::lines yields zero lines,
        // so the header is exactly the blank line and the
        // file STARTS with a newline. Presence is the
        // doc's Some-ness, not its non-emptiness.
        let spec = DriverSpec {
            doc: Some(""),
            name: "answer",
            generics: vec![],
            params: vec![("n", Ty::Leaf(0))],
            ret: Ty::Leaf(2),
            preconditions: vec![],
            body: Stmt::Expr(Expr::Lit(Lit::Bool(true))),
            tests: vec![],
        };
        assert_eq!(
            generate_rust_from_spec_assembled(&spec),
            "\npub fn answer(n: i64) -> bool {\n    true\n}\n"
        );
    }

    #[test]
    fn generate_rust_string_param_test_placement() {
        // The generated test places the expr_to_rust
        // rendering of a String test input BARE in call
        // position — a &str literal against the
        // function's String parameter, and the expected
        // value bare in the assert. Pinned byte-exactly,
        // as the driver performs it. The consequence, for
        // the gate phase: the file compiles as a library
        // (the test module is cfg(test)-gated, and the
        // probe's lib compile never exercises it), but
        // compiling it WITH tests (`rustc --test`) rejects
        // the call with E0308 — the spec-Expr side has no
        // counterpart to the IR side's ownership rewrite.
        let spec = DriverSpec {
            doc: None,
            name: "shout",
            generics: vec![],
            params: vec![("s", Ty::Leaf(3))],
            ret: Ty::Leaf(3),
            preconditions: vec![],
            body: Stmt::Expr(Expr::Var("s")),
            tests: vec![DriverTest {
                name: None,
                input: SExpr::StringLit("hey"),
                expected: SExpr::StringLit("hey"),
            }],
        };
        assert_eq!(
            generate_rust_from_spec_assembled(&spec),
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
            )
        );
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
        // plus the 24 slice-2 specs (20 + the 4 Phase 55 specs)
        // plus the 16 slice-3 specs plus
        // the 35 slice-4 specs plus the 27 slice-5 specs plus the
        // 6 slice-6 specs, plus rust_generic_param_text (Phase 62),
        // plus the 6 Phase 78 specs (rust_expr_field_text,
        // rust_expr_method_form, rust_expr_method_text,
        // rust_expr_record_text, rust_field_access_text,
        // rust_record_lit_text), plus the 6 Phase 85 specs
        // (rust_call_form, rust_call_len_form, rust_call_len_text,
        // rust_call_len_char_text, rust_index_form,
        // rust_index_char_text).
        let src = std::fs::read_to_string("examples/self_host/codegen.morph").unwrap();
        assert!(src.contains("Codegen"), "missing module Codegen");
        let spec_decls = src.lines().filter(|l| l.starts_with("spec ")).count();
        assert_eq!(spec_decls, 129, "codegen.morph should declare 129 specs");
        assert!(src.matches("test \"").count() >= 182, "codegen.morph should carry in-spec tests");
    }
}
