// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC TEMPLATE LIBRARY (v0.4)                         │
// │  Structural patterns for synthesis seeding               │
// │                                                          │
// │  Templates are IR blueprints with strategic holes.       │
// │  Each template encodes a known algorithmic paradigm.     │
// └──────────────────────────────────────────────────────────┘

use crate::spec::ast::{FunctionSpec, TypeRef};
use super::engine::{IRNode, IRLiteral, IRBinOp, IRUnaryOp, CollectionKind, IRPattern};

/// All available template patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateKind {
    /// Divide-and-conquer: split, recurse, merge
    DivideAndConquer,
    /// Linear scan: for loop over collection
    LinearScan,
    /// Two-pointer walk
    TwoPointer,
    /// Sliding window
    SlidingWindow,
    /// Accumulator / fold
    Fold,
    /// Binary search
    BinarySearch,
    /// Memoized recursion (dynamic programming)
    Memoization,
    /// Iterator chain: map → filter → collect
    IteratorChain,
    /// Simple computation: arithmetic, no structure
    SimpleCompute,
}

/// Generate IR templates for a given spec
pub fn generate_templates(spec: &FunctionSpec) -> Vec<(TemplateKind, IRNode)> {
    let mut templates = Vec::new();

    // Detect spec shape and generate matching templates
    if has_sort_like_signature(spec) {
        templates.push((TemplateKind::DivideAndConquer, build_divide_and_conquer(spec)));
        templates.push((TemplateKind::IteratorChain, build_iterator_chain(spec)));
    }

    if has_search_like_signature(spec) {
        templates.push((TemplateKind::BinarySearch, build_binary_search(spec)));
        templates.push((TemplateKind::LinearScan, build_linear_scan(spec)));
    }

    if has_fold_like_signature(spec) {
        templates.push((TemplateKind::Fold, build_fold(spec)));
    }

    if has_two_pointer_pattern(spec) {
        templates.push((TemplateKind::TwoPointer, build_two_pointer(spec)));
        templates.push((TemplateKind::SlidingWindow, build_sliding_window(spec)));
    }

    // Always include simple compute + linear scan as fallback
    templates.push((TemplateKind::SimpleCompute, build_simple_compute(spec)));
    templates.push((TemplateKind::LinearScan, build_linear_scan(spec)));

    templates
}

// ── Template Builders ──────────────────────────────────────

