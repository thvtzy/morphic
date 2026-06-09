// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC LLM PROMPTS                                     │
// │  Prompt engineering for synthesis candidate generation    │
// └──────────────────────────────────────────────────────────┘

use crate::spec::ast::*;

/// Build the system + user prompt for synthesizing implementations
pub fn build_synthesis_prompt(spec: &FunctionSpec, num_candidates: usize) -> String {
    let mut prompt = String::new();

    // System role description
    prompt.push_str(&format!(
        "You are Morphic, an AI program synthesis engine.\n\
         Given a formal specification, generate {} complete, correct Rust implementation(s).\n\
         Each implementation must be a separate code block.\n\n",
        num_candidates
    ));

    // Specification context
    prompt.push_str("=== SPECIFICATION ===\n\n");
    prompt.push_str(&format!("Function: {}\n", spec.name));

    // Parameters
    let params: Vec<String> = spec.params.iter()
        .map(|p| format!("{}: {}", p.name, p.ty))
        .collect();
    prompt.push_str(&format!("Input: {}\n", params.join(", ")));
    prompt.push_str(&format!("Output: {}\n", spec.return_type));

    // Doc comment
    if let Some(ref doc) = spec.doc {
        prompt.push_str(&format!("Description: {}\n", doc));
    }

    // Preconditions
    if !spec.preconditions.is_empty() {
        prompt.push_str("\n--- Preconditions (must be true before execution) ---\n");
        for pre in &spec.preconditions {
            prompt.push_str(&format!("  REQUIRE: {}\n", format_constraint(pre)));
        }
    }

    // Postconditions + constraints
    if !spec.postconditions.is_empty() {
        prompt.push_str("\n--- Postconditions (must be true after execution) ---\n");
        for post in &spec.postconditions {
            prompt.push_str(&format!("  ENSURE: {}\n", format_constraint(post)));
        }
    }

    // Constraints
    if !spec.postconditions.is_empty() || spec.preconditions.is_empty() {
        prompt.push_str("\n--- Constraints ---\n");
        prompt.push_str("The implementation MUST satisfy ALL of the above.\n");
    }

    // Complexity bounds
    if let Some(ref cplx) = spec.complexity {
        prompt.push_str(&format!(
            "\n--- Performance Requirement ---\n  {}: {}\n",
            match cplx.dimension {
                ComplexityDimension::Time => "Time complexity",
                ComplexityDimension::Space => "Space complexity",
                ComplexityDimension::AmortizedTime => "Amortized time",
                ComplexityDimension::Communication => "Communication",
            },
            format_big_o(&cplx.bound)
        ));
    }

    // Test cases
    if !spec.tests.is_empty() {
        prompt.push_str("\n--- Expected Behavior (tests) ---\n");
        for (i, test) in spec.tests.iter().enumerate() {
            let fallback = format!("test {}", i + 1);
            let tn = test.name.as_deref().unwrap_or(&fallback);
            prompt.push_str(&format!(
                "  {}: input = {} → expected = {}\n",
                tn,
                format_expr(&test.input),
                format_expr(&test.expected_output),
            ));
        }
    }

    // Generics info
    if !spec.generics.is_empty() {
        let gens: Vec<String> = spec.generics.iter()
            .map(|g| format!("{}", g.name))
            .collect();
        prompt.push_str(&format!("\nGeneric type parameters: {}\n", gens.join(", ")));
    }

    // Instruction block
    prompt.push_str("\n=== INSTRUCTIONS ===\n\n");
    prompt.push_str(&format!(
        "Generate {} implementation(s) for `{}` as Rust function bodies.\n\
         Each implementation MUST:\n\
         1. Be syntactically correct Rust code\n\
         2. Satisfy ALL constraints and conditions\n\
         3. Handle ALL edge cases\n\
         4. Meet the complexity bounds (if specified)\n\
         5. Be idiomatic and readable\n\n\
         Return each implementation separately. Start each with:\n\
         ```rust\n\
         pub fn {}(...) -> ... {{\n\
         ...\n\
         }}\n\
         ```\n",
        num_candidates, spec.name, spec.name
    ));

    // Emphasize diversity
    if num_candidates > 1 {
        prompt.push_str(&format!(
            "\nIMPORTANT: Generate {} DIFFERENT approaches (e.g., iterative, recursive, using iterators, functional style).\n",
            num_candidates
        ));
    }

    prompt
}

