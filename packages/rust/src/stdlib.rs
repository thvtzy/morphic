//! The 29 frozen standard-library entries as real
//! Rust functions, generated from their shipped
//! bodies by the Morphic pipeline's Rust renderer —
//! do not edit (regenerate with
//! `morphic build --facade`; assembly re-runs the
//! crowning checks and refuses on a moved crown).
//!
//! Each function carries its tier and verdict basis
//! in its rustdoc; the same facts are in the crate's
//! `morphic.manifest.json`. Functions whose
//! specifications freeze a `require:` row carry it
//! as a genuine `debug_assert!` (NF-116-2, taken at
//! this crate's surface — see the crate docs).

// Generated code: the emitter's uniform rendering
// style (full parenthesization of expressions,
// exhaustive match arms, accumulator bindings)
// trips these stylistic lints by construction —
// measured at Phase 123 as 51 warnings over the 29
// functions (unused_parens ×40,
// unreachable_patterns ×10, unused_mut ×1). The
// allowances are scoped to this generated module so
// the functions stay byte-identical to the
// pipeline's rendering while the crate builds
// warning-free (gate clause 3).
#![allow(unused_parens, unreachable_patterns, unused_mut)]

/// int_abs(x) = |x|. The Int minimum is excluded by precondition: its absolute value is not representable in Int (STDLIB_SCOPE.md 2.2, frozen domain note).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 16 pinned, 1 spec-silent
pub fn int_abs(x: i64) -> i64 {
#[cfg(debug_assertions)]
{
    debug_assert!((x != ((0 - 9223372036854775807) - 1)), "Precondition violated");
}

    if (x < 0) {
        (0 - x)
} else {
        x
}
}

/// int_sign(x) = -1 when x is negative, 0 when x is zero, 1 when x is positive (STDLIB_SCOPE.md 2.2). The Int minimum maps to -1; no exclusion is needed.
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 11 pinned, 0 spec-silent
pub fn int_sign(x: i64) -> i64 {
    if (if (x < (0 - 1)) { (0 - 1) } else { x } > 1) {
        1
} else {
        if (x < (0 - 1)) {
            (0 - 1)
} else {
            x
}
}
}

/// int_min(a, b) = the smaller of a and b; equal arguments give that value (STDLIB_SCOPE.md 2.2).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 16 pinned, 0 spec-silent
pub fn int_min(a: i64, b: i64) -> i64 {
    if (a > b) {
        b
} else {
        a
}
}

/// int_max(a, b) = the larger of a and b; equal arguments give that value (STDLIB_SCOPE.md 2.2).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 16 pinned, 0 spec-silent
pub fn int_max(a: i64, b: i64) -> i64 {
    if (a < b) {
        b
} else {
        a
}
}

/// int_clamp(x, lo, hi) = x confined to [lo, hi]: lo when x is below, hi when x is above, x between (STDLIB_SCOPE.md 2.2). Requires lo <= hi; at lo = hi every x gives lo.
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 13 pinned, 2 spec-silent
pub fn int_clamp(x: i64, lo: i64, hi: i64) -> i64 {
#[cfg(debug_assertions)]
{
    debug_assert!((lo <= hi), "Precondition violated");
}

    if (if (x < lo) { lo } else { x } > hi) {
        hi
} else {
        if (x < lo) {
            lo
} else {
            x
}
}
}

/// int_is_even(x) = true iff 2 divides x, sign-independent: -4 is even, -3 is not (STDLIB_SCOPE.md 2.2).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 15 pinned, 0 spec-silent
pub fn int_is_even(x: i64) -> bool {
    ((x % 2) == 0)
}

/// int_is_odd(x) = true iff 2 does not divide x; the exact complement of int_is_even, negatives included (STDLIB_SCOPE.md 2.2).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 15 pinned, 0 spec-silent
pub fn int_is_odd(x: i64) -> bool {
    ((x % 2) != 0)
}

/// char_is_digit(c) = true iff c is in '0'..='9' (STDLIB_SCOPE.md 2.3).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 101 pinned, 0 spec-silent
pub fn char_is_digit(c: char) -> bool {
    ((c >= '0') && (c <= '9'))
}

/// char_is_alpha(c) = true iff c is in 'A'..='Z' or 'a'..='z' — ASCII letters only (STDLIB_SCOPE.md 2.3).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 101 pinned, 0 spec-silent
pub fn char_is_alpha(c: char) -> bool {
    (((c >= 'A') && (c <= 'Z')) || ((c >= 'a') && (c <= 'z')))
}