fn build_divide_and_conquer(spec: &FunctionSpec) -> IRNode {
    IRNode::Block(vec![
        // if len <= 1: return input
        IRNode::If {
            condition: Box::new(IRNode::BinOp(
                IRBinOp::Le,
                Box::new(IRNode::UnaryOp(IRUnaryOp::Len, Box::new(param_var(spec, 0)))),
                Box::new(IRNode::Literal(IRLiteral::Int(1))),
            )),
            then_branch: Box::new(IRNode::Return(Box::new(param_var(spec, 0)))),
            else_branch: Box::new(IRNode::Block(vec![
                // pivot = input[0]
                IRNode::Let {
                    name: "pivot".into(),
                    value: Box::new(IRNode::Call {
                        function: "first".into(),
                        args: vec![param_var(spec, 0)],
                    }),
                    body: Box::new(IRNode::Block(vec![
                        // left = elements < pivot
                        IRNode::Let {
                            name: "left".into(),
                            value: Box::new(IRNode::Filter {
                                predicate: Box::new(IRNode::Lambda {
                                    params: vec![("x".into(), TypeRef::Named("T".into()))],
                                    body: Box::new(IRNode::BinOp(
                                        IRBinOp::Lt,
                                        Box::new(IRNode::Var("x".into())),
                                        Box::new(IRNode::Var("pivot".into())),
                                    )),
                                }),
                                over: Box::new(IRNode::Call {
                                    function: "rest".into(),
                                    args: vec![param_var(spec, 0)],
                                }),
                            }),
                            body: Box::new(IRNode::Block(vec![
                                // right = elements >= pivot
                                IRNode::Let {
                                    name: "right".into(),
                                    value: Box::new(IRNode::Filter {
                                        predicate: Box::new(IRNode::Lambda {
                                            params: vec![("x".into(), TypeRef::Named("T".into()))],
                                            body: Box::new(IRNode::BinOp(
                                                IRBinOp::Ge,
                                                Box::new(IRNode::Var("x".into())),
                                                Box::new(IRNode::Var("pivot".into())),
                                            )),
                                        }),
                                        over: Box::new(IRNode::Call {
                                            function: "rest".into(),
                                            args: vec![param_var(spec, 0)],
                                        }),
                                    }),
                                    body: Box::new(IRNode::Block(vec![
                                        // sort(left) + [pivot] + sort(right)
                                        IRNode::Return(
                                            Box::new(IRNode::Call {
                                                function: "concat".into(),
                                                args: vec![
                                                    IRNode::Call {
                                                        function: spec.name.clone(),
                                                        args: vec![IRNode::Var("left".into())],
                                                    },
                                                    IRNode::Collection {
                                                        kind: CollectionKind::List,
                                                        elements: vec![IRNode::Var("pivot".into())],
                                                    },
                                                    IRNode::Call {
                                                        function: spec.name.clone(),
                                                        args: vec![IRNode::Var("right".into())],
                                                    },
                                                ],
                                            }),
                                        ),
                                    ])),
                                },
                            ])),
                        },
                    ])),
                },
            ])),
        },
    ])
}

fn build_linear_scan(spec: &FunctionSpec) -> IRNode {
    IRNode::Block(vec![
        IRNode::Alloc {
            name: "result".into(),
            ty: spec.return_type.clone(),
            initial: Box::new(IRNode::Collection { kind: CollectionKind::List, elements: vec![] }),
        },
        IRNode::For {
            var: "i".into(),
            start: Box::new(IRNode::Literal(IRLiteral::Int(0))),
            end: Box::new(IRNode::UnaryOp(IRUnaryOp::Len, Box::new(param_var(spec, 0)))),
            body: Box::new(IRNode::Block(vec![
                IRNode::Assign {
                    target: Box::new(IRNode::Var("result".into())),
                    value: Box::new(IRNode::Call {
                        function: "push".into(),
                        args: vec![
                            IRNode::Var("result".into()),
                            IRNode::Call {
                                function: "transform".into(),
                                args: vec![
                                    IRNode::Index(
                                        Box::new(param_var(spec, 0)),
                                        Box::new(IRNode::Var("i".into())),
                                    ),
                                ],
                            },
                        ],
                    }),
                },
            ])),
        },
        IRNode::Return(Box::new(IRNode::Var("result".into()))),
    ])
}

