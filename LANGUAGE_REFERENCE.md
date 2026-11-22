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

A character literal is exactly one Unicode scalar value between
single quotes: `'a'`, `'é'`, `'😀'`. The escape forms are `'\n'`,
`'\r'`, `'\t'`, `'\\'`, `'\''`, and `'\u{...}'` (1–6 hex digits
naming a Unicode scalar value, e.g. `'\u{1F600}'`). Empty (`''`),
multi-character (`'ab'`), and unterminated char literals are parse
errors.

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
    constraint: list_is_sorted(output)
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
| Predicate | `predicate(args)` | `list_is_sorted(output)` |
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
| Function call | `len(list)`, `list_is_sorted(arr)` |
| Method call | `output.is_err()`, `list.len()` |
| Record literal | `Point { x: 1.0, y: 2.0 }` |

### Binary Operations

| Operator | Precedence | Meaning |
|---|---|---|
| `*` `/` `%` | 7 | Multiplication, division, modulo |
| `+` `-` | 6 | Addition, subtraction |
| `++` | 6 | Concatenation (strings, lists) |
| `<` `<=` `>` `>=` | 4 | Comparison |
| `==` `!=` | 4 | Equality |
| `&&` | 3 | Logical AND |
| `\|\|` | 2 | Logical OR |

`++` concatenates two strings or two lists and shares the additive level: it is left-associative and binds tighter than the comparisons, so `"Vec<" ++ inner ++ ">"` groups as `("Vec<" ++ inner) ++ ">"`. Its operands must both be `String` or both be `List` with compatible element types — anything else (numbers, mixed operands, mismatched lists) is a type error. Unlike `+`, it is never arithmetic.

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

Field access, indexing, and method calls are *postfix* forms:
they chain after any primary expression — a variable, a function
call result, a parenthesized expression, a literal — and compose
with each other, left to right:

```morphic
output.unwrap().tokens      // method call, then field access
(state.tokens)[0].kind      // index after parentheses, then field
make(state).pos             // field access on a call result
```

### Method Calls

A method call is a postfix form: `receiver.method(args)`. The
set of methods is a **closed builtin table**, keyed by the
receiver's type:

| Receiver | Method | Result |
|---|---|---|
| `Result<T, E>` | `is_ok()` | `Bool` — true when the value is `Ok` |
| `Result<T, E>` | `is_err()` | `Bool` — true when the value is `Err` |
| `Result<T, E>` | `unwrap()` | `T` — the `Ok` payload |
| `Option<T>` | `is_some()` | `Bool` — true when the value is `Some` |
| `Option<T>` | `is_none()` | `Bool` — true when the value is `None` |
| `Option<T>` | `unwrap()` | `T` — the `Some` payload |
| `List<T>` | `len()` | `Int` — the method spelling of `len(list)` |

```morphic
constraint: output.is_err() || len(output.unwrap().tokens) == len(state.tokens) + 1
constraint: output.is_ok() || state.pos >= len(state.source)
constraint: output.is_err() || output.unwrap().len() >= len(state.tokens)
```

Calling any other method is an error naming the method. There
are no user-defined methods; generalizing method resolution to
module specs whose first parameter matches the receiver (UFCS)
is a recorded future extension, not part of the language today.

### Record Literals

A record value is constructed by naming its declared record
type (section 14) and giving every field a value:

```morphic
Point { x: 1.0, y: 2.0 }
Token { kind: 0, lexeme: "spec", line: 1, col: 1 }
TokenizerState { source: "hi", pos: 0, line: 1, col: 1, tokens: [] }
```

The type name is part of the syntax — the literal is
self-describing, and the bare `{ field: value, ... }` form
without a type name is **not** part of the language. The field
set must match the declaration exactly (a missing, extra, or
duplicated field is an error naming the field), and each field
value is checked against its declared type. A record literal
is a general expression form — it may appear in constraints,
in tests, and nested inside other literals and constructor
calls:

