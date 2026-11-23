//! Phase 132 (G4, D4a's staged second supply form) —
//! named-reference function supply.
//!
//! Since Phase 130 a function argument could be supplied
//! only as a lambda literal: a test row naming a sibling
//! spec in a function position (`(double_it, 21)`) checked
//! (the typeck Var arm resolves the name to the Function
//! type) but bound nothing — `value_of_expr` has no module
//! index to resolve the name against, and the row was
//! Unbindable. This phase lands the second supply form
//! G4_SCOPE §2.5 froze: a bare name in a function position
//! resolves through the module index (`SpecIndex` — the
//! index the prelude merge lives in, so shipped entries
//! resolve exactly as siblings do, and a module's own
//! definition wins a collision) to a function value built
//! from that spec's denoted body, applicable through
//! Phase 130's machinery under the same budget + depth
//! discipline.
//!
//! The pins below measure, in order: the frozen §3
//! exhibits (`double_it` / `apply_named` bind + judge
//! in-process); a shipped entry supplied by name, and the
//! Phase 110 collision rule at the same site; a recursive
//! named reference inside and past the standing bounds;
//! the decline taxonomy (unknown name, a name denoting no
//! executable body, a signature that does not fit the
//! position, a name in a non-function position — each
//! declines BY NAME, never silently binds); §2.6's
//! no-equality rule over named references; the shrunk
//! Unbindable reason (tuples are the remaining
//! unrepresentable class); an acyclic composition supplied
//! by name; and lambda supply standing untouched.

use morphic::spec::ast::{Expr, FunctionSpec};
use morphic::spec::{parser, typeck};
use morphic::synthesis::engine::{IRNode, SpecIndex};
use morphic::synthesis::eval::{self, TestOutcome, TestRun, Value};

/// Parse + check a module and build the index the supply
/// resolves through — the production construction
/// (`SpecIndex::from_module`, prelude merged).
fn checked_index(source: &str) -> (Vec<FunctionSpec>, SpecIndex) {
    let module = parser::parse(source).expect("probe source parses");
    let checked = typeck::check(module).expect("probe source typechecks");
    let index = SpecIndex::from_module(&checked.spec);
    (checked.spec.functions, index)
}

fn spec_named<'a>(specs: &'a [FunctionSpec], name: &str) -> &'a FunctionSpec {
    specs
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("spec {name} in the checked module"))
}

/// The frozen §3 specimen module (the Phase 127 probe's
/// source, verbatim).
const NAMED_MODULE: &str = r#"Probe {
    spec double_it {
        input: x: Int
        output: Int
        constraint: output == 2 * x
        test { input: 21; expect: 42 }
    }
    spec apply_named {
        input: f: fn(Int) -> Int, x: Int
        output: Int
        constraint: output == f(x)
        test { input: (double_it, 21); expect: 42 }
    }
}"#;

#[test]
fn phase132_named_reference_binds_and_judges() {
    let (specs, index) = checked_index(NAMED_MODULE);
    let apply_named = spec_named(&specs, "apply_named");

    // The context-free route stands exactly as before: it
    // carries no module index, so the name still binds
    // nothing there.
    assert!(eval::value_of_expr(&Expr::Var("double_it".into())).is_none());
    assert_eq!(eval::bindable_test_count(apply_named), 0);

    // The index route: the bare name maps to a function
    // value, the row binds, and the spec's own denoted
    // body judges it in-process.
    let value = eval::value_of_expr_in(&Expr::Var("double_it".into()), &index);
    assert!(
        matches!(value, Some(Value::Function { .. })),
        "a named reference maps to a function value through the index: {value:?}"
    );
    assert_eq!(eval::bindable_test_count_in(apply_named, &index), 1);
    assert_eq!(
        eval::test_bindability_in(apply_named, &index),
        vec![true]
    );
    let body = index
        .callee_body("apply_named")
        .expect("apply_named denotes its equation body");
    let outcomes = eval::test_outcomes_in(&body, apply_named, &index);
    assert_eq!(outcomes, vec![TestOutcome::Passed]);
    assert_eq!(
        eval::run_spec_tests_in(&body, apply_named, &index),
        TestRun {
            evaluable: 1,
            passed: 1
        }
    );
    assert_eq!(eval::valued_test_count_in(&body, apply_named, &index), 1);
}