fn build_binary_search(spec: &FunctionSpec) -> IRNode {
    let p0 = || param_var(spec, 0);
    let p1 = || param_var(spec, 1);
    IRNode::Block(vec![
        IRNode::Alloc {
            name: "lo".into(), ty: TypeRef::Int,
            initial: Box::new(IRNode::Literal(IRLiteral::Int(0))),
        },
        IRNode::Alloc {
            name: "hi".into(), ty: TypeRef::Int,
            initial: Box::new(IRNode::UnaryOp(IRUnaryOp::Len, Box::new(p0()))),
        },
        IRNode::While {
            condition: Box::new(IRNode::BinOp(
                IRBinOp::Lt,
                Box::new(IRNode::Var("lo".into())),
                Box::new(IRNode::Var("hi".into())),
            )),
            invariant: Some("0 <= lo && lo <= hi && hi <= len(haystack)".into()),
            body: Box::new(IRNode::Block(vec![
                // mid = lo + (hi-lo)/2
                IRNode::Let {
                    name: "mid".into(),
                    value: Box::new(IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(IRNode::Var("lo".into())),
                        Box::new(IRNode::BinOp(
                            IRBinOp::Div,
                            Box::new(IRNode::BinOp(
                                IRBinOp::Sub,
                                Box::new(IRNode::Var("hi".into())),
                                Box::new(IRNode::Var("lo".into())),
                            )),
                            Box::new(IRNode::Literal(IRLiteral::Int(2))),
                        )),
                    )),
                    body: Box::new(IRNode::Block(vec![
                        // if haystack[mid] == needle: return Some(mid)
                        IRNode::If {
                            condition: Box::new(IRNode::BinOp(
                                IRBinOp::Eq,
                                Box::new(IRNode::Index(
                                    Box::new(p0()),
                                    Box::new(IRNode::Var("mid".into())),
                                )),
                                Box::new(p1()),
                            )),
                            then_branch: Box::new(IRNode::Return(
                                Box::new(IRNode::Collection {
                                    kind: CollectionKind::Tuple,
                                    elements: vec![
                                        IRNode::Var("mid".into()),
                                    ],
                                }),
                            )),
                            else_branch: Box::new(IRNode::If {
                                condition: Box::new(IRNode::BinOp(
                                    IRBinOp::Lt,
                                    Box::new(IRNode::Index(
                                        Box::new(p0()),
                                        Box::new(IRNode::Var("mid".into())),
                                    )),
                                    Box::new(p1()),
                                )),
                                then_branch: Box::new(IRNode::Assign {
                                    target: Box::new(IRNode::Var("lo".into())),
                                    value: Box::new(IRNode::BinOp(
                                        IRBinOp::Add,
                                        Box::new(IRNode::Var("mid".into())),
                                        Box::new(IRNode::Literal(IRLiteral::Int(1))),
                                    )),
                                }),
                                else_branch: Box::new(IRNode::Assign {
                                    target: Box::new(IRNode::Var("hi".into())),
                                    value: Box::new(IRNode::Var("mid".into())),
                                }),
                            }),
                        },
                    ])),
                },
            ])),
        },
        // return None
        IRNode::Return(Box::new(IRNode::Var("None".into()))),
    ])
}

fn build_two_pointer(_spec: &FunctionSpec) -> IRNode {
    IRNode::Block(vec![
        IRNode::Alloc { name: "left".into(), ty: TypeRef::Int, initial: Box::new(IRNode::Literal(IRLiteral::Int(0))) },
        IRNode::Alloc { name: "right".into(), ty: TypeRef::Int, initial: Box::new(IRNode::Literal(IRLiteral::Int(0))) },
        IRNode::While {
            condition: Box::new(IRNode::BinOp(
                IRBinOp::Lt,
                Box::new(IRNode::Var("right".into())),
                Box::new(IRNode::Call { function: "len".into(), args: vec![param_var(_spec, 0)] }),
            )),
            invariant: None,
            body: Box::new(IRNode::Block(vec![
                IRNode::If {
                    condition: Box::new(IRNode::BinOp(
                        IRBinOp::Eq,
                        Box::new(IRNode::Var("condition".into())),
                        Box::new(IRNode::Literal(IRLiteral::Bool(true))),
                    )),
                    then_branch: Box::new(IRNode::Assign {
                        target: Box::new(IRNode::Var("left".into())),
                        value: Box::new(IRNode::BinOp(
                            IRBinOp::Add,
                            Box::new(IRNode::Var("left".into())),
                            Box::new(IRNode::Literal(IRLiteral::Int(1))),
                        )),
                    }),
                    else_branch: Box::new(IRNode::Assign {
                        target: Box::new(IRNode::Var("right".into())),
                        value: Box::new(IRNode::BinOp(
                            IRBinOp::Add,
                            Box::new(IRNode::Var("right".into())),
                            Box::new(IRNode::Literal(IRLiteral::Int(1))),
                        )),
                    }),
                },
            ])),
        },
        IRNode::Return(Box::new(IRNode::Var("left".into()))),
    ])
}