/// char_is_whitespace(c) = true iff c is one of space, tab, newline, carriage return — the standing tokenizer spec's is_whitespace contract, adopted unchanged (STDLIB_SCOPE.md 2.3).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 101 pinned, 0 spec-silent
pub fn char_is_whitespace(c: char) -> bool {
    ((((c == ' ') || (c == '\t')) || (c == '\n')) || (c == '\r'))
}

/// char_is_uppercase(c) = true iff c is in 'A'..='Z'; non-letters give false (STDLIB_SCOPE.md 2.3).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 101 pinned, 0 spec-silent
pub fn char_is_uppercase(c: char) -> bool {
    ((c >= 'A') && (c <= 'Z'))
}

/// char_is_lowercase(c) = true iff c is in 'a'..='z'; non-letters give false (STDLIB_SCOPE.md 2.3).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 101 pinned, 0 spec-silent
pub fn char_is_lowercase(c: char) -> bool {
    ((c >= 'a') && (c <= 'z'))
}

/// char_to_digit(c) = Some(d) when c is in '0'..='9' with d the digit's value, None for every other char (STDLIB_SCOPE.md 2.3).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 101 pinned, 0 spec-silent
///
/// Known defect — the Char-pattern widening (Phase 123, named in the open): this entry's shipped body is a Match over Char literal patterns, which the Rust emitter renders as `_` patterns (the pattern renderer's Literal catch-all, in lockstep with the synthesized emitter since the M4 gate). As generated, the first arm matches every input: this function returns Some(0) for every Char and does NOT stand at its frozen battery in this crate (1 of its 101 battery points — the digit 0 itself — agrees, by accident of arm order). The npm package's form of this function is exact (the JavaScript renderer carries Char patterns). The entry is carried named — not silently wrong, and not silently dropped from the frozen surface of 29. Closing the widening is a lockstep cascade of the Phase 113 shape, recorded as the assembling phase's named blocker.
pub fn char_to_digit(c: char) -> Option<i64> {
    match c {
        _ => {
            Some(0)
        },
        _ => {
            Some(1)
        },
        _ => {
            Some(2)
        },
        _ => {
            Some(3)
        },
        _ => {
            Some(4)
        },
        _ => {
            Some(5)
        },
        _ => {
            Some(6)
        },
        _ => {
            Some(7)
        },
        _ => {
            Some(8)
        },
        _ => {
            Some(9)
        },
        _ => {
            None
        },
    }
}

/// string_is_empty(s) = true iff s is the empty string: len(s) == 0 (STDLIB_SCOPE.md 2.4).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 15 pinned, 0 spec-silent
pub fn string_is_empty(s: String) -> bool {
    ((s).len() as i64 == 0)
}

/// result_unwrap_or(r, d) = the Ok payload of r, or d when r is Err (STDLIB_SCOPE.md 2.5).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 36 pinned, 0 spec-silent
pub fn result_unwrap_or(r: Result<i64, String>, d: i64) -> i64 {
    match r {
        Ok(v) => {
            v
        },
        Err(_) => {
            d
        },
    }
}

/// option_unwrap_or(o, d) = the Some payload of o, or d when o is None (STDLIB_SCOPE.md 2.5).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 28 pinned, 0 spec-silent
pub fn option_unwrap_or(o: Option<i64>, d: i64) -> i64 {
    match o {
        Some(v) => {
            v
        },
        None => {
            d
        },
    }
}

/// option_ok_or(o, e) = Ok(v) when o is Some(v), or Err(e) with e unchanged when o is None (STDLIB_SCOPE.md 2.5).
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 9 pinned, 0 spec-silent
pub fn option_ok_or(o: Option<i64>, e: String) -> Result<i64, String> {
    match o {
        Some(v) => {
            Ok(v)
        },
        None => {
            Err(e)
        },
    }
}

/// list_is_empty(xs) = true iff xs has no elements: len(xs) == 0 (STDLIB_SCOPE.md 2.6). Tier-1.
///
/// Tier: tier-1
/// Verdict basis: z3-verified
/// Battery: 15 pinned, 0 spec-silent
pub fn list_is_empty(xs: Vec<i64>) -> bool {
    ((xs).len() as i64 == 0)
}