#[cfg(not(feature = "wasm-playground"))]
#[test]
fn phase132_shipped_entry_supply_and_collision_rule() {
    // A shipped entry named in a function position: the
    // module does not define `int_abs`; the prelude merge
    // in the index supplies it, with its shipped body.
    let (specs, index) = checked_index(
        r#"Probe {
            spec apply_abs {
                input: f: fn(Int) -> Int, x: Int
                output: Int
                constraint: output == f(x)
                test { input: (int_abs, (0 - 7)); expect: 7 }
            }
        }"#,
    );
    let apply_abs = spec_named(&specs, "apply_abs");
    let body = index.callee_body("apply_abs").expect("denoted body");
    assert_eq!(
        eval::test_outcomes_in(&body, apply_abs, &index),
        vec![TestOutcome::Passed],
        "the shipped entry applies through the named supply"
    );

    // The collision rule (Phase 110): a module defining
    // its own `int_abs` wins — the named supply binds the
    // module's own spec (here the identity), never the
    // prelude's. The expectation is the identity's answer,
    // so the two worlds are distinguishable by the judged
    // value itself.
    let (specs, index) = checked_index(
        r#"Probe {
            spec int_abs {
                input: x: Int
                output: Int
                constraint: output == x
                test { input: (0 - 7); expect: (0 - 7) }
            }
            spec apply_abs {
                input: f: fn(Int) -> Int, x: Int
                output: Int
                constraint: output == f(x)
                test { input: (int_abs, (0 - 7)); expect: (0 - 7) }
            }
        }"#,
    );
    let apply_abs = spec_named(&specs, "apply_abs");
    let body = index.callee_body("apply_abs").expect("denoted body");
    assert_eq!(
        eval::test_outcomes_in(&body, apply_abs, &index),
        vec![TestOutcome::Passed],
        "the module's own int_abs (identity) is what the name denotes"
    );
}

#[test]
fn phase132_recursive_named_reference_within_and_past_the_bounds() {
    // A recursive named spec in the guard-row idiom (the
    // row `G || output == P` states "not-G implies
    // output == P"): sum_to(n) = 0 when n <= 0, else
    // n + sum_to(n - 1). Recursion is a named-spec feature
    // (§2.2): the named reference is the vehicle, and it
    // spends the standing budget/depth like any other
    // application.
    let (specs, index) = checked_index(
        r#"Probe {
            spec sum_to {
                input: n: Int
                output: Int
                constraint: n > 0 || output == 0
                constraint: n <= 0 || output == n + sum_to(n - 1)
                test { input: 5; expect: 15 }
            }
            spec apply_sum {
                input: f: fn(Int) -> Int, x: Int
                output: Int
                constraint: output == f(x)
                test { input: (sum_to, 0); expect: 0 }
                test { input: (sum_to, 5); expect: 15 }
                test { input: (sum_to, 9); expect: 45 }
            }
        }"#,
    );
    let apply_sum = spec_named(&specs, "apply_sum");
    let body = index.callee_body("apply_sum").expect("denoted body");
    assert_eq!(
        eval::test_outcomes_in(&body, apply_sum, &index),
        vec![
            TestOutcome::Passed,
            TestOutcome::Passed,
            TestOutcome::Passed
        ],
        "recursion within the bounds judges like any other application"
    );

    // Past the depth bound the same reference declines
    // NotEvaluable — the depth bound's own decline, by
    // name, never a hang and never a fabricated value:
    // sum_to(10) would nest eleven applications.
    let (specs, index) = checked_index(
        r#"Probe {
            spec sum_to {
                input: n: Int
                output: Int
                constraint: n > 0 || output == 0
                constraint: n <= 0 || output == n + sum_to(n - 1)
                test { input: 5; expect: 15 }
            }
            spec apply_sum_deep {
                input: f: fn(Int) -> Int, x: Int
                output: Int
                constraint: output == f(x)
                test { input: (sum_to, 10); expect: 55 }
            }
        }"#,
    );
    let apply_sum_deep = spec_named(&specs, "apply_sum_deep");
    let body = index.callee_body("apply_sum_deep").expect("denoted body");
    let outcomes = eval::test_outcomes_in(&body, apply_sum_deep, &index);
    match &outcomes[0] {
        TestOutcome::BodyNotEvaluable(reason) => {
            assert!(
                reason.contains("depth bound"),
                "the decline is the depth bound's: {reason}"
            );
        }
        other => panic!("expected the depth-bound decline, got {other:?}"),
    }

    // A recursion with no base case declines the same
    // way (it spends depth until the bound, then stops).
    let (specs, index) = checked_index(
        r#"Probe {
            spec sum_eq {
                input: n: Int
                output: Int
                constraint: output == n + sum_eq(n - 1)
                test { input: 3; expect: 6 }
            }
            spec apply_eq {
                input: f: fn(Int) -> Int, x: Int
                output: Int
                constraint: output == f(x)
                test { input: (sum_eq, 3); expect: 6 }
            }
        }"#,
    );
    let apply_eq = spec_named(&specs, "apply_eq");
    let body = index.callee_body("apply_eq").expect("denoted body");
    let outcomes = eval::test_outcomes_in(&body, apply_eq, &index);
    assert!(
        matches!(outcomes[0], TestOutcome::BodyNotEvaluable(_)),
        "a base-case-free recursion declines, never hangs: {outcomes:?}"
    );
}