fn build_sliding_window(_spec: &FunctionSpec) -> IRNode {
    IRNode::Block(vec![
        IRNode::Alloc { name: "window_sum".into(), ty: TypeRef::Int, initial: Box::new(IRNode::Literal(IRLiteral::Int(0))) },
        IRNode::Alloc { name: "max_sum".into(), ty: TypeRef::Int, initial: Box::new(IRNode::Literal(IRLiteral::Int(0))) },
        IRNode::For {
            var: "i".into(),
            start: Box::new(IRNode::Literal(IRLiteral::Int(0))),
            end: Box::new(IRNode::UnaryOp(IRUnaryOp::Len, Box::new(param_var(_spec, 0)))),
            body: Box::new(IRNode::Block(vec![
                IRNode::Assign {
                    target: Box::new(IRNode::Var("window_sum".into())),
                    value: Box::new(IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(IRNode::Var("window_sum".into())),
                        Box::new(IRNode::Index(
                            Box::new(param_var(_spec, 0)),
                            Box::new(IRNode::Var("i".into())),
                        )),
                    )),
                },
                IRNode::If {
                    condition: Box::new(IRNode::BinOp(
                        IRBinOp::Gt,
                        Box::new(IRNode::Var("window_sum".into())),
                        Box::new(IRNode::Var("max_sum".into())),
                    )),
                    then_branch: Box::new(IRNode::Assign {
                        target: Box::new(IRNode::Var("max_sum".into())),
                        value: Box::new(IRNode::Var("window_sum".into())),
                    }),
                    else_branch: Box::new(IRNode::Block(vec![])),
                },
            ])),
        },
        IRNode::Return(Box::new(IRNode::Var("max_sum".into()))),
    ])
}

fn build_fold(spec: &FunctionSpec) -> IRNode {
    IRNode::Block(vec![
        IRNode::Alloc {
            name: "acc".into(),
            ty: spec.return_type.clone(),
            initial: Box::new(IRNode::Literal(IRLiteral::Int(0))),
        },
        IRNode::For {
            var: "i".into(),
            start: Box::new(IRNode::Literal(IRLiteral::Int(0))),
            end: Box::new(IRNode::UnaryOp(IRUnaryOp::Len, Box::new(param_var(spec, 0)))),
            body: Box::new(IRNode::Assign {
                target: Box::new(IRNode::Var("acc".into())),
                value: Box::new(IRNode::Call {
                    function: "combine".into(),
                    args: vec![
                        IRNode::Var("acc".into()),
                        IRNode::Index(
                            Box::new(param_var(spec, 0)),
                            Box::new(IRNode::Var("i".into())),
                        ),
                    ],
                }),
            }),
        },
        IRNode::Return(Box::new(IRNode::Var("acc".into()))),
    ])
}

fn build_iterator_chain(spec: &FunctionSpec) -> IRNode {
    let p = param_var(spec, 0);
    IRNode::Return(Box::new(IRNode::Map {
        function: Box::new(IRNode::Lambda {
            params: vec![("x".into(), TypeRef::Named("T".into()))],
            body: Box::new(IRNode::Var("x".into())),
        }),
        over: Box::new(IRNode::Filter {
            predicate: Box::new(IRNode::Lambda {
                params: vec![("x".into(), TypeRef::Named("T".into()))],
                body: Box::new(IRNode::Literal(IRLiteral::Bool(true))),
            }),
            over: Box::new(p),
        }),
    }))
}

fn build_simple_compute(_spec: &FunctionSpec) -> IRNode {
    IRNode::Block(vec![
        IRNode::Return(Box::new(IRNode::Literal(IRLiteral::Int(0)))),
    ])
}

// ── Spec Shape Detection ──────────────────────────────────

