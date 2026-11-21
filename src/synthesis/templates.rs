// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC TEMPLATE LIBRARY                                │
// │  Structural patterns for synthesis seeding               │
// │                                                          │
// │  Templates are complete IR bodies (no holes) keyed       │
// │  STRUCTURALLY on the spec's signature: parameter and     │
// │  return types, plus the comparator evidence a            │
// │  function-typed parameter carries. Never on spec         │
// │  names. A template whose shape cannot instantiate        │
// │  cleanly for a spec does not fire for it.                │
// │                                                          │
// │  Phase 59 (G2) rewired this library into the engine's    │
// │  seed_from_templates (it previously had no production    │
// │  caller, and the engine's own builders were bare         │
// │  Holes). The v0.4 builders were removed, not repaired:   │
// │  they were stale against current signatures (the         │
// │  divide-and-conquer recursion passed one argument to     │
// │  a two-parameter spec; binary search returned a          │
// │  tuple-wrapped index) and were built from pseudo-        │
// │  calls ("first", "rest", "concat", "transform",          │
// │  "push", "combine") that render as undefined free        │
// │  functions; the two-pointer detection keyed on spec      │
// │  NAME keywords, which the structural-keying rule         │
// │  forbids. What remains is the three templates whose      │
// │  bodies are built only from nodes the Rust emitter       │
// │  renders and the selector's type verdict does not        │
// │  refute — measured in the Phase 59 support matrix.       │
// └──────────────────────────────────────────────────────────┘

use crate::spec::ast::{FunctionSpec, TypeRef};
use super::engine::{IRNode, IRLiteral, IRBinOp, IRUnaryOp};

/// All available template patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateKind {
    /// A manual bubble sort in the proven scan vocabulary
    /// (Alloc/For/Index/Assign/Let/If), calling the spec's
    /// comparator parameter DIRECTLY as a function value.
    /// Phase 61 replaced the Phase 59 construction (the IR
    /// Sort node): the Sort node renders `sort_by(cmp)`,
    /// which rustc rejects E0631 against the generated
    /// signature `cmp: fn(T, T) -> bool` — and no adapter
    /// lambda reaches an `Ordering` either (Phase 59's
    /// support matrix). The scan form's remaining wall is
    /// one level deeper and is recorded, not hidden: every
    /// element read goes through `.clone()` (the IR's
    /// Clone unary op), because a plain index read is a
    /// move out of the list (E0507, probed in Phase 61 for
    /// the plain-read and insertion constructions alike);
    /// under the generated signature's bound-free `T` the
    /// clone read itself is E0599. The construction is the
    /// unique probed one that compiles the moment the
    /// signature carries a `Clone` bound (Phase 61 priced
    /// the unlock: probe-only `sort<T: Clone>` renders of
    /// this body compile and pass 6/6; the plain-read body
    /// still dies E0507 there). The bound is an
    /// emitter-contract decision, reserved for the owner;
    /// this template is the ready construction for it.
    SortPrimitive,
    /// Adjacent-pair scan: Alloc a Bool accumulator, walk
    /// `1..len(list)` once, clear the accumulator on any
    /// out-of-order adjacent pair.
    AdjacentScan,
    /// Binary search over the list parameter, comparisons
    /// threaded through the spec's Int-valued comparator
    /// parameter, `Some(mid)` / `None` returns.
    BinarySearch,
}

impl TemplateKind {
    /// The provenance name recorded on seeded candidates.
    pub fn name(&self) -> &'static str {
        match self {
            TemplateKind::SortPrimitive => "sort_primitive",
            TemplateKind::AdjacentScan => "adjacent_scan",
            TemplateKind::BinarySearch => "binary_search",
        }
    }
}

/// Generate IR templates for a given spec. At most one
/// template fires per spec: the three keying shapes are
/// mutually exclusive (a List→List sort with a Bool
/// comparator, a single-param Bool scan over a concrete
/// element type, an Option-returning search with an Int
/// comparator).
pub fn generate_templates(spec: &FunctionSpec) -> Vec<(TemplateKind, IRNode)> {
    let mut templates = Vec::new();

    if let Some(shape) = sort_shape(spec) {
        templates.push((TemplateKind::SortPrimitive, build_sort_primitive(&shape, spec)));
    } else if let Some(shape) = search_shape(spec) {
        templates.push((TemplateKind::BinarySearch, build_binary_search(&shape)));
    } else if let Some(shape) = scan_shape(spec) {
        templates.push((TemplateKind::AdjacentScan, build_adjacent_scan(&shape)));
    }

    templates
}

