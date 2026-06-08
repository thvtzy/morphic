# 🏗 Morphic Architecture

> **v0.1.0** — Pre-alpha. Architecture is fluid but principled.

---

## High-Level Pipeline

```
.morph File
    │
    ▼
┌──────────────────────────────────────────────────────────────┐
│                     MORPHIC COMPILER                          │
│                                                               │
│  ┌──────────┐   ┌───────────┐   ┌────────────────────────┐   │
│  │  PARSER  │──▶│ TYPE CK   │──▶│  SYNTHESIS ENGINE       │   │
│  │ (1 pass) │   │ (1 pass)  │   │                          │   │
│  └──────────┘   └───────────┘   │  ┌────────────────────┐ │   │
│                                  │  │ PHASE 1: SEED      │ │   │
│                                  │  │ • LLM candidates   │ │   │
│                                  │  │ • Template patterns│ │   │
│                                  │  │ • Test induction   │ │   │
│                                  │  │ • Random bootstrap │ │   │
│                                  │  └────────┬───────────┘ │   │
│                                  │           ▼              │   │
│                                  │  ┌────────────────────┐ │   │
│                                  │  │ PHASE 2: MCTS      │ │   │
│                                  │  │ • Select (UCT)     │ │   │
│                                  │  │ • Expand           │ │   │
│                                  │  │ • Rollout          │ │   │
│                                  │  │ • Backpropagate    │ │   │
│                                  │  └────────┬───────────┘ │   │
│                                  │           ▼              │   │
│                                  │  ┌────────────────────┐ │   │
│                                  │  │ PHASE 3: VERIFY    │ │   │
│                                  │  │ • IR → SMT-LIB2    │ │   │
│                                  │  │ • Z3 Theorem Prover│ │   │
│                                  │  │ • Counterexample   │ │   │
│                                  │  └────────┬───────────┘ │   │
│                                  │           ▼              │   │
│                                  │  ┌────────────────────┐ │   │
│                                  │  │ PHASE 4: EVOLVE    │ │   │
│                                  │  │ • Tournament select│ │   │
│                                  │  │ • Crossover        │ │   │
│                                  │  │ • Mutation         │ │   │
│                                  │  └────────────────────┘ │   │
│                                  └──────────┬───────────────┘   │
│                                             ▼                   │
│  ┌──────────────┐   ┌───────────────┐                          │
│  │  SELECTOR    │──▶│   CODEGEN     │                          │
│  │  (Pareto)    │   │  (5 targets)  │                          │
│  └──────────────┘   └───────────────┘                          │
└──────────────────────────────────────────────────────────────┘
                                             │
                    ┌────────────────────────┼────────────────────────┐
                    ▼                        ▼                        ▼
              Rust Source              C Source              WASM / JS / PY
```

---

## Module Map

### `src/spec/` — The Spec Language

| File | Role | Lines |
|---|---|---|
| `ast.rs` | Abstract Syntax Tree — 30+ type variants for specs, constraints, expressions, types | 380 |
| `parser.rs` | Recursive descent parser — `.morph` source → AST | 1,168 |
| `typeck.rs` | Type checker — variable scoping, type unification, constraint validation | 267 |

**Key types:**
- `Spec` — top-level spec file (name, imports, functions, types, invariants)
- `FunctionSpec` — a single spec (params, pre/postconditions, complexity bounds, tests)
- `Constraint` — logical formulas (Forall, Exists, Implies, Eq, Order, Predicate...)
- `Expr` — expression nodes (Var, Literal, Call, BinOp, Lambda, Comprehension...)
- `TypeRef` — type references (Int, List<T>, Map<K,V>, Function(A,B), Stream<T>...)

### `src/synthesis/` — The Synthesis Engine

| File | Role | Lines |
|---|---|---|
| `engine.rs` | **Core engine** — MCTS, Genetic Algorithm, LLM integration, seed population, IR types | 1,802 |
| `selector.rs` | Pareto-optimal candidate selection, tournament, dominance analysis | 328 |
| `interactive.rs` | Live synthesis dashboard — real-time progress bars, candidate streaming | 232 |

**Key types:**
- `CandidateImplementation` — a synthesized implementation (IR body, score, provenance)
- `IRNode` — 30+ Intermediate Representation node types (the heart of Morphic)
- `MCTSNode` — Monte Carlo Tree Search node (state, visits, reward, children)
- `SynthesisConfig` — configurable parameters for each phase
- `SynthesisEngine` — orchestrator (seed → MCTS → verify → refine → score)

**The IR (Intermediate Representation):**
```
IRNode variants:
├── Control: Hole, Block, Let, If, While, For, Match, Return
├── Values:  Literal, Var, Collection, Typed
├── Ops:     BinOp, UnaryOp
├── Funcs:   Call, Lambda, Closure
├── HOF:     Map, Filter, Fold, Sort
├── State:   Alloc, Assign
└── IRPattern: Wildcard, Variable, Literal, Constructor, Guard
```

### `src/verify/` — Formal Verification

| File | Role | Lines |
|---|---|---|
| `verifier.rs` | Z3 verification pipeline — parallel verification, counterexample generation | 642 |
| `smt.rs` | SMT-LIB2 translation — IR → SMT formula, Z3 subprocess/FFI interface | 328 |

**Key types:**
- `Verifier` — main verification engine (per-constraint, with timeouts)
- `VerificationResult` — status (Verified/Failed/Inconclusive) + counterexample
- `SmtSolver` — Z3 interface (subprocess or FFI)
- `SmtFormula` — SMT-LIB2 formula builder (declarations, assertions, options)

