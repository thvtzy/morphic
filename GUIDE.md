# 🚀 Morphic Beginner's Guide

> *You've never used Morphic before. In 10 minutes, you'll watch a compiler invent algorithms for you.*

---

## What is Morphic?

Every programming language you've used asks you to write **HOW** to do something.

Morphic asks **WHAT** you want.

```morphic
// Traditional programming: YOU write the algorithm
fn sort(list) {
    // quicksort? mergesort? you decide. you implement.
}

// Morphic: YOU write what "sorted" MEANS
spec sort {
    input: list: List<Int>
    output: List<Int>
    constraint: is_sorted(output)
    constraint: is_permutation(list, output)
}
```

The compiler finds the best algorithm. Proves it correct. Generates the code.

---

## Installation

### Prerequisites
- **Rust 1.75+** — [Install Rust](https://rustup.rs)
- **Git** — [Download Git](https://git-scm.com)

```bash
# Clone Morphic
git clone https://github.com/thvtzy/morphic.git
cd morphic

# Build (includes Z3 verifier automatically)
cargo build --release

# Verify installation
./target/release/morphic --version
```

### Optional: LLM Integration

```bash
# For AI-powered synthesis (Ollama)
curl -fsSL https://ollama.com/install.sh | sh
ollama pull codellama:13b

# Enable in Morphic
cargo build --release --features llm-remote
```

---

## Your First Morphic Program

### Step 1: Create a project

```bash
morphic init hello
cd hello
```

This creates:
```
hello/
├── Morphic.toml    # Project config
└── main.morph      # Your spec file
```

### Step 2: Write a specification

Open `main.morph` and write:

```morphic
Calculator {
spec add {
    input: a: Int, b: Int
    output: Int

    // What must be true about the output?
    constraint: output == a + b

    // Test cases (required for synthesis)
    test "two plus three" {
        input: (2, 3)
        expect: 5
    }

    test "zero plus zero" {
        input: (0, 0)
        expect: 0
    }

    test "negative numbers" {
        input: (-5, 3)
        expect: -2
    }
}
}
```

### Step 3: Build (synthesize the implementation)

```bash
morphic build main.morph --target rust
```

Output:
```rust
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_plus_three() {
        assert_eq!(add(2, 3), 5);
    }
    // ... more tests
}
```

### Step 4: Check the spec (without codegen)

```bash
morphic check main.morph
```

This runs Z3 verification on your spec without generating code.

---

## The Morphic Language

### Basic Structure

A `.morph` file contains a **module** with one or more **specs**:

```morphic
ModuleName {
    spec function_name {
        // inputs
        input: param1: Type, param2: Type

        // output type
        output: ReturnType

        // what must be true?
        constraint: some_condition

        // how fast should it be? (optional)
        optimize: time < O(n_log_n)

        // example inputs/outputs
        test { input: something; expect: something_else }
    }
}
```

### Types

| Type | Meaning | Example |
|---|---|---|
| `Int` | Integer | `42`, `-7` |
| `Float` | Floating-point | `3.14` |
| `Bool` | Boolean | `true`, `false` |
| `String` | Text | `"hello"` |
| `List<T>` | List of T | `List<Int>` = `[1, 2, 3]` |
| `Map<K, V>` | Key-value map | `Map<String, Int>` |
| `Option<T>` | Maybe T | `Some(5)` or `None()` |
| `Result<T, E>` | Ok or error | `Result<Int, String>` |

### Constraints

Constraints tell the compiler what "correct" means:

| Kind | Syntax | Meaning |
|---|---|---|
| **Equality** | `constraint: output == value` | Output must equal value |
| **Comparison** | `constraint: output < 100` | Output must be less than 100 |
| **Logical AND** | `constraint: a > 0 && b > 0` | Both must be true |
| **Precondition** | `require: input > 0` | Must be true BEFORE function runs |
| **Postcondition** | `ensure: output > input` | Must be true AFTER function runs |
| **Invariant** | `invariant: len(stack) >= 0` | Must ALWAYS be true |
| **True** | `constraint: true` | Placeholder (always holds) |

### Complexity Bounds

Tell the compiler how efficient your code must be:

| Bound | Meaning |
|---|---|
| `O(1)` | Constant time |
| `O(log n)` | Logarithmic |
| `O(n)` | Linear |
| `O(n_log_n)` | Linearithmic |
| `O(n²)` | Quadratic |

```morphic
spec search {
    input: haystack: List<Int>, needle: Int
    output: Option<Int>

    constraint: /* found if present, None if not */

    optimize: time < O(log n)   // must be binary search or better
    optimize: space < O(1)      // constant extra memory
}
```

### Test Cases

Tests serve double duty: they verify correctness **AND** seed the synthesis engine.

```morphic
test "description" {
    input: (arg1, arg2)
    expect: expected_output
}
```

**Rule of thumb:** provide 3-5 test cases covering:
1. Normal input (happy path)
2. Edge cases (empty, zero, max, min)
3. Failure cases (if applicable)

---

## The Synthesis Process

When you run `morphic build`, here's what happens:

```
Your .morph spec
    │
    ▼
┌─ PARSER ──────────┐  Reads your spec
└───────┬───────────┘
        │
        ▼
┌─ TYPE CHECKER ────┐  Validates types & constraints
└───────┬───────────┘
        │
        ▼
┌─ SYNTHESIS ───────┐  Generates candidate implementations
│  • Test seeding    │  Uses your test cases
│  • Templates       │  Known patterns (divide-conquer, loops)
│  • Search (MCTS)   │  Explores implementation space
└───────┬───────────┘
        │
        ▼
┌─ Z3 VERIFY  ──────┐  Proves each candidate correct
│                    │  (or finds counterexamples)
└───────┬───────────┘
        │
        ▼
┌─ CODEGEN  ────────┐  Generates Rust, C, WASM, Python, or JS
└────────────────────┘
```

### Optimization Levels

```bash
morphic build -O1 spec.morph   # Correctness only (fast)
morphic build -O2 spec.morph   # Balanced (default)
morphic build -O3 spec.morph   # Aggressive optimization
morphic build -O4 spec.morph   # Maximum (exhaustive search)
```

Higher levels = more candidates explored = better implementation, but slower.

---

## Common Patterns

### Pattern 1: Pure Computation

```morphic
spec factorial {
    input: n: Int
    output: Int

    require: n >= 0
    constraint: output > 0
    constraint: if n == 0 then output == 1
    optimize: time < O(n)

    test { input: 0; expect: 1 }
    test { input: 5; expect: 120 }
}
```

### Pattern 2: Collection Transformation

```morphic
spec filter_positive {
    input: numbers: List<Int>
    output: List<Int>

    constraint: forall x: Int in output: x > 0
    constraint: len(output) <= len(numbers)

    optimize: time < O(n)

    test { input: [1, -2, 3, -4]; expect: [1, 3] }
    test { input: [-1, -2]; expect: [] }
}
```

### Pattern 3: Search Problem

```morphic
spec find_max {
    input: numbers: List<Int>
    output: Option<Int>

    require: len(numbers) > 0
    constraint: forall x: Int in numbers: x <= output.unwrap()
    constraint: output.unwrap() in numbers || output == None()

    optimize: time < O(n)
    optimize: space < O(1)

    test { input: [1, 5, 3, 9, 2]; expect: Some(9) }
    test { input: [-5, -2, -10]; expect: Some(-2) }
}
```

---

## Target Languages

Morphic can generate code for multiple targets:

```bash
morphic build spec.morph --target rust        # Rust (default)
morphic build spec.morph --target c           # C
morphic build spec.morph --target wasm        # WebAssembly
morphic build spec.morph --target python      # Python
morphic build spec.morph --target javascript  # JavaScript
```

---

## Troubleshooting

### "No implementation satisfied all constraints"

The synthesis engine couldn't find a valid implementation. Try:

1. **Add more test cases** — more examples = better synthesis seeds
2. **Relax complexity bounds** — `O(n²)` is easier than `O(n log n)`
3. **Increase optimization level** — `morphic build -O4 spec.morph`
4. **Simplify constraints** — try `constraint: true` and add back one at a time

### "Z3 solver not found"

```bash
# Z3 is bundled automatically. If issues occur:
cargo build --release --features z3-support
```

### "Parse error at line..."

Check your syntax:
- Module wrapper: `ModuleName { spec name { ... } }`
- Semicolons? Nope — Morphic doesn't use semicolons
- All specs must be inside a `ModuleName { }` block

---

## Next Steps

1. **Read [LANGUAGE_REFERENCE.md](LANGUAGE_REFERENCE.md)** — full language spec
2. **Read [ARCHITECTURE.md](ARCHITECTURE.md)** — understand the compiler
3. **Write more specs** — check `examples/` for inspiration
4. **Contributing** — see [CONTRIBUTING.md](CONTRIBUTING.md)

---

## Quick Reference Card

```
morphic init <name>          Create new project
morphic build <file>         Compile spec → code
morphic check <file>         Verify spec (no codegen)
morphic synthesize <file>    Interactive mode

Options:
  --target rust|c|wasm|python|javascript
  -O1|-O2|-O3|-O4            Optimization level

File structure:
  ModuleName {
    spec name {
      input: name: Type, ...
      output: Type
      require: precondition     (optional)
      constraint: condition     (required)
      ensure: postcondition     (optional)
      invariant: always_true    (optional)
      optimize: time < O(...)   (optional)
      optimize: space < O(...)  (optional)
      test { input: ...; expect: ... }  (recommended)
    }
  }
```

---

*Built with Rust, Z3, and the conviction that compilers should do more work than programmers.*