// ── Keying (structural — types and comparator evidence only) ──

/// Sort shape: the return type is `List<E>`, some parameter
/// is `List<E>` for the SAME `E`, and some parameter is a
/// function `(E, E) -> Bool` — the comparator evidence. The
/// same-E discipline excludes map-like shapes
/// (`List<T>, fn(T) -> U -> List<U>`).
struct SortShape {
    list_param: String,
    cmp_param: String,
}

fn sort_shape(spec: &FunctionSpec) -> Option<SortShape> {
    let TypeRef::List(elem) = &spec.return_type else {
        return None;
    };
    let list_param = spec
        .params
        .iter()
        .find(|p| matches!(&p.ty, TypeRef::List(e) if e == elem))
        .map(|p| p.name.clone())?;
    let cmp_param = spec
        .params
        .iter()
        .find(|p| {
            matches!(
                &p.ty,
                TypeRef::Function(args, ret)
                    if args.len() == 2
                        && &args[0] == elem.as_ref()
                        && &args[1] == elem.as_ref()
                        && ret.as_ref() == &TypeRef::Bool
            )
        })
        .map(|p| p.name.clone())?;
    Some(SortShape { list_param, cmp_param })
}

/// Search shape: the return type is `Option<_>`, the first
/// parameter is `List<E>`, the second is `E` itself (the
/// needle), and some parameter is a function
/// `(E, E) -> Int` — the three-way comparator evidence.
struct SearchShape {
    list_param: String,
    needle_param: String,
    cmp_param: String,
}

fn search_shape(spec: &FunctionSpec) -> Option<SearchShape> {
    if !matches!(spec.return_type, TypeRef::Option(_)) {
        return None;
    }
    let first = spec.params.first()?;
    let TypeRef::List(elem) = &first.ty else {
        return None;
    };
    let second = spec.params.get(1)?;
    if &second.ty != elem.as_ref() {
        return None;
    }
    let cmp_param = spec
        .params
        .iter()
        .find(|p| {
            matches!(
                &p.ty,
                TypeRef::Function(args, ret)
                    if args.len() == 2
                        && &args[0] == elem.as_ref()
                        && &args[1] == elem.as_ref()
                        && ret.as_ref() == &TypeRef::Int
            )
        })
        .map(|p| p.name.clone())?;
    Some(SearchShape {
        list_param: first.name.clone(),
        needle_param: second.name.clone(),
        cmp_param,
    })
}

/// Adjacent-scan shape: exactly one parameter, of type
/// `List<E>` where `E` is a CONCRETE ordered scalar (the
/// rendered scan compares elements with `<=`, which needs a
/// concrete ordered element type — a generic element cannot
/// be compared under the bound-free generated signature),
/// and a `Bool` return.
struct ScanShape {
    list_param: String,
}

fn scan_shape(spec: &FunctionSpec) -> Option<ScanShape> {
    if spec.return_type != TypeRef::Bool || spec.params.len() != 1 {
        return None;
    }
    let TypeRef::List(elem) = &spec.params[0].ty else {
        return None;
    };
    if !is_concrete_ordered_scalar(elem) {
        return None;
    }
    Some(ScanShape {
        list_param: spec.params[0].name.clone(),
    })
}

/// Element types whose rendered values support `<=` in the
/// generated Rust without trait bounds: the fixed-width and
/// arbitrary-precision integers, floats, chars, and strings.
/// (Bool is excluded: an adjacency ordering over booleans is
/// not the scan's meaning. Generic/Named elements are
/// excluded: unbounded type parameters have no order.)
/// `pub(crate)` since Phase 60: the forall-fold constraint
/// family (synthesis::engine) applies the same admission
/// discipline to the elements its mined predicate compares.
pub(crate) fn is_concrete_ordered_scalar(ty: &TypeRef) -> bool {
    matches!(
        ty,
        TypeRef::Int
            | TypeRef::I8
            | TypeRef::I16
            | TypeRef::I32
            | TypeRef::I64
            | TypeRef::U8
            | TypeRef::U16
            | TypeRef::U32
            | TypeRef::U64
            | TypeRef::Float
            | TypeRef::Char
            | TypeRef::String
    )
}

