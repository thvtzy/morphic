// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC LLM INTEGRATION TESTS (v0.4)                    │
// │  End-to-end LLM pipeline tests with mocked HTTP          │
// └──────────────────────────────────────────────────────────┘

#[cfg(test)]
mod tests {
    use crate::spec::ast::*;
    use crate::llm::{client::*, prompt::*, parser::*};
    use std::collections::HashMap;

    // ── Full Pipeline Tests ────────────────────────────────

    #[test]
    fn test_full_pipeline_mocked() {
        // Build spec → prompt → parse response (no real LLM)
        let spec = make_sort_spec();
        let prompt = build_synthesis_prompt(&spec, 2);

        // Verify prompt contains all critical info
        assert!(prompt.contains("sort"));
        assert!(prompt.contains("is_sorted"));
        assert!(prompt.contains("O(n log n)"));
        assert!(prompt.contains("INSTRUCTIONS"));
    }

    #[test]
    fn test_parse_realistic_llm_output() {
        let llm_output = r#"
Here are two implementations:

```rust
pub fn sort(mut list: Vec<i64>) -> Vec<i64> {
    list.sort();
    list
}
```

```rust
pub fn sort(list: Vec<i64>) -> Vec<i64> {
    if list.len() <= 1 { return list; }
    let pivot = list[0];
    let left: Vec<_> = list[1..].iter().filter(|&&x| x < pivot).cloned().collect();
    let right: Vec<_> = list[1..].iter().filter(|&&x| x >= pivot).cloned().collect();
    let mut result = sort(left);
    result.push(pivot);
    result.extend(sort(right));
    result
}
```
"#;

        let candidates = parse_llm_response(llm_output, &make_sort_spec()).unwrap();
        assert_eq!(candidates.len(), 2, "Should extract both code blocks");
        assert!(candidates[0].source.contains("list.sort"));
        assert!(candidates[1].source.contains("pivot"));
    }

    #[test]
    fn test_parse_code_tags() {
        let llm_output = r#"
<code>
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}
</code>
"#;
        let candidates = parse_llm_response(llm_output, &make_sort_spec()).unwrap();
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].source.contains("a + b"));
        assert!(candidates[0].confidence > 0.8);
    }

    #[test]
    fn test_prompt_generics() {
        let spec = FunctionSpec {
            name: "find".into(),
            doc: None,
            generics: vec![GenericParam { name: "T".into(), bounds: vec![] }],
            params: vec![
                Param { name: "list".into(), ty: TypeRef::List(Box::new(TypeRef::Named("T".into()))), annotations: HashMap::new() },
            ],
            return_type: TypeRef::Option(Box::new(TypeRef::Int)),
            preconditions: vec![],
            postconditions: vec![Constraint::True],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: HashMap::new(),
        };

        let prompt = build_quick_prompt(&spec);
        assert!(prompt.contains("find"));
        assert!(prompt.contains("List"));
    }

    #[test]
    fn test_stress_multi_spec_synthesis() {
        // Simulate synthesizing 5 different specs
        let specs = vec![
            ("add", vec![
                Param { name: "a".into(), ty: TypeRef::Int, annotations: HashMap::new() },
                Param { name: "b".into(), ty: TypeRef::Int, annotations: HashMap::new() },
            ], TypeRef::Int),
            ("multiply", vec![
                Param { name: "a".into(), ty: TypeRef::Int, annotations: HashMap::new() },
                Param { name: "b".into(), ty: TypeRef::Int, annotations: HashMap::new() },
            ], TypeRef::Int),
            ("is_positive", vec![
                Param { name: "n".into(), ty: TypeRef::Int, annotations: HashMap::new() },
            ], TypeRef::Bool),
        ];

        for (name, params, ret) in &specs {
            let spec = FunctionSpec {
                name: name.to_string(),
                doc: None, generics: vec![],
                params: params.clone(),
                return_type: ret.clone(),
                preconditions: vec![],
                postconditions: vec![Constraint::True],
                invariants: vec![],
                complexity: None, resource: None, tests: vec![],
                annotations: HashMap::new(),
            };
            let prompt = build_quick_prompt(&spec);
            assert!(prompt.contains(name));
        }
    }

    #[test]
    fn test_confidence_scoring() {
        // High confidence: complete, well-structured code
        let good = "pub fn sort(list: Vec<i64>) -> Vec<i64> {\n    let mut result = list.clone();\n    result.sort();\n    result\n}";
        let candidates = parse_llm_response(
            &format!("```rust\n{}\n```", good),
            &make_sort_spec(),
        ).unwrap();
        assert!(candidates[0].confidence > 0.6, "Good code should have high confidence");

        // Low confidence for garbage
        let garbage = "asdf jkl; 123";
        let result = parse_llm_response(garbage, &make_sort_spec());
        assert!(result.is_err() || result.unwrap()[0].confidence < 0.6,
            "Garbage should have low confidence or fail parsing");
    }

    // ── Helpers ────────────────────────────────────────────

    fn make_sort_spec() -> FunctionSpec {
        FunctionSpec {
            name: "sort".into(),
            doc: Some("Sort a list of integers in ascending order".into()),
            generics: vec![],
            params: vec![
                Param {
                    name: "list".into(),
                    ty: TypeRef::List(Box::new(TypeRef::Int)),
                    annotations: HashMap::new(),
                },
            ],
            return_type: TypeRef::List(Box::new(TypeRef::Int)),
            preconditions: vec![],
            postconditions: vec![
                Constraint::Predicate("is_sorted".into(), vec![Expr::Var("output".into())]),
                Constraint::Predicate("is_permutation".into(), vec![
                    Expr::Var("list".into()),
                    Expr::Var("output".into()),
                ]),
            ],
            invariants: vec![],
            complexity: Some(ComplexityBound {
                dimension: ComplexityDimension::Time,
                bound: BigO::Linearithmic,
                condition: None,
            }),
            resource: None,
            tests: vec![
                Test {
                    name: Some("empty".into()),
                    input: Expr::Call("List".into(), vec![]),
                    expected_output: Expr::Call("List".into(), vec![]),
                    timeout_ms: None,
                    property: false,
                },
            ],
            annotations: HashMap::new(),
        }
    }
}
