// ─────────────────────────────────────────────────────────────
//  SYNTHESIZED CODEGEN — GENERATED FILE. DO NOT EDIT.
//
//  The bootstrap artifact of M5-S1 (Phase 51): the M3 Codegen
//  module's 112 synthesized winners, compiled into this crate
//  and called directly by the bootstrap adapter
//  (src/codegen/bootstrap_adapter.rs). Each winner is rendered
//  through the ORIGINAL `codegen::generate` and wrapped in its
//  own module (so names cannot collide).
//
//  WINNER-SET PROVENANCE (fixed, run-independent — M3 gate,
//  Phases 41–42): 110 constraint-derived seeds — equation 54,
//  guard_table 39, composition_guard 7, pair_guard_table 4,
//  expr_pin_guard 3, guarded_identity 1, multi_guard 1,
//  mixed_pair_guard_table 1 — plus the 2 by-design seedless
//  pool winners (rust_ident_empty, rust_target_route; Mutation
//  provenance, deterministic across extraction passes).
//
//  REGENERATION (always from the ORIGINAL pipeline — the
//  generated compiler never generates a compiler):
//      cargo test -- --ignored --nocapture m5_s1_regen_artifact
//  rewrites this file in place; the companion test
//  `m5_s1_artifact_byte_identity` proves a fresh extraction +
//  render reproduces this file byte-for-byte.
//
//  Two mechanical adaptations separate this file from the M3
//  gate's raw render (target/m3-gate/synthesized_codegen.rs,
//  123,741 B); winner function bodies are byte-identical:
//  (1) each winner's generated `#[cfg(test)] mod tests` block
//      is stripped (55 of 112 winners carry one): verbatim
//      inclusion breaks `cargo test` compilation — the
//      generated tests pass &str literals to String parameters
//      (E0308, the Phase 20 record) — and the M3 gate compiles
//      this module with rustc WITHOUT --test, so the blocks
//      are inert in the judged artifact;
//  (2) each wrapper `mod` is promoted to `pub(crate) mod`: the
//      gate render is a standalone crate root, while here the
//      adapter must call the winners through their wrappers
//      (M5 audit, item B).
//
//  Judged by: the M3 gate (src/codegen/m3_gate.rs) for the
//  winner set; the M5-S1 equivalence proof
//  (src/codegen/m5_s1.rs) for this compiled form.
// ─────────────────────────────────────────────────────────────

#![allow(dead_code, unused)]