```morphic
expect: Ok(TokenizerState { source: "spec", pos: 4, line: 1, col: 5, tokens: [
    Token { kind: 0, lexeme: "spec", line: 1, col: 1 }
] })
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
    constraint: list_is_sorted(output)
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

Values of a record type are constructed with the named
record-literal form, `Person { name: ..., age: ..., email: ... }`
— see *Record Literals* in section 11.

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

```ebnf
Import ::= 'import' StringLiteral ('as' Identifier)?
```

Imports are parsed and accepted, but **not resolved**:
no file is loaded or merged, and qualified calls
(`algo::is_sorted`) are not part of the language. A
user-facing import mechanism is a language feature with
its own gate, deliberately left out of the 1.0 gate
(`STDLIB_SCOPE.md` §3).

The standard library needs no import: it ships with the
compiler and is merged into every module as a prelude
(Appendix A), so its entries are called by plain name.

---

## Appendix A: Standard Library

The standard library ships with the compiler and is merged
into every module as a **prelude** — its entries are called
by plain name, with no import (decision D2a; the scope and
gate are frozen in `STDLIB_SCOPE.md`). Entries are frozen at
the concrete instantiations named below (`List<Int>`,
`Option<Int>`, `Result<Int, String>`); polymorphic forms are
not part of 1.0.

Every entry stands at one of three standards, printed in its
row:

- **Builtin** — implemented as a compiler builtin; exact on
  the per-layer battery (interpreter / encoder fold /
  compiled runner agreement), extremes included.
- **Tier-1** — the entry's shipped body is **Z3-Verified ∧
  100% golden-exact** through the production pipeline.
- **Tier-2** — **executed-exact** on the full battery through
  the compiled runner, the tier printed here and in the
  entry's doc comment. Tier-2 exists only for the List
  element-level entries: the verifier binds a List parameter
  as its length only, so no element-level entry can Verify
  until the post-1.0 list-encoder campaign (§3.9 of the
  audit) lands, with this library as its test corpus.

### Builtins

| Entry | Signature | Tier | Semantics | Example |
|---|---|---|---|---|
| `len` | `(List<T> \| String) → Int` | Builtin | The length: elements of a list, characters of a string; empty → 0 | `len("abc") == 3` |
| `int_to_str` | `Int → String` | Builtin | The decimal text of the integer, exactly Rust's `i64` Display | `int_to_str(0 - 42) == "-42"` |

**`int_to_str` (F2, added Phase 100).** The Int→decimal-text
conversion: `int_to_str(n)` is the decimal rendering of `n`
exactly as Rust's `Display` for `i64` renders it — no leading
zeros, no `+`, a single leading `-` for negatives
(`int_to_str(0)` is `"0"`, `int_to_str(-42)` is `"-42"`). It
takes exactly one integer argument; any other arity or an
argument of a non-integer type is a type error. It is
available in constraints and in implementation bodies (it is
a plain call — `Expr::Call` / `IRNode::Call`; there is no
dedicated syntax). The name is reserved by the builtin: a
user spec named `int_to_str` does not shadow it in the
interpreter or the verifier, the standing treatment of `len`
and the constructors. Verification note: the Z3 encoder
evaluates the conversion exactly at literal arguments
(a literal fold) and treats a symbolic argument as an
uninterpreted function of the integer — proofs therefore
hold for the real conversion, while a refutation that
would rest on the uninterpreted value is reported as
inconclusive instead (the refutation-degradation rule,
F2_DESIGN.md §2.3).

### Int

| Entry | Signature | Tier | Semantics | Example |
|---|---|---|---|---|
| `int_abs` | `Int → Int` | Tier-1 | \|x\|; 0 → 0. The Int minimum is excluded by precondition (`require:`) — its absolute value is unrepresentable in Int | `int_abs(0 - 7) == 7` |
| `int_sign` | `Int → Int` | Tier-1 | −1 when x < 0, 0 when x = 0, 1 when x > 0; the Int minimum maps to −1 | `int_sign(0 - 3) == (0 - 1)` |
| `int_min` | `(Int, Int) → Int` | Tier-1 | The smaller argument; equal arguments → that value | `int_min(4, 9) == 4` |
| `int_max` | `(Int, Int) → Int` | Tier-1 | The larger argument; equal arguments → that value | `int_max(4, 9) == 9` |
| `int_clamp` | `(Int, Int, Int) → Int` | Tier-1 | x confined to [lo, hi]; `require: lo <= hi`; at lo = hi every x gives lo | `int_clamp(15, 0, 10) == 10` |
| `int_is_even` | `Int → Bool` | Tier-1 | True iff 2 divides x — sign-independent (−4 is even, −3 is not) | `int_is_even(0 - 4) == true` |
| `int_is_odd` | `Int → Bool` | Tier-1 | True iff 2 does not divide x; the exact negation of `int_is_even` | `int_is_odd(0 - 3) == true` |

**`int_gcd` — named in the frozen scope, not shipped.**
`int_gcd : (Int, Int) → Int` (the greatest non-negative
common divisor; gcd(0, 0) = 0; arguments equal to the Int
minimum excluded by precondition) is a frozen Tier-1 entry
of `STDLIB_SCOPE.md` §2.2 that does **not** ship in 1.0: it
is a named blocker, not a silent drop. Measured in Phase
111: no seed family builds Euclid's algorithm, and the
statement judge cannot crown a faithful hand body either —
the quantified divisibility-preservation invariant over `%`
returns Z3 Unknown. Closing it needs judge-side
quantified-mod work plus an Euclid seed family; whether it
leaves the 1.0 scope, returns after 1.0, or is re-tiered is
the owner's amendment decision, pending.

### Char (ASCII ranges, frozen as written)

| Entry | Signature | Tier | Semantics | Example |
|---|---|---|---|---|
| `char_is_digit` | `Char → Bool` | Tier-1 | True iff the char is in `'0'..='9'` | `char_is_digit('7') == true` |
| `char_is_alpha` | `Char → Bool` | Tier-1 | True iff the char is in `'A'..='Z'` or `'a'..='z'` | `char_is_alpha('q') == true` |
| `char_is_whitespace` | `Char → Bool` | Tier-1 | True iff the char is space, tab, LF or CR — exactly the tokenizer spec's `is_whitespace` set, adopted unchanged | `char_is_whitespace('\t') == true` |
| `char_is_uppercase` | `Char → Bool` | Tier-1 | True iff the char is in `'A'..='Z'`; non-letters → false | `char_is_uppercase('Q') == true` |
| `char_is_lowercase` | `Char → Bool` | Tier-1 | True iff the char is in `'a'..='z'`; non-letters → false | `char_is_lowercase('q') == true` |
| `char_to_digit` | `Char → Option<Int>` | Tier-1 | `'0'..='9'` → `Some(d)` with d the digit's value; every other char → `None` | `char_to_digit('7') == Some(7)` |

### String basics

The String surface is `len` (above), concatenation as the
`++` operator (F1), and one entry. The transform class
(reverse / case / replace / split / join) does not ship —
there is no per-character iteration in the expression
language, so such bodies cannot even be stated (F5, out per
decision D5a).

| Entry | Signature | Tier | Semantics | Example |
|---|---|---|---|---|
| `string_is_empty` | `String → Bool` | Tier-1 | True iff `len(s) == 0` | `string_is_empty("") == true` |

### Result / Option

Existing surface (not entries): the closed method table —
`Result::{is_ok, is_err, unwrap}`,
`Option::{is_some, is_none, unwrap}`, `List::len` —
available in constraints and bodies since the M1-features
campaign (Option's arm added Phase 112).

| Entry | Signature | Tier | Semantics | Example |
|---|---|---|---|---|
| `result_unwrap_or` | `(Result<Int, String>, Int) → Int` | Tier-1 | `Ok(v)` → v; `Err(_)` → the default argument | `result_unwrap_or(Err("x"), 7) == 7` |
| `option_unwrap_or` | `(Option<Int>, Int) → Int` | Tier-1 | `Some(v)` → v; `None` → the default argument | `option_unwrap_or(None, 7) == 7` |
| `option_ok_or` | `(Option<Int>, String) → Result<Int, String>` | Tier-1 | `Some(v)` → `Ok(v)`; `None` → `Err(e)` with e the argument, unchanged | `option_ok_or(None, "missing") == Err("missing")` |

### List

| Entry | Signature | Tier | Semantics | Example |
|---|---|---|---|---|
| `list_is_empty` | `List<Int> → Bool` | Tier-1 | True iff `len(xs) == 0` | `list_is_empty(List()) == true` |
| `list_reverse` | `List<Int> → List<Int>` | Tier-2 | The elements in reverse order; empty → empty; singleton → itself | `list_reverse(List(1, 2, 3)) == List(3, 2, 1)` |
| `list_contains` | `(List<Int>, Int) → Bool` | Tier-2 | True iff some element equals the target; empty → false | `list_contains(List(1, 2, 3), 2) == true` |
| `list_sum` | `List<Int> → Int` | Tier-2 | The sum of the elements; empty → 0 | `list_sum(List(1, 2, 3)) == 6` |
| `list_product` | `List<Int> → Int` | Tier-2 | The product of the elements; empty → 1; any zero element → 0 | `list_product(List(2, 3, 4)) == 24` |
| `list_min` | `List<Int> → Option<Int>` | Tier-2 | Empty → `None`; otherwise `Some(m)` with m the least element | `list_min(List(3, 1, 2)) == Some(1)` |
| `list_max` | `List<Int> → Option<Int>` | Tier-2 | Empty → `None`; otherwise `Some(m)` with m the greatest element | `list_max(List(3, 1, 2)) == Some(3)` |
| `list_take` | `(List<Int>, Int) → List<Int>` | Tier-2 | The first n elements, n clamped into [0, len(xs)]: n ≤ 0 → empty; n ≥ len(xs) → xs unchanged | `list_take(List(1, 2, 3), 2) == List(1, 2)` |
| `list_drop` | `(List<Int>, Int) → List<Int>` | Tier-2 | xs without its first n elements, with the same clamping | `list_drop(List(1, 2, 3), 2) == List(3)` |
| `list_is_sorted` | `List<Int> → Bool` | Tier-2 | True iff the list is non-strictly ascending (every adjacent pair a ≤ b); empty and singleton → true | `list_is_sorted(List(1, 2, 2, 3)) == true` |
| `list_sort` | `List<Int> → List<Int>` | Tier-2 | The elements in non-strict ascending order; the output is a permutation of the input (judged on the executed battery) | `list_sort(List(3, 1, 2)) == List(1, 2, 3)` |
| `list_binary_search` | `(List<Int>, Int) → Option<Int>` | Tier-2 | `require:` the list is sorted. Present → `Some(i)` with xs[i] equal to the target (with duplicate targets any valid index satisfies the contract — the specific index is spec-silent); absent → `None`. The shipped body is a first-match scan (the G5 scope-around: generated Rust cannot index a Vec by an Int variable) | `list_binary_search(List(1, 2, 3), 2) == Some(1)` |

### The old Appendix A names, accounted

The pre-1.0 Appendix A listed 20 names as "(Planned)".
Two were already real (`len`, `int_to_str` — the builtins
above). Of the remaining 18 (the accounting frozen in
`STDLIB_SCOPE.md` §2.7):

- **Ship (10)**, as the `list_*` entries above: `is_sorted`
  → `list_is_sorted`, `contains` → `list_contains`,
  `reverse` → `list_reverse`, `sort` → `list_sort`, `sum`
  → `list_sum`, `product` → `list_product`, `min` →
  `list_min`, `max` → `list_max` (the scalar forms ship as
  `int_min` / `int_max`), `take` → `list_take`, `drop` →
  `list_drop`.
- **Removed (8)**, each with its recorded reason:
  `is_permutation` (its constraint form is a `Predicate` —
  unencodable; the property survives inside `list_sort`'s
  executed battery, not as an entry), `filter`, `map`,
  `fold`, `all`, `any` (the higher-order class — out per
  D6a; interpreter function values, G4, are the first
  post-1.0 build), `zip` (tuple elements are unmodelled at
  every layer), `range` (no analysis in the Phase 106
  audit — excluded conservatively).

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