/// list_reverse(xs) = the elements of xs in reverse order; empty gives empty, a singleton gives itself (STDLIB_SCOPE.md 2.6). Tier-2: executed-exact on the full battery; element-level verification awaits the post-1.0 list encoder (D7a).
///
/// Tier: tier-2
/// Verdict basis: executed-exact
/// Battery: 15 pinned, 0 spec-silent
pub fn list_reverse(xs: Vec<i64>) -> Vec<i64> {
    let mut out: Vec<i64> = vec![];
    for i in 0..xs.len() {
        out.push((xs)[((xs.len() - 1) - i)]);
    }
    return out;
}

/// list_contains(xs, x) = true iff some element of xs equals x; the empty list gives false (STDLIB_SCOPE.md 2.6). Tier-2: executed-exact on the full battery; element-level verification awaits the post-1.0 list encoder (D7a).
///
/// Tier: tier-2
/// Verdict basis: executed-exact
/// Battery: 75 pinned, 0 spec-silent
pub fn list_contains(xs: Vec<i64>, x: i64) -> bool {
    let mut found: bool = false;
    for i in 0..xs.len() {
        found = (found || ((xs)[i] == x));
    }
    return found;
}

/// list_sum(xs) = the sum of the elements of xs; the empty list gives 0. Arithmetic is the language's standing Int arithmetic (STDLIB_SCOPE.md 2.6). Tier-2: executed-exact on the full battery; element-level verification awaits the post-1.0 list encoder (D7a).
///
/// Tier: tier-2
/// Verdict basis: executed-exact
/// Battery: 15 pinned, 0 spec-silent
pub fn list_sum(xs: Vec<i64>) -> i64 {
    let mut acc: i64 = 0;
    for i in 0..xs.len() {
        acc = (acc + (xs)[i]);
    }
    return acc;
}

/// list_product(xs) = the product of the elements of xs; the empty list gives 1, and any zero element gives 0 (STDLIB_SCOPE.md 2.6). Tier-2: executed-exact on the full battery; element-level verification awaits the post-1.0 list encoder (D7a).
///
/// Tier: tier-2
/// Verdict basis: executed-exact
/// Battery: 13 pinned, 0 spec-silent
pub fn list_product(xs: Vec<i64>) -> i64 {
    let mut acc: i64 = 1;
    for i in 0..xs.len() {
        acc = (acc * (xs)[i]);
    }
    return acc;
}

/// list_min(xs) = Some(m) with m the least element of xs, or None when xs is empty — the empty case is a value, not a precondition (STDLIB_SCOPE.md 2.6). Tier-2: executed-exact on the full battery; element-level verification awaits the post-1.0 list encoder (D7a).
///
/// Tier: tier-2
/// Verdict basis: executed-exact
/// Battery: 15 pinned, 0 spec-silent
pub fn list_min(xs: Vec<i64>) -> Option<i64> {
    if ((xs).len() as i64 == 0) {
        None
} else {
        let mut m: i64 = (xs)[0];
        for i in 1..xs.len() {
            m = if ((xs)[i] < m) { (xs)[i] } else { m };
        }
        return Some(m);
}
}

/// list_max(xs) = Some(m) with m the greatest element of xs, or None when xs is empty — the empty case is a value, not a precondition (STDLIB_SCOPE.md 2.6). Tier-2: executed-exact on the full battery; element-level verification awaits the post-1.0 list encoder (D7a).
///
/// Tier: tier-2
/// Verdict basis: executed-exact
/// Battery: 15 pinned, 0 spec-silent
pub fn list_max(xs: Vec<i64>) -> Option<i64> {
    if ((xs).len() as i64 == 0) {
        None
} else {
        let mut m: i64 = (xs)[0];
        for i in 1..xs.len() {
            m = if ((xs)[i] > m) { (xs)[i] } else { m };
        }
        return Some(m);
}
}

/// list_take(xs, n) = the first n elements of xs, with n clamped into [0, len(xs)]: n <= 0 gives the empty list, n >= len(xs) gives xs unchanged (STDLIB_SCOPE.md 2.6). Tier-2: executed-exact on the full battery; element-level verification awaits the post-1.0 list encoder (D7a).
///
/// Tier: tier-2
/// Verdict basis: executed-exact
/// Battery: 28 pinned, 0 spec-silent
pub fn list_take(xs: Vec<i64>, n: i64) -> Vec<i64> {
    let mut out: Vec<i64> = vec![];
    let mut k: i64 = 0;
    for i in 0..xs.len() {
        if (k < n) {
            out.push((xs)[i])
} else {
}
        k = (k + 1);
    }
    return out;
}

