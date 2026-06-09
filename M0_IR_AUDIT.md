# M0: IR Audit — Self-Hosting Readiness

> Date: 2026-06-09
> Before writing a single `.morph` spec, we must know: can the IR express compiler-level code?

---

## Methodology

Scanned all 1,168 lines of `src/spec/parser.rs` (the simplest real compiler component).
Every Rust pattern was tagged: ✅ (IR supports) or ❌ (IR gap).

---

## Findings

### ✅ Supported Patterns (16 patterns)

| Pattern | IR Construct | Frequency |
|---|---|---|
| `while` loop | `IRNode::While` | 8 uses |
| `match` expression | `IRNode::Match` | 14 uses |
| `let` binding | `IRNode::Let` | 12 uses |
| `if/else` | `IRNode::If` | 6 uses |
| `for` loop | `IRNode::For` | 2 uses |
| Function call | `IRNode::Call` | 40+ uses |
| `return` | `IRNode::Return` | 6 uses |
| Binary/Unary ops | `IRNode::BinOp/UnaryOp` | 20+ uses |
| `Vec::push()` | `IRNode::Call` | 15 uses |
| `Vec::new()` | `IRNode::Collection` | 8 uses |
| `.iter()/.map()/.filter()/.collect()` | `IRNode::Map/Filter` | 4 uses |
| Integer/boolean literals | `IRNode::Literal` | 30+ uses |
| Variable reference | `IRNode::Var` | 100+ uses |
| Block scoping | `IRNode::Block` | Implicit |
| Lambda/closure | `IRNode::Lambda` | 0 uses (available) |
| Allocation | `IRNode::Alloc` | Available |

### ❌ IR GAPS — Critical for Self-Hosting (9 gaps)

| # | Pattern | Why Critical | What's Missing |
|---|---|---|---|
| G1 | **Struct definitions** | Parser has `Parser<'a> { chars, pos, line, col }` — stateful struct | `IRNode::StructDef` + `IRNode::ImplBlock` |
| G2 | **Self methods** | `self.peek()`, `self.advance()`, `self.is_eof()` — OOP-style | `IRNode::MethodCall` with implicit self |
| G3 | **Result with ?** | EVERY function returns `Result<_, ParseError>` and uses `?` for propagation | `IRNode::TryPropagate` or equivalent |
| G4 | **`loop {}`** | Parser has `loop { ... break }` pattern — not `while` with condition | `IRNode::Loop` with `IRNode::Break` |
| G5 | **String operations** | `s.push()`, `chars().collect()`, `.as_str()`, `.to_string()`, `format!()` | Primitive string methods |
| G6 | **`&str` parameters** | `eat_keyword(&mut self, kw: &str)` — reference types | Reference type in IR |
| G7 | **Mutable reference (`&mut self`)** | ALL parser methods mutate `self` | `IRNode::MutRef` or `&mut` type |
| G8 | **`Option` / `.unwrap()` pattern** | `self.peek()` returns `Option<char>`, `.unwrap_or()` everywhere | `IRNode::OptionUnwrap` |
| G9 | **Enum definitions** | `ParseError`, `BinOp`, `Constraint`, etc. — ADT definitions | `IRNode::EnumDef` |

---

## Impact Assessment

```
Critical path analysis — can we synthesize a parser WITHOUT these?

G1 (struct):      CANNOT — parser IS a struct with state. BLOCKING.
G2 (self):        CANNOT — all parser methods mutate struct fields via self.
G3 (Result/?):    CAN work around with explicit match, but 10x more code.
G4 (loop):        CAN work around with while true { } — acceptable.
G5 (string ops):  CAN work around by wrapping in helper functions.
G6 (&str):        CAN work around by using String + indexing.
G7 (&mut self):   CANNOT — same as G2. Mutating self through refs.
G8 (Option):      CAN work around with explicit match.
G9 (enum):        CAN work around with tagged unions (i8 discriminant + struct).
```

**Verdict: 3 BLOCKING gaps (G1, G2, G7). Self-hosting cannot proceed without resolving them.**

---

## Resolution Plan

### Phase A: Add Minimal IR Nodes (Before any .morph writing)

```
IRNode additions needed:
├── StructDef { name, fields: Vec<(String, TypeRef)> }  // G1
├── ImplBlock { target, methods: Vec<IRNode> }           // G1
├── Method { name, self_kind, params, body }             // G2 + G7
├── Self_ { method, args }                               // G2 (self.method())
├── Loop { body }                                        // G4
├── Break(Option<Box<IRNode>>)                          // G4 (break with value)
├── Continue                                            // G4
├── TryPropagate(Box<IRNode>)                           // G3 (?)
├── OptionUnwrap { option, kind }                       // G8
└── EnumDef { name, variants }                          // G9
```

### Phase B: String/Reference Workarounds

- `&str` → `String` with offset-based indexing
- `format!()` → decompose into concatenation IR nodes (IRNode::Concat)
- `.as_str()` → Reinterpret String as str (no-op in generated Rust)
- `self.peek()` → explicit `self.chars[pos]` via IRNode::Index

### Phase C: Method Over Struct

Before synthesis, rewrite parser from OOP-style (`self.method()`) to functional-style (pass struct explicitly). This avoids needing G2+G7 in the IR at all.

```rust
// BEFORE (needs self, &mut)
fn advance(&mut self) { self.pos += 1; }

// AFTER (functional, pass struct)
fn advance(state: ParserState) -> ParserState { 
    ParserState { pos: state.pos + 1, ..state }
}
```

This is more Rust code but eliminates 3 blocking IR gaps at once.

---

## Decision

| Approach | Pros | Cons | Verdict |
|---|---|---|---|
| Add 10 IR nodes + handle self | General solution | 10 new IR nodes, all visitors need updating | Heavy but permanent |
| Functional rewrite parser | No IR changes needed | More code, slower, less idiomatic | Lightweight but hacky |
| **HYBRID** | Minimal IR additions, functional-style for self | Medium effort | ✅ **RECOMMENDED** |

**Hybrid plan:**
1. Add ONLY: `Loop`, `Break`, `StructDef`, `TryPropagate` (4 nodes)
2. Use functional-style struct passing (no `self`, no `&mut self`, no `impl`)
3. Use explicit `match` instead of `?`
4. Use `String` + index instead of `&str`

---

## Impact on Timeline

| Task | Effort | Blocks |
|---|---|---|
| Add 4 IR nodes | 2 hours | Nothing |
| Update all visitors (depth, count, codegen) | 3 hours | Synthesis |
| Rewrite parser example in functional style | 2 hours | M1 |
| Test IR roundtrip | 1 hour | M1 |
| **Total M0 overhead** | **~1 day** | |

---

## Gate Decision

- [x] **IR audit complete** — 9 gaps identified, 3 blocking
- [ ] **4 IR nodes added** — Loop, Break, StructDef, TryPropagate
- [ ] **All visitors updated** — depth, count, codegen, find_holes
- [ ] **Functional parser example written** — proof that IR can express parser
- [ ] **Test passes** — existing 25 tests unchanged + new IR tests

**NEXT:** Proceed to IR node additions (Phase A hybrid).