// ── Template Builders ──────────────────────────────────────

fn build_sort_primitive(shape: &SortShape, spec: &FunctionSpec) -> IRNode {
    let list = &shape.list_param;
    let cmp = &shape.cmp_param;
    let taken: Vec<&str> = vec![list.as_str(), cmp.as_str()];
    let a = fresh_name("a", &taken);
    let i = fresh_name("i", &[taken[0], taken[1], a.as_str()]);
    let j = fresh_name("j", &[taken[0], taken[1], a.as_str(), i.as_str()]);
    let tmp = fresh_name("tmp", &[taken[0], taken[1], a.as_str(), i.as_str(), j.as_str()]);

    let var = |n: &str| IRNode::Var(n.to_string());
    let int = |n: i64| IRNode::Literal(IRLiteral::Int(n));
    // An element read: the Index place through the Clone
    // unary op — see the TemplateKind::SortPrimitive doc
    // for why a plain read cannot be used (E0507) and why
    // the clone is the priced-unlock form (E0599 today).
    let read = |index: IRNode| {
        IRNode::UnaryOp(
            IRUnaryOp::Clone,
            Box::new(IRNode::Index(Box::new(var(&a)), Box::new(index))),
        )
    };
    let jm1 = || {
        IRNode::BinOp(
            IRBinOp::Sub,
            Box::new(var(&j)),
            Box::new(int(1)),
        )
    };

    IRNode::Block(vec![
        IRNode::Alloc {
            name: a.clone(),
            ty: spec.return_type.clone(),
            initial: Box::new(var(list)),
        },
        IRNode::For {
            var: i.clone(),
            start: Box::new(int(0)),
            end: Box::new(IRNode::UnaryOp(
                IRUnaryOp::Len,
                Box::new(var(&a)),
            )),
            body: Box::new(IRNode::For {
                var: j.clone(),
                start: Box::new(int(1)),
                end: Box::new(IRNode::UnaryOp(
                    IRUnaryOp::Len,
                    Box::new(var(&a)),
                )),
                body: Box::new(IRNode::If {
                    condition: Box::new(IRNode::Call {
                        function: cmp.clone(),
                        args: vec![read(var(&j)), read(jm1())],
                    }),
                    then_branch: Box::new(IRNode::Let {
                        name: tmp.clone(),
                        value: Box::new(read(jm1())),
                        body: Box::new(IRNode::Block(vec![
                            IRNode::Assign {
                                target: Box::new(IRNode::Index(
                                    Box::new(var(&a)),
                                    Box::new(jm1()),
                                )),
                                value: Box::new(read(var(&j))),
                            },
                            IRNode::Assign {
                                target: Box::new(IRNode::Index(
                                    Box::new(var(&a)),
                                    Box::new(var(&j)),
                                )),
                                value: Box::new(var(&tmp)),
                            },
                        ])),
                    }),
                    else_branch: Box::new(IRNode::Block(vec![])),
                }),
            }),
        },
        IRNode::Return(Box::new(var(&a))),
    ])
}

fn build_adjacent_scan(shape: &ScanShape) -> IRNode {
    let list = &shape.list_param;
    let result = fresh_name("result", &[list.as_str()]);
    let i = fresh_name("i", &[list.as_str(), result.as_str()]);
    IRNode::Block(vec![
        IRNode::Alloc {
            name: result.clone(),
            ty: TypeRef::Bool,
            initial: Box::new(IRNode::Literal(IRLiteral::Bool(true))),
        },
        IRNode::For {
            var: i.clone(),
            start: Box::new(IRNode::Literal(IRLiteral::Int(1))),
            end: Box::new(IRNode::UnaryOp(
                IRUnaryOp::Len,
                Box::new(IRNode::Var(list.clone())),
            )),
            body: Box::new(IRNode::Assign {
                target: Box::new(IRNode::Var(result.clone())),
                value: Box::new(IRNode::BinOp(
                    IRBinOp::And,
                    Box::new(IRNode::Var(result.clone())),
                    Box::new(IRNode::BinOp(
                        IRBinOp::Le,
                        Box::new(IRNode::Index(
                            Box::new(IRNode::Var(list.clone())),
                            Box::new(IRNode::BinOp(
                                IRBinOp::Sub,
                                Box::new(IRNode::Var(i.clone())),
                                Box::new(IRNode::Literal(IRLiteral::Int(1))),
                            )),
                        )),
                        Box::new(IRNode::Index(
                            Box::new(IRNode::Var(list.clone())),
                            Box::new(IRNode::Var(i)),
                        )),
                    )),
                )),
            }),
        },
        IRNode::Return(Box::new(IRNode::Var(result))),
    ])
}