/// list_drop(xs, n) = xs without its first n elements, with n clamped into [0, len(xs)]: n <= 0 gives xs unchanged, n >= len(xs) gives the empty list (STDLIB_SCOPE.md 2.6). Tier-2: executed-exact on the full battery; element-level verification awaits the post-1.0 list encoder (D7a).
///
/// Tier: tier-2
/// Verdict basis: executed-exact
/// Battery: 28 pinned, 0 spec-silent
pub fn list_drop(xs: Vec<i64>, n: i64) -> Vec<i64> {
    let mut out: Vec<i64> = vec![];
    let mut k: i64 = 0;
    for i in 0..xs.len() {
        if (k >= n) {
            out.push((xs)[i])
} else {
}
        k = (k + 1);
    }
    return out;
}

/// list_is_sorted(xs) = true iff xs is non-strictly ascending (every adjacent pair a <= b); the empty list and singletons give true. The library form of Appendix A's is_sorted (STDLIB_SCOPE.md 2.6). Tier-2: executed-exact on the full battery; element-level verification awaits the post-1.0 list encoder (D7a).
///
/// Tier: tier-2
/// Verdict basis: executed-exact
/// Battery: 15 pinned, 0 spec-silent
pub fn list_is_sorted(xs: Vec<i64>) -> bool {
    let mut ok: bool = true;
    for i in 1..xs.len() {
        ok = (ok && ((xs)[(i - 1)] <= (xs)[i]));
    }
    return ok;
}

/// list_sort(xs) = the elements of xs in non-strict ascending order; the output is a permutation of the input, judged on the executed battery (the is_permutation Predicate form is not a shipped entry, STDLIB_SCOPE.md 2.7). Empty and singleton lists give themselves (STDLIB_SCOPE.md 2.6). Tier-2: executed-exact on the full battery; element-level verification awaits the post-1.0 list encoder (D7a).
///
/// Tier: tier-2
/// Verdict basis: executed-exact
/// Battery: 15 pinned, 0 spec-silent
pub fn list_sort(xs: Vec<i64>) -> Vec<i64> {
    let mut out: Vec<i64> = vec![];
    for i in 0..xs.len() {
        let mut x: i64 = (xs)[i];
        let mut pos: i64 = (out).len() as i64;
        let mut k: i64 = 0;
        for j in 0..out.len() {
            if ((pos == (out).len() as i64) && ((out)[j] > x)) {
                pos = k;
} else {
}
            k = (k + 1);
        }
        let mut next: Vec<i64> = vec![];
        let mut k2: i64 = 0;
        for j in 0..out.len() {
            if (k2 == pos) {
                next.push(x)
} else {
}
            next.push((out)[j]);
            k2 = (k2 + 1);
        }
        if (pos == (out).len() as i64) {
            next.push(x)
} else {
}
        out = next;
    }
    return out;
}

/// list_binary_search(xs, x) = Some(i) with xs\[i] == x when x occurs in the sorted list xs, or None when it does not. With duplicate targets any valid index satisfies the contract — the specific index is spec-silent. Hand body at the executed standard (the G5 scope-around, STDLIB_SCOPE.md 2.6): the body is a scan, because generated Rust cannot index a Vec by an Int variable (the language has no usize — G5), so the While-form binary search cannot compile; the frozen contract is the search semantics above, and per-entry complexity is deliberately outside the gate (STDLIB_SCOPE.md 3). Tier-2: executed-exact on the full battery; element-level verification awaits the post-1.0 list encoder (D7a).
///
/// Tier: tier-2
/// Verdict basis: executed-exact
/// Battery: 64 pinned, 3 spec-silent
pub fn list_binary_search(xs: Vec<i64>, x: i64) -> Option<i64> {
#[cfg(debug_assertions)]
{
    debug_assert!(list_is_sorted(xs.clone()), "Precondition violated");
}

    let mut res: Option<i64> = None;
    let mut k: i64 = 0;
    for i in 0..xs.len() {
        if (((xs)[i] == x) && (res == None)) {
            res = Some(k);
} else {
}
        k = (k + 1);
    }
    return res;
}

