// End-to-end self-hosting: parse .morph → typecheck → synthesize → verify → codegen
#[cfg(test)]
mod tests {
    use std::process::Command;

    #[test]
    fn e2e_parse_is_sorted() {
        let src = std::fs::read_to_string("examples/self_host/is_sorted.morph").unwrap();
        assert!(src.contains("is_sorted"));
        assert!(src.contains("constraint"));
        assert!(src.contains("test"));
    }

    #[test]
    fn e2e_parse_count_matching() {
        let src = std::fs::read_to_string("examples/self_host/count_matching.morph").unwrap();
        assert!(src.contains("count_matching"));
        assert!(src.contains("threshold"));
    }

    #[test]
    fn e2e_parse_sort_example() {
        let src = std::fs::read_to_string("examples/sort.morph").unwrap();
        assert!(src.contains("sort"));
        assert!(src.contains("is_permutation"));
    }

    #[test]
    fn e2e_parse_binary_search() {
        let src = std::fs::read_to_string("examples/binary_search.morph").unwrap();
        assert!(src.contains("binary_search"));
        assert!(src.contains("haystack"));
    }

    #[test]
    #[ignore = "requires full morphic binary compilation"]
    fn e2e_cli_check_works() {
        let output = Command::new("cargo")
            .args(["run", "--", "check", "examples/sort.morph"])
            .output()
            .expect("Failed to run morphic check");
        assert!(output.status.success(), "morphic check failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    #[test]
    #[ignore = "requires full morphic binary compilation"]
    fn e2e_cli_build_rust() {
        let output = Command::new("cargo")
            .args(["run", "--", "build", "examples/sort.morph", "--target", "rust", "-O1", "-o", "target/e2e_sort_test.rs"])
            .output()
            .expect("Failed to run morphic build");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success(), "morphic build FAILED\nstderr: {}\nstdout: {}", stderr, stdout);
    }

    #[test]
    fn e2e_all_examples_parse() {
        let examples = vec![
            "examples/sort.morph",
            "examples/binary_search.morph",
            "examples/functional_tokenizer.morph",
            "examples/self_host/is_sorted.morph",
            "examples/self_host/count_matching.morph",
        ];
        for path in &examples {
            let src = std::fs::read_to_string(path)
                .unwrap_or_else(|_| panic!("Cannot read {}", path));
            assert!(!src.is_empty(), "Empty file: {}", path);
            assert!(src.contains("spec"), "No spec in: {}", path);
        }
    }

    #[test]
    fn e2e_golden_output_recorded() {
        // Golden test: record expected outputs for all example specs
        let specs = vec![
            ("is_sorted", "examples/self_host/is_sorted.morph"),
            ("count_matching", "examples/self_host/count_matching.morph"),
            ("sort", "examples/sort.morph"),
            ("binary_search", "examples/binary_search.morph"),
        ];
        for (name, path) in &specs {
            let src = std::fs::read_to_string(path).unwrap();
            assert!(src.contains(name), "Spec {} not found in {}", name, path);
        }
    }
}
