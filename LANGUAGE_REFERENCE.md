# 📘 Morphic Language Reference

> Complete specification of the `.morph` language — v0.2.0

---

## Table of Contents

1. [Lexical Structure](#1-lexical-structure)
2. [Module](#2-module)
3. [Specifications](#3-specifications)
4. [Types](#4-types)
5. [Parameters](#5-parameters)
6. [Constraints](#6-constraints)
7. [Complexity Bounds](#7-complexity-bounds)
8. [Resource Bounds](#8-resource-bounds)
9. [Invariants](#9-invariants)
10. [Tests](#10-tests)
11. [Expressions](#11-expressions)
12. [Annotations](#12-annotations)
13. [Generics](#13-generics)
14. [Type Definitions](#14-type-definitions)
15. [Imports](#15-imports)

---

## 1. Lexical Structure

### Comments

```morphic
// Line comment — everything after // is ignored
/// Doc comment — attaches to the next element

/*
   Block comment
   spans multiple lines
*/
```

### Identifiers

```
Identifier ::= [a-zA-Z_][a-zA-Z0-9_]*
```

Valid: `sort`, `my_function`, `findMax`, `_private`, `T`, `Type1`

### Keywords

```
spec, input, output, constraint, require, ensure,
invariant, optimize, resource, type, import,
test, forall, exists, true, false, O
```

### Literals

| Kind | Examples |
|---|---|
| Integer | `42`, `-7`, `0`, `1000000` |
| Float | `3.14`, `-0.5`, `1.0e10` |
| Boolean | `true`, `false` |
| String | `"hello"`, `"with \"escape\""` |
| Character | `'a'`, `'\n'` |
| Unit | `()` |

---

## 2. Module

A `.morph` file is a **module** — a named container for specs, types, and invariants.

```ebnf
Module ::= Identifier '{' (Import | Spec | TypeDef | Invariant)* '}'
```

```morphic
Calculator {
    import "math.morph"

    type Point = { x: Int, y: Int }

    spec add {
        input: a: Int, b: Int
        output: Int
        constraint: output == a + b
        test { input: (2, 3); expect: 5 }
    }
}
```

---

## 3. Specifications

A `spec` declares a function's contract. The compiler synthesizes the body.

### Full Syntax

```ebnf
Spec ::= 'spec' Identifier GenericParams? '{'
    DocComment?
    Params
    Returns
    Preconditions?
    Postconditions?
    Constraints?
    Invariants?
    ComplexityBounds?
    ResourceBounds?
    Tests?
    Annotations?
'}'
```

### Minimal Spec

```morphic
spec identity {
    input: x: Int
    output: Int
    constraint: output == x
    test { input: 42; expect: 42 }
}
```

### Complete Spec

```morphic
spec binary_search<T> {
    /// Find `needle` in sorted `haystack`. Returns index or None.

    input: haystack: List<T>, needle: T, cmp: fn(T, T) -> Int
    output: Option<Int>

    require: is_sorted(haystack, cmp)
    require: len(haystack) >= 0

    ensure: output == None() || haystack[output.unwrap()] == needle

    constraint: forall i: Int in range(len(output)):
        output[i] >= 0 && output[i] < len(haystack)

    invariant: len(haystack) >= 0

    optimize: time < O(log_n)
    optimize: space < O(1)

    resource: memory < 1MB

    test "found" { input: ([1,3,5,7], 5); expect: Some(2) }
    test "not found" { input: ([1,3,5,7], 2); expect: None() }

    @priority = "high"
    @author = "thvtzy"
}
```

---

## 4. Types

### Primitive Types

| Type | Description | Example Value |
|---|---|---|
| `Int` | Arbitrary-precision integer | `42`, `-7` |
| `Float` | Double-precision float | `3.14` |
| `Bool` | Boolean | `true`, `false` |
| `String` | UTF-8 string | `"hello"` |
| `Char` | Unicode character | `'a'` |
| `Unit` | Void / nothing | `()` |

### Fixed-Width Integers

| Type | Range |
|---|---|
| `I8` | -128 .. 127 |
| `I16` | -32,768 .. 32,767 |
| `I32` | -2³¹ .. 2³¹-1 |
| `I64` | -2⁶³ .. 2⁶³-1 |
| `U8` | 0 .. 255 |
| `U16` | 0 .. 65,535 |
| `U32` | 0 .. 2³²-1 |
| `U64` | 0 .. 2⁶⁴-1 |

### Container Types

| Type | Syntax | Example |
|---|---|---|
| List | `List<T>` | `List<Int>` |
| Set | `Set<T>` | `Set<String>` |
| Map | `Map<K, V>` | `Map<String, Int>` |
| Option | `Option<T>` | `Option<Int>` = `Some(5)` or `None()` |
| Result | `Result<T, E>` | `Result<Int, String>` |
| Tuple | `(T1, T2, ...)` | `(Int, String, Bool)` |
| Array | `[T; N]` | `[I32; 256]` |
| Reference | `&T` | `&Int` |
| Function | `fn(T1, T2) -> R` | `fn(Int, Int) -> Bool` |
| Stream | `Stream<T>` | `Stream<Int>` |

---

## 5. Parameters

```ebnf
Params ::= 'input' ':' Param (',' Param)*
Param  ::= Identifier ':' Type
```

```morphic
spec example {
    input: name: String, age: Int, scores: List<Float>
    output: Result<Float, String>
    // ...
}
```

Parameters are immutable. Use `input` or `params` keyword interchangeably.

---

## 6. Constraints

Constraints describe WHAT must be true, not HOW to achieve it.

### Preconditions (`require`)

Must be true **before** the function executes. Violations are caller errors.

```morphic
spec divide {
    input: a: Int, b: Int
    output: Int
    require: b != 0              // Precondition
    constraint: output * b == a
}
```

### Postconditions (`ensure`)

Must be true **after** the function executes. Violations are implementation errors.

```morphic
spec abs_value {
    input: x: Int
    output: Int
    ensure: output >= 0           // Postcondition
    ensure: output == x || output == -x
}
```

### Constraints (`constraint`)

Logical formulas the output must satisfy. Equivalent to postconditions in syntax.

```morphic
spec sort {
    input: list: List<Int>
    output: List<Int>
    constraint: is_sorted(output)
    constraint: len(output) == len(list)
}
```

### Constraint Forms

| Form | Syntax | Example |
|---|---|---|
| Equality | `a == b` | `output == input` |
| Inequality | `a < b`, `a <= b`, `a > b`, `a >= b` | `output >= 0` |
| Negation | `!constraint` | `!(output < 0)` |
| Conjunction | `a && b` | `x > 0 && y > 0` |
| Disjunction | `a || b` | `x == 0 || x == 1` |
| Implication | `a implies b` | `x > 0 implies output > 0` |
| Universal | `forall x: T in collection: body` | `forall x in list: x > 0` |
| Existential | `exists x: T in collection: body` | `exists x in list: x == 5` |
| Predicate | `predicate(args)` | `is_sorted(output)` |
| Always true | `true` | `constraint: true` |

---

## 7. Complexity Bounds

Specify the computational complexity the implementation must achieve.

```ebnf
Complexity ::= 'optimize' ':' Dimension ('<' | '<=') 'O' '(' Bound ')'
Dimension ::= 'time' | 'space' | 'amortized' | 'communication'
```

| Bound | Syntax | Meaning |
|---|---|---|
| Constant | `O(1)` | Does not grow with input |
| Logarithmic | `O(log_n)` | Binary search class |
| Linear | `O(n)` | One pass through input |
| Linearithmic | `O(n_log_n)` | Sorting class |
| Quadratic | `O(n2)` | Nested loops |
| Cubic | `O(n3)` | Triple-nested |
| Polynomial | `O(n^k)` | Arbitrary k |
| Exponential | `O(k^n)` | Bruteforce |
| Custom | `O(custom_name)` | User-defined |

```morphic
optimize: time < O(n_log_n)     // Faster than n²
optimize: space < O(n)          // At most linear memory
```

---

## 8. Resource Bounds

Limit runtime resource consumption.

```ebnf
Resource ::= 'resource' ':' ResourceKind ('<' Amount)?
ResourceKind ::= 'memory' | 'allocations' | 'syscalls' | 'net_io' | 'disk_io'
Amount ::= Integer ('KB' | 'MB' | 'GB')?
```

```morphic
resource: memory < 10MB
resource: allocations < O(n)
resource: syscalls < 1000
```

---

## 9. Invariants

Properties that must always hold — before, during, and after execution.

```ebnf
Invariant ::= 'invariant' ':' Constraint
```

```morphic
spec push {
    input: stack: Stack<T>, item: T
    output: Stack<T>

    invariant: len(output) == len(stack) + 1  // Always +1
    invariant: top(output) == item             // Item on top
}
```

Invariants are especially useful for mutable data structures and loop synthesis.

---

## 10. Tests

Test cases provide example I/O pairs. They serve double duty:
1. **Verification** — the synthesized implementation must pass all tests
2. **Synthesis seeding** — I/O pairs bootstrap the synthesis engine

```ebnf
Test ::= 'test' (StringLiteral | Identifier)? '{'
    'input' ':' Expr
    ('expect' | 'expected' | 'output') ':' Expr
    ('timeout' ':' Integer)?    // timeout in ms
    ('property' ':' Bool)?      // property-based test?
'}'
```

### Concrete Tests

```morphic
test "empty list" {
    input: []
    expect: []
}

test {
    input: [3, 1, 2]
    expect: [1, 2, 3]
}
```

### Best Practices

- **Minimum 3 tests** per spec
- Cover: empty input, single element, normal case, edge case
- Test names are optional but recommended
- Use `property: true` for property-based tests (future)

---

## 11. Expressions

Expressions appear in constraints, tests, and type annotations.

### Atomic Expressions

| Kind | Example |
|---|---|
| Variable | `x`, `output`, `input_list` |
| Integer literal | `42`, `-7` |
| Float literal | `3.14` |
| Boolean literal | `true`, `false` |
| String literal | `"hello"` |
| Function call | `len(list)`, `is_sorted(arr)` |

### Binary Operations

| Operator | Precedence | Meaning |
|---|---|---|
| `*` `/` `%` | 7 | Multiplication, division, modulo |
| `+` `-` | 6 | Addition, subtraction |
| `<` `<=` `>` `>=` | 4 | Comparison |
| `==` `!=` | 4 | Equality |
| `&&` | 3 | Logical AND |
| `\|\|` | 2 | Logical OR |

### Unary Operations

| Operator | Meaning |
|---|---|
| `-` | Arithmetic negation |
| `!` | Logical negation |

### Field Access & Indexing

```morphic
point.x               // Field access
list[i]               // Index access
matrix[i][j]          // Nested index
```

### Lambda Expressions

```morphic
|x, y| x + y                    // Lambda: add two values
|a, b| -> a > b                 // Lambda with arrow
fn(x: Int, y: Int) -> x + y     // Explicit types
```

### Comprehensions (Future)

```morphic
[x * 2 for x in numbers if x > 0]   // List comprehension
```

---

## 12. Annotations

Metadata attached to specs using `@` syntax.

```morphic
spec critical_function {
    @priority = "high"
    @author = "thvtzy"
    @version = "1.0"
    @deprecated = "use better_sort instead"

    input: list: List<Int>
    output: List<Int>
    constraint: is_sorted(output)
    test { input: [3,1,2]; expect: [1,2,3] }
}
```

Annotations are key-value pairs stored as strings. They do not affect synthesis.

---

## 13. Generics

Specs and types can be parameterized over types.

```ebnf
GenericParams ::= '<' Identifier (',' Identifier)* '>'
```

```morphic
spec sort<T> {
    input: list: List<T>, cmp: fn(T, T) -> Bool
    output: List<T>
    constraint: is_sorted(output, cmp)
    test { input: ([3,1,2], |a,b| a < b); expect: [1,2,3] }
}

spec find<T> {
    input: haystack: List<T>, needle: T
    output: Option<Int>
    constraint: /* needle found at output index, or None */
    test { input: ([1,2,3], 2); expect: Some(1) }
}
```

Type parameters can have bounds (future):
```morphic
spec sort<T: Ord> { ... }          // T must be orderable
spec clone<T: Clone> { ... }       // T must be cloneable
```

---

## 14. Type Definitions

Define custom types using the `type` keyword.

### Record Types (Structs)

```morphic
type Point = {
    x: Float
    y: Float
}

type Person = {
    name: String
    age: Int
    email: Option<String>
}
```

### Type Aliases

```morphic
type UserId = Int
type Scores = List<Float>
type Callback = fn(Int, Int) -> Bool
```

### ADT / Enums (Future)

```morphic
type Option<T> = Some { value: T } | None {}

type Result<T, E> = Ok { value: T } | Err { error: E }
```

---

## 15. Imports

Import other `.morph` files or standard library modules.

```ebnf
Import ::= 'import' StringLiteral ('as' Identifier)?
```

```morphic
MainModule {
    import "std/collections.morph"
    import "std/algorithms.morph" as algo
    import "../lib/my_math.morph"

    spec my_sort<T> {
        input: list: List<T>
        output: List<T>
        constraint: algo::is_sorted(output)
        test { input: [3,1,2]; expect: [1,2,3] }
    }
}
```

---

## Appendix A: Standard Library (Planned)

Functions assumed to be available in constraints:

| Predicate | Signature | Meaning |
|---|---|---|
| `is_sorted` | `List<T>, fn(T,T)->Bool → Bool` | Elements are in order |
| `is_permutation` | `List<T>, List<T> → Bool` | Same elements, possibly rearranged |
| `len` | `List<T> → Int` | Number of elements |
| `contains` | `List<T>, T → Bool` | Element exists in list |
| `reverse` | `List<T> → List<T>` | Reversed list |
| `sort` | `List<T> → List<T>` | Sorted copy |
| `filter` | `List<T>, fn(T)→Bool → List<T>` | Keep matching elements |
| `map` | `List<T>, fn(T)→U → List<U>` | Transform each element |
| `fold` | `List<T>, U, fn(U,T)→U → U` | Accumulate |
| `sum` | `List<Int> → Int` | Sum of elements |
| `product` | `List<Int> → Int` | Product of elements |
| `min` | `List<T> → T` | Minimum element |
| `max` | `List<T> → T` | Maximum element |
| `all` | `List<T>, fn(T)→Bool → Bool` | All satisfy predicate |
| `any` | `List<T>, fn(T)→Bool → Bool` | Any satisfies predicate |
| `range` | `Int → List<Int>` | `[0, 1, ..., n-1]` |
| `zip` | `List<T>, List<U> → List<(T,U)>` | Pair elements |
| `take` | `List<T>, Int → List<T>` | First n elements |
| `drop` | `List<T>, Int → List<T>` | Skip first n elements |

---

## Appendix B: Error Codes

| Error | Meaning |
|---|---|
| `E001` | Parse error — check syntax |
| `E002` | Type mismatch — check parameter types |
| `E003` | Undefined variable — variable not in scope |
| `E004` | Missing constraint — every spec needs at least one |
| `E005` | Missing test — every spec needs at least one test |
| `E006` | Complexity violation — implementation doesn't meet bound |
| `E007` | Verification failure — Z3 found a counterexample |

---

*Version 0.2.0 — Last updated 2026-06-09*