fn build_binary_search(shape: &SearchShape) -> IRNode {
    let haystack = &shape.list_param;
    let needle = &shape.needle_param;
    let cmp = &shape.cmp_param;
    let taken: Vec<&str> = vec![haystack.as_str(), needle.as_str(), cmp.as_str()];
    let lo = fresh_name("lo", &taken);
    let hi = fresh_name("hi", &taken);
    let mid = fresh_name("mid", &[taken[0], taken[1], taken[2], lo.as_str(), hi.as_str()]);

    let var = |n: &str| IRNode::Var(n.to_string());
    let int = |n: i64| IRNode::Literal(IRLiteral::Int(n));
    let cmp_mid = || {
        IRNode::Call {
            function: cmp.clone(),
            args: vec![
                IRNode::Index(
                    Box::new(var(haystack)),
                    Box::new(var(&mid)),
                ),
                var(needle),
            ],
        }
    };

    IRNode::Block(vec![
        IRNode::Alloc {
            name: lo.clone(),
            ty: TypeRef::Int,
            initial: Box::new(int(0)),
        },
        IRNode::Alloc {
            name: hi.clone(),
            ty: TypeRef::Int,
            initial: Box::new(IRNode::UnaryOp(
                IRUnaryOp::Len,
                Box::new(var(haystack)),
            )),
        },
        IRNode::While {
            condition: Box::new(IRNode::BinOp(
                IRBinOp::Lt,
                Box::new(var(&lo)),
                Box::new(var(&hi)),
            )),
            invariant: None,
            body: Box::new(IRNode::Let {
                name: mid.clone(),
                value: Box::new(IRNode::BinOp(
                    IRBinOp::Add,
                    Box::new(var(&lo)),
                    Box::new(IRNode::BinOp(
                        IRBinOp::Div,
                        Box::new(IRNode::BinOp(
                            IRBinOp::Sub,
                            Box::new(var(&hi)),
                            Box::new(var(&lo)),
                        )),
                        Box::new(int(2)),
                    )),
                )),
                body: Box::new(IRNode::If {
                    condition: Box::new(IRNode::BinOp(
                        IRBinOp::Eq,
                        Box::new(cmp_mid()),
                        Box::new(int(0)),
                    )),
                    then_branch: Box::new(IRNode::Return(Box::new(IRNode::Call {
                        function: "Some".into(),
                        args: vec![var(&mid)],
                    }))),
                    else_branch: Box::new(IRNode::If {
                        condition: Box::new(IRNode::BinOp(
                            IRBinOp::Lt,
                            Box::new(cmp_mid()),
                            Box::new(int(0)),
                        )),
                        then_branch: Box::new(IRNode::Assign {
                            target: Box::new(var(&lo)),
                            value: Box::new(IRNode::BinOp(
                                IRBinOp::Add,
                                Box::new(var(&mid)),
                                Box::new(int(1)),
                            )),
                        }),
                        else_branch: Box::new(IRNode::Assign {
                            target: Box::new(var(&hi)),
                            value: Box::new(var(&mid)),
                        }),
                    }),
                }),
            }),
        },
        // `None` is a variant, not a call: a bare Var renders
        // it exactly (the v0.4 builder's one sound piece).
        IRNode::Return(Box::new(var("None"))),
    ])
}

