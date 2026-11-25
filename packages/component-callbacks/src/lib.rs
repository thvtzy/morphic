//! The callback component's glue (DISEBAR_16_SCOPE §5):
//! the facade's five higher-order entries behind the
//! `library-callbacks` world. Each entry's function
//! argument is one of the world's three host imports —
//! the import IS the callback: the glue hands the
//! facade a plain Rust function value that calls the
//! import, and the facade's bodies do the rest. No
//! function is re-implemented.
//!
//! The correspondence (frozen at §5):
//!   map-apply -> list-map's f
//!   pred      -> list-filter's / list-all's /
//!                list-any's p
//!   combine   -> list-fold's f, accumulator first —
//!                the frozen LEFT fold's order:
//!                f(f(f(init, x0), x1), ...)

wit_bindgen::generate!({ path: "wit", world: "library-callbacks" });

struct Component;

impl Guest for Component {
    fn list_map(xs: Vec<i64>) -> Vec<i64> {
        morphic::stdlib::list_map(xs, |x| crate::map_apply(x))
    }
    fn list_filter(xs: Vec<i64>) -> Vec<i64> {
        morphic::stdlib::list_filter(xs, |x| crate::pred(x))
    }
    fn list_fold(xs: Vec<i64>, init: i64) -> i64 {
        morphic::stdlib::list_fold(xs, init, |acc, x| crate::combine(acc, x))
    }
    fn list_all(xs: Vec<i64>) -> bool {
        morphic::stdlib::list_all(xs, |x| crate::pred(x))
    }
    fn list_any(xs: Vec<i64>) -> bool {
        morphic::stdlib::list_any(xs, |x| crate::pred(x))
    }
}

export!(Component);