### `src/codegen/` — Code Generation

| File | Role | Lines |
|---|---|---|
| `mod.rs` | Multi-target codegen — IR → Rust/C/WASM/Python/JavaScript | 932 |

Each target has: type translation, expression translation, statement translation, test generation.

### `src/main.rs` — CLI

| Commands | Description |
|---|---|
| `morphic build <file>` | Compile .morph → synthesized implementation |
| `morphic check <file>` | Verify spec without codegen |
| `morphic synthesize <file>` | Interactive synthesis with live dashboard |
| `morphic init <name>` | Create new Morphic project |
| `morphic lsp` | Language Server Protocol (TODO) |

---

## Synthesis Algorithm (Detail)

### Phase 1: Seed Population

```
Input: FunctionSpec
Output: Initial population of CandidateImplementations

1. TEST-INDUCED SEED:
   For each test (input → expected_output):
     Generate IR match: pattern(input) → return(expected_output)
     Default case: Hole (to be filled by search)

2. TEMPLATE SEEDS:
   Match spec shape to known patterns:
   - List<T> → List<T> with sorting constraints → "Divide & Conquer" template
   - Collection + predicate → "Iterator Chain" template
   - Simple transformation → "Loop" template

3. LLM SEEDS (if enabled):
   Build prompt from spec → call LLM → parse output → multiple IR candidates

4. RANDOM SEEDS (bootstrap diversity):
   Generate random IR trees up to depth 3
```

### Phase 2: Monte Carlo Tree Search

```
For iteration in 0..max_iterations:
  1. SELECT:
     Start from root. While node has children:
       Pick child with max UCT value:
         UCT = (reward / visits) + C * sqrt(ln(parent_visits) / visits)
     Unvisited children get priority (UCT = ∞)

  2. EXPAND:
     Find first Hole in selected node's IR
     Generate diverse fillings:
       - Type-directed (based on expected_type)
       - From known library functions
       - LLM-generated completions
     Create one child per filling

  3. ROLLOUT:
     Pick random child, fill remaining holes randomly
     Complete IR → evaluate → get reward

  4. BACKPROPAGATE:
     Propagate reward up through ancestor nodes
     visits += 1, total_reward += reward
```

### Phase 3: Z3 Verification

```
For each candidate:
  For each constraint:
    1. Build SMT formula:
       - Assert: implementation(input) = output
       - Assert: NOT(constraint(input, output))
    2. Query Z3:
       - UNSAT → constraint ALWAYS holds ✓
       - SAT   → Z3 found a counterexample ✗
    3. If counterexample: record inputs/output for debugging

  Compute constraint_score = passed / total
```

### Phase 4: Genetic Refinement

```
For generation in 0..10:
  1. EVALUATE all candidates (score = weighted composite)
  2. ELITISM: keep top 10%
  3. Fill rest:
     - TOURNAMENT SELECT (size 5): pick 2 parents
     - CROSSOVER (p=0.7): swap compatible subtrees
     - MUTATION (p=0.3):
       • un-synthesize (replace subtree with hole)
       • inline (inline function call)
       • swap (swap equivalent expressions)
       • tighten loop (optimize iteration)
       • parallel hint (add rayon annotation)
       • random rewrite
```

---

## Key Design Decisions

### 1. IR-First, Not Text-First
All synthesis operates on the IR — not on source code text. This means:
- Mutations are type-safe
- Crossovers are semantically coherent
- SMT translation is direct and reliable
- Codegen can target any language

### 2. Constraints as the Specification Language
Instead of examples or test cases alone, Morphic uses **constraints**:
- **Preconditions**: what must be true before the function runs
- **Postconditions**: what must be true after
- **Invariants**: what must always be true
- **Complexity bounds**: O(n log n), O(1) space, etc.

Constraints are directly translatable to SMT formulas for Z3.

### 3. Hybrid Over Monolithic
No single technique solves program synthesis:
- **LLMs** are creative but unreliable
- **Search** is systematic but slow
- **Formal methods** are precise but need candidates

Combining all three gives each technique's strengths while covering weaknesses.

### 4. Progressive Optimization Levels
```
Opt Level 1: Correctness only (small search, no LLM)
Opt Level 2: Correctness + reasonable performance (LLM enabled)
Opt Level 3: Aggressive optimization (full MCTS + GA)
Opt Level 4: Maximum (exhaustive search, large population)
```

---

## Future: Self-Hosting

The endgame: Morphic written in Morphic.

```
Phase 1: Write Morphic compiler's specs in Morphic
Phase 2: Morphic synthesizes its own components
Phase 3: Self-optimizing — the compiler improves itself
```

This is on the roadmap for v0.4+.

---

## Dependencies

| Dependency | Role | Optional? |
|---|---|---|
| `logos` | Lexer generator | No |
| `chumsky` | Parser combinator | No (future) |
| `clap` | CLI argument parsing | No |
| `colored` | Terminal colors | No |
| `indicatif` | Progress bars | No |
| `petgraph` | Graph data structures | No |
| `rayon` | Parallel computation | No |
| `dashmap` | Concurrent hashmap | No |
| `serde` / `serde_json` | Serialization | No |
| `rand` | Random number generation | No |
| `z3` | Z3 FFI bindings | Yes (feature `z3-static`) |
| `llama-cpp-2` | Local LLM inference | Yes (feature `llm-local`) |
| `reqwest` / `tokio` | Remote LLM API | Yes (feature `llm-remote`) |