fn has_sort_like_signature(spec: &FunctionSpec) -> bool {
    let ret_is_list = matches!(spec.return_type, TypeRef::List(_));
    let first_param_is_list = spec.params.first()
        .map(|p| matches!(p.ty, TypeRef::List(_)))
        .unwrap_or(false);
    ret_is_list && first_param_is_list
}

fn has_search_like_signature(spec: &FunctionSpec) -> bool {
    let ret_is_option = matches!(spec.return_type, TypeRef::Option(_));
    let ret_is_bool = matches!(spec.return_type, TypeRef::Bool);
    let first_is_list = spec.params.first()
        .map(|p| matches!(p.ty, TypeRef::List(_)))
        .unwrap_or(false);
    (ret_is_option || ret_is_bool) && first_is_list
}

fn has_fold_like_signature(spec: &FunctionSpec) -> bool {
    let ret_is_scalar = matches!(
        spec.return_type,
        TypeRef::Int | TypeRef::Float | TypeRef::Bool | TypeRef::String
    );
    let first_is_list = spec.params.first()
        .map(|p| matches!(p.ty, TypeRef::List(_)))
        .unwrap_or(false);
    ret_is_scalar && first_is_list
}

fn has_two_pointer_pattern(spec: &FunctionSpec) -> bool {
    let name = spec.name.to_lowercase();
    let keywords = ["find", "search", "pair", "subarray", "sum", "window", "duplicate", "palindrome"];
    keywords.iter().any(|k| name.contains(k))
}

// ── Helpers ────────────────────────────────────────────────

fn param_var(spec: &FunctionSpec, index: usize) -> IRNode {
    spec.params.get(index)
        .map(|p| IRNode::Var(p.name.clone()))
        .unwrap_or_else(|| IRNode::Literal(IRLiteral::Int(0)))
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::spec::ast::*;

    fn make_sort_spec() -> FunctionSpec {
        FunctionSpec {
            name: "sort".into(), doc: None, generics: vec![],
            params: vec![
                Param { name: "list".into(), ty: TypeRef::List(Box::new(TypeRef::Int)), annotations: HashMap::new() },
            ],
            return_type: TypeRef::List(Box::new(TypeRef::Int)),
            preconditions: vec![], postconditions: vec![], invariants: vec![],
            complexity: None, resource: None, tests: vec![], annotations: HashMap::new(),
        }
    }

    fn make_search_spec() -> FunctionSpec {
        FunctionSpec {
            name: "binary_search".into(), doc: None, generics: vec![],
            params: vec![
                Param { name: "list".into(), ty: TypeRef::List(Box::new(TypeRef::Int)), annotations: HashMap::new() },
                Param { name: "target".into(), ty: TypeRef::Int, annotations: HashMap::new() },
            ],
            return_type: TypeRef::Option(Box::new(TypeRef::Int)),
            preconditions: vec![], postconditions: vec![], invariants: vec![],
            complexity: None, resource: None, tests: vec![], annotations: HashMap::new(),
        }
    }

    #[test]
    fn test_sort_templates() {
        let spec = make_sort_spec();
        let templates = generate_templates(&spec);
        // Should include divide-conquer + iterator + linear scan + simple compute
        assert!(templates.len() >= 3);
        let kinds: Vec<_> = templates.iter().map(|(k, _)| *k).collect();
        assert!(kinds.contains(&TemplateKind::DivideAndConquer));
        assert!(kinds.contains(&TemplateKind::IteratorChain));
    }

    #[test]
    fn test_search_templates() {
        let spec = make_search_spec();
        let templates = generate_templates(&spec);
        let kinds: Vec<_> = templates.iter().map(|(k, _)| *k).collect();
        assert!(kinds.contains(&TemplateKind::BinarySearch));
        assert!(kinds.contains(&TemplateKind::LinearScan));
    }

    #[test]
    fn test_shape_detection() {
        assert!(has_sort_like_signature(&make_sort_spec()));
        assert!(has_search_like_signature(&make_search_spec()));
        assert!(!has_sort_like_signature(&make_search_spec()));
    }
}