#[test]
fn phase132_named_reference_declines_by_name() {
    // (1) A name unknown to the index in hand: the
    // checker requires a test row's names to be defined
    // in the module, so this decline is the binder's own
    // guard for an index built over a subset (here:
    // `from_specs` over apply_named alone — the prelude
    // merges, `double_it` does not). The row declines
    // Unbindable, naming the name and the class; it never
    // silently binds.
    let (specs, _full_index) = checked_index(NAMED_MODULE);
    let apply_named = spec_named(&specs, "apply_named");
    let subset = SpecIndex::from_specs(std::slice::from_ref(apply_named));
    let body = subset.callee_body("apply_named").expect("denoted body");
    let outcomes = eval::test_outcomes_in(&body, apply_named, &subset);
    match &outcomes[0] {
        TestOutcome::Unbindable(reason) => {
            assert!(reason.contains("double_it"), "names the name: {reason}");
            assert!(reason.contains("unknown"), "names the class: {reason}");
        }
        other => panic!("expected Unbindable, got {other:?}"),
    }

    // (2) A name that resolves to a spec denoting no
    // executable body: `lower_bound`'s only constraint is
    // an inequality — a property, not a body.
    let (specs, index) = checked_index(
        r#"Probe {
            spec lower_bound {
                input: x: Int
                output: Int
                constraint: output >= x
                test { input: 3; expect: 3 }
            }
            spec apply_named {
                input: f: fn(Int) -> Int, x: Int
                output: Int
                constraint: output == f(x)
                test { input: (lower_bound, 21); expect: 21 }
            }
        }"#,
    );
    let apply_named = spec_named(&specs, "apply_named");
    let body = index.callee_body("apply_named").expect("denoted body");
    let outcomes = eval::test_outcomes_in(&body, apply_named, &index);
    match &outcomes[0] {
        TestOutcome::Unbindable(reason) => {
            assert!(reason.contains("lower_bound"), "names the name: {reason}");
            assert!(
                reason.contains("no executable body"),
                "names the class: {reason}"
            );
        }
        other => panic!("expected Unbindable, got {other:?}"),
    }

    // (3) A name whose signature does not fit the
    // function position: `double_it` is fn(Int) -> Int;
    // the position requires fn(Int, Int) -> Int.
    let (specs, index) = checked_index(
        r#"Probe {
            spec double_it {
                input: x: Int
                output: Int
                constraint: output == 2 * x
                test { input: 21; expect: 42 }
            }
            spec apply_pair {
                input: f: fn(Int, Int) -> Int, x: Int
                output: Int
                constraint: output == f(x, x)
                test { input: (double_it, 21); expect: 42 }
            }
        }"#,
    );
    let apply_pair = spec_named(&specs, "apply_pair");
    let body = index.callee_body("apply_pair").expect("denoted body");
    let outcomes = eval::test_outcomes_in(&body, apply_pair, &index);
    match &outcomes[0] {
        TestOutcome::Unbindable(reason) => {
            assert!(reason.contains("double_it"), "names the name: {reason}");
            assert!(
                reason.contains("does not match"),
                "names the class: {reason}"
            );
        }
        other => panic!("expected Unbindable, got {other:?}"),
    }

    // (4) A name in a non-function position: `double_it`
    // supplied for an Int parameter is not a supply form
    // at all — the row declines, naming the name.
    let (specs, index) = checked_index(
        r#"Probe {
            spec double_it {
                input: x: Int
                output: Int
                constraint: output == 2 * x
                test { input: 21; expect: 42 }
            }
            spec pair_sum {
                input: a: Int, b: Int
                output: Int
                constraint: output == a + b
                test { input: (double_it, 3); expect: 45 }
            }
        }"#,
    );
    let pair_sum = spec_named(&specs, "pair_sum");
    let body = index.callee_body("pair_sum").expect("denoted body");
    let outcomes = eval::test_outcomes_in(&body, pair_sum, &index);
    match &outcomes[0] {
        TestOutcome::Unbindable(reason) => {
            assert!(reason.contains("double_it"), "names the name: {reason}");
            assert!(
                reason.contains("non-function position"),
                "names the class: {reason}"
            );
        }
        other => panic!("expected Unbindable, got {other:?}"),
    }
}