pub(crate) mod rust_type_name {
//! Rust name of a scalar TypeRef: the leaf table of ty_to_rust. Composite types (List..Stream) and Named/Generic are recursive or name-carrying forms, specified in a later slice and deliberately unpinned here. The table is not injective: Int (0) and I64 (9) both name i64.

pub fn rust_type_name(tag: i64) -> String {
    match tag {
        0 => {
            "i64".to_string()
        },
        1 => {
            "f64".to_string()
        },
        2 => {
            "bool".to_string()
        },
        3 => {
            "String".to_string()
        },
        4 => {
            "char".to_string()
        },
        5 => {
            "()".to_string()
        },
        6 => {
            "i8".to_string()
        },
        7 => {
            "i16".to_string()
        },
        8 => {
            "i32".to_string()
        },
        9 => {
            "i64".to_string()
        },
        10 => {
            "u8".to_string()
        },
        11 => {
            "u16".to_string()
        },
        12 => {
            "u32".to_string()
        },
        13 => {
            "u64".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_binop_symbol {
//! Infix symbol of an IR binary operator in Rust output (the operator table of ir_to_rust_expr). Append (17) and Merge (18) have no infix symbol -- they render as method calls -- and are deliberately unpinned: the emitter's own table binds '.push'/'.extend' for them, but those values are dead because the arms are special-cased first. Concat (16) shares Add's '+'.

pub fn rust_binop_symbol(op: i64) -> String {
    match op {
        0 => {
            "+".to_string()
        },
        1 => {
            "-".to_string()
        },
        2 => {
            "*".to_string()
        },
        3 => {
            "/".to_string()
        },
        4 => {
            "%".to_string()
        },
        5 => {
            "&&".to_string()
        },
        6 => {
            "||".to_string()
        },
        7 => {
            "^".to_string()
        },
        8 => {
            "==".to_string()
        },
        9 => {
            "!=".to_string()
        },
        10 => {
            "<".to_string()
        },
        11 => {
            "<=".to_string()
        },
        12 => {
            ">".to_string()
        },
        13 => {
            ">=".to_string()
        },
        14 => {
            "<<".to_string()
        },
        15 => {
            ">>".to_string()
        },
        16 => {
            "+".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_unaryop_symbol {
//! Prefix symbol of an IR unary operator: only Neg and Not have one (emitted parenthesized, as (- x) and (! x)). All other unary operators are unpinned here; they render as method calls or index forms (see rust_unaryop_form).

pub fn rust_unaryop_symbol(op: i64) -> String {
    match op {
        0 => {
            "-".to_string()
        },
        1 => {
            "!".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_unaryop_method {
//! Method name of an IR unary operator rendered as a zero-argument method call (x.abs(), x.len(), x.reverse(), x.clone()). The remaining operators are unpinned here: Neg/Not are prefix forms and Head/Tail/Init/Last are index forms (see rust_unaryop_form).

pub fn rust_unaryop_method(op: i64) -> String {
    match op {
        2 => {
            "abs".to_string()
        },
        3 => {
            "len".to_string()
        },
        8 => {
            "reverse".to_string()
        },
        9 => {
            "clone".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_unaryop_form {
//! Emission family of an IR unary operator: 0 = prefix symbol ((- x), (! x)), 1 = method call (x.m()), 2 = list-part index/slice form (Head, Tail, Init, Last, whose bespoke templates belong to the expression-walker slice). Total over all ten operators. Note: the method family includes Reverse, whose emitted call returns () in today's emitter -- a defect of the walker slice's template, not of this classification.

pub fn rust_unaryop_form(op: i64) -> i64 {
    match op {
        0 => {
            0
        },
        1 => {
            0
        },
        2 => {
            1
        },
        3 => {
            1
        },
        4 => {
            2
        },
        5 => {
            2
        },
        6 => {
            2
        },
        7 => {
            2
        },
        8 => {
            1
        },
        9 => {
            1
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_collection_form {
//! Emission form of a collection literal in EXPRESSION position: 0 = vec![..] macro (List), 1 = std::collections::HashSet::from([..]) (Set), 2 = std::collections::HashMap::from([..]) (Map), 3 = array literal [..] (Array), 4 = tuple literal (..) (Tuple). Total over all five kinds. Statement position is a different, position-dependent rule -- List, Set and Map all collapse to the vec![..]; form there -- and belongs to the statement-walker slice.

pub fn rust_collection_form(kind: i64) -> i64 {
    match kind {
        0 => {
            0
        },
        1 => {
            1
        },
        2 => {
            2
        },
        3 => {
            3
        },
        4 => {
            4
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_keyword {
//! Whether a name is a Rust keyword for the emitter (is_rust_keyword): the exact 51-word set the emitter lists -- strict, reserved and weak keywords mixed, not any single Rust edition's set (for example it has no 'gen'). Matching is exact and case-sensitive: 'Self' is a keyword, 'SELF' and 'selfish' are not. rust_ident appends '_' to a name exactly when this holds.

pub fn rust_keyword(name: String) -> bool {
    (((((((((((((((((((((((((((((((((((((((((((((((((((name == "as") || (name == "break")) || (name == "const")) || (name == "continue")) || (name == "crate")) || (name == "else")) || (name == "enum")) || (name == "extern")) || (name == "false")) || (name == "fn")) || (name == "for")) || (name == "if")) || (name == "impl")) || (name == "in")) || (name == "let")) || (name == "loop")) || (name == "match")) || (name == "mod")) || (name == "move")) || (name == "mut")) || (name == "pub")) || (name == "ref")) || (name == "return")) || (name == "self")) || (name == "Self")) || (name == "static")) || (name == "struct")) || (name == "super")) || (name == "trait")) || (name == "true")) || (name == "type")) || (name == "unsafe")) || (name == "use")) || (name == "where")) || (name == "while")) || (name == "async")) || (name == "await")) || (name == "dyn")) || (name == "abstract")) || (name == "become")) || (name == "box")) || (name == "do")) || (name == "final")) || (name == "macro")) || (name == "override")) || (name == "priv")) || (name == "typeof")) || (name == "unsized")) || (name == "virtual")) || (name == "yield")) || (name == "try"))
}

}

pub(crate) mod spec_binop_symbol {
//! Operator symbol expr_to_rust uses when emitting a spec expression into generated Rust (test assertions in generate_rust's output) -- a SEPARATE, weaker table than rust_binop_symbol. The '?' rows are the emitter's real fallback for the operators it does not map (Mod, Lt, Le, Gt, Ge, Concat) and are pinned deliberately: a synthesized replacement must reproduce today's bytes, fallback included. Fixing the fallback is a separate later decision. Total over all fourteen operators.

pub fn spec_binop_symbol(op: i64) -> String {
    match op {
        0 => {
            "+".to_string()
        },
        1 => {
            "-".to_string()
        },
        2 => {
            "*".to_string()
        },
        3 => {
            "/".to_string()
        },
        4 => {
            "?".to_string()
        },
        5 => {
            "&&".to_string()
        },
        6 => {
            "||".to_string()
        },
        7 => {
            "==".to_string()
        },
        8 => {
            "!=".to_string()
        },
        9 => {
            "?".to_string()
        },
        10 => {
            "?".to_string()
        },
        11 => {
            "?".to_string()
        },
        12 => {
            "?".to_string()
        },
        13 => {
            "?".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_type_form {
//! Template family of a TypeRef code in ty_to_rust (src/codegen/mod.rs:1093): 0 = scalar leaf (codes 0-13, named by rust_type_name), 1 = name-carrying (Generic 14, Named 15: the output is the rust_ident-rendered name itself -- slice 3 owns rust_ident), 2 = wrap-one (prefix + inner + suffix: List, Set, Option, Ref, Stream), 3 = wrap-two (prefix + first + separator + second + suffix: Map, Result), 4 = tuple join (prefix + parts joined by separator + suffix), 5 = array (prefix + element + separator + decimal size + suffix), 6 = function (prefix + args joined by separator + midfix + return type + suffix). Total over all 26 TypeRef codes. The fixed texts are pinned by rust_type_prefix / rust_type_separator / rust_type_midfix / rust_type_suffix; the concatenation itself is unpinned (no concat operator in the spec language -- see the slice-2 note).

pub fn rust_type_form(tag: i64) -> i64 {
    match tag {
        0 => {
            0
        },
        1 => {
            0
        },
        2 => {
            0
        },
        3 => {
            0
        },
        4 => {
            0
        },
        5 => {
            0
        },
        6 => {
            0
        },
        7 => {
            0
        },
        8 => {
            0
        },
        9 => {
            0
        },
        10 => {
            0
        },
        11 => {
            0
        },
        12 => {
            0
        },
        13 => {
            0
        },
        14 => {
            1
        },
        15 => {
            1
        },
        16 => {
            2
        },
        17 => {
            2
        },
        18 => {
            3
        },
        19 => {
            2
        },
        20 => {
            3
        },
        21 => {
            4
        },
        22 => {
            5
        },
        23 => {
            6
        },
        24 => {
            2
        },
        25 => {
            2
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_type_prefix {
//! Fixed opening text of a composite type form in ty_to_rust (src/codegen/mod.rs:1107-1122), keyed by TypeRef code. Set (17) is UNQUALIFIED 'HashSet<' -- unlike the fully qualified Set literal in ir_to_rust_expr, and no import is ever generated, so a Set-typed signature fails E0433 in real output; pinned as-is: a synthesized replacement must reproduce today's bytes. Scalar and name codes have no prefix and are unpinned here.

pub fn rust_type_prefix(tag: i64) -> String {
    match tag {
        16 => {
            "Vec<".to_string()
        },
        17 => {
            "HashSet<".to_string()
        },
        18 => {
            "HashMap<".to_string()
        },
        19 => {
            "Option<".to_string()
        },
        20 => {
            "Result<".to_string()
        },
        21 => {
            "(".to_string()
        },
        22 => {
            "[".to_string()
        },
        23 => {
            "fn(".to_string()
        },
        24 => {
            "&".to_string()
        },
        25 => {
            "impl Iterator<Item = ".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_type_suffix {
//! Fixed closing text of a composite type form in ty_to_rust (src/codegen/mod.rs:1107-1122), keyed by TypeRef code. Function (23) and Ref (24) close with the empty string: a function's return type follows its midfix directly, and a reference is prefix-only. Scalar and name codes have no suffix and are unpinned here.

pub fn rust_type_suffix(tag: i64) -> String {
    match tag {
        16 => {
            ">".to_string()
        },
        17 => {
            ">".to_string()
        },
        18 => {
            ">".to_string()
        },
        19 => {
            ">".to_string()
        },
        20 => {
            ">".to_string()
        },
        21 => {
            ")".to_string()
        },
        22 => {
            "]".to_string()
        },
        23 => {
            "".to_string()
        },
        24 => {
            "".to_string()
        },
        25 => {
            ">".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_type_separator {
//! Fixed text between the rendered children of a composite type form in ty_to_rust: ', ' between Map/Result children, tuple parts and function arguments; '; ' between an array's element type and its size. The size itself is rendered by the Int -> decimal rule, which the spec language cannot express (see rust_literal_form). Wrap-one and name/scalar codes have no separator and are unpinned here.

pub fn rust_type_separator(tag: i64) -> String {
    match tag {
        18 => {
            ", ".to_string()
        },
        20 => {
            ", ".to_string()
        },
        21 => {
            ", ".to_string()
        },
        22 => {
            "; ".to_string()
        },
        23 => {
            ", ".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_type_midfix {
//! Fixed text between the joined argument list and the return type of a function type in ty_to_rust: ') -> '. Only Function (23) has a midfix; every other code is unpinned here.

pub fn rust_type_midfix(tag: i64) -> String {
    match tag {
        23 => {
            ") -> ".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_literal_form {
//! Rendering family of an IR literal in expression position (the Literal arm of ir_to_rust_expr, src/codegen/mod.rs:605-612); pattern position uses the same six kinds with its own fallback table (rust_pattern_literal_form). 0 = decimal text of the value (Int: n.to_string()), 1 = f64 Display text (Float: f.to_string() -- 1.0 renders '1', not '1.0'), 2 = value text (Bool, via rust_literal_bool), 3 = quoted template (String: a quote, the escaped contents, a quote -- the escape rule is per-character and mirror-pinned, Phase 29), 4 = quoted template (Char: a single quote, the escaped character, a single quote), 5 = fixed text (Unit, via rust_literal_fixed_text). Total over all six IRLiteral kinds. Families 0 and 1 are classifications only: turning a number into its decimal/Display text is not expressible in the spec language (no Int -> String conversion, no float formatting), and families 3 and 4 need the concatenation the language lacks -- all recorded in the ROAD M3 notes. What is pinned here is WHICH rule applies to each kind, completely.

pub fn rust_literal_form(kind: i64) -> i64 {
    match kind {
        0 => {
            0
        },
        1 => {
            1
        },
        2 => {
            2
        },
        3 => {
            3
        },
        4 => {
            4
        },
        5 => {
            5
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_literal_bool {
//! Text of a Bool literal: IRLiteral::Bool rendered by b.to_string() in ir_to_rust_expr (src/codegen/mod.rs:607) and identically in ir_pattern_to_rust (src/codegen/mod.rs:818) -- 'true' for true, 'false' for false. Total over the Bool domain; serves both walkers.

pub fn rust_literal_bool(value: bool) -> String {
    match value {
        true => {
            "true".to_string()
        },
        false => {
            "false".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_literal_fixed_text {
//! Value-independent text of an IR literal kind in expression position: only Unit (5) has one -- '()' (src/codegen/mod.rs:611). Every other kind's text depends on the literal's value and is unpinned here.

pub fn rust_literal_fixed_text(kind: i64) -> String {
    match kind {
        5 => {
            "()".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_pattern_form {
//! Rendering family of an IR pattern in ir_pattern_to_rust (src/codegen/mod.rs:813): 0 = fixed text (Wildcard, via rust_pattern_fixed_text), 1 = name (Variable: the rust_ident-rendered name itself -- slice 3 owns rust_ident), 2 = literal (Literal: the family of rust_pattern_literal_form), 3 = constructor template (the name, then '(' + sub-patterns joined ', ' + ')' -- or the bare name when there are no sub-patterns, pinned by rust_pattern_ctor_name), 4 = guard template (the pattern, then ' if ', then the guard text -- the guard is already text in the IR and is interpolated raw). Total over all five IRPattern kinds. The templates' fixed texts are stated here; their assembly needs concatenation and is unpinned (slice-2 note).

pub fn rust_pattern_form(kind: i64) -> i64 {
    match kind {
        0 => {
            0
        },
        1 => {
            1
        },
        2 => {
            2
        },
        3 => {
            3
        },
        4 => {
            4
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_pattern_fixed_text {
//! Value-independent text of an IR pattern kind: only Wildcard (0) has one -- '_' (src/codegen/mod.rs:815).

pub fn rust_pattern_fixed_text(kind: i64) -> String {
    match kind {
        0 => {
            "_".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_pattern_literal_form {
//! Rendering family of a literal inside a pattern (the Literal arm of ir_pattern_to_rust, src/codegen/mod.rs:817-822): 0 = value text (Int renders its decimal text; Bool renders via rust_literal_bool), 1 = wildcard fallback (Float, Char and Unit patterns render '_' regardless of value -- pinned by rust_pattern_literal_text; such a pattern therefore matches EVERYTHING, silently widening the match arm), 2 = quoted template (String: a quote, the escaped contents, a quote -- as in expression position; the escape rule is per-character and mirror-pinned, Phase 29). Total over all six IRLiteral kinds.

pub fn rust_pattern_literal_form(kind: i64) -> i64 {
    match kind {
        0 => {
            0
        },
        1 => {
            1
        },
        2 => {
            0
        },
        3 => {
            2
        },
        4 => {
            1
        },
        5 => {
            1
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_pattern_literal_text {
//! Fixed text of the pattern-literal fallback rows (src/codegen/mod.rs:821): Float (1), Char (4) and Unit (5) literal patterns all render '_'. Int, Bool and String literals are value-dependent and unpinned here.

pub fn rust_pattern_literal_text(kind: i64) -> String {
    match kind {
        1 => {
            "_".to_string()
        },
        4 => {
            "_".to_string()
        },
        5 => {
            "_".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_pattern_ctor_name {
//! A constructor pattern with NO sub-patterns renders as its bare name (ir_pattern_to_rust, src/codegen/mod.rs:823-827): output == name exactly, for every name. The name arrives already rendered by rust_ident (slice 3 owns sanitization). The with-sub-patterns branch -- the name, then '(' + the joined sub-pattern renderings + ')' -- is assembly and unpinned (slice-2 note). Multi-input spec: no in-spec tests (the test syntax cannot bind tuple inputs); golden cases live in tests/m3_proof.rs.

pub fn rust_pattern_ctor_name(name: String, has_subs: bool) -> String {
    match has_subs {
        false => {
            name
        },
        _ => {
            "".to_string()
        },
    }
}
}

pub(crate) mod rust_expr_form {
//! Rendering family of a spec expression in expr_to_rust (src/codegen/mod.rs:1177-1200) -- the walker that emits spec tests into generated code (generate_rust's test module) and constraint operands into precondition asserts: 0 = name (Var: the rust_ident-rendered name), 1 = decimal text (IntLit), 2 = value text (BoolLit, via rust_literal_bool), 3 = quoted template (StringLit: a quote, the escaped contents, a quote -- the escape rule is per-character and mirror-pinned, Phase 29), 4 = call template (the name, then '(' + args joined ', ' + ')'), 5 = binary template ('(' + left + ' ' + symbol + ' ' + right + ')' with the symbol from spec_binop_symbol, slice 1), 6 = fallback fixed text '??' (via rust_expr_fixed_text). Total over all thirteen Expr kinds. Two sharp edges of the original are pinned deliberately: FloatLit (2) and UnaryOp (9) have NO arm and fall through to '??' like the structural kinds -- a float literal in a spec test renders as '??' in the generated test. And StringLit's rendering carries a placement caveat, pinned as-is: the E0308 it can cause (a &str literal passed where the synthesized function takes String) is a property of generate_rust placing this rendering in call position -- the driver slice owns it; the IR side's ownership rewrite (ir_to_rust_expr_typed) has no counterpart here.

pub fn rust_expr_form(kind: i64) -> i64 {
    match kind {
        0 => {
            0
        },
        1 => {
            1
        },
        2 => {
            6
        },
        3 => {
            2
        },
        4 => {
            3
        },
        5 => {
            6
        },
        6 => {
            6
        },
        7 => {
            4
        },
        8 => {
            5
        },
        9 => {
            6
        },
        10 => {
            6
        },
        11 => {
            6
        },
        12 => {
            6
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_expr_fixed_text {
//! Fixed text of expr_to_rust's catch-all (src/codegen/mod.rs:1199): every Expr kind without an arm -- FloatLit (2), Field (5), Index (6), UnaryOp (9), Lambda (10), Length (11), Comprehension (12) -- renders '??'.

pub fn rust_expr_fixed_text(kind: i64) -> String {
    match kind {
        2 => {
            "??".to_string()
        },
        5 => {
            "??".to_string()
        },
        6 => {
            "??".to_string()
        },
        9 => {
            "??".to_string()
        },
        10 => {
            "??".to_string()
        },
        11 => {
            "??".to_string()
        },
        12 => {
            "??".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_constraint_form {
//! Rendering family of a constraint in constraint_to_rust (src/codegen/mod.rs:1162-1174) -- the walker behind generate_rust's precondition debug_assert!s: 0 = fixed text 'true' (True itself, plus every kind the catch-all swallows: Expr, Forall, Exists, Implies, Or, Predicate -- pinned row by row in rust_constraint_fixed_text), 1 = negation template ('!(' + inner rendering + ')'), 2 = equality template (left + ' == ' + right, operands rendered by expr_to_rust), 3 = order (rust_order_form: LessThan/LessThanOrEqual are templates over expr_to_rust operands, GreaterThan/GreaterThanOrEqual fall back to 'true'), 4 = conjunction template ('(' + parts joined ' && ' + ')'). Total over all eleven Constraint kinds. Pinning family 0 completely is the point: an Or precondition, for example, asserts nothing in generated code today.

pub fn rust_constraint_form(kind: i64) -> i64 {
    match kind {
        0 => {
            0
        },
        1 => {
            0
        },
        2 => {
            0
        },
        3 => {
            0
        },
        4 => {
            0
        },
        5 => {
            4
        },
        6 => {
            0
        },
        7 => {
            1
        },
        8 => {
            2
        },
        9 => {
            3
        },
        10 => {
            0
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_constraint_fixed_text {
//! Fixed text of constraint_to_rust's True arm and catch-all (src/codegen/mod.rs:1163,1173): True (0), Expr (1), Forall (2), Exists (3), Implies (4), Or (6) and Predicate (10) all render 'true'.

pub fn rust_constraint_fixed_text(kind: i64) -> String {
    match kind {
        0 => {
            "true".to_string()
        },
        1 => {
            "true".to_string()
        },
        2 => {
            "true".to_string()
        },
        3 => {
            "true".to_string()
        },
        4 => {
            "true".to_string()
        },
        6 => {
            "true".to_string()
        },
        10 => {
            "true".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_order_form {
//! Rendering family of an Order constraint's operator (constraint_to_rust, src/codegen/mod.rs:1166-1167): 0 = comparison template with separator ' < ' (LessThan), 1 = comparison template with separator ' <= ' (LessThanOrEqual), 2 = fallback fixed text 'true' (GreaterThan and GreaterThanOrEqual have no arm and are swallowed by the catch-all, so a '>' precondition asserts nothing in generated code). Total over all four OrderOp codes. The separators are pinned by rust_order_separator.

pub fn rust_order_form(op: i64) -> i64 {
    match op {
        0 => {
            0
        },
        1 => {
            1
        },
        2 => {
            2
        },
        3 => {
            2
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_order_separator {
//! Infix separator of an order comparison in constraint_to_rust, spaces included: ' < ' for LessThan (0), ' <= ' for LessThanOrEqual (1). GreaterThan and GreaterThanOrEqual never reach a separator (rust_order_form family 2) and are unpinned here.

pub fn rust_order_separator(op: i64) -> String {
    match op {
        0 => {
            " < ".to_string()
        },
        1 => {
            " <= ".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_condition_wrap {
//! Whether expr_in_condition_position (src/codegen/mod.rs:587-601) wraps a node's rendering in parentheses when the node appears as an if/while condition or a match scrutinee: the block-like kinds -- Block (1), Let (2), If (3), While (4), For (5), Match (6), Lambda (13), Closure (14), Loop (24), StructDef (26) -- wrap; every other kind renders bare (a BinOp, for example, is already parenthesized by its own template and is NOT wrapped again). Total over all 28 IRNode kinds. The wrap template itself ('(' + rendering + ')') is assembly and unpinned (slice-2 note); what is pinned is exactly the classification the original's match states.

pub fn rust_condition_wrap(kind: i64) -> bool {
    ((((((((((kind == 1) || (kind == 2)) || (kind == 3)) || (kind == 4)) || (kind == 5)) || (kind == 6)) || (kind == 13)) || (kind == 14)) || (kind == 24)) || (kind == 26))
}

}

pub(crate) mod rust_ident_suffix {
//! The keyword-suffix step of rust_ident (src/codegen/mod.rs:49-51), stated as a composition over rust_keyword: a name that is a Rust keyword gets '_' appended; any other name passes through unchanged. The input is the PER-CHAR STEP'S OUTPUT (the sanitized string -- never empty: the empty name becomes '_' in that step, pinned by rust_ident_empty), not a raw spec name; the per-char step itself is finding F5. Stating the guard over the sanitized string loses nothing: sanitization only ever introduces '_' characters or a leading '_' prefix, and no keyword contains '_', so a raw name is a keyword iff its sanitization is one.

pub fn rust_ident_suffix(name: String) -> String {
    match (((((((((((((((((((((((((((((((((((((((((((((((((((name == "as") || (name == "break")) || (name == "const")) || (name == "continue")) || (name == "crate")) || (name == "else")) || (name == "enum")) || (name == "extern")) || (name == "false")) || (name == "fn")) || (name == "for")) || (name == "if")) || (name == "impl")) || (name == "in")) || (name == "let")) || (name == "loop")) || (name == "match")) || (name == "mod")) || (name == "move")) || (name == "mut")) || (name == "pub")) || (name == "ref")) || (name == "return")) || (name == "self")) || (name == "Self")) || (name == "static")) || (name == "struct")) || (name == "super")) || (name == "trait")) || (name == "true")) || (name == "type")) || (name == "unsafe")) || (name == "use")) || (name == "where")) || (name == "while")) || (name == "async")) || (name == "await")) || (name == "dyn")) || (name == "abstract")) || (name == "become")) || (name == "box")) || (name == "do")) || (name == "final")) || (name == "macro")) || (name == "override")) || (name == "priv")) || (name == "typeof")) || (name == "unsized")) || (name == "virtual")) || (name == "yield")) || (name == "try")) {
        true => {
            (name.clone() + "_")
        },
        false => {
            name
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_ident_empty {
//! The empty-name fallback of rust_ident's per-char step (src/codegen/mod.rs:46-48): an empty name sanitizes to '_'. This is the only case of the per-char step the spec language pins completely -- the step is a per-character loop and the language has no character access (finding F5); every non-empty name is unpinned here and covered by the mirror. The fallback feeds the suffix step: '_' is not a keyword, so the full rust_ident of the empty name is exactly '_'.

pub fn rust_ident_empty(name: String) -> String {
    match name.as_str() {
        "" => {
            "_".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_typed_form {
//! Dispatch family of ir_to_rust_expr_typed (src/codegen/mod.rs:549-558) over IRNode kinds: 0 = delegate to the untyped emitter ir_to_rust_expr (no ownership rewrite -- every kind except Literal and Typed, INCLUDING composites: a string literal nested inside a BinOp, Call or Collection is rendered untyped and stays bare even where a String is expected) -- with one Phase 29 exception: the untyped emitter's Concat arm applies the ownership/borrow treatment to its own operands directly (defects 1 and 2a of the unfreeze inventory), so a literal that IS a Concat operand no longer stays bare; literals nested in any other composite still do, 1 = Literal -- the ownership check (rust_string_ownership) decides owned vs bare, 2 = Typed -- re-enter the typed emitter with the node's declared type as the expectation (the untyped emitter routes Typed nodes here too, src/codegen/mod.rs:809). Total over all 28 IRNode kinds.

pub fn rust_typed_form(kind: i64) -> i64 {
    match kind {
        9 => {
            1
        },
        22 => {
            2
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_string_ownership {
//! The ownership leaf check of ir_to_rust_expr_typed (src/codegen/mod.rs:552-554): a String literal is emitted owned -- rust_string_literal_owned_text instead of rust_string_literal_text -- exactly when its literal kind is String (3) AND the expected type is String (ExpectedTypeCode 3). 'expected' is an ExpectedTypeCode: a TypeRef code for Some(type), 26 for None. Callers pass Some(String) only via: a Return in a String-returning function (fn_ret, src/codegen/mod.rs:400), an Alloc whose declared type is String (src/codegen/mod.rs:406), a tail position inheriting the function's return type (ir_to_rust_tail, src/codegen/mod.rs:507-540; generate_rust seeds the body with Some(return type)), or a Typed node annotated String (src/codegen/mod.rs:551 and 809). Every other expectation -- any other type, or none -- leaves the literal a &str. Only the String literal kind is ever rewritten; no other literal kind has a &str form. Multi-input spec: no in-spec tests (the test syntax cannot bind tuple inputs); golden cases live in tests/m3_proof.rs.

pub fn rust_string_ownership(lit: i64, expected: i64) -> bool {
    match lit {
        3 => {
            match expected {
                3 => {
                    true
                },
                _ => {
                    false
                },
            }
        },
        _ => {
            false
        },
    }
}
}

pub(crate) mod rust_string_literal_text {
//! The bare rendering of a String literal: a quote, the ESCAPED contents, a quote. The escaping follows Rust source rules (a quote or backslash takes a backslash; newline, carriage return and tab use their short forms; other control characters use the unicode escape form; every other character, including non-ASCII text, is emitted as-is) and is a per-character transformation -- inexpressible in the spec language (F5) and therefore mirror-pinned (tests/m3_proof.rs). This spec's equation pins the WRAP: it is exact exactly on the identity subdomain (contents with no quote, backslash or control character, where escaping changes nothing) and silent off it -- cross-validation counts such contents as spec-silent for this spec. Before Phase 29 the emitter rendered the contents RAW and this equation was total; the user-approved unfreeze of the three rendering defects replaced the raw rendering with the escaped one. This one template serves three emitter sites: the Literal arm of ir_to_rust_expr, the Literal arm of ir_pattern_to_rust, and the delegate target of ir_to_rust_expr_typed whenever rust_string_ownership does not fire. Slice 2 could only classify it (rust_literal_form family 3, rust_pattern_literal_form family 2) because the spec language had no concatenation; F1's resolution makes the template itself a statable equation.

pub fn rust_string_literal_text(contents: String) -> String {
    (("\"".to_string() + &contents) + "\"")
}

}

pub(crate) mod rust_string_literal_owned_text {
//! The owned rendering of a String literal (src/codegen/mod.rs:552-554): the bare template (rust_string_literal_text) with '.to_string()' appended -- exactly what ir_to_rust_expr_typed emits when rust_string_ownership fires, converting the &str literal into a String where the expected type requires one. The bare template it composes escapes its contents (Phase 29; see rust_string_literal_text) -- this spec's equation is exact on the same identity subdomain.

pub fn rust_string_literal_owned_text(contents: String) -> String {
    (("\"".to_string() + &contents) + "\".to_string()")
}

}

pub(crate) mod rust_doc_line {
//! One line of generate_rust's module doc header (src/codegen/mod.rs:73): '//! ' + the doc line + a newline. The doc string is split into lines by the driver (str::lines -- iteration, mirror-only) and the header exists only when the spec has a doc comment; after the last line the driver emits one blank line (rust_file_fixed_text, piece 1).

pub fn rust_doc_line(line: String) -> String {
    (("//! ".to_string() + &line) + "\n")
}

}

pub(crate) mod rust_param_text {
//! One parameter of the generated signature (src/codegen/mod.rs:91): the rust_ident-rendered name, ': ', the ty_to_rust-rendered type. Both parts arrive already rendered (the slice-2/3 representation rule); the ', ' join over the parameter list is iteration and lives in the mirror. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_param_text(name: String, ty: String) -> String {
    ((name.clone() + ": ") + &ty)
}
}

pub(crate) mod rust_generics_text {
//! The generics bracket of the generated signature (src/codegen/mod.rs:80-88): no generics renders the empty string; otherwise '<' + the ', '-joined rust_ident-rendered generic names + '>'. The joined text arrives pre-rendered -- the join is iteration, mirror-only. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_generics_text(joined: String, present: bool) -> String {
    match present {
        false => {
            "".to_string()
        },
        true => {
            (("<".to_string() + &joined) + ">")
        },
        _ => {
            "".to_string()
        },
    }
}
}

pub(crate) mod rust_signature {
//! The generated function's signature line (src/codegen/mod.rs:107-110) over pre-rendered parts: 'pub fn ' + the rust_ident-rendered name + the generics text (rust_generics_text) + '(' + the ', '-joined parameter texts (rust_param_text) + ') -> ' + the ty_to_rust-rendered return type + ' {' and a newline. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_signature(name: String, generics: String, params: String, ret: String) -> String {
    ((((((("pub fn ".to_string() + &name) + &generics) + "(") + &params) + ") -> ") + &ret) + " {\n")
}
}

pub(crate) mod rust_precondition_assert {
//! One precondition assert line inside generate_rust's debug block (src/codegen/mod.rs:100-101): four spaces, 'debug_assert!(', the constraint_to_rust rendering, ', "Precondition violated");' and a newline. The rendering arrives pre-rendered (slice 2 owns constraint_to_rust's families); note a constraint the walker swallows still asserts -- as 'true' -- it is not dropped at this layer.

pub fn rust_precondition_assert(rendered: String) -> String {
    (("    debug_assert!(".to_string() + &rendered) + ", \"Precondition violated\");\n")
}

}

pub(crate) mod rust_precondition_block {
//! generate_rust's precondition block (src/codegen/mod.rs:96-104) over the pre-joined assert lines (rust_precondition_assert): the '#[cfg(debug_assertions)]' attribute line, '{', the asserts, '}' and a blank line. The block exists only when the spec has preconditions -- that branch is the driver's. Recorded as-is (surprise 13 in the ROAD M3 notes): the block is emitted BEFORE the signature, as a free-standing block at module level whose asserts name the function's parameters -- outside the function body entirely.

pub fn rust_precondition_block(asserts: String) -> String {
    (("#[cfg(debug_assertions)]\n{\n".to_string() + &asserts) + "}\n\n")
}

}

pub(crate) mod rust_test_name_form {
//! Which name a generated test gets (src/codegen/mod.rs:136): 0 = the test's own name (rust_ident-rendered), 1 = the default 'test_{i}' with i the test's index. Only the CHOICE is pinned: the default's text needs the index as decimal, which the spec language cannot express (finding F2) -- the mirror renders it exactly.

pub fn rust_test_name_form(has_name: bool) -> i64 {
    match has_name {
        true => {
            0
        },
        false => {
            1
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_test_case {
//! One generated test's block (src/codegen/mod.rs:139-150) over pre-rendered parts: the '#[test]' attribute and 'fn' line at four spaces with the test's final name (rust_ident-rendered, chosen per rust_test_name_form), then at eight spaces the call 'let result = <fn>(<input>);' and the assertion 'assert_eq!(result, <expected>);', then the closing brace at four spaces. The function name arrives rust_ident-rendered; input and expected arrive as expr_to_rust renderings (slice 2 owns that walker) placed BARE in call/assert position -- the recorded E0308 property of the generated-test path (a String test input renders as a &str literal against a String parameter) is a property of this placement, pinned as-is. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_test_case(test_name: String, fn_name: String, input_text: String, expected_text: String) -> String {
    (((((((("    #[test]\n    fn ".to_string() + &test_name) + "() {\n        let result = ") + &fn_name) + "(") + &input_text) + ");\n        assert_eq!(result, ") + &expected_text) + ");\n    }\n")
}
}

pub(crate) mod rust_test_module {
//! generate_rust's test module wrapper (src/codegen/mod.rs:131-133,151) over the pre-joined test blocks (rust_test_case): a blank line, '#[cfg(test)]', 'mod tests {', the 'use super::*;' import and a blank line, then the blocks, then '}'. The module exists only when the spec has tests -- that branch is the driver's.

pub fn rust_test_module(tests: String) -> String {
    (("\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n".to_string() + &tests) + "}\n")
}

}

pub(crate) mod rust_file_fixed_text {
//! generate_rust's two standalone fixed texts -- the ones no slice-3 template carries: piece 0 = the function body's closing brace and newline, emitted after the rendered body (src/codegen/mod.rs:127); piece 1 = the blank line after the doc header block (src/codegen/mod.rs:75). Every other fixed text of the driver lives inside a slice-3 template (rust_signature, rust_precondition_block, rust_test_module).

pub fn rust_file_fixed_text(piece: i64) -> String {
    match piece {
        0 => {
            "}\n".to_string()
        },
        1 => {
            "\n".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_expr_binop_form {
//! Emission family of an IR binary operator in ir_to_rust_expr (src/codegen/mod.rs:753-781): 0 = general infix ('(' + left + ' ' + symbol + ' ' + right + ')' with the symbol from rust_binop_symbol), 1 = Concat (the owned/borrowed '+' form -- rust_binop_concat_text over the two operand treatments), 2 = Append (the '.push' method form), 3 = Merge (the '.extend' method form). Total over all nineteen IRBinOp codes. The branch is taken BEFORE the operator table is consulted for families 1/2/3: the table's '.push'/'.extend' bindings for Append/Merge are dead values (see rust_binop_symbol), and Concat's '+' symbol row is used by no template -- family 1 has its own operand treatments (Phase 29).

pub fn rust_expr_binop_form(op: i64) -> i64 {
    match op {
        0 => {
            0
        },
        1 => {
            0
        },
        2 => {
            0
        },
        3 => {
            0
        },
        4 => {
            0
        },
        5 => {
            0
        },
        6 => {
            0
        },
        7 => {
            0
        },
        8 => {
            0
        },
        9 => {
            0
        },
        10 => {
            0
        },
        11 => {
            0
        },
        12 => {
            0
        },
        13 => {
            0
        },
        14 => {
            0
        },
        15 => {
            0
        },
        16 => {
            1
        },
        17 => {
            2
        },
        18 => {
            3
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_binop_infix_text {
//! The general infix rendering of a BinOp node (src/codegen/mod.rs:778): '(' + left + ' ' + symbol + ' ' + right + ')' with the symbol looked up in rust_binop_symbol -- stated as a composition over that call, the way the walker consults the table. The operands arrive rendered by ir_to_rust_expr itself (bare: whatever parenthesization exists comes from the operands' own templates). Contract: op is a general operator (rust_expr_binop_form family 0); Concat/Append/Merge branch away before this template. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_binop_infix_text(op: i64, l: String, r: String) -> String {
    (((((("(".to_string() + &l) + " ") + &(match op { 0 => "+", 1 => "-", 2 => "*", 3 => "/", 4 => "%", 5 => "&&", 6 => "||", 7 => "^", 8 => "==", 9 => "!=", 10 => "<", 11 => "<=", 12 => ">", 13 => ">=", 14 => "<<", 15 => ">>", 16 => "+", _ => "" })) + " ") + &r) + ")")
}
}

pub(crate) mod rust_binop_append_text {
//! The Append rendering of a BinOp node (src/codegen/mod.rs:762-763): left + '.push(' + right + ')'. The left operand is interpolated BARE (no added parentheses -- a block-like left operand would not parse, a pre-existing property of the arm, pinned as-is); the symbol table's '.push' binding for Append is a dead value and plays no part here. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_binop_append_text(l: String, r: String) -> String {
    (((l.clone() + ".push(") + &r) + ")")
}
}

pub(crate) mod rust_binop_merge_text {
//! The Merge rendering of a BinOp node (src/codegen/mod.rs:764-765): left + '.extend(' + right + ')'. As with Append, the left operand is interpolated bare and the table's '.extend' binding is dead. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_binop_merge_text(l: String, r: String) -> String {
    (((l.clone() + ".extend(") + &r) + ")")
}
}

pub(crate) mod rust_binop_concat_text {
//! The Concat rendering of a BinOp node (src/codegen/mod.rs:766-777, as amended by Phase 29): '(' + owned-left + ' + ' + borrowed-right + ')'. The two operands do NOT arrive as their plain renderings: the left arrives treated by concat_owned_operand (rust_concat_left_owned decides, rust_concat_left_text renders) and the right by concat_borrowed_operand (rust_concat_right_form decides, rust_concat_right_text renders), because Rust's Add for text is String + &str -- an owned left operand and a borrowed right one. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_binop_concat_text(owned: String, borrowed: String) -> String {
    (((("(".to_string() + &owned) + " + ") + &borrowed) + ")")
}
}

pub(crate) mod rust_concat_left_owned {
//! Whether the LEFT operand of a Concat takes the ownership treatment (concat_owned_operand, src/codegen/mod.rs; Phase 29 defect 1, extended by Phase 39): exactly when the operand node is directly a String literal -- node kind Literal (9) with literal kind String (3) -- or directly a variable -- node kind Var (10). A bare literal in left position is a &str, and &str + &str has no Add impl (E0369); it takes the same .to_string() ownership ir_to_rust_expr_typed applies in expectation positions. A bare variable in left position MOVES (Phase 39, E0382): Rust's + consumes its left operand, so a variable used again later on the execution path fails with borrow of moved value; it takes the owned-copy treatment, .clone() -- the literal treatment's analogue (no borrow form exists: &String + &str has no Add impl). Every other operand shape evaluates to a fresh String value and renders as usual. The match is on the DIRECT node only: a Typed node wrapping a string literal (or a variable) is not this rule -- the Typed arm delegates to the typed emitter, whose own ownership rule (slice 3) decides. 'kind' is an IRNodeKindCode, 'lit' an IRLiteralKindCode (meaningful only when kind is 9). Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_concat_left_owned(kind: i64, lit: i64) -> bool {
    match kind {
        9 => {
            match lit {
                3 => {
                    true
                },
                _ => {
                    false
                },
            }
        },
        10 => {
            true
        },
        _ => {
            false
        },
    }
}
}

pub(crate) mod rust_concat_left_text {
//! The left Concat operand's final text (concat_owned_operand, src/codegen/mod.rs): when rust_concat_left_owned fires, the operand's OWNED FORM, arriving pre-rendered as 'owned_text' -- formed per shape: for a String literal, the owned literal template (rust_string_literal_owned_text, the quoted text under .to_string()); for a variable, the clone text (the variable's rendered name under .clone() -- the same text slice 4's rust_unary_text pins for the Clone operator, op 9; Phase 39). Otherwise the operand's own rendering, unchanged. Both parts arrive pre-rendered; which applies is the leaf check's Bool. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_concat_left_text(owned: bool, rendered: String, owned_text: String) -> String {
    match owned {
        true => {
            owned_text
        },
        false => {
            rendered
        },
        _ => {
            "".to_string()
        },
    }
}
}

pub(crate) mod rust_concat_right_form {
//! Emission family of the RIGHT operand of a Concat (concat_borrowed_operand, src/codegen/mod.rs:640-651; Phase 29 defect 2a -- String + String is E0308, so the right operand is borrowed): 0 = bare (the operand is a String literal: already a &str; borrowing it would produce a &&str, which has no Add impl), 1 = parenthesized borrow (the block-like kinds -- the is_block_like_expr set, src/codegen/mod.rs:653-666: Block (1), Let (2), If (3), While (4), For (5), Match (6), Lambda (13), Closure (14), Loop (24), StructDef (26)), 2 = plain borrow (every other kind). 'kind' is an IRNodeKindCode, 'lit' an IRLiteralKindCode (meaningful only when kind is 9: kind 9 with lit 3 is family 0, kind 9 with any other lit is family 2). Total over all 28 kinds x 6 literal kinds. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_concat_right_form(kind: i64, lit: i64) -> i64 {
    match kind {
        1 => {
            1
        },
        2 => {
            1
        },
        3 => {
            1
        },
        4 => {
            1
        },
        5 => {
            1
        },
        6 => {
            1
        },
        9 => {
            match lit {
                3 => {
                    0
                },
                _ => {
                    2
                },
            }
        },
        13 => {
            1
        },
        14 => {
            1
        },
        24 => {
            1
        },
        26 => {
            1
        },
        _ => {
            2
        },
    }
}
}

pub(crate) mod rust_concat_right_text {
//! The right Concat operand's final text, by rust_concat_right_form: family 0 renders bare (the operand's own rendering), family 1 renders '&(' + rendering + ')', family 2 renders '&' + rendering. A nested Concat operand (kind 11, family 2) renders '&' + its already-parenthesized template -- textually equal to family 1's form for that operand; the families are the emitter's decision procedure (concat_borrowed_operand's match order), not distinct texts. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_concat_right_text(form: i64, rendered: String) -> String {
    match form {
        0 => {
            rendered
        },
        1 => {
            (("&(".to_string() + &rendered) + ")")
        },
        2 => {
            ("&".to_string() + &rendered)
        },
        _ => {
            "".to_string()
        },
    }
}
}

pub(crate) mod rust_unary_text {
//! The full rendering of a UnaryOp node (src/codegen/mod.rs:789-809) over the operand's own rendering: Neg '(- x)' and Not '(! x)' (the prefix family of rust_unaryop_form, symbols from rust_unaryop_symbol); Abs/Len/Reverse/Clone 'x.m()' (the method family, names from rust_unaryop_method -- Reverse's call returns () in Rust: a defect of this template, pinned as-is, not of the classification); Head '(x)[0]', Tail '(x)[1..].to_vec()', Init '(x)[..(x).len() - 1].to_vec()', Last '(x)[(x).len() - 1]' (the list-part family: indexing/slicing lowerings whose operand text is emitted TWICE for Init and Last, sound for the IR's pure expression forms). Total over all ten IRUnaryOp codes. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_unary_text(op: i64, inner: String) -> String {
    match op {
        0 => {
            (("(- ".to_string() + &inner) + ")")
        },
        1 => {
            (("(! ".to_string() + &inner) + ")")
        },
        2 => {
            (inner.clone() + ".abs()")
        },
        3 => {
            (inner.clone() + ".len()")
        },
        4 => {
            (("(".to_string() + &inner) + ")[0]")
        },
        5 => {
            (("(".to_string() + &inner) + ")[1..].to_vec()")
        },
        6 => {
            (((("(".to_string() + &inner) + ")[..(") + &inner) + ").len() - 1].to_vec()")
        },
        7 => {
            (((("(".to_string() + &inner) + ")[(") + &inner) + ").len() - 1]")
        },
        8 => {
            (inner.clone() + ".reverse()")
        },
        9 => {
            (inner.clone() + ".clone()")
        },
        _ => {
            "".to_string()
        },
    }
}
}

pub(crate) mod rust_collection_prefix {
//! Fixed opening text of a collection literal in EXPRESSION position (the Collection arm of ir_to_rust_expr, src/codegen/mod.rs:838-852), keyed by CollectionKind code: List 'vec![', Set 'std::collections::HashSet::from([', Map 'std::collections::HashMap::from([', Array '[', Tuple '('. Set and Map are FULLY QUALIFIED -- no import is generated for winner files, so the bare paths would fail E0433 (contrast ty_to_rust's unqualified 'HashSet<' in type position, rust_type_prefix). The family classification is rust_collection_form (slice 1); the elements arrive pre-joined with ', ' (the join is iteration, mirror-only) and the closing text is rust_collection_suffix. Statement position is a different rule (slice 5): List, Set and Map all collapse to the vec![..] form there.

pub fn rust_collection_prefix(kind: i64) -> String {
    match kind {
        0 => {
            "vec![".to_string()
        },
        1 => {
            "std::collections::HashSet::from([".to_string()
        },
        2 => {
            "std::collections::HashMap::from([".to_string()
        },
        3 => {
            "[".to_string()
        },
        4 => {
            "(".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_collection_suffix {
//! Fixed closing text of a collection literal in expression position (src/codegen/mod.rs:838-852), keyed by CollectionKind code: List ']', Set '])', Map '])', Array ']', Tuple ')'. Pairs with rust_collection_prefix.

pub fn rust_collection_suffix(kind: i64) -> String {
    match kind {
        0 => {
            "]".to_string()
        },
        1 => {
            "])".to_string()
        },
        2 => {
            "])".to_string()
        },
        3 => {
            "]".to_string()
        },
        4 => {
            ")".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_map_text {
//! The Map rendering of ir_to_rust_expr (src/codegen/mod.rs:853-859): '(' + over + ').iter().map(' + function + ').collect()'. Both children arrive rendered by ir_to_rust_expr itself. Note the collect has no type annotation, so the result type is inferred at the use site. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_map_text(over: String, func: String) -> String {
    (((("(".to_string() + &over) + ").iter().map(") + &func) + ").collect()")
}
}

pub(crate) mod rust_filter_text {
//! The Filter rendering of ir_to_rust_expr (src/codegen/mod.rs:860-866): '(' + over + ').iter().filter(' + predicate + ').collect()'. The predicate is rendered by ir_to_rust_expr and passed as the closure value; filter hands it &Self::Item, so the emitted predicate receives a reference -- the known Vec<&T> flavor of this template's output (pinned as-is; the template is what this spec states). Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_filter_text(over: String, pred: String) -> String {
    (((("(".to_string() + &over) + ").iter().filter(") + &pred) + ").collect()")
}
}

pub(crate) mod rust_fold_text {
//! The Fold rendering of ir_to_rust_expr (src/codegen/mod.rs:867-873): '(' + over + ').iter().fold(' + initial + ', ' + operation + ')'. All three children arrive rendered by ir_to_rust_expr itself; note the fold argument order in the text is (initial, operation) although the IR node fields are declared (operation, initial, over). Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_fold_text(over: String, initial: String, operation: String) -> String {
    (((((("(".to_string() + &over) + ").iter().fold(") + &initial) + ", ") + &operation) + ")")
}
}

pub(crate) mod rust_sort_text {
//! The Sort rendering of ir_to_rust_expr (src/codegen/mod.rs:874-882): the statement form (bind, sort, yield) wrapped as a block expression so the sorted value is the value -- '{ let mut sorted = ' + collection + '; sorted.sort_by(' + comparator + '); sorted }'. Both children arrive rendered by ir_to_rust_expr itself. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_sort_text(coll: String, cmp: String) -> String {
    (((("{ let mut sorted = ".to_string() + &coll) + "; sorted.sort_by(") + &cmp) + "); sorted }")
}
}

pub(crate) mod rust_call_text {
//! The Call rendering of ir_to_rust_expr (src/codegen/mod.rs:810-813): the rust_ident-rendered function name + '(' + the argument renderings joined ', ' + ')'. The name arrives rendered (slice 3 owns rust_ident) and the arguments pre-joined (the join is iteration, mirror-only); each argument is rendered by ir_to_rust_expr itself, UNTYPED -- a String argument renders as a bare &str literal even where the callee takes String (the same placement property as rust_test_case's, one walker down). Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_call_text(func: String, args: String) -> String {
    (((func.clone() + "(") + &args) + ")")
}
}

pub(crate) mod rust_condition_text {
//! expr_in_condition_position (src/codegen/mod.rs:734-740) assembled: a block-like node's rendering is wrapped in parentheses, every other rendering passes through bare -- stated as a composition over rust_condition_wrap (slice 2), the classification the original consults. Serves the If arm's condition and, as the base rendering, the Match arm's scrutinee (rust_match_scrutinee_text). Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_condition_text(kind: i64, rendered: String) -> String {
    match ((((((((((kind == 1) || (kind == 2)) || (kind == 3)) || (kind == 4)) || (kind == 5)) || (kind == 6)) || (kind == 13)) || (kind == 14)) || (kind == 24)) || (kind == 26)) {
        true => {
            (("(".to_string() + &rendered) + ")")
        },
        false => {
            rendered
        },
        _ => {
            "".to_string()
        },
    }
}
}

pub(crate) mod rust_if_text {
//! The If rendering of ir_to_rust_expr (src/codegen/mod.rs:814-822): 'if ' + condition + ' { ' + then + ' } else { ' + else + ' }'. The condition arrives condition-position-rendered (rust_condition_text); the branches are rendered by ir_to_rust_expr itself, UNTYPED -- the typed dispatcher's ownership rewrite does not reach them (slice 3's rust_typed_form owns that boundary). Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_if_text(cond: String, then_text: String, else_text: String) -> String {
    (((((("if ".to_string() + &cond) + " { ") + &then_text) + " } else { ") + &else_text) + " }")
}
}

pub(crate) mod rust_lambda_text {
//! The Lambda/Closure rendering of ir_to_rust_expr (src/codegen/mod.rs:823-832): '|' + the parameter names joined ', ' + '| { ' + body + ' }'. Lambda and Closure share ONE arm -- a Closure's captured environment is not rendered at all. The names arrive rust_ident-rendered and pre-joined (the join is iteration, mirror-only); the parameters' TYPES are dropped by the arm (the generated closure is unannotated). The body is rendered by ir_to_rust_expr itself. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_lambda_text(params: String, body: String) -> String {
    (((("|".to_string() + &params) + "| { ") + &body) + " }")
}
}

pub(crate) mod rust_match_arm_text {
//! One arm of the Match rendering (src/codegen/mod.rs:883-890): pattern + ' => ' + body. The pattern arrives rendered by ir_pattern_to_rust (slice 2 owns that walker) and the body by ir_to_rust_expr itself. The arms are joined ', ' by the Match template (the join is iteration, mirror-only). Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_match_arm_text(pat: String, body: String) -> String {
    ((pat.clone() + " => ") + &body)
}
}

pub(crate) mod rust_match_text {
//! The Match rendering of ir_to_rust_expr (src/codegen/mod.rs:883-898): 'match ' + scrutinee + ' { ' + arms + ' }'. The scrutinee arrives rendered by match_scrutinee_to_rust (rust_match_scrutinee_text over the rust_condition_text base) and the arms pre-joined ', ' (rust_match_arm_text each). Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_match_text(scrutinee: String, arms: String) -> String {
    (((("match ".to_string() + &scrutinee) + " { ") + &arms) + " }")
}
}

pub(crate) mod rust_match_scrutinee_bare {
//! Whether a match scrutinee is itself a bare String literal (is_bare_string_literal, src/codegen/mod.rs:708-714): directly a Literal node (kind 9) of literal kind String (3). The original recurses through Typed wrappers -- a Typed scrutinee's bareness is its inner node's, which the flat (kind, lit) codes do not carry -- so kind 22 (Typed) is deliberately unpinned here and decided by the mirror's recursion. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_match_scrutinee_bare(kind: i64, lit: i64) -> bool {
    match kind {
        9 => {
            match lit {
                3 => {
                    true
                },
                _ => {
                    false
                },
            }
        },
        _ => {
            false
        },
    }
}
}

pub(crate) mod rust_match_scrutinee_form {
//! Whether match_scrutinee_to_rust (src/codegen/mod.rs:683-695; Phase 29 defect 2b) appends '.as_str()' to the scrutinee's condition-position rendering: 1 exactly when some case pattern is or contains a String literal (has_string_pat) AND the scrutinee is not itself a bare String literal (rust_match_scrutinee_bare) -- a literal scrutinee is already a &str, its patterns already match, and &str has no stable as_str() (E0658). 0 otherwise. has_string_pat is a property of the case patterns' TREE (pattern_has_string_literal recurses through Constructor and Guard patterns, src/codegen/mod.rs:697-706); patterns arrive pre-rendered in this slice's representation, so the Bool arrives precomputed -- the recursion is mirror-only (see the slice-4 notes). The rule is syntactic (the emitter carries no types); its recorded blind spot is a scrutinee variable let-bound to a bare literal. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_match_scrutinee_form(has_string_pat: bool, bare: bool) -> i64 {
    match has_string_pat {
        false => {
            0
        },
        true => {
            match bare {
                false => {
                    1
                },
                true => {
                    0
                },
                _ => {
                    0
                },
            }
        },
        _ => {
            0
        },
    }
}
}

pub(crate) mod rust_match_scrutinee_text {
//! The match scrutinee's final text (match_scrutinee_to_rust, src/codegen/mod.rs:683-695): the condition-position rendering (rust_condition_text), with '.as_str()' appended exactly when rust_match_scrutinee_form is 1. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_match_scrutinee_text(form: i64, rendered: String) -> String {
    match form {
        0 => {
            rendered
        },
        1 => {
            (rendered.clone() + ".as_str()")
        },
        _ => {
            "".to_string()
        },
    }
}
}

pub(crate) mod rust_block_text {
//! The block-expression wrap shared by two sites with one outer form: the Block arm of ir_to_rust_expr (src/codegen/mod.rs:900-913) over the assembled statement renderings, and expr_block_stmt (src/codegen/mod.rs:716-719) wrapping a single statement rendering -- the form the Let/While/For arms of ir_to_rust_expr (src/codegen/mod.rs:914-916) take. '{ ' + inner + ' }' in both. The inner arrives rendered AND end-trimmed (both sites trim_end the statement rendering) in a neutral Unit return-type context; the statement rendering itself is the statement-walker slice (slice 5), whose only loss here is the String-ownership rewrite in nested return/tail positions. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_block_text(inner: String) -> String {
    (("{ ".to_string() + &inner) + " }")
}
}

pub(crate) mod rust_loop_text {
//! The Loop rendering of ir_to_rust_expr (src/codegen/mod.rs:917-922): 'loop {' + a newline + the statement-rendered body + '}'. Note there is NO space before the closing brace and none after the newline -- the body's own indentation (or lack of it: the statement walker renders at indent 0 here) is entirely the body's. The body arrives rendered and end-trimmed by the statement walker (slice 5) in a neutral Unit return-type context.

pub fn rust_loop_text(body: String) -> String {
    (("loop {\n".to_string() + &body) + "}")
}
}

pub(crate) mod rust_struct_def_text {
//! The StructDef rendering of ir_to_rust_expr (src/codegen/mod.rs:923-935): '{ struct ' + name + ' { ' + fields + ' } ' + body + ' }' -- an item definition followed by the body that uses it, inside one block expression (Rust allows item declarations inside a block). The name arrives rust_ident-rendered; the fields pre-joined ', ', each field 'name: type' by the same template as rust_param_text (the original formats both with '{}: {}' over rust_ident and ty_to_rust); the body is rendered by ir_to_rust_expr itself. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_struct_def_text(name: String, fields: String, body: String) -> String {
    (((((("{ struct ".to_string() + &name) + " { ") + &fields) + " } ") + &body) + " }")
}
}

pub(crate) mod rust_index_text {
//! The Index rendering of ir_to_rust_expr (src/codegen/mod.rs:936-938): '(' + collection + ')[' + index + ']'. The collection is parenthesized, the index is not. Both children arrive rendered by ir_to_rust_expr itself. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_index_text(coll: String, idx: String) -> String {
    (((("(".to_string() + &coll) + ")[") + &idx) + "]")
}
}

pub(crate) mod rust_try_text {
//! The TryPropagate rendering of ir_to_rust_expr (src/codegen/mod.rs:939-941): '(' + inner + ')?'. The operand is parenthesized before the '?' so a composite operand's boundary survives. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_try_text(inner: String) -> String {
    (("(".to_string() + &inner) + ")?")
}
}

pub(crate) mod rust_assign_text {
//! The Assign rendering of ir_to_rust_expr (src/codegen/mod.rs:942-948): '(' + target + ' = ' + value + ')'. Both children arrive rendered by ir_to_rust_expr itself; the whole assignment is parenthesized because an assignment expression's value is () and its boundary must survive embedding. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_assign_text(target: String, value: String) -> String {
    (((("(".to_string() + &target) + " = ") + &value) + ")")
}
}

pub(crate) mod rust_return_text {
//! The Return rendering of ir_to_rust_expr (src/codegen/mod.rs:949-954): '(return ' + inner + ')'. Parenthesized because return greedily consumes a following expression -- '(return 5) + 1' must not become 'return (5 + 1)'. The operand is rendered by ir_to_rust_expr itself, UNTYPED: a returned String literal stays bare here; the ownership rewrite for return positions lives in the statement walker (ir_to_rust's Return arm consults fn_ret, slice 5). Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_return_text(inner: String) -> String {
    (("(return ".to_string() + &inner) + ")")
}
}

pub(crate) mod rust_break_text {
//! The Break rendering of ir_to_rust_expr (src/codegen/mod.rs:955-958): with a value, '(break ' + value + ')'; without, the fixed text '(break)'. Parenthesized for the same greedy-consumption reason as rust_return_text. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_break_text(has_value: bool, value: String) -> String {
    match has_value {
        false => {
            "(break)".to_string()
        },
        true => {
            (("(break ".to_string() + &value) + ")")
        },
        _ => {
            "".to_string()
        },
    }
}
}

pub(crate) mod rust_hole_text {
//! The fixed text of the Hole arm of ir_to_rust_expr (src/codegen/mod.rs:959): 'panic!("Hole not filled")'. panic! has type ! and coerces anywhere, so the generated file still parses and compiles with the gap loud instead of silent (the Phase 13 convention). Keyed by IRNode kind like the other fixed-text tables: only Hole (0) has this text.

pub fn rust_hole_text(kind: i64) -> String {
    match kind {
        0 => {
            "panic!(\"Hole not filled\")".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_unsupported_text {
//! The loud fallback for a shape with no honest expression form (unsupported_expr, src/codegen/mod.rs:725-727): 'panic!("unsupported IR in expression position: ' + shape + '")'. In THIS walker exactly one arm uses it -- Alloc (an Alloc binds a name for later statements and has no value of its own), with shape 'Alloc'; the statement walker has further call sites (slice 5). Following the Hole convention, the panic! keeps the generated file parseable with the gap named.

pub fn rust_unsupported_text(shape: String) -> String {
    (("panic!(\"unsupported IR in expression position: ".to_string() + &shape) + "\")")
}

}

pub(crate) mod rust_stmt_form {
//! Dispatch family of ir_to_rust (src/codegen/mod.rs:310-498), the statement walker: which arm renders a node in statement position. 0 = Hole (the two-line TODO + panic), 1 = Block (concatenation of the statements, no template of its own), 2 = Let, 3 = If, 4 = While, 5 = For, 6 = Match, 7 = Call (terminated), 8 = Return, 9 = Alloc, 10 = Assign, 11 = Collection (statement form, rust_stmt_collection_text), 12 = Map, 13 = Filter, 14 = Fold, 15 = Sort (three-line statement form), 16 = Typed (transparent recursion -- the annotation is dropped in statement position), 17 = the fallback: the node's ir_to_rust_expr rendering terminated with ';' (Literal, Var, BinOp, UnaryOp, Lambda, Closure, Index, Loop, Break, StructDef, TryPropagate -- expression shapes in statement position, Loop and StructDef included: they keep their expression renderings). Arm codes are in the original's source order. Total over all 28 IRNode kinds; a kind outside the 28 falls to the fallback arm exactly as the original's '_' arm catches it.

pub fn rust_stmt_form(kind: i64) -> i64 {
    match kind {
        0 => {
            0
        },
        1 => {
            1
        },
        2 => {
            2
        },
        3 => {
            3
        },
        4 => {
            4
        },
        5 => {
            5
        },
        6 => {
            6
        },
        7 => {
            7
        },
        8 => {
            8
        },
        15 => {
            11
        },
        16 => {
            12
        },
        17 => {
            13
        },
        18 => {
            14
        },
        19 => {
            15
        },
        20 => {
            9
        },
        21 => {
            10
        },
        22 => {
            16
        },
        _ => {
            17
        },
    }
}

}

pub(crate) mod rust_tail_form {
//! Dispatch family of ir_to_rust_tail (src/codegen/mod.rs:507-541), the tail-position walker: 0 = statement fallthrough (the node keeps its ir_to_rust statement rendering, which is already tail-shaped where it needs to be: Hole, Let, If, While, For, Match, Return, Alloc, Assign, Loop, Break, StructDef), 1 = bare expression (pad + the typed rendering + a newline, NO semicolon -- the value survives as the enclosing block's value: Literal, Var, BinOp, UnaryOp, Lambda, Closure, Collection, Call, Map, Filter, Fold, Sort, Index, TryPropagate), 2 = Block split (every statement but the last renders as a statement; the last renders as a tail -- rust_stmt_seq_form position 1), 3 = Typed re-entry (the node's own annotation replaces the inherited expectation). Total over all 28 IRNode kinds; a kind outside the 28 takes the statement fallthrough, as the original's '_' arm routes it to ir_to_rust.

pub fn rust_tail_form(kind: i64) -> i64 {
    match kind {
        1 => {
            2
        },
        22 => {
            3
        },
        7 => {
            1
        },
        9 => {
            1
        },
        10 => {
            1
        },
        11 => {
            1
        },
        12 => {
            1
        },
        13 => {
            1
        },
        14 => {
            1
        },
        15 => {
            1
        },
        16 => {
            1
        },
        17 => {
            1
        },
        18 => {
            1
        },
        19 => {
            1
        },
        23 => {
            1
        },
        27 => {
            1
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_stmt_collection_prefix {
//! Fixed opening text of a collection literal in STATEMENT position (the Collection arm of ir_to_rust, src/codegen/mod.rs:418-451), keyed by CollectionKind code: List 'vec![', Set 'vec![', Map 'vec![', Array '[', Tuple '('. Set and Map COLLAPSE to the List form here -- the fully qualified HashSet/HashMap forms of expression position (slice 4's rust_collection_prefix) appear only where the collection's value is used; as a statement the value is discarded, and the original renders all three sequence kinds identically. Pairs with rust_stmt_collection_suffix.

pub fn rust_stmt_collection_prefix(kind: i64) -> String {
    match kind {
        0 => {
            "vec![".to_string()
        },
        1 => {
            "vec![".to_string()
        },
        2 => {
            "vec![".to_string()
        },
        3 => {
            "[".to_string()
        },
        4 => {
            "(".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_stmt_collection_suffix {
//! Fixed closing text of a collection literal in statement position (src/codegen/mod.rs:418-451), keyed by CollectionKind code: List ']', Set ']', Map ']', Array ']', Tuple ')'. The collapse is complete: Set and Map also lose expression position's '])' ending (slice 4's rust_collection_suffix), because the whole rendering is the List form. Pairs with rust_stmt_collection_prefix; the terminating ';' belongs to rust_stmt_collection_text.

pub fn rust_stmt_collection_suffix(kind: i64) -> String {
    match kind {
        0 => {
            "]".to_string()
        },
        1 => {
            "]".to_string()
        },
        2 => {
            "]".to_string()
        },
        3 => {
            "]".to_string()
        },
        4 => {
            ")".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}

}

pub(crate) mod rust_expectation_source {
//! Which expectation each expectation-carrying recursive site of the two statement walkers passes to the typed emitter (ir_to_rust_expr_typed -- slice 3's rust_typed_form dispatch and rust_string_ownership leaf rule consume it). Sources: 0 = Some(fn_ret), the enclosing function's return type; 1 = Some(the node's own declared type); 2 = the inherited expectation, passed through unchanged. Sites: 0 = a Let's body (tail), 1 = an If's then branch (tail), 2 = an If's else branch (tail), 3 = a Match arm's body (tail), 4 = a Return's expression (the statement walker's typed call), 5 = an Alloc's initial value (the Alloc's declared type), 6 = a tail-position Typed node (its annotation overrides the inherited expectation), 7 = a tail Block's last statement (the expectation is inherited unchanged -- the Block itself sets none). Sites outside these two functions are not this table's: generate_rust seeds a function body with Some(return type) (the driver, slice 6), and expr_block_stmt's inner tail passes None (the expression walker's Block arm, slice 4).

pub fn rust_expectation_source(site: i64) -> i64 {
    match site {
        5 => {
            1
        },
        6 => {
            1
        },
        7 => {
            2
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_stmt_child_indent {
//! The relative indent (in levels of four spaces) at which each part of a statement rendering is placed, over the placement sites of ir_to_rust and ir_to_rust_tail (src/codegen/mod.rs:310-541). The pad STRING is '    ' repeated (indent + delta) times -- the repetition is iteration and mirror-only; this table pins the arithmetic. Sites: 0 = a Block's statement (statement position), 1 = a Let's tail body, 2 = an If branch body (tail), 3 = a While body, 4 = a For body, 5 = a Match arm's header line (the pattern line and the arm's closing '},' -- the pad_inner lines), 6 = a Match arm's body (tail), 7 = a tail Block's non-last statement, 8 = a tail Block's last statement (tail). Everything else in a rendering sits at the node's own indent (delta 0 by the templates' use of pad): the header lines, and the While/For/Match closing braces. Two placements take NO pad at all -- the If arm's closing braces and the Sort arm's continuation lines; those are fixed-text facts of rust_stmt_if_text and rust_stmt_sort_text, not rows of this table. expr_block_stmt renders its inner statement at absolute indent 0 in a neutral Unit context -- the expression walker's site (slice 4), not a row here.

pub fn rust_stmt_child_indent(site: i64) -> i64 {
    match site {
        2 => {
            1
        },
        3 => {
            1
        },
        4 => {
            1
        },
        5 => {
            1
        },
        6 => {
            2
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_stmt_seq_form {
//! Which renderer renders one statement of a Block, by the Block's position and the statement's place in it: 0 = the statement walker (ir_to_rust), 1 = the tail walker (ir_to_rust_tail). Positions: 0 = statement position (ir_to_rust's Block arm -- EVERY statement renders as a statement, last included: the Block's own value is discarded with the enclosing statement's), 1 = tail position (ir_to_rust_tail's Block arm -- every statement but the last renders as a statement; the last renders as a tail, so its value becomes the block's). A position code other than 1 takes the statement renderer, as every non-tail context does. The concatenation of the chosen renderings is the Block's rendering (mirror-only iteration; see the slice-5 notes).

pub fn rust_stmt_seq_form(position: i64, last: bool) -> i64 {
    match position {
        1 => {
            match last {
                true => {
                    1
                },
                _ => {
                    0
                },
            }
        },
        _ => {
            0
        },
    }
}
}

pub(crate) mod rust_stmt_hole_text {
//! The Hole arm of ir_to_rust (src/codegen/mod.rs:316-319): TWO lines at the node's indent -- a TODO comment, then the loud panic terminated as a statement. The panic text is the same fixed text as the expression walker's Hole (slice 4's rust_hole_text), but the statement arm is its own format site: it adds the comment line, pads both lines, and terminates with ';'. Following the Hole convention the panic keeps the generated file parseable with the gap named.

pub fn rust_stmt_hole_text(pad: String) -> String {
    (((pad.clone() + "// TODO: synthesis incomplete\n") + &pad) + "panic!(\"Hole not filled\");\n")
}

}

pub(crate) mod rust_stmt_let_text {
//! The Let arm of ir_to_rust (src/codegen/mod.rs:328-337): pad + 'let ' + name + ' = ' + value + ';' + a newline, followed IMMEDIATELY by the body's tail rendering (the Let has no closing line of its own -- the body IS the Let's continuation at the same indent, rust_stmt_child_indent site 1). The name arrives rust_ident-rendered (slice 3); the value arrives rendered by ir_to_rust_expr UNTYPED (a String literal value stays bare -- the Let value position carries no expectation); the body arrives rendered by ir_to_rust_tail with expectation Some(fn_ret) (rust_expectation_source site 0), so a bare-expression body yields its value.

pub fn rust_stmt_let_text(pad: String, name: String, value: String, body: String) -> String {
    ((((((pad.clone() + "let ") + &name) + " = ") + &value) + ";\n") + &body)
}
}

pub(crate) mod rust_stmt_if_text {
//! The If arm of ir_to_rust (src/codegen/mod.rs:339-349): pad + 'if ' + condition + ' {' + newline + the then branch's tail rendering + '} else {' + newline + the else branch's tail rendering + '}' + newline. The condition arrives condition-position-rendered (slice 4's rust_condition_text); the branches arrive tail-rendered at indent + 1 with expectation Some(fn_ret) (rust_stmt_child_indent site 2, rust_expectation_source sites 1-2). QUIRK, pinned as-is: the two closing braces carry NO pad -- the format string interpolates no indent before them, unlike the While/For/Match arms, whose closings sit at pad. At indent 0 the difference is invisible; nested, an If's braces land at column 0.

pub fn rust_stmt_if_text(pad: String, cond: String, then_text: String, else_text: String) -> String {
    (((((((pad.clone() + "if ") + &cond) + " {\n") + &then_text) + "} else {\n") + &else_text) + "}\n")
}
}

pub(crate) mod rust_stmt_while_text {
//! The While arm of ir_to_rust (src/codegen/mod.rs:352-362): with an invariant, a comment line at pad ('// invariant: ' + the invariant text, which arrives RAW from the IR -- no walker renders it) precedes the loop; then pad + 'while ' + condition + ' {' + newline + the body's statement rendering at indent + 1 (rust_stmt_child_indent site 3 -- a STATEMENT rendering, not a tail: a while body yields nothing) + pad + '}' + newline. The condition arrives condition-position-rendered (slice 4's rust_condition_text). Unlike the If arm, the closing brace sits at pad.

pub fn rust_stmt_while_text(has_inv: bool, inv: String, pad: String, cond: String, body: String) -> String {
    match has_inv {
        true => {
            ((((((((((pad.clone() + "// invariant: ") + &inv) + "\n") + &pad) + "while ") + &cond) + " {\n") + &body) + &pad) + "}\n")
        },
        false => {
            ((((((pad.clone() + "while ") + &cond) + " {\n") + &body) + &pad) + "}\n")
        },
        _ => {
            "".to_string()
        },
    }
}
}

pub(crate) mod rust_stmt_for_text {
//! The For arm of ir_to_rust (src/codegen/mod.rs:364-373): pad + 'for ' + var + ' in ' + start + '..' + end + ' {' + newline + the body's statement rendering at indent + 1 (rust_stmt_child_indent site 4 -- statement, not tail, as with While) + pad + '}' + newline. The variable arrives rust_ident-rendered (slice 3); the bounds arrive rendered by ir_to_rust_expr, the range unparenthesized and exclusive, exactly as the original formats it.

pub fn rust_stmt_for_text(pad: String, var: String, start: String, end: String, body: String) -> String {
    ((((((((((pad.clone() + "for ") + &var) + " in ") + &start) + "..") + &end) + " {\n") + &body) + &pad) + "}\n")
}
}

pub(crate) mod rust_stmt_match_head_text {
//! The head line of the Match arm of ir_to_rust (src/codegen/mod.rs:376): pad + 'match ' + scrutinee + ' {' + newline. The scrutinee arrives rendered by match_scrutinee_to_rust (slice 4's scrutinee trio over the condition-position base -- the same helper both Match arms consult). The arm lines and the closing line are rust_stmt_match_arm_text and rust_stmt_match_close_text; joining the arms is iteration (mirror-only).

pub fn rust_stmt_match_head_text(pad: String, scrutinee: String) -> String {
    (((pad.clone() + "match ") + &scrutinee) + " {\n")
}
}

pub(crate) mod rust_stmt_match_arm_text {
//! One case line-group of the Match arm of ir_to_rust (src/codegen/mod.rs:378-384): pad_inner + pattern + ' => {' + newline + the body's tail rendering at indent + 2 + pad_inner + '},' + newline. pad_inner (the pad of indent + 1) frames the arm; the body sits one level deeper still (rust_stmt_child_indent sites 5 and 6). The pattern arrives rendered by ir_pattern_to_rust (slice 2); the body arrives tail-rendered with expectation Some(fn_ret) (rust_expectation_source site 3), so a bare-expression arm body yields its value inside the braces. Not slice 4's rust_match_arm_text: that is the EXPRESSION match's one-line 'pattern => body' arm; the statement match wraps each body in its own braces.

pub fn rust_stmt_match_arm_text(pad_inner: String, pat: String, body: String) -> String {
    (((((pad_inner.clone() + &pat) + " => {\n") + &body) + &pad_inner) + "},\n")
}
}

pub(crate) mod rust_stmt_match_close_text {
//! The closing line of the Match arm of ir_to_rust (src/codegen/mod.rs:386): pad + '}' + newline -- at the Match's own indent, like the While/For closings (and unlike the If arm's unpadded braces).

pub fn rust_stmt_match_close_text(pad: String) -> String {
    (pad.clone() + "}\n")
}

}

pub(crate) mod rust_stmt_call_text {
//! The Call arm of ir_to_rust (src/codegen/mod.rs:389-398): pad + function + '(' + arguments + ');' + newline -- the expression call template (slice 4's rust_call_text) terminated as a statement, its value discarded. The function name arrives rust_ident-rendered (slice 3) and the arguments pre-joined with ', ' (the join is iteration, mirror-only); each argument is rendered by ir_to_rust_expr. In tail position the same call renders WITHOUT the ';' (rust_tail_form family 1) -- the pair is the recorded fix for the unterminated-statement syntax error.

pub fn rust_stmt_call_text(pad: String, func: String, args: String) -> String {
    ((((pad.clone() + &func) + "(") + &args) + ");\n")
}
}

pub(crate) mod rust_stmt_return_text {
//! The Return arm of ir_to_rust (src/codegen/mod.rs:400-402): pad + 'return ' + rendered + ';' + newline. The rendered expression arrives from ir_to_rust_expr_typed with expectation Some(fn_ret) (rust_expectation_source site 4) -- this arm is the statement walker's own typed call, the one through which a String literal in a String-returning function takes the owned form (slice 3's rust_string_ownership decides, rust_string_literal_owned_text renders). Not slice 4's rust_return_text: that is the expression walker's parenthesized '(return x)', whose operand is rendered UNTYPED.

pub fn rust_stmt_return_text(pad: String, rendered: String) -> String {
    (((pad.clone() + "return ") + &rendered) + ";\n")
}
}

pub(crate) mod rust_stmt_alloc_text {
//! The Alloc arm of ir_to_rust (src/codegen/mod.rs:404-409): pad + 'let mut ' + name + ': ' + type + ' = ' + initial + ';' + newline. The name arrives rust_ident-rendered (slice 3), the type rendered by ty_to_rust (slices 1-2), and the initial value rendered by ir_to_rust_expr_typed with expectation Some(the Alloc's OWN declared type) (rust_expectation_source site 5) -- so a String-typed Alloc's String literal initial takes the owned form, where a Let's value would stay bare. In expression position an Alloc has no honest form at all (slice 4's rust_unsupported_text); statement position is its only rendering.

pub fn rust_stmt_alloc_text(pad: String, name: String, ty: String, initial: String) -> String {
    (((((((pad.clone() + "let mut ") + &name) + ": ") + &ty) + " = ") + &initial) + ";\n")
}
}

pub(crate) mod rust_stmt_assign_text {
//! The Assign arm of ir_to_rust (src/codegen/mod.rs:411-416): pad + target + ' = ' + value + ';' + newline. Both children arrive rendered by ir_to_rust_expr. Not slice 4's rust_assign_text: the expression form is parenthesized ('(t = v)') because an assignment's boundary must survive embedding; the statement form is bare, as a statement needs no boundary.

pub fn rust_stmt_assign_text(pad: String, target: String, value: String) -> String {
    ((((pad.clone() + &target) + " = ") + &value) + ";\n")
}
}

pub(crate) mod rust_stmt_collection_text {
//! The Collection arm of ir_to_rust (src/codegen/mod.rs:418-451) assembled: pad + the statement-position prefix + the elements + the statement-position suffix + ';' + newline, stated as a composition over rust_stmt_collection_prefix / rust_stmt_collection_suffix, the classifications the arm consults. The elements arrive pre-joined with ', ' (the join is iteration, mirror-only), each rendered by ir_to_rust_expr. The ';' is the recorded fix for the unterminated-collection syntax error; in tail position the same collection renders bare in its EXPRESSION form (fully qualified for Set/Map -- rust_tail_form family 1 over slice 4's affixes), never this collapsed one.

pub fn rust_stmt_collection_text(kind: i64, pad: String, elems: String) -> String {
    ((((pad.clone() + &(match kind { 0 => "vec![", 1 => "vec![", 2 => "vec![", 3 => "[", 4 => "(", _ => "" })) + &elems) + &(match kind { 0 => "]", 1 => "]", 2 => "]", 3 => "]", 4 => ")", _ => "" })) + ";\n")
}
}

pub(crate) mod rust_stmt_map_text {
//! The Map arm of ir_to_rust (src/codegen/mod.rs:453-460): pad + '(' + over + ').iter().map(' + function + ').collect();' + newline -- slice 4's rust_map_text template terminated as a statement, its collected value discarded. Both children arrive rendered by ir_to_rust_expr.

pub fn rust_stmt_map_text(pad: String, over: String, func: String) -> String {
    (((((pad.clone() + "(") + &over) + ").iter().map(") + &func) + ").collect();\n")
}
}

pub(crate) mod rust_stmt_filter_text {
//! The Filter arm of ir_to_rust (src/codegen/mod.rs:462-469): pad + '(' + over + ').iter().filter(' + predicate + ').collect();' + newline -- slice 4's rust_filter_text template terminated as a statement. Both children arrive rendered by ir_to_rust_expr (the predicate with the same reference-receiving property slice 4 recorded).

pub fn rust_stmt_filter_text(pad: String, over: String, pred: String) -> String {
    (((((pad.clone() + "(") + &over) + ").iter().filter(") + &pred) + ").collect();\n")
}
}

pub(crate) mod rust_stmt_fold_text {
//! The Fold arm of ir_to_rust (src/codegen/mod.rs:471-479): pad + '(' + over + ').iter().fold(' + initial + ', ' + operation + ');' + newline -- slice 4's rust_fold_text template terminated as a statement, with the same (initial, operation) text order against the IR's (operation, initial, over) field order. All three children arrive rendered by ir_to_rust_expr.

pub fn rust_stmt_fold_text(pad: String, over: String, initial: String, operation: String) -> String {
    (((((((pad.clone() + "(") + &over) + ").iter().fold(") + &initial) + ", ") + &operation) + ");\n")
}
}

pub(crate) mod rust_stmt_sort_text {
//! The Sort arm of ir_to_rust (src/codegen/mod.rs:481-491): THREE lines -- pad + 'let mut sorted = ' + collection + ';' + newline, then 'sorted.sort_by(' + comparator + ');' + newline, then 'sorted;' + newline, the final value discarded by its own ';'. QUIRK, pinned as-is: only the first line carries the pad; the sort_by and sorted lines are interpolated with NO indent at any depth (the format string has no pad for them), unlike every other multi-line arm. Both children arrive rendered by ir_to_rust_expr. In tail position a Sort instead takes slice 4's rust_sort_text block-expression form, which KEEPS the value -- the statement/tail pair is the sharpest instance of the position deciding the rendering.

pub fn rust_stmt_sort_text(pad: String, coll: String, cmp: String) -> String {
    (((((((pad.clone() + "let mut sorted = ") + &coll) + ";\n") + "sorted.sort_by(") + &cmp) + ");\n") + "sorted;\n")
}
}

pub(crate) mod rust_stmt_expr_text {
//! The fallback arm of ir_to_rust (src/codegen/mod.rs:496): pad + the node's ir_to_rust_expr rendering + ';' + newline. This arm serves exactly rust_stmt_form family 17 -- the expression shapes in statement position (Literal, Var, BinOp, UnaryOp, Lambda, Closure, Index, Loop, Break, StructDef, TryPropagate): each keeps its expression rendering whole (a statement Loop keeps the 'loop {...}' block form; a statement StructDef keeps its block-wrapped definition) and is merely terminated. The ';' after a block-form rendering ('};') is part of the pinned text.

pub fn rust_stmt_expr_text(pad: String, rendered: String) -> String {
    ((pad.clone() + &rendered) + ";\n")
}
}

pub(crate) mod rust_tail_expr_text {
//! The bare-expression arm of ir_to_rust_tail (src/codegen/mod.rs:529-538): pad + the typed rendering + a newline -- NO semicolon, so the value becomes the enclosing block's value (a ';' here would discard it to () and fail the function's return type, E0308 -- the defect this walker exists to fix). Serves rust_tail_form family 1 (Literal, Var, BinOp, UnaryOp, Lambda, Closure, Collection, Call, Map, Filter, Fold, Sort, Index, TryPropagate). The rendering arrives from ir_to_rust_expr_typed under the position's expectation (rust_expectation_source: fn_ret at the Let's/If's/Match's value positions, the Typed node's own annotation inside one, the inherited expectation through a tail Block's last statement) -- slice 3's rust_string_ownership is the one rewrite that expectation drives.

pub fn rust_tail_expr_text(pad: String, rendered: String) -> String {
    ((pad.clone() + &rendered) + "\n")
}
}

pub(crate) mod rust_tail_block_text {
//! The Block arm of ir_to_rust_tail (src/codegen/mod.rs:514-524) assembled: the non-last statements' renderings (each by ir_to_rust, rust_stmt_seq_form position 1) concatenated with the last statement's tail rendering -- a plain concatenation with no separator and no wrap, the two parts arriving pre-rendered. The split itself (all but last vs last) and the iteration are mirror-only; an EMPTY Block renders the empty string (no last statement exists, and the arm emits nothing). The same assembly serves a statement-position Block, whose every statement takes the statement renderer (rust_stmt_seq_form position 0): the parts differ, the concatenation is this one.

pub fn rust_tail_block_text(rest: String, last: String) -> String {
    (rest.clone() + &last)
}
}

pub(crate) mod rust_target_route {
//! generate's dispatch on the target name (src/codegen/mod.rs:12-22): 0 = the Rust driver (generate_rust -- this module's assembly), 1 = the C generator, 2 = the Wasm generator, 3 = the Python generator, 4 = the JavaScript generator (TWO names route here: 'javascript' and 'js'), 5 = the IR debug dump (debug_ir_string over the candidate body, seeded at indent 0), 6 = unsupported -- generate returns Err(CodegenError::UnsupportedTarget) carrying the name verbatim (rust_target_result_form, rust_unsupported_target_text). Matching is exact and case-sensitive (a match on the &str): 'Rust', 'JS', 'rs' and '' are all route 6. Routes 1-4 name the other targets' generators, whose internals sit outside this module under the ROAD's one-target rule; only their routing is pinned here. The rows are String-equality guards -- the rust_ident_empty shape: fully statable in the spec, and (like it) outside every admitted seed family, whose guards key on Int/Bool only.

pub fn rust_target_route(target: String) -> i64 {
    match target.as_str() {
        "rust" => {
            0
        },
        "c" => {
            1
        },
        "wasm" => {
            2
        },
        "python" => {
            3
        },
        "javascript" => {
            4
        },
        "js" => {
            4
        },
        "ir" => {
            5
        },
        "java" => {
            6
        },
        "Rust" => {
            6
        },
        "" => {
            6
        },
        _ => {
            6
        },
    }
}

}

pub(crate) mod rust_target_result_form {
//! Whether generate returns Ok or Err for a rust_target_route route (src/codegen/mod.rs:12-22): 0 = Ok -- routes 0-5, every named target produces text (generate_rust in particular has no error path at all: no fallible call inside, it ends in Ok unconditionally), 1 = Err -- route 6 only, CodegenError::UnsupportedTarget carrying the target name. Total over the seven routes; route codes outside 0..=6 never occur and are unpinned.

pub fn rust_target_result_form(route: i64) -> i64 {
    match route {
        6 => {
            1
        },
        _ => {
            0
        },
    }
}

}

pub(crate) mod rust_unsupported_target_text {
//! The Display text of CodegenError::UnsupportedTarget (src/codegen/mod.rs:1415): 'Unsupported target language: ' + the target name, verbatim -- the error generate's route 6 returns. The name is carried through unmodified: no rust_ident sanitization, no case folding, no trimming.

pub fn rust_unsupported_target_text(target: String) -> String {
    ("Unsupported target language: ".to_string() + &target)
}

}

pub(crate) mod rust_doc_header_text {
//! generate_rust's module doc header, assembled (src/codegen/mod.rs:70-76): the doc's lines -- split by str::lines and each rendered by rust_doc_line, concatenated (the split and the concatenation are iteration, mirror-only) -- followed by ONE blank line, rust_file_fixed_text piece 1, emitted inside the doc branch: a doc of Some("") has zero lines and still yields exactly the blank line. Whether the header appears at all is the driver's branch (the spec's doc is Some rather than None); rust_driver_section_text states that rule, with this text as its present-arm input.

pub fn rust_doc_header_text(lines: String) -> String {
    (lines.clone() + "\n")
}

}

pub(crate) mod rust_driver_section_text {
//! generate_rust's conditional-section rule, shared by its three optional sections (src/codegen/mod.rs:70-76, 96-104, 131-152): a section whose presence flag holds contributes its assembled text; otherwise it contributes the empty string -- no placeholder, comment or partial frame is emitted for an absent section. The three call sites differ only in what presence MEANS: the doc header's flag is the spec's doc being Some (a present-but-empty doc still emits its blank line -- rust_doc_header_text), while the precondition block's and the test module's flags are their lists being non-empty. One spec serves all three sites, as rust_block_text serves two sites in slices 4-5. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_driver_section_text(present: bool, text: String) -> String {
    match present {
        true => {
            text
        },
        false => {
            "".to_string()
        },
        _ => {
            "".to_string()
        },
    }
}
}

pub(crate) mod rust_file_text {
//! generate_rust's whole-file assembly (src/codegen/mod.rs:66-155) over pre-rendered sections, in emission order: the doc header (rust_doc_header_text, via rust_driver_section_text), the precondition block (rust_precondition_block, via rust_driver_section_text -- emitted BEFORE the signature, as a free-standing block at module level whose asserts name the function's parameters: slice 3 recorded the placement surprise, this equation is where it lands in the file), the signature (rust_signature), the body, the function's closing brace (the text of rust_file_fixed_text piece 0), and the test module (rust_test_module, via rust_driver_section_text). Absent sections arrive as the empty string (rust_driver_section_text's absent arm), so the equation is unconditional. The body arrives rendered by ir_to_rust_tail at indent 1, with the function's return type as both fn_ret and the Some expectation -- the driver's own seed, the one expectation site outside rust_expectation_source's table (that table's doc names this site as the driver's); statement-shaped bodies fall through the tail renderer to the statement walker unchanged, bare-expression bodies keep their value as the function's return. Exactly one function per file, and NO import is ever generated at file level -- the only use statement in the output is the test module's inner 'use super::*;', inside rust_test_module. Multi-input spec: no in-spec tests; golden in tests/m3_proof.rs.

pub fn rust_file_text(header: String, pre_block: String, signature: String, body: String, test_module: String) -> String {
    (((((header.clone() + &pre_block) + &signature) + &body) + "}\n") + &test_module)
}
}