/// A local name that does not collide with any of the
/// spec's parameter names (a template local shadowing the
/// parameter it reads would silently change the body).
/// `pub(crate)` since Phase 60: the forall-fold constraint
/// family (synthesis::engine) names its scan locals by the
/// same discipline.
pub(crate) fn fresh_name(base: &str, taken: &[&str]) -> String {
    let mut name = base.to_string();
    while taken.iter().any(|t| *t == name) {
        name.push('_');
    }
    name
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::spec::ast::*;

    fn param(name: &str, ty: TypeRef) -> Param {
        Param {
            name: name.into(),
            ty,
            annotations: HashMap::new(),
        }
    }

    fn spec(
        name: &str,
        generics: Vec<String>,
        params: Vec<Param>,
        return_type: TypeRef,
    ) -> FunctionSpec {
        FunctionSpec {
            name: name.into(),
            doc: None,
            generics: generics
                .into_iter()
                .map(|name| GenericParam { name, bounds: vec![] })
                .collect(),
            params,
            return_type,
            preconditions: vec![],
            postconditions: vec![],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: HashMap::new(),
        }
    }

    fn generic(name: &str) -> TypeRef {
        TypeRef::Generic(name.into())
    }

    fn sort_spec(name: &str) -> FunctionSpec {
        spec(
            name,
            vec!["T".into()],
            vec![
                param("list", TypeRef::List(Box::new(generic("T")))),
                param(
                    "cmp",
                    TypeRef::Function(
                        vec![generic("T"), generic("T")],
                        Box::new(TypeRef::Bool),
                    ),
                ),
            ],
            TypeRef::List(Box::new(generic("T"))),
        )
    }

    fn search_spec(name: &str) -> FunctionSpec {
        spec(
            name,
            vec!["T".into()],
            vec![
                param("haystack", TypeRef::List(Box::new(generic("T")))),
                param("needle", generic("T")),
                param(
                    "cmp",
                    TypeRef::Function(
                        vec![generic("T"), generic("T")],
                        Box::new(TypeRef::Int),
                    ),
                ),
            ],
            TypeRef::Option(Box::new(TypeRef::I64)),
        )
    }

    #[test]
    fn sort_shape_fires_sort_primitive() {
        let templates = generate_templates(&sort_spec("sort"));
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].0, TemplateKind::SortPrimitive);
        // Phase 61: the construction is the scan-form bubble
        // sort — a Block, no Sort node anywhere, the
        // comparator parameter called directly, hole-free.
        fn contains_sort(node: &IRNode) -> bool {
            match node {
                IRNode::Sort { .. } => true,
                IRNode::Block(stmts) => stmts.iter().any(contains_sort),
                IRNode::Let { value, body, .. } => contains_sort(value) || contains_sort(body),
                IRNode::If { condition, then_branch, else_branch } => {
                    contains_sort(condition) || contains_sort(then_branch) || contains_sort(else_branch)
                }
                IRNode::For { start, end, body, .. } => {
                    contains_sort(start) || contains_sort(end) || contains_sort(body)
                }
                IRNode::Call { args, .. } => args.iter().any(contains_sort),
                IRNode::Return(inner) => contains_sort(inner),
                IRNode::BinOp(_, l, r) => contains_sort(l) || contains_sort(r),
                IRNode::UnaryOp(_, inner) => contains_sort(inner),
                IRNode::Index(c, i) => contains_sort(c) || contains_sort(i),
                IRNode::Assign { target, value } => contains_sort(target) || contains_sort(value),
                IRNode::Alloc { initial, .. } => contains_sort(initial),
                _ => false,
            }
        }
        let body = &templates[0].1;
        assert!(matches!(body, IRNode::Block(_)), "scan form is a Block: {:?}", body);
        assert!(!contains_sort(body), "the IR Sort node must not appear: {:?}", body);
        let text = format!("{:?}", body);
        assert!(text.contains("function: \"cmp\""), "comparator called directly: {}", text);
        assert_eq!(crate::synthesis::engine::count_holes(body), 0, "template bodies are hole-free");
    }

    #[test]
    fn keying_is_name_independent() {
        // The same signature under an unrelated name fires;
        // the flagship name over a scalar signature does not.
        assert_eq!(generate_templates(&sort_spec("zzz")).len(), 1);
        let scalar_named_sort = spec(
            "sort",
            vec![],
            vec![param("x", TypeRef::Int)],
            TypeRef::Int,
        );
        assert!(generate_templates(&scalar_named_sort).is_empty());
    }

    #[test]
    fn sort_shape_needs_the_comparator_evidence() {
        // List<T> -> List<T> with no function parameter: no
        // comparator evidence, no template.
        let no_cmp = spec(
            "reverse",
            vec!["T".into()],
            vec![param("list", TypeRef::List(Box::new(generic("T"))))],
            TypeRef::List(Box::new(generic("T"))),
        );
        assert!(generate_templates(&no_cmp).is_empty());
        // A map shape: fn(T) -> U with a List<U> return is
        // not a sort (the same-E discipline).
        let map_shape = spec(
            "mapped",
            vec!["T".into(), "U".into()],
            vec![
                param("list", TypeRef::List(Box::new(generic("T")))),
                param(
                    "f",
                    TypeRef::Function(vec![generic("T")], Box::new(generic("U"))),
                ),
            ],
            TypeRef::List(Box::new(generic("U"))),
        );
        assert!(generate_templates(&map_shape).is_empty());
    }

    #[test]
    fn search_shape_fires_binary_search_with_some_and_none() {
        let templates = generate_templates(&search_spec("binary_search"));
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].0, TemplateKind::BinarySearch);
        let text = format!("{:?}", templates[0].1);
        // Some(mid) is a Call (renders `Some(mid)`); the None
        // return is a bare Var (renders `None`, not `None()`).
        assert!(text.contains("function: \"Some\""), "{}", text);
        assert!(text.contains("Var(\"None\")"), "{}", text);
        // Comparisons go through the comparator parameter.
        assert!(text.contains("function: \"cmp\""), "{}", text);
    }

    #[test]
    fn search_shape_needs_option_return_and_int_comparator() {
        let mut no_option = search_spec("binary_search");
        no_option.return_type = TypeRef::I64;
        assert!(generate_templates(&no_option).is_empty());
        let mut bool_cmp = search_spec("binary_search");
        bool_cmp.params[2].ty = TypeRef::Function(
            vec![generic("T"), generic("T")],
            Box::new(TypeRef::Bool),
        );
        // A Bool comparator with an Option return matches
        // neither the search shape (Int comparator) nor the
        // sort shape (List return).
        assert!(generate_templates(&bool_cmp).is_empty());
    }

    #[test]
    fn scan_shape_fires_for_concrete_element_bool_return() {
        let s = spec(
            "is_sorted",
            vec![],
            vec![param("list", TypeRef::List(Box::new(TypeRef::Int)))],
            TypeRef::Bool,
        );
        let templates = generate_templates(&s);
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].0, TemplateKind::AdjacentScan);
    }

    #[test]
    fn scan_shape_rejects_generic_elements_and_extra_params() {
        // Generic element: `<=` cannot render under the
        // bound-free signature.
        let generic_elem = spec(
            "all_ordered",
            vec!["T".into()],
            vec![param("list", TypeRef::List(Box::new(generic("T"))))],
            TypeRef::Bool,
        );
        assert!(generate_templates(&generic_elem).is_empty());
        // Two parameters is not the scan shape.
        let two_params = spec(
            "check",
            vec![],
            vec![
                param("list", TypeRef::List(Box::new(TypeRef::Int))),
                param("strict", TypeRef::Bool),
            ],
            TypeRef::Bool,
        );
        assert!(generate_templates(&two_params).is_empty());
    }

    #[test]
    fn fold_shape_fires_nothing_in_g2() {
        // count_matching's shape (List<Int>, Int -> Int) is
        // the forall-fold family's territory (campaign slice
        // G3): G2 records the fit and emits nothing.
        let s = spec(
            "count_matching",
            vec![],
            vec![
                param("list", TypeRef::List(Box::new(TypeRef::Int))),
                param("threshold", TypeRef::Int),
            ],
            TypeRef::Int,
        );
        assert!(generate_templates(&s).is_empty());
    }

    #[test]
    fn scalar_and_string_shapes_fire_nothing() {
        // The M2/M3 corpus shapes: scalar and String
        // signatures (no List-typed parameter anywhere).
        let cases = vec![
            spec(
                "clamp",
                vec![],
                vec![param("x", TypeRef::Int), param("lo", TypeRef::Int)],
                TypeRef::Int,
            ),
            spec(
                "emit",
                vec![],
                vec![param("s", TypeRef::String), param("n", TypeRef::Int)],
                TypeRef::String,
            ),
            spec(
                "join",
                vec![],
                vec![param("parts", TypeRef::List(Box::new(TypeRef::String)))],
                TypeRef::String,
            ),
        ];
        for s in &cases {
            assert!(
                generate_templates(s).is_empty(),
                "{} must not fire a template",
                s.name
            );
        }
    }
}
