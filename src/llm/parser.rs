// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC LLM RESPONSE PARSER                              │
// │  Extracts code candidates from raw LLM output             │
// └──────────────────────────────────────────────────────────┘

use crate::spec::ast::FunctionSpec;
use super::client::CodeCandidate;

/// Parse raw LLM text output into structured code candidates.
///
/// Handles multiple formats:
/// - Markdown code fences (```rust ... ```)
/// - Code block tags (<code>...</code>)
/// - Raw Rust function bodies
pub fn parse_llm_response(
    raw: &str,
    _spec: &FunctionSpec,
) -> Result<Vec<CodeCandidate>, String> {
    let mut candidates = Vec::new();

    // Strategy 1: Markdown code fences with language tags
    let fence_candidates = extract_fenced_blocks(raw, "```");
    candidates.extend(fence_candidates);

    // Strategy 2: XML-style code blocks
    let xml_candidates = extract_xml_blocks(raw);
    candidates.extend(xml_candidates);

    // Strategy 3: If no structured blocks found, try the whole response
    if candidates.is_empty() {
        let trimmed = raw.trim();
        if !trimmed.is_empty() && looks_like_rust(trimmed) {
            candidates.push(CodeCandidate {
                source: trimmed.to_string(),
                language: "rust".into(),
                confidence: 0.5,
                explanation: Some("Extracted from raw LLM output (no code block markers)".into()),
            });
        }
    }

    // Assign confidence based on structure quality
    for candidate in &mut candidates {
        candidate.confidence = assess_confidence(&candidate.source);
    }

    if candidates.is_empty() {
        Err("No parseable code found in LLM response".into())
    } else {
        Ok(candidates)
    }
}

/// Extract code blocks delimited by triple backticks
fn extract_fenced_blocks(text: &str, fence: &str) -> Vec<CodeCandidate> {
    let mut candidates = Vec::new();
    let mut in_block = false;
    let mut current_lang = String::new();
    let mut current_code = String::new();

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with(fence) {
            if in_block {
                // Closing fence
                if !current_code.is_empty() {
                    candidates.push(CodeCandidate {
                        source: current_code.clone(),
                        language: if current_lang.is_empty() { "unknown".into() } else { current_lang.clone() },
                        confidence: 0.8,
                        explanation: Some(format!("Extracted from {} fence block", fence)),
                    });
                }
                current_code.clear();
                current_lang.clear();
                in_block = false;
            } else {
                // Opening fence — may have language tag
                let lang = trimmed[fence.len()..].trim().to_string();
                current_lang = lang;
                current_code.clear();
                in_block = true;
            }
        } else if in_block {
            if !current_code.is_empty() {
                current_code.push('\n');
            }
            current_code.push_str(line);
        }
    }

    // Handle unclosed fence
    if in_block && !current_code.is_empty() {
        candidates.push(CodeCandidate {
            source: current_code,
            language: current_lang,
            confidence: 0.5,
            explanation: Some("Extracted from unclosed fence block".into()),
        });
    }

    candidates
}

/// Extract code from <code>...</code> XML-style blocks
fn extract_xml_blocks(text: &str) -> Vec<CodeCandidate> {
    let mut candidates = Vec::new();
    let mut start = 0;

    while let Some(tag_start) = text[start..].find("<code>") {
        let abs_start = start + tag_start + 6;
        if let Some(tag_end) = text[abs_start..].find("</code>") {
            let code = text[abs_start..abs_start + tag_end].trim().to_string();
            if !code.is_empty() {
                candidates.push(CodeCandidate {
                    source: code,
                    language: "rust".into(),
                    confidence: 0.9,
                    explanation: Some("Extracted from <code> block".into()),
                });
            }
            start = abs_start + tag_end + 7;
        } else {
            break;
        }
    }

    candidates
}

/// Quick heuristic: does this look like Rust code?
fn looks_like_rust(text: &str) -> bool {
    let indicators = [
        "fn ", "pub fn ", "impl ", "struct ", "enum ",
        "let ", "let mut ", "use ", "mod ", "Vec<",
        "->", "=>", "println!", "panic!",
    ];

    let score = indicators.iter()
        .filter(|&&ind| text.contains(ind))
        .count();

    score >= 2
}

/// Assess how likely this code is correct based on structure
fn assess_confidence(code: &str) -> f32 {
    let mut score = 0.5f32;

    // Has function signature: good sign
    if code.contains("fn ") || code.contains("pub fn ") {
        score += 0.1;
    }

    // Has return statement or expression
    if code.contains("return ") || code.contains("->") {
        score += 0.05;
    }

    // Balanced braces (rough check)
    let open = code.matches('{').count();
    let close = code.matches('}').count();
    if open > 0 && open == close {
        score += 0.1;
    }

    // Has indentation (structured)
    if code.contains("\n    ") || code.contains("\n\t") {
        score += 0.05;
    }

    // Not too short (probably incomplete)
    if code.len() < 20 {
        score -= 0.3;
    }

    // Not too long (probably noise)
    if code.len() > 5000 {
        score -= 0.2;
    }

    score.clamp(0.0, 1.0)
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rust_fence_block() {
        let input = r#"
Here is the implementation:

```rust
fn add(a: i64, b: i64) -> i64 {
    a + b
}
```

Done.
"#;
        let candidates = parse_llm_response(input, &dummy_spec()).unwrap();
        assert!(!candidates.is_empty());
        assert!(candidates[0].source.contains("fn add"));
    }

    #[test]
    fn test_parse_multiple_blocks() {
        let input = r#"
```rust
fn sort_v1(list: Vec<i64>) -> Vec<i64> {
    // quicksort
    todo!()
}
```

```rust
fn sort_v2(list: Vec<i64>) -> Vec<i64> {
    // mergesort
    todo!()
}
```
"#;
        let candidates = parse_llm_response(input, &dummy_spec()).unwrap();
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn test_parse_no_code() {
        let input = "Just some text without code blocks.";
        assert!(parse_llm_response(input, &dummy_spec()).is_err());
    }

    fn dummy_spec() -> FunctionSpec {
        FunctionSpec {
            name: "test".into(),
            doc: None,
            generics: vec![],
            params: vec![],
            return_type: crate::spec::ast::TypeRef::Unit,
            preconditions: vec![],
            postconditions: vec![],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: std::collections::HashMap::new(),
        }
    }
}