/// Build a lighter prompt for quick candidate generation
pub fn build_quick_prompt(spec: &FunctionSpec) -> String {
    format!(
        "Generate Rust code for a function `{}` that takes ({}) and returns {}.\n\
         Constraints: {}\n\
         Tests: {}\n\
         Return ONLY valid Rust code.",
        spec.name,
        spec.params.iter().map(|p| format!("{}: {}", p.name, p.ty)).collect::<Vec<_>>().join(", "),
        spec.return_type,
        spec.postconditions.iter()
            .map(|c| format_constraint(c))
            .collect::<Vec<_>>()
            .join("; "),
        spec.tests.iter()
            .map(|t| format!("{} → {}", format_expr(&t.input), format_expr(&t.expected_output)))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

// ── Formatting Helpers ────────────────────────────────────

fn format_constraint(c: &Constraint) -> String {
    match c {
        Constraint::True => "true".into(),
        Constraint::Expr(e) => format_expr(e),
        Constraint::Eq(a, b) => format!("{} == {}", format_expr(a), format_expr(b)),
        Constraint::Order(op, a, b) => {
            let op_str = match op {
                OrderOp::LessThan => "<",
                OrderOp::LessThanOrEqual => "<=",
                OrderOp::GreaterThan => ">",
                OrderOp::GreaterThanOrEqual => ">=",
            };
            format!("{} {} {}", format_expr(a), op_str, format_expr(b))
        }
        Constraint::And(parts) => {
            parts.iter()
                .map(format_constraint)
                .collect::<Vec<_>>()
                .join(" AND ")
        }
        Constraint::Implies(a, b) => {
            format!("({} IMPLIES {})", format_constraint(a), format_constraint(b))
        }
        Constraint::Not(inner) => format!("NOT({})", format_constraint(inner)),
        Constraint::Forall { vars, body } => {
            let v: Vec<String> = vars.iter().map(|(n, t)| format!("{}: {}", n, t)).collect();
            format!("FORALL {}: {}", v.join(", "), format_constraint(body))
        }
        Constraint::Predicate(name, args) => {
            let a: Vec<String> = args.iter().map(format_expr).collect();
            format!("{}({})", name, a.join(", "))
        }
        _ => format!("{:?}", c),
    }
}

fn format_expr(expr: &Expr) -> String {
    match expr {
        Expr::IntLit(n) => n.to_string(),
        Expr::FloatLit(f) => f.to_string(),
        Expr::BoolLit(b) => b.to_string(),
        Expr::StringLit(s) => format!("\"{}\"", s),
        Expr::Var(name) => name.clone(),
        Expr::Field(base, field) => format!("{}.{}", format_expr(base), field),
        Expr::Index(base, idx) => format!("{}[{}]", format_expr(base), format_expr(idx)),
        Expr::Call(name, args) => {
            let a: Vec<String> = args.iter().map(format_expr).collect();
            format!("{}({})", name, a.join(", "))
        }
        Expr::BinOp(op, l, r) => {
            let op_str = match op {
                BinOp::Add => "+", BinOp::Sub => "-",
                BinOp::Mul => "*", BinOp::Div => "/",
                BinOp::Eq => "==", BinOp::Neq => "!=",
                BinOp::Lt => "<", BinOp::Le => "<=",
                BinOp::Gt => ">", BinOp::Ge => ">=",
                BinOp::And => "&&", BinOp::Or => "||",
                BinOp::Mod => "%", BinOp::Concat => "++",
            };
            format!("({} {} {})", format_expr(l), op_str, format_expr(r))
        }
        Expr::UnaryOp(op, inner) => {
            let op_str = match op {
                UnaryOp::Neg => "-", UnaryOp::Not => "!",
            };
            format!("({}{})", op_str, format_expr(inner))
        }
        _ => format!("{:?}", expr),
    }
}

fn format_big_o(bound: &BigO) -> String {
    match bound {
        BigO::Constant => "O(1)".into(),
        BigO::Logarithmic => "O(log n)".into(),
        BigO::Linear => "O(n)".into(),
        BigO::Linearithmic => "O(n log n)".into(),
        BigO::Quadratic => "O(n²)".into(),
        BigO::Cubic => "O(n³)".into(),
        BigO::Polynomial(k) => format!("O(n^{})", k),
        BigO::Exponential(k) => format!("O({}^n)", k),
        BigO::Custom(s) => format!("O({})", s),
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_test_spec() -> FunctionSpec {
        FunctionSpec {
            name: "sort".into(),
            doc: Some("Sort a list".into()),
            generics: vec![],
            params: vec![
                Param { name: "list".into(), ty: TypeRef::List(Box::new(TypeRef::Int)), annotations: HashMap::new() },
            ],
            return_type: TypeRef::List(Box::new(TypeRef::Int)),
            preconditions: vec![],
            postconditions: vec![
                Constraint::Predicate("is_sorted".into(), vec![Expr::Var("output".into())]),
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
                    name: Some("simple".into()),
                    input: Expr::Call("List".into(), vec![
                        Expr::IntLit(3), Expr::IntLit(1), Expr::IntLit(2),
                    ]),
                    expected_output: Expr::Call("List".into(), vec![
                        Expr::IntLit(1), Expr::IntLit(2), Expr::IntLit(3),
                    ]),
                    timeout_ms: None,
                    property: false,
                },
            ],
            annotations: HashMap::new(),
        }
    }

    #[test]
    fn test_prompt_contains_spec_name() {
        let spec = make_test_spec();
        let prompt = build_synthesis_prompt(&spec, 1);
        assert!(prompt.contains("sort"));
        assert!(prompt.contains("is_sorted"));
        assert!(prompt.contains("O(n log n)"));
    }

    #[test]
    fn test_prompt_contains_tests() {
        let spec = make_test_spec();
        let prompt = build_synthesis_prompt(&spec, 1);
        assert!(prompt.contains("3, 1, 2"));
        assert!(prompt.contains("1, 2, 3"));
    }

    #[test]
    fn test_quick_prompt() {
        let spec = make_test_spec();
        let prompt = build_quick_prompt(&spec);
        assert!(prompt.contains("sort"));
        assert!(prompt.contains("is_sorted"));
    }
}