#[test]
fn phase132_named_reference_has_no_equality() {
    // §2.6 (D5a) covers named references unchanged: a
    // named reference IS a function value, so a judged
    // comparison against one declines — the language
    // compares function RESULTS on points instead. Here
    // the candidate returns its function parameter and
    // the expectation is the named reference itself: the
    // body evaluates, the comparison declines.
    let (specs, index) = checked_index(
        r#"Probe {
            spec double_it {
                input: x: Int
                output: Int
                constraint: output == 2 * x
                test { input: 21; expect: 42 }
            }
            spec identity_fn {
                input: f: fn(Int) -> Int
                output: fn(Int) -> Int
                constraint: output == f
                test { input: double_it; expect: double_it }
            }
        }"#,
    );
    let identity_fn = spec_named(&specs, "identity_fn");
    let body = IRNode::Var("f".into());
    let outcomes = eval::test_outcomes_in(&body, identity_fn, &index);
    match &outcomes[0] {
        TestOutcome::BodyNotEvaluable(reason) => {
            assert!(
                reason.contains("no equality"),
                "the §2.6 decline: {reason}"
            );
        }
        other => panic!("expected the comparison to decline, got {other:?}"),
    }
}

#[test]
fn phase132_named_composition_supply() {
    // An acyclic composition supplied by name: quad_it's
    // equation body calls double_it twice; the resolver
    // route (the standing consumer construction) resolves
    // those calls, and the supplied value applies like
    // any other closure.
    let (specs, index) = checked_index(
        r#"Probe {
            spec double_it {
                input: x: Int
                output: Int
                constraint: output == 2 * x
                test { input: 21; expect: 42 }
            }
            spec quad_it {
                input: x: Int
                output: Int
                constraint: output == double_it(double_it(x))
                test { input: 3; expect: 12 }
            }
            spec apply_quad {
                input: f: fn(Int) -> Int, x: Int
                output: Int
                constraint: output == f(x)
                test { input: (quad_it, 3); expect: 12 }
            }
        }"#,
    );
    let apply_quad = spec_named(&specs, "apply_quad");
    let body = index.callee_body("apply_quad").expect("denoted body");
    assert_eq!(
        eval::test_outcomes_in(&body, apply_quad, &index),
        vec![TestOutcome::Passed]
    );
}

#[test]
fn phase132_unbindable_reason_names_only_what_remains() {
    // After this phase the remaining class a test input
    // can carry that NO route maps is the tuple (there is
    // no tuple value): the generic Unbindable reason
    // names exactly that, and no longer names the named
    // function reference — a supply form since this
    // phase, whose failures decline with their own named
    // reasons (phase132_named_reference_declines_by_name).
    let (specs, index) = checked_index(
        r#"Probe {
            spec nested_pair {
                input: pair: List<Int>, x: Int
                output: Int
                constraint: output == x
                test { input: ((1, 2), 3); expect: 3 }
            }
        }"#,
    );
    let nested = spec_named(&specs, "nested_pair");
    let body = IRNode::Var("x".into());
    for outcomes in [
        eval::test_outcomes_in(&body, nested, &index),
        eval::test_outcomes(&body, nested),
    ] {
        match &outcomes[0] {
            TestOutcome::Unbindable(reason) => {
                assert!(reason.contains("tuple"), "names what remains: {reason}");
                assert!(
                    !reason.contains("named function reference"),
                    "no longer names the landed supply form: {reason}"
                );
            }
            other => panic!("expected Unbindable, got {other:?}"),
        }
    }
}

#[test]
fn phase132_lambda_supply_untouched() {
    // The non-mover at the same surface: lambda supply on
    // the context-free route judges exactly as Phase 130
    // left it (the frozen apply_twice specimen, 2/2).
    let (specs, _index) = checked_index(
        r#"Probe {
            spec apply_twice {
                input: f: fn(Int) -> Int, x: Int
                output: Int
                constraint: output == f(f(x))
                test { input: (|n| n + 1, 40); expect: 42 }
                test { input: (|n| n * 2, 3); expect: 12 }
            }
        }"#,
    );
    let apply_twice = spec_named(&specs, "apply_twice");
    let body = IRNode::Call {
        function: "f".into(),
        args: vec![IRNode::Call {
            function: "f".into(),
            args: vec![IRNode::Var("x".into())],
        }],
    };
    assert_eq!(
        eval::run_spec_tests(&body, apply_twice),
        TestRun {
            evaluable: 2,
            passed: 2
        }
    );
}
