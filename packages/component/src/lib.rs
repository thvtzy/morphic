//! The library component's glue (DISEBAR_16_SCOPE §4):
//! the facade's 30 non-HO exports behind the
//! `morphic:library` world, one line per export. The
//! five higher-order entries are NOT here — WIT has no
//! function-typed values, so they cross as host-imported
//! callbacks in the sibling callback component
//! (Phase 160, DISEBAR_16_SCOPE §5).

wit_bindgen::generate!({ path: "wit", world: "library" });

use exports::morphic::library::functions::Guest;

struct Component;

impl Guest for Component {
    fn int_abs(x: i64) -> i64 { morphic::stdlib::int_abs(x) }
    fn int_sign(x: i64) -> i64 { morphic::stdlib::int_sign(x) }
    fn int_min(a: i64, b: i64) -> i64 { morphic::stdlib::int_min(a, b) }
    fn int_max(a: i64, b: i64) -> i64 { morphic::stdlib::int_max(a, b) }
    fn int_clamp(x: i64, lo: i64, hi: i64) -> i64 { morphic::stdlib::int_clamp(x, lo, hi) }
    fn int_is_even(x: i64) -> bool { morphic::stdlib::int_is_even(x) }
    fn int_is_odd(x: i64) -> bool { morphic::stdlib::int_is_odd(x) }
    fn char_is_digit(c: char) -> bool { morphic::stdlib::char_is_digit(c) }
    fn char_is_alpha(c: char) -> bool { morphic::stdlib::char_is_alpha(c) }
    fn char_is_whitespace(c: char) -> bool { morphic::stdlib::char_is_whitespace(c) }
    fn char_is_uppercase(c: char) -> bool { morphic::stdlib::char_is_uppercase(c) }
    fn char_is_lowercase(c: char) -> bool { morphic::stdlib::char_is_lowercase(c) }
    fn char_to_digit(c: char) -> Option<i64> { morphic::stdlib::char_to_digit(c) }
    fn string_is_empty(s: String) -> bool { morphic::stdlib::string_is_empty(s) }
    fn result_unwrap_or(r: Result<i64, String>, d: i64) -> i64 { morphic::stdlib::result_unwrap_or(r, d) }
    fn option_unwrap_or(o: Option<i64>, d: i64) -> i64 { morphic::stdlib::option_unwrap_or(o, d) }
    fn option_ok_or(o: Option<i64>, e: String) -> Result<i64, String> { morphic::stdlib::option_ok_or(o, e) }
    fn list_is_empty(xs: Vec<i64>) -> bool { morphic::stdlib::list_is_empty(xs) }
    fn list_reverse(xs: Vec<i64>) -> Vec<i64> { morphic::stdlib::list_reverse(xs) }
    fn list_contains(xs: Vec<i64>, x: i64) -> bool { morphic::stdlib::list_contains(xs, x) }
    fn list_sum(xs: Vec<i64>) -> i64 { morphic::stdlib::list_sum(xs) }
    fn list_product(xs: Vec<i64>) -> i64 { morphic::stdlib::list_product(xs) }
    fn list_min(xs: Vec<i64>) -> Option<i64> { morphic::stdlib::list_min(xs) }
    fn list_max(xs: Vec<i64>) -> Option<i64> { morphic::stdlib::list_max(xs) }
    fn list_take(xs: Vec<i64>, n: i64) -> Vec<i64> { morphic::stdlib::list_take(xs, n) }
    fn list_drop(xs: Vec<i64>, n: i64) -> Vec<i64> { morphic::stdlib::list_drop(xs, n) }
    fn list_is_sorted(xs: Vec<i64>) -> bool { morphic::stdlib::list_is_sorted(xs) }
    fn list_sort(xs: Vec<i64>) -> Vec<i64> { morphic::stdlib::list_sort(xs) }
    fn list_binary_search(xs: Vec<i64>, x: i64) -> Option<i64> { morphic::stdlib::list_binary_search(xs, x) }
    fn range(lo: i64, hi: i64) -> Vec<i64> { morphic::stdlib::range(lo, hi) }
}

export!(Component);
