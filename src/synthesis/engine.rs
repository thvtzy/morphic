// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC SYNTHESIS ENGINE                                │
// │                                                          │
// │  Architecture:                                            │
// │  ┌─────────┐   ┌──────────┐   ┌──────────┐   ┌────────┐ │
// │  │  Spec   │──▶│   MCTS   │──▶│   Gen    │──▶│ Verify │ │
// │  │  Tree   │   │  Search  │   │ Candidate│   │  (Z3)  │ │
// │  └─────────┘   └──────────┘   └──────────┘   └────────┘ │
// │                      │                ▲            │     │
// │                      └─────── reward ─┘            │     │
// │                                 ┌────────────┐     │     │
// │                                 │  LLM Hint  │◀────┘     │
// │                                 └────────────┘           │
// └──────────────────────────────────────────────────────────┘

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::spec::ast::*;
use crate::verify::verifier::VerificationStatus;
use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};

// ── Global synthesis statistics ──────────────────────────

static CANDIDATE_COUNT: AtomicU64 = AtomicU64::new(0);
static ELAPSED_MS: AtomicU64 = AtomicU64::new(0);

pub fn candidate_count() -> u64 { CANDIDATE_COUNT.load(Ordering::Relaxed) }
pub fn elapsed_ms() -> u64 { ELAPSED_MS.load(Ordering::Relaxed) }

// ── Engine RNG (Phase 66, v0.6 D3) ─────────────────────────
//
// Every random draw in the engine goes through ONE
// thread-local stream, replacing the `engine_rng()`
// handles the code used before (same distribution role,
// same call sites). By default the stream is seeded from
// OS entropy — the native CLI's behaviour is unchanged:
// its runs were never reproducible and still are not. The
// playground profile seeds the stream explicitly through
// `seed_engine_rng`, after which a run is a pure function
// of (spec, seed): reproducible bug reports and pinnable
// gate cases. Handles are clones of one Rc, so a draw
// through any handle advances the single stream, exactly
// like the thread_rng handles this replaces.

thread_local! {
    static ENGINE_RNG: Rc<RefCell<StdRng>> =
        Rc::new(RefCell::new(StdRng::from_entropy()));
}

/// Seed the engine's RNG stream. Call before a synthesis
/// run to make it reproducible (the playground/D3 path);
/// the native CLI never calls this.
pub fn seed_engine_rng(seed: u64) {
    ENGINE_RNG.with(|r| *r.borrow_mut() = StdRng::seed_from_u64(seed));
}

/// A handle on the thread's engine RNG stream.
#[derive(Clone)]
pub(crate) struct SharedRng(Rc<RefCell<StdRng>>);

impl RngCore for SharedRng {
    fn next_u32(&mut self) -> u32 {
        self.0.borrow_mut().next_u32()
    }
    fn next_u64(&mut self) -> u64 {
        self.0.borrow_mut().next_u64()
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.borrow_mut().fill_bytes(dest)
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.0.borrow_mut().try_fill_bytes(dest)
    }
}

/// The engine's RNG handle — the one replacement for every
/// `engine_rng()` call site in this module.
pub(crate) fn engine_rng() -> SharedRng {
    ENGINE_RNG.with(|r| SharedRng(r.clone()))
}

// ── Candidate Representation ──────────────────────────────

/// A synthesized implementation candidate
///
/// Each candidate is an AST subtree representing a possible
/// implementation of the spec. Candidates live in a space
/// where mutations, crossovers, and refinements are valid
/// operations.
#[derive(Debug, Clone)]
pub struct CandidateImplementation {
    /// Unique identifier
    pub id: u64,

    /// The implementation body (in Intermediate Representation)
    pub body: IRNode,

    /// Source function this implements
    pub spec_name: String,

    /// The spec this implements
    pub spec: FunctionSpec,

    /// Quality score (0.0–1.0)
    pub score: f64,

    /// Scores broken down by dimension
    pub scores: ScoreBreakdown,

    /// Number of mutations from original seed
    pub generation: u64,

    /// How this candidate was created
    pub provenance: Provenance,

    /// Whether this candidate passed verification
    pub verified: bool,

    /// The verifier's full verdict on this candidate, recorded
    /// by `verify_all` at the moment it is computed (the single
    /// source of truth — selection reads it, never recomputes
    /// it). `None` until the candidate has been through
    /// verification. `verified` above is the collapsed form
    /// (`status == Verified`) kept for existing consumers.
    pub verification_status: Option<VerificationStatus>,
}

#[derive(Debug, Clone)]
pub struct ScoreBreakdown {
    /// How many test cases passed
    pub test_pass_ratio: f64,
    /// How well it satisfies constraints (Z3 score)
    pub constraint_score: f64,
    /// Complexity efficiency vs spec bounds
    pub complexity_score: f64,
    /// Readability / code quality
    pub quality_score: f64,
    /// Synthesis runtime in microseconds
    pub synthesis_time_us: u64,
}

#[derive(Debug, Clone)]
pub enum Provenance {
    /// Generated by LLM from spec
    LLMGenerated { model: String, prompt_hash: u64 },
    /// Random mutation of parent
    Mutation { parent_id: u64, mutation_kind: String },
    /// Crossover between two parents
    Crossover { parent_a: u64, parent_b: u64 },
    /// Seeded from test cases (inductive synthesis)
    TestInduced { num_tests: usize },
    /// Seeded from the spec's constraints — the declared
    /// contract rather than measured examples: an equation
    /// constraint used directly as the body, or a set of
    /// guard-table constraints compiled into a match. `kind`
    /// is "equation" or "guard_table".
    ConstraintInduced { kind: String },
    /// Built from predefined template
    Template { template_name: String },
    /// Human-provided partial implementation
    HumanGuided,
}

// ── Intermediate Representation (IR) ─────────────────────

/// The Morphic IR — an abstraction over concrete syntax.
///
/// The IR is designed to be:
/// - Language-agnostic (compile to Rust, C, WASM, etc.)
/// - Mutable (nodes can be rewritten during search)
/// - Verifiable (easily lowered to Z3 SMT-LIB)
/// - Lightweight (cheap to clone for GA/MCTS operations)
#[derive(Debug, Clone)]
pub enum IRNode {
    /// Empty / hole — to be filled by synthesis
    Hole { id: u64, expected_type: Option<TypeRef> },

    /// Sequence of statements
    Block(Vec<IRNode>),

    /// Variable binding: let x = value in body
    Let {
        name: String,
        value: Box<IRNode>,
        body: Box<IRNode>,
    },

    /// If-then-else
    If {
        condition: Box<IRNode>,
        then_branch: Box<IRNode>,
        else_branch: Box<IRNode>,
    },

    /// While loop
    While {
        condition: Box<IRNode>,
        body: Box<IRNode>,
        invariant: Option<String>, // Z3 loop invariant
    },

    /// For loop with known bound
    For {
        var: String,
        start: Box<IRNode>,
        end: Box<IRNode>,
        body: Box<IRNode>,
    },

    /// Pattern match
    Match {
        scrutinee: Box<IRNode>,
        cases: Vec<(IRPattern, IRNode)>,
    },

    /// Function call
    Call {
        function: String,
        args: Vec<IRNode>,
    },

    /// Return statement
    Return(Box<IRNode>),

    /// Literal value
    Literal(IRLiteral),

    /// Variable reference
    Var(String),

    /// Binary operation
    BinOp(IRBinOp, Box<IRNode>, Box<IRNode>),

    /// Unary operation
    UnaryOp(IRUnaryOp, Box<IRNode>),

    /// Lambda expression
    Lambda {
        params: Vec<(String, TypeRef)>,
        body: Box<IRNode>,
    },

    /// Closure (lambda with captured environment)
    Closure {
        params: Vec<(String, TypeRef)>,
        captures: Vec<String>,
        body: Box<IRNode>,
    },

    /// Collection literal
    Collection {
        kind: CollectionKind,
        elements: Vec<IRNode>,
    },

    /// Map / Higher-order function
    Map {
        function: Box<IRNode>,
        over: Box<IRNode>,
    },

    /// Filter
    Filter {
        predicate: Box<IRNode>,
        over: Box<IRNode>,
    },

    /// Fold / Reduce
    Fold {
        operation: Box<IRNode>,
        initial: Box<IRNode>,
        over: Box<IRNode>,
    },

    /// Sort by comparator
    Sort {
        comparator: Box<IRNode>,
        collection: Box<IRNode>,
    },

    /// Allocate mutable state
    Alloc {
        name: String,
        ty: TypeRef,
        initial: Box<IRNode>,
    },

    /// Mutation
    Assign {
        target: Box<IRNode>,
        value: Box<IRNode>,
    },

    /// Raw expression with type annotation
    Typed {
        node: Box<IRNode>,
        ty: TypeRef,
    },

    /// Index access: collection[index]
    Index(Box<IRNode>, Box<IRNode>),

    // ── v0.5 Self-Hosting IR additions ──────────────────

    /// Infinite loop with body (until Break). The
    /// `invariant` field is Phase 96's addition, the
    /// `While` precedent's sibling: raw spec-syntax
    /// constraint text over the loop's environment,
    /// given semantics by the verifier's statement judge
    /// (initiation / consecution / exit obligations) and
    /// by nothing else — codegen ignores it, exactly as
    /// it treats a `While` invariant as comment text.
    /// A field addition to an existing variant: no IR
    /// kind code moves.
    Loop {
        body: Box<IRNode>,
        invariant: Option<String>, // Z3 loop invariant
    },

    /// Break from a Loop, optionally returning a value
    Break { value: Option<Box<IRNode>> },

    /// Struct/record type definition
    StructDef {
        name: String,
        fields: Vec<(String, TypeRef)>,
        body: Box<IRNode>,
    },

    /// Try-operand: if Result is Err, propagate it up
    /// Equivalent to Rust's `?` operator
    TryPropagate(Box<IRNode>),

    // ── M1 feature additions (Phase 73) ────────────────────

    /// Field access on a record value: base.field
    /// (M1 feature 2, decision D2a). Appended at the END of
    /// this enum: kind 28 in the IRNode kind-code table the
    /// Codegen specs pin (0–27 through TryPropagate); no
    /// existing code moves. The interpreter's record value
    /// lands in Phase 75 and the Z3 record encoding in
    /// Phase 76 — until then the evaluation/encoding arms
    /// are honest NotEvaluable/unencodable arms, named as
    /// such at each site.
    FieldAccess {
        base: Box<IRNode>,
        field: String,
    },

    /// Record construction: TypeName { field: value, ... }
    /// (M1 feature 3, decision D3a — the IR half of the spec
    /// level Expr::RecordLit). Appended at the END: kind 29.
    /// Field order is the literal's written order; the
    /// declared field order is a typechecker concern
    /// (Phase 74).
    RecordLit {
        type_name: String,
        fields: Vec<(String, IRNode)>,
    },
}

#[derive(Debug, Clone)]
pub enum IRLiteral {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Char(char),
    Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IRBinOp {
    Add, Sub, Mul, Div, Mod,
    And, Or, Xor,
    Eq, Neq, Lt, Le, Gt, Ge,
    Shl, Shr,
    Concat,
    Append,
    Merge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IRUnaryOp {
    Neg, Not, Abs, Len,
    Head, Tail, Init, Last,
    Reverse,
    Clone,
}

#[derive(Debug, Clone)]
pub enum IRPattern {
    Wildcard,
    Variable(String),
    Literal(IRLiteral),
    Constructor(String, Vec<IRPattern>),
    Guard(Box<IRPattern>, String), // Pattern with guard condition
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionKind {
    List,
    Set,
    Map,
    Array,
    Tuple,
}

// ── MCTS Node ─────────────────────────────────────────────

/// A node in the Monte Carlo Tree Search over implementation space
#[derive(Debug, Clone)]
struct MCTSNode {
    /// The IR state at this node
    state: IRNode,

    /// Parent node index (for backpropagation)
    parent: Option<usize>,

    /// Children indices
    children: Vec<usize>,

    /// Visit count for UCT
    visits: u64,

    /// Total accumulated reward
    total_reward: f64,

    /// Whether this is a terminal (complete implementation)
    is_terminal: bool,

    /// Number of holes remaining
    hole_count: usize,
}

impl MCTSNode {
    fn uct_value(&self, parent_visits: u64, exploration: f64) -> f64 {
        if self.visits == 0 {
            return f64::MAX; // Unvisited nodes get priority
        }
        let exploitation = self.total_reward / self.visits as f64;
        let exploration = exploration * ((parent_visits as f64).ln() / self.visits as f64).sqrt();
        exploitation + exploration
    }
}

// ── Synthesis Configuration ───────────────────────────────

#[derive(Debug, Clone)]
pub struct SynthesisConfig {
    /// Optimization level (1=correctness only, 4=aggressive optimization)
    pub opt_level: u8,

    /// Maximum MCTS iterations
    pub max_iterations: u64,

    /// Maximum wall-clock time (milliseconds)
    pub max_time_ms: u64,

    /// Exploration constant for UCT selection
    pub uct_exploration: f64,

    /// Population size for genetic phase
    pub population_size: usize,

    /// Tournament size for selection
    pub tournament_size: usize,

    /// Mutation rate (probability per node)
    pub mutation_rate: f64,

    /// Crossover rate
    pub crossover_rate: f64,

    /// Whether to use LLM for candidate generation
    pub use_llm: bool,

    /// Whether to use Z3 for verification
    pub use_z3: bool,

    /// Minimum score threshold for acceptance
    pub min_score: f64,
}

impl Default for SynthesisConfig {
    fn default() -> Self {
        Self {
            opt_level: 3,
            max_iterations: 10_000,
            max_time_ms: 30_000, // 30 seconds
            uct_exploration: 1.414,
            population_size: 100,
            tournament_size: 5,
            mutation_rate: 0.3,
            crossover_rate: 0.7,
            use_llm: true,
            use_z3: true,
            min_score: 0.0,
        }
    }
}

impl SynthesisConfig {
    pub fn from_opt_level(level: u8) -> Self {
        match level {
            1 => Self {
                opt_level: 1,
                max_iterations: 1_000,
                max_time_ms: 5_000,
                use_llm: false,
                use_z3: true,
                population_size: 20,
                ..Default::default()
            },
            2 => Self {
                opt_level: 2,
                max_iterations: 5_000,
                max_time_ms: 15_000,
                use_llm: true,
                use_z3: true,
                population_size: 50,
                ..Default::default()
            },
            3 => Self::default(),
            4 => Self {
                opt_level: 4,
                max_iterations: 100_000,
                max_time_ms: 60_000,
                use_llm: true,
                use_z3: true,
                population_size: 500,
                uct_exploration: 2.0,
                min_score: 0.9,
                ..Default::default()
            },
            _ => Self::default(),
        }
    }
}

// ── Synthesis Engine ──────────────────────────────────────

pub struct SynthesisEngine {
    config: SynthesisConfig,
    candidates: Vec<CandidateImplementation>,
    next_id: AtomicU64,
    /// The module the synthesized spec lives in, consulted when
    /// admitting constraint seeds whose bodies call other specs.
    /// Empty unless the engine was built with `with_module` —
    /// a spec synthesized standalone behaves exactly as before
    /// (composition equations simply do not resolve, hence do
    /// not seed).
    spec_index: SpecIndex,
}

impl SynthesisEngine {
    pub fn new(config: SynthesisConfig) -> Self {
        Self {
            config,
            candidates: Vec::new(),
            next_id: AtomicU64::new(1),
            spec_index: SpecIndex::empty(),
        }
    }

    /// An engine that knows the whole module the target spec
    /// belongs to, so spec-to-spec calls in its constraints can
    /// resolve (see `SpecIndex`).
    pub fn with_module(config: SynthesisConfig, specs: &[FunctionSpec]) -> Self {
        let mut engine = Self::new(config);
        engine.spec_index = SpecIndex::from_specs(specs);
        engine
    }

    /// An engine that knows the whole module INCLUDING its
    /// type declarations (Phase 81): the record-era seed
    /// families read declared record fields from the index
    /// (the record-completion builder, remainder (i-b)),
    /// exactly as the verifier's `with_module` index does
    /// since Phase 76. Modules declaring no record types
    /// behave identically under either constructor — the
    /// declarations are inert for every other family.
    pub fn with_module_spec(config: SynthesisConfig, module: &Spec) -> Self {
        let mut engine = Self::new(config);
        engine.spec_index = SpecIndex::from_module(module);
        engine
    }

    fn new_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn run(&mut self, spec: &FunctionSpec) -> Result<Vec<CandidateImplementation>, SynthesisError> {
        let start = crate::timeutil::now();

        // Phase 1: Seed the population
        let mut population = self.seed_population(spec)?;

        // Protect the spec-derived seeds (test-induced and
        // constraint-induced). They are the only candidates
        // whose shape is derived from the spec itself — its
        // measured behavior or its declared contract — and the
        // search phases are free to lose them: the GA's elitism
        // keeps only the top 10% by composite score per
        // generation, and nothing else in Phases 2–3 guarantees
        // their survival. They are re-admitted just before
        // final scoring (Phase 4), so selection always sees
        // them; whether they WIN is still up to their scores
        // and the selector's gates.
        let protected_seeds: Vec<CandidateImplementation> = population
            .iter()
            .filter(|c| {
                matches!(
                    c.provenance,
                    Provenance::TestInduced { .. } | Provenance::ConstraintInduced { .. }
                )
            })
            .cloned()
            .collect();

        // Phase 2: MCTS-guided exploration
        population = self.mcts_search(population, spec)?;

        // Phase 3: Genetic refinement
        if self.config.opt_level >= 3 {
            population = self.genetic_refine(population, spec)?;
        }

        // Re-admit any protected seed the search dropped, then
        // Phase 4 scores it on the same footing as everyone else.
        readmit_protected_seeds(&mut population, &protected_seeds);

        // Phase 4: Scoring & ranking
        self.score_and_rank(&mut population, spec);

        // Phase 5: Filter passing candidates
        let passing: Vec<CandidateImplementation> = population
            .into_iter()
            .filter(|c| c.score >= self.config.min_score)
            .collect();

        // Update stats
        CANDIDATE_COUNT.store(self.candidates.len() as u64, Ordering::Relaxed);
        ELAPSED_MS.store(start.elapsed_ms(), Ordering::Relaxed);

        Ok(passing)
    }

    // ── Phase 1: Seeding ──────────────────────────────────

    fn seed_population(&mut self, spec: &FunctionSpec) -> Result<Vec<CandidateImplementation>, SynthesisError> {
        let mut population = Vec::with_capacity(self.config.population_size);

        // Seed 1: From test cases (inductive synthesis)
        if !spec.tests.is_empty() {
            let test_induced = self.seed_from_tests(spec)?;
            population.extend(test_induced);
        }

        // Seed 2: From templates / pattern matching
        let template = self.seed_from_templates(spec)?;
        population.extend(template);

        // Seed 3: From constraints (spec-derived bodies: an
        // equation constraint used directly as the body, or a
        // guard-table constraint set compiled into a match).
        // These are the only seeds that can be spec-exact on
        // inputs no test exercises — the range/lookup/test-less
        // pools previously contained no Verified candidate in
        // any measured draw, so the refutation gate had nothing
        // spec-exact to crown there.
        let constraint_seeds = self.seed_from_constraints(spec);
        population.extend(constraint_seeds);

        // Seed 4: From LLM (if enabled)
        if self.config.use_llm {
            let llm_seeds = self.seed_from_llm(spec)?;
            population.extend(llm_seeds);
        }

        // Seed 5: Random AST generation (bootstrap diversity)
        while population.len() < self.config.population_size {
            let random = self.seed_random(spec);
            population.push(random);
        }

        // Deduplicate — TRUE duplicates only (Phase 59 fix).
        // This used to dedup by ir_equivalent, which compares
        // only root-variant discriminants: any two same-root
        // bodies counted as duplicates, so a Match-rooted
        // guard_table seed standing next to the Match-rooted
        // test-induced seed was silently dropped (through
        // Phase 58 the bare-Hole template seeds happened to
        // buffer the adjacency; structurally-keyed templates
        // removed the buffer and the M2/M3 guard-table
        // winners collapsed — see the regression test
        // guard_table_seed_survives_seed_population_dedup).
        // Body identity here is the Debug fingerprint, the
        // same identity readmit_protected_seeds already uses.
        // ir_equivalent itself is unchanged: replace_subtree
        // still uses it as its documented approximation.
        population.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        population.dedup_by(|a, b| ir_identical(&a.body, &b.body));
        population.truncate(self.config.population_size);

        self.candidates = population.clone();

        Ok(population)
    }

    fn seed_from_tests(&self, spec: &FunctionSpec) -> Result<Vec<CandidateImplementation>, SynthesisError> {
        if spec.tests.is_empty() {
            return Ok(Vec::new());
        }

        // Inductive synthesis: use test I/O pairs to derive implementation
        // Start with a match on test inputs → return test outputs
        let mut cases = Vec::new();
        for test in &spec.tests {
            let pattern = expr_to_pattern(&test.input);
            let output = expr_to_ir(&test.expected_output);
            cases.push((pattern, output));
        }

        // Wrap in a match with a default hole
        cases.push((IRPattern::Wildcard, IRNode::Hole {
            id: self.new_id(),
            expected_type: Some(spec.return_type.clone()),
        }));

        let body = IRNode::Match {
            scrutinee: Box::new(build_input_ref(&spec.params)),
            cases,
        };

        let candidate = CandidateImplementation {
            id: self.new_id(),
            body,
            spec_name: spec.name.clone(),
            spec: spec.clone(),
            score: 0.0,
            scores: empty_scores(),
            generation: 0,
            provenance: Provenance::TestInduced { num_tests: spec.tests.len() },
            verified: false,
            verification_status: None,
        };

        Ok(vec![candidate])
    }

    /// Seed 3: candidates built from the spec's constraints —
    /// the declared contract, where `seed_from_tests` uses the
    /// measured examples. Three shapes are admitted, each only
    /// when the resulting body lies in the verifier's encodable
    /// subset (a seed the verifier cannot judge would be one
    /// more Inconclusive pool member, indistinguishable from
    /// the junk this family exists to displace):
    ///
    /// - **equation**: a postcondition `output == <expr>` whose
    ///   right-hand side converts to IR whole (no dropped
    ///   subexpressions) becomes a candidate whose body is that
    ///   expression — the spec states its body outright.
    /// - **guard table**: a set of postconditions of the form
    ///   `<guard on the input> || output == <literal>` becomes
    ///   one match over the input with an arm per row plus a
    ///   wildcard arm carrying the typed-literal default for
    ///   the return type (the spec is silent off-table; the
    ///   default is a documented guess). The input may be an
    ///   Int (rows pin by interval scan) or a Bool (rows pin
    ///   by finite-domain enumeration).
    /// - **guarded identity**: a two-parameter spec whose rows
    ///   pin the output to the other parameter
    ///   (`<guards on g> || output == p`) becomes one match
    ///   over the guard parameter returning that parameter on
    ///   the pinned arms, with the same typed-literal wildcard
    ///   for the combinations no row pins.
    /// - **composition guard**: rows guarded by ONE shared
    ///   Bool-valued expression — a Bool parameter or a call
    ///   to another spec of the module — pinning the output to
    ///   an arbitrary expression over the parameters
    ///   (`rust_keyword(name)==true || output == name`) become
    ///   one match over that guard expression, with the call
    ///   resolved before admission like an equation's. The
    ///   guarded-identity shape generalized: it claims a call
    ///   guard or a compound pin outright, and — Phase 34's
    ///   precedence — a plain-variable guard with only
    ///   literal/bare-variable pins exactly when neither the
    ///   guard table nor guarded identity admits the spec,
    ///   so the families never produce two seeds for one
    ///   spec.
    /// - **multi-guard truth table**: rows guarded by a SET
    ///   of shared Bool-valued expressions — Bool parameters
    ///   or calls to indexed Bool specs — each row's leaves
    ///   constraining any subset of them and pinning the
    ///   output to an arbitrary expression over the
    ///   parameters (`has_string_pat==false||bare==true||
    ///   output==1`) become nested matches over the guard
    ///   expressions, the pinned cells of the joint
    ///   {false, true}^n table recovered by joint
    ///   falsify-every-leaf over the rows. The
    ///   composition-guard shape with the one-guard
    ///   requirement lifted: it requires at least two
    ///   distinct guard expressions (one is the composition
    ///   family's) and at most MAX_MULTI_GUARD_EXPRESSIONS,
    ///   so the families never produce two seeds for one
    ///   spec.
    /// - **expression-pin guard table**: a set of rows guarded
    ///   on ONE Int parameter (the key) whose output pins are
    ///   arbitrary expressions over the parameters
    ///   (`op<2||op>2||output==inner ++ ".len()"`) becomes one
    ///   match over the key with an arm per row plus the
    ///   typed-literal wildcard. It claims only shapes the
    ///   earlier families cannot state — at least one pin must
    ///   be a compound expression, where the guard table
    ///   admits only literals and guarded identity only the
    ///   one bare variable — so the families never produce two
    ///   seeds for one spec.
    /// - **pair-keyed guard table**: a set of rows guarded on
    ///   TWO Int parameters (the keys) whose output pins are
    ///   literals (`kind<9||kind>9||lit<3||lit>3||
    ///   output==true`) becomes one nested match over the
    ///   keys, the pinned cell recovered by joint
    ///   falsify-every-leaf over the pair — cells no row
    ///   decides stay unpinned and fall to the wildcard. It
    ///   claims only shapes the earlier families cannot
    ///   state — the guard table admits one parameter, the
    ///   expression-pin family requires a compound pin — so
    ///   the families never produce two seeds for one spec.
    /// - **mixed pair-keyed guard table**: a set of rows
    ///   guarded jointly on ONE Int parameter and ONE Bool
    ///   parameter (the keys) whose output pins are literals
    ///   (`position<1||position>1||last==false||output==1`)
    ///   becomes one nested match over the keys, the pinned
    ///   cells recovered by joint falsify-every-leaf over
    ///   the Int span crossed with the Bool domain — cells
    ///   no row decides stay unpinned and fall to the
    ///   wildcard. It claims only the (Int, Bool) key
    ///   signature no earlier family states — the guard
    ///   table admits one parameter, the pair-keyed table
    ///   two Int keys, the composition families Bool-domain
    ///   leaves only — so the families never produce two
    ///   seeds for one spec.
    ///
    /// A spec whose constraints fit neither shape gets no
    /// constraint seed and behaves exactly as before. Equations
    /// whose bodies call other specs are resolved against the
    /// engine's module index first (see `SpecIndex`): with a
    /// module they can seed like any other equation; without
    /// one — or when the call cannot resolve — they are skipped
    /// by the same admission rule.
    fn seed_from_constraints(&self, spec: &FunctionSpec) -> Vec<CandidateImplementation> {
        let mut seeds = Vec::new();
        // Whether any family has claimed the spec — read by
        // the forall-fold precedence gate below (a Cell so
        // the flag is readable between `admit` calls; the
        // closure's mutable borrow of `seeds` is not).
        let claimed = std::cell::Cell::new(false);
        let mut admit = |kind: &str, body: IRNode| {
            claimed.set(true);
            seeds.push(CandidateImplementation {
                id: self.new_id(),
                body,
                spec_name: spec.name.clone(),
                spec: spec.clone(),
                score: 0.0,
                scores: empty_scores(),
                generation: 0,
                provenance: Provenance::ConstraintInduced { kind: kind.into() },
                verified: false,
                verification_status: None,
            });
        };
        if let Some(body) = equation_seed_body(spec, &self.spec_index) {
            admit("equation", body);
        }
        if let Some(body) = guard_table_seed_body(spec, &self.spec_index) {
            admit("guard_table", body);
        }
        if let Some(body) = guarded_identity_seed_body(spec, &self.spec_index) {
            admit("guarded_identity", body);
        }
        if let Some(body) = composition_guard_seed_body(spec, &self.spec_index) {
            admit("composition_guard", body);
        }
        if let Some(body) = multi_guard_seed_body(spec, &self.spec_index) {
            admit("multi_guard", body);
        }
        if let Some(body) = expr_pin_guard_seed_body(spec, &self.spec_index) {
            admit("expr_pin_guard", body);
        }
        if let Some(body) = pair_guard_table_seed_body(spec, &self.spec_index) {
            admit("pair_guard_table", body);
        }
        if let Some(body) = mixed_pair_guard_table_seed_body(spec, &self.spec_index) {
            admit("mixed_pair_guard_table", body);
        }
        // Phase 81 (M1 remainders (i) + (i-b)): the two
        // record-era families, appended after every
        // standing family. Their territories are disjoint
        // from all of the above by shape (a two-row
        // complementary-guard spec over a computed guard;
        // a record-returning spec stated as partial field
        // equations), so no standing seed moves.
        if let Some(body) = complementary_guard_seed_body(spec, &self.spec_index) {
            admit("complementary_guard", body);
        }
        if let Some(body) = record_completion_seed_body(spec, &self.spec_index) {
            admit("record_completion", body);
        }
        // Phase 85 (M1 remainder (iii)): the tokenize
        // template family, appended after the Phase 81
        // families. Its territory is disjoint from
        // every standing family by shape (a tokenizing
        // step/loop over a state record, keyed on the
        // amended rows' shape), so no standing seed
        // moves; char_equation's right of first refusal
        // is untouched (its classifiers' shape is
        // disjoint from this one's).
        if let Some(body) = tokenize_template_seed_body(spec, &self.spec_index) {
            admit("tokenize_template", body);
        }
        if let Some(body) = char_equation_seed_body(spec, &self.spec_index) {
            admit("char_equation", body);
        }
        // Phase 60 (campaign slice G3): the forall-fold
        // family runs LAST and claims only a spec no
        // earlier family seeded — its territory rule,
        // enforced here (the char_equation right of first
        // refusal generalized to all nine families), so
        // the families never produce two constructed
        // bodies for one spec.
        if !claimed.get() {
            if let Some(body) = forall_fold_seed_body(spec, &self.spec_index) {
                admit("forall_fold", body);
            }
        }
        seeds
    }

    /// Instrument surface: the kinds of the constraint
    /// seeds this engine builds for `spec`, in family
    /// order. The Phase 60 firing inventory reads the
    /// family census directly instead of inferring it
    /// from synthesis pools.
    pub fn constraint_seed_kinds(&self, spec: &FunctionSpec) -> Vec<String> {
        self.seed_from_constraints(spec)
            .iter()
            .map(|c| match &c.provenance {
                Provenance::ConstraintInduced { kind } => kind.clone(),
                _ => unreachable!("constraint seeds carry ConstraintInduced provenance"),
            })
            .collect()
    }

    fn seed_from_templates(&self, spec: &FunctionSpec) -> Result<Vec<CandidateImplementation>, SynthesisError> {
        // Algorithm templates (Phase 59, campaign slice G2):
        // the template library (super::templates) is keyed
        // STRUCTURALLY on the spec's signature — parameter /
        // return types plus the comparator evidence of a
        // function-typed parameter, never on spec names — and
        // returns complete, hole-free bodies for the shapes it
        // can instantiate cleanly (at most one per spec), or
        // nothing. Through Phase 58 this function pushed three
        // bare Holes from local builders and the library had
        // no production caller; the Phase 57 census measured
        // the consequence (exactly one template candidate per
        // pool, always the same hole, collapsed by dedup).
        let mut seeds = Vec::new();
        for (kind, body) in super::templates::generate_templates(spec) {
            seeds.push(CandidateImplementation {
                id: self.new_id(),
                body,
                spec_name: spec.name.clone(),
                spec: spec.clone(),
                score: 0.0,
                scores: empty_scores(),
                generation: 0,
                provenance: Provenance::Template {
                    template_name: kind.name().into(),
                },
                verified: false,
                verification_status: None,
            });
        }
        Ok(seeds)
    }

    fn seed_from_llm(&self, spec: &FunctionSpec) -> Result<Vec<CandidateImplementation>, SynthesisError> {
        // Generate prompt for LLM
        let prompt = self.build_llm_prompt(spec);

        // Call LLM (local via llama.cpp or remote via API)
        let llm_output = self.call_llm(&prompt)?;

        // Parse LLM output into IR nodes
        let candidates = self.parse_llm_output(&llm_output, spec)?;

        Ok(candidates)
    }

    fn seed_random(&self, spec: &FunctionSpec) -> CandidateImplementation {
        // Fresh bodies are generated type-aware (see
        // random_ir_tree_typed): the seeds are the pool's
        // foundation — MCTS completions inherit their root
        // shape — so a type-blind seed poisons every completion
        // drawn from it, and the selector's return-type gate
        // cannot crown a sound body the pool does not contain.
        let body = random_ir_tree_typed(spec, 3, self);
        CandidateImplementation {
            id: self.new_id(),
            body,
            spec_name: spec.name.clone(),
            spec: spec.clone(),
            score: 0.0,
            scores: empty_scores(),
            generation: 0,
            provenance: Provenance::Mutation { parent_id: 0, mutation_kind: "random_seed".into() },
            verified: false,
            verification_status: None,
        }
    }

    // ── Phase 2: MCTS Search ──────────────────────────────

    fn mcts_search(
        &mut self,
        population: Vec<CandidateImplementation>,
        spec: &FunctionSpec,
    ) -> Result<Vec<CandidateImplementation>, SynthesisError> {
        let mut population = population;

        // Pass 1: a tree rooted at one random population member —
        // the historical behavior. It explores completions of that
        // one body only: selection always descends from tree[0],
        // so no other member's completions are ever harvested.
        let root_body =
            population[engine_rng().gen_range(0..population.len())].body.clone();
        let harvested = self.mcts_harvest_tree(&root_body, spec, "mcts_harvest");
        population.extend(harvested);

        // Pass 2: one tree per test-induced seed, rooted at the
        // seed itself. The seed is the only candidate whose shape
        // is derived from the spec's measured behavior, yet under
        // pass 1 alone it was expanded only when it happened to be
        // the random root (~1 in population_size runs) — its
        // completions (the wildcard hole filled → hole-free bodies
        // passing every test the literal arms reach) never entered
        // the pool, and the holed seed itself is barred from
        // winning by the selector's completeness gate whenever any
        // hole-free candidate exists. Rooting a tree at the seed
        // guarantees its completions are harvested too; whether
        // they WIN is still up to scoring and the gate.
        let seed_bodies: Vec<IRNode> = population
            .iter()
            .filter(|c| matches!(c.provenance, Provenance::TestInduced { .. }))
            .map(|c| c.body.clone())
            .collect();
        for seed_body in &seed_bodies {
            if count_holes(seed_body) == 0 {
                continue; // Already complete — nothing to harvest.
            }
            let harvested = self.mcts_harvest_tree(seed_body, spec, "mcts_seed_harvest");
            population.extend(harvested);
        }

        Ok(population)
    }

    /// Run one MCTS tree rooted at `root_state` for the configured
    /// iteration budget and harvest the terminal nodes the search
    /// actually visited. `mutation_kind` labels the harvested
    /// candidates' provenance so seed-rooted harvests stay
    /// distinguishable from the general one.
    fn mcts_harvest_tree(
        &mut self,
        root_state: &IRNode,
        spec: &FunctionSpec,
        mutation_kind: &str,
    ) -> Vec<CandidateImplementation> {
        let root_holes = count_holes(root_state);
        let mut tree: Vec<MCTSNode> = vec![MCTSNode {
            state: root_state.clone(),
            parent: None,
            children: Vec::new(),
            visits: 0,
            total_reward: 0.0,
            is_terminal: root_holes == 0,
            hole_count: root_holes,
        }];

        for _ in 0..self.config.max_iterations {
            // Selection — the tree is never empty (the root is
            // pushed before the loop), so this always finds a node.
            let idx = self
                .mcts_select(&tree)
                .expect("a rooted MCTS tree is never empty");

            // Expansion
            if !tree[idx].is_terminal && tree[idx].children.is_empty() {
                let children = self.mcts_expand(&tree[idx], spec);
                let parent_idx = idx;
                for child_state in children {
                    let child_idx = tree.len();
                    let hole_count = count_holes(&child_state);
                    tree.push(MCTSNode {
                        state: child_state,
                        parent: Some(parent_idx),
                        children: Vec::new(),
                        visits: 0,
                        total_reward: 0.0,
                        is_terminal: hole_count == 0,
                        hole_count,
                    });
                    tree[parent_idx].children.push(child_idx);
                }
            }

            // Simulation: pick a child and simulate to completion
            let selected_child = if tree[idx].children.is_empty() {
                idx
            } else {
                let child_idx = engine_rng().gen_range(0..tree[idx].children.len());
                tree[idx].children[child_idx]
            };

            // Rollout: fill remaining holes randomly
            let completed = self.mcts_rollout(&tree[selected_child].state, spec);
            let reward = self.mcts_evaluate(&completed, spec);

            // Backpropagation
            self.mcts_backpropagate(&mut tree, selected_child, reward);
        }

        // Harvest: extract best complete implementations from tree
        let mut harvested: Vec<CandidateImplementation> = Vec::new();
        for node in &tree {
            if node.is_terminal && node.visits > 0 {
                let score = node.total_reward / node.visits as f64;
                if score >= self.config.min_score {
                    harvested.push(CandidateImplementation {
                        id: self.new_id(),
                        body: node.state.clone(),
                        spec_name: spec.name.clone(),
                        spec: spec.clone(),
                        score,
                        scores: ScoreBreakdown {
                            test_pass_ratio: score,
                            constraint_score: score,
                            complexity_score: 1.0,
                            quality_score: 0.5,
                            synthesis_time_us: 0,
                        },
                        generation: 1,
                        provenance: Provenance::Mutation {
                            parent_id: 0,
                            mutation_kind: mutation_kind.into(),
                        },
                        verified: false,
                        verification_status: None,
                    });
                }
            }
        }

        harvested
    }

    fn mcts_select(&self, tree: &[MCTSNode]) -> Option<usize> {
        if tree.is_empty() { return None; }
        let total_visits: u64 = tree.iter().map(|n| n.visits).sum();
        let root = tree[0].clone();
        let mut current = 0usize;

        loop {
            if tree[current].is_terminal || tree[current].children.is_empty() {
                return Some(current);
            }

            let mut best_child = None;
            let mut best_uct = -1.0f64;

            for &child_idx in &tree[current].children {
                let uct = tree[child_idx].uct_value(total_visits, self.config.uct_exploration);
                if uct > best_uct {
                    best_uct = uct;
                    best_child = Some(child_idx);
                }
            }

            match best_child {
                Some(idx) => current = idx,
                None => return Some(current),
            }
        }
    }

    fn mcts_expand(&self, node: &MCTSNode, spec: &FunctionSpec) -> Vec<IRNode> {
        let mut children = Vec::new();

        // Find first hole in the tree
        let holes = find_hole_positions(&node.state);
        if holes.is_empty() {
            return children;
        }

        // Generate multiple fillings for the first hole
        let hole = &holes[0];
        let fillings = generate_hole_fillings(hole, node, spec);

        for filling in fillings {
            let mut new_state = node.state.clone();
            replace_hole(&mut new_state, hole, &filling);
            children.push(new_state);
        }

        children
    }

    fn mcts_rollout(&self, node: &IRNode, spec: &FunctionSpec) -> IRNode {
        let mut current = node.clone();
        let max_iterations = 100; // Prevent infinite loops
        let params: Vec<String> = spec.params.iter().map(|p| p.name.clone()).collect();

        for _ in 0..max_iterations {
            let holes = find_hole_positions(&current);
            if holes.is_empty() { break; }

            let hole = &holes[0];
            // Random filling for rollout — variables only from
            // the scope at the hole's position, never invented.
            let scope = scope_at_path(&current, &hole.path, &params);
            let filling = random_filling(hole, &scope);
            replace_hole(&mut current, hole, &filling);
        }

        current
    }

    fn mcts_evaluate(&self, node: &IRNode, spec: &FunctionSpec) -> f64 {
        let candidate = CandidateImplementation {
            id: 0,
            body: node.clone(),
            spec_name: spec.name.clone(),
            spec: spec.clone(),
            score: 0.0,
            scores: empty_scores(),
            generation: 0,
            provenance: Provenance::Template { template_name: "eval".into() },
            verified: false,
            verification_status: None,
        };

        let scores = evaluate_candidate(&candidate, spec);
        // The MCTS reward is the same composite used everywhere
        // else, so the search is steered by measured test
        // behavior with the same dominance invariant.
        composite_score(&scores)
    }

    fn mcts_backpropagate(&self, tree: &mut Vec<MCTSNode>, leaf_idx: usize, reward: f64) {
        let mut current = leaf_idx;
        loop {
            tree[current].visits += 1;
            tree[current].total_reward += reward;
            if let Some(parent) = tree[current].parent {
                current = parent;
            } else {
                break;
            }
        }
    }

    // ── Phase 3: Genetic Refinement ───────────────────────

    fn genetic_refine(
        &mut self,
        population: Vec<CandidateImplementation>,
        _spec: &FunctionSpec,
    ) -> Result<Vec<CandidateImplementation>, SynthesisError> {
        let mut pop: Vec<CandidateImplementation> = population
            .into_iter()
            .map(|c| CandidateImplementation { id: self.new_id(), ..c })
            .collect();
        let mut rng = engine_rng();

        for _gen in 0..10 { // Genetic generations
            // Evaluate all
            for c in &mut pop {
                c.scores = evaluate_candidate(c, &c.spec);
                c.score = composite_score(&c.scores);
            }

            // Sort by fitness
            pop.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

            // Elitism: keep top 10%
            let elite_count = (pop.len() as f64 * 0.1) as usize;
            let mut new_pop: Vec<CandidateImplementation> = pop[..elite_count].to_vec();

            // Fill rest with crossover + mutation
            while new_pop.len() < pop.len() {
                let parent_a = tournament_select(&pop, self.config.tournament_size, &mut rng);
                let parent_b = tournament_select(&pop, self.config.tournament_size, &mut rng);

                let child = if rng.gen::<f64>() < self.config.crossover_rate {
                    self.crossover(parent_a, parent_b)
                } else if rng.gen::<f64>() < self.config.mutation_rate {
                    self.mutate(parent_a)
                } else {
                    parent_a.clone()
                };

                new_pop.push(CandidateImplementation {
                    id: self.new_id(),
                    generation: parent_a.generation.max(parent_b.generation) + 1,
                    ..child
                });
            }

            pop = new_pop;
        }

        Ok(pop)
    }

    fn crossover(&self, parent_a: &CandidateImplementation, parent_b: &CandidateImplementation) -> CandidateImplementation {
        // Semantic crossover: swap subtrees of same type
        let sub_a = random_subtree(&parent_a.body);
        let sub_b = random_subtree(&parent_b.body);

        let mut child_body = parent_a.body.clone();
        if type_compatible(&sub_a, &sub_b) {
            replace_subtree(&mut child_body, &sub_a, &sub_b);
        }

        CandidateImplementation {
            id: 0,
            body: child_body,
            spec_name: parent_a.spec_name.clone(),
            spec: parent_a.spec.clone(),
            score: 0.0,
            scores: empty_scores(),
            generation: 0,
            provenance: Provenance::Crossover {
                parent_a: parent_a.id,
                parent_b: parent_b.id,
            },
            verified: false,
            verification_status: None,
        }
    }

    fn mutate(&self, parent: &CandidateImplementation) -> CandidateImplementation {
        use rand::Rng;
        let mut rng = engine_rng();
        let mut body = parent.body.clone();

        let kind = match rng.gen_range(0u32..6) {
            0 => {
                // Replace a subtree with a hole (un-synthesize)
                let subtree = random_subtree(&body);
                let hole = IRNode::Hole { id: self.new_id(), expected_type: None };
                replace_subtree(&mut body, &subtree, &hole);
                "un_synthesize"
            }
            1 => {
                // Inline a function call
                inline_random_call(&mut body);
                "inline"
            }
            2 => {
                // Swap equivalent expressions
                swap_equivalent(&mut body);
                "swap_equivalent"
            }
            3 => {
                // Tighten loop / optimize
                optimize_loop(&mut body);
                "optimize_loop"
            }
            4 => {
                // Add parallelization hint
                add_parallel_hint(&mut body);
                "parallel"
            }
            _ => {
                // Random AST rewrite
                random_rewrite(&mut body);
                "rewrite"
            }
        };

        CandidateImplementation {
            id: 0,
            body,
            spec_name: parent.spec_name.clone(),
            spec: parent.spec.clone(),
            score: 0.0,
            scores: empty_scores(),
            generation: parent.generation + 1,
            provenance: Provenance::Mutation {
                parent_id: parent.id,
                mutation_kind: kind.into(),
            },
            verified: false,
            verification_status: None,
        }
    }

    // ── Phase 4: Scoring ──────────────────────────────────

    fn score_and_rank(&self, population: &mut Vec<CandidateImplementation>, _spec: &FunctionSpec) {
        for c in population.iter_mut() {
            c.scores = evaluate_candidate(c, &c.spec);
            c.score = composite_score(&c.scores);
        }
        population.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    }

    // ── LLM Integration ───────────────────────────────────

    fn build_llm_prompt(&self, spec: &FunctionSpec) -> String {
        format!(
            r#"You are Morphic, a program synthesis engine.
Given a formal specification, generate Rust code that implements it.

SPECIFICATION for `{}`:
- Input parameters: {}
- Return type: {}
- Preconditions: {}
- Postconditions: {}
- Complexity requirement: {}
- Test cases: {}

Generate a COMPLETE, CORRECT Rust implementation.
The implementation MUST:
1. Satisfy all constraints
2. Meet the complexity bounds
3. Handle all edge cases
4. Be idiomatic and readable

Return ONLY the function body code (the function signature is auto-generated).
Write the code inside <code>...</code> tags."#,
            spec.name,
            format_params(&spec.params),
            spec.return_type,
            format_constraints(&spec.preconditions),
            format_constraints(&spec.postconditions),
            spec.complexity.as_ref().map(|c| format!("{} {}", c.dimension_name(), c.bound)).unwrap_or_else(|| "none".into()),
            format_tests(&spec.tests),
        )
    }

    fn call_llm(&self, prompt: &str) -> Result<String, SynthesisError> {
        // Try llama.cpp first (local), fall back to API
        #[cfg(feature = "llm-local")]
        {
            // Use llama-cpp-2 for local inference
            // let output = llama_cpp_2::generate(prompt, 512, 0.7);
            // return Ok(output);
            return Err(SynthesisError::LlmError("Local LLM not configured".into()));
        }

        #[cfg(feature = "llm-remote")]
        {
            // Use API-based LLM
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| SynthesisError::LlmError(format!("Tokio error: {}", e)))?;
            rt.block_on(async {
                let client = reqwest::Client::new();
                let resp = client
                    .post("http://localhost:11434/api/generate")
                    .json(&serde_json::json!({
                        "model": "codellama:13b",
                        "prompt": prompt,
                        "stream": false,
                        "temperature": 0.7,
                        "max_tokens": 2048,
                    }))
                    .send()
                    .await
                    .map_err(|e| SynthesisError::LlmError(format!("HTTP error: {}", e)))?;

                let json: serde_json::Value = resp.json().await
                    .map_err(|e| SynthesisError::LlmError(format!("JSON error: {}", e)))?;

                Ok(json["response"].as_str().unwrap_or("").to_string())
            })
        }

        #[cfg(not(any(feature = "llm-local", feature = "llm-remote")))]
        {
            Err(SynthesisError::LlmError("No LLM backend enabled".into()))
        }
    }

    fn parse_llm_output(&self, output: &str, spec: &FunctionSpec) -> Result<Vec<CandidateImplementation>, SynthesisError> {
        // Extract code block from LLM output
        let code = if let Some(start) = output.find("<code>") {
            let start = start + 6;
            let end = output.rfind("</code>").unwrap_or(output.len());
            &output[start..end]
        } else if let Some(start) = output.find("```") {
            let start = start + 3;
            let rest = &output[start..];
            let end = rest.find("```").unwrap_or(rest.len());
            &rest[..end]
        } else {
            output
        };

        // Parse Rust code into IR
        parse_rust_into_ir(code, spec)
    }
}

// ── Public API ─────────────────────────────────────────────

/// Main entry point: synthesize implementations for a spec
pub fn synthesize(spec: &FunctionSpec, opt_level: u8) -> Result<Vec<CandidateImplementation>, SynthesisError> {
    let config = SynthesisConfig::from_opt_level(opt_level);
    let mut engine = SynthesisEngine::new(config);
    engine.run(spec)
}

/// Synthesize with the spec's whole module in scope, so
/// constraints that call sibling specs can seed (and be
/// judged) like any other constraint. `specs` is the module's
/// full spec list, including `spec` itself.
pub fn synthesize_with_module(
    spec: &FunctionSpec,
    opt_level: u8,
    specs: &[FunctionSpec],
) -> Result<Vec<CandidateImplementation>, SynthesisError> {
    let config = SynthesisConfig::from_opt_level(opt_level);
    let mut engine = SynthesisEngine::with_module(config, specs);
    engine.run(spec)
}

/// `synthesize_with_module` over a whole parsed module, so
/// the engine's index carries the module's type
/// declarations as well as its specs (Phase 81; see
/// `SynthesisEngine::with_module_spec`).
pub fn synthesize_with_module_spec(
    spec: &FunctionSpec,
    opt_level: u8,
    module: &Spec,
) -> Result<Vec<CandidateImplementation>, SynthesisError> {
    let config = SynthesisConfig::from_opt_level(opt_level);
    let mut engine = SynthesisEngine::with_module_spec(config, module);
    engine.run(spec)
}

/// Re-admit protected seeds that the search phases dropped, so
/// the final pool always contains them (they are re-scored by
/// the caller's scoring phase like every other candidate).
/// Presence is judged by structural body equality: `IRNode`
/// has no `PartialEq`, so the `Debug` representation serves as
/// the fingerprint — it is structural and deterministic, hole
/// ids included, so a surviving clone of a seed matches exactly
/// and is not duplicated.
fn readmit_protected_seeds(
    population: &mut Vec<CandidateImplementation>,
    seeds: &[CandidateImplementation],
) {
    for seed in seeds {
        let fingerprint = format!("{:?}", seed.body);
        if !population.iter().any(|c| format!("{:?}", c.body) == fingerprint) {
            population.push(seed.clone());
        }
    }
}

// ── Error Types ───────────────────────────────────────────

#[derive(Debug)]
pub enum SynthesisError {
    NoImplementationFound,
    VerificationFailed(String),
    LlmError(String),
    TimeoutElapsed,
    InvalidSpec(String),
    InternalError(String),
}

impl std::fmt::Display for SynthesisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SynthesisError::NoImplementationFound => write!(f, "No implementation satisfies all constraints"),
            SynthesisError::VerificationFailed(msg) => write!(f, "Verification failed: {}", msg),
            SynthesisError::LlmError(msg) => write!(f, "LLM error: {}", msg),
            SynthesisError::TimeoutElapsed => write!(f, "Synthesis timeout elapsed"),
            SynthesisError::InvalidSpec(msg) => write!(f, "Invalid specification: {}", msg),
            SynthesisError::InternalError(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for SynthesisError {}

// ── Helper Functions ──────────────────────────────────────

fn empty_scores() -> ScoreBreakdown {
    ScoreBreakdown {
        test_pass_ratio: 0.0,
        constraint_score: 0.0,
        complexity_score: 1.0,
        quality_score: 1.0,
        synthesis_time_us: 0,
    }
}

/// Composite fitness/selection score.
///
/// Measured test behavior dominates by construction: with
/// test_pass_ratio weighted 0.85 and every other dimension 0.05,
/// a tpr gap of 0.25 contributes 0.2125 while the other three
/// dimensions together can swing at most 0.15 (each is bounded
/// by 1.0) — so a ≥0.25 tpr gap ALWAYS decides the ranking, and
/// the remaining dimensions act only as tie-breakers. The old
/// weights (0.4/0.3/0.2/0.1) let the flat placeholder constraint
/// score (0.7 for every hole-free body) plus shape heuristics
/// crown candidates that pass no test at all; the probe measured
/// exactly that (winner tpr 0.000 in 34/42 runs). Every consumer
/// — score_and_rank, the GA, verify_all's re-scoring, and the
/// MCTS reward — goes through this one function so the invariant
/// holds across the whole pipeline.
pub fn composite_score(scores: &ScoreBreakdown) -> f64 {
    0.85 * scores.test_pass_ratio
        + 0.05 * scores.constraint_score
        + 0.05 * scores.complexity_score
        + 0.05 * scores.quality_score
}

pub fn count_holes(node: &IRNode) -> usize {
    match node {
        IRNode::Hole { .. } => 1,
        IRNode::Block(stmts) => stmts.iter().map(count_holes).sum(),
        IRNode::Let { value, body, .. } => count_holes(value) + count_holes(body),
        IRNode::If { condition, then_branch, else_branch } =>
            count_holes(condition) + count_holes(then_branch) + count_holes(else_branch),
        IRNode::While { condition, body, .. } => count_holes(condition) + count_holes(body),
        IRNode::For { start, end, body, .. } => count_holes(start) + count_holes(end) + count_holes(body),
        IRNode::Match { scrutinee, cases } =>
            count_holes(scrutinee) + cases.iter().map(|(_, b)| count_holes(b)).sum::<usize>(),
        IRNode::Call { args, .. } => args.iter().map(count_holes).sum(),
        IRNode::Return(expr) => count_holes(expr),
        IRNode::BinOp(_, l, r) => count_holes(l) + count_holes(r),
        IRNode::UnaryOp(_, inner) => count_holes(inner),
        IRNode::Lambda { body, .. } | IRNode::Closure { body, .. } => count_holes(body),
        IRNode::Collection { elements, .. } => elements.iter().map(count_holes).sum(),
        IRNode::Map { function, over } | IRNode::Filter { predicate: function, over } =>
            count_holes(function) + count_holes(over),
        IRNode::Fold { operation, initial, over } =>
            count_holes(operation) + count_holes(initial) + count_holes(over),
        IRNode::Sort { comparator, collection } => count_holes(comparator) + count_holes(collection),
        IRNode::Alloc { initial, .. } => count_holes(initial),
        IRNode::Assign { target, value } => count_holes(target) + count_holes(value),
        IRNode::Typed { node, .. } => count_holes(node),
        _ => 0, // Literals, Vars, etc.
    }
}

fn find_hole_positions(node: &IRNode) -> Vec<HoleRef> {
    let mut holes = Vec::new();
    find_holes_rec(node, &mut Vec::new(), &mut holes);
    holes
}

type HolePath = Vec<PathElem>;

#[derive(Debug, Clone)]
enum PathElem {
    Block(usize),
    LetValue,
    LetBody,
    IfCond,
    IfThen,
    IfElse,
    WhileCond,
    WhileBody,
    ForStart,
    ForEnd,
    ForBody,
    MatchScrutinee,
    MatchCase(usize),
    CallArg(usize),
    Return,
    BinOpL,
    BinOpR,
    UnaryInner,
    LambdaBody,
    CollectionElem(usize),
    MapFunc,
    MapOver,
    FilterPred,
    FilterOver,
    FoldOp,
    FoldInit,
    FoldOver,
    SortComp,
    SortColl,
    AllocInit,
    AssignTarget,
    AssignValue,
    TypedNode,
}

#[derive(Debug, Clone)]
struct HoleRef {
    path: HolePath,
    /// The type the hole declares for its filling
    /// (`IRNode::Hole::expected_type`), captured when the hole
    /// was found. Holes are only ever located through
    /// `find_hole_positions`, so this is always the type of the
    /// actual hole at `path` in the tree the ref was taken from.
    /// Filling generation reads it to stay type-directed; before
    /// this field existed the fillers never saw the type at all.
    expected_type: Option<TypeRef>,
}

fn find_holes_rec(node: &IRNode, path: &mut HolePath, holes: &mut Vec<HoleRef>) {
    match node {
        IRNode::Hole { expected_type, .. } => {
            holes.push(HoleRef {
                path: path.clone(),
                expected_type: expected_type.clone(),
            });
        }
        IRNode::Block(stmts) => {
            for (i, stmt) in stmts.iter().enumerate() {
                path.push(PathElem::Block(i));
                find_holes_rec(stmt, path, holes);
                path.pop();
            }
        }
        IRNode::Let { value, body, .. } => {
            path.push(PathElem::LetValue);
            find_holes_rec(value, path, holes);
            path.pop();
            path.push(PathElem::LetBody);
            find_holes_rec(body, path, holes);
            path.pop();
        }
        IRNode::If { condition, then_branch, else_branch } => {
            path.push(PathElem::IfCond);
            find_holes_rec(condition, path, holes);
            path.pop();
            path.push(PathElem::IfThen);
            find_holes_rec(then_branch, path, holes);
            path.pop();
            path.push(PathElem::IfElse);
            find_holes_rec(else_branch, path, holes);
            path.pop();
        }
        IRNode::While { condition, body, .. } => {
            path.push(PathElem::WhileCond);
            find_holes_rec(condition, path, holes);
            path.pop();
            path.push(PathElem::WhileBody);
            find_holes_rec(body, path, holes);
            path.pop();
        }
        IRNode::For { start, end, body, .. } => {
            path.push(PathElem::ForStart);
            find_holes_rec(start, path, holes);
            path.pop();
            path.push(PathElem::ForEnd);
            find_holes_rec(end, path, holes);
            path.pop();
            path.push(PathElem::ForBody);
            find_holes_rec(body, path, holes);
            path.pop();
        }
        IRNode::Match { scrutinee, cases } => {
            path.push(PathElem::MatchScrutinee);
            find_holes_rec(scrutinee, path, holes);
            path.pop();
            for (i, (_, body)) in cases.iter().enumerate() {
                path.push(PathElem::MatchCase(i));
                find_holes_rec(body, path, holes);
                path.pop();
            }
        }
        IRNode::Call { args, .. } => {
            for (i, arg) in args.iter().enumerate() {
                path.push(PathElem::CallArg(i));
                find_holes_rec(arg, path, holes);
                path.pop();
            }
        }
        IRNode::Return(expr) => {
            path.push(PathElem::Return);
            find_holes_rec(expr, path, holes);
            path.pop();
        }
        IRNode::BinOp(_, l, r) => {
            path.push(PathElem::BinOpL);
            find_holes_rec(l, path, holes);
            path.pop();
            path.push(PathElem::BinOpR);
            find_holes_rec(r, path, holes);
            path.pop();
        }
        IRNode::UnaryOp(_, inner) => {
            path.push(PathElem::UnaryInner);
            find_holes_rec(inner, path, holes);
            path.pop();
        }
        IRNode::Collection { elements, .. } => {
            for (i, e) in elements.iter().enumerate() {
                path.push(PathElem::CollectionElem(i));
                find_holes_rec(e, path, holes);
                path.pop();
            }
        }
        IRNode::Map { function, over } => {
            path.push(PathElem::MapFunc);
            find_holes_rec(function, path, holes);
            path.pop();
            path.push(PathElem::MapOver);
            find_holes_rec(over, path, holes);
            path.pop();
        }
        IRNode::Filter { predicate, over } => {
            path.push(PathElem::FilterPred);
            find_holes_rec(predicate, path, holes);
            path.pop();
            path.push(PathElem::FilterOver);
            find_holes_rec(over, path, holes);
            path.pop();
        }
        IRNode::Fold { operation, initial, over } => {
            path.push(PathElem::FoldOp);
            find_holes_rec(operation, path, holes);
            path.pop();
            path.push(PathElem::FoldInit);
            find_holes_rec(initial, path, holes);
            path.pop();
            path.push(PathElem::FoldOver);
            find_holes_rec(over, path, holes);
            path.pop();
        }
        IRNode::Sort { comparator, collection } => {
            path.push(PathElem::SortComp);
            find_holes_rec(comparator, path, holes);
            path.pop();
            path.push(PathElem::SortColl);
            find_holes_rec(collection, path, holes);
            path.pop();
        }
        IRNode::Lambda { body, .. } => {
            path.push(PathElem::LambdaBody);
            find_holes_rec(body, path, holes);
            path.pop();
        }
        IRNode::Alloc { initial, .. } => {
            path.push(PathElem::AllocInit);
            find_holes_rec(initial, path, holes);
            path.pop();
        }
        IRNode::Assign { target, value } => {
            path.push(PathElem::AssignTarget);
            find_holes_rec(target, path, holes);
            path.pop();
            path.push(PathElem::AssignValue);
            find_holes_rec(value, path, holes);
            path.pop();
        }
        IRNode::Typed { node, .. } => {
            path.push(PathElem::TypedNode);
            find_holes_rec(node, path, holes);
            path.pop();
        }
        _ => {} // Literals, vars — no children
    }
}

fn replace_hole(node: &mut IRNode, hole: &HoleRef, replacement: &IRNode) {
    replace_at_path(node, &hole.path, 0, replacement);
}

/// Mutable counterpart of `scope_at_path`: descend `path`
/// (produced by `find_hole_positions` on the same tree, so
/// node/step pairs always correspond) and substitute the node
/// at the end of it. Every PathElem variant is handled — the
/// previous version implemented only the Block step and silently
/// left holes anywhere else (match arms, If branches, Let
/// bodies, ...) in place, which made MCTS expansion produce
/// still-holed clones of the parent and rollouts unable to fill
/// such holes at all.
fn replace_at_path(node: &mut IRNode, path: &[PathElem], depth: usize, replacement: &IRNode) {
    if depth >= path.len() {
        *node = replacement.clone();
        return;
    }
    match (node, &path[depth]) {
        (IRNode::Block(stmts), PathElem::Block(i)) => {
            if let Some(next) = stmts.get_mut(*i) {
                replace_at_path(next, path, depth + 1, replacement);
            }
        }
        (IRNode::Let { value, .. }, PathElem::LetValue) => {
            replace_at_path(value, path, depth + 1, replacement)
        }
        (IRNode::Let { body, .. }, PathElem::LetBody) => {
            replace_at_path(body, path, depth + 1, replacement)
        }
        (IRNode::If { condition, .. }, PathElem::IfCond) => {
            replace_at_path(condition, path, depth + 1, replacement)
        }
        (IRNode::If { then_branch, .. }, PathElem::IfThen) => {
            replace_at_path(then_branch, path, depth + 1, replacement)
        }
        (IRNode::If { else_branch, .. }, PathElem::IfElse) => {
            replace_at_path(else_branch, path, depth + 1, replacement)
        }
        (IRNode::While { condition, .. }, PathElem::WhileCond) => {
            replace_at_path(condition, path, depth + 1, replacement)
        }
        (IRNode::While { body, .. }, PathElem::WhileBody) => {
            replace_at_path(body, path, depth + 1, replacement)
        }
        (IRNode::For { start, .. }, PathElem::ForStart) => {
            replace_at_path(start, path, depth + 1, replacement)
        }
        (IRNode::For { end, .. }, PathElem::ForEnd) => {
            replace_at_path(end, path, depth + 1, replacement)
        }
        (IRNode::For { body, .. }, PathElem::ForBody) => {
            replace_at_path(body, path, depth + 1, replacement)
        }
        (IRNode::Match { scrutinee, .. }, PathElem::MatchScrutinee) => {
            replace_at_path(scrutinee, path, depth + 1, replacement)
        }
        (IRNode::Match { cases, .. }, PathElem::MatchCase(i)) => {
            if let Some((_, body)) = cases.get_mut(*i) {
                replace_at_path(body, path, depth + 1, replacement);
            }
        }
        (IRNode::Call { args, .. }, PathElem::CallArg(i)) => {
            if let Some(next) = args.get_mut(*i) {
                replace_at_path(next, path, depth + 1, replacement);
            }
        }
        (IRNode::Return(inner), PathElem::Return) => {
            replace_at_path(inner, path, depth + 1, replacement)
        }
        (IRNode::BinOp(_, l, _), PathElem::BinOpL) => {
            replace_at_path(l, path, depth + 1, replacement)
        }
        (IRNode::BinOp(_, _, r), PathElem::BinOpR) => {
            replace_at_path(r, path, depth + 1, replacement)
        }
        (IRNode::UnaryOp(_, inner), PathElem::UnaryInner) => {
            replace_at_path(inner, path, depth + 1, replacement)
        }
        (IRNode::Lambda { body, .. }, PathElem::LambdaBody) => {
            replace_at_path(body, path, depth + 1, replacement)
        }
        (IRNode::Closure { body, .. }, PathElem::LambdaBody) => {
            replace_at_path(body, path, depth + 1, replacement)
        }
        (IRNode::Collection { elements, .. }, PathElem::CollectionElem(i)) => {
            if let Some(next) = elements.get_mut(*i) {
                replace_at_path(next, path, depth + 1, replacement);
            }
        }
        (IRNode::Map { function, .. }, PathElem::MapFunc) => {
            replace_at_path(function, path, depth + 1, replacement)
        }
        (IRNode::Map { over, .. }, PathElem::MapOver) => {
            replace_at_path(over, path, depth + 1, replacement)
        }
        (IRNode::Filter { predicate, .. }, PathElem::FilterPred) => {
            replace_at_path(predicate, path, depth + 1, replacement)
        }
        (IRNode::Filter { over, .. }, PathElem::FilterOver) => {
            replace_at_path(over, path, depth + 1, replacement)
        }
        (IRNode::Fold { operation, .. }, PathElem::FoldOp) => {
            replace_at_path(operation, path, depth + 1, replacement)
        }
        (IRNode::Fold { initial, .. }, PathElem::FoldInit) => {
            replace_at_path(initial, path, depth + 1, replacement)
        }
        (IRNode::Fold { over, .. }, PathElem::FoldOver) => {
            replace_at_path(over, path, depth + 1, replacement)
        }
        (IRNode::Sort { comparator, .. }, PathElem::SortComp) => {
            replace_at_path(comparator, path, depth + 1, replacement)
        }
        (IRNode::Sort { collection, .. }, PathElem::SortColl) => {
            replace_at_path(collection, path, depth + 1, replacement)
        }
        (IRNode::Alloc { initial, .. }, PathElem::AllocInit) => {
            replace_at_path(initial, path, depth + 1, replacement)
        }
        (IRNode::Assign { target, .. }, PathElem::AssignTarget) => {
            replace_at_path(target, path, depth + 1, replacement)
        }
        (IRNode::Assign { value, .. }, PathElem::AssignValue) => {
            replace_at_path(value, path, depth + 1, replacement)
        }
        (IRNode::Typed { node: inner, .. }, PathElem::TypedNode) => {
            replace_at_path(inner, path, depth + 1, replacement)
        }
        // Path does not match this node — leave it unchanged
        // (defensive; pairs always correspond in practice).
        _ => {}
    }
}

fn replace_subtree(root: &mut IRNode, target: &IRNode, replacement: &IRNode) {
    if std::ptr::eq(root, target) {
        *root = replacement.clone();
        return;
    }
    // Recursive search-and-replace (simplified — real version walks the tree)
    // Uses structural equality as approximation
    if ir_equivalent(root, target) {
        *root = replacement.clone();
    }
}

fn type_compatible(a: &IRNode, b: &IRNode) -> bool {
    // Simplified: any two nodes are compatible for crossover
    true
}

fn random_subtree(node: &IRNode) -> IRNode {
    // Simplified: return a copy of a random subtree
    node.clone()
}

fn ir_equivalent(a: &IRNode, b: &IRNode) -> bool {
    // Simplified structural equivalence (root-variant
    // discriminant only). Used by replace_subtree as its
    // documented approximation — NOT by seed_population's
    // dedup since Phase 59 (see ir_identical).
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// True body identity: the Debug fingerprint, the same
/// identity readmit_protected_seeds uses. seed_population's
/// dedup removes only bodies that are actually identical.
fn ir_identical(a: &IRNode, b: &IRNode) -> bool {
    format!("{:?}", a) == format!("{:?}", b)
}

fn build_input_ref(params: &[Param]) -> IRNode {
    if params.len() == 1 {
        IRNode::Var(params[0].name.clone())
    } else {
        let fields: Vec<IRNode> = params.iter()
            .map(|p| IRNode::Var(p.name.clone()))
            .collect();
        IRNode::Collection {
            kind: CollectionKind::Tuple,
            elements: fields,
        }
    }
}

// ── Template builders ─────────────────────────────────────
//
// Removed in Phase 59 (G2): the three builders that lived
// here (build_divide_and_conquer / build_iterator_chain /
// build_simple_loop) were bare `Hole`s. The real template
// library is super::templates, wired in seed_from_templates.

// ── Mutation helpers ──────────────────────────────────────

/// Literal fillings whose type is exactly `ty` — the menu for a
/// hole that knows its expected type. Returns None for types
/// with no literal menu: named/generic types, functions, maps,
/// sets (codegen emits them unimported), arrays, and Option —
/// the IR has no Option literal node, so there is no honest
/// `None` filling to offer. Callers fall back to the legacy
/// type-blind menu on None.
fn typed_literal_fillings(ty: &TypeRef) -> Option<Vec<IRNode>> {
    fn lit(l: IRLiteral) -> IRNode {
        IRNode::Literal(l)
    }
    Some(match ty {
        TypeRef::Bool => vec![lit(IRLiteral::Bool(true)), lit(IRLiteral::Bool(false))],
        TypeRef::Int
        | TypeRef::I8
        | TypeRef::I16
        | TypeRef::I32
        | TypeRef::I64
        | TypeRef::U8
        | TypeRef::U16
        | TypeRef::U32
        | TypeRef::U64 => vec![lit(IRLiteral::Int(0)), lit(IRLiteral::Int(1))],
        TypeRef::Float => vec![lit(IRLiteral::Float(0.0))],
        TypeRef::String => vec![lit(IRLiteral::String(String::new()))],
        TypeRef::Char => vec![lit(IRLiteral::Char('a'))],
        TypeRef::Unit => vec![lit(IRLiteral::Unit)],
        TypeRef::List(_) => vec![IRNode::Collection {
            kind: CollectionKind::List,
            elements: Vec::new(),
        }],
        TypeRef::Tuple(parts) => {
            let mut elements = Vec::new();
            for part in parts {
                elements.push(typed_literal_fillings(part)?.into_iter().next()?);
            }
            vec![IRNode::Collection {
                kind: CollectionKind::Tuple,
                elements,
            }]
        }
        _ => return None,
    })
}

/// Does this node have type `ty` by construction? Only the
/// shapes `expr_to_ir` can produce for a test's expected output
/// (literals and collections of literals) are recognized;
/// anything else — Vars, Calls, BinOps, Holes — is conservatively
/// rejected, so a value is only admitted to a typed menu when
/// its type is certain.
fn node_matches_type(node: &IRNode, ty: &TypeRef) -> bool {
    match (node, ty) {
        (IRNode::Literal(IRLiteral::Bool(_)), TypeRef::Bool) => true,
        (
            IRNode::Literal(IRLiteral::Int(_)),
            TypeRef::Int
            | TypeRef::I8
            | TypeRef::I16
            | TypeRef::I32
            | TypeRef::I64
            | TypeRef::U8
            | TypeRef::U16
            | TypeRef::U32
            | TypeRef::U64,
        ) => true,
        (IRNode::Literal(IRLiteral::Float(_)), TypeRef::Float) => true,
        (IRNode::Literal(IRLiteral::String(_)), TypeRef::String) => true,
        (IRNode::Literal(IRLiteral::Char(_)), TypeRef::Char) => true,
        (IRNode::Literal(IRLiteral::Unit), TypeRef::Unit) => true,
        (IRNode::Collection { kind: CollectionKind::List, elements }, TypeRef::List(elem)) => {
            elements.iter().all(|e| node_matches_type(e, elem))
        }
        (IRNode::Collection { kind: CollectionKind::Tuple, elements }, TypeRef::Tuple(parts)) => {
            elements.len() == parts.len()
                && elements
                    .iter()
                    .zip(parts.iter())
                    .all(|(e, t)| node_matches_type(e, t))
        }
        _ => false,
    }
}

fn generate_hole_fillings(hole: &HoleRef, node: &MCTSNode, spec: &FunctionSpec) -> Vec<IRNode> {
    // Type-directed menu: when the hole knows its expected type
    // (the TestInduced seed's wildcard hole carries the spec's
    // return type), every filling must be an expression of that
    // type. The legacy menu below is type-blind — it always led
    // with Int(0) — so seed completions put `_ => 0` in Bool and
    // String functions: honest tpr 1.0 (the arm is unreachable
    // by the tests) but rejected by rustc (E0308).
    if let Some(ty) = &hole.expected_type {
        if let Some(mut fillings) = typed_literal_fillings(ty) {
            // A test's expected output is return-typed by
            // construction, so the distinct outputs belong in
            // the menu too (e.g. the real Int/String values the
            // spec already names).
            for test in &spec.tests {
                let output = expr_to_ir(&test.expected_output);
                if node_matches_type(&output, ty)
                    && !fillings
                        .iter()
                        .any(|f| format!("{:?}", f) == format!("{:?}", output))
                {
                    fillings.push(output);
                }
            }
            return fillings;
        }
    }
    // Generate diverse fillings for the hole. Every variable a
    // filling references must be in scope at the hole's position:
    // the previous version hardcoded Var("x") and `a + b` — names
    // bound nowhere — so expanded children could never evaluate
    // or compile, whatever the search did with them.
    // In production: use LLM, type-directed search, library functions
    let params: Vec<String> = spec.params.iter().map(|p| p.name.clone()).collect();
    let scope = scope_at_path(&node.state, &hole.path, &params);
    vec![
        IRNode::Literal(IRLiteral::Int(0)),
        IRNode::Literal(IRLiteral::Bool(true)),
        random_scoped_leaf(&scope),
        IRNode::BinOp(IRBinOp::Add,
            Box::new(random_scoped_leaf(&scope)),
            Box::new(random_scoped_leaf(&scope))),
    ]
}

/// Collect the names a match pattern binds (variable patterns,
/// recursively through constructors and guards).
fn pattern_bindings(pattern: &IRPattern, out: &mut Vec<String>) {
    match pattern {
        IRPattern::Variable(name) => out.push(name.clone()),
        IRPattern::Constructor(_, subs) => {
            for sub in subs {
                pattern_bindings(sub, out);
            }
        }
        IRPattern::Guard(inner, _) => pattern_bindings(inner, out),
        IRPattern::Wildcard | IRPattern::Literal(_) => {}
    }
}

/// The variable names in scope at position `path` inside `root`:
/// the spec's parameter names, plus every binder enclosing the
/// position (a `Let` name in its body, a `For` variable in its
/// body, lambda/closure parameters, match-arm pattern bindings).
/// Paths come from `find_hole_positions` on the same tree, so
/// node/step pairs always correspond; on any mismatch the walk
/// stops defensively with the scope accumulated so far.
fn scope_at_path(root: &IRNode, path: &[PathElem], params: &[String]) -> Vec<String> {
    let mut scope: Vec<String> = params.to_vec();
    let mut current = root;
    for elem in path {
        match (current, elem) {
            (IRNode::Block(stmts), PathElem::Block(i)) => match stmts.get(*i) {
                Some(next) => current = next,
                None => break,
            },
            (IRNode::Let { value, .. }, PathElem::LetValue) => current = value,
            (IRNode::Let { name, body, .. }, PathElem::LetBody) => {
                scope.push(name.clone());
                current = body;
            }
            (IRNode::If { condition, .. }, PathElem::IfCond) => current = condition,
            (IRNode::If { then_branch, .. }, PathElem::IfThen) => current = then_branch,
            (IRNode::If { else_branch, .. }, PathElem::IfElse) => current = else_branch,
            (IRNode::While { condition, .. }, PathElem::WhileCond) => current = condition,
            (IRNode::While { body, .. }, PathElem::WhileBody) => current = body,
            (IRNode::For { start, .. }, PathElem::ForStart) => current = start,
            (IRNode::For { end, .. }, PathElem::ForEnd) => current = end,
            (IRNode::For { var, body, .. }, PathElem::ForBody) => {
                scope.push(var.clone());
                current = body;
            }
            (IRNode::Match { scrutinee, .. }, PathElem::MatchScrutinee) => current = scrutinee,
            (IRNode::Match { cases, .. }, PathElem::MatchCase(i)) => match cases.get(*i) {
                Some((pattern, body)) => {
                    pattern_bindings(pattern, &mut scope);
                    current = body;
                }
                None => break,
            },
            (IRNode::Call { args, .. }, PathElem::CallArg(i)) => match args.get(*i) {
                Some(next) => current = next,
                None => break,
            },
            (IRNode::Return(inner), PathElem::Return) => current = inner,
            (IRNode::BinOp(_, l, _), PathElem::BinOpL) => current = l,
            (IRNode::BinOp(_, _, r), PathElem::BinOpR) => current = r,
            (IRNode::UnaryOp(_, inner), PathElem::UnaryInner) => current = inner,
            (IRNode::Lambda { params, body }, PathElem::LambdaBody) => {
                scope.extend(params.iter().map(|(n, _)| n.clone()));
                current = body;
            }
            (IRNode::Closure { params, body, .. }, PathElem::LambdaBody) => {
                scope.extend(params.iter().map(|(n, _)| n.clone()));
                current = body;
            }
            (IRNode::Collection { elements, .. }, PathElem::CollectionElem(i)) => {
                match elements.get(*i) {
                    Some(next) => current = next,
                    None => break,
                }
            }
            (IRNode::Map { function, .. }, PathElem::MapFunc) => current = function,
            (IRNode::Map { over, .. }, PathElem::MapOver) => current = over,
            (IRNode::Filter { predicate, .. }, PathElem::FilterPred) => current = predicate,
            (IRNode::Filter { over, .. }, PathElem::FilterOver) => current = over,
            (IRNode::Fold { operation, .. }, PathElem::FoldOp) => current = operation,
            (IRNode::Fold { initial, .. }, PathElem::FoldInit) => current = initial,
            (IRNode::Fold { over, .. }, PathElem::FoldOver) => current = over,
            (IRNode::Sort { comparator, .. }, PathElem::SortComp) => current = comparator,
            (IRNode::Sort { collection, .. }, PathElem::SortColl) => current = collection,
            (IRNode::Alloc { initial, .. }, PathElem::AllocInit) => current = initial,
            (IRNode::Assign { target, .. }, PathElem::AssignTarget) => current = target,
            (IRNode::Assign { value, .. }, PathElem::AssignValue) => current = value,
            (IRNode::Typed { node, .. }, PathElem::TypedNode) => current = node,
            // Path does not match this node — stop with what we have.
            _ => break,
        }
    }
    scope
}

/// A random leaf that is valid wherever it lands: a variable
/// drawn from `scope` when the scope is non-empty, otherwise a
/// literal. Generation must never invent a name — a `Var` that
/// nothing binds can neither evaluate nor compile.
fn random_scoped_leaf(scope: &[String]) -> IRNode {
    use rand::Rng;
    if scope.is_empty() {
        IRNode::Literal(IRLiteral::Int(0))
    } else {
        let idx = engine_rng().gen_range(0..scope.len());
        IRNode::Var(scope[idx].clone())
    }
}

fn random_filling(hole: &HoleRef, scope: &[String]) -> IRNode {
    use rand::Rng;
    // Type-directed draw: a hole that knows its expected type is
    // filled from the typed literal menu — never with a
    // wrong-typed literal, a Return wrapper, or a fresh Hole
    // (the legacy arms below, kept exactly for untyped holes).
    if let Some(ty) = &hole.expected_type {
        if let Some(fillings) = typed_literal_fillings(ty) {
            let idx = engine_rng().gen_range(0..fillings.len());
            return fillings.into_iter().nth(idx).unwrap();
        }
    }
    match engine_rng().gen_range(0u32..5) {
        0 => IRNode::Literal(IRLiteral::Int(0)),
        1 => IRNode::Literal(IRLiteral::Bool(true)),
        2 => random_scoped_leaf(scope),
        3 => IRNode::Return(Box::new(random_scoped_leaf(scope))),
        _ => IRNode::Hole { id: 0, expected_type: None },
    }
}

fn random_ir_tree(spec: &FunctionSpec, depth: u32, engine: &SynthesisEngine) -> IRNode {
    // Generation starts in the scope of the spec's parameters;
    // binders the generator itself creates extend it on the way
    // down (see random_ir_tree_scoped).
    let scope: Vec<String> = spec.params.iter().map(|p| p.name.clone()).collect();
    random_ir_tree_scoped(spec, depth, engine, &scope)
}

fn random_ir_tree_scoped(
    spec: &FunctionSpec,
    depth: u32,
    engine: &SynthesisEngine,
    scope: &[String],
) -> IRNode {
    if depth == 0 {
        return IRNode::Hole { id: engine.new_id(), expected_type: None };
    }
    use rand::Rng;
    match engine_rng().gen_range(0u32..8) {
        0 => IRNode::Block(vec![
            random_ir_tree_scoped(spec, depth - 1, engine, scope),
            // The trailing value used to be a hardcoded
            // Var("result") — a name bound nowhere, so every tree
            // containing this arm was unevaluable dead weight.
            IRNode::Return(Box::new(random_scoped_leaf(scope))),
        ]),
        1 => IRNode::If {
            condition: Box::new(random_ir_tree_scoped(spec, depth - 1, engine, scope)),
            then_branch: Box::new(random_ir_tree_scoped(spec, depth - 1, engine, scope)),
            else_branch: Box::new(random_ir_tree_scoped(spec, depth - 1, engine, scope)),
        },
        2 => {
            // The loop variable is in scope in the body only.
            let mut body_scope = scope.to_vec();
            body_scope.push("i".into());
            IRNode::For {
                var: "i".into(),
                start: Box::new(IRNode::Literal(IRLiteral::Int(0))),
                end: Box::new(IRNode::Literal(IRLiteral::Int(10))),
                body: Box::new(random_ir_tree_scoped(spec, depth - 1, engine, &body_scope)),
            }
        }
        // An unfilled leaf is an honest Hole (tracked by count_holes
        // and penalized at evaluation) — never a fake `Call` to "_",
        // which codegen would emit as a call to a nonexistent function.
        3 => IRNode::Hole { id: engine.new_id(), expected_type: None },
        4 => IRNode::Return(Box::new(random_ir_tree_scoped(spec, depth - 1, engine, scope))),
        5 => IRNode::Collection { kind: CollectionKind::List, elements: vec![] },
        _ => IRNode::Hole { id: engine.new_id(), expected_type: None },
    }
}

// ── Type-directed random generation ─────────────────────
//
// The typed counterpart of random_ir_tree, used by seed_random
// (the one call site that creates fresh candidate bodies). The
// legacy generator above is type-blind: measured over the M2
// signatures, 31.4% of its seed bodies were PROVABLY mistyped
// for their spec and only 1.3% provably matching (see the
// property test). Those seeds are not a cosmetic problem —
// MCTS only fills holes, so completions inherit the seed's
// root shape, and the selector's return-type gate (Phase 11)
// can only crown what the pool contains: in roughly half of
// draws a test-less spec's hole-free pool was uniformly
// mistyped and the gate's documented fallback crowned a
// mistyped winner anyway.
//
// This generator threads the spec's declared return type
// through generation, reusing the verdict's own certainty
// rules (selector::certainly_equal / int_family — no second
// inference is written here). The discipline:
//   * the ROOT shape is generated in the return type's
//     context, from shapes the conservative inference can
//     type as that type (Matches) or cannot type at all
//     (Unknown, neutral) — never one it can prove wrong;
//   * a statement-only tree (For/While root, a Block whose
//     tail is a statement) is never the entire body of a
//     non-Unit function — loops appear only as statements
//     inside a Block that ends in a typed tail;
//   * every Return the generator emits wraps an expression
//     generated in the return type's context (Returns are
//     produced only by the tail generator, whose target is
//     always the return type; expression/statement positions
//     never produce one).
//
// The shape menu is additionally restricted to what the Rust
// codegen can actually render — a stricter filter than the
// verdict's: no Call (emitted as a call to a nonexistent free
// function — `len(x)` is E0425), no Index/Head/Tail/Init
// (unsupported in the expression emitter), no Reverse (emits
// `.reverse()`, which returns ()), no Concat (emitted as
// String + String, which Rust rejects), no Len unary (emits
// `.len()` → usize, but the signature's Int is i64). Those
// shapes remain available to the legacy generator and to
// hole fillings; fresh bodies are simply not built from them.

/// A name in scope together with its declared type — the
/// generation-time mirror of the verdict's type environment.
type TypedScope = Vec<(String, TypeRef)>;

/// Generate a fresh candidate body for `spec` in the context
/// of its declared return type. Counterpart of random_ir_tree
/// for callers that HAVE a type context (seed_random);
/// random_ir_tree itself is unchanged for callers without one.
fn random_ir_tree_typed(spec: &FunctionSpec, depth: u32, engine: &SynthesisEngine) -> IRNode {
    let scope: TypedScope = spec
        .params
        .iter()
        .map(|p| (p.name.clone(), p.ty.clone()))
        .collect();
    gen_typed_tail(&spec.return_type, depth, engine, &scope)
}

/// Can a collection literal's elements be generated for this
/// element type without risking a render/type mismatch?
/// Literal elements of these types are inferred by rustc in
/// exactly the position the verdict claims for them; String
/// elements would render as `&str` inside a `vec![…]` (the
/// typed-literal rewrite only fires for a bare literal node),
/// so non-empty collections are generated only for these.
fn literal_safe_element(ty: &TypeRef) -> bool {
    super::selector::int_family(ty) || matches!(ty, TypeRef::Bool | TypeRef::Char)
}

/// Generate an expression of target type `ty` for an
/// EXPRESSION position (operand, condition, scrutinee, Let
/// value, Return value, collection element): shapes the
/// expression emitter can render and the verdict can type as
/// `ty` or not at all. For the types with a literal menu
/// (Bool, the integer family, Float, String, Char, Unit,
/// List, Tuple of menufull parts) every shape emitted here is
/// in fact certainly `ty` under the verdict's rules — the
/// menu, Var and operator rules below are exactly the shapes
/// infer_ir_type can type. For types with no menu (Named,
/// Option, Map, …) the only honest leaf is a typed Hole
/// (Unknown to the verdict), which hole filling can later
/// complete with the Phase 10 typed menu.
fn gen_typed_expr(
    ty: &TypeRef,
    depth: u32,
    engine: &SynthesisEngine,
    scope: &TypedScope,
) -> IRNode {
    use rand::Rng;
    let mut rng = engine_rng();

    // Applicable shapes, as option codes. Composite shapes are
    // depth-gated; leaves are always available.
    let mut options: Vec<u8> = Vec::new();
    let literals = typed_literal_fillings(ty);
    if literals.is_some() {
        options.push(0); // literal of exactly this type
    }
    let matching_vars: Vec<&str> = scope
        .iter()
        .filter(|(_, vty)| super::selector::certainly_equal(vty, ty))
        .map(|(name, _)| name.as_str())
        .collect();
    if !matching_vars.is_empty() {
        options.push(1); // a variable of exactly this type
    }
    let binop_ok = matches!(ty, TypeRef::Bool | TypeRef::Float)
        || super::selector::int_family(ty);
    let unop_ok = matches!(ty, TypeRef::Bool) || super::selector::int_family(ty);
    let collection_ok = match ty {
        TypeRef::List(elem) => literal_safe_element(elem),
        TypeRef::Tuple(parts) => parts.iter().all(literal_safe_element),
        _ => false,
    };
    if depth > 0 {
        if binop_ok {
            options.push(2);
        }
        if unop_ok {
            options.push(3);
        }
        if collection_ok {
            options.push(4);
        }
    }
    if options.is_empty() {
        // No honest shape for this type in expression
        // position: an explicitly typed Hole (Unknown), never
        // a wrong-typed guess.
        return IRNode::Hole {
            id: engine.new_id(),
            expected_type: Some(ty.clone()),
        };
    }

    match options[rng.gen_range(0..options.len())] {
        0 => {
            let menu = literals.unwrap();
            let idx = rng.gen_range(0..menu.len());
            menu.into_iter().nth(idx).unwrap()
        }
        1 => {
            let idx = rng.gen_range(0..matching_vars.len());
            IRNode::Var(matching_vars[idx].to_string())
        }
        2 => gen_typed_binop(ty, depth, engine, scope),
        3 => {
            // Unary operators whose result the verdict types as
            // the operand's family: Not on Bool, Neg/Abs on
            // integers. (Len/Head/Tail/Reverse are excluded —
            // see the section header for the codegen reasons.)
            if *ty == TypeRef::Bool {
                IRNode::UnaryOp(
                    IRUnaryOp::Not,
                    Box::new(gen_typed_expr(&TypeRef::Bool, depth - 1, engine, scope)),
                )
            } else {
                let inner = gen_typed_expr(&TypeRef::Int, depth - 1, engine, scope);
                let op = if rng.gen_bool(0.5) {
                    IRUnaryOp::Neg
                } else {
                    IRUnaryOp::Abs
                };
                IRNode::UnaryOp(op, Box::new(inner))
            }
        }
        _ => {
            // Non-empty collection of certainly-typed elements.
            match ty {
                TypeRef::List(elem) => {
                    let n = rng.gen_range(0..=3usize);
                    let elements = (0..n)
                        .map(|_| gen_typed_expr(elem, depth - 1, engine, scope))
                        .collect();
                    IRNode::Collection {
                        kind: CollectionKind::List,
                        elements,
                    }
                }
                TypeRef::Tuple(parts) => {
                    let elements = parts
                        .iter()
                        .map(|p| gen_typed_expr(p, depth - 1, engine, scope))
                        .collect();
                    IRNode::Collection {
                        kind: CollectionKind::Tuple,
                        elements,
                    }
                }
                _ => unreachable!("collection option gated on List/Tuple"),
            }
        }
    }
}

/// A binary operation whose result the verdict types as `ty`:
/// comparisons and logical ops for Bool (operands of one
/// certain type — Int or Bool, the operand types whose
/// literals and variables render and compare soundly in
/// Rust), arithmetic for the integer family and Float.
/// Div/Mod never take a constant-zero divisor (rustc's
/// unconditional_panic lint rejects `x / 0` at compile time).
fn gen_typed_binop(
    ty: &TypeRef,
    depth: u32,
    engine: &SynthesisEngine,
    scope: &TypedScope,
) -> IRNode {
    use rand::Rng;
    let mut rng = engine_rng();
    let bin = |op: IRBinOp, l: IRNode, r: IRNode| {
        IRNode::BinOp(op, Box::new(l), Box::new(r))
    };
    // A divisor that cannot be a compile-time zero: a nonzero
    // literal, or a variable (its value is not a constant).
    let divisor = |rng: &mut SharedRng, scope: &TypedScope| -> IRNode {
        let int_vars: Vec<&str> = scope
            .iter()
            .filter(|(_, vty)| super::selector::int_family(vty))
            .map(|(name, _)| name.as_str())
            .collect();
        if !int_vars.is_empty() && rng.gen_bool(0.5) {
            IRNode::Var(int_vars[rng.gen_range(0..int_vars.len())].to_string())
        } else {
            IRNode::Literal(IRLiteral::Int(if rng.gen_bool(0.5) { 1 } else { 2 }))
        }
    };

    if *ty == TypeRef::Bool {
        if rng.gen_bool(0.5) {
            // Logical: both operands Bool.
            let op = if rng.gen_bool(0.5) { IRBinOp::And } else { IRBinOp::Or };
            let l = gen_typed_expr(&TypeRef::Bool, depth - 1, engine, scope);
            let r = gen_typed_expr(&TypeRef::Bool, depth - 1, engine, scope);
            bin(op, l, r)
        } else {
            // Comparison: both operands of one certain type.
            let ot = if rng.gen_bool(0.5) {
                TypeRef::Int
            } else {
                TypeRef::Bool
            };
            let op = [
                IRBinOp::Eq,
                IRBinOp::Neq,
                IRBinOp::Lt,
                IRBinOp::Le,
                IRBinOp::Gt,
                IRBinOp::Ge,
            ][rng.gen_range(0..6)];
            let l = gen_typed_expr(&ot, depth - 1, engine, scope);
            let r = gen_typed_expr(&ot, depth - 1, engine, scope);
            bin(op, l, r)
        }
    } else if *ty == TypeRef::Float {
        let op = [IRBinOp::Add, IRBinOp::Sub, IRBinOp::Mul][rng.gen_range(0..3)];
        let l = gen_typed_expr(&TypeRef::Float, depth - 1, engine, scope);
        let r = gen_typed_expr(&TypeRef::Float, depth - 1, engine, scope);
        bin(op, l, r)
    } else {
        // Integer family: arithmetic on Int operands (the
        // family unifies under the verdict's rules).
        let op = [
            IRBinOp::Add,
            IRBinOp::Sub,
            IRBinOp::Mul,
            IRBinOp::Div,
            IRBinOp::Mod,
        ][rng.gen_range(0..5)];
        let l = gen_typed_expr(&TypeRef::Int, depth - 1, engine, scope);
        let r = if matches!(op, IRBinOp::Div | IRBinOp::Mod) {
            divisor(&mut rng, scope)
        } else {
            gen_typed_expr(&TypeRef::Int, depth - 1, engine, scope)
        };
        bin(op, l, r)
    }
}

/// Generate a body for a TAIL position (function body, Block
/// tail, If branch, Match arm, Let body) whose value must be
/// of type `ty`. In this generator `ty` is always the spec's
/// return type — the only place Returns are emitted, each
/// wrapping an expression generated for that same type.
/// Everything gen_typed_expr produces is also a valid tail;
/// the structural shapes (If/Match/Let/Block/Return) exist
/// only here, because the expression emitter cannot render
/// them in operand positions.
fn gen_typed_tail(
    ty: &TypeRef,
    depth: u32,
    engine: &SynthesisEngine,
    scope: &TypedScope,
) -> IRNode {
    use rand::Rng;
    let mut rng = engine_rng();
    if depth == 0 {
        return gen_typed_expr(ty, 0, engine, scope);
    }
    // Scrutinee candidates for a Match: in-scope Int/Bool
    // variables (literal patterns of those types render and
    // match soundly; a String scrutinee would need &str
    // patterns the emitter does not produce).
    let scrutinees: Vec<&(String, TypeRef)> = scope
        .iter()
        .filter(|(_, vty)| super::selector::int_family(vty) || *vty == TypeRef::Bool)
        .collect();

    let mut options: Vec<u8> = vec![0]; // plain expression tail
    options.push(1); // If with typed branches
    if !scrutinees.is_empty() {
        options.push(2); // Match on an in-scope variable
    }
    options.push(3); // Let with a simply-typed value
    options.push(4); // Block: statements, then a typed tail
    options.push(5); // Return of a typed expression

    match options[rng.gen_range(0..options.len())] {
        0 => gen_typed_expr(ty, depth, engine, scope),
        1 => IRNode::If {
            condition: Box::new(gen_typed_expr(&TypeRef::Bool, depth - 1, engine, scope)),
            then_branch: Box::new(gen_typed_tail(ty, depth - 1, engine, scope)),
            else_branch: Box::new(gen_typed_tail(ty, depth - 1, engine, scope)),
        },
        2 => {
            let (name, sty) = scrutinees[rng.gen_range(0..scrutinees.len())];
            let mut cases: Vec<(IRPattern, IRNode)> = Vec::new();
            if *sty == TypeRef::Bool {
                cases.push((
                    IRPattern::Literal(IRLiteral::Bool(true)),
                    gen_typed_tail(ty, depth - 1, engine, scope),
                ));
            } else {
                let first = rng.gen_range(0..3i64);
                cases.push((
                    IRPattern::Literal(IRLiteral::Int(first)),
                    gen_typed_tail(ty, depth - 1, engine, scope),
                ));
                if rng.gen_bool(0.5) {
                    cases.push((
                        IRPattern::Literal(IRLiteral::Int(first + 1)),
                        gen_typed_tail(ty, depth - 1, engine, scope),
                    ));
                }
            }
            cases.push((
                IRPattern::Wildcard,
                gen_typed_tail(ty, depth - 1, engine, scope),
            ));
            IRNode::Match {
                scrutinee: Box::new(IRNode::Var(name.clone())),
                cases,
            }
        }
        3 => {
            // The value's type is one the emitter types
            // exactly (an Int/Bool/Char literal or expression
            // binds at that type in Rust as in the verdict);
            // String values are excluded — a `let` value
            // renders untyped, so a String literal would bind
            // as &str while the verdict binds String.
            let vt = [TypeRef::Int, TypeRef::Bool, TypeRef::Char]
                [rng.gen_range(0..3)]
            .clone();
            let value = gen_typed_expr(&vt, depth - 1, engine, scope);
            let name = format!("v{}", engine.new_id());
            let mut body_scope = scope.clone();
            body_scope.push((name.clone(), vt));
            IRNode::Let {
                name,
                value: Box::new(value),
                body: Box::new(gen_typed_tail(ty, depth - 1, engine, &body_scope)),
            }
        }
        4 => {
            let n = rng.gen_range(1..=3usize);
            let mut stmts: Vec<IRNode> = (0..n)
                .map(|_| gen_typed_stmt(depth - 1, engine, scope))
                .collect();
            stmts.push(gen_typed_tail(ty, depth - 1, engine, scope));
            IRNode::Block(stmts)
        }
        _ => IRNode::Return(Box::new(gen_typed_expr(ty, depth - 1, engine, scope))),
    }
}

/// An expression used as a statement: its value is
/// discarded, so it is generated for a simple type (Int or
/// Bool) whose shapes are always available and render as a
/// terminated expression statement.
fn gen_typed_expr_stmt(depth: u32, engine: &SynthesisEngine, scope: &TypedScope) -> IRNode {
    use rand::Rng;
    let ty = if engine_rng().gen_bool(0.5) {
        TypeRef::Int
    } else {
        TypeRef::Bool
    };
    gen_typed_expr(&ty, depth, engine, scope)
}

/// Generate a STATEMENT: a node whose value is discarded.
/// Statements never contain a Return (a mistyped or even
/// well-typed stray Return in statement position would hijack
/// the body's value flow and its verdict); loops live here,
/// as statements inside a Block — never as a whole body.
fn gen_typed_stmt(depth: u32, engine: &SynthesisEngine, scope: &TypedScope) -> IRNode {
    use rand::Rng;
    let mut rng = engine_rng();
    if depth == 0 {
        return gen_typed_expr_stmt(depth, engine, scope);
    }
    match rng.gen_range(0..4u32) {
        0 => gen_typed_expr_stmt(depth, engine, scope),
        1 => {
            // For over a literal range; the loop variable is
            // Int-typed inside the body, as the verdict's
            // range rule also concludes.
            let var = format!("i{}", engine.new_id());
            let int_vars: Vec<&str> = scope
                .iter()
                .filter(|(_, vty)| super::selector::int_family(vty))
                .map(|(name, _)| name.as_str())
                .collect();
            let end = if !int_vars.is_empty() && rng.gen_bool(0.5) {
                IRNode::Var(int_vars[rng.gen_range(0..int_vars.len())].to_string())
            } else {
                IRNode::Literal(IRLiteral::Int(10))
            };
            let mut body_scope = scope.clone();
            body_scope.push((var.clone(), TypeRef::Int));
            IRNode::For {
                var,
                start: Box::new(IRNode::Literal(IRLiteral::Int(0))),
                end: Box::new(end),
                body: Box::new(gen_typed_stmt(depth - 1, engine, &body_scope)),
            }
        }
        2 => IRNode::While {
            condition: Box::new(gen_typed_expr(&TypeRef::Bool, depth - 1, engine, scope)),
            body: Box::new(gen_typed_stmt(depth - 1, engine, scope)),
            invariant: None,
        },
        _ => {
            let n = rng.gen_range(1..=2usize);
            let stmts: Vec<IRNode> = (0..n)
                .map(|_| gen_typed_stmt(depth - 1, engine, scope))
                .collect();
            IRNode::Block(stmts)
        }
    }
}

fn parse_rust_into_ir(code: &str, _spec: &FunctionSpec) -> Result<Vec<CandidateImplementation>, SynthesisError> {
    // Simplified: wrap raw code in an opaque IR node
    // In production: use a proper Rust parser (syn) to build IR
    Ok(vec![CandidateImplementation {
        id: 0,
        body: IRNode::Typed {
            node: Box::new(IRNode::Block(vec![])),
            ty: TypeRef::Named("RawRust".into()),
        },
        spec_name: "".into(),
        spec: _spec.clone(),
        score: 0.0,
        scores: empty_scores(),
        generation: 0,
        provenance: Provenance::LLMGenerated {
            model: "codellama:13b".into(),
            prompt_hash: 0,
        },
        verified: false,
        verification_status: None,
    }])
}

// ── Constraint-derived seed bodies ────────────────────────
//
// The builders below read the spec's postconditions (the
// parser stores every `constraint:` line there) and produce
// a candidate body from the declared contract alone. All are
// total and conservative: any shape they do not recognize
// yields None, never a guessed body.

/// The variable postconditions state the result with. The
/// verifier binds the candidate's symbolic value under exactly
/// this name (see verify::verifier::verify_constraint), and
/// every spec in the tree writes it so.
const OUTPUT_VAR: &str = "output";

/// One postcondition read as an equation body: if it has the
/// form `output == <expr>` (either orientation), <expr> does
/// not mention `output` itself, and the conversion via the
/// same `expr_to_ir` the test-induced seed uses is whole
/// (`expr_to_ir` drops subexpressions it cannot represent into
/// Holes, and a holed body is not the spec's body), the
/// converted body; otherwise None. Shared by the equation seed
/// builder and by the resolver (`SpecIndex::callee_body`), so
/// both read the same "first equation" of a spec.
fn equation_rhs(c: &Constraint) -> Option<IRNode> {
    let (lhs, rhs) = match c {
        Constraint::Expr(Expr::BinOp(BinOp::Eq, l, r)) => (l.as_ref(), r.as_ref()),
        Constraint::Eq(l, r) => (l, r),
        _ => return None,
    };
    let body_expr = if matches!(lhs, Expr::Var(v) if v == OUTPUT_VAR) {
        rhs
    } else if matches!(rhs, Expr::Var(v) if v == OUTPUT_VAR) {
        lhs
    } else {
        return None;
    };
    if expr_mentions_var(body_expr, OUTPUT_VAR) {
        return None;
    }
    let body = expr_to_ir(body_expr);
    if count_holes(&body) > 0 {
        return None;
    }
    Some(body)
}

/// The first convertible equation body of a spec, if any —
/// the resolver's view of "the spec's body" when the spec is
/// stated as an equation.
fn first_equation_body(spec: &FunctionSpec) -> Option<IRNode> {
    spec.postconditions.iter().find_map(equation_rhs)
}

/// The equation seed's body: the first equation whose body —
/// with any spec-to-spec calls resolved against the module
/// (`SpecIndex::inline_calls`) — the verifier can encode.
/// Resolution is what admits the composition specs: their
/// equations are calls (`output == is_known_node_kind(kind)`),
/// and the admitted seed body is the resolved callee body, so
/// the interpreter can measure it, selection can crown it, and
/// codegen can emit it standalone. An equation that does not
/// resolve (unknown callee, cycle, depth) or does not encode
/// is skipped like any other unusable shape.
fn equation_seed_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    for c in &spec.postconditions {
        let Some(raw) = equation_rhs(c) else { continue };
        let Some(body) = index.inline_calls(&raw) else { continue };
        if crate::verify::verifier::body_is_encodable_with(spec, &body, index) {
            return Some(body);
        }
    }
    None
}

fn expr_mentions_var(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Var(v) => v == name,
        Expr::IntLit(_) | Expr::FloatLit(_) | Expr::BoolLit(_) | Expr::StringLit(_)
        | Expr::CharLit(_) => false,
        Expr::Field(inner, _) => expr_mentions_var(inner, name),
        Expr::Index(coll, idx) => {
            expr_mentions_var(coll, name) || expr_mentions_var(idx, name)
        }
        Expr::Call(_, args) => args.iter().any(|a| expr_mentions_var(a, name)),
        Expr::BinOp(_, l, r) => expr_mentions_var(l, name) || expr_mentions_var(r, name),
        Expr::UnaryOp(_, inner) => expr_mentions_var(inner, name),
        Expr::Lambda(_, body) => expr_mentions_var(body, name),
        Expr::Length(inner) => expr_mentions_var(inner, name),
        Expr::Comprehension { collection, body, .. } => {
            expr_mentions_var(collection, name) || expr_mentions_var(body, name)
        }
        // M1 features (Phase 73): structural recursion over
        // the new forms.
        Expr::MethodCall { receiver, args, .. } => {
            expr_mentions_var(receiver, name)
                || args.iter().any(|a| expr_mentions_var(a, name))
        }
        Expr::RecordLit { fields, .. } => {
            fields.iter().any(|(_, v)| expr_mentions_var(v, name))
        }
    }
}

/// The guard-table body: one match over the spec's single
/// parameter, with literal arms for the inputs its guard
/// rows pin and a wildcard arm for the rest. The parameter
/// is an Int (rows recognized by their guard leaves against
/// Int constants) or a Bool (rows over the finite domain —
/// a total Bool table pins both domain values, so its
/// wildcard arm is unreachable, but the match convention
/// keeps it: the verifier's symbolic match requires a final
/// unconditional arm).
///
/// Rows are recognized exactly as the guard family's rows
/// are (see `guard_row_parts`): an Or of guard leaves plus
/// one output pin. Every pin must convert to an
/// `IRNode::Literal` — the pin test that keeps this family
/// disjoint from the expression-pin guard table (compound
/// pins) and from guarded identity (a bare-variable pin).
/// A row-shaped postcondition deviating on any of this —
/// a non-literal pin, a guard leaf that does not parse on
/// the parameter — yields no seed at all (all rows or
/// none, the guarded-identity rule); non-row
/// postconditions are skipped, as they always have been.
///
/// The pinned cells are recovered by JOINT
/// falsify-every-leaf — the pair-keyed family's recovery
/// lifted to one key (Phase 37, closing Phase 36's
/// Finding 1). Before Phase 37 a row was recovered only
/// where `pin_int_value` found exactly ONE falsifying
/// value, and a row pinning a SET of values — a band
/// (`site<0||site>4||output==0`) or an Eq-leaf catch-all
/// pinning a complement — was silently SKIPPED: the
/// builder emitted a body contradicting the row wherever
/// its pin differed from the wildcard's default, and the
/// seed was Z3-refuted in every run. Now each row
/// contributes its whole falsifying REGION: the candidate
/// span is the leaf constants' range widened by two on
/// each side — outside the outermost constants every
/// leaf's truth is stable, so the span's endpoints double
/// as the two unbounded tail classes and the finite span
/// describes the whole (infinite) table. A cell is pinned
/// exactly where every row firing there agrees on the
/// value; rows pinning one cell to two different values
/// are jointly inconsistent — not a table, no seed. A row
/// whose guards are jointly always true fires nowhere and
/// is subsumed vacuously.
///
/// Arm materialization keeps the builder's standing
/// convention — arms in row order, a single-value row
/// arming its value exactly as it always has (even where
/// its pin equals the wildcard's value: node_arity arms
/// 0 → 0 and 1 → 0 beside a 0 wildcard) — and adds the
/// pair family's fall-through discipline for the cells
/// only a band or catch-all row pins: the wildcard carries
/// the pinned fall-through value when the tail classes
/// agree on one (rust_stmt_form's catch-all pins both
/// tails to 17, so its wildcard IS 17, not the typed
/// default 0), else the typed-literal default (the Phase
/// 10 menu's first entry — the spec is silent off-table,
/// so the default is a documented guess; the Phase 15
/// classification showed the mirrors return exactly that
/// default on unpinned cases). A band row's interior cells
/// pinning the wildcard's own value need no arm — the
/// wildcard already yields it — and its tail cells ride
/// the wildcard, which carries their value by the
/// fall-through rule; the two tails pinning different
/// values is not this body's shape (no seed). Every body
/// the pre-Phase-37 builder produced is reproduced
/// byte-identically (the golden-fixture discipline): a
/// single-value row's region is a single interior cell —
/// the ±2 widening puts two probe points on each side, so
/// a region reaching a tail always shows at least two
/// span cells — and specs without band rows recover the
/// same cells, the same arms, and the same default
/// wildcard. The raw builder stops at constructing the
/// match; admission is the seed builder's step.
fn guard_table_body(spec: &FunctionSpec) -> Option<IRNode> {
    if spec.params.len() != 1 {
        return None;
    }
    let param = &spec.params[0];
    if is_int_family(&param.ty) {
        guard_table_body_int(spec, param)
    } else if matches!(param.ty, TypeRef::Bool) {
        guard_table_body_bool(spec, param)
    } else {
        None
    }
}

/// The Int guard table (see `guard_table_body`): joint
/// region recovery over the candidate span.
fn guard_table_body_int(spec: &FunctionSpec, param: &Param) -> Option<IRNode> {
    // Pass one: parse every row-shaped postcondition into
    // (guard leaves, literal pin), collecting the leaf
    // constants the candidate span is built from.
    let mut rows: Vec<(Vec<(BinOp, bool, i64)>, IRNode)> = Vec::new();
    let mut constants: Vec<i64> = Vec::new();
    for c in &spec.postconditions {
        let Some((pin_side, guard_leaves)) = guard_row_parts(c) else {
            continue; // not a row: skipped, as in every guard builder
        };
        // Row-shaped: it must be an Int guard row of this
        // table, or the spec states a row this body would
        // not honor — a deviation, so no seed.
        let pin = expr_to_ir(pin_side);
        if !matches!(pin, IRNode::Literal(_)) {
            return None; // the pin test: literal pins only
        }
        let mut guards: Vec<(BinOp, bool, i64)> = Vec::new();
        for leaf in guard_leaves {
            guards.push(guard_leaf(leaf, &param.name)?);
        }
        constants.extend(guards.iter().map(|g| g.2));
        rows.push((guards, pin));
    }
    if rows.is_empty() {
        return None;
    }
    // The candidate span (see the doc comment): the leaf
    // constants' range widened by two on each side.
    // Constants at the i64 extremes leave no room for the
    // widening — refused rather than scanned wrongly.
    let lo = constants.iter().min().copied()?.checked_sub(2)?;
    let hi = constants.iter().max().copied()?.checked_add(2)?;
    if hi as i128 - lo as i128 + 1 > 100_000 {
        return None; // a span too wide for the scan to state
    }
    // Each row's falsifying region over the span, and the
    // joint table: a cell is pinned exactly where every
    // row firing there agrees on the value.
    let mut regions: Vec<Vec<i64>> = Vec::new();
    let mut pinned: HashMap<i64, IRNode> = HashMap::new();
    for (guards, pin) in &rows {
        let region: Vec<i64> = (lo..=hi)
            .filter(|x| guards.iter().all(|g| !guard_leaf_holds(g, *x)))
            .collect();
        for cell in &region {
            match pinned.get(cell) {
                None => {
                    pinned.insert(*cell, pin.clone());
                }
                Some(p) if literal_pin_eq(p, pin) => {}
                Some(_) => return None, // conflicting pins: not a table
            }
        }
        regions.push(region);
    }
    if pinned.is_empty() {
        return None;
    }
    // The wildcard's value: the pinned fall-through value
    // when the tail classes agree on one, else the
    // typed-literal default.
    let default = typed_literal_fillings(&spec.return_type)?
        .into_iter()
        .next()?;
    let mut fall: Vec<&IRNode> = Vec::new();
    for endpoint in [lo, hi] {
        if let Some(p) = pinned.get(&endpoint) {
            if !fall.iter().any(|q| literal_pin_eq(q, p)) {
                fall.push(p);
            }
        }
    }
    if fall.len() > 1 {
        return None; // the two tails disagree: no single wildcard states this table
    }
    let wildcard = fall
        .first()
        .map(|p| (*p).clone())
        .unwrap_or_else(|| default.clone());
    // Arms in row order (see the doc comment): a
    // single-value row arms its cell unconditionally; a
    // band row arms each interior cell the wildcard does
    // not already yield.
    let mut cases: Vec<(IRPattern, IRNode)> = Vec::new();
    let mut armed: Vec<i64> = Vec::new();
    for ((_, pin), region) in rows.iter().zip(regions.iter()) {
        for cell in region {
            if armed.contains(cell) {
                continue;
            }
            if region.len() > 1
                && (*cell == lo || *cell == hi || literal_pin_eq(pin, &wildcard))
            {
                continue; // the wildcard already yields this cell's value
            }
            armed.push(*cell);
            cases.push((IRPattern::Literal(IRLiteral::Int(*cell)), pin.clone()));
        }
    }
    if cases.is_empty() {
        return None; // every pinned cell rides the wildcard: a constant, not a table
    }
    cases.push((IRPattern::Wildcard, wildcard));
    Some(IRNode::Match {
        scrutinee: Box::new(IRNode::Var(param.name.clone())),
        cases,
    })
}

/// The Bool guard table (see `guard_table_body`): the same
/// joint recovery over the finite domain. A Bool guard
/// leaf (Eq/Neq against a BoolLit) is false at exactly one
/// domain value, so a row's falsifying region is a
/// singleton or empty — band rows cannot arise here — but
/// the deviation discipline is the same: a row-shaped
/// postcondition with a non-literal pin or an unparseable
/// leaf, and two rows pinning one domain value to
/// different literals, refuse the seed instead of being
/// silently skipped / dropped first-wins (Phase 37).
/// Materialization is unchanged: one arm per pinned value
/// in row order, the typed-literal default on the
/// wildcard.
fn guard_table_body_bool(spec: &FunctionSpec, param: &Param) -> Option<IRNode> {
    let mut rows: Vec<(Vec<bool>, IRNode)> = Vec::new();
    let mut pinned: HashMap<bool, IRNode> = HashMap::new();
    for c in &spec.postconditions {
        let Some((pin_side, guard_leaves)) = guard_row_parts(c) else {
            continue; // not a row: skipped, as in every guard builder
        };
        // Row-shaped: it must be a Bool guard row of this
        // table, or the spec states a row this body would
        // not honor — a deviation, so no seed.
        let pin = expr_to_ir(pin_side);
        if !matches!(pin, IRNode::Literal(_)) {
            return None; // the pin test: literal pins only
        }
        let mut guards: Vec<(BinOp, bool, bool)> = Vec::new();
        for leaf in guard_leaves {
            guards.push(guard_leaf_bool(leaf, &param.name)?);
        }
        let region: Vec<bool> = [false, true]
            .into_iter()
            .filter(|x| guards.iter().all(|g| !guard_leaf_bool_holds(g, *x)))
            .collect();
        for cell in &region {
            match pinned.get(cell) {
                None => {
                    pinned.insert(*cell, pin.clone());
                }
                Some(p) if literal_pin_eq(p, &pin) => {}
                Some(_) => return None, // conflicting pins: not a table
            }
        }
        rows.push((region, pin));
    }
    if pinned.is_empty() {
        return None;
    }
    let mut cases: Vec<(IRPattern, IRNode)> = Vec::new();
    let mut seen: Vec<bool> = Vec::new();
    for (region, pin) in &rows {
        for cell in region {
            if seen.contains(cell) {
                continue; // an agreeing duplicate: subsumed
            }
            seen.push(*cell);
            cases.push((IRPattern::Literal(IRLiteral::Bool(*cell)), pin.clone()));
        }
    }
    let default = typed_literal_fillings(&spec.return_type)?
        .into_iter()
        .next()?;
    cases.push((IRPattern::Wildcard, default));
    Some(IRNode::Match {
        scrutinee: Box::new(IRNode::Var(param.name.clone())),
        cases,
    })
}

/// The guard-table seed's body: the guard-table match,
/// admitted when the verifier can encode it in the module
/// context.
fn guard_table_seed_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let body = guard_table_body(spec)?;
    if crate::verify::verifier::body_is_encodable_with(spec, &body, index) {
        Some(body)
    } else {
        None
    }
}

/// The guarded-identity body: one match over the guard
/// parameter of a two-parameter spec whose rows pin the
/// output to the OTHER parameter — the identity target:
/// `<guards on g> || output == p`. The arm for each pinned
/// guard value returns the identity parameter itself;
/// every input combination no row pins falls to the
/// wildcard arm carrying the typed-literal default — the
/// guard-table stance, so the complementary branch is a
/// documented guess, never a claim about the spec.
///
/// Conservative in the same way as the other builders:
/// exactly two parameters (one guard — Int or Bool family —
/// and one identity target whose type is the return type,
/// since the pinned arm returns it directly); every
/// row-shaped postcondition must be an identity row for the
/// SAME target — a row pinning a literal, a compound
/// expression, or another parameter is a shape deviation
/// and yields no seed at all, rather than a body that
/// silently drops the row — while non-row postconditions
/// are skipped as in the guard-table builder. Specs with
/// more than two parameters (a second parameter would have
/// to be pinned too), two candidate identity targets, or a
/// guard on the identity parameter itself are not admitted.
fn guarded_identity_body(spec: &FunctionSpec) -> Option<IRNode> {
    if spec.params.len() != 2 {
        return None;
    }
    // Either parameter may be the guard; the rows decide.
    guarded_identity_body_with_roles(spec, 0, 1)
        .or_else(|| guarded_identity_body_with_roles(spec, 1, 0))
}

fn guarded_identity_body_with_roles(
    spec: &FunctionSpec,
    guard_idx: usize,
    identity_idx: usize,
) -> Option<IRNode> {
    let guard_param = &spec.params[guard_idx];
    let identity_param = &spec.params[identity_idx];
    if !is_int_family(&guard_param.ty) && !matches!(guard_param.ty, TypeRef::Bool) {
        return None;
    }
    // The pinned arm returns the identity parameter itself,
    // so it must BE the return value's type.
    if identity_param.ty != spec.return_type {
        return None;
    }
    let mut cases: Vec<(IRPattern, IRNode)> = Vec::new();
    let mut seen: Vec<GuardPin> = Vec::new();
    for c in &spec.postconditions {
        if guard_row_parts(c).is_none() {
            continue; // not a row: skipped, as in guard_table_body
        }
        // Row-shaped: it must be an identity row for this
        // target, or the spec states a row this body would
        // not honor — a deviation, so no seed.
        let pinned = guard_identity_row(c, guard_param, &identity_param.name)?;
        if seen.contains(&pinned) {
            continue; // an agreeing duplicate: every identity row pins the same Var, so it is subsumed
        }
        seen.push(pinned);
        cases.push((
            pinned.pattern(),
            IRNode::Var(identity_param.name.clone()),
        ));
    }
    if cases.is_empty() {
        return None;
    }
    let default = typed_literal_fillings(&spec.return_type)?
        .into_iter()
        .next()?;
    cases.push((IRPattern::Wildcard, default));
    Some(IRNode::Match {
        scrutinee: Box::new(IRNode::Var(guard_param.name.clone())),
        cases,
    })
}

/// The guarded-identity seed's body: the identity match,
/// admitted when the verifier can encode it in the module
/// context.
fn guarded_identity_seed_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let body = guarded_identity_body(spec)?;
    if crate::verify::verifier::body_is_encodable_with(spec, &body, index) {
        Some(body)
    } else {
        None
    }
}

/// The composition-guard body: the guarded-identity shape
/// generalized along both of its axes. The guard is ONE
/// expression shared by every row, valued in the finite
/// Bool domain: a Bool parameter, or a call to another spec
/// of the module whose return is Bool (the guard in
/// `rust_ident_suffix` is `rust_keyword(name)` — a
/// composition, which is where the family gets its name).
/// The output pin is any expression over the parameters —
/// a literal, a variable, a concatenation — where the
/// earlier families admitted only a literal (guard table)
/// or the one identity variable (guarded identity).
///
/// Rows are recognized exactly as the guard family's rows
/// are (see `guard_row_parts`): an Or of guard leaves plus
/// one output pin, the pinned guard value being the unique
/// domain value falsifying every leaf — here the leaves
/// compare the guard EXPRESSION against BoolLit constants
/// (`guard_leaf_on_expr`), so a call guard is pinned by
/// evaluating its leaves over {false, true} without ever
/// executing the call. A row-shaped postcondition that
/// deviates — a pin mentioning `output`, a pin or guard
/// calling a spec the index does not hold, leaves comparing
/// two different guard expressions, a guard whose value is
/// not Bool — yields no seed at all (all rows or none, the
/// guarded-identity rule); non-row postconditions are
/// skipped as in the guard-table builder.
///
/// Territory: the earlier families keep their shapes WHEN
/// THEY CAN STATE THEM. This builder's original rule fired
/// only where they cannot state the spec at all — the guard
/// is a call, or some pin is a compound expression — and
/// refused every plain-variable guard with literal /
/// bare-variable pins outright. Phase 34 restates that
/// refusal as a PRECEDENCE for the plain-guard case: the
/// shape is claimed here exactly when NEITHER earlier
/// family admits the spec (the guard table refuses it —
/// its arity is one, its pins must be literals; guarded
/// identity refuses it — its arity is two, a literal row
/// deviates, the pin must be the remaining parameter),
/// with one guard of its own: a bare-variable pin must
/// name a parameter of the return type, the only way the
/// pin can BE the output (guarded identity's own condition
/// on its identity parameter). The measured shape is
/// `rust_concat_left_text`: bare-variable pins under a
/// plain Bool parameter at arity three, seedless BETWEEN
/// the families — guarded identity's arity cap on one
/// side, the guard table's literal pin test on the other.
/// The precedence is the disjointness proof, not a hope:
/// whenever an earlier family admits the spec this builder
/// still refuses, so no spec ever gains a second seed.
///
/// The emitted body is one match over the guard expression
/// — a call guard is evaluated once, as the scrutinee — with
/// one arm per pinned guard value returning the row's pin
/// expression, and the typed-literal wildcard for guard
/// values no row pins: the guard-table stance, a documented
/// guess the spec is silent about. (Both measured specs pin
/// the whole domain, so their wildcards are unreachable.)
fn composition_guard_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let mut guard_expr: Option<Expr> = None;
    let mut rows: Vec<(bool, IRNode, Expr)> = Vec::new();
    for c in &spec.postconditions {
        if guard_row_parts(c).is_none() {
            continue; // not a row: skipped, as in guard_table_body
        }
        // Row-shaped: it must be a composition row for the
        // one shared guard expression, or the spec states a
        // row this body would not honor — a deviation, so
        // no seed.
        let (target, pinned, pin, pin_side) = composition_guard_row(c, index)?;
        match &guard_expr {
            None => guard_expr = Some(target),
            Some(g) if expr_same(g, &target) => {}
            Some(_) => return None, // two guard expressions: not one guard
        }
        if let Some((_, existing_pin, _)) = rows.iter().find(|(p, _, _)| *p == pinned) {
            // A second row pinning this guard value: an
            // agreeing pin is subsumed; a different one is a
            // row this body would not honor — a deviation,
            // not a first-wins drop (Phase 37).
            if !literal_pin_eq(existing_pin, &pin) {
                return None;
            }
            continue;
        }
        rows.push((pinned, pin, pin_side));
    }
    if rows.is_empty() {
        return None;
    }
    let guard_expr = guard_expr?;
    let scrutinee = composition_scrutinee(spec, &guard_expr, index)?;
    // Territory (see the doc comment): a plain-variable
    // guard with only literal/bare-variable pins belongs to
    // the earlier families WHEN THEY ADMIT IT — the guard
    // table (literal pins) or guarded identity (the one
    // bare variable). The Phase 34 precedence: where
    // neither admits the spec, this builder claims the
    // shape instead of leaving it seedless between the
    // families, provided every bare-variable pin names a
    // parameter of the return type.
    let guard_is_call = matches!(guard_expr, Expr::Call(..));
    let any_compound_pin = rows.iter().any(|(_, _, side)| {
        !matches!(
            side,
            Expr::Var(_)
                | Expr::IntLit(_)
                | Expr::FloatLit(_)
                | Expr::BoolLit(_)
                | Expr::StringLit(_)
                | Expr::CharLit(_)
        )
    });
    if !guard_is_call && !any_compound_pin {
        let earlier_family_claims =
            guard_table_body(spec).is_some() || guarded_identity_body(spec).is_some();
        let var_pins_return_typed = rows.iter().all(|(_, _, side)| match side {
            Expr::Var(v) => spec
                .params
                .iter()
                .find(|p| &p.name == v)
                .map(|p| p.ty == spec.return_type)
                .unwrap_or(false),
            _ => true,
        });
        if earlier_family_claims || !var_pins_return_typed {
            return None;
        }
    }
    let mut cases: Vec<(IRPattern, IRNode)> = rows
        .into_iter()
        .map(|(pinned, pin, _)| (IRPattern::Literal(IRLiteral::Bool(pinned)), pin))
        .collect();
    let default = typed_literal_fillings(&spec.return_type)?
        .into_iter()
        .next()?;
    cases.push((IRPattern::Wildcard, default));
    Some(IRNode::Match {
        scrutinee: Box::new(scrutinee),
        cases,
    })
}

/// The composition-guard seed's body: the composition match
/// with any spec-to-spec calls resolved against the module
/// (the equation seed's admission precedent — a raw call
/// body could not be executed by the interpreter or emitted
/// standalone), admitted when the verifier can encode the
/// resolved body in the module context.
fn composition_guard_seed_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let raw = composition_guard_body(spec, index)?;
    let body = index.inline_calls(&raw)?;
    if crate::verify::verifier::body_is_encodable_with(spec, &body, index) {
        Some(body)
    } else {
        None
    }
}

/// The most distinct guard expressions a multi-guard
/// truth table may range over. The joint table doubles
/// per guard — 2^n cells to falsify against, and a nested
/// match whose size follows — so the cap keeps the family
/// at shapes a spec author would actually write as a
/// truth table. The measured shape needs two.
const MAX_MULTI_GUARD_EXPRESSIONS: usize = 4;

/// The multi-guard truth-table body: the composition-guard
/// shape with the one-shared-guard requirement lifted —
/// the generalization the Phase 31 measurement specified
/// for `rust_match_scrutinee_form`, whose rows guard on
/// TWO different Bool expressions and were seedless solely
/// because `composition_guard_row` requires one shared
/// guard.
///
/// Rows are recognized exactly as the guard family's rows
/// are (see `guard_row_parts`): an Or of guard leaves plus
/// one output pin, the pin admitted the way the
/// composition family admits pins (`composition_pin`). The
/// leaves of a row may compare ANY subset of the guard
/// expressions against BoolLit constants
/// (`guard_leaf_on_expr`); grouped per expression, each
/// group pins a single value by falsify-every-leaf
/// (`pin_bool_value`), and the row then pins every cell of
/// the joint {false, true}^n table extending that partial
/// assignment. The guard set is the distinct expressions
/// across all rows, in first-appearance order (postcondition
/// order, leaf order within a row). Every guard expression
/// must be a valid composition scrutinee — a Bool
/// parameter, or a call to an indexed spec returning Bool
/// (`composition_scrutinee`). A row-shaped postcondition
/// deviating on any of this — a pin mentioning `output`,
/// an unindexed call in a pin or a guard, a leaf comparing
/// no BoolLit, a guard group pinning no single value, a
/// non-Bool guard — yields no seed at all (all rows or
/// none, the guarded-identity rule); non-row postconditions
/// are skipped as in the guard-table builder.
///
/// The pinned cells are recovered by JOINT
/// falsify-every-leaf over the whole table — the
/// pair-keyed family's joint falsification on the Bool
/// domain: a cell is pinned exactly where every row firing
/// there agrees on the value. Two rows pinning one cell to
/// different values are jointly inconsistent — not a
/// truth table, no seed. Cells NO row decides stay
/// unpinned and fall to the wildcard: a partial table is
/// admitted, never forced.
///
/// Territory: the family requires at least TWO distinct
/// guard expressions — one is the composition family's
/// shape, and its bodies stay exactly what they were (the
/// golden fixture proves it) — and at most
/// MAX_MULTI_GUARD_EXPRESSIONS. The Int families cannot
/// state these rows (their leaves parse parameter-vs-Int
/// comparisons only), and neither can the earlier Bool
/// families (the guard table's arity is one, guarded
/// identity's guard is a lone parameter, composition's is
/// a lone expression), so no spec ever gains a second
/// seed.
///
/// The emitted body is nested matches over the guard
/// expressions, outermost-first in guard order: along any
/// single execution path each guard is evaluated at most
/// once, as one match's scrutinee (a call scrutinee may
/// repeat textually across sibling subtrees, but specs are
/// pure, so the repetition is value-identical). A subtree
/// whose cells' effective values — the pin where pinned,
/// the typed-literal default where not — all agree renders
/// that value directly, with no match at all. Every match
/// ends in the wildcard carrying the typed-literal default:
/// the composition stance (the Bool domain is total, so
/// with both literal arms present the wildcard is the
/// verifier-required unconditional last arm, reached only
/// where a subtree was not compressed). The raw builder
/// stops at constructing the matches; admission is the
/// seed builder's step.
fn multi_guard_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    // Pass one: parse the rows into partial assignments
    // over guard EXPRESSIONS (first-appearance order is
    // fixed in pass two).
    let mut parsed: Vec<(Vec<(Expr, bool)>, IRNode)> = Vec::new();
    for c in &spec.postconditions {
        if guard_row_parts(c).is_none() {
            continue; // not a row: skipped, as in guard_table_body
        }
        // Row-shaped: it must be a multi-guard row, or the
        // spec states a row this body would not honor — a
        // deviation, so no seed.
        parsed.push(multi_guard_row(c, index)?);
    }
    if parsed.is_empty() {
        return None;
    }
    // Pass two: the distinct guard expressions, in
    // first-appearance order, each validated as a
    // composition scrutinee.
    let mut guards: Vec<Expr> = Vec::new();
    let mut scrutinees: Vec<IRNode> = Vec::new();
    for (partial, _) in &parsed {
        for (target, _) in partial {
            if !guards.iter().any(|g| expr_same(g, target)) {
                scrutinees.push(composition_scrutinee(spec, target, index)?);
                guards.push(target.clone());
            }
        }
    }
    let n = guards.len();
    if n < 2 || n > MAX_MULTI_GUARD_EXPRESSIONS {
        return None; // one guard is composition's; past the cap the table doubles per guard
    }
    // Joint falsify-every-leaf over the joint table: cell
    // index bit (n - 1 - g) is guard g's value in the cell.
    let mut table: Vec<Option<IRNode>> = vec![None; 1 << n];
    for (partial, pin) in &parsed {
        let indexed: Vec<(usize, bool)> = partial
            .iter()
            .map(|(target, value)| {
                let g = guards
                    .iter()
                    .position(|e| expr_same(e, target))
                    .expect("every row guard was collected in pass two");
                (g, *value)
            })
            .collect();
        for (idx, cell) in table.iter_mut().enumerate() {
            let fires = indexed
                .iter()
                .all(|(g, v)| ((idx >> (n - 1 - g)) & 1 == 1) == *v);
            if fires {
                match cell {
                    None => *cell = Some(pin.clone()),
                    Some(p) if literal_pin_eq(p, pin) => {}
                    Some(_) => return None, // conflicting pins: not a truth table
                }
            }
        }
    }
    if table.iter().all(|c| c.is_none()) {
        return None;
    }
    let default = typed_literal_fillings(&spec.return_type)?
        .into_iter()
        .next()?;
    Some(multi_guard_match(&scrutinees, 0, &table, &default))
}

/// One level of the multi-guard body: `table` is the
/// contiguous slice of the joint table under the prefix of
/// guard values already matched — its first half has the
/// next guard false, its second half true. A slice whose
/// effective values (the pin where pinned, the default
/// where not) all agree renders that value directly.
fn multi_guard_match(
    scrutinees: &[IRNode],
    depth: usize,
    table: &[Option<IRNode>],
    default: &IRNode,
) -> IRNode {
    let effective = |cell: &Option<IRNode>| cell.clone().unwrap_or_else(|| default.clone());
    let first = effective(&table[0]);
    if table.iter().all(|c| literal_pin_eq(&effective(c), &first)) {
        return first;
    }
    let half = table.len() / 2;
    let (false_half, true_half) = table.split_at(half);
    IRNode::Match {
        scrutinee: Box::new(scrutinees[depth].clone()),
        cases: vec![
            (
                IRPattern::Literal(IRLiteral::Bool(false)),
                multi_guard_match(scrutinees, depth + 1, false_half, default),
            ),
            (
                IRPattern::Literal(IRLiteral::Bool(true)),
                multi_guard_match(scrutinees, depth + 1, true_half, default),
            ),
            (IRPattern::Wildcard, default.clone()),
        ],
    }
}

/// One multi-guard row: row-shaped (see `guard_row_parts`),
/// its guard leaves comparing BoolLit constants against
/// any subset of the guard expressions — each expression's
/// leaves pinning a single value of the {false, true}
/// domain by falsify-every-leaf — and its output pin an
/// admissible expression (see `composition_pin`). Returns
/// the partial assignment (guard expression, pinned value)
/// and the pin as IR. Any deviation is None; the builder
/// treats a row-shaped postcondition failing here as a
/// shape deviation, not a skippable non-row.
fn multi_guard_row(c: &Constraint, index: &SpecIndex) -> Option<(Vec<(Expr, bool)>, IRNode)> {
    let (pin_side, guard_leaves) = guard_row_parts(c)?;
    let pin = composition_pin(pin_side, index)?;
    let mut groups: Vec<(Expr, Vec<(BinOp, bool, bool)>)> = Vec::new();
    for leaf in guard_leaves {
        let (target, normalized) = guard_leaf_on_expr(leaf)?;
        match groups.iter_mut().find(|(t, _)| expr_same(t, &target)) {
            Some((_, gs)) => gs.push(normalized),
            None => groups.push((target, vec![normalized])),
        }
    }
    let mut partial = Vec::new();
    for (target, gs) in groups {
        partial.push((target, pin_bool_value(&gs)?));
    }
    Some((partial, pin))
}

/// The multi-guard seed's body: the nested truth-table
/// match with any spec-to-spec calls resolved against the
/// module (the composition seed's admission precedent — a
/// raw call body could not be executed by the interpreter
/// or emitted standalone), admitted when the verifier can
/// encode the resolved body in the module context.
fn multi_guard_seed_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let raw = multi_guard_body(spec, index)?;
    let body = index.inline_calls(&raw)?;
    if crate::verify::verifier::body_is_encodable_with(spec, &body, index) {
        Some(body)
    } else {
        None
    }
}

/// The expression-pin guard-table body: the guard-table
/// shape with the pin test generalized. Rows are recognized
/// exactly as the guard family's rows are (see
/// `guard_row_parts`): an Or of guard leaves plus one output
/// pin. Every row's guard leaves compare ONE shared Int
/// parameter — the key — against Int constants
/// (`guard_leaf`), and the key value a row pins is recovered
/// by `pin_int_value`'s falsify-every-leaf scan, the
/// single-value recovery the guard table's region recovery
/// generalizes. What changes is the pin: where the guard
/// table requires an `IRNode::Literal`, this family
/// admits any pin `composition_pin` admits — a hole-free,
/// output-free expression over the spec's parameters calling
/// only indexed specs (a concatenation template, a bare
/// variable, a call). The measured shapes are the slice-4
/// text templates: `rust_unary_text` (ten Concat pins under
/// `op`), `rust_concat_right_text` and
/// `rust_match_scrutinee_text` (a bare-Var pin beside Concat
/// pins under `form`).
///
/// The key is nominated by the first row-shaped
/// postcondition: the one Int-family parameter under which
/// all of its leaves parse and pin a single value. (A row's
/// leaves name their parameter, so at most one parameter can
/// qualify; zero — the row is guarded on two keys, on a Bool
/// or String parameter, or on no parameter — is not this
/// family, and two would be ambiguous: both yield no seed.)
/// Every later row-shaped postcondition must then be a row
/// under THAT key, or the spec states a row this body would
/// not honor — a deviation, so no seed at all (all rows or
/// none, the guarded-identity rule); non-row postconditions
/// are skipped as in the guard-table builder.
///
/// Territory (the pin test keeps the families disjoint, so
/// no spec ever gains a second seed): at least one row's pin
/// must be a COMPOUND expression — not a literal and not a
/// bare variable. A set of all-literal pins is a guard table
/// (or, at an arity the guard table does not take, the
/// recorded pair-keyed generalization's open shape) and a
/// set of all-bare-variable pins is guarded identity's exact
/// shape; mixed sets — a literal or bare-Var pin beside a
/// compound one — ARE admitted wholesale: those rows are
/// just pins, and the measured specs both carry a bare-Var
/// row beside their Concat rows.
///
/// The emitted body is one match over the key parameter,
/// one arm per pinned key value returning the row's pin
/// expression, and the typed-literal wildcard for key values
/// no row pins: the guard-table stance, a documented guess
/// the spec is silent about. A second row pinning an
/// already-pinned key value is skipped only when its pin
/// agrees with the first row's — an agreeing duplicate is
/// subsumed; a conflicting one is a deviation (Phase 37).
/// The raw builder stops at constructing the
/// match; admission is the seed builder's step.
fn expr_pin_guard_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let mut key: Option<&Param> = None;
    let mut rows: Vec<(i64, IRNode, Expr)> = Vec::new();
    for c in &spec.postconditions {
        if guard_row_parts(c).is_none() {
            continue; // not a row: skipped, as in guard_table_body
        }
        // Row-shaped: it must be an expression-pin row under
        // the one shared key, or the spec states a row this
        // body would not honor — a deviation, so no seed.
        let (pinned, pin, pin_side) = match key {
            Some(k) => expr_pin_guard_row(c, k, index)?,
            None => {
                let mut found: Option<(&Param, (i64, IRNode, Expr))> = None;
                for param in &spec.params {
                    if !is_int_family(&param.ty) {
                        continue;
                    }
                    if let Some(row) = expr_pin_guard_row(c, param, index) {
                        if found.is_some() {
                            return None; // two keys parse one row: ambiguous
                        }
                        found = Some((param, row));
                    }
                }
                let (param, row) = found?;
                key = Some(param);
                row
            }
        };
        if let Some((_, existing_pin, _)) = rows.iter().find(|(p, _, _)| *p == pinned) {
            // A second row pinning this key value: an
            // agreeing pin is subsumed; a different one is a
            // row this body would not honor — a deviation,
            // not a first-wins drop (Phase 37).
            if !literal_pin_eq(existing_pin, &pin) {
                return None;
            }
            continue;
        }
        rows.push((pinned, pin, pin_side));
    }
    if rows.is_empty() {
        return None;
    }
    let key = key?;
    // Territory (see the doc comment): at least one pin must
    // be a compound expression — the test the composition
    // builder applies to its pin sides.
    let any_compound_pin = rows.iter().any(|(_, _, side)| {
        !matches!(
            side,
            Expr::Var(_)
                | Expr::IntLit(_)
                | Expr::FloatLit(_)
                | Expr::BoolLit(_)
                | Expr::StringLit(_)
                | Expr::CharLit(_)
        )
    });
    if !any_compound_pin {
        return None;
    }
    let mut cases: Vec<(IRPattern, IRNode)> = rows
        .into_iter()
        .map(|(pinned, pin, _)| (IRPattern::Literal(IRLiteral::Int(pinned)), pin))
        .collect();
    let default = typed_literal_fillings(&spec.return_type)?
        .into_iter()
        .next()?;
    cases.push((IRPattern::Wildcard, default));
    Some(IRNode::Match {
        scrutinee: Box::new(IRNode::Var(key.name.clone())),
        cases,
    })
}

/// One expression-pin guard row: row-shaped (see
/// `guard_row_parts`), its guard leaves all comparing the
/// key parameter against Int constants and pinning a single
/// key value (`pin_int_value`), and its output pin an
/// admissible expression (see `composition_pin`). Returns
/// the pinned key value, the pin as IR, and the pin's source
/// expression (the builder's territory rule reads it). Any
/// deviation is None; the builder treats a row-shaped
/// postcondition failing here as a shape deviation, not a
/// skippable non-row.
fn expr_pin_guard_row(
    c: &Constraint,
    key: &Param,
    index: &SpecIndex,
) -> Option<(i64, IRNode, Expr)> {
    let (pin_side, guard_leaves) = guard_row_parts(c)?;
    let pin = composition_pin(pin_side, index)?;
    let mut guards: Vec<(BinOp, bool, i64)> = Vec::new();
    for leaf in guard_leaves {
        guards.push(guard_leaf(leaf, &key.name)?);
    }
    Some((pin_int_value(&guards)?, pin, pin_side.clone()))
}

/// The expression-pin guard-table seed's body: the match
/// with any spec-to-spec calls in its pins resolved against
/// the module (the composition seed's admission precedent —
/// a raw call body could not be executed by the interpreter
/// or emitted standalone), admitted when the verifier can
/// encode the resolved body in the module context.
fn expr_pin_guard_seed_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let raw = expr_pin_guard_body(spec, index)?;
    let body = index.inline_calls(&raw)?;
    if crate::verify::verifier::body_is_encodable_with(spec, &body, index) {
        Some(body)
    } else {
        None
    }
}

/// One pair-keyed guard row, parsed: its guard leaves
/// normalized against the key each constrains (0 = the
/// first parameter, 1 = the second), and its output pin
/// converted to an IR literal.
struct PairGuardRow {
    leaves: Vec<(usize, BinOp, bool, i64)>,
    pin: IRNode,
}

/// Pin equality, by Debug rendering — the golden
/// fixture's byte-identity standard (IRNode derives no
/// PartialEq). Applied to literal pins by the pair builder
/// (its pin test restricts them to literals) and to
/// arbitrary pin expressions by the multi-guard builder's
/// conflict test; Debug rendering is shape-agnostic, so
/// the standard is the same.
fn literal_pin_eq(a: &IRNode, b: &IRNode) -> bool {
    format!("{:?}", a) == format!("{:?}", b)
}

/// The pair-keyed guard-table body: the guard-table shape
/// lifted from one Int parameter to a PAIR of them — the
/// generalization the Phase 31 measurement specified for
/// `rust_concat_left_owned`, `rust_concat_right_form` and
/// `rust_match_scrutinee_bare`, whose rows key on a (kind,
/// lit) pair and were seedless solely because
/// `guard_table_body` admits exactly one parameter.
///
/// Rows are recognized exactly as the guard family's rows
/// are (see `guard_row_parts`): an Or of guard leaves plus
/// one output pin. Every leaf must parse via `guard_leaf`
/// against one of the two parameters — the keys — and the
/// leaves must jointly range over BOTH (a one-key row set
/// is the single-key families' shape, refused here on the
/// two-key requirement). Every pin must convert to an
/// `IRNode::Literal` — the guard table's own pin test,
/// which is what keeps this family disjoint from the
/// expression-pin guard table (it requires a compound
/// pin) and from guarded identity (a bare-variable pin).
/// A row-shaped postcondition deviating on any of this —
/// a leaf on neither key, a non-literal pin — yields no
/// seed at all (all rows or none, the guarded-identity
/// rule); non-row postconditions are skipped as in the
/// guard-table builder.
///
/// The pinned cells are recovered by JOINT falsify-every-
/// leaf, the two-axis form of `pin_int_value`'s scan: for
/// each key, the candidate span is the leaf constants'
/// range widened by two on each side — outside the
/// outermost constants every leaf's truth is stable, so
/// the span's endpoints double as the two unbounded tail
/// classes and the finite grid describes the whole
/// (infinite) table. A cell is pinned exactly where the
/// rows jointly determine ONE output value: every row
/// whose guard leaves all fail at the cell pins it there.
/// Rows pinning one cell to two different values are
/// jointly inconsistent — not a table, no seed. Rows
/// pinning whole rows or columns (a kind-only row, a
/// lit-only row) pin their cells by the same joint
/// falsification. Cells NO row decides stay unpinned —
/// scrutinee_bare's kind-22 column is the measured case —
/// and fall to the wildcard: a partial table is admitted,
/// never forced.
///
/// The emitted body is one nested match: over the first
/// key, then over the second inside any column whose
/// pinned cells disagree. A column whose pinned cells all
/// agree renders that literal directly, and a column whose
/// pinned cells all equal the fall-through value needs no
/// arm at all. The wildcards carry the value the pinned
/// fall-through cells determine (the tail columns', and
/// for an inner match the column's tail cells') — NOT
/// blindly the typed-literal default: Bool's menu starts
/// at `true`, but both measured Bool tables fall through
/// to a pinned `false`. Only when no pinned cell falls
/// through does a wildcard carry the typed-literal
/// default, the guard-table stance. The raw builder stops
/// at constructing the match; admission is the seed
/// builder's step.
fn pair_guard_table_body(spec: &FunctionSpec) -> Option<IRNode> {
    if spec.params.len() != 2 {
        return None;
    }
    let keys = [&spec.params[0], &spec.params[1]];
    if !is_int_family(&keys[0].ty) || !is_int_family(&keys[1].ty) {
        return None;
    }
    let mut rows: Vec<PairGuardRow> = Vec::new();
    let mut constants: [Vec<i64>; 2] = [Vec::new(), Vec::new()];
    for c in &spec.postconditions {
        let Some((pin_side, guard_leaves)) = guard_row_parts(c) else {
            continue; // not a row: skipped, as in guard_table_body
        };
        // Row-shaped: it must be a pair row, or the spec
        // states a row this body would not honor — a
        // deviation, so no seed.
        let pin = expr_to_ir(pin_side);
        if !matches!(pin, IRNode::Literal(_)) {
            return None; // the pin test: literal pins only
        }
        let mut leaves: Vec<(usize, BinOp, bool, i64)> = Vec::new();
        for leaf in guard_leaves {
            if let Some((op, on_left, constant)) = guard_leaf(leaf, &keys[0].name) {
                leaves.push((0, op, on_left, constant));
            } else if let Some((op, on_left, constant)) = guard_leaf(leaf, &keys[1].name) {
                leaves.push((1, op, on_left, constant));
            } else {
                return None; // a guard leaf on neither key
            }
        }
        for (ki, _, _, constant) in &leaves {
            constants[*ki].push(*constant);
        }
        rows.push(PairGuardRow { leaves, pin });
    }
    if rows.is_empty() {
        return None;
    }
    // The leaves must range over both keys.
    let mut uses = [false, false];
    for row in &rows {
        for (ki, ..) in &row.leaves {
            uses[*ki] = true;
        }
    }
    if !uses[0] || !uses[1] {
        return None;
    }
    // Candidate spans per key (see the doc comment): the
    // leaf constants' range widened by two on each side.
    // Constants at the i64 extremes leave no room for the
    // widening — refused rather than scanned wrongly.
    let mut spans = [(0i64, 0i64); 2];
    for (ki, span) in spans.iter_mut().enumerate() {
        let lo = constants[ki].iter().min()?.checked_sub(2)?;
        let hi = constants[ki].iter().max()?.checked_add(2)?;
        *span = (lo, hi);
    }
    let width = |span: (i64, i64)| span.1 as i128 - span.0 as i128 + 1;
    if width(spans[0]) * width(spans[1]) > 100_000 {
        return None; // a span too wide for the joint scan to state
    }
    // Joint falsify-every-leaf over the grid (see the doc
    // comment): a cell is pinned where every row whose
    // guard leaves all fail there agrees on the value.
    let mut pinned: HashMap<(i64, i64), IRNode> = HashMap::new();
    for a in spans[0].0..=spans[0].1 {
        for b in spans[1].0..=spans[1].1 {
            let mut cell: Option<&IRNode> = None;
            for row in &rows {
                let falsified = row.leaves.iter().all(|(ki, op, on_left, constant)| {
                    let guard = (*op, *on_left, *constant);
                    !guard_leaf_holds(&guard, if *ki == 0 { a } else { b })
                });
                if falsified {
                    match cell {
                        None => cell = Some(&row.pin),
                        Some(p) if literal_pin_eq(p, &row.pin) => {}
                        Some(_) => return None, // conflicting pins: not a table
                    }
                }
            }
            if let Some(pin) = cell {
                pinned.insert((a, b), pin.clone());
            }
        }
    }
    if pinned.is_empty() {
        return None;
    }
    let default = typed_literal_fillings(&spec.return_type)?
        .into_iter()
        .next()?;
    let column = |a: i64| -> Vec<(i64, &IRNode)> {
        (spans[1].0..=spans[1].1)
            .filter_map(|b| pinned.get(&(a, b)).map(|p| (b, p)))
            .collect()
    };
    // The outer wildcard's value: the pinned cells that
    // fall through to it — the tail columns and the
    // columns no row pins — must agree on it.
    let mut fall: Vec<&IRNode> = Vec::new();
    for a in spans[0].0..=spans[0].1 {
        let col = column(a);
        if a == spans[0].0 || a == spans[0].1 || col.is_empty() {
            for (_, p) in col {
                if !fall.iter().any(|q| literal_pin_eq(q, p)) {
                    fall.push(p);
                }
            }
        }
    }
    if fall.len() > 1 {
        return None; // disagreeing fall-through pins: not this body's shape
    }
    let outer_default = fall
        .first()
        .map(|p| (*p).clone())
        .unwrap_or_else(|| default.clone());
    let mut cases: Vec<(IRPattern, IRNode)> = Vec::new();
    for a in spans[0].0..=spans[0].1 {
        if a == spans[0].0 || a == spans[0].1 {
            continue; // the tail columns fall through to the wildcard
        }
        let col = column(a);
        if col.is_empty() || col.iter().all(|(_, p)| literal_pin_eq(p, &outer_default)) {
            continue; // unpinned, or the fall-through already yields its value
        }
        let first = col[0].1.clone();
        let body = if col.iter().all(|(_, p)| literal_pin_eq(p, &first)) {
            first // a constant column needs no inner match
        } else {
            // The inner wildcard is determined the same
            // way, over this column's tail cells.
            let mut inner_fall: Vec<&IRNode> = Vec::new();
            for (b, p) in &col {
                if *b == spans[1].0 || *b == spans[1].1 {
                    if !inner_fall.iter().any(|q| literal_pin_eq(q, p)) {
                        inner_fall.push(p);
                    }
                }
            }
            if inner_fall.len() > 1 {
                return None;
            }
            let inner_default = inner_fall
                .first()
                .map(|p| (*p).clone())
                .unwrap_or_else(|| default.clone());
            let mut inner: Vec<(IRPattern, IRNode)> = col
                .iter()
                .filter(|(b, p)| {
                    *b != spans[1].0 && *b != spans[1].1 && !literal_pin_eq(p, &inner_default)
                })
                .map(|(b, p)| (IRPattern::Literal(IRLiteral::Int(*b)), (*p).clone()))
                .collect();
            inner.push((IRPattern::Wildcard, inner_default));
            IRNode::Match {
                scrutinee: Box::new(IRNode::Var(keys[1].name.clone())),
                cases: inner,
            }
        };
        cases.push((IRPattern::Literal(IRLiteral::Int(a)), body));
    }
    cases.push((IRPattern::Wildcard, outer_default));
    Some(IRNode::Match {
        scrutinee: Box::new(IRNode::Var(keys[0].name.clone())),
        cases,
    })
}

/// The pair-keyed guard-table seed's body: the nested
/// match, admitted when the verifier can encode it in the
/// module context. No call resolution step (the guard-table
/// seed's admission precedent): the pins are literals, so
/// the body contains no spec calls to resolve.
fn pair_guard_table_seed_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let body = pair_guard_table_body(spec)?;
    if crate::verify::verifier::body_is_encodable_with(spec, &body, index) {
        Some(body)
    } else {
        None
    }
}

/// One mixed pair-keyed guard row: row-shaped (see
/// `guard_row_parts`), its guard leaves split between the
/// Int key (parsed via `guard_leaf`) and the Bool key
/// (parsed via `guard_leaf_bool`), and its output pin
/// converted to an IR literal.
struct MixedGuardRow {
    int_leaves: Vec<(BinOp, bool, i64)>,
    bool_leaves: Vec<(BinOp, bool, bool)>,
    pin: IRNode,
}

/// The mixed pair-keyed guard-table body: the pair-keyed
/// family's shape with one key of each domain — one
/// Int-family parameter and one Bool parameter — the
/// family the Phase 36 measurement specified for
/// `rust_stmt_seq_form`, whose rows guard jointly on
/// (position: Int, last: Bool) and which every earlier
/// family refused on a stated predicate (guard_table:
/// arity; pair_guard_table: both keys Int-family;
/// composition/multi-guard: Bool-domain leaves only, and
/// seq_form's rows carry Lt/Gt leaves on the Int key;
/// expression-pin guard table: one Int key, and its
/// territory requires a compound pin where these pins are
/// literal).
///
/// Rows are recognized exactly as the guard family's rows
/// are (see `guard_row_parts`): an Or of guard leaves plus
/// one output pin. Every leaf must parse via `guard_leaf`
/// against the Int key or via `guard_leaf_bool` against
/// the Bool key, and the leaves must jointly range over
/// BOTH keys (a one-key row set is an earlier family's
/// shape, refused here on the two-key requirement). Every
/// pin must convert to an `IRNode::Literal` — the pin
/// test that keeps this family disjoint from the
/// expression-pin guard table (it requires a compound
/// pin) and from guarded identity (a bare-variable pin).
/// A row-shaped postcondition deviating on any of this
/// yields no seed at all (all rows or none); non-row
/// postconditions are skipped as in the other guard
/// builders.
///
/// The pinned cells are recovered by JOINT
/// falsify-every-leaf over the mixed grid — the pair
/// family's recovery with the second axis the Bool
/// family's finite domain: the Int candidate span is the
/// leaf constants' range widened by two on each side
/// (outside the outermost constants every Int leaf's
/// truth is stable, so the span endpoints double as the
/// two unbounded tail classes), crossed with {false,
/// true}. Rows contribute REGIONS, not single points — a
/// band row on the Int key pins its whole band across the
/// Bool split wherever its Bool leaves also fail. A cell
/// is pinned exactly where every row firing there agrees
/// on the value; rows pinning one cell to two different
/// values are jointly inconsistent — not a table, no
/// seed. Cells no row decides stay unpinned and fall to
/// the wildcard: a partial table is admitted, never
/// forced.
///
/// The emitted body is one nested match in the pair
/// family's column discipline: over the Int key first
/// (literal patterns, span order), then over the Bool key
/// inside any column whose pinned cells disagree. A
/// column whose pinned cells all agree renders that
/// literal directly, and a column whose pinned cells all
/// equal the fall-through value needs no arm at all. The
/// wildcards carry the pinned fall-through value when the
/// cells falling through to them are value-uniform — the
/// outer wildcard over the tail columns and unpinned
/// columns, the inner over the Bool cell it does not arm —
/// else the typed-literal default. The raw builder stops
/// at constructing the match; admission is the seed
/// builder's step.
fn mixed_pair_guard_table_body(spec: &FunctionSpec) -> Option<IRNode> {
    if spec.params.len() != 2 {
        return None;
    }
    // The key signature: exactly one Int-family key and
    // exactly one Bool key, in either signature order.
    let (int_key, bool_key) = match (
        is_int_family(&spec.params[0].ty),
        matches!(spec.params[0].ty, TypeRef::Bool),
        is_int_family(&spec.params[1].ty),
        matches!(spec.params[1].ty, TypeRef::Bool),
    ) {
        (true, false, false, true) => (&spec.params[0], &spec.params[1]),
        (false, true, true, false) => (&spec.params[1], &spec.params[0]),
        _ => return None,
    };
    let mut rows: Vec<MixedGuardRow> = Vec::new();
    let mut constants: Vec<i64> = Vec::new();
    for c in &spec.postconditions {
        let Some((pin_side, guard_leaves)) = guard_row_parts(c) else {
            continue; // not a row: skipped, as in the other guard builders
        };
        // Row-shaped: it must be a mixed pair row, or the
        // spec states a row this body would not honor — a
        // deviation, so no seed.
        let pin = expr_to_ir(pin_side);
        if !matches!(pin, IRNode::Literal(_)) {
            return None; // the pin test: literal pins only
        }
        let mut int_leaves: Vec<(BinOp, bool, i64)> = Vec::new();
        let mut bool_leaves: Vec<(BinOp, bool, bool)> = Vec::new();
        for leaf in guard_leaves {
            if let Some((op, on_left, constant)) = guard_leaf(leaf, &int_key.name) {
                int_leaves.push((op, on_left, constant));
                constants.push(constant);
            } else if let Some((op, on_left, constant)) = guard_leaf_bool(leaf, &bool_key.name) {
                bool_leaves.push((op, on_left, constant));
            } else {
                return None; // a guard leaf on neither key
            }
        }
        rows.push(MixedGuardRow {
            int_leaves,
            bool_leaves,
            pin,
        });
    }
    if rows.is_empty() {
        return None;
    }
    // The leaves must range over both keys.
    let uses_int = rows.iter().any(|r| !r.int_leaves.is_empty());
    let uses_bool = rows.iter().any(|r| !r.bool_leaves.is_empty());
    if !uses_int || !uses_bool {
        return None;
    }
    // The Int candidate span (see the doc comment): the
    // leaf constants' range widened by two on each side.
    // Constants at the i64 extremes leave no room for the
    // widening — refused rather than scanned wrongly.
    let lo = constants.iter().min().copied()?.checked_sub(2)?;
    let hi = constants.iter().max().copied()?.checked_add(2)?;
    if (hi as i128 - lo as i128 + 1) * 2 > 100_000 {
        return None; // a span too wide for the joint scan to state
    }
    // Joint falsify-every-leaf over the mixed grid (see
    // the doc comment): a cell is pinned where every row
    // whose guard leaves all fail there agrees on the
    // value.
    let mut pinned: HashMap<(i64, bool), IRNode> = HashMap::new();
    for a in lo..=hi {
        for b in [false, true] {
            let mut cell: Option<&IRNode> = None;
            for row in &rows {
                let falsified = row
                    .int_leaves
                    .iter()
                    .all(|g| !guard_leaf_holds(g, a))
                    && row
                        .bool_leaves
                        .iter()
                        .all(|g| !guard_leaf_bool_holds(g, b));
                if falsified {
                    match cell {
                        None => cell = Some(&row.pin),
                        Some(p) if literal_pin_eq(p, &row.pin) => {}
                        Some(_) => return None, // conflicting pins: not a table
                    }
                }
            }
            if let Some(pin) = cell {
                pinned.insert((a, b), pin.clone());
            }
        }
    }
    if pinned.is_empty() {
        return None;
    }
    let default = typed_literal_fillings(&spec.return_type)?
        .into_iter()
        .next()?;
    let column = |a: i64| -> Vec<(bool, &IRNode)> {
        [false, true]
            .into_iter()
            .filter_map(|b| pinned.get(&(a, b)).map(|p| (b, p)))
            .collect()
    };
    // The outer wildcard's value: the pinned cells that
    // fall through to it — the tail columns and the
    // columns no row pins — must agree on it.
    let mut fall: Vec<&IRNode> = Vec::new();
    for a in lo..=hi {
        let col = column(a);
        if a == lo || a == hi || col.is_empty() {
            for (_, p) in col {
                if !fall.iter().any(|q| literal_pin_eq(q, p)) {
                    fall.push(p);
                }
            }
        }
    }
    if fall.len() > 1 {
        return None; // disagreeing fall-through pins: not this body's shape
    }
    let outer_default = fall
        .first()
        .map(|p| (*p).clone())
        .unwrap_or_else(|| default.clone());
    let mut cases: Vec<(IRPattern, IRNode)> = Vec::new();
    for a in lo..=hi {
        if a == lo || a == hi {
            continue; // the tail columns fall through to the wildcard
        }
        let col = column(a);
        if col.is_empty() || col.iter().all(|(_, p)| literal_pin_eq(p, &outer_default)) {
            continue; // unpinned, or the fall-through already yields its value
        }
        let first = col[0].1.clone();
        let body = if col.iter().all(|(_, p)| literal_pin_eq(p, &first)) {
            first // a constant column needs no inner match
        } else {
            // The column's pinned cells disagree — with a
            // two-value Bool axis that means both cells are
            // pinned to different values. The inner
            // wildcard carries the false cell's pin (the
            // fall-through value of the cell it does not
            // arm); the true cell is armed against it.
            let inner_default = col
                .iter()
                .find(|(b, _)| !*b)
                .map(|(_, p)| (*p).clone())
                .unwrap_or_else(|| default.clone());
            let mut inner: Vec<(IRPattern, IRNode)> = col
                .iter()
                .filter(|(_, p)| !literal_pin_eq(p, &inner_default))
                .map(|(b, p)| (IRPattern::Literal(IRLiteral::Bool(*b)), (*p).clone()))
                .collect();
            inner.push((IRPattern::Wildcard, inner_default));
            IRNode::Match {
                scrutinee: Box::new(IRNode::Var(bool_key.name.clone())),
                cases: inner,
            }
        };
        cases.push((IRPattern::Literal(IRLiteral::Int(a)), body));
    }
    if cases.is_empty() {
        return None; // every pinned cell rides the wildcard: a constant, not a table
    }
    cases.push((IRPattern::Wildcard, outer_default));
    Some(IRNode::Match {
        scrutinee: Box::new(IRNode::Var(int_key.name.clone())),
        cases,
    })
}

/// The mixed pair-keyed guard-table seed's body: the
/// nested match, admitted when the verifier can encode it
/// in the module context. No call resolution step (the
/// pair-keyed seed's admission precedent): the pins are
/// literals, so the body contains no spec calls to
/// resolve.
fn mixed_pair_guard_table_seed_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let body = mixed_pair_guard_table_body(spec)?;
    if crate::verify::verifier::body_is_encodable_with(spec, &body, index) {
        Some(body)
    } else {
        None
    }
}

/// Are two guard expressions STRUCTURAL complements — one
/// holds exactly where the other fails, by syntax alone?
/// The admitted pairs: `Not(G)` against `G` (either side),
/// and a complementary comparison — Lt/Ge, Le/Gt, Eq/Neq,
/// in either order — over `expr_same` operands IN THE SAME
/// ORDER. Anything else (swapped operands, different
/// operands, a non-comparison shape) is not a complement
/// this module will state: complement detection stays
/// syntactic and exact (Phase 80 remainder (i), whose
/// pricing names the mispaired-denotation risk: a wrong
/// complement would change what a call MEANS in
/// verification, so the pattern is deliberately narrower
/// than most standing builders).
fn guards_complementary(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::UnaryOp(UnaryOp::Not, inner), other)
        | (other, Expr::UnaryOp(UnaryOp::Not, inner)) => expr_same(inner.as_ref(), other),
        (Expr::BinOp(o1, l1, r1), Expr::BinOp(o2, l2, r2)) => {
            let complementary = matches!(
                (o1, o2),
                (BinOp::Lt, BinOp::Ge)
                    | (BinOp::Ge, BinOp::Lt)
                    | (BinOp::Le, BinOp::Gt)
                    | (BinOp::Gt, BinOp::Le)
                    | (BinOp::Eq, BinOp::Neq)
                    | (BinOp::Neq, BinOp::Eq)
            );
            complementary && expr_same(l1, l2) && expr_same(r1, r2)
        }
        _ => false,
    }
}

/// `len(x)` in a spec expression reaches the IR as a
/// `Call` to the builtin — the form the verifier's `len`
/// arm and the Phase 76 mirror body both use. The parser
/// may state it as `Expr::Length`, which `expr_to_ir`
/// lowers to a Hole; the Phase 81 builders therefore
/// rewrite `Length` to the call form before lowering, so
/// their denotations carry no hole for a stated length.
/// Every other shape passes through untouched.
fn len_as_call(expr: &Expr) -> Expr {
    match expr {
        Expr::Length(inner) => Expr::Call("len".into(), vec![len_as_call(inner)]),
        Expr::Field(inner, f) => Expr::Field(Box::new(len_as_call(inner)), f.clone()),
        Expr::Index(c, i) => Expr::Index(Box::new(len_as_call(c)), Box::new(len_as_call(i))),
        Expr::Call(n, args) => Expr::Call(
            n.clone(),
            args.iter().map(len_as_call).collect(),
        ),
        Expr::BinOp(op, l, r) => Expr::BinOp(
            *op,
            Box::new(len_as_call(l)),
            Box::new(len_as_call(r)),
        ),
        Expr::UnaryOp(op, inner) => Expr::UnaryOp(*op, Box::new(len_as_call(inner))),
        Expr::Lambda(params, body) => {
            Expr::Lambda(params.clone(), Box::new(len_as_call(body)))
        }
        Expr::Comprehension {
            binder,
            collection,
            body,
        } => Expr::Comprehension {
            binder: binder.clone(),
            collection: Box::new(len_as_call(collection)),
            body: Box::new(len_as_call(body)),
        },
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => Expr::MethodCall {
            receiver: Box::new(len_as_call(receiver)),
            name: name.clone(),
            args: args.iter().map(len_as_call).collect(),
        },
        Expr::RecordLit { type_name, fields } => Expr::RecordLit {
            type_name: type_name.clone(),
            fields: fields
                .iter()
                .map(|(n, v)| (n.clone(), len_as_call(v)))
                .collect(),
        },
        leaf => leaf.clone(),
    }
}

/// Lower a spec expression for a Phase 81 denotation: the
/// `len` rewrite, then the standing `expr_to_ir`. None when
/// the lowered tree carries a Hole — a holed denotation is
/// not the spec's body (the `equation_rhs` rule).
fn denotation_ir(expr: &Expr) -> Option<IRNode> {
    let body = expr_to_ir(&len_as_call(expr));
    if count_holes(&body) > 0 {
        None
    } else {
        Some(body)
    }
}

/// One complementary-guard row read as (guard, pin): a
/// postcondition that is an Or of exactly two leaves, one
/// an `output == E` pin (either orientation) and the other
/// the guard. Neither side may mention `output` (the guard
/// states an input condition; a pin mentioning `output`
/// is not a value the body can denote). Any other shape —
/// not an Or, not exactly two leaves, two pins, no pin, a
/// `Constraint` that is not an `Expr` — is not a row of
/// this family.
fn complementary_row_parts(c: &Constraint) -> Option<(&Expr, &Expr)> {
    let Constraint::Expr(expr) = c else {
        return None;
    };
    let mut leaves = Vec::new();
    flatten_or(expr, &mut leaves);
    if leaves.len() != 2 {
        return None;
    }
    let mut guard: Option<&Expr> = None;
    let mut pin: Option<&Expr> = None;
    for leaf in leaves {
        if let Expr::BinOp(BinOp::Eq, l, r) = leaf {
            let side = if matches!(l.as_ref(), Expr::Var(v) if v == OUTPUT_VAR) {
                Some(r.as_ref())
            } else if matches!(r.as_ref(), Expr::Var(v) if v == OUTPUT_VAR) {
                Some(l.as_ref())
            } else {
                None
            };
            if let Some(side) = side {
                if pin.is_some() {
                    return None; // two output pins: ambiguous
                }
                pin = Some(side);
                continue;
            }
        }
        if guard.is_some() {
            return None; // two guard leaves: not this family's row
        }
        guard = Some(leaf);
    }
    let (guard, pin) = (guard?, pin?);
    if expr_mentions_var(guard, OUTPUT_VAR) || expr_mentions_var(pin, OUTPUT_VAR) {
        return None;
    }
    Some((guard, pin))
}

/// The complementary-guard body (Phase 81, remainder (i)
/// of the Phase 80 gate re-run): a spec whose
/// postconditions are EXACTLY two rows `G || output == E`
/// with structurally complementary guards denotes the
/// conditional `If(G1, E2, E1)` — where G1 holds, the
/// first row is satisfied vacuously and the second row's
/// pin must hold; where G1 fails, the first row's pin
/// must. The measured shape is the tokenizer's `peek`:
/// `state.pos<len(state.source)||output==None()` and
/// `state.pos>=len(state.source)||output==Some(state.source[state.pos])`
/// — two rows over a computed comparison guard, neither an
/// equation nor a key-indexed guard table, which is why
/// every earlier builder returned None for it and
/// `next_char`'s postconditions (which call `peek(state)`)
/// could not be inlined by the verifier.
///
/// Exactly two postconditions, both rows of this shape,
/// or no denotation: a third postcondition of ANY shape
/// is a statement this body would not honor, so the
/// builder declines rather than skipping it (the
/// deviation discipline of the guard families, tightened
/// to the whole spec because the denotation claims to be
/// the spec's whole meaning). Complement detection is
/// `guards_complementary` — syntactic and exact.
///
/// Registered at the END of the `SpecIndex::callee_body`
/// chain: it fires only where all eight predecessors
/// return None, so no existing denotation moves.
fn complementary_guard_body(spec: &FunctionSpec) -> Option<IRNode> {
    if spec.postconditions.len() != 2 {
        return None;
    }
    let (g1, e1) = complementary_row_parts(&spec.postconditions[0])?;
    let (g2, e2) = complementary_row_parts(&spec.postconditions[1])?;
    if !guards_complementary(g1, g2) {
        return None;
    }
    Some(IRNode::If {
        condition: Box::new(denotation_ir(g1)?),
        then_branch: Box::new(denotation_ir(e2)?),
        else_branch: Box::new(denotation_ir(e1)?),
    })
}

/// The complementary-guard seed's body: the denotation,
/// with any spec-to-spec calls resolved against the module
/// (the composition-guard seed's admission precedent),
/// admitted when the verifier can encode the resolved body
/// in the module context. A body calling NO indexed spec
/// needs no resolution; where `inline_calls` refuses it on
/// shape grounds alone it is admitted raw — the
/// encodability gate is the admission standard either way.
fn complementary_guard_seed_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let raw = complementary_guard_body(spec)?;
    let body = match index.inline_calls(&raw) {
        Some(body) => body,
        None => {
            if body_calls_indexed(&raw, index) {
                return None;
            }
            raw
        }
    };
    if crate::verify::verifier::body_is_encodable_with(spec, &body, index) {
        Some(body)
    } else {
        None
    }
}

/// Does an IR body call any spec the index holds? (Builtin
/// and constructor calls — `len`, `Some`, `Ok` — are not
/// indexed specs and do not count.)
fn body_calls_indexed(node: &IRNode, index: &SpecIndex) -> bool {
    match node {
        IRNode::Call { function, args } => {
            index.get(function).is_some()
                || args.iter().any(|a| body_calls_indexed(a, index))
        }
        IRNode::BinOp(_, l, r) => {
            body_calls_indexed(l, index) || body_calls_indexed(r, index)
        }
        IRNode::UnaryOp(_, inner) => body_calls_indexed(inner, index),
        IRNode::If {
            condition,
            then_branch,
            else_branch,
        } => {
            body_calls_indexed(condition, index)
                || body_calls_indexed(then_branch, index)
                || body_calls_indexed(else_branch, index)
        }
        IRNode::Index(c, i) => {
            body_calls_indexed(c, index) || body_calls_indexed(i, index)
        }
        IRNode::FieldAccess { base, .. } => body_calls_indexed(base, index),
        IRNode::RecordLit { fields, .. } => {
            fields.iter().any(|(_, v)| body_calls_indexed(v, index))
        }
        IRNode::Match { scrutinee, cases } => {
            body_calls_indexed(scrutinee, index)
                || cases.iter().any(|(_, b)| body_calls_indexed(b, index))
        }
        _ => false,
    }
}

/// One record-completion row read as (guard leaf, field,
/// pin): an Or of exactly two leaves — one guard leaf and
/// one pin of the form `output.<field> == <rhs>` (either
/// orientation), the rhs not mentioning `output`. Any
/// other shape is not a row of this family.
fn record_completion_row_parts(c: &Constraint) -> Option<(&Expr, &str, &Expr)> {
    let Constraint::Expr(expr) = c else {
        return None;
    };
    let mut leaves = Vec::new();
    flatten_or(expr, &mut leaves);
    if leaves.len() != 2 {
        return None;
    }
    let mut guard: Option<&Expr> = None;
    let mut pin: Option<(&str, &Expr)> = None;
    for leaf in leaves {
        if let Expr::BinOp(BinOp::Eq, l, r) = leaf {
            let side = match (l.as_ref(), r.as_ref()) {
                (Expr::Field(base, f), rhs)
                    if matches!(base.as_ref(), Expr::Var(v) if v == OUTPUT_VAR) =>
                {
                    Some((f.as_str(), rhs))
                }
                (rhs, Expr::Field(base, f))
                    if matches!(base.as_ref(), Expr::Var(v) if v == OUTPUT_VAR) =>
                {
                    Some((f.as_str(), rhs))
                }
                _ => None,
            };
            if let Some(side) = side {
                if pin.is_some() {
                    return None; // two field pins in one row
                }
                pin = Some(side);
                continue;
            }
        }
        if guard.is_some() {
            return None; // two guard leaves: not this family's row
        }
        guard = Some(leaf);
    }
    let (guard, (field, rhs)) = (guard?, pin?);
    if expr_mentions_var(guard, OUTPUT_VAR) || expr_mentions_var(rhs, OUTPUT_VAR) {
        return None;
    }
    Some((guard, field, rhs))
}

/// An unconditional field equation read as (field, rhs):
/// a postcondition that is exactly `output.<field> ==
/// <rhs>` (either orientation), the rhs not mentioning
/// `output`. (An Or postcondition is a row, not an
/// equation, and is refused here by shape.)
fn record_completion_equation(c: &Constraint) -> Option<(&str, &Expr)> {
    let Constraint::Expr(expr) = c else {
        return None;
    };
    let Expr::BinOp(BinOp::Eq, l, r) = expr else {
        return None;
    };
    let (field, rhs) = match (l.as_ref(), r.as_ref()) {
        (Expr::Field(base, f), rhs)
            if matches!(base.as_ref(), Expr::Var(v) if v == OUTPUT_VAR) =>
        {
            (f.as_str(), rhs)
        }
        (rhs, Expr::Field(base, f))
            if matches!(base.as_ref(), Expr::Var(v) if v == OUTPUT_VAR) =>
        {
            (f.as_str(), rhs)
        }
        _ => return None,
    };
    if expr_mentions_var(rhs, OUTPUT_VAR) {
        return None;
    }
    Some((field, rhs))
}

/// The record-completion body (Phase 81, remainder (i-b)
/// of the Phase 80 gate re-run): a seed that builds a record
/// construction from a spec's partial field equations, so
/// a spec like the tokenizer's `next_char` can be
/// CROWNED, not only hand-verified. Phase 92 dual-registers
/// it as a `callee_body` denotation (appended at the END
/// of the chain, after the complementary-guard entry, so
/// it fires only where all nine predecessors decline):
/// the same construction is next_char's resolver meaning,
/// which is what lets bodies and rows that CALL next_char
/// be judged at all (the design doc's finding 2 — before
/// Phase 92 the builder was seed-only and next_char
/// denoted nowhere).
///
/// The exact rule. The spec's return type must resolve
/// (through the index's declarations) to a record type R,
/// and exactly one parameter — the base — must have that
/// same record type. Every postcondition must classify as
/// one of:
///   - an unconditional field equation `output.f == rhs`;
///   - a guard row `G' || output.f == rhs` whose single
///     guard leaf is either `expr_same` to the family's
///     one guard G or its structural complement
///     (`guards_complementary`). The first row's leaf
///     defines G. A row's pin holds where its guard leaf
///     FAILS: a leaf `expr_same` to G pins its field on
///     the G-false side; a complementary leaf pins on the
///     G-true side.
/// At least one unconditional equation AND at least one
/// guard row are required (a spec with only equations is
/// an equation-family shape stated fieldwise; a spec with
/// only rows states no unconditional field).
///
/// The constructed body is `R { … }` over the DECLARED
/// fields, in declared order. Each field's value:
///   - its unconditional rhs, if it has one;
///   - `If(G, rhs_true, rhs_false)` when pinned on both
///     sides;
///   - `If(G, rhs, base.f)` / `If(G, base.f, rhs)` when
///     pinned on one side only — the unpinned side CARRIES
///     the base record's field, which is the rule the
///     module's own spec comment states ("Fields no row
///     pins … are carried over unchanged");
///   - `base.f` (carried) when no equation names it.
///
/// Decline conditions (any one returns None): the return
/// type is not a declared record, or the index carries no
/// declarations; the base parameter is not unique; a
/// postcondition classifies as neither shape; a pin names
/// a field R does not declare; a field is pinned twice on
/// the same side (or unconditionally) with non-`expr_same`
/// right-hand sides; a field carries both an unconditional
/// equation and guard pins; a guard leaf is neither
/// `expr_same` to G nor its complement; a guard or rhs
/// mentions `output`; any lowered piece carries a Hole.
/// The measured shape is `next_char`: `output.pos ==
/// state.pos + 1` unconditionally; `line` pinned on the
/// newline side only (carried otherwise); `col` pinned on
/// both sides (`1` at a newline, `state.col + 1` past
/// one); `source`/`tokens` carried — the mirror body,
/// derived from the spec alone.
fn record_completion_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let TypeRef::Named(ret_name) = &spec.return_type else {
        return None;
    };
    let (resolved, declared) = index.record_fields(ret_name)?;
    let mut bases: Vec<&Param> = Vec::new();
    for p in &spec.params {
        if let TypeRef::Named(n) = &p.ty {
            if let Some((r, _)) = index.record_fields(n) {
                if r == resolved {
                    bases.push(p);
                }
            }
        }
    }
    if bases.len() != 1 {
        return None;
    }
    let base = bases[0];
    let declared_names: Vec<&str> = declared.iter().map(|(n, _)| n.as_str()).collect();

    let mut uncond: HashMap<&str, &Expr> = HashMap::new();
    let mut on_true: HashMap<&str, &Expr> = HashMap::new();
    let mut on_false: HashMap<&str, &Expr> = HashMap::new();
    let mut guard: Option<&Expr> = None;
    let mut n_equations = 0usize;
    let mut n_rows = 0usize;
    for c in &spec.postconditions {
        if let Some((field, rhs)) = record_completion_equation(c) {
            if !declared_names.contains(&field) {
                return None;
            }
            if on_true.contains_key(field) || on_false.contains_key(field) {
                return None; // unconditional AND guarded: two statements of one field
            }
            match uncond.get(field) {
                Some(existing) if !expr_same(existing, rhs) => return None,
                Some(_) => {}
                None => {
                    uncond.insert(field, rhs);
                }
            }
            n_equations += 1;
            continue;
        }
        if let Some((leaf, field, rhs)) = record_completion_row_parts(c) {
            if !declared_names.contains(&field) {
                return None;
            }
            if uncond.contains_key(field) {
                return None; // unconditional AND guarded
            }
            let g = match guard {
                None => {
                    guard = Some(leaf);
                    leaf
                }
                Some(g) => g,
            };
            let slot = if expr_same(leaf, g) {
                &mut on_false // the pin holds where the leaf fails
            } else if guards_complementary(leaf, g) {
                &mut on_true // the pin holds where G holds
            } else {
                return None; // a second, unrelated guard: not one guard's rows
            };
            match slot.get(field) {
                Some(existing) if !expr_same(existing, rhs) => return None,
                Some(_) => {}
                None => {
                    slot.insert(field, rhs);
                }
            }
            n_rows += 1;
            continue;
        }
        return None; // a postcondition of neither shape: this body would not honor it
    }
    if n_equations == 0 || n_rows == 0 {
        return None;
    }
    let guard_ir = denotation_ir(guard?)?;
    // Phase 87: when the one guard feeds two or more
    // fields' Ifs, bind it once in a Let and let every
    // If test the bound name. The Phase 85 shape
    // cloned the guard tree into each guarded field,
    // and for the measured shape (next_char: the
    // guard is the inlined peek) that evaluates peek —
    // a full source count + nth scan — once per
    // guarded field per call; the Phase 87 profile
    // priced the duplication at ≈0.78 of the ratio
    // on the big workload. The name must be fresh:
    // not a parameter, not `output`, and not
    // mentioned in any postcondition (every denotation
    // piece derives from those, so a name absent from
    // all three cannot be captured or shadowed into
    // a different meaning). A trivial guard (a bare
    // Var or Literal) is not worth a binding. If no
    // candidate name is fresh, the unshared body
    // stands.
    let guarded_fields = declared
        .iter()
        .filter(|(n, _)| {
            on_true.contains_key(n.as_str()) || on_false.contains_key(n.as_str())
        })
        .count();
    let shared_name: Option<String> =
        if guarded_fields >= 2 && !matches!(guard_ir, IRNode::Var(_) | IRNode::Literal(_)) {
            ["__rc_guard", "__rc_guard_2", "__rc_guard_3"]
                .iter()
                .find(|cand| {
                    spec.params.iter().all(|p| p.name != **cand)
                        && **cand != OUTPUT_VAR
                        && spec.postconditions.iter().all(|c| {
                            !matches!(c, Constraint::Expr(e) if expr_mentions_var(e, cand))
                        })
                })
                .map(|s| s.to_string())
        } else {
            None
        };
    let cond_ir: IRNode = match &shared_name {
        Some(n) => IRNode::Var(n.clone()),
        None => guard_ir.clone(),
    };
    let carry = |field: &str| IRNode::FieldAccess {
        base: Box::new(IRNode::Var(base.name.clone())),
        field: field.into(),
    };
    let mut fields: Vec<(String, IRNode)> = Vec::with_capacity(declared.len());
    for (name, _) in &declared {
        let value = if let Some(rhs) = uncond.get(name.as_str()) {
            denotation_ir(rhs)?
        } else {
            match (on_true.get(name.as_str()), on_false.get(name.as_str())) {
                (Some(t), Some(f)) => IRNode::If {
                    condition: Box::new(cond_ir.clone()),
                    then_branch: Box::new(denotation_ir(t)?),
                    else_branch: Box::new(denotation_ir(f)?),
                },
                (Some(t), None) => IRNode::If {
                    condition: Box::new(cond_ir.clone()),
                    then_branch: Box::new(denotation_ir(t)?),
                    else_branch: Box::new(carry(name)),
                },
                (None, Some(f)) => IRNode::If {
                    condition: Box::new(cond_ir.clone()),
                    then_branch: Box::new(carry(name)),
                    else_branch: Box::new(denotation_ir(f)?),
                },
                (None, None) => carry(name),
            }
        };
        fields.push((name.clone(), value));
    }
    let record = IRNode::RecordLit {
        type_name: resolved,
        fields,
    };
    Some(match shared_name {
        Some(n) => IRNode::Let {
            name: n,
            value: Box::new(guard_ir),
            body: Box::new(record),
        },
        None => record,
    })
}

/// The record-completion seed's body: the construction,
/// with spec-to-spec calls resolved against the module —
/// resolution is REQUIRED here (the guard of the measured
/// shape calls `peek(state)`, and a raw call body could
/// not be executed by the interpreter or emitted
/// standalone: the composition-guard seed's precedent) —
/// admitted when the verifier can encode the resolved body
/// in the module context.
fn record_completion_seed_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let raw = record_completion_body(spec, index)?;
    let body = index.inline_calls(&raw)?;
    if crate::verify::verifier::body_is_encodable_with(spec, &body, index) {
        Some(body)
    } else {
        None
    }
}

/// Follow alias/newtype names on a type reference to
/// the type it denotes (bounded, cycle-safe). The
/// tokenize template's keying reads signatures through
/// the module's own aliases (`StepResult` for the
/// step's Result, `TokenKind` for the token's kind).
fn tokenize_resolve_alias(index: &SpecIndex, ty: &TypeRef) -> TypeRef {
    let mut cur = ty.clone();
    let mut seen = std::collections::HashSet::new();
    while let TypeRef::Named(n) = &cur {
        if !seen.insert(n.clone()) {
            break;
        }
        match index.types.get(n).map(|td| &td.kind) {
            Some(TypeKind::Alias(t)) | Some(TypeKind::Newtype(t)) => {
                cur = t.clone()
            }
            _ => break,
        }
    }
    cur
}

/// Whether an expression tree mentions a method call
/// of any of the given names (the tokenize rows'
/// shape evidence: the is_err/is_ok split and the
/// unwrap projection, as the Phase 84 rows state
/// them).
fn tokenize_expr_has_method(e: &Expr, names: &[&str]) -> bool {
    match e {
        Expr::MethodCall { receiver, name, args } => {
            names.contains(&name.as_str())
                || tokenize_expr_has_method(receiver, names)
                || args.iter().any(|a| tokenize_expr_has_method(a, names))
        }
        Expr::BinOp(_, l, r) => {
            tokenize_expr_has_method(l, names)
                || tokenize_expr_has_method(r, names)
        }
        Expr::UnaryOp(_, x) => tokenize_expr_has_method(x, names),
        Expr::Call(_, args) => {
            args.iter().any(|a| tokenize_expr_has_method(a, names))
        }
        Expr::Index(collection, index) => {
            tokenize_expr_has_method(collection, names)
                || tokenize_expr_has_method(index, names)
        }
        Expr::Field(object, _) => tokenize_expr_has_method(object, names),
        Expr::Length(x) => tokenize_expr_has_method(x, names),
        Expr::Lambda(_, body) => tokenize_expr_has_method(body, names),
        Expr::Comprehension {
            collection, body, ..
        } => {
            tokenize_expr_has_method(collection, names)
                || tokenize_expr_has_method(body, names)
        }
        Expr::RecordLit { fields, .. } => fields
            .iter()
            .any(|(_, v)| tokenize_expr_has_method(v, names)),
        _ => false,
    }
}

/// The tokenize rows' shape: some postcondition
/// mentions an is_err/is_ok method call (the Ok/Err
/// split) and some postcondition mentions unwrap
/// (the payload projection) — the amended rows'
/// joint shape, and the only constraint evidence
/// the template keys on.
fn tokenize_rows_shape(spec: &FunctionSpec) -> bool {
    let mut split = false;
    let mut unwrap = false;
    for c in &spec.postconditions {
        if let Constraint::Expr(e) = c {
            split |= tokenize_expr_has_method(e, &["is_err", "is_ok"]);
            unwrap |= tokenize_expr_has_method(e, &["unwrap"]);
        }
    }
    split && unwrap
}

/// The tokenizer-state shape the template keys on:
/// the record R a spec's single parameter carries,
/// its token record T, and the two declarations'
/// names — or None when the shape is not the
/// tokenizer's. Field evidence, all of it: R declares
/// source: String, pos/line/col: Int and
/// tokens: List<T>; T declares kind: Int (through
/// aliases), lexeme: String, line/col: Int.
struct TokenizeShape {
    state_decl: String,
    token_decl: String,
    state_fields: Vec<(String, TypeRef)>,
    token_fields: Vec<(String, TypeRef)>,
}

fn tokenize_shape(spec: &FunctionSpec, index: &SpecIndex) -> Option<TokenizeShape> {
    if spec.params.len() != 1 {
        return None;
    }
    let TypeRef::Named(state_name) = &spec.params[0].ty else {
        return None;
    };
    let (state_decl, state_fields) = index.record_fields(state_name)?;
    let field_ty = |fields: &[(String, TypeRef)], n: &str| {
        fields.iter().find(|(f, _)| f == n).map(|(_, t)| t.clone())
    };
    if field_ty(&state_fields, "source") != Some(TypeRef::String) {
        return None;
    }
    for f in ["pos", "line", "col"] {
        if field_ty(&state_fields, f) != Some(TypeRef::Int) {
            return None;
        }
    }
    let Some(TypeRef::List(tok_ty)) = field_ty(&state_fields, "tokens") else {
        return None;
    };
    let TypeRef::Named(tok_name) = tok_ty.as_ref() else {
        return None;
    };
    let (token_decl, token_fields) = index.record_fields(tok_name)?;
    let kind_ty = field_ty(&token_fields, "kind")?;
    if tokenize_resolve_alias(index, &kind_ty) != TypeRef::Int {
        return None;
    }
    if field_ty(&token_fields, "lexeme") != Some(TypeRef::String) {
        return None;
    }
    for f in ["line", "col"] {
        if field_ty(&token_fields, f) != Some(TypeRef::Int) {
            return None;
        }
    }
    Some(TokenizeShape {
        state_decl,
        token_decl,
        state_fields,
        token_fields,
    })
}

/// A callee's signature against the state shape:
/// one parameter of the state record, returning the
/// resolved type `ret` — the template's callees are
/// found by name (IR calls are by name; there is no
/// other way to call them) and admitted only when
/// their declared signatures are the tokenizer's.
fn tokenize_callee(
    index: &SpecIndex,
    name: &str,
    shape: &TokenizeShape,
    ret: &TypeRef,
) -> bool {
    let Some(f) = index.get(name) else {
        return false;
    };
    if f.params.len() != 1 {
        return false;
    }
    let TypeRef::Named(pn) = &f.params[0].ty else {
        return false;
    };
    let Some((pdecl, _)) = index.record_fields(pn) else {
        return false;
    };
    pdecl == shape.state_decl
        && tokenize_resolve_alias(index, &f.return_type) == *ret
}

/// A Char classifier callee's signature: one Char
/// parameter, Bool return.
fn tokenize_classifier(index: &SpecIndex, name: &str) -> bool {
    let Some(f) = index.get(name) else {
        return false;
    };
    f.params.len() == 1
        && f.params[0].ty == TypeRef::Char
        && tokenize_resolve_alias(index, &f.return_type) == TypeRef::Bool
}

/// The tokenize STEP body construction, shared by the
/// `tokenize_template` seed (below) and the Phase 92
/// step denotation (`tokenize_step_denotation_body`):
/// the step-shaped spec's body — skip, dispatch on the
/// current char, consume, stamp, close — built from
/// the shape alone. Returns None when the spec's
/// return is not the step return (Result<R, String>
/// over the shape's state record) or when any callee
/// the body calls is absent or mis-signed. The seed
/// adds its own admission gate (the rows' shape) on
/// top of this construction; the construction itself
/// keys on shape and signatures only.
fn tokenize_step_body(
    spec: &FunctionSpec,
    index: &SpecIndex,
    shape: &TokenizeShape,
) -> Option<IRNode> {
    let param = spec.params[0].name.clone();
    let state_ty = TypeRef::Named(shape.state_decl.clone());
    let ret = tokenize_resolve_alias(index, &spec.return_type);
    let v = |n: &str| IRNode::Var(n.to_string());
    let fa = |base: IRNode, field: &str| IRNode::FieldAccess {
        base: Box::new(base),
        field: field.to_string(),
    };
    let call = |f: &str, args: Vec<IRNode>| IRNode::Call {
        function: f.to_string(),
        args,
    };
    // The generated callees take the state record by
    // value: a call whose argument stays live after
    // the call must pass a clone (the UnaryOp Clone
    // rendering), or the generated code dies E0382.
    // A call that reinitializes its argument
    // (cur = next_char(cur)) moves it bare, legally.
    let vc = |n: &str| {
        IRNode::UnaryOp(IRUnaryOp::Clone, Box::new(IRNode::Var(n.to_string())))
    };
    let lit_i = |n: i64| IRNode::Literal(IRLiteral::Int(n));
    let lit_s = |s: &str| IRNode::Literal(IRLiteral::String(s.to_string()));
    // A record of the state shape built from `base`'s
    // fields, with the tokens field replaced.
    let state_record = |base: &str, tokens: IRNode| {
        let fields = shape
            .state_fields
            .iter()
            .map(|(n, _)| {
                let value = if n == "tokens" {
                    tokens.clone()
                } else {
                    fa(v(base), n)
                };
                (n.clone(), value)
            })
            .collect();
        IRNode::RecordLit {
            type_name: shape.state_decl.clone(),
            fields,
        }
    };
    // A token of the token shape stamped at the
    // post-skip state's position (D4a START). The
    // stamp reads are the caller's to supply: the
    // ident arm reads them off the post-skip state
    // directly (it stays live), the punctuation arms
    // bind them into locals first (below).
    let token_record = |kind: IRNode, lexeme: IRNode, stamp_line: IRNode, stamp_col: IRNode| {
        let fields = shape
            .token_fields
            .iter()
            .map(|(n, _)| {
                let value = match n.as_str() {
                    "kind" => kind.clone(),
                    "lexeme" => lexeme.clone(),
                    "line" => stamp_line.clone(),
                    "col" => stamp_col.clone(),
                    _ => lit_i(0),
                };
                (n.clone(), value)
            })
            .collect();
        IRNode::RecordLit {
            type_name: shape.token_decl.clone(),
            fields,
        }
    };

    let is_step_return = matches!(
        &ret,
        TypeRef::Result(ok, err)
            if matches!(tokenize_resolve_alias(index, ok.as_ref()), TypeRef::Named(n) if n == shape.state_decl)
                && tokenize_resolve_alias(index, err.as_ref()) == TypeRef::String
    );
    if is_step_return {
        // The step callees, by name + signature.
        if !tokenize_callee(index, "skip_whitespace", &shape, &state_ty)
            || !tokenize_callee(
                index,
                "peek",
                &shape,
                &TypeRef::Option(Box::new(TypeRef::Char)),
            )
            || !tokenize_callee(index, "next_char", &shape, &state_ty)
            || !tokenize_classifier(index, "is_ident_start")
            || !tokenize_classifier(index, "is_ident_continue")
        {
            return None;
        }
        // The single-character arm for one punctuation
        // char: consume it, stamp its kind, close.
        // Phase 87: the stamp (line/col) is bound into
        // locals BEFORE the consume, so `next_char` is
        // the post-skip state's last use and moves it
        // bare — the Phase 85 shape cloned the whole
        // state (String + token Vec) here because the
        // token record still read `s` after the call.
        let punct_arm = |kind: i64, ch: &str| {
            IRNode::Block(vec![
                IRNode::Alloc {
                    name: "stamp_line".to_string(),
                    ty: TypeRef::Int,
                    initial: Box::new(fa(v("s"), "line")),
                },
                IRNode::Alloc {
                    name: "stamp_col".to_string(),
                    ty: TypeRef::Int,
                    initial: Box::new(fa(v("s"), "col")),
                },
                IRNode::Alloc {
                    name: "end".to_string(),
                    ty: state_ty.clone(),
                    initial: Box::new(call("next_char", vec![v("s")])),
                },
                IRNode::Alloc {
                    name: "toks".to_string(),
                    ty: TypeRef::List(Box::new(TypeRef::Named(
                        shape.token_decl.clone(),
                    ))),
                    initial: Box::new(fa(v("end"), "tokens")),
                },
                IRNode::BinOp(
                    IRBinOp::Append,
                    Box::new(v("toks")),
                    Box::new(token_record(
                        lit_i(kind),
                        lit_s(ch),
                        v("stamp_line"),
                        v("stamp_col"),
                    )),
                ),
                IRNode::Return(Box::new(call(
                    "Ok",
                    vec![state_record("end", v("toks"))],
                ))),
            ])
        };
        // The identifier arm: consume while the
        // classifiers continue, accumulating the
        // lexeme a char at a time, then stamp kind 0
        // (IDENT). Phase 87: the loop reads the
        // current char by Index (the Phase 85 index
        // rendering: chars().nth off the state's own
        // source/pos, totalized to the NUL sentinel
        // out of range) instead of matching on
        // peek(cur.clone()) — the by-value peek cloned
        // the whole state on every char, and the
        // profile priced those clones as the step's
        // dominant remaining cost. The shapes agree
        // wherever the mirror's peek does: out of
        // range reads as the sentinel, which is not
        // an ident-continue char, so the loop breaks
        // at EOF exactly where peek's None breaks it;
        // a genuine NUL char breaks it too, exactly
        // where peek's Some('\0') fails the classifier.
        let ident_arm = IRNode::Block(vec![
            IRNode::Alloc {
                name: "lexeme".to_string(),
                ty: TypeRef::String,
                initial: Box::new(lit_s("")),
            },
            IRNode::Alloc {
                name: "cur".to_string(),
                ty: state_ty.clone(),
                initial: Box::new(vc("s")),
            },
            IRNode::Loop {
                // Phase 96 invariant (the verifier's
                // statement judge gives this text its
                // semantics; see `stmt_invariant_close`):
                // the frame + progress facts the step's
                // rows read off the consumed state —
                // next_char carries source and tokens
                // unchanged and advances pos, so all
                // three are preserved by construction
                // and the consecution discharges.
                invariant: Some(
                    "cur.source == s.source; len(cur.tokens) == len(s.tokens); cur.pos >= s.pos"
                        .to_string(),
                ),
                body: Box::new(IRNode::Block(vec![
                    IRNode::Alloc {
                        name: "ch".to_string(),
                        ty: TypeRef::Char,
                        initial: Box::new(IRNode::Index(
                            Box::new(fa(v("cur"), "source")),
                            Box::new(fa(v("cur"), "pos")),
                        )),
                    },
                    IRNode::If {
                        condition: Box::new(call(
                            "is_ident_continue",
                            vec![v("ch")],
                        )),
                        then_branch: Box::new(IRNode::Block(vec![
                            IRNode::BinOp(
                                IRBinOp::Append,
                                Box::new(v("lexeme")),
                                Box::new(v("ch")),
                            ),
                            IRNode::Assign {
                                target: Box::new(v("cur")),
                                value: Box::new(call(
                                    "next_char",
                                    vec![v("cur")],
                                )),
                            },
                        ])),
                        else_branch: Box::new(IRNode::Block(vec![IRNode::Break {
                            value: None,
                        }])),
                    },
                ])),
            },
            IRNode::Alloc {
                name: "toks".to_string(),
                ty: TypeRef::List(Box::new(TypeRef::Named(
                    shape.token_decl.clone(),
                ))),
                initial: Box::new(fa(v("cur"), "tokens")),
            },
            IRNode::BinOp(
                IRBinOp::Append,
                Box::new(v("toks")),
                Box::new(token_record(
                    lit_i(0),
                    v("lexeme"),
                    fa(v("s"), "line"),
                    fa(v("s"), "col"),
                )),
            ),
            IRNode::Return(Box::new(call(
                "Ok",
                vec![state_record("cur", v("toks"))],
            ))),
        ]);
        // The dispatch: ident first (its classifier
        // owns every ident-start char), then the
        // punctuation table, then the error arm.
        // The error arm binds its message in a typed
        // Alloc first: Call arguments render untyped,
        // so a bare literal would reach Err as a &str
        // where String is expected (E0308).
        let mut dispatch = IRNode::Block(vec![
            IRNode::Alloc {
                name: "msg".to_string(),
                ty: TypeRef::String,
                initial: Box::new(lit_s("Unexpected character")),
            },
            IRNode::Return(Box::new(call("Err", vec![v("msg")]))),
        ]);
        for (ch, kind) in [
            ('=', 12),
            ('>', 11),
            ('<', 10),
            (',', 9),
            (':', 7),
            ('}', 6),
            ('{', 5),
            (')', 4),
            ('(', 3),
        ] {
            dispatch = IRNode::If {
                condition: Box::new(IRNode::BinOp(
                    IRBinOp::Eq,
                    Box::new(v("c")),
                    Box::new(IRNode::Literal(IRLiteral::Char(ch))),
                )),
                then_branch: Box::new(punct_arm(kind, &ch.to_string())),
                else_branch: Box::new(dispatch),
            };
        }
        dispatch = IRNode::If {
            condition: Box::new(call("is_ident_start", vec![v("c")])),
            then_branch: Box::new(ident_arm),
            else_branch: Box::new(dispatch),
        };
        return Some(IRNode::Block(vec![
            IRNode::Alloc {
                name: "s".to_string(),
                ty: state_ty.clone(),
                initial: Box::new(call("skip_whitespace", vec![v(&param)])),
            },
            // Phase 87: the dispatch reads the current
            // char by Index under an explicit EOF test
            // instead of matching on peek(s.clone()) —
            // the by-value peek cloned the whole state
            // on every step (see the ident arm's note
            // for the agreement argument: the length
            // test takes the Ok arm exactly where
            // peek's None does, and a genuine NUL char
            // falls through every classifier and
            // punctuation test to the error arm,
            // exactly where peek's Some('\0') lands).
            // The `len` call's argument is the source
            // projection, so the Phase 85 len rendering
            // gives it the char-count form.
            IRNode::If {
                condition: Box::new(IRNode::BinOp(
                    IRBinOp::Ge,
                    Box::new(fa(v("s"), "pos")),
                    Box::new(call("len", vec![fa(v("s"), "source")])),
                )),
                then_branch: Box::new(IRNode::Block(vec![IRNode::Return(Box::new(
                    call("Ok", vec![v("s")]),
                ))])),
                else_branch: Box::new(IRNode::Block(vec![
                    IRNode::Alloc {
                        name: "c".to_string(),
                        ty: TypeRef::Char,
                        initial: Box::new(IRNode::Index(
                            Box::new(fa(v("s"), "source")),
                            Box::new(fa(v("s"), "pos")),
                        )),
                    },
                    dispatch,
                ])),
            },
        ]));
    }

    None
}

/// The tokenize template seed (Phase 85, family
/// `tokenize_template`): a hand-authored tokenizing
/// step — and, composed over it, the tokenizing loop
/// — for the one spec shape the Phase 80 record
/// priced as remainder (iii): a single state-record
/// parameter, the tokenizer's field shape, the
/// amended rows' is_err/is_ok + unwrap shape, and
/// the module's own step callees present with the
/// tokenizer's signatures.
///
/// STEP form (the spec returns Result<R, String>):
/// skip_whitespace, then a dispatch on the current
/// char — end of input is the Ok arm; an ident-start
/// char consumes an identifier through the
/// is_ident_continue loop; the seven punctuation
/// chars stamp their mirror kinds (LPAREN 3,
/// RPAREN 4, LBRACE 5, RBRACE 6, COLON 7, COMMA 9,
/// LT 10, GT 11, EQ 12 — the mirror's table, stated
/// here as data); anything else is the error arm.
/// (Phase 87: the dispatch reads the char by Index
/// under a length test rather than matching on a
/// by-value peek — see the body.) The token stamps
/// the post-skip state's line/col (the D4a START rule)
/// and the consumed state's fields close the step.
/// The error arm's message is the constant
/// "Unexpected character": the mirror's full text
/// ('Unexpected character '<c>' at line <n>') is not
/// expressible in IR — a Char reaches a String only
/// through Append, and the line number is an Int,
/// whose conversion to text is finding F2 — so the
/// constant is this template's one named deviation
/// from the mirror, itemized wherever the template
/// is executed against the mirror batteries.
///
/// LOOP form (the spec returns Result<List<T>,
/// String>): the step spec is found in the index by
/// the STEP keying itself and called by its name;
/// the loop threads the state through the step's Ok
/// payload (TryPropagate) and returns the final
/// state's tokens. Phase 87: the loop breaks on the
/// step's position progress (see the body) rather
/// than testing peek per token.
///
/// Admission standard — stated, not smuggled: the
/// bodies contain loops (While at Phase 85, Loop
/// since Phase 87), which the encoder has no
/// arm for, so no encodability gate can pass them;
/// like the char_equation family before the D5a
/// encoding, this family is admitted knowing Z3
/// cannot judge its bodies (their verdict is
/// Inconclusive by construction), and its evidence
/// standard is executed agreement against the
/// mirror batteries, measured in the cross-
/// validation harness. Names are load-bearing here
/// in exactly one place: the callees are called by
/// name because IR calls are by name — every other
/// key (fields, signatures, the rows' shape) is
/// structural.
fn tokenize_template_seed_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let shape = tokenize_shape(spec, index)?;
    if !tokenize_rows_shape(spec) {
        return None;
    }
    let param = spec.params[0].name.clone();
    let state_ty = TypeRef::Named(shape.state_decl.clone());
    let ret = tokenize_resolve_alias(index, &spec.return_type);
    let v = |n: &str| IRNode::Var(n.to_string());
    let fa = |base: IRNode, field: &str| IRNode::FieldAccess {
        base: Box::new(base),
        field: field.to_string(),
    };
    let call = |f: &str, args: Vec<IRNode>| IRNode::Call {
        function: f.to_string(),
        args,
    };
    if let Some(step) = tokenize_step_body(spec, index, &shape) {
        return Some(step);
    }

    let is_loop_return = matches!(
        &ret,
        TypeRef::Result(ok, err)
            if matches!(tokenize_resolve_alias(index, ok.as_ref()), TypeRef::List(t) if matches!(tokenize_resolve_alias(index, t.as_ref()), TypeRef::Named(n) if n == shape.token_decl))
                && tokenize_resolve_alias(index, err.as_ref()) == TypeRef::String
    );
    if is_loop_return {
        if !tokenize_callee(
            index,
            "peek",
            &shape,
            &TypeRef::Option(Box::new(TypeRef::Char)),
        ) {
            return None;
        }
        // The step spec: any OTHER spec in the index
        // the STEP keying itself admits (same shape,
        // step return, rows' shape, callees present) —
        // called by its found name.
        let mut step_candidates: Vec<&FunctionSpec> = index
            .by_name
            .values()
            .filter(|f| {
                f.name != spec.name
                    && tokenize_shape(f, index).is_some_and(|s2| {
                        s2.state_decl == shape.state_decl
                            && s2.token_decl == shape.token_decl
                    })
                    && tokenize_rows_shape(f)
                    && matches!(
                        tokenize_resolve_alias(index, &f.return_type),
                        TypeRef::Result(ref ok, ref err)
                            if matches!(tokenize_resolve_alias(index, ok.as_ref()), TypeRef::Named(ref n) if *n == shape.state_decl)
                                && tokenize_resolve_alias(index, err.as_ref()) == TypeRef::String
                    )
            })
            .collect();
        step_candidates.sort_by(|a, b| a.name.cmp(&b.name));
        let step_name = step_candidates.first().map(|f| f.name.clone())?;
        return Some(IRNode::Block(vec![
            IRNode::Alloc {
                name: "s".to_string(),
                ty: state_ty.clone(),
                initial: Box::new(v(&param)),
            },
            // Phase 87: the loop runs on a position-
            // progress break instead of the Phase 85
            // While over `peek(s.clone()) != None` —
            // that condition cloned the whole state
            // (String + the growing token Vec) and ran
            // peek's source scan once per token, and
            // the profile priced the clone as the loop
            // form's dominant cost. The break is the
            // same EOF test stated on the step's own
            // contract: the step this builder keys on
            // returns Ok with the position advanced
            // exactly when it appended a token (its
            // ident/punctuation arms consume a char;
            // its EOF arm — reached after the skip, so
            // trailing whitespace may still advance the
            // position once, into the final no-progress
            // step — changes nothing). A step that
            // leaves the position unchanged is the EOF
            // step; errors leave through TryPropagate
            // before the test. (The peek-callee
            // requirement above stands: the step this
            // loop drives still keys on peek.)
            IRNode::Alloc {
                name: "prev".to_string(),
                ty: TypeRef::Int,
                initial: Box::new(fa(v("s"), "pos")),
            },
            IRNode::Loop {
                // Phase 96 invariant: the token-
                // accumulation frame the spec's one row
                // reads — the step never shrinks the
                // token list, so its length stays at or
                // above the input's at every loop head.
                invariant: Some(format!("len(s.tokens) >= len({}.tokens)", param)),
                body: Box::new(IRNode::Block(vec![
                    IRNode::Alloc {
                        name: "step".to_string(),
                        ty: state_ty.clone(),
                        initial: Box::new(IRNode::TryPropagate(Box::new(call(
                            &step_name,
                            vec![v("s")],
                        )))),
                    },
                    IRNode::Assign {
                        target: Box::new(v("s")),
                        value: Box::new(v("step")),
                    },
                    IRNode::If {
                        condition: Box::new(IRNode::BinOp(
                            IRBinOp::Eq,
                            Box::new(fa(v("s"), "pos")),
                            Box::new(v("prev")),
                        )),
                        then_branch: Box::new(IRNode::Block(vec![IRNode::Break {
                            value: None,
                        }])),
                        else_branch: Box::new(IRNode::Block(vec![])),
                    },
                    IRNode::Assign {
                        target: Box::new(v("prev")),
                        value: Box::new(fa(v("s"), "pos")),
                    },
                ])),
            },
            IRNode::Return(Box::new(call(
                "Ok",
                vec![fa(v("s"), "tokens")],
            ))),
        ]));
    }
    None
}

/// The keyed skip denotation (Phase 92, the design
/// doc's §2.6 item 2): the meaning of the tokenizer
/// module's state→state slot, supplied — not derived.
///
/// Why keyed, never derived: skip's two rows are
/// satisfied by the identity body and by every partial
/// skip, so no body can be DERIVED from them; a builder
/// that returned "a body the rows admit" would denote
/// the identity and call it skip, which is the α
/// failure wearing a resolver costume. This builder
/// instead recognizes a STRUCTURAL slot — a single
/// parameter of the tokenizer's state-record shape
/// (tokenize_shape), a return of that same record, and
/// the three callees the body calls present with the
/// tokenizer's signatures (peek, next_char,
/// is_whitespace) — and returns the mirror-faithful
/// body for that slot: the Phase 85 hand transcription
/// (Alloc cur; While over a peek Match; Assign cur =
/// next_char(cur); Return cur), parameterized by the
/// spec's own parameter name and the shape's declared
/// names. The rows are never consulted: the slot is
/// the key, exactly as the template family's field and
/// signature keys are. Registered in the callee_body
/// chain AFTER every row-derived builder, so it fires
/// only for a state→state spec no row builder denotes
/// — in the measured module that is skip_whitespace
/// alone (next_char denotes through record completion,
/// two entries earlier).
///
/// The body is statement-mode (While): today's
/// sym_eval cannot encode it, so a call to skip still
/// judges Inconclusive — the denotation makes the
/// meaning AVAILABLE (to the Phase 90 BMC discipline
/// now, to the statement-mode encoder in Phase 93),
/// it does not manufacture a verdict.
fn skip_denotation_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    let shape = tokenize_shape(spec, index)?;
    let ret = tokenize_resolve_alias(index, &spec.return_type);
    if !matches!(&ret, TypeRef::Named(n) if n == &shape.state_decl) {
        return None;
    }
    let state_ty = TypeRef::Named(shape.state_decl.clone());
    if !tokenize_callee(
        index,
        "peek",
        &shape,
        &TypeRef::Option(Box::new(TypeRef::Char)),
    ) || !tokenize_callee(index, "next_char", &shape, &state_ty)
        || !tokenize_classifier(index, "is_whitespace")
    {
        return None;
    }
    let param = spec.params[0].name.as_str();
    let v = |n: &str| IRNode::Var(n.to_string());
    let call = |f: &str, args: Vec<IRNode>| IRNode::Call {
        function: f.to_string(),
        args,
    };
    Some(IRNode::Block(vec![
        IRNode::Alloc {
            name: "cur".to_string(),
            ty: state_ty,
            initial: Box::new(v(param)),
        },
        IRNode::While {
            condition: Box::new(IRNode::Match {
                scrutinee: Box::new(call(
                    "peek",
                    vec![IRNode::UnaryOp(
                        IRUnaryOp::Clone,
                        Box::new(v("cur")),
                    )],
                )),
                cases: vec![
                    (
                        IRPattern::Constructor("None".to_string(), vec![]),
                        IRNode::Literal(IRLiteral::Bool(false)),
                    ),
                    (
                        IRPattern::Constructor(
                            "Some".to_string(),
                            vec![IRPattern::Variable("ch".to_string())],
                        ),
                        call("is_whitespace", vec![v("ch")]),
                    ),
                ],
            }),
            body: Box::new(IRNode::Block(vec![IRNode::Assign {
                target: Box::new(v("cur")),
                value: Box::new(call("next_char", vec![v("cur")])),
            }])),
            // Phase 96 invariant: the frame (source
            // and token count carried unchanged), the
            // progress floor (pos never retreats), and
            // the ∀-between fact in row (c)'s own shape
            // — everything between the entry position
            // and the current one is whitespace. The
            // verifier's statement judge discharges
            // initiation / consecution / exit over this
            // text; the interpreter and codegen ignore
            // it, as they do every loop invariant.
            invariant: Some(format!(
                "cur.source == {p}.source; cur.pos >= {p}.pos; \
                 len(cur.tokens) == len({p}.tokens); \
                 forall i: Int in range(len(cur.source)): \
                 i < {p}.pos || i >= cur.pos || is_whitespace(cur.source[i])",
                p = param
            )),
        },
        IRNode::Return(Box::new(v("cur"))),
    ]))
}

/// The keyed tokenize-step denotation (Phase 92, the
/// design doc's §2.6 item 3): the meaning of the
/// tokenizer module's step slot — a single parameter
/// of the state shape, the step return
/// Result<R, String> — is the template's STEP body
/// (`tokenize_step_body`, the same construction the
/// Phase 85 seed family builds).
///
/// The keying difference from the seed is deliberate
/// and recorded: the seed ADDITIONALLY gates on the
/// rows' shape (tokenize_rows_shape), because a seed
/// claims to satisfy its spec and the rows are its
/// admission evidence. A denotation claims something
/// else — the meaning of a structural slot, against
/// which candidates and rows are then judged — so it
/// keys on the shape and the callees' signatures
/// alone, and the rows are never consulted. (The
/// practical consequence is also the honest one: a
/// future row amendment — R2, Phase 96 — cannot make
/// the step's meaning vanish from under the judging
/// of tokenize_all's loop, whose body calls the step
/// by name.) Like the skip denotation, the body is
/// statement-mode: it judges Inconclusive until
/// Phase 93, and denotes from today.
fn tokenize_step_denotation_body(
    spec: &FunctionSpec,
    index: &SpecIndex,
) -> Option<IRNode> {
    let shape = tokenize_shape(spec, index)?;
    tokenize_step_body(spec, index, &shape)
}

/// The Char-classifier tree validator: the number of
/// comparison leaves in `node` when the tree is a pure
/// classifier over the parameter `param` — `||` / `&&` /
/// `!` connectives over leaves that each compare the
/// parameter against a Char literal, in either order, with
/// any of the six comparison operators — and None on any
/// other node (arithmetic, a call the inliner left behind,
/// a match, a literal, a comparison between the parameter
/// and itself, a constant comparison). The leaf count is
/// the caller's evidence the tree mentions the parameter
/// at all: a tree with zero leaves is a constant, not a
/// classifier.
fn char_classifier_tree_leaves(node: &IRNode, param: &str) -> Option<usize> {
    match node {
        IRNode::BinOp(IRBinOp::And | IRBinOp::Or, l, r) => Some(
            char_classifier_tree_leaves(l, param)?
                + char_classifier_tree_leaves(r, param)?,
        ),
        IRNode::BinOp(
            IRBinOp::Eq | IRBinOp::Neq | IRBinOp::Lt | IRBinOp::Le | IRBinOp::Gt | IRBinOp::Ge,
            l,
            r,
        ) => {
            let leaf = matches!(
                (l.as_ref(), r.as_ref()),
                (IRNode::Var(v), IRNode::Literal(IRLiteral::Char(_)))
                | (IRNode::Literal(IRLiteral::Char(_)), IRNode::Var(v))
                    if v == param
            );
            if leaf {
                Some(1)
            } else {
                None
            }
        }
        IRNode::UnaryOp(IRUnaryOp::Not, inner) => char_classifier_tree_leaves(inner, param),
        _ => None,
    }
}

/// The char-equation seed's body: the postcondition's own
/// right-hand side, with spec calls inlined — the equation
/// family's body, admitted for the one signature that
/// family's admission gate cannot judge.
///
/// The measured shape (Phase 48) is the tokenizer's three
/// Char classifiers: a total Bool equation over ONE Char
/// parameter, built from CharLit equalities and Char range
/// comparisons joined by `||`/`&&`. Every earlier family
/// refuses it on a stated predicate (the Phase 49
/// diagnosis, pinned by test): the equation family at its
/// Z3-encodability gate — the shape converts and inlines,
/// but the Char parameter binds `Sym::Opaque`, Char
/// literals encode Opaque, and `sym_binop` has no
/// comparison arm for Opaque operands, so the body is
/// unencodable by construction; guard_table on its
/// Int/Bool param-type filter; guarded_identity on arity;
/// composition_guard and multi_guard on their guard-row
/// shape (and BoolLit leaves); expr_pin_guard on its
/// Int-only key search; pair_guard_table and
/// mixed_pair_guard_table on arity and key types.
///
/// This is a separate family rather than an extension of
/// the equation builder, for two recorded reasons. (1)
/// The admission standard differs categorically: equation
/// seeds are admitted BECAUSE Z3 can judge them (every
/// measured equation winner is Verified), while these
/// seeds are admitted knowing Z3 CANNOT judge them —
/// Char is Opaque, so the seed's verdict is Inconclusive
/// by construction, and its evidence standard is the
/// Phase 48 one: executed agreement over the full
/// 774-char battery. One kind name covering both would
/// falsify the equation family's recorded meaning. (2) A
/// separate builder leaves `equation_seed_body` — and
/// with it all 54 measured equation bodies — literally
/// untouched; the golden fixture and the resolver-visible
/// body diff prove no prior spec's seed moved.
///
/// ── Phase 76 addendum (D5a): the premise above is
/// retired. Char now encodes as its Unicode scalar value,
/// so every classifier tree this builder's validator
/// accepts (||/&&/! over param-vs-CharLit comparisons) is
/// encodable, and by the precedence below the equation
/// family owns every one of them — this builder's
/// territory is empty in practice, and the classifiers'
/// seeds are equation seeds with Z3 verdicts (Verified),
/// their executed agreement re-measured as a cross-check.
/// The builder and its precedence stay in place as the
/// recorded fallback: it would admit exactly a classifier
/// tree the equation family still cannot encode, should
/// the encoder's Char fragment ever narrow again. The
/// family's separate existence is now a historical record
/// of the pre-D5a admission standard, not a live one.
///
/// Territory disjointness is by construction, on two
/// independent discriminators. The signature — exactly
/// one parameter, of type Char, with a Bool return — is
/// a region the seven row/key families refuse on the
/// filters above. Against the equation family the builder
/// carries the Phase 34 precedence (right of first
/// refusal): it admits only when `equation_seed_body`
/// returns None, so a Char spec the equation family can
/// state encodably (a constant equation, an encodable
/// second postcondition) stays the equation family's.
/// Within the shape, the inlined body must validate as a
/// Char-classifier tree (see `char_classifier_tree_leaves`)
/// with at least one comparison leaf — constant
/// comparisons, param-vs-param leaves and arithmetic are
/// refused even where the precedence would let them
/// through, and a call that does not inline to a
/// classifier tree (an unindexed callee, a cycle, a
/// callee denoting a match) leaves a node the validator
/// refuses. The first postcondition (in spec order) whose
/// inlined body validates is the seed — the equation
/// family's iteration discipline.
fn char_equation_seed_body(spec: &FunctionSpec, index: &SpecIndex) -> Option<IRNode> {
    if spec.params.len() != 1 {
        return None;
    }
    let param = &spec.params[0];
    if !matches!(param.ty, TypeRef::Char) {
        return None;
    }
    if !matches!(spec.return_type, TypeRef::Bool) {
        return None;
    }
    // Precedence: the equation family owns anything it
    // can state encodably.
    if equation_seed_body(spec, index).is_some() {
        return None;
    }
    for c in &spec.postconditions {
        let Some(raw) = equation_rhs(c) else { continue };
        let Some(body) = index.inline_calls(&raw) else { continue };
        match char_classifier_tree_leaves(&body, &param.name) {
            Some(leaves) if leaves > 0 => return Some(body),
            _ => continue,
        }
    }
    None
}

/// A constraint read as a single comparison between two
/// expressions: (operator, lhs, rhs). Two parser encodings
/// denote the same shape — `Constraint::Order`, and the
/// `Constraint::Expr(BinOp)` form the parser actually
/// produces for `a > b` in constraint position (its
/// expression parser consumes the comparison before the
/// constraint layer can claim it; measured on
/// count_matching, whose postconditions all arrive in the
/// Expr form) — so readers normalize both. Only the six
/// comparison operators qualify.
fn constraint_comparison(c: &Constraint) -> Option<(IRBinOp, &Expr, &Expr)> {
    match c {
        Constraint::Order(op, l, r) => {
            let ir = match op {
                OrderOp::LessThan => IRBinOp::Lt,
                OrderOp::LessThanOrEqual => IRBinOp::Le,
                OrderOp::GreaterThan => IRBinOp::Gt,
                OrderOp::GreaterThanOrEqual => IRBinOp::Ge,
            };
            Some((ir, l, r))
        }
        Constraint::Expr(Expr::BinOp(op, l, r)) => match op {
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Neq => {
                Some((binop_to_ir(*op), l, r))
            }
            _ => None,
        },
        _ => None,
    }
}

/// Whether a constraint mentions a variable anywhere in
/// its expressions (the constraint-level companion of
/// `expr_mentions_var`).
fn constraint_mentions_var(c: &Constraint, name: &str) -> bool {
    match c {
        Constraint::True => false,
        Constraint::Expr(e) => expr_mentions_var(e, name),
        Constraint::Forall { body, .. } | Constraint::Exists { body, .. } => {
            constraint_mentions_var(body, name)
        }
        Constraint::Implies(a, b) => {
            constraint_mentions_var(a, name) || constraint_mentions_var(b, name)
        }
        Constraint::And(cs) | Constraint::Or(cs) => {
            cs.iter().any(|c| constraint_mentions_var(c, name))
        }
        Constraint::Not(inner) => constraint_mentions_var(inner, name),
        Constraint::Eq(l, r) | Constraint::Order(_, l, r) => {
            expr_mentions_var(l, name) || expr_mentions_var(r, name)
        }
        Constraint::Predicate(_, args) => args.iter().any(|a| expr_mentions_var(a, name)),
    }
}

/// The forall-fold seed's body: a counting scan built from
/// the quantified predicate a `Forall` postcondition
/// states — the tenth constraint-seed family, and the one
/// standalone spec shape with a derivable construction
/// (Phase 57 answer 3).
///
/// The measured shape is count_matching: an Int return;
/// exactly two parameters, one a `List<E>`; a postcondition
/// `forall x: E in <list>: <pred> implies <consequent
/// mentioning output>`, where <pred> is ONE comparison
/// between the quantified element variable and a scalar
/// parameter of the same type (`x > threshold`). The
/// parser discards the Forall's collection expression
/// (`parse_forall` reads and drops it), so the iterated
/// list is inferred: the spec's unique List parameter,
/// whose element type must equal the quantified variable's
/// declared type — two List parameters make the inference
/// ambiguous and are refused, as is any other arity than
/// the measured two-parameter signature.
///
/// The construction (see `forall_fold_scan`) mines the
/// predicate and builds the scan the Phase 59 support
/// matrix proved compilable — Alloc accumulator, For over
/// `0..len(list)`, the element read by Index, a conditional
/// increment — NOT the IR Fold node, which renders E0308
/// (its closure binds `&i64` where the IR body uses the
/// element by value, and the IR has no Deref node).
///
/// Admission, exactly: the return is in the Int family (a
/// count); the element type is a concrete ordered scalar
/// (the templates' discipline — a generic element's
/// comparison cannot render under the bound-free generated
/// signature); the spec's in-spec tests are non-empty and
/// ALL bind in the interpreter (the family's judging
/// bridge re-reads them from the generated test module —
/// see below). The first Forall postcondition in spec
/// order decides: a deviating one is this family's shape
/// refused (the guard families' row discipline), not a
/// skippable non-row. NOT admitted, by design and
/// recorded here: compound predicates (conjunctions,
/// disjunctions, negations of comparisons), literal
/// thresholds, arithmetic on either side of the
/// comparison, predicate calls, Index-form element access
/// (is_sorted's adjacent-index shape — a different
/// construction, owned by the adjacent-scan template),
/// a Forall without the implication, a consequent that
/// does not mention `output`, and an `Exists`.
///
/// The admission standard differs from the equation
/// family's, as char_equation's does: the scan body is a
/// loop, Z3 encodes none of the loop/mutation nodes, so
/// the seed's verdict is Inconclusive BY CONSTRUCTION
/// (measured, Phase 60 probe) and no constraint score can
/// crown it; and the interpreter binds the spec's tests
/// but cannot evaluate For/Alloc/Assign bodies, so its
/// interpreter tpr is 0.0 by coverage, not by behavior
/// (measured: evaluable 3, passed 0, beside constant-0's
/// measured 1/3). The seed's evidence standard is
/// therefore the compiled vehicle, reached through the
/// runner's Phase 60 trigger extension: where a
/// constraint-constructed seed is present but
/// interpreter-unevaluable, the compiled runner judges
/// the finalists. Absent that bridge this family would
/// seed a body selection can never crown.
///
/// Territory: the family claims only specs no earlier
/// family seeded — the precedence is enforced in
/// `seed_from_constraints` (the char_equation right of
/// first refusal, generalized to all nine families), and
/// the Phase 60 inventory over the 142-spec corpus
/// measures the firing set as exactly {count_matching}:
/// every earlier family's signature region (scalar
/// equations and guard rows over Int/Bool/Char keys) is
/// disjoint from a two-parameter List signature carrying
/// a Forall, and the three other standalone specs fail
/// the return/List gates outright.
fn forall_fold_seed_body(spec: &FunctionSpec, _index: &SpecIndex) -> Option<IRNode> {
    if !is_int_family(&spec.return_type) {
        return None;
    }
    if spec.params.len() != 2 {
        return None;
    }
    let list_param = spec.params.iter().find(|p| matches!(p.ty, TypeRef::List(_)))?;
    let TypeRef::List(elem_box) = &list_param.ty else {
        return None;
    };
    let elem: &TypeRef = elem_box.as_ref();
    if !super::templates::is_concrete_ordered_scalar(elem) {
        return None;
    }
    if spec.tests.is_empty() || super::eval::bindable_test_count(spec) != spec.tests.len() {
        return None;
    }
    for c in &spec.postconditions {
        let Constraint::Forall { vars, body } = c else {
            continue;
        };
        return forall_fold_scan(spec, list_param, elem, vars, body);
    }
    None
}

/// The scan for one admitted Forall (see
/// `forall_fold_seed_body` for the admission record): the
/// implication's antecedent is the counting predicate,
/// its consequent must mention `output` (the quantified
/// statement is evidence about the result, not a bare
/// element property), and the constructed body counts the
/// elements satisfying the predicate — the construction
/// the Phase 57 audit derived from the shape, whose
/// stated constraints (a one-way implication plus bounds)
/// constrain but do not by themselves determine a count.
fn forall_fold_scan(
    spec: &FunctionSpec,
    list_param: &Param,
    elem: &TypeRef,
    vars: &[(String, TypeRef)],
    body: &Constraint,
) -> Option<IRNode> {
    if vars.len() != 1 {
        return None;
    }
    let (elem_name, elem_ty) = &vars[0];
    if elem_ty != elem {
        return None;
    }
    let Constraint::Implies(antecedent, consequent) = body else {
        return None;
    };
    if !constraint_mentions_var(consequent, OUTPUT_VAR) {
        return None;
    }
    let (op, lhs, rhs) = constraint_comparison(antecedent)?;
    // One side of the comparison is exactly the quantified
    // element variable; the other is exactly one scalar
    // parameter of the element's type. The written
    // orientation is preserved (no operator normalization:
    // `limit >= x` builds `limit >= list[i]`).
    let (elem_on_left, scalar_expr) = if matches!(lhs, Expr::Var(v) if v == elem_name) {
        (true, rhs)
    } else if matches!(rhs, Expr::Var(v) if v == elem_name) {
        (false, lhs)
    } else {
        return None;
    };
    let Expr::Var(scalar_name) = scalar_expr else {
        return None;
    };
    if scalar_name == elem_name {
        return None;
    }
    let scalar_param = spec.params.iter().find(|p| &p.name == scalar_name)?;
    if scalar_param.name == list_param.name || &scalar_param.ty != elem {
        return None;
    }
    // Construction (Phase 59's proven scan form; shape A
    // of the Phase 60 probe — both shapes compiled and
    // passed 3/3, this one folds the condition into the
    // assigned value exactly as the adjacent scan folds
    // its comparison into the accumulator's update):
    // Alloc the accumulator, walk `0..len(list)`, add
    // `if <pred> { 1 } else { 0 }` per element — the
    // predicate with the element variable replaced by the
    // indexed access — and return the accumulator.
    let taken: Vec<&str> = spec.params.iter().map(|p| p.name.as_str()).collect();
    let acc = super::templates::fresh_name("count", &taken);
    let mut idx_taken: Vec<&str> = taken.clone();
    idx_taken.push(acc.as_str());
    let idx = super::templates::fresh_name("i", &idx_taken);
    let elem_access = IRNode::Index(
        Box::new(IRNode::Var(list_param.name.clone())),
        Box::new(IRNode::Var(idx.clone())),
    );
    let scalar_ref = IRNode::Var(scalar_name.clone());
    let (pred_l, pred_r) = if elem_on_left {
        (elem_access, scalar_ref)
    } else {
        (scalar_ref, elem_access)
    };
    let int = |n: i64| IRNode::Literal(IRLiteral::Int(n));
    Some(IRNode::Block(vec![
        IRNode::Alloc {
            name: acc.clone(),
            ty: spec.return_type.clone(),
            initial: Box::new(int(0)),
        },
        IRNode::For {
            var: idx,
            start: Box::new(int(0)),
            end: Box::new(IRNode::UnaryOp(
                IRUnaryOp::Len,
                Box::new(IRNode::Var(list_param.name.clone())),
            )),
            body: Box::new(IRNode::Assign {
                target: Box::new(IRNode::Var(acc.clone())),
                value: Box::new(IRNode::BinOp(
                    IRBinOp::Add,
                    Box::new(IRNode::Var(acc.clone())),
                    Box::new(IRNode::If {
                        condition: Box::new(IRNode::BinOp(op, Box::new(pred_l), Box::new(pred_r))),
                        then_branch: Box::new(int(1)),
                        else_branch: Box::new(int(0)),
                    }),
                )),
            }),
        },
        IRNode::Return(Box::new(IRNode::Var(acc))),
    ]))
}

/// One composition-guard row: row-shaped (see
/// `guard_row_parts`), its guard leaves all comparing ONE
/// guard expression against BoolLit constants and pinning a
/// single value of the {false, true} domain, and its output
/// pin an admissible expression (see `composition_pin`).
/// Returns the guard expression, the pinned guard value,
/// the pin as IR, and the pin's source expression (the
/// builder's territory rule reads it). Any deviation is
/// None; the builder treats a row-shaped postcondition
/// failing here as a shape deviation, not a skippable
/// non-row.
fn composition_guard_row(
    c: &Constraint,
    index: &SpecIndex,
) -> Option<(Expr, bool, IRNode, Expr)> {
    let (pin_side, guard_leaves) = guard_row_parts(c)?;
    let pin = composition_pin(pin_side, index)?;
    let mut target: Option<Expr> = None;
    let mut guards: Vec<(BinOp, bool, bool)> = Vec::new();
    for leaf in guard_leaves {
        let (leaf_target, normalized) = guard_leaf_on_expr(leaf)?;
        match &target {
            None => target = Some(leaf_target),
            Some(t) if expr_same(t, &leaf_target) => {}
            Some(_) => return None, // one row, two guard expressions
        }
        guards.push(normalized);
    }
    Some((target?, pin_bool_value(&guards)?, pin, pin_side.clone()))
}

/// An admissible composition pin: the pin expression
/// converted whole via `expr_to_ir` (a dropped subexpression
/// would surface as a Hole), never mentioning `output`
/// itself, and calling only specs the module index holds —
/// an unindexed call in a pin could neither be resolved for
/// execution nor judged, so it is a deviation, as an
/// unindexed guard call is.
fn composition_pin(pin_side: &Expr, index: &SpecIndex) -> Option<IRNode> {
    if expr_mentions_var(pin_side, OUTPUT_VAR) {
        return None;
    }
    if !expr_calls_indexed(pin_side, index) {
        return None;
    }
    let pin = expr_to_ir(pin_side);
    if count_holes(&pin) > 0 {
        return None;
    }
    Some(pin)
}

/// The composition guard's scrutinee: the guard expression
/// as IR, when the expression is one whose value ranges
/// over the Bool domain — a Bool parameter of the spec, or
/// a call to an indexed spec returning Bool with matching
/// arity and only indexed calls inside its arguments.
/// Anything else (an Int or String variable, a compound
/// expression, an unknown or non-Bool callee) is not a
/// composition guard: the Int domain stays with the
/// guard-table family's interval machinery, and a String
/// guard's equality leaves are a separate, unadmitted
/// family (rust_ident_empty's shape).
fn composition_scrutinee(
    spec: &FunctionSpec,
    guard_expr: &Expr,
    index: &SpecIndex,
) -> Option<IRNode> {
    if expr_mentions_var(guard_expr, OUTPUT_VAR) {
        return None;
    }
    match guard_expr {
        Expr::Var(v) => {
            let param = spec.params.iter().find(|p| &p.name == v)?;
            if !matches!(param.ty, TypeRef::Bool) {
                return None;
            }
            Some(IRNode::Var(v.clone()))
        }
        Expr::Call(name, args) => {
            let callee = index.get(name)?;
            if callee.return_type != TypeRef::Bool || callee.params.len() != args.len() {
                return None;
            }
            if !expr_calls_indexed(guard_expr, index) {
                return None;
            }
            let scrutinee = expr_to_ir(guard_expr);
            if count_holes(&scrutinee) > 0 {
                return None;
            }
            Some(scrutinee)
        }
        _ => None,
    }
}

/// A composition guard leaf normalized to the guard
/// expression it constrains and (operator, constant_on_left,
/// constant): an Eq/Neq between some expression and a
/// BoolLit constant, in either operand order — the
/// `guard_leaf_bool` normalization with the parameter
/// variable replaced by an arbitrary expression. A leaf
/// between two BoolLits constrains no expression and is not
/// a guard leaf; ordering comparisons and anything else are
/// not Bool guard leaves either.
fn guard_leaf_on_expr(leaf: &Expr) -> Option<(Expr, (BinOp, bool, bool))> {
    let Expr::BinOp(op, l, r) = leaf else {
        return None;
    };
    if !matches!(op, BinOp::Eq | BinOp::Neq) {
        return None;
    }
    match (l.as_ref(), r.as_ref()) {
        (Expr::BoolLit(b), target) if !matches!(target, Expr::BoolLit(_)) => {
            Some((target.clone(), (*op, true, *b)))
        }
        (target, Expr::BoolLit(b)) if !matches!(target, Expr::BoolLit(_)) => {
            Some((target.clone(), (*op, false, *b)))
        }
        _ => None,
    }
}

/// Does every call in the expression target a spec the
/// module index holds? The walk `expr_mentions_var` makes,
/// judging call targets instead of variable mentions.
fn expr_calls_indexed(expr: &Expr, index: &SpecIndex) -> bool {
    match expr {
        Expr::Call(name, args) => {
            index.get(name).is_some() && args.iter().all(|a| expr_calls_indexed(a, index))
        }
        Expr::Var(_)
        | Expr::IntLit(_)
        | Expr::FloatLit(_)
        | Expr::BoolLit(_)
        | Expr::StringLit(_)
        | Expr::CharLit(_) => true,
        Expr::Field(inner, _) => expr_calls_indexed(inner, index),
        Expr::Index(coll, idx) => {
            expr_calls_indexed(coll, index) && expr_calls_indexed(idx, index)
        }
        Expr::BinOp(_, l, r) => {
            expr_calls_indexed(l, index) && expr_calls_indexed(r, index)
        }
        Expr::UnaryOp(_, inner) => expr_calls_indexed(inner, index),
        Expr::Lambda(_, body) => expr_calls_indexed(body, index),
        Expr::Length(inner) => expr_calls_indexed(inner, index),
        Expr::Comprehension {
            collection, body, ..
        } => expr_calls_indexed(collection, index) && expr_calls_indexed(body, index),
        // M1 features (Phase 73): a method call is not a spec
        // call — its name resolves against the D1a builtin
        // table (Phase 74), never the module index — so only
        // the receiver and arguments are walked. A record
        // literal walks its field values.
        Expr::MethodCall { receiver, args, .. } => {
            expr_calls_indexed(receiver, index)
                && args.iter().all(|a| expr_calls_indexed(a, index))
        }
        Expr::RecordLit { fields, .. } => {
            fields.iter().all(|(_, v)| expr_calls_indexed(v, index))
        }
    }
}

/// Structural equality of two spec expressions — the
/// composition builder's test that every row constrains the
/// SAME guard expression. (The spec AST derives no PartialEq;
/// binders — lambdas, comprehensions — never compare equal:
/// they are not guard expressions, and refusing them is the
/// conservative direction.)
fn expr_same(a: &Expr, b: &Expr) -> bool {
    match (a, b) {
        (Expr::Var(x), Expr::Var(y)) => x == y,
        (Expr::IntLit(x), Expr::IntLit(y)) => x == y,
        (Expr::FloatLit(x), Expr::FloatLit(y)) => x.to_bits() == y.to_bits(),
        (Expr::BoolLit(x), Expr::BoolLit(y)) => x == y,
        (Expr::StringLit(x), Expr::StringLit(y)) => x == y,
        (Expr::CharLit(x), Expr::CharLit(y)) => x == y,
        (Expr::Field(x, f), Expr::Field(y, g)) => f == g && expr_same(x, y),
        (Expr::Index(c1, i1), Expr::Index(c2, i2)) => {
            expr_same(c1, c2) && expr_same(i1, i2)
        }
        (Expr::Call(n1, a1), Expr::Call(n2, a2)) => {
            n1 == n2
                && a1.len() == a2.len()
                && a1.iter().zip(a2.iter()).all(|(x, y)| expr_same(x, y))
        }
        (Expr::BinOp(o1, l1, r1), Expr::BinOp(o2, l2, r2)) => {
            o1 == o2 && expr_same(l1, l2) && expr_same(r1, r2)
        }
        (Expr::UnaryOp(o1, x), Expr::UnaryOp(o2, y)) => o1 == o2 && expr_same(x, y),
        (Expr::Length(x), Expr::Length(y)) => expr_same(x, y),
        // M1 features (Phase 73): structural equality is
        // well-defined for both new forms (names, then
        // subexpressions in order).
        (
            Expr::MethodCall { receiver: r1, name: n1, args: a1 },
            Expr::MethodCall { receiver: r2, name: n2, args: a2 },
        ) => {
            n1 == n2
                && expr_same(r1, r2)
                && a1.len() == a2.len()
                && a1.iter().zip(a2.iter()).all(|(x, y)| expr_same(x, y))
        }
        (
            Expr::RecordLit { type_name: t1, fields: f1 },
            Expr::RecordLit { type_name: t2, fields: f2 },
        ) => {
            t1 == t2
                && f1.len() == f2.len()
                && f1.iter().zip(f2.iter()).all(|((n1, v1), (n2, v2))| {
                    n1 == n2 && expr_same(v1, v2)
                })
        }
        _ => false,
    }
}

// ── Spec index: resolving spec-to-spec calls ──────────────

/// How deep spec-call inlining may nest, on both resolution
/// paths (IR substitution here, symbolic evaluation in the
/// verifier). Composition in the M2 modules is one level deep;
/// the cap exists so a pathological call graph — diamonds that
/// duplicate a callee body at every level — stays bounded
/// instead of expanding without limit. A call that would
/// exceed the cap, or that closes a cycle, is unresolvable:
/// callers treat the body containing it as unencodable
/// (Inconclusive), never as a guessed value.
pub(crate) const MAX_INLINE_DEPTH: usize = 8;

/// The specs of one module, by name: the shared resolver for
/// spec-to-spec calls. A spec's body, for resolution purposes,
/// is exactly the body its constraint seed would be built
/// from — the first convertible equation's right-hand side,
/// the guard-table match for a lookup spec with no equation,
/// the guarded-identity match for a two-parameter spec
/// whose rows pin the output to a parameter, the
/// composition-guard match for a spec whose rows are
/// guarded by a shared Bool expression (a call, or a Bool
/// parameter with expression pins — or, by the Phase 34
/// precedence, with literal/bare-variable pins no earlier
/// family admits), the multi-guard truth-table match for a
/// spec whose rows range over several shared Bool
/// expressions, or the expression-pin
/// guard-table match for a spec whose rows pin expressions
/// under one Int key — produced by
/// the same builders the seed family uses, minus the
/// encodability admission (admission is a property of a
/// body in a caller's context, judged by the verifier; the
/// resolver only answers "what does this name denote"). A
/// spec with none of these shapes denotes nothing here and
/// calls to it stay unresolvable.
#[derive(Debug, Clone, Default)]
pub struct SpecIndex {
    by_name: HashMap<String, FunctionSpec>,
    /// The module's type declarations, by name — carried
    /// only when the index is built from a whole module
    /// (`from_module`); `from_specs` leaves it empty, and
    /// consumers that need declarations (the verifier's
    /// record binding, Phase 76) degrade honestly to their
    /// pre-declaration behavior when it is.
    types: HashMap<String, TypeDef>,
}

impl SpecIndex {
    pub fn from_specs(specs: &[FunctionSpec]) -> Self {
        Self {
            by_name: specs
                .iter()
                .map(|s| (s.name.clone(), s.clone()))
                .collect(),
            types: HashMap::new(),
        }
    }

    /// An index over a whole parsed module: its specs AND
    /// its type declarations.
    pub fn from_module(module: &Spec) -> Self {
        Self {
            by_name: module
                .functions
                .iter()
                .map(|s| (s.name.clone(), s.clone()))
                .collect(),
            types: module
                .types
                .iter()
                .map(|t| (t.name.clone(), t.clone()))
                .collect(),
        }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn get(&self, name: &str) -> Option<&FunctionSpec> {
        self.by_name.get(name)
    }

    /// The declared fields of the record a name denotes,
    /// following alias/newtype names to the declaration —
    /// the typechecker's `declared_record_fields` reading,
    /// as an Option: the resolved declaration's name and
    /// its fields, or None when the name is undeclared,
    /// cyclic, or not a record. Needs the declarations
    /// (`from_module`); a `from_specs` index answers None.
    pub fn record_fields(
        &self,
        name: &str,
    ) -> Option<(String, Vec<(String, TypeRef)>)> {
        let mut current = name;
        let mut seen = std::collections::HashSet::new();
        loop {
            let td = self.types.get(current)?;
            match &td.kind {
                TypeKind::Record { fields } => {
                    return Some((td.name.clone(), fields.clone()));
                }
                TypeKind::Alias(TypeRef::Named(next))
                | TypeKind::Newtype(TypeRef::Named(next)) => {
                    if !seen.insert(current.to_string()) {
                        return None;
                    }
                    current = next;
                }
                _ => return None,
            }
        }
    }

    /// The body a call to `name` denotes: the callee's first
    /// equation body, its guard-table body when it has no
    /// equation, its guarded-identity body when it has
    /// neither, its composition-guard body when it has none
    /// of those, its multi-guard truth-table body when it
    /// has none of those either, its expression-pin
    /// guard-table body when it has none of those, its pair-keyed
    /// guard-table body when it has none of those, or its mixed
    /// pair-keyed guard-table body when it has none of those. Phase 92
    /// appends three more, in order: its record-completion body when
    /// it has none of those (the Phase 81 seed builder, dual-registered
    /// as a denotation — next_char denotes through it), its keyed skip
    /// denotation when it has none of those, or its keyed tokenize-step
    /// denotation when it has none of those either (the two keyed
    /// builders are shape-keyed, never row-derived — see their docs).
    /// The body
    /// is NOT call-resolved — it may itself contain calls;
    /// resolving those is the consumer's step, under the
    /// consumer's cycle/depth guard.
    pub fn callee_body(&self, name: &str) -> Option<IRNode> {
        let spec = self.by_name.get(name)?;
        first_equation_body(spec)
            .or_else(|| guard_table_body(spec))
            .or_else(|| guarded_identity_body(spec))
            .or_else(|| composition_guard_body(spec, self))
            .or_else(|| multi_guard_body(spec, self))
            .or_else(|| expr_pin_guard_body(spec, self))
            .or_else(|| pair_guard_table_body(spec))
            .or_else(|| mixed_pair_guard_table_body(spec))
            .or_else(|| complementary_guard_body(spec))
            .or_else(|| record_completion_body(spec, self))
            .or_else(|| skip_denotation_body(spec, self))
            .or_else(|| tokenize_step_denotation_body(spec, self))
    }

    /// IR-level inlining: replace every call to an indexed
    /// spec in `body` by the callee's body with the (already
    /// inlined) arguments substituted for the callee's
    /// parameters, transitively. Calls to names the index does
    /// not hold — builtins like `len` and the `List` literal,
    /// template pseudo-calls — pass through untouched; judging
    /// them is the verifier's business, not the resolver's.
    ///
    /// Returns None when a call to an indexed spec cannot be
    /// resolved: unknown to the index's body builders, arity
    /// mismatch, a cycle or the depth cap, a capture risk in
    /// substitution, or a tree shape outside the seed builders'
    /// output set (the only inputs this is applied to are
    /// equation bodies and resolved callees, so any other shape
    /// means the body was never the resolver's to expand).
    pub fn inline_calls(&self, body: &IRNode) -> Option<IRNode> {
        self.inline_rec(body, &[], 0)
    }

    fn inline_rec(
        &self,
        node: &IRNode,
        visiting: &[String],
        depth: usize,
    ) -> Option<IRNode> {
        match node {
            IRNode::Literal(_) | IRNode::Var(_) => Some(node.clone()),
            IRNode::BinOp(op, l, r) => Some(IRNode::BinOp(
                *op,
                Box::new(self.inline_rec(l, visiting, depth)?),
                Box::new(self.inline_rec(r, visiting, depth)?),
            )),
            IRNode::UnaryOp(op, inner) => Some(IRNode::UnaryOp(
                *op,
                Box::new(self.inline_rec(inner, visiting, depth)?),
            )),
            IRNode::Match { scrutinee, cases } => {
                let scrutinee = Box::new(self.inline_rec(scrutinee, visiting, depth)?);
                let mut out = Vec::with_capacity(cases.len());
                for (pat, body) in cases {
                    out.push((pat.clone(), self.inline_rec(body, visiting, depth)?));
                }
                Some(IRNode::Match {
                    scrutinee,
                    cases: out,
                })
            }
            IRNode::Call { function, args } => {
                // Arguments are caller-side expressions: any
                // calls inside them resolve in the caller's own
                // resolution context, before the callee is
                // entered — so `f(f(x))` is not a cycle.
                let mut inlined_args = Vec::with_capacity(args.len());
                for a in args {
                    inlined_args.push(self.inline_rec(a, visiting, depth)?);
                }
                if !self.by_name.contains_key(function) {
                    return Some(IRNode::Call {
                        function: function.clone(),
                        args: inlined_args,
                    });
                }
                if depth >= MAX_INLINE_DEPTH
                    || visiting.iter().any(|v| v == function)
                {
                    return None;
                }
                let callee = &self.by_name[function];
                if callee.params.len() != inlined_args.len() {
                    return None;
                }
                let callee_body = self.callee_body(function)?;
                let substituted =
                    substitute_params(&callee_body, &callee.params, &inlined_args)?;
                let mut visiting = visiting.to_vec();
                visiting.push(function.clone());
                self.inline_rec(&substituted, &visiting, depth + 1)
            }
            // Phase 81: the resolver's shape set is widened
            // to the Phase 73–77 nodes (If was always an IR
            // node but outside this set; FieldAccess, Index
            // and RecordLit are the record-era shapes). The
            // widening only turns shape-refusals into
            // resolutions — a body whose calls could not
            // resolve for any other reason (cycle, depth,
            // arity, capture) still returns None — and the
            // record-completion seed (remainder (i-b)) is
            // its first consumer: its guard calls a callee
            // whose denotation is an If over an Index.
            IRNode::If {
                condition,
                then_branch,
                else_branch,
            } => Some(IRNode::If {
                condition: Box::new(self.inline_rec(condition, visiting, depth)?),
                then_branch: Box::new(self.inline_rec(then_branch, visiting, depth)?),
                else_branch: Box::new(self.inline_rec(else_branch, visiting, depth)?),
            }),
            IRNode::FieldAccess { base, field } => Some(IRNode::FieldAccess {
                base: Box::new(self.inline_rec(base, visiting, depth)?),
                field: field.clone(),
            }),
            IRNode::Index(coll, idx) => Some(IRNode::Index(
                Box::new(self.inline_rec(coll, visiting, depth)?),
                Box::new(self.inline_rec(idx, visiting, depth)?),
            )),
            IRNode::RecordLit { type_name, fields } => {
                let mut out = Vec::with_capacity(fields.len());
                for (name, value) in fields {
                    out.push((name.clone(), self.inline_rec(value, visiting, depth)?));
                }
                Some(IRNode::RecordLit {
                    type_name: type_name.clone(),
                    fields: out,
                })
            }
            // Phase 87 shape widening (see inline_rec's
            // Phase 81 note): Let joins the resolver's
            // set so a denotation may bind a shared
            // subterm — the record-completion seed's
            // shared guard (Phase 87) is its first
            // consumer. The bound name is a binder:
            // calls in the value resolve in the caller's
            // context (the value sits outside the
            // binder's scope), calls in the body under
            // it; substitution below treats the name
            // exactly like a match binder.
            IRNode::Let { name, value, body } => Some(IRNode::Let {
                name: name.clone(),
                value: Box::new(self.inline_rec(value, visiting, depth)?),
                body: Box::new(self.inline_rec(body, visiting, depth)?),
            }),
            _ => None,
        }
    }
}

/// Substitute argument trees for a callee's parameters in its
/// body. The bodies the resolver produces contain no binders
/// except match variable patterns, and substitution respects
/// them: a pattern-bound name shadows the parameter inside its
/// arm, and if any argument mentions a pattern-bound name
/// freely the substitution would capture it — refused (None)
/// rather than renamed, as no M2 callee binds anything. Any
/// shape outside the builders' output set is likewise refused.
fn substitute_params(
    node: &IRNode,
    params: &[Param],
    args: &[IRNode],
) -> Option<IRNode> {
    // Capture screen: names bound by patterns anywhere in the
    // callee body vs names occurring free in the arguments.
    let mut bound: Vec<&str> = Vec::new();
    collect_pattern_binders(node, &mut bound);
    if !bound.is_empty() {
        let mut free: Vec<String> = Vec::new();
        for a in args {
            collect_free_vars(a, &mut free)?;
        }
        if bound.iter().any(|b| free.iter().any(|f| f == b)) {
            return None;
        }
    }
    substitute_rec(node, params, args, &[])
}

fn collect_pattern_binders<'a>(node: &'a IRNode, out: &mut Vec<&'a str>) {
    match node {
        IRNode::Match { scrutinee, cases } => {
            collect_pattern_binders(scrutinee, out);
            for (pat, body) in cases {
                if let IRPattern::Variable(name) = pat {
                    out.push(name.as_str());
                }
                collect_pattern_binders(body, out);
            }
        }
        IRNode::BinOp(_, l, r) => {
            collect_pattern_binders(l, out);
            collect_pattern_binders(r, out);
        }
        IRNode::UnaryOp(_, inner) => collect_pattern_binders(inner, out),
        IRNode::Call { args, .. } => {
            for a in args {
                collect_pattern_binders(a, out);
            }
        }
        // Phase 81 shape widening (see inline_rec): these
        // shapes bind nothing, but the walk must reach the
        // whole tree.
        IRNode::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_pattern_binders(condition, out);
            collect_pattern_binders(then_branch, out);
            collect_pattern_binders(else_branch, out);
        }
        IRNode::FieldAccess { base, .. } => collect_pattern_binders(base, out),
        IRNode::Index(c, i) => {
            collect_pattern_binders(c, out);
            collect_pattern_binders(i, out);
        }
        IRNode::RecordLit { fields, .. } => {
            for (_, v) in fields {
                collect_pattern_binders(v, out);
            }
        }
        // Phase 87: a Let's name is a binder for the
        // capture screen's purposes, exactly like a
        // match variable — an argument mentioning it
        // freely must not be substituted under it.
        IRNode::Let { name, value, body } => {
            out.push(name.as_str());
            collect_pattern_binders(value, out);
            collect_pattern_binders(body, out);
        }
        _ => {}
    }
}

fn collect_free_vars(node: &IRNode, out: &mut Vec<String>) -> Option<()> {
    match node {
        IRNode::Var(name) => {
            out.push(name.clone());
            Some(())
        }
        IRNode::Literal(_) => Some(()),
        IRNode::BinOp(_, l, r) => {
            collect_free_vars(l, out)?;
            collect_free_vars(r, out)
        }
        IRNode::UnaryOp(_, inner) => collect_free_vars(inner, out),
        IRNode::Call { args, .. } => {
            for a in args {
                collect_free_vars(a, out)?;
            }
            Some(())
        }
        IRNode::Match { scrutinee, cases } => {
            collect_free_vars(scrutinee, out)?;
            for (_, body) in cases {
                collect_free_vars(body, out)?;
            }
            Some(())
        }
        // Phase 81 shape widening (see inline_rec).
        IRNode::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_free_vars(condition, out)?;
            collect_free_vars(then_branch, out)?;
            collect_free_vars(else_branch, out)
        }
        IRNode::FieldAccess { base, .. } => collect_free_vars(base, out),
        IRNode::Index(c, i) => {
            collect_free_vars(c, out)?;
            collect_free_vars(i, out)
        }
        IRNode::RecordLit { fields, .. } => {
            for (_, v) in fields {
                collect_free_vars(v, out)?;
            }
            Some(())
        }
        // Phase 87 shape widening (see inline_rec).
        // The walk is the same over-approximation the
        // Match arm makes: the bound name's uses in
        // the body are collected as if free, which can
        // only make the capture screen refuse more.
        IRNode::Let { value, body, .. } => {
            collect_free_vars(value, out)?;
            collect_free_vars(body, out)
        }
        _ => None,
    }
}

fn substitute_rec(
    node: &IRNode,
    params: &[Param],
    args: &[IRNode],
    shadowed: &[&str],
) -> Option<IRNode> {
    match node {
        IRNode::Var(name) => {
            if shadowed.contains(&name.as_str()) {
                return Some(node.clone());
            }
            match params.iter().position(|p| &p.name == name) {
                Some(i) => Some(args[i].clone()),
                None => Some(node.clone()),
            }
        }
        IRNode::Literal(_) => Some(node.clone()),
        IRNode::BinOp(op, l, r) => Some(IRNode::BinOp(
            *op,
            Box::new(substitute_rec(l, params, args, shadowed)?),
            Box::new(substitute_rec(r, params, args, shadowed)?),
        )),
        IRNode::UnaryOp(op, inner) => Some(IRNode::UnaryOp(
            *op,
            Box::new(substitute_rec(inner, params, args, shadowed)?),
        )),
        IRNode::Call { function, args: call_args } => {
            let mut out = Vec::with_capacity(call_args.len());
            for a in call_args {
                out.push(substitute_rec(a, params, args, shadowed)?);
            }
            Some(IRNode::Call {
                function: function.clone(),
                args: out,
            })
        }
        IRNode::Match { scrutinee, cases } => {
            let scrutinee = Box::new(substitute_rec(scrutinee, params, args, shadowed)?);
            let mut out = Vec::with_capacity(cases.len());
            for (pat, body) in cases {
                let body = match pat {
                    IRPattern::Variable(name) => {
                        let mut shadowed = shadowed.to_vec();
                        shadowed.push(name.as_str());
                        substitute_rec(body, params, args, &shadowed)?
                    }
                    _ => substitute_rec(body, params, args, shadowed)?,
                };
                out.push((pat.clone(), body));
            }
            Some(IRNode::Match {
                scrutinee,
                cases: out,
            })
        }
        // Phase 81 shape widening (see inline_rec).
        IRNode::If {
            condition,
            then_branch,
            else_branch,
        } => Some(IRNode::If {
            condition: Box::new(substitute_rec(condition, params, args, shadowed)?),
            then_branch: Box::new(substitute_rec(then_branch, params, args, shadowed)?),
            else_branch: Box::new(substitute_rec(else_branch, params, args, shadowed)?),
        }),
        IRNode::FieldAccess { base, field } => Some(IRNode::FieldAccess {
            base: Box::new(substitute_rec(base, params, args, shadowed)?),
            field: field.clone(),
        }),
        IRNode::Index(coll, idx) => Some(IRNode::Index(
            Box::new(substitute_rec(coll, params, args, shadowed)?),
            Box::new(substitute_rec(idx, params, args, shadowed)?),
        )),
        IRNode::RecordLit { type_name, fields } => {
            let mut out = Vec::with_capacity(fields.len());
            for (name, value) in fields {
                out.push((
                    name.clone(),
                    substitute_rec(value, params, args, shadowed)?,
                ));
            }
            Some(IRNode::RecordLit {
                type_name: type_name.clone(),
                fields: out,
            })
        }
        // Phase 87 shape widening (see inline_rec):
        // the Let's value substitutes under the
        // caller's shadowing; its body substitutes
        // with the bound name shadowing, exactly like
        // a match variable's arm above.
        IRNode::Let { name, value, body } => {
            let value = Box::new(substitute_rec(value, params, args, shadowed)?);
            let mut inner = shadowed.to_vec();
            inner.push(name.as_str());
            let body = Box::new(substitute_rec(body, params, args, &inner)?);
            Some(IRNode::Let {
                name: name.clone(),
                value,
                body,
            })
        }
        _ => None,
    }
}

fn is_int_family(ty: &TypeRef) -> bool {
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
    )
}

/// One guard row split into its two parts: the output pin
/// (the non-`output` side of the row's single `output == …`
/// leaf) and the remaining leaves, each still to be parsed
/// as a guard by the caller's domain. A postcondition is
/// row-shaped when it is an Or of at least two leaves with
/// exactly one output pin among them; anything else — a bare
/// equation, an ordering on `output`, two pins, no guard
/// leaf — is not a row, and every guard-table-family builder
/// skips it alike. An Eq leaf that does not pin the output
/// is not consumed here: it may still be a guard on the
/// parameter (`param == v`), so it stays with the leaves.
fn guard_row_parts<'a>(c: &'a Constraint) -> Option<(&'a Expr, Vec<&'a Expr>)> {
    let Constraint::Expr(expr) = c else {
        return None;
    };
    let mut leaves = Vec::new();
    flatten_or(expr, &mut leaves);
    if leaves.len() < 2 {
        return None;
    }
    let mut pin: Option<&Expr> = None;
    let mut guards: Vec<&Expr> = Vec::new();
    for leaf in leaves {
        if let Expr::BinOp(BinOp::Eq, l, r) = leaf {
            let side = if matches!(l.as_ref(), Expr::Var(v) if v == OUTPUT_VAR) {
                Some(r.as_ref())
            } else if matches!(r.as_ref(), Expr::Var(v) if v == OUTPUT_VAR) {
                Some(l.as_ref())
            } else {
                None
            };
            if let Some(side) = side {
                if pin.is_some() {
                    return None; // two output pins in one row: ambiguous
                }
                pin = Some(side);
                continue;
            }
        }
        guards.push(leaf);
    }
    if guards.is_empty() {
        return None;
    }
    Some((pin?, guards))
}

/// A Bool guard leaf normalized to (operator,
/// constant_on_left, constant): an Eq/Neq between the
/// parameter and a BoolLit constant, in either operand
/// order. Ordering comparisons and anything else — another
/// variable, a compound side — are not Bool guard leaves.
fn guard_leaf_bool(leaf: &Expr, param: &str) -> Option<(BinOp, bool, bool)> {
    let Expr::BinOp(op, l, r) = leaf else {
        return None;
    };
    if !matches!(op, BinOp::Eq | BinOp::Neq) {
        return None;
    }
    match (l.as_ref(), r.as_ref()) {
        (Expr::Var(v), Expr::BoolLit(b)) if v == param => Some((*op, false, *b)),
        (Expr::BoolLit(b), Expr::Var(v)) if v == param => Some((*op, true, *b)),
        _ => None,
    }
}

/// Does the normalized Bool guard leaf hold at `param == x`?
fn guard_leaf_bool_holds(guard: &(BinOp, bool, bool), x: bool) -> bool {
    let (op, constant_on_left, constant) = guard;
    let (a, b) = if *constant_on_left {
        (*constant, x)
    } else {
        (x, *constant)
    };
    match op {
        BinOp::Eq => a == b,
        BinOp::Neq => a != b,
        _ => unreachable!("guard_leaf_bool admits only Eq/Neq"),
    }
}

/// The single Int value falsifying EVERY guard leaf, if
/// exactly one exists — the probe scan: outside the
/// outermost constants each
/// comparison's truth is stable, and the two extra probe
/// points on each side expose a false-region extending to
/// infinity, so exactly one falsifying probe point ⟺ the
/// guards pin exactly that single value.
fn pin_int_value(guards: &[(BinOp, bool, i64)]) -> Option<i64> {
    let lo = guards.iter().map(|g| g.2).min()? - 2;
    let hi = guards.iter().map(|g| g.2).max()? + 2;
    let mut pinned: Option<i64> = None;
    for x in lo..=hi {
        if guards.iter().all(|g| !guard_leaf_holds(g, x)) {
            if pinned.is_some() {
                return None; // pins a range, not a single value
            }
            pinned = Some(x);
        }
    }
    pinned
}

/// The single Bool value falsifying EVERY guard leaf, if
/// exactly one exists — finite-domain enumeration, the
/// Bool counterpart of `pin_int_value`'s probe scan.
fn pin_bool_value(guards: &[(BinOp, bool, bool)]) -> Option<bool> {
    let mut pinned: Option<bool> = None;
    for x in [false, true] {
        if guards.iter().all(|g| !guard_leaf_bool_holds(g, x)) {
            if pinned.is_some() {
                return None; // pins the whole domain, not a single value
            }
            pinned = Some(x);
        }
    }
    pinned
}

/// A guard parameter value pinned by a row, in either
/// guard domain (Int comparisons or Bool equalities).
#[derive(Debug, Clone, Copy, PartialEq)]
enum GuardPin {
    Int(i64),
    Bool(bool),
}

impl GuardPin {
    fn pattern(&self) -> IRPattern {
        match self {
            GuardPin::Int(v) => IRPattern::Literal(IRLiteral::Int(*v)),
            GuardPin::Bool(b) => IRPattern::Literal(IRLiteral::Bool(*b)),
        }
    }
}

/// One guarded-identity row: row-shaped (see
/// `guard_row_parts`), its output pin exactly the identity
/// parameter, and its guard leaves — all on the guard
/// parameter, parsed by that parameter's domain — pinning a
/// single guard value. Returns the pinned guard value. Any
/// deviation (the pin is a literal, a compound expression,
/// or another parameter; a guard leaf does not parse on the
/// guard parameter; the guards pin no single value) is
/// None; the builder treats a row-shaped postcondition
/// failing here as a shape deviation, not a skippable
/// non-row.
fn guard_identity_row(c: &Constraint, guard_param: &Param, identity: &str) -> Option<GuardPin> {
    let (pin_side, guard_leaves) = guard_row_parts(c)?;
    if !matches!(pin_side, Expr::Var(v) if v == identity) {
        return None;
    }
    if is_int_family(&guard_param.ty) {
        let mut guards: Vec<(BinOp, bool, i64)> = Vec::new();
        for leaf in guard_leaves {
            guards.push(guard_leaf(leaf, &guard_param.name)?);
        }
        Some(GuardPin::Int(pin_int_value(&guards)?))
    } else if matches!(guard_param.ty, TypeRef::Bool) {
        let mut guards: Vec<(BinOp, bool, bool)> = Vec::new();
        for leaf in guard_leaves {
            guards.push(guard_leaf_bool(leaf, &guard_param.name)?);
        }
        Some(GuardPin::Bool(pin_bool_value(&guards)?))
    } else {
        None
    }
}

fn flatten_or<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    match expr {
        Expr::BinOp(BinOp::Or, l, r) => {
            flatten_or(l, out);
            flatten_or(r, out);
        }
        _ => out.push(expr),
    }
}

/// A guard leaf normalized to (operator, constant_on_left,
/// constant): a comparison between the parameter and an Int
/// constant, in either operand order. Anything else — another
/// variable, a compound side, a non-comparison operator — is
/// not a guard leaf.
fn guard_leaf(leaf: &Expr, param: &str) -> Option<(BinOp, bool, i64)> {
    let Expr::BinOp(op, l, r) = leaf else {
        return None;
    };
    if !matches!(
        op,
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Neq
    ) {
        return None;
    }
    match (l.as_ref(), r.as_ref()) {
        (Expr::Var(v), Expr::IntLit(n)) if v == param => Some((*op, false, *n)),
        (Expr::IntLit(n), Expr::Var(v)) if v == param => Some((*op, true, *n)),
        _ => None,
    }
}

/// Does the normalized guard leaf hold at `param == x`?
fn guard_leaf_holds(guard: &(BinOp, bool, i64), x: i64) -> bool {
    let (op, constant_on_left, constant) = guard;
    let (a, b) = if *constant_on_left {
        (*constant, x)
    } else {
        (x, *constant)
    };
    match op {
        BinOp::Lt => a < b,
        BinOp::Le => a <= b,
        BinOp::Gt => a > b,
        BinOp::Ge => a >= b,
        BinOp::Eq => a == b,
        BinOp::Neq => a != b,
        _ => unreachable!("guard_leaf admits only comparisons"),
    }
}

pub fn expr_to_pattern(expr: &Expr) -> IRPattern {
    match expr {
        Expr::IntLit(n) => IRPattern::Literal(IRLiteral::Int(*n)),
        Expr::StringLit(s) => IRPattern::Literal(IRLiteral::String(s.clone())),
        Expr::CharLit(c) => IRPattern::Literal(IRLiteral::Char(*c)),
        Expr::BoolLit(b) => IRPattern::Literal(IRLiteral::Bool(*b)),
        Expr::Call(name, args) => {
            let sub_patterns: Vec<IRPattern> = args.iter().map(expr_to_pattern).collect();
            IRPattern::Constructor(name.clone(), sub_patterns)
        }
        Expr::Var(v) => IRPattern::Variable(v.clone()),
        _ => IRPattern::Wildcard,
    }
}

pub(crate) fn expr_to_ir(expr: &Expr) -> IRNode {
    match expr {
        Expr::IntLit(n) => IRNode::Literal(IRLiteral::Int(*n)),
        Expr::FloatLit(f) => IRNode::Literal(IRLiteral::Float(*f)),
        Expr::BoolLit(b) => IRNode::Literal(IRLiteral::Bool(*b)),
        Expr::StringLit(s) => IRNode::Literal(IRLiteral::String(s.clone())),
        Expr::CharLit(c) => IRNode::Literal(IRLiteral::Char(*c)),
        Expr::Var(v) => IRNode::Var(v.clone()),
        Expr::Call(name, args) => IRNode::Call {
            function: name.clone(),
            args: args.iter().map(expr_to_ir).collect(),
        },
        Expr::BinOp(op, l, r) => IRNode::BinOp(
            binop_to_ir(*op),
            Box::new(expr_to_ir(l)),
            Box::new(expr_to_ir(r)),
        ),
        Expr::UnaryOp(op, inner) => IRNode::UnaryOp(
            unaryop_to_ir(*op),
            Box::new(expr_to_ir(inner)),
        ),
        // M1 features (Phase 73): Field and Index lower for
        // real now — before this phase both fell into the Hole
        // arm below, which is the direct cause of Phase 48's
        // 24/24 seedless record-spec runs (audit section 2.1).
        // IRNode::Index existed all along; only the lowering
        // refused it. FieldAccess is the new kind 28 (D2a).
        Expr::Field(base, field) => IRNode::FieldAccess {
            base: Box::new(expr_to_ir(base)),
            field: field.clone(),
        },
        Expr::Index(base, idx) => IRNode::Index(
            Box::new(expr_to_ir(base)),
            Box::new(expr_to_ir(idx)),
        ),
        // Record construction (D3a): the IR half of the spec
        // literal, kind 29. Field values lower recursively.
        Expr::RecordLit { type_name, fields } => IRNode::RecordLit {
            type_name: type_name.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| (name.clone(), expr_to_ir(value)))
                .collect(),
        },
        // MethodCall lowering is the Hole, by design
        // (re-pinned in Phase 76): the IR kind table has no
        // method-call kind (the audit's section 4 appends
        // only FieldAccess/RecordLit), and method semantics
        // land where the node is consumed — the
        // typechecker's D1a builtin table (Phase 74) and
        // the Z3 encoder's Result/Option forms (Phase 76),
        // both of which work on the spec Expr directly.
        // Pinned by
        // expr_to_ir_method_call_lowers_to_hole_no_ir_kind.
        Expr::MethodCall { .. } => IRNode::Hole { id: 0, expected_type: None },
        _ => IRNode::Hole { id: 0, expected_type: None },
    }
}

fn binop_to_ir(op: BinOp) -> IRBinOp {
    match op {
        BinOp::Add => IRBinOp::Add,
        BinOp::Sub => IRBinOp::Sub,
        BinOp::Mul => IRBinOp::Mul,
        BinOp::Div => IRBinOp::Div,
        BinOp::Mod => IRBinOp::Mod,
        BinOp::And => IRBinOp::And,
        BinOp::Or => IRBinOp::Or,
        BinOp::Eq => IRBinOp::Eq,
        BinOp::Neq => IRBinOp::Neq,
        BinOp::Lt => IRBinOp::Lt,
        BinOp::Le => IRBinOp::Le,
        BinOp::Gt => IRBinOp::Gt,
        BinOp::Ge => IRBinOp::Ge,
        BinOp::Concat => IRBinOp::Concat,
    }
}

fn unaryop_to_ir(op: UnaryOp) -> IRUnaryOp {
    match op {
        UnaryOp::Neg => IRUnaryOp::Neg,
        UnaryOp::Not => IRUnaryOp::Not,
    }
}

fn evaluate_candidate(candidate: &CandidateImplementation, spec: &FunctionSpec) -> ScoreBreakdown {
    // Test pass ratio — REAL execution via the IR interpreter
    // (synthesis::eval): the candidate body is run against every
    // spec test whose input/expected values can be mapped to
    // evaluator values, and the ratio is passed / evaluable.
    // A candidate that cannot be evaluated on a test (holes,
    // unbound variables, unknown calls, …) FAILS that test —
    // it does not get the benefit of the doubt.
    //
    // FALLBACK: if a spec has tests but NONE of them can be
    // mapped (e.g. tuple test inputs carrying a lambda for a
    // fn-typed parameter — the evaluator has no function
    // values, so sort's and binary_search's tests still
    // cannot bind; tuple inputs of first-order values DO bind
    // elementwise since Phase 53), there is no truth signal
    // to measure. Keep the previous structural behavior
    // (tests assumed passed) rather than punishing every
    // candidate for an encoding limitation. Specs with no tests
    // at all keep the neutral 0.5.
    let test_score = if spec.tests.is_empty() {
        0.5 // No tests: neutral score
    } else {
        let run = super::eval::run_spec_tests(&candidate.body, spec);
        if run.evaluable == 0 {
            1.0 // FALLBACK (see above): no mappable tests — legacy structural score
        } else {
            run.passed as f64 / run.evaluable as f64
        }
    };

    // Constraint score: structural similarity to constraint shape
    let constraint_score = 0.7; // Placeholder — real version checks Z3

    // Hole penalty: a body with unfilled Holes has not actually run
    // any test or satisfied any constraint — those parts of it are
    // literally missing. Correctness credit therefore decays by
    // half per hole (1 hole → ×0.5, 2 holes → ×0.25, …), so a holed
    // candidate always ranks strictly below an otherwise-equal
    // hole-free one. selector::select_best additionally hard-gates
    // holed candidates whenever a hole-free candidate exists.
    let holes = count_holes(&candidate.body);
    let hole_factor = 0.5f64.powi(holes as i32);

    // Complexity: check if body matches Big-O bound
    let complexity_score = estimate_complexity(&candidate.body, spec);

    // Quality: AST size/readability heuristic
    let quality_score = estimate_quality(&candidate.body);

    ScoreBreakdown {
        test_pass_ratio: test_score * hole_factor,
        constraint_score: constraint_score * hole_factor,
        complexity_score,
        quality_score,
        synthesis_time_us: 0,
    }
}

fn estimate_complexity(body: &IRNode, spec: &FunctionSpec) -> f64 {
    let depth = ast_depth(body);
    let has_nested_loops = count_loops(body) > 1;
    let has_recursion = has_recursive_calls(body);

    if let Some(ref bound) = spec.complexity {
        match bound.bound {
            BigO::Constant => if depth < 2 { 1.0 } else { 0.3 },
            BigO::Linear => if depth < 6 && !has_nested_loops { 1.0 } else { 0.4 },
            BigO::Linearithmic => if !has_recursion || depth < 12 { 1.0 } else { 0.5 },
            BigO::Quadratic => if has_nested_loops { 1.0 } else { 0.6 },
            _ => 0.7, // Unknown — neutral score
        }
    } else {
        0.8 // No bound specified
    }
}

fn ast_depth(node: &IRNode) -> usize {
    match node {
        IRNode::Hole { .. } | IRNode::Literal(_) | IRNode::Var(_) => 1,
        IRNode::Block(stmts) => 1 + stmts.iter().map(ast_depth).max().unwrap_or(0),
        IRNode::Let { value, body, .. } => 1 + ast_depth(value).max(ast_depth(body)),
        IRNode::If { condition, then_branch, else_branch } =>
            1 + ast_depth(condition).max(ast_depth(then_branch).max(ast_depth(else_branch))),
        IRNode::While { condition, body, .. } => 1 + ast_depth(condition).max(ast_depth(body)),
        IRNode::For { start, end, body, .. } =>
            1 + ast_depth(start).max(ast_depth(end)).max(ast_depth(body)),
        IRNode::Match { scrutinee, cases } =>
            1 + ast_depth(scrutinee).max(cases.iter().map(|(_, b)| ast_depth(b)).max().unwrap_or(0)),
        IRNode::Call { args, .. } => 1 + args.iter().map(ast_depth).max().unwrap_or(0),
        IRNode::Return(expr) => 1 + ast_depth(expr),
        IRNode::BinOp(_, l, r) => 1 + ast_depth(l).max(ast_depth(r)),
        IRNode::UnaryOp(_, inner) => 1 + ast_depth(inner),
        IRNode::Lambda { body, .. } | IRNode::Closure { body, .. } => 1 + ast_depth(body),
        IRNode::Collection { elements, .. } => 1 + elements.iter().map(ast_depth).max().unwrap_or(0),
        IRNode::Map { function, over } => 1 + ast_depth(function).max(ast_depth(over)),
        IRNode::Filter { predicate, over } => 1 + ast_depth(predicate).max(ast_depth(over)),
        IRNode::Fold { operation, initial, over } =>
            1 + ast_depth(operation).max(ast_depth(initial)).max(ast_depth(over)),
        IRNode::Sort { comparator, collection } => 1 + ast_depth(comparator).max(ast_depth(collection)),
        IRNode::Alloc { initial, .. } => 1 + ast_depth(initial),
        IRNode::Assign { target, value } => 1 + ast_depth(target).max(ast_depth(value)),
        IRNode::Typed { node, .. } => 1 + ast_depth(node),
        IRNode::Index(coll, idx) => 1 + ast_depth(coll).max(ast_depth(idx)),
        IRNode::Loop { body, .. } => 1 + ast_depth(body),
        IRNode::Break { value } => 1 + value.as_ref().map(|v| ast_depth(v)).unwrap_or(0),
        IRNode::StructDef { body, .. } => 1 + ast_depth(body),
        IRNode::TryPropagate(inner) => 1 + ast_depth(inner),
        // M1 features (Phase 73).
        IRNode::FieldAccess { base, .. } => 1 + ast_depth(base),
        IRNode::RecordLit { fields, .. } => {
            1 + fields.iter().map(|(_, v)| ast_depth(v)).max().unwrap_or(0)
        }
    }
}

fn count_loops(body: &IRNode) -> usize {
    match body {
        IRNode::While { body: b, .. } | IRNode::For { body: b, .. } =>
            1 + count_loops(b),
        IRNode::Block(stmts) => stmts.iter().map(count_loops).sum(),
        IRNode::If { then_branch, else_branch, .. } =>
            count_loops(then_branch) + count_loops(else_branch),
        IRNode::Let { value, body, .. } => count_loops(value) + count_loops(body),
        IRNode::Match { cases, .. } => cases.iter().map(|(_, b)| count_loops(b)).sum(),
        _ => 0,
    }
}

fn has_recursive_calls(body: &IRNode) -> bool {
    match body {
        IRNode::Call { function, .. } => function == "self",
        IRNode::Block(stmts) => stmts.iter().any(has_recursive_calls),
        IRNode::If { then_branch, else_branch, .. } =>
            has_recursive_calls(then_branch) || has_recursive_calls(else_branch),
        IRNode::Match { cases, .. } => cases.iter().any(|(_, b)| has_recursive_calls(b)),
        _ => false,
    }
}

fn estimate_quality(body: &IRNode) -> f64 {
    // Heuristic: smaller, well-structured AST = higher quality
    let size = ast_node_count(body);
    let depth = ast_depth(body);

    if size < 3 { return 0.3; } // Too small
    if depth > 20 { return 0.3; } // Too deep
    if size > 500 { return 0.4; } // Too large
    0.7 // Default decent score
}

fn ast_node_count(node: &IRNode) -> usize {
    match node {
        IRNode::Hole { .. } | IRNode::Literal(_) | IRNode::Var(_) => 1,
        IRNode::Block(stmts) => 1 + stmts.iter().map(ast_node_count).sum::<usize>(),
        IRNode::Let { value, body, .. } => 1 + ast_node_count(value) + ast_node_count(body),
        IRNode::If { condition, then_branch, else_branch } =>
            1 + ast_node_count(condition) + ast_node_count(then_branch) + ast_node_count(else_branch),
        IRNode::While { condition, body, .. } =>
            1 + ast_node_count(condition) + ast_node_count(body),
        IRNode::For { start, end, body, .. } =>
            1 + ast_node_count(start) + ast_node_count(end) + ast_node_count(body),
        IRNode::Match { scrutinee, cases } =>
            1 + ast_node_count(scrutinee)
                + cases.iter().map(|(_, b)| ast_node_count(b)).sum::<usize>(),
        IRNode::Call { args, .. } => 1 + args.iter().map(ast_node_count).sum::<usize>(),
        IRNode::Return(expr) => 1 + ast_node_count(expr),
        IRNode::BinOp(_, l, r) => 1 + ast_node_count(l) + ast_node_count(r),
        IRNode::UnaryOp(_, inner) => 1 + ast_node_count(inner),
        IRNode::Lambda { body, .. } | IRNode::Closure { body, .. } => 1 + ast_node_count(body),
        IRNode::Collection { elements, .. } => 1 + elements.iter().map(ast_node_count).sum::<usize>(),
        IRNode::Map { function, over } => 1 + ast_node_count(function) + ast_node_count(over),
        IRNode::Filter { predicate, over } => 1 + ast_node_count(predicate) + ast_node_count(over),
        IRNode::Fold { operation, initial, over } =>
            1 + ast_node_count(operation) + ast_node_count(initial) + ast_node_count(over),
        IRNode::Sort { comparator, collection } =>
            1 + ast_node_count(comparator) + ast_node_count(collection),
        IRNode::Alloc { initial, .. } => 1 + ast_node_count(initial),
        IRNode::Assign { target, value } => 1 + ast_node_count(target) + ast_node_count(value),
        IRNode::Typed { node, .. } => 1 + ast_node_count(node),
        IRNode::Index(coll, idx) => 1 + ast_node_count(coll) + ast_node_count(idx),
        IRNode::Loop { body, .. } => 1 + ast_node_count(body),
        IRNode::Break { value } => 1 + value.as_ref().map(|v| ast_node_count(v)).unwrap_or(0),
        IRNode::StructDef { body, .. } => 1 + ast_node_count(body),
        IRNode::TryPropagate(inner) => 1 + ast_node_count(inner),
        // M1 features (Phase 73).
        IRNode::FieldAccess { base, .. } => 1 + ast_node_count(base),
        IRNode::RecordLit { fields, .. } => {
            1 + fields.iter().map(|(_, v)| ast_node_count(v)).sum::<usize>()
        }
    }
}

fn tournament_select<'a>(
    population: &'a [CandidateImplementation],
    tournament_size: usize,
    rng: &mut impl Rng,
) -> &'a CandidateImplementation {
    let mut best = &population[rng.gen_range(0..population.len())];
    for _ in 1..tournament_size {
        let challenger = &population[rng.gen_range(0..population.len())];
        if challenger.score > best.score {
            best = challenger;
        }
    }
    best
}

// ── Stub helpers ──────────────────────────────────────────

fn inline_random_call(_body: &mut IRNode) {}
fn swap_equivalent(_body: &mut IRNode) {}
fn optimize_loop(_body: &mut IRNode) {}
fn add_parallel_hint(_body: &mut IRNode) {}
fn random_rewrite(_body: &mut IRNode) {}

fn format_params(params: &[Param]) -> String {
    params.iter()
        .map(|p| format!("{}: {}", p.name, p.ty))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_constraints(constraints: &[Constraint]) -> String {
    if constraints.is_empty() {
        return "none".into();
    }
    constraints.iter()
        .map(|c| format!("- {}", format_constraint(c)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_constraint(c: &Constraint) -> String {
    match c {
        Constraint::True => "true".into(),
        Constraint::Expr(e) => format!("{:?}", e),
        Constraint::Eq(a, b) => format!("{:?} == {:?}", a, b),
        Constraint::Order(op, a, b) => format!("{:?} {:?} {:?}", a, op, b),
        Constraint::And(parts) => parts.iter().map(format_constraint).collect::<Vec<_>>().join(" && "),
        Constraint::Implies(a, b) => format!("({} => {})", format_constraint(a), format_constraint(b)),
        Constraint::Not(inner) => format!("!({})", format_constraint(inner)),
        _ => format!("{:?}", c),
    }
}

fn format_tests(tests: &[Test]) -> String {
    tests.iter()
        .map(|t| format!("  {} → {}", format_expr(&t.input), format_expr(&t.expected_output)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_expr(expr: &Expr) -> String {
    match expr {
        Expr::IntLit(n) => n.to_string(),
        Expr::StringLit(s) => format!("\"{}\"", s),
        Expr::CharLit(c) => format!("{:?}", c),
        Expr::Var(v) => v.clone(),
        Expr::Call(name, args) => {
            let args: Vec<String> = args.iter().map(format_expr).collect();
            format!("{}({})", name, args.join(", "))
        }
        _ => format!("{:?}", expr),
    }
}

impl ComplexityBound {
    fn dimension_name(&self) -> &str {
        match self.dimension {
            ComplexityDimension::Time => "time",
            ComplexityDimension::Space => "space",
            ComplexityDimension::AmortizedTime => "amortized time",
            ComplexityDimension::Communication => "communication",
        }
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_spec() -> FunctionSpec {
        FunctionSpec {
            name: "test".into(),
            doc: None,
            generics: vec![],
            params: vec![],
            return_type: TypeRef::Unit,
            preconditions: vec![],
            postconditions: vec![],
            invariants: vec![],
            complexity: None,
            resource: None,
            tests: vec![],
            annotations: HashMap::new(),
        }
    }

    /// True if the tree contains a `Call` to the placeholder
    /// function "_" anywhere (mirrors the `count_holes` traversal).
    fn contains_placeholder_call(node: &IRNode) -> bool {
        match node {
            IRNode::Call { function, args } =>
                function == "_" || args.iter().any(contains_placeholder_call),
            IRNode::Block(stmts) => stmts.iter().any(contains_placeholder_call),
            IRNode::Let { value, body, .. } =>
                contains_placeholder_call(value) || contains_placeholder_call(body),
            IRNode::If { condition, then_branch, else_branch } =>
                contains_placeholder_call(condition)
                    || contains_placeholder_call(then_branch)
                    || contains_placeholder_call(else_branch),
            IRNode::While { condition, body, .. } =>
                contains_placeholder_call(condition) || contains_placeholder_call(body),
            IRNode::For { start, end, body, .. } =>
                contains_placeholder_call(start)
                    || contains_placeholder_call(end)
                    || contains_placeholder_call(body),
            IRNode::Match { scrutinee, cases } =>
                contains_placeholder_call(scrutinee)
                    || cases.iter().any(|(_, b)| contains_placeholder_call(b)),
            IRNode::Return(expr) => contains_placeholder_call(expr),
            IRNode::BinOp(_, l, r) =>
                contains_placeholder_call(l) || contains_placeholder_call(r),
            IRNode::UnaryOp(_, inner) => contains_placeholder_call(inner),
            IRNode::Lambda { body, .. } | IRNode::Closure { body, .. } =>
                contains_placeholder_call(body),
            IRNode::Collection { elements, .. } =>
                elements.iter().any(contains_placeholder_call),
            IRNode::Map { function, over } | IRNode::Filter { predicate: function, over } =>
                contains_placeholder_call(function) || contains_placeholder_call(over),
            IRNode::Fold { operation, initial, over } =>
                contains_placeholder_call(operation)
                    || contains_placeholder_call(initial)
                    || contains_placeholder_call(over),
            IRNode::Sort { comparator, collection } =>
                contains_placeholder_call(comparator) || contains_placeholder_call(collection),
            IRNode::Alloc { initial, .. } => contains_placeholder_call(initial),
            IRNode::Assign { target, value } =>
                contains_placeholder_call(target) || contains_placeholder_call(value),
            IRNode::Typed { node, .. } => contains_placeholder_call(node),
            _ => false, // Holes, Literals, Vars, etc.
        }
    }

    fn make_candidate(body: IRNode, spec: &FunctionSpec) -> CandidateImplementation {
        CandidateImplementation {
            id: 0,
            body,
            spec_name: spec.name.clone(),
            spec: spec.clone(),
            score: 0.0,
            scores: empty_scores(),
            generation: 0,
            provenance: Provenance::Template { template_name: "test".into() },
            verified: false,
            verification_status: None,
        }
    }

    /// Measured truth must dominate the composite score. With
    /// the old weights (0.4 tpr / 0.3 constraint / 0.2 complexity
    /// / 0.1 quality) a candidate passing NO test could outscore
    /// one passing ALL of them on the flat placeholder constraint
    /// score (0.7 for every hole-free body) plus shape heuristics
    /// — measured probe winners were exactly that: tpr 0.000.
    /// Invariant: a test_pass_ratio gap of ≥ 0.25 always decides,
    /// no matter what the other dimensions say.
    #[test]
    fn composite_score_lets_measured_truth_dominate() {
        let breakdown = |tpr: f64, constraint: f64, complexity: f64, quality: f64| {
            ScoreBreakdown {
                test_pass_ratio: tpr,
                constraint_score: constraint,
                complexity_score: complexity,
                quality_score: quality,
                synthesis_time_us: 0,
            }
        };
        // Passes every measured test, gets nothing else — must
        // beat a candidate that passes none and maxes the rest.
        let truthful = breakdown(1.0, 0.0, 0.0, 0.0);
        let fancy = breakdown(0.0, 1.0, 1.0, 1.0);
        assert!(
            composite_score(&truthful) > composite_score(&fancy),
            "truthful {} vs fancy {}",
            composite_score(&truthful),
            composite_score(&fancy)
        );
        // The 0.25-gap invariant against the worst case: the
        // ahead candidate gets zero from every other dimension,
        // the behind candidate gets the maximum.
        let ahead = breakdown(0.25, 0.0, 0.0, 0.0);
        let behind = breakdown(0.0, 1.0, 1.0, 1.0);
        assert!(composite_score(&ahead) > composite_score(&behind));
    }

    fn test_induced_candidate(spec: &FunctionSpec) -> CandidateImplementation {
        let mut c = make_candidate(
            IRNode::Match {
                scrutinee: Box::new(IRNode::Var("kind".into())),
                cases: vec![(
                    IRPattern::Wildcard,
                    IRNode::Hole { id: 42, expected_type: None },
                )],
            },
            spec,
        );
        c.provenance = Provenance::TestInduced { num_tests: 3 };
        c
    }

    /// Seed protection, unit level: a protected seed missing
    /// from the population is re-admitted; one whose body already
    /// survived is not duplicated; no seeds is a no-op.
    #[test]
    fn readmit_protected_seeds_restores_dropped_seed() {
        let spec = is_literal_kind_spec();
        let seed = test_induced_candidate(&spec);
        let other = make_candidate(IRNode::Literal(IRLiteral::Bool(true)), &spec);

        let mut pop = vec![other.clone()];
        readmit_protected_seeds(&mut pop, &[seed.clone()]);
        assert_eq!(pop.len(), 2);
        assert!(matches!(
            pop[1].provenance,
            Provenance::TestInduced { .. }
        ));

        // Already present (same body): no duplicate.
        let mut pop = vec![seed.clone(), other];
        readmit_protected_seeds(&mut pop, &[seed]);
        assert_eq!(pop.len(), 2);

        // Nothing protected: population untouched.
        let mut pop = vec![make_candidate(IRNode::Literal(IRLiteral::Int(1)), &spec)];
        readmit_protected_seeds(&mut pop, &[]);
        assert_eq!(pop.len(), 1);
    }

    /// Seed protection, pipeline level: the final pool returned
    /// by `synthesize` for is_literal_kind contains the
    /// TestInduced seed, carrying its measured score — every test
    /// passed (1.0) times the one-hole decay (×0.5) = 0.5. The
    /// search phases may do what they like; the seed's presence
    /// in the pool selection sees is guaranteed by re-admission
    /// in `run`, not by luck.
    #[test]
    fn synthesized_pool_keeps_test_induced_seed() {
        let spec = is_literal_kind_spec();
        let pool = synthesize(&spec, 1).expect("opt-1 synthesis must succeed");
        let seed = pool
            .iter()
            .find(|c| matches!(c.provenance, Provenance::TestInduced { .. }))
            .expect("final pool must contain the TestInduced seed");
        assert!(
            (seed.scores.test_pass_ratio - 0.5).abs() < 1e-9,
            "seed tpr was {}",
            seed.scores.test_pass_ratio
        );
    }

    /// Load one of the real M2 specs (the same helper shape as
    /// the verifier tests). Constraint seeds are derived from
    /// spec text, so their tests must run against spec text —
    /// hand-built ASTs would test the test, not the derivation.
    fn load_m2_spec(file: &str, name: &str) -> FunctionSpec {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/self_host/{}.morph", root, file);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = crate::spec::parser::parse(&source).unwrap();
        let checked = crate::spec::typeck::check(parsed).unwrap();
        checked
            .spec
            .functions
            .iter()
            .find(|f| f.name == name)
            .unwrap()
            .clone()
    }

    fn constraint_seed(
        engine: &SynthesisEngine,
        spec: &FunctionSpec,
        kind: &str,
    ) -> CandidateImplementation {
        engine
            .seed_from_constraints(spec)
            .into_iter()
            .find(|c| matches!(&c.provenance, Provenance::ConstraintInduced { kind: k } if k == kind))
            .unwrap_or_else(|| panic!("{} must have a {} constraint seed", spec.name, kind))
    }

    fn eval_int_body(body: &IRNode, var: &str, value: i64) -> crate::synthesis::eval::Value {
        let mut env = crate::synthesis::eval::Env::new();
        env.insert(var.into(), crate::synthesis::eval::Value::Int(value));
        crate::synthesis::eval::eval(body, &env).expect("seed body must evaluate")
    }

    fn load_example_spec(rel: &str, name: &str) -> FunctionSpec {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/{}", root, rel);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = crate::spec::parser::parse(&source).unwrap();
        let checked = crate::spec::typeck::check(parsed).unwrap();
        checked
            .spec
            .functions
            .iter()
            .find(|f| f.name == name)
            .unwrap()
            .clone()
    }

    /// Phase 59 (G2): the dedup hazard, documented as a test.
    /// `ir_equivalent` compares only root-variant
    /// discriminants, so two DIFFERENT Block bodies are
    /// "equivalent" to it. The hazard DID bite — not the
    /// template set (at most one template per spec, and the
    /// three target specs have no constraint seeds), but the
    /// CONSTRAINT seeds: the bare-Hole template seeds had
    /// been buffering a Match-rooted guard_table seed from
    /// the Match-rooted test-induced seed, and removing the
    /// holes exposed it (regression test:
    /// guard_table_seed_survives_seed_population_dedup,
    /// written failing-first). The fix moved
    /// seed_population's dedup to ir_identical (Debug
    /// fingerprint); ir_equivalent survives only as
    /// replace_subtree's documented approximation.
    #[test]
    fn dedup_equivalence_is_discriminant_only() {
        let a = IRNode::Block(vec![]);
        let b = IRNode::Block(vec![IRNode::Return(Box::new(IRNode::Literal(
            IRLiteral::Int(1),
        )))]);
        assert!(ir_equivalent(&a, &b), "discriminant-only: distinct Blocks compare equal");
        assert!(!ir_equivalent(
            &a,
            &IRNode::Sort {
                comparator: Box::new(IRNode::Var("cmp".into())),
                collection: Box::new(IRNode::Var("list".into())),
            }
        ));
        // The dedup predicate: distinct bodies are NOT
        // identical; a body is identical to itself.
        assert!(!ir_identical(&a, &b), "dedup must keep distinct same-root bodies");
        assert!(ir_identical(&a, &a.clone()), "dedup removes true duplicates");
    }

    /// Phase 59 (G2): for each structurally-keyed target
    /// spec, seed_population (which ends with the
    /// discriminant dedup) contains EXACTLY ONE template
    /// candidate — the library's real, hole-free body under
    /// its recorded name — alongside the test-induced seed.
    /// Through Phase 58 the same populations contained one
    /// template candidate that was a bare Hole.
    #[test]
    fn template_seeds_are_real_bodies_and_survive_dedup() {
        let cases = [
            ("examples/sort.morph", "sort", "sort_primitive"),
            ("examples/binary_search.morph", "binary_search", "binary_search"),
            ("examples/self_host/is_sorted.morph", "is_sorted", "adjacent_scan"),
        ];
        for (rel, name, template_name) in cases {
            let spec = load_example_spec(rel, name);
            let mut engine = SynthesisEngine::new(SynthesisConfig::from_opt_level(1));
            let population = engine.seed_population(&spec).expect("seed_population");
            let templates: Vec<&CandidateImplementation> = population
                .iter()
                .filter(|c| matches!(c.provenance, Provenance::Template { .. }))
                .collect();
            assert_eq!(
                templates.len(),
                1,
                "{}: exactly one template seed must survive dedup, got {}",
                name,
                templates.len()
            );
            let seed = templates[0];
            match &seed.provenance {
                Provenance::Template { template_name: n } => {
                    assert_eq!(n, template_name, "{}: template name", name)
                }
                _ => unreachable!(),
            }
            assert_eq!(
                count_holes(&seed.body),
                0,
                "{}: the template seed must be a complete body, not a hole",
                name
            );
            assert!(
                population
                    .iter()
                    .any(|c| matches!(c.provenance, Provenance::TestInduced { .. })),
                "{}: the test-induced seed must also survive",
                name
            );
        }
    }

    /// Phase 59 (G2): a spec whose shape no template fits
    /// (count_matching — the forall-fold family's territory,
    /// slice G3) gets NO template seed: the wiring must not
    /// resurrect the old always-a-hole behavior.
    #[test]
    fn template_seeds_do_not_fire_without_a_fitting_shape() {
        let spec = load_example_spec("examples/self_host/count_matching.morph", "count_matching");
        let mut engine = SynthesisEngine::new(SynthesisConfig::from_opt_level(1));
        let population = engine.seed_population(&spec).expect("seed_population");
        assert!(
            !population
                .iter()
                .any(|c| matches!(c.provenance, Provenance::Template { .. })),
            "count_matching must have no template seed in G2"
        );
    }

    /// Phase 59 (G2): REGRESSION test, written failing-first.
    /// seed_population ends with `dedup_by(ir_equivalent)`,
    /// and ir_equivalent compares only root discriminants.
    /// Through Phase 58 the three bare-Hole template seeds
    /// sat between the test-induced seed (a Match) and the
    /// constraint seeds in seed order, acting as a
    /// discriminant buffer: a Match-rooted guard_table seed
    /// was never adjacent to the Match-rooted test-induced
    /// seed. With real templates keyed structurally, specs
    /// no template fits have NO buffer, and the dedup eats
    /// the guard_table seed before the protected-seed
    /// snapshot is taken — readmission cannot restore what
    /// the snapshot never saw. (Measured: the M2/M3
    /// cross-validation winners for guard-table specs —
    /// type_tag_name, rust_type_name — fell to Mutation
    /// bodies with 0 agreement the moment the holes left.)
    #[test]
    fn guard_table_seed_survives_seed_population_dedup() {
        let spec = load_m2_spec("printer", "type_tag_name");
        let mut engine = SynthesisEngine::new(SynthesisConfig::from_opt_level(1));
        let population = engine.seed_population(&spec).expect("seed_population");
        assert!(
            population.iter().any(|c| matches!(
                &c.provenance,
                Provenance::ConstraintInduced { kind } if kind == "guard_table"
            )),
            "the guard_table seed must survive seed_population's dedup"
        );
    }

    /// Constraint seeds (i): a range spec's equation constraint
    /// IS its body — `output == (tag>=0&&tag<=8)` — so the seed
    /// built from it must be Verified by the candidate-encoding
    /// verifier (the pool previously contained NO Verified
    /// candidate for this spec in any measured draw), must
    /// survive to the final pool under seed protection, and
    /// must win selection once the gates can see it.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_equation_seed_is_verified_and_wins() {
        let spec = load_m2_spec("validator", "is_known_type_tag");
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the equation seed");
        let seed = constraint_seed(&engine, &spec, "equation");
        assert_eq!(count_holes(&seed.body), 0);

        let result = crate::verify::verifier::Verifier::new().verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        let pool = synthesize(&spec, 1).expect("opt-1 synthesis must succeed");
        assert!(
            pool.iter()
                .any(|c| matches!(c.provenance, Provenance::ConstraintInduced { .. })),
            "final pool must contain the constraint seed"
        );
        let verified = crate::verify::verifier::verify_all(pool, &spec).expect("verify_all");
        let winner =
            crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
        assert!(winner.verified, "the spec-exact seed must win verified");
        assert!(
            (winner.scores.test_pass_ratio - 1.0).abs() < 1e-9,
            "winner tpr was {}",
            winner.scores.test_pass_ratio
        );
        assert_eq!(
            format!("{:?}", winner.body),
            format!("{:?}", seed.body),
            "the winning body must be the constraint's own expression"
        );
    }

    /// Phase 25 (finding F1): the parser now surfaces `++` as
    /// BinOp::Concat, so a spec can finally STATE a string
    /// assembly. The fixtures below were impossible before this
    /// phase: `parse` rejected the second `+` outright, so the
    /// specs could not even be written. Their equation seeds
    /// must exist (the postcondition IS the body), be
    /// Z3-Verified, win selection, and execute through the IR
    /// interpreter at tpr 1.000 — the full path every layer
    /// below the parser already supported.
    ///
    /// The two specs split the codegen leg between them, for a
    /// reason that predates this phase and is recorded here
    /// plainly: the frozen emitter renders IR Concat as Rust
    /// `+`, and its string-ownership rewrite (`.to_string()`)
    /// fires only on a bare literal in an expectation-carrying
    /// position — never on a Concat operand. So a literal-first
    /// assembly (wrap_vec) renders `(("Vec<" + inner) + ">")`,
    /// which rustc rejects (E0369: a `&str` literal cannot be
    /// the left operand of `+` with a String), while a
    /// variable-first assembly (exclaim) renders
    /// `(inner + "!")` — String + &str — and compiles.
    /// Extending the ownership treatment to Concat operands is
    /// emitter work for the M3 walker/driver slices; this
    /// phase freezes the emitter, so wrap_vec's generated code
    /// is asserted byte-for-byte and exclaim carries the
    /// compile leg.
    ///
    /// Phase 29 update (re-pin E-1/E-2 of the unfreeze
    /// inventory): the user approved unfreezing the emitter
    /// for exactly this defect family, and the Concat arm now
    /// renders the left operand owned and the right operand
    /// borrowed. wrap_vec's generated code is therefore
    /// `(("Vec<".to_string() + &inner) + ">")` and BOTH specs
    /// carry a compile leg below; exclaim's rendering is
    /// unchanged (a literal right operand stays a bare &str).
    ///
    /// Phase 39 update (re-pin E-1 of the second unfreeze
    /// inventory): a variable in Concat left position now
    /// takes the owned-copy treatment (`.clone()`), because
    /// a bare rendering MOVES the variable (E0382 when it is
    /// used again). exclaim's generated code is therefore
    /// `(inner.clone() + "!")`; its compile leg below is
    /// unchanged (the clone is value-transparent), and
    /// wrap_vec's byte assertion is unchanged (its variable
    /// sits on the right, borrowed).
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn concat_equation_seed_is_verified_and_wins() {
        let src = r#"
ConcatFixture {
spec wrap_vec {
    doc: "Wrap an already-rendered inner type in Vec<...>: the M3 template shape."
    input: inner: String
    output: String

    constraint: output == "Vec<" ++ inner ++ ">"

    test "plain" { input: "i64" expect: "Vec<i64>" }
    test "empty" { input: "" expect: "Vec<>" }
    test "nested" { input: "Vec<u8>" expect: "Vec<Vec<u8>>" }
}

spec exclaim {
    doc: "Append a fixed suffix to a String: the variable-first assembly shape."
    input: inner: String
    output: String

    constraint: output == inner ++ "!"

    test "plain" { input: "wow" expect: "wow!" }
    test "empty" { input: "" expect: "!" }
}
}
"#;
        let parsed = crate::spec::parser::parse(src).expect("concat fixture must parse");
        let checked =
            crate::spec::typeck::check(parsed).expect("concat fixture must typecheck");
        let module = checked.spec.functions;
        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);

        // ── wrap_vec: seed, verdict, execution, selection ──
        let spec = module_spec(&module, "wrap_vec");
        let seed = constraint_seed(&engine, &spec, "equation");
        assert_eq!(count_holes(&seed.body), 0);
        assert!(
            format!("{:?}", seed.body).contains("Concat"),
            "the equation seed's body must be the concat assembly: {:?}",
            seed.body
        );
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        // Executed through the IR interpreter: the assembled
        // strings are exactly the template's, nested case
        // included — tpr's machinery, checked by hand here.
        for (inner, expected) in [
            ("i64", "Vec<i64>"),
            ("", "Vec<>"),
            ("Vec<u8>", "Vec<Vec<u8>>"),
        ] {
            let mut env = crate::synthesis::eval::Env::new();
            env.insert(
                "inner".into(),
                crate::synthesis::eval::Value::Str(inner.into()),
            );
            let v = crate::synthesis::eval::eval(&seed.body, &env)
                .expect("seed body must evaluate");
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Str(expected.into()),
                "inner {:?}",
                inner
            );
        }

        let pool = synthesize_with_module(&spec, 1, &module)
            .expect("opt-1 synthesis must succeed");
        assert!(
            pool.iter()
                .any(|c| matches!(c.provenance, Provenance::ConstraintInduced { .. })),
            "final pool must contain the constraint seed"
        );
        let verified = verifier.verify_all(pool, &spec).expect("verify_all");
        let winner =
            crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
        assert!(winner.verified, "the spec-exact seed must win verified");
        assert!(
            (winner.scores.test_pass_ratio - 1.0).abs() < 1e-9,
            "winner tpr was {}",
            winner.scores.test_pass_ratio
        );
        assert_eq!(
            format!("{:?}", winner.body),
            format!("{:?}", seed.body),
            "the winning body must be the constraint's own expression"
        );

        // The generated code is the emitter's Concat
        // rendering after the Phase 29 fix, byte-for-byte:
        // left operand owned, right operand borrowed.
        let code = crate::codegen::generate(&winner, "rust").expect("codegen");
        assert!(
            code.contains(r#"(("Vec<".to_string() + &inner) + ">")"#),
            "generated code must render the assembly owned/borrowed:\n{}",
            code
        );

        // And it compiles — the leg this shape could not
        // carry before Phase 29 (same standalone rustc check
        // as exclaim's, below).
        let dir = std::env::temp_dir().join(format!(
            "morphic_concat_e2e_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("wrap_vec.rs");
        std::fs::write(&file, &code).unwrap();
        let out = std::process::Command::new("rustc")
            .args(["--edition", "2021", "--crate-type", "lib"])
            .arg(&file)
            .arg("-o")
            .arg(dir.join("libwrap_vec.rlib"))
            .output()
            .expect("failed to run rustc");
        assert!(
            out.status.success(),
            "generated Rust failed to compile:\n{}\n--- code ---\n{}",
            String::from_utf8_lossy(&out.stderr),
            code
        );

        // ── exclaim: the full chain, compile included ──
        let spec = module_spec(&module, "exclaim");
        let seed = constraint_seed(&engine, &spec, "equation");
        assert_eq!(count_holes(&seed.body), 0);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        let pool = synthesize_with_module(&spec, 1, &module)
            .expect("opt-1 synthesis must succeed");
        let verified = verifier.verify_all(pool, &spec).expect("verify_all");
        let winner =
            crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
        assert!(winner.verified, "the spec-exact seed must win verified");
        assert!(
            (winner.scores.test_pass_ratio - 1.0).abs() < 1e-9,
            "winner tpr was {}",
            winner.scores.test_pass_ratio
        );
        assert_eq!(
            format!("{:?}", winner.body),
            format!("{:?}", seed.body),
            "the winning body must be the constraint's own expression"
        );

        // The generated Rust must compile — the same standalone
        // check the synthesis probes run (rustc --crate-type
        // lib on the emitted file).
        let code = crate::codegen::generate(&winner, "rust").expect("codegen");
        let dir = std::env::temp_dir().join(format!(
            "morphic_concat_e2e_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("exclaim.rs");
        std::fs::write(&file, &code).unwrap();
        let out = std::process::Command::new("rustc")
            .args(["--edition", "2021", "--crate-type", "lib"])
            .arg(&file)
            .arg("-o")
            .arg(dir.join("libexclaim.rlib"))
            .output()
            .expect("failed to run rustc");
        assert!(
            out.status.success(),
            "generated Rust failed to compile:\n{}\n--- code ---\n{}",
            String::from_utf8_lossy(&out.stderr),
            code
        );
    }

    /// Constraint seeds (ii): a lookup spec's guard rows
    /// enumerate the whole table, including rows no in-spec test
    /// exercises (2 → "Float", 3 → "Bool", 7 → "Option" for
    /// type_tag_name). The guard-table seed must return the
    /// pinned output on EVERY row — evaluated here through the
    /// IR interpreter — and be Verified.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_guard_table_seed_covers_every_row() {
        let spec = load_m2_spec("printer", "type_tag_name");
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let seed = constraint_seed(&engine, &spec, "guard_table");
        assert_eq!(count_holes(&seed.body), 0);

        let rows = [
            (1, "Int"),
            (2, "Float"),
            (3, "Bool"),
            (4, "String"),
            (6, "List"),
            (7, "Option"),
        ];
        for (tag, name) in rows {
            let v = eval_int_body(&seed.body, "tag", tag);
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Str(name.into()),
                "tag {} must print as {}",
                tag,
                name
            );
        }
        // Off-table the spec is silent; the wildcard arm carries
        // the typed default for String (Phase 15: the mirrors
        // abstain there and the recorded winner value was "").
        let v = eval_int_body(&seed.body, "tag", 0);
        assert_eq!(v, crate::synthesis::eval::Value::Str(String::new()));

        let result = crate::verify::verifier::Verifier::new().verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);
    }

    /// Constraint seeds (ii, Int table): node_arity's rows pin
    /// six kinds; its seventh constraint (`output >= 0`) is not
    /// a guard row and must be skipped as one — yet still be
    /// PROVEN of the seed (every row value and the Int default
    /// satisfy the floor), so the seed is Verified whole.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_guard_table_seed_for_node_arity() {
        let spec = load_m2_spec("validator", "node_arity");
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the guard-table seed");
        let seed = constraint_seed(&engine, &spec, "guard_table");

        let rows = [(0, 0), (1, 0), (2, 2), (3, 1), (5, 2), (6, 1)];
        for (kind, arity) in rows {
            let v = eval_int_body(&seed.body, "kind", kind);
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Int(arity),
                "kind {} must have arity {}",
                kind,
                arity
            );
        }
        // Unpinned kinds (Call=4, ListLiteral=8) get the typed
        // Int default, which satisfies the spec's floor.
        let v = eval_int_body(&seed.body, "kind", 4);
        assert_eq!(v, crate::synthesis::eval::Value::Int(0));

        let result = crate::verify::verifier::Verifier::new().verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);
    }

    /// Constraint seeds (iii): the two test-less specs have
    /// never had a truth signal of any kind — no tests, hence no
    /// TestInduced seed, and no Verified pool candidate in any
    /// measured draw. Their constraints are equations, so the
    /// equation seed must exist, be Verified, and reach the
    /// final pool under protection.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_equation_seeds_give_testless_specs_verified_candidates() {
        for (file, name) in [("validator", "tags_compatible"), ("printer", "needs_parens")] {
            let spec = load_m2_spec(file, name);
            let engine = SynthesisEngine::new(SynthesisConfig::default());
            let seed = constraint_seed(&engine, &spec, "equation");
            let result = crate::verify::verifier::Verifier::new().verify_one(&seed, &spec);
            assert_eq!(
                result.status,
                VerificationStatus::Verified,
                "{}'s equation seed must verify",
                name
            );
            let pool = synthesize(&spec, 1).expect("opt-1 synthesis must succeed");
            assert!(
                pool.iter()
                    .any(|c| matches!(c.provenance, Provenance::ConstraintInduced { .. })),
                "{}'s final pool must contain the constraint seed",
                name
            );
        }
    }

    /// Constraint seeds (iv): the composition specs' equations
    /// call other specs — the body converts to IR but lies
    /// outside the verifier's encodable subset, so NO seed is
    /// admitted (an unverifiable seed would be exactly the
    /// Inconclusive filler this phase exists to displace). A
    /// spec whose only constraint fits neither pattern (a bare
    /// ordering on output) likewise gets no seed.
    #[test]
    fn constraint_seeds_absent_where_constraints_fit_neither_pattern() {
        for (file, name) in [("validator", "validate_kind_tag"), ("printer", "print_node_kind")] {
            let spec = load_m2_spec(file, name);
            let engine = SynthesisEngine::new(SynthesisConfig::default());
            assert!(
                engine.seed_from_constraints(&spec).is_empty(),
                "{} must not get a constraint seed",
                name
            );
        }

        let mut spec = dummy_spec();
        spec.name = "floor_only".into();
        spec.params = vec![Param {
            name: "x".into(),
            ty: TypeRef::Int,
            annotations: HashMap::new(),
        }];
        spec.return_type = TypeRef::Int;
        spec.postconditions = vec![Constraint::Expr(Expr::BinOp(
            BinOp::Ge,
            Box::new(Expr::Var("output".into())),
            Box::new(Expr::IntLit(0)),
        ))];
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        assert!(engine.seed_from_constraints(&spec).is_empty());
    }

    /// Load every spec of one M2 module file — the module
    /// context spec-to-spec calls resolve against.
    fn load_m2_module(file: &str) -> Vec<FunctionSpec> {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/self_host/{}.morph", root, file);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = crate::spec::parser::parse(&source).unwrap();
        let checked = crate::spec::typeck::check(parsed).unwrap();
        checked.spec.functions
    }

    fn module_spec(module: &[FunctionSpec], name: &str) -> FunctionSpec {
        module.iter().find(|f| f.name == name).unwrap().clone()
    }

    fn contains_call_to(node: &IRNode, name: &str) -> bool {
        match node {
            IRNode::Call { function, args } => {
                function == name || args.iter().any(|a| contains_call_to(a, name))
            }
            IRNode::BinOp(_, l, r) => {
                contains_call_to(l, name) || contains_call_to(r, name)
            }
            IRNode::UnaryOp(_, inner) => contains_call_to(inner, name),
            IRNode::Match { scrutinee, cases } => {
                contains_call_to(scrutinee, name)
                    || cases.iter().any(|(_, b)| contains_call_to(b, name))
            }
            _ => false,
        }
    }

    /// Spec calls (i): with the module present, validate_kind_tag's
    /// equation — `output == is_known_node_kind(kind)` — finally
    /// yields a seed: the call is resolved against the callee's
    /// own constraint body, so the seed IS the range check the
    /// composed spec denotes. It must be Verified by the
    /// module-aware verifier and win module-aware synthesis at
    /// tpr 1.000 with its own body. (Without the module the same
    /// spec gets no seed — see
    /// constraint_seeds_absent_where_constraints_fit_neither_pattern,
    /// which runs registry-less and is unchanged.)
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn composition_equation_seed_is_verified_and_wins() {
        let module = load_m2_module("validator");
        let spec = module_spec(&module, "validate_kind_tag");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seed = constraint_seed(&engine, &spec, "equation");
        assert_eq!(count_holes(&seed.body), 0);
        assert!(
            !contains_call_to(&seed.body, "is_known_node_kind"),
            "the seed body must be the resolved callee body, not the call: {:?}",
            seed.body
        );
        // The resolved body is the callee's range, executed here
        // through the IR interpreter — including the untested
        // boundary points 9 (in) and 10 (out).
        for (kind, expected) in [(0, true), (4, true), (9, true), (10, false), (77, false)] {
            let v = eval_int_body(&seed.body, "kind", kind);
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Bool(expected),
                "kind {}",
                kind
            );
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        let pool = synthesize_with_module(&spec, 1, &module)
            .expect("opt-1 synthesis must succeed");
        assert!(
            pool.iter()
                .any(|c| matches!(c.provenance, Provenance::ConstraintInduced { .. })),
            "final pool must contain the constraint seed"
        );
        let verified = verifier.verify_all(pool, &spec).expect("verify_all");
        let winner =
            crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
        assert!(winner.verified, "the spec-exact seed must win verified");
        assert!(
            (winner.scores.test_pass_ratio - 1.0).abs() < 1e-9,
            "winner tpr was {}",
            winner.scores.test_pass_ratio
        );
        assert_eq!(
            format!("{:?}", winner.body),
            format!("{:?}", seed.body),
            "the winning body must be the resolved seed body"
        );
    }

    /// Spec calls (ii): print_node_kind composes node_kind_name —
    /// a guard-table spec with NO equation — so the resolver
    /// falls back to the callee's guard-table body and the seed
    /// is the name lookup itself. Verified by the module-aware
    /// verifier, and wins module-aware synthesis at tpr 1.000.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn composition_seed_for_print_node_kind_is_verified_and_wins() {
        let module = load_m2_module("printer");
        let spec = module_spec(&module, "print_node_kind");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seed = constraint_seed(&engine, &spec, "equation");
        assert_eq!(count_holes(&seed.body), 0);
        assert!(
            !contains_call_to(&seed.body, "node_kind_name"),
            "the seed body must be the resolved callee body, not the call: {:?}",
            seed.body
        );
        // The lookup itself, on rows no in-spec test exercises
        // (0 → "Literal") as well as both tested rows.
        for (kind, expected) in [(0, "Literal"), (2, "BinaryOp"), (7, "Lambda")] {
            let v = eval_int_body(&seed.body, "kind", kind);
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Str(expected.into()),
                "kind {}",
                kind
            );
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        let pool = synthesize_with_module(&spec, 1, &module)
            .expect("opt-1 synthesis must succeed");
        assert!(
            pool.iter()
                .any(|c| matches!(c.provenance, Provenance::ConstraintInduced { .. })),
            "final pool must contain the constraint seed"
        );
        let verified = verifier.verify_all(pool, &spec).expect("verify_all");
        let winner =
            crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
        assert!(winner.verified, "the spec-exact seed must win verified");
        assert!(
            (winner.scores.test_pass_ratio - 1.0).abs() < 1e-9,
            "winner tpr was {}",
            winner.scores.test_pass_ratio
        );
        assert_eq!(
            format!("{:?}", winner.body),
            format!("{:?}", seed.body),
            "the winning body must be the resolved seed body"
        );
    }

    /// Spec calls (iii): a cyclic pair of specs (a calls b, b
    /// calls a) must not hang or panic resolution — the cycle
    /// simply yields no seed, exactly like an unresolvable call.
    /// This test completing at all is the termination evidence.
    #[test]
    fn cyclic_specs_get_no_constraint_seed() {
        let cyclic = |name: &str, callee: &str| {
            let mut spec = dummy_spec();
            spec.name = name.into();
            spec.params = vec![Param {
                name: "x".into(),
                ty: TypeRef::Int,
                annotations: HashMap::new(),
            }];
            spec.return_type = TypeRef::Bool;
            spec.postconditions = vec![Constraint::Expr(Expr::BinOp(
                BinOp::Eq,
                Box::new(Expr::Var("output".into())),
                Box::new(Expr::Call(callee.into(), vec![Expr::Var("x".into())])),
            ))];
            spec
        };
        let a = cyclic("cyc_a", "cyc_b");
        let b = cyclic("cyc_b", "cyc_a");
        let module = vec![a.clone(), b];
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        assert!(engine.seed_from_constraints(&a).is_empty());
        assert!(engine.seed_from_constraints(&module[1]).is_empty());
    }

    // ── Phase 24: guard-table generalizations ─────────────
    //
    // The two seedless shapes of the Phase 23 measurement.
    // Both were already verifier-encodable (their pools held
    // Verified candidates in every run); only the builders
    // refused them: guard rows over a finite Bool domain
    // (rust_literal_bool — guard_table_body admitted Int
    // params only, guard_leaf read IntLit constants, and the
    // pinned value came from an integer-interval scan) and a
    // guarded identity (rust_pattern_ctor_name — two params,
    // a Bool-equality guard leaf, and an output pin that is a
    // parameter Var where the guard table required a literal).

    fn load_codegen_module() -> Vec<FunctionSpec> {
        load_m2_module("codegen")
    }

    fn eval_seed_body(
        body: &IRNode,
        bindings: &[(&str, crate::synthesis::eval::Value)],
    ) -> crate::synthesis::eval::Value {
        let mut env = crate::synthesis::eval::Env::new();
        for (name, value) in bindings {
            env.insert((*name).into(), value.clone());
        }
        crate::synthesis::eval::eval(body, &env).expect("seed body must evaluate")
    }

    /// Guard generalizations (i): rust_literal_bool's rows pin
    /// the whole finite Bool domain. The guard-table seed must
    /// now exist — one arm per pinned row in spec order, then
    /// the typed-literal wildcard (unreachable on a total Bool
    /// table, retained by the match convention the verifier's
    /// fall-through rule requires) — be Verified in module
    /// context, and win module-aware synthesis with its own
    /// body: the same shape its Phase 23 pool winner took, now
    /// structural instead of drawn.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_guard_table_seed_for_bool_param() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_literal_bool");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the guard-table seed");
        let seed = constraint_seed(&engine, &spec, "guard_table");
        assert_eq!(count_holes(&seed.body), 0);

        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("value".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Bool(true)),
                    IRNode::Literal(IRLiteral::String("true".into())),
                ),
                (
                    IRPattern::Literal(IRLiteral::Bool(false)),
                    IRNode::Literal(IRLiteral::String("false".into())),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::String(String::new())),
                ),
            ],
        };
        assert_eq!(format!("{:?}", seed.body), format!("{:?}", expected));

        // Executed over the whole finite domain.
        for (value, text) in [(true, "true"), (false, "false")] {
            let v = eval_seed_body(
                &seed.body,
                &[("value", crate::synthesis::eval::Value::Bool(value))],
            );
            assert_eq!(v, crate::synthesis::eval::Value::Str(text.into()));
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        let pool = synthesize_with_module(&spec, 1, &module)
            .expect("opt-1 synthesis must succeed");
        let verified = verifier.verify_all(pool, &spec).expect("verify_all");
        let winner =
            crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
        assert!(winner.verified, "the spec-exact seed must win verified");
        assert!(
            (winner.scores.test_pass_ratio - 1.0).abs() < 1e-9,
            "winner tpr was {}",
            winner.scores.test_pass_ratio
        );
        assert_eq!(
            format!("{:?}", winner.body),
            format!("{:?}", seed.body),
            "the winning body must be the guard table itself"
        );
    }

    fn var_expr(name: &str) -> Expr {
        Expr::Var(name.into())
    }

    fn eq_expr(l: Expr, r: Expr) -> Expr {
        Expr::BinOp(BinOp::Eq, Box::new(l), Box::new(r))
    }

    /// One guard-row constraint: the Or of the given leaves.
    fn or_row(leaves: Vec<Expr>) -> Constraint {
        let mut it = leaves.into_iter();
        let first = it.next().expect("a row needs at least one leaf");
        Constraint::Expr(it.fold(first, |acc, leaf| {
            Expr::BinOp(BinOp::Or, Box::new(acc), Box::new(leaf))
        }))
    }

    fn guard_spec(
        name: &str,
        params: Vec<(&str, TypeRef)>,
        return_type: TypeRef,
        postconditions: Vec<Constraint>,
    ) -> FunctionSpec {
        let mut spec = dummy_spec();
        spec.name = name.into();
        spec.params = params
            .into_iter()
            .map(|(n, ty)| Param {
                name: n.into(),
                ty,
                annotations: HashMap::new(),
            })
            .collect();
        spec.return_type = return_type;
        spec.postconditions = postconditions;
        spec
    }

    /// Guard generalizations (i, partial): a Bool table with a
    /// single row pins one domain value; the other falls to
    /// the wildcard — the typed-literal default decides it,
    /// exactly as an Int table's unpinned inputs are decided.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_guard_table_seed_for_partial_bool_table() {
        // `value==true||output=="false"` pins value == false.
        let spec = guard_spec(
            "bool_partial",
            vec![("value", TypeRef::Bool)],
            TypeRef::String,
            vec![or_row(vec![
                eq_expr(var_expr("value"), Expr::BoolLit(true)),
                eq_expr(var_expr("output"), Expr::StringLit("false".into())),
            ])],
        );
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let seed = constraint_seed(&engine, &spec, "guard_table");
        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("value".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Bool(false)),
                    IRNode::Literal(IRLiteral::String("false".into())),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::String(String::new())),
                ),
            ],
        };
        assert_eq!(format!("{:?}", seed.body), format!("{:?}", expected));
        let v = eval_seed_body(
            &seed.body,
            &[("value", crate::synthesis::eval::Value::Bool(false))],
        );
        assert_eq!(v, crate::synthesis::eval::Value::Str("false".into()));
        let v = eval_seed_body(
            &seed.body,
            &[("value", crate::synthesis::eval::Value::Bool(true))],
        );
        assert_eq!(v, crate::synthesis::eval::Value::Str(String::new()));

        let result = crate::verify::verifier::Verifier::new().verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);
    }

    /// Guard generalizations (ii): rust_pattern_ctor_name's
    /// single row pins output to the `name` parameter where
    /// `has_subs` is false; the complementary branch is
    /// honestly unpinned and falls to the wildcard. The seed
    /// must exist (kind `guarded_identity`), be Verified in
    /// module context, and win module-aware synthesis
    /// deterministically — Phase 23 drew a different body per
    /// run, with the silent branch returning "" or the name by
    /// luck; the seed fixes it to the wildcard "".
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_guarded_identity_seed_for_ctor_name() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_pattern_ctor_name");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the guarded-identity seed");
        let seed = constraint_seed(&engine, &spec, "guarded_identity");
        assert_eq!(count_holes(&seed.body), 0);

        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("has_subs".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Bool(false)),
                    IRNode::Var("name".into()),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::String(String::new())),
                ),
            ],
        };
        assert_eq!(format!("{:?}", seed.body), format!("{:?}", expected));

        // The pinned branch returns the name itself; the
        // spec-silent branch returns the wildcard default.
        let v = eval_seed_body(
            &seed.body,
            &[
                ("name", crate::synthesis::eval::Value::Str("Some".into())),
                ("has_subs", crate::synthesis::eval::Value::Bool(false)),
            ],
        );
        assert_eq!(v, crate::synthesis::eval::Value::Str("Some".into()));
        let v = eval_seed_body(
            &seed.body,
            &[
                ("name", crate::synthesis::eval::Value::Str("Some".into())),
                ("has_subs", crate::synthesis::eval::Value::Bool(true)),
            ],
        );
        assert_eq!(v, crate::synthesis::eval::Value::Str(String::new()));

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        // Deterministic across draws: the same winning body in
        // three independent synthesis runs.
        for _ in 0..3 {
            let pool = synthesize_with_module(&spec, 1, &module)
                .expect("opt-1 synthesis must succeed");
            let verified = verifier.verify_all(pool, &spec).expect("verify_all");
            let winner =
                crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
            assert!(winner.verified, "the identity seed must win verified");
            assert_eq!(
                format!("{:?}", winner.body),
                format!("{:?}", seed.body),
                "the winning body must be the identity seed in every draw"
            );
        }
    }

    /// Guard generalizations (ii, Int guards): the identity
    /// builder reuses the Int row recognition too — a row
    /// pinning an Int guard parameter by comparisons and the
    /// output to the remaining parameter.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_guarded_identity_seed_with_int_guard() {
        // `kind<0||kind>0||output==name` pins kind == 0.
        let spec = guard_spec(
            "int_guarded_identity",
            vec![("name", TypeRef::String), ("kind", TypeRef::Int)],
            TypeRef::String,
            vec![or_row(vec![
                Expr::BinOp(
                    BinOp::Lt,
                    Box::new(var_expr("kind")),
                    Box::new(Expr::IntLit(0)),
                ),
                Expr::BinOp(
                    BinOp::Gt,
                    Box::new(var_expr("kind")),
                    Box::new(Expr::IntLit(0)),
                ),
                eq_expr(var_expr("output"), var_expr("name")),
            ])],
        );
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let seed = constraint_seed(&engine, &spec, "guarded_identity");
        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(0)),
                    IRNode::Var("name".into()),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::String(String::new())),
                ),
            ],
        };
        assert_eq!(format!("{:?}", seed.body), format!("{:?}", expected));
        let v = eval_seed_body(
            &seed.body,
            &[
                ("name", crate::synthesis::eval::Value::Str("Unit".into())),
                ("kind", crate::synthesis::eval::Value::Int(0)),
            ],
        );
        assert_eq!(v, crate::synthesis::eval::Value::Str("Unit".into()));
        let v = eval_seed_body(
            &seed.body,
            &[
                ("name", crate::synthesis::eval::Value::Str("Unit".into())),
                ("kind", crate::synthesis::eval::Value::Int(3)),
            ],
        );
        assert_eq!(v, crate::synthesis::eval::Value::Str(String::new()));

        let result = crate::verify::verifier::Verifier::new().verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);
    }

    /// Guard generalizations (negatives): near-miss shapes
    /// still get NO constraint seed — the generalizations stay
    /// as conservative as the builders they extend. Each spec
    /// below deviates in exactly one way.
    #[test]
    fn constraint_seed_near_misses_get_no_seed() {
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let mut cases: Vec<FunctionSpec> = Vec::new();

        // Bool table whose pin is not a literal (output == the
        // parameter itself): not a table row, and a single
        // param cannot be a guarded identity. Phase 34: the
        // composition precedence does not claim it either —
        // the bare-variable pin names the Bool guard
        // parameter, whose type is not the return type, so
        // the pin cannot BE the output (the precedence
        // branch's return-type condition).
        cases.push(guard_spec(
            "bool_pin_not_literal",
            vec![("value", TypeRef::Bool)],
            TypeRef::String,
            vec![or_row(vec![
                eq_expr(var_expr("value"), Expr::BoolLit(false)),
                eq_expr(var_expr("output"), var_expr("value")),
            ])],
        ));

        // Bool row whose guards falsify nowhere (both domain
        // values satisfy some leaf): pins no single value.
        cases.push(guard_spec(
            "bool_row_pins_nothing",
            vec![("value", TypeRef::Bool)],
            TypeRef::String,
            vec![or_row(vec![
                eq_expr(var_expr("value"), Expr::BoolLit(true)),
                eq_expr(var_expr("value"), Expr::BoolLit(false)),
                eq_expr(var_expr("output"), Expr::StringLit("x".into())),
            ])],
        ));

        // The output pin is a compound expression (a call to
        // a spec the index does not hold), not exactly the
        // remaining parameter: no family's pin admission
        // takes an unindexed call.
        cases.push(guard_spec(
            "identity_pin_compound",
            vec![("name", TypeRef::String), ("has_subs", TypeRef::Bool)],
            TypeRef::String,
            vec![or_row(vec![
                eq_expr(var_expr("has_subs"), Expr::BoolLit(true)),
                eq_expr(
                    var_expr("output"),
                    Expr::Call("render".into(), vec![var_expr("name")]),
                ),
            ])],
        ));

        // A guard leaf that is not a literal comparison
        // (parameter against parameter).
        cases.push(guard_spec(
            "identity_guard_not_literal",
            vec![("name", TypeRef::String), ("has_subs", TypeRef::Bool)],
            TypeRef::String,
            vec![or_row(vec![
                eq_expr(var_expr("has_subs"), var_expr("name")),
                eq_expr(var_expr("output"), var_expr("name")),
            ])],
        ));

        // Phase 34 reclassification: three shapes stood here
        // until this phase — a guarded-identity spec with a
        // second row pinning a LITERAL, a spec whose pin is
        // the GUARD parameter itself, and a three-parameter
        // bare-variable pin. Each was recorded against
        // GUARDED IDENTITY's predicate, and guarded identity
        // still refuses all three on that predicate. What
        // changed is the composition family's territory rule:
        // restated as a precedence (right of first refusal),
        // it claims exactly the plain-guard, literal /
        // bare-variable-pin shapes the earlier families
        // refuse — see
        // constraint_composition_precedence_both_directions,
        // where the three shapes are asserted as
        // composition-guard seeds whose bodies honor their
        // rows.

        for spec in &cases {
            assert!(
                engine.seed_from_constraints(spec).is_empty(),
                "{}: near-miss shape must get no constraint seed",
                spec.name
            );
        }
    }

    // ── Phase 49: char-equation seeds ─────────────────────
    //
    // The tokenizer's three Char classifiers (is_whitespace,
    // is_ident_start, is_ident_continue) state total Bool
    // equations over one Char parameter — CharLit equalities
    // and Char range comparisons joined by ||/&&. Phase 48
    // measured: no seed reached any of their pools in any
    // run, no pool contained an exact body, and the crowned
    // winners were wildcard-`true` test-induced completions
    // agreeing with the mirrors on only 5/774, 54/774 and
    // 64/774 of the char battery.
    //
    // The diagnosis, pinned by
    // char_equation_diagnosis_why_the_eight_families_refuse
    // below (every assertion in it holds before AND after
    // this phase — it documents the other families, which do
    // not change): [Phase 76: this framing is superseded —
    // the test is re-stated as
    // d5a_diagnosis_equation_family_now_admits_classifiers,
    // and the equation bullet below is the pre-D5a record;
    // see the Phase 76 re-statement at the end of this
    // section.]
    //
    //  - equation: the shape MATCHES — equation_rhs converts
    //    each postcondition (CharLit → IR Char literal,
    //    comparisons → BinOp, no holes) and inline_calls
    //    resolves is_ident_continue's call. The refusal is
    //    the admission gate: body_is_encodable_with runs
    //    sym_eval, the Char parameter binds Sym::Opaque,
    //    Char literals encode Opaque, and sym_binop has no
    //    (Eq|Neq|Lt|Le|Gt|Ge, Opaque, Opaque) arm — an
    //    Opaque operand makes the whole expression
    //    unencodable. A Z3-encodability gate, not a shape
    //    refusal.
    //  - guard_table: param-type filter — the dispatch
    //    requires an Int-family or Bool parameter; Char
    //    falls through to None. (The postcondition is an
    //    equation, not guard rows, besides.)
    //  - guarded_identity: arity filter — exactly two
    //    parameters (guard + identity target); the
    //    classifiers have one.
    //  - composition_guard / multi_guard: shape matchers —
    //    the postcondition is not a guard row
    //    (guard_row_parts returns None, so no rows parse),
    //    and their leaves must compare a guard expression
    //    against BoolLit constants, not a Char parameter
    //    against CharLits. Both wrappers also apply the
    //    encodability gate.
    //  - expr_pin_guard: key-type filter — the key search
    //    tries Int-family parameters only; a Char parameter
    //    is never tried. Guard rows required besides, and
    //    the encodability gate applies.
    //  - pair_guard_table: arity + key-type filters —
    //    exactly two parameters, both Int-family.
    //  - mixed_pair_guard_table: arity + key-signature
    //    filter — exactly two parameters, one Int-family
    //    and one Bool.
    //
    // The fix is a NINTH family, char_equation — not an
    // equation-family extension, for two recorded reasons:
    // (1) the admission standard differs categorically:
    // equation seeds are admitted BECAUSE Z3 can judge them
    // (every measured equation winner is Verified), while
    // char-equation seeds are admitted knowing Z3 CANNOT
    // judge them — their verdict is Inconclusive by
    // construction and their evidence standard is executed
    // agreement over the char battery; one kind name
    // covering both would falsify the equation family's
    // recorded meaning. (2) The established pattern: each
    // admitted shape with a distinct discriminator got its
    // own family and kind name (Phases 28/32/33/34/38), and
    // a separate builder leaves equation_seed_body — and
    // therefore all 54 measured equation bodies — literally
    // untouched.
    //
    // Territory disjointness: the signature discriminator
    // (exactly one parameter, of type Char; Bool return)
    // is a region the seven row/key families refuse on
    // their stated filters above, and against the equation
    // family the builder carries the Phase 34 precedence:
    // it admits only when equation_seed_body returns None
    // (right of first refusal — if the equation family can
    // state the spec encodably, it owns it). Within the
    // shape, the inlined body must be a pure Char-classifier
    // tree: ||/&&/! over comparison leaves, each leaf
    // comparing the parameter against a Char literal (in
    // either order), with at least one leaf — so constant
    // equations, param-vs-param comparisons and arithmetic
    // are refused even where the precedence would let them
    // through. The body is the postcondition's own
    // right-hand side with spec calls inlined — the
    // equation family's body, verbatim.
    //
    // ── Phase 76 re-statement (D5a landed): the Char
    // scalar encoding retires this family's premise. The
    // three classifiers' bodies are now encodable, so the
    // equation family's admission gate opens for them and
    // — by char_equation's own precedence rule — the
    // equation family owns them: their seeds are kind
    // "equation", and Z3's verdict on them is Verified,
    // not Inconclusive-by-construction. Every tree
    // char_classifier_tree_leaves accepts is a Bool tree
    // over param-vs-CharLit comparisons, all encodable
    // under D5a, so char_equation's territory is empty in
    // practice; the builder remains as the recorded
    // fallback for a classifier tree the equation family
    // cannot encode. The tests below were re-stated
    // accordingly (the diagnosis's encodability assertions
    // flipped; the seed-kind and verdict assertions moved
    // to the equation family; the 774/774 executed
    // agreement re-measured on the verified seed as a
    // cross-check, per the audit's D5a terms — re-measured,
    // not assumed carried). The seven row/key families'
    // filter refusals are unchanged and still pinned.

    fn load_tokenizer_module() -> Vec<FunctionSpec> {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/functional_tokenizer.morph", root);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = crate::spec::parser::parse(&source).unwrap();
        let checked = crate::spec::typeck::check(parsed).unwrap();
        checked.spec.functions
    }

    fn tokenizer_spec(module: &[FunctionSpec], name: &str) -> FunctionSpec {
        module.iter().find(|f| f.name == name).unwrap().clone()
    }

    /// The Phase 45 char battery, as the cross-validation
    /// harness builds it: U+0000..=U+02FF plus six probes.
    fn phase49_char_battery() -> Vec<char> {
        let mut battery: Vec<char> = ('\u{0}'..='\u{2FF}').collect();
        battery.extend(['\u{660}', '\u{6F0}', '\u{B2}', '\u{FF10}', '😀', '\u{10FFFF}']);
        battery
    }

    fn eval_char_body(body: &IRNode, c: char) -> bool {
        let mut env = crate::synthesis::eval::Env::new();
        env.insert("c".into(), crate::synthesis::eval::Value::Char(c));
        match crate::synthesis::eval::eval(body, &env) {
            Ok(crate::synthesis::eval::Value::Bool(b)) => b,
            other => panic!("classifier body not Bool-evaluable on {:?}: {:?}", c, other),
        }
    }

    /// The diagnosis, as measured (see the section
    /// comment). Phase 76 (D5a) flipped exactly one half of
    /// it: the equation family's refusal point is GONE —
    /// the classifier bodies now encode (Char is a scalar
    /// sort), the equation family admits them, and
    /// char_equation's precedence rule therefore leaves it
    /// nothing. The seven row/key families' refusals are
    /// the Phase 49 facts, unchanged: they refuse on their
    /// stated filters, not on encodability.
    #[test]
    fn d5a_diagnosis_equation_family_now_admits_classifiers() {
        let module = load_tokenizer_module();
        let index = SpecIndex::from_specs(&module);
        for name in ["is_whitespace", "is_ident_start", "is_ident_continue"] {
            let spec = tokenizer_spec(&module, name);
            // The equation shape converts and resolves.
            let raw = first_equation_body(&spec)
                .unwrap_or_else(|| panic!("{}: equation_rhs must convert", name));
            let inlined = index
                .inline_calls(&raw)
                .unwrap_or_else(|| panic!("{}: calls must resolve", name));
            // D5a: the body is encodable now — Char binds
            // as its scalar value and Char comparisons
            // encode (the Phase 49 gate is retired).
            assert!(
                crate::verify::verifier::body_is_encodable_with(&spec, &inlined, &index),
                "{}: classifier body must be encodable after D5a",
                name
            );
            assert!(
                equation_seed_body(&spec, &index).is_some(),
                "{}: equation family must admit at its (now open) gate",
                name
            );
            // char_equation's precedence leaves it nothing:
            // the equation family owns every classifier tree.
            assert!(
                char_equation_seed_body(&spec, &index).is_none(),
                "{}: char_equation must yield to the equation family",
                name
            );
            // Every other family refuses on its stated
            // filter (param type / arity / row shape / key
            // type — see the section comment).
            assert!(guard_table_body(&spec).is_none(), "{}: guard_table", name);
            assert!(guarded_identity_body(&spec).is_none(), "{}: guarded_identity", name);
            assert!(
                composition_guard_body(&spec, &index).is_none(),
                "{}: composition_guard",
                name
            );
            assert!(multi_guard_body(&spec, &index).is_none(), "{}: multi_guard", name);
            assert!(
                expr_pin_guard_body(&spec, &index).is_none(),
                "{}: expr_pin_guard",
                name
            );
            assert!(pair_guard_table_body(&spec).is_none(), "{}: pair_guard_table", name);
            assert!(
                mixed_pair_guard_table_body(&spec).is_none(),
                "{}: mixed_pair_guard_table",
                name
            );
        }
    }

    /// The three classifiers each gain exactly one seed —
    /// since Phase 76 (D5a), of kind equation: the
    /// postcondition's own right-hand side (calls inlined),
    /// admitted at the equation family's Z3 gate and
    /// Verified by it. (Phase 49's form of this test pinned
    /// the char_equation kind and an Inconclusive verdict —
    /// the pre-D5a admission standard, retired; see the
    /// section comment.)
    #[test]
    fn equation_seed_for_tokenizer_classifiers_is_verified() {
        let module = load_tokenizer_module();
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        for name in ["is_whitespace", "is_ident_start", "is_ident_continue"] {
            let spec = tokenizer_spec(&module, name);
            let seeds = engine.seed_from_constraints(&spec);
            assert_eq!(
                seeds.len(),
                1,
                "{}: exactly the equation seed",
                name
            );
            assert!(
                !seeds.iter().any(|c| matches!(
                    &c.provenance,
                    Provenance::ConstraintInduced { kind } if kind == "char_equation"
                )),
                "{}: no char_equation seed after D5a",
                name
            );
            let seed = constraint_seed(&engine, &spec, "equation");
            assert_eq!(count_holes(&seed.body), 0, "{}: seed must be hole-free", name);
            // Module-aware, as the pipeline judges:
            // is_ident_continue's constraint calls
            // is_ident_start, which only the module index
            // can inline on the constraint side.
            let result =
                crate::verify::verifier::Verifier::with_specs(&module)
                    .verify_one(&seed, &spec);
            // The verdict is configuration-shaped, per the
            // standing pattern: with Z3 the seed is proven;
            // the no-Z3 build returns the honest
            // Inconclusive (the playground contract) — the
            // seed itself is admitted in both, because
            // admission runs on the pure encodability
            // predicate.
            #[cfg(feature = "z3-support")]
            assert_eq!(
                result.status,
                crate::verify::verifier::VerificationStatus::Verified,
                "{}: Z3 must verify the classifier seed after D5a",
                name
            );
            #[cfg(not(feature = "z3-support"))]
            assert_eq!(
                result.status,
                crate::verify::verifier::VerificationStatus::Inconclusive,
                "{}: the no-Z3 build must stay honestly Inconclusive",
                name
            );
        }
        // is_whitespace / is_ident_start: the seed body IS
        // the resolver-visible equation body, verbatim.
        let index = SpecIndex::from_specs(&module);
        for name in ["is_whitespace", "is_ident_start"] {
            let spec = tokenizer_spec(&module, name);
            let seed = constraint_seed(&engine, &spec, "equation");
            assert_eq!(
                format!("{:?}", seed.body),
                format!("{:?}", index.callee_body(name).unwrap()),
                "{}: seed body must be the postcondition RHS verbatim",
                name
            );
        }
        // is_ident_continue: the seed body is the INLINED
        // tree — is_ident_start's disjunction substituted
        // for the call, the digit range appended.
        let spec = tokenizer_spec(&module, "is_ident_continue");
        let seed = constraint_seed(&engine, &spec, "equation");
        let ch = |c: char| IRNode::Literal(IRLiteral::Char(c));
        let var = || IRNode::Var("c".to_string());
        let bin = |op: IRBinOp, l: IRNode, r: IRNode| {
            IRNode::BinOp(op, Box::new(l), Box::new(r))
        };
        let expected = bin(
            IRBinOp::Or,
            bin(
                IRBinOp::Or,
                bin(
                    IRBinOp::Or,
                    bin(IRBinOp::And, bin(IRBinOp::Ge, var(), ch('a')), bin(IRBinOp::Le, var(), ch('z'))),
                    bin(IRBinOp::And, bin(IRBinOp::Ge, var(), ch('A')), bin(IRBinOp::Le, var(), ch('Z'))),
                ),
                bin(IRBinOp::Eq, var(), ch('_')),
            ),
            bin(IRBinOp::And, bin(IRBinOp::Ge, var(), ch('0')), bin(IRBinOp::Le, var(), ch('9'))),
        );
        assert_eq!(
            format!("{:?}", seed.body),
            format!("{:?}", expected),
            "is_ident_continue: seed body must be the inlined classifier tree"
        );
    }

    /// Executed agreement over the full 774-char battery,
    /// re-measured after D5a (the audit's terms: the
    /// standing result is re-measured, not assumed
    /// carried). The seed is now the equation family's and
    /// carries a Z3 proof (previous test); this battery is
    /// the cross-check that the VERIFIED body also agrees
    /// with the hand mirror on every executed char — a
    /// Verified claim contradicted here would be a
    /// soundness bug, not a statistic.
    #[test]
    fn equation_seed_executes_exactly_over_char_battery() {
        let module = load_tokenizer_module();
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let battery = phase49_char_battery();
        assert_eq!(battery.len(), 774);
        let mirrors: Vec<(&str, fn(char) -> bool)> = vec![
            ("is_whitespace", |c: char| matches!(c, ' ' | '\t' | '\n' | '\r')),
            ("is_ident_start", |c: char| c.is_ascii_alphabetic() || c == '_'),
            ("is_ident_continue", |c: char| c.is_ascii_alphanumeric() || c == '_'),
        ];
        for (name, mirror) in mirrors {
            let spec = tokenizer_spec(&module, name);
            let seed = constraint_seed(&engine, &spec, "equation");
            let agree = battery
                .iter()
                .filter(|&&c| eval_char_body(&seed.body, c) == mirror(c))
                .count();
            assert_eq!(
                agree,
                774,
                "{}: seed must execute exactly over the battery (got {}/774)",
                name,
                agree
            );
        }
    }

    fn load_tokenizer_spec() -> crate::spec::ast::Spec {
        let root = env!("CARGO_MANIFEST_DIR");
        let path = format!("{}/examples/functional_tokenizer.morph", root);
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = crate::spec::parser::parse(&source).unwrap();
        crate::spec::typeck::check(parsed).unwrap().spec
    }

    /// Phase 81 (M1 remainder (i)): the complementary-guard
    /// builder denotes `peek` — the two-row spec over a
    /// computed comparison guard that every earlier builder
    /// returned None for — and the denotation enters peek's
    /// synthesis pool as a seed.
    #[test]
    fn phase81_complementary_guard_denotes_peek_and_seeds() {
        let module = load_tokenizer_spec();
        let index = SpecIndex::from_module(&module);
        let body = index
            .callee_body("peek")
            .expect("peek must denote after Phase 81");
        assert!(
            matches!(body, IRNode::If { .. }),
            "peek's denotation must be the complementary-guard If, got {:?}",
            body
        );
        let engine =
            SynthesisEngine::with_module_spec(SynthesisConfig::default(), &module);
        let peek = tokenizer_spec(&module.functions, "peek");
        let kinds = engine.constraint_seed_kinds(&peek);
        assert!(
            kinds.iter().any(|k| k == "complementary_guard"),
            "peek's seed kinds must include complementary_guard, got {:?}",
            kinds
        );
    }

    /// Phase 81: complement detection stays syntactic and
    /// exact — the decline side of the builder.
    #[test]
    fn phase81_complementary_guard_declines() {
        let spec_of = |src: &str| {
            crate::spec::parser::parse(src)
                .unwrap()
                .functions
                .into_iter()
                .next()
                .unwrap()
        };
        // Non-complementary guards (both Lt): no denotation.
        let spec = spec_of(
            "T { spec f { input: x: Int output: Int \
             constraint: x<0||output==1 constraint: x<5||output==2 } }",
        );
        assert!(complementary_guard_body(&spec).is_none());
        // Swapped operands are NOT expr_same operands: the
        // complement of `x<0` stated as `0>=x` is declined.
        let spec = spec_of(
            "T { spec f { input: x: Int output: Int \
             constraint: x<0||output==1 constraint: 0>=x||output==2 } }",
        );
        assert!(complementary_guard_body(&spec).is_none());
        // Three rows: not exactly two postconditions.
        let spec = spec_of(
            "T { spec f { input: x: Int output: Int \
             constraint: x<0||output==1 constraint: x>=0||output==2 \
             constraint: output==output } }",
        );
        assert!(complementary_guard_body(&spec).is_none());
        // The Not pair is a complement at the AST level
        // (the Phase 80 pricing admits it). It is not
        // reachable from parsed text today — the parser
        // hoists a leading `!` to a constraint-level Not
        // and rejects `!(…)` in guard-leaf position — so it
        // is pinned here on constructed expressions, and
        // the reachable complement forms are the comparison
        // pairs (peek's Lt/Ge, next_char's Eq/Neq).
        let g = Expr::BinOp(
            BinOp::Lt,
            Box::new(Expr::Var("x".into())),
            Box::new(Expr::IntLit(0)),
        );
        let not_g = Expr::UnaryOp(UnaryOp::Not, Box::new(g.clone()));
        assert!(guards_complementary(&not_g, &g));
        assert!(guards_complementary(&g, &not_g));
        assert!(!guards_complementary(&g, &g));
    }

    /// Phase 81 (M1 remainder (i-b)): the record-completion
    /// builder constructs next_char's body from its partial
    /// field equations, and its seed — with the peek call in
    /// the guard resolved — enters next_char's pool.
    #[test]
    fn phase81_record_completion_builds_next_char() {
        let module = load_tokenizer_spec();
        let index = SpecIndex::from_module(&module);
        let spec = tokenizer_spec(&module.functions, "next_char");
        let body = record_completion_body(&spec, &index)
            .expect("next_char must complete after Phase 81");
        // Phase 87 re-statement: the guard feeds two
        // fields (line, col), so the builder binds it
        // once in a Let and the fields test the bound
        // name — the Phase 81 shape cloned the guard
        // tree (the inlined peek: a full source scan)
        // into every guarded field. Unwrap the Let to
        // the construction it binds.
        let IRNode::Let {
            name: guard_name,
            value: guard_value,
            body: record,
        } = &body
        else {
            panic!("record completion must bind the shared guard, got {:?}", body);
        };
        assert!(
            format!("{:?}", guard_value).contains("peek"),
            "the bound value must be the guard (with its peek call): {:?}",
            guard_value
        );
        let IRNode::RecordLit { type_name, fields } = record.as_ref() else {
            panic!("the Let's body must be the RecordLit, got {:?}", record);
        };
        assert_eq!(type_name, "TokenizerState");
        let names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["source", "pos", "line", "col", "tokens"]);
        // pos carries the unconditional equation (an Add);
        // line and col are guard conditionals; source and
        // tokens are carried field projections.
        assert!(matches!(fields[1].1, IRNode::BinOp(..)));
        assert!(matches!(fields[2].1, IRNode::If { .. }));
        assert!(matches!(fields[3].1, IRNode::If { .. }));
        // Phase 87: both guarded fields test the bound
        // name, not a cloned guard tree.
        for f in [&fields[2].1, &fields[3].1] {
            let IRNode::If { condition, .. } = f else {
                unreachable!()
            };
            assert!(
                matches!(condition.as_ref(), IRNode::Var(n) if n == guard_name),
                "a guarded field must test the Let's name: {:?}",
                f
            );
        }
        assert!(matches!(fields[0].1, IRNode::FieldAccess { .. }));
        assert!(matches!(fields[4].1, IRNode::FieldAccess { .. }));
        // The seed resolves the guard's peek call and fires.
        let engine =
            SynthesisEngine::with_module_spec(SynthesisConfig::default(), &module);
        let kinds = engine.constraint_seed_kinds(&spec);
        assert!(
            kinds.iter().any(|k| k == "record_completion"),
            "next_char's seed kinds must include record_completion, got {:?}",
            kinds
        );
        let seed = constraint_seed(&engine, &spec, "record_completion");
        assert!(
            !body_calls_indexed(&seed.body, &index),
            "the record-completion seed must not carry a raw peek call"
        );
    }

    /// Phase 81: the record-completion builder's decline
    /// side — no declarations in the index, and a record
    /// spec stated with ordering postconditions instead of
    /// field equations.
    #[test]
    fn phase81_record_completion_declines() {
        let module = load_tokenizer_spec();
        let spec = tokenizer_spec(&module.functions, "next_char");
        let bare = SpecIndex::from_specs(&module.functions);
        assert!(
            record_completion_body(&spec, &bare).is_none(),
            "without type declarations the builder must decline"
        );
        let index = SpecIndex::from_module(&module);
        let skip = tokenizer_spec(&module.functions, "skip_whitespace");
        assert!(
            record_completion_body(&skip, &index).is_none(),
            "skip_whitespace states orderings, not field equations: decline"
        );
        // peek returns Option<Char>, not the record: decline.
        let peek = tokenizer_spec(&module.functions, "peek");
        assert!(record_completion_body(&peek, &index).is_none());
    }

    /// Phase 92 (encoder extension, design doc §2.6):
    /// the callee_body chain's three appended entries
    /// denote the three specs that denoted nowhere —
    /// next_char through record completion, skip through
    /// its keyed denotation, tokenize_one through the
    /// keyed step denotation — and every earlier
    /// denotation is byte-identical to its builder's
    /// direct output (the chain's append discipline:
    /// new entries fire only where all predecessors
    /// decline).
    #[test]
    fn phase92_callee_body_denotes_next_char_skip_and_step() {
        let module = load_tokenizer_spec();
        let index = SpecIndex::from_module(&module);
        // next_char: the chain entry returns exactly the
        // record-completion builder's body.
        let next_char = tokenizer_spec(&module.functions, "next_char");
        let denoted = index
            .callee_body("next_char")
            .expect("next_char must denote after Phase 92");
        let direct = record_completion_body(&next_char, &index)
            .expect("record completion builds next_char");
        assert_eq!(
            format!("{:?}", denoted),
            format!("{:?}", direct),
            "next_char's denotation must be the record-completion body"
        );
        // skip_whitespace: the keyed denotation is the
        // Phase 85 hand body, parameterized by the
        // spec's own parameter name and declared types
        // — for this module, literally the hand tree.
        let v = |n: &str| IRNode::Var(n.to_string());
        let call = |f: &str, args: Vec<IRNode>| IRNode::Call {
            function: f.to_string(),
            args,
        };
        let expected_skip = IRNode::Block(vec![
            IRNode::Alloc {
                name: "cur".to_string(),
                ty: TypeRef::Named("TokenizerState".to_string()),
                initial: Box::new(v("state")),
            },
            IRNode::While {
                condition: Box::new(IRNode::Match {
                    scrutinee: Box::new(call(
                        "peek",
                        vec![IRNode::UnaryOp(
                            IRUnaryOp::Clone,
                            Box::new(v("cur")),
                        )],
                    )),
                    cases: vec![
                        (
                            IRPattern::Constructor("None".to_string(), vec![]),
                            IRNode::Literal(IRLiteral::Bool(false)),
                        ),
                        (
                            IRPattern::Constructor(
                                "Some".to_string(),
                                vec![IRPattern::Variable("ch".to_string())],
                            ),
                            call("is_whitespace", vec![v("ch")]),
                        ),
                    ],
                }),
                body: Box::new(IRNode::Block(vec![IRNode::Assign {
                    target: Box::new(v("cur")),
                    value: Box::new(call("next_char", vec![v("cur")])),
                }])),
                // Phase 96: the denotation's While
                // carries its invariant (frame +
                // progress + the ∀-between fact) — the
                // text the verifier's statement judge
                // discharges; every other consumer
                // ignores it.
                invariant: Some(
                    "cur.source == state.source; cur.pos >= state.pos; \
                     len(cur.tokens) == len(state.tokens); \
                     forall i: Int in range(len(cur.source)): \
                     i < state.pos || i >= cur.pos || is_whitespace(cur.source[i])"
                        .to_string(),
                ),
            },
            IRNode::Return(Box::new(v("cur"))),
        ]);
        let denoted_skip = index
            .callee_body("skip_whitespace")
            .expect("skip_whitespace must denote after Phase 92");
        assert_eq!(
            format!("{:?}", denoted_skip),
            format!("{:?}", expected_skip),
            "skip's denotation must be the hand While body"
        );
        // tokenize_one: the denotation IS the template
        // STEP body (the seed builder's construction,
        // reached here without the seed's rows gate).
        let one = tokenizer_spec(&module.functions, "tokenize_one");
        let denoted_one = index
            .callee_body("tokenize_one")
            .expect("tokenize_one must denote after Phase 92");
        let seed_one = tokenize_template_seed_body(&one, &index)
            .expect("the template seed builds tokenize_one");
        assert_eq!(
            format!("{:?}", denoted_one),
            format!("{:?}", seed_one),
            "tokenize_one's denotation must be the template STEP body"
        );
        // Earlier denotations unmoved: peek still denotes
        // through the complementary-guard builder (the
        // chain's previous last entry), byte-identical.
        let peek = tokenizer_spec(&module.functions, "peek");
        let denoted_peek = index
            .callee_body("peek")
            .expect("peek still denotes");
        let direct_peek = complementary_guard_body(&peek)
            .expect("complementary guard builds peek");
        assert_eq!(
            format!("{:?}", denoted_peek),
            format!("{:?}", direct_peek),
            "peek's denotation must not move"
        );
        // tokenize_all denotes nowhere: its LOOP body is
        // a candidate, not a meaning — the loop is judged
        // against the step's denotation, it does not
        // denote itself.
        assert!(
            index.callee_body("tokenize_all").is_none(),
            "tokenize_all must not denote (its body is the candidate under judgment)"
        );
    }

    /// Phase 92 teeth: the keyed builders are keyed on
    /// SHAPE, and the shape is exact — a same-named spec
    /// whose record, signature, or callee set differs
    /// gets NO denotation. Each mutation below keeps the
    /// spec's name (and, except where noted, its rows);
    /// only the structure moves.
    #[test]
    fn phase92_keyed_denotations_decline_on_shape_mutations() {
        let module = load_tokenizer_spec();
        // (a) A load-bearing field changes type (pos:
        // Int → String): the fingerprint is gone, so
        // both keyed builders decline.
        let mut m = module.clone();
        for td in &mut m.types {
            if td.name == "TokenizerState" {
                let crate::spec::ast::TypeKind::Record { fields } = &mut td.kind
                else {
                    panic!("TokenizerState is a record");
                };
                fields[1].1 = TypeRef::String;
            }
        }
        let index = SpecIndex::from_module(&m);
        assert!(
            index.callee_body("skip_whitespace").is_none(),
            "a re-typed state field must decline the skip denotation"
        );
        assert!(
            index.callee_body("tokenize_one").is_none(),
            "a re-typed state field must decline the step denotation"
        );
        // (a2) An EXTRA field, by contrast, does not
        // decline: the keying is the five-field
        // fingerprint (the template's own discipline —
        // its state_record iterates the declared fields
        // and carries the rest), and both bodies thread
        // the record whole, so an extra field is carried
        // opaquely and the meaning is unchanged. Pinned
        // as the designed behavior, not an oversight.
        let mut m = module.clone();
        for td in &mut m.types {
            if td.name == "TokenizerState" {
                let crate::spec::ast::TypeKind::Record { fields } = &mut td.kind
                else {
                    panic!("TokenizerState is a record");
                };
                fields.push(("extra".to_string(), TypeRef::Int));
            }
        }
        let index = SpecIndex::from_module(&m);
        assert!(
            index.callee_body("skip_whitespace").is_some(),
            "an extra field is carried: the fingerprint keying still denotes"
        );
        assert!(
            index.callee_body("tokenize_one").is_some(),
            "an extra field is carried: the fingerprint keying still denotes"
        );
        // (b) A state field renamed: same decline.
        let mut m = module.clone();
        for td in &mut m.types {
            if td.name == "TokenizerState" {
                let crate::spec::ast::TypeKind::Record { fields } = &mut td.kind
                else {
                    panic!("TokenizerState is a record");
                };
                fields[0].0 = "src".to_string();
            }
        }
        let index = SpecIndex::from_module(&m);
        assert!(index.callee_body("skip_whitespace").is_none());
        assert!(index.callee_body("tokenize_one").is_none());
        // (c) skip_whitespace with a different return
        // type: the state→state signature is gone.
        let mut m = module.clone();
        for f in &mut m.functions {
            if f.name == "skip_whitespace" {
                f.return_type = TypeRef::Bool;
            }
        }
        let index = SpecIndex::from_module(&m);
        assert!(
            index.callee_body("skip_whitespace").is_none(),
            "a Bool-returning skip_whitespace must not denote as the skip"
        );
        // (d) The module without is_whitespace: the
        // skip body calls it, so the skip denotation
        // declines — while the step denotation, whose
        // body never calls is_whitespace, stands. The
        // dependency sets are the bodies' own call sets.
        let mut m = module.clone();
        m.functions.retain(|f| f.name != "is_whitespace");
        let index = SpecIndex::from_module(&m);
        assert!(
            index.callee_body("skip_whitespace").is_none(),
            "no is_whitespace callee: no skip denotation"
        );
        assert!(
            index.callee_body("tokenize_one").is_some(),
            "the step body does not call is_whitespace: its denotation stands"
        );
        // (e) The module without peek: both bodies call
        // peek (the skip's loop condition, the step's
        // callee keying), so both decline.
        let mut m = module.clone();
        m.functions.retain(|f| f.name != "peek");
        let index = SpecIndex::from_module(&m);
        assert!(index.callee_body("skip_whitespace").is_none());
        assert!(
            index.callee_body("tokenize_one").is_none(),
            "no peek callee: no step denotation"
        );
    }

    /// Admitted synthetic shapes: the range conjunction
    /// (Phase 45's is_digit shape), literal-on-the-left
    /// leaves, != and ! connectives, and a call composition
    /// whose callee is itself a classifier. Phase 49 pinned
    /// these as char_equation admissions; since Phase 76
    /// (D5a) every one of them is encodable, so the
    /// equation family admits them (same bodies, same
    /// executed behavior) and char_equation admits nothing.
    #[test]
    /// Phase 85 (M1 remainder (iii)): the tokenize
    /// template family fires on both tokenize specs —
    /// the STEP form for tokenize_one, the LOOP form
    /// for tokenize_all — and on nothing else in the
    /// module.
    #[test]
    fn phase85_tokenize_template_fires_on_the_tokenize_pair() {
        let module = load_tokenizer_spec();
        let engine =
            SynthesisEngine::with_module_spec(SynthesisConfig::default(), &module);
        for name in ["tokenize_one", "tokenize_all"] {
            let spec = tokenizer_spec(&module.functions, name);
            let kinds = engine.constraint_seed_kinds(&spec);
            assert!(
                kinds.iter().any(|k| k == "tokenize_template"),
                "{}'s seed kinds must include tokenize_template, got {:?}",
                name,
                kinds
            );
        }
        for name in [
            "is_whitespace",
            "is_ident_start",
            "is_ident_continue",
            "skip_whitespace",
            "peek",
            "next_char",
        ] {
            let spec = tokenizer_spec(&module.functions, name);
            let kinds = engine.constraint_seed_kinds(&spec);
            assert!(
                !kinds.iter().any(|k| k == "tokenize_template"),
                "{} must not gain a tokenize_template seed, got {:?}",
                name,
                kinds
            );
        }
        // The STEP body is the mirror's shape: it opens
        // with the skip allocation and returns the peek
        // dispatch; the LOOP body loops over the step.
        let one = tokenizer_spec(&module.functions, "tokenize_one");
        let seed = constraint_seed(&engine, &one, "tokenize_template");
        let text = format!("{:?}", seed.body);
        assert!(
            text.contains("skip_whitespace") && text.contains("is_ident_start"),
            "the step body must call the skip and the ident classifier: {}",
            text
        );
        let all = tokenizer_spec(&module.functions, "tokenize_all");
        let seed = constraint_seed(&engine, &all, "tokenize_template");
        let text = format!("{:?}", seed.body);
        assert!(
            text.contains("tokenize_one") && text.contains("TryPropagate"),
            "the loop body must thread the step's Ok payload: {}",
            text
        );
    }

    /// Phase 85: the template's keying declines — a
    /// state-shaped spec without the rows' shape, and
    /// a rows-shaped spec without the module's step
    /// callees, both get no seed.
    #[test]
    fn phase85_tokenize_template_declines() {
        // (a) The full tokenizer module, but tokenize_one
        // renamed into a spec whose rows lose the
        // is_err/is_ok + unwrap shape: rebuild the module
        // with tokenize_one's postconditions replaced by
        // a single unrelated row.
        let module = load_tokenizer_spec();
        let engine =
            SynthesisEngine::with_module_spec(SynthesisConfig::default(), &module);
        let mut stripped = tokenizer_spec(&module.functions, "tokenize_one");
        stripped.postconditions = vec![Constraint::Expr(Expr::BoolLit(true))];
        let kinds = engine.constraint_seed_kinds(&stripped);
        assert!(
            !kinds.iter().any(|k| k == "tokenize_template"),
            "rows-shape decline failed, got {:?}",
            kinds
        );
        // (b) tokenize_one alone in an index with no
        // callees: the signature evidence is absent.
        let lone = tokenizer_spec(&module.functions, "tokenize_one");
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let kinds = engine.constraint_seed_kinds(&lone);
        assert!(
            !kinds.iter().any(|k| k == "tokenize_template"),
            "callee decline failed, got {:?}",
            kinds
        );
    }

    fn classifier_shapes_admitted_by_equation_family() {
        // (a) Range conjunction over the param.
        let src = r#"
        T {
        spec is_digit {
            input: c: Char
            output: Bool
            constraint: output == (c >= '0' && c <= '9')
        }
        }
        "#;
        let module = crate::spec::parser::parse(src).unwrap().functions;
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seed = constraint_seed(&engine, &module[0], "equation");
        assert!((0..=0x2FFu32)
            .filter_map(char::from_u32)
            .all(|c| eval_char_body(&seed.body, c) == c.is_ascii_digit()));

        // (b) Literal-on-the-left leaves, a != leaf, and a
        // negated comparison under the connectives.
        let src = r#"
        T {
        spec not_x_lower {
            input: c: Char
            output: Bool
            constraint: output == (('a' <= c && c <= 'z') && c != 'x' && !('m' == c))
        }
        }
        "#;
        let module = crate::spec::parser::parse(src).unwrap().functions;
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seed = constraint_seed(&engine, &module[0], "equation");
        assert!((0..=0x2FFu32).filter_map(char::from_u32).all(|c| {
            eval_char_body(&seed.body, c)
                == (('a'..='z').contains(&c) && c != 'x' && c != 'm')
        }));

        // (c) Call composition: the caller's seed is the
        // callee's tree inlined under its own leaf.
        let src = r#"
        T {
        spec is_vowel {
            input: c: Char
            output: Bool
            constraint: output == (c == 'a' || c == 'e' || c == 'i' || c == 'o' || c == 'u')
        }
        spec is_vowel_or_digit {
            input: c: Char
            output: Bool
            constraint: output == (is_vowel(c) || (c >= '0' && c <= '9'))
        }
        }
        "#;
        let module = crate::spec::parser::parse(src).unwrap().functions;
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let caller = module.iter().find(|f| f.name == "is_vowel_or_digit").unwrap();
        let seed = constraint_seed(&engine, caller, "equation");
        assert!(
            !format!("{:?}", seed.body).contains("Call"),
            "caller seed must be fully inlined: {:?}",
            seed.body
        );
        assert!((0..=0x2FFu32).filter_map(char::from_u32).all(|c| {
            eval_char_body(&seed.body, c)
                == ("aeiou".contains(c) || c.is_ascii_digit())
        }));
    }

    /// Refused shapes: each must yield NO char_equation
    /// seed (the comments name the refusing predicate).
    #[test]
    fn char_equation_refused_shapes() {
        let cases: Vec<(&str, &str)> = vec![
            // Arity: a second parameter exists, even though
            // the equation mentions only the Char param.
            (
                "two params",
                r#"
                T {
                spec f {
                    input: c: Char, n: Int
                    output: Bool
                    constraint: output == (c == ' ')
                }
                }
                "#,
            ),
            // Arity + leaf discipline: the equation mentions
            // a second (Char) parameter — param-vs-param is
            // not a classifier leaf.
            (
                "second param in equation",
                r#"
                T {
                spec f {
                    input: c: Char, d: Char
                    output: Bool
                    constraint: output == (c == d)
                }
                }
                "#,
            ),
            // Leaf discipline on a single param: param vs
            // param is not a comparison against a literal.
            (
                "param vs param",
                r#"
                T {
                spec f {
                    input: c: Char
                    output: Bool
                    constraint: output == (c == c)
                }
                }
                "#,
            ),
            // Leaf discipline: a constant comparison mentions
            // no parameter — there is no classifier leaf.
            (
                "constant comparison",
                r#"
                T {
                spec f {
                    input: c: Char
                    output: Bool
                    constraint: output == ('a' == 'a')
                }
                }
                "#,
            ),
            // Non-comparison leaf: arithmetic inside the
            // equation is outside the classifier tree.
            (
                "arithmetic leaf",
                r#"
                T {
                spec f {
                    input: c: Char
                    output: Bool
                    constraint: output == (c >= 'a' && c + 'a' == 'b')
                }
                }
                "#,
            ),
            // Return-type filter: the output is not Bool.
            (
                "non-Bool output",
                r#"
                T {
                spec f {
                    input: c: Char
                    output: Int
                    constraint: output == 1
                }
                }
                "#,
            ),
            // Signature filter: a String parameter's
            // equality equation is the recorded unadmitted
            // String territory (rust_ident_empty's family),
            // not this one — and the equation family states
            // it encodably besides.
            (
                "String-keyed lookalike",
                r#"
                T {
                spec f {
                    input: s: String
                    output: Bool
                    constraint: output == (s == "x" || s == "y")
                }
                }
                "#,
            ),
            // Signature filter: a Bool parameter's equation
            // is encodable territory the equation family
            // owns; the discriminator is Char.
            (
                "Bool-param lookalike",
                r#"
                T {
                spec f {
                    input: b: Bool
                    output: Bool
                    constraint: output == (b == true)
                }
                }
                "#,
            ),
            // Signature filter: an Int range equation is
            // the equation family's home shape.
            (
                "Int range lookalike",
                r#"
                T {
                spec f {
                    input: n: Int
                    output: Bool
                    constraint: output == (n >= 0 && n <= 8)
                }
                }
                "#,
            ),
        ];
        for (label, src) in cases {
            let module = crate::spec::parser::parse(src).unwrap().functions;
            let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
            let seeds = engine.seed_from_constraints(&module[0]);
            assert!(
                !seeds.iter().any(|c| matches!(
                    &c.provenance,
                    Provenance::ConstraintInduced { kind } if kind == "char_equation"
                )),
                "{}: must not gain a char_equation seed (got {:?})",
                label,
                seeds.iter().map(|c| format!("{:?}", c.provenance)).collect::<Vec<_>>()
            );
        }

        // Precedence (the Phase 34 right of first refusal):
        // a spec the equation family CAN state encodably —
        // here via a second, constant equation — belongs to
        // the equation family even though its first
        // postcondition is classifier-shaped. The
        // char-equation builder must refuse it wholesale.
        let src = r#"
        T {
        spec f {
            input: c: Char
            output: Bool
            constraint: output == (c == ' ')
            constraint: output == true
        }
        }
        "#;
        let module = crate::spec::parser::parse(src).unwrap().functions;
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&module[0]);
        assert_eq!(seeds.len(), 1, "precedence case: exactly the equation seed");
        let _ = constraint_seed(&engine, &module[0], "equation");
    }

    // ── Phase 60: forall-fold seeds (campaign slice G3) ───
    //
    // The tenth constraint-seed family. count_matching is
    // the one standalone spec whose construction is
    // derivable from its constraints (Phase 57 answer 3):
    // a Forall over the list parameter whose implication
    // antecedent is a comparison of the quantified element
    // against a scalar parameter — the counting predicate.
    // The family mines that predicate and builds the scan
    // (Alloc accumulator + For + Index + conditional
    // increment, the Phase 59-proven compilable form — NOT
    // the Fold node, which renders E0308). The seed is
    // admitted knowing Z3 cannot judge it (Inconclusive by
    // construction, the char_equation admission standard):
    // its evidence is the compiled vehicle, reached through
    // the runner's Phase 60 trigger extension.

    /// The expected scan body for a count-shaped spec, in
    /// the family's construction: Alloc the accumulator,
    /// walk `0..len(list)`, add `if <pred> { 1 } else { 0 }`
    /// per element (the predicate with the element variable
    /// replaced by the indexed access, orientation as
    /// written), return the accumulator.
    fn forall_fold_expected_body(
        list: &str,
        scalar: &str,
        op: IRBinOp,
        elem_on_left: bool,
    ) -> IRNode {
        let var = |n: &str| IRNode::Var(n.to_string());
        let int = |n: i64| IRNode::Literal(IRLiteral::Int(n));
        let elem = IRNode::Index(Box::new(var(list)), Box::new(var("i")));
        let (l, r) = if elem_on_left {
            (elem, var(scalar))
        } else {
            (var(scalar), elem)
        };
        IRNode::Block(vec![
            IRNode::Alloc {
                name: "count".into(),
                ty: TypeRef::Int,
                initial: Box::new(int(0)),
            },
            IRNode::For {
                var: "i".into(),
                start: Box::new(int(0)),
                end: Box::new(IRNode::UnaryOp(IRUnaryOp::Len, Box::new(var(list)))),
                body: Box::new(IRNode::Assign {
                    target: Box::new(var("count")),
                    value: Box::new(IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(var("count")),
                        Box::new(IRNode::If {
                            condition: Box::new(IRNode::BinOp(op, Box::new(l), Box::new(r))),
                            then_branch: Box::new(int(1)),
                            else_branch: Box::new(int(0)),
                        }),
                    )),
                }),
            },
            IRNode::Return(Box::new(var("count"))),
        ])
    }

    #[test]
    fn forall_fold_seed_for_count_matching() {
        let spec = load_example_spec(
            "examples/self_host/count_matching.morph",
            "count_matching",
        );
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the forall_fold seed");
        let seed = constraint_seed(&engine, &spec, "forall_fold");
        assert_eq!(count_holes(&seed.body), 0, "the scan body must be complete");
        assert_eq!(
            format!("{:?}", seed.body),
            format!(
                "{:?}",
                forall_fold_expected_body("list", "threshold", IRBinOp::Gt, true)
            ),
            "the seed body must be the scan built from the mined predicate `x > threshold`"
        );
        // Judging reality, recorded: the interpreter binds
        // the tests (Phase 53) but cannot evaluate a
        // For/Alloc/Assign body, so the seed's interpreter
        // run is evaluable-but-zero — a coverage fact the
        // runner's Phase 60 trigger extension exists to
        // bridge. Z3 is Inconclusive by construction (loop
        // bodies are outside its encodable subset).
        let run = crate::synthesis::eval::run_spec_tests(&seed.body, &spec);
        assert_eq!(
            run,
            crate::synthesis::eval::TestRun { evaluable: 3, passed: 0 },
            "the interpreter must bind all 3 tests and evaluate none of the scan body"
        );
        let result = crate::verify::verifier::Verifier::new().verify_one(&seed, &spec);
        assert_eq!(
            result.status,
            VerificationStatus::Inconclusive,
            "Z3 cannot judge a loop body — Inconclusive by construction"
        );
    }

    #[test]
    fn forall_fold_seed_is_structural_not_nominal() {
        // The same shape under unrelated names fires, with
        // the spec's own parameter names in the body.
        let src = r#"
        T {
        spec count_above {
            input: values: List<Int>, limit: Int
            output: Int
            constraint: forall x: Int in values:
                x > limit implies output > 0
            constraint: output >= 0
            constraint: output <= len(values)
            test "all" { input: ([10, 20, 30], 5); expect: 3 }
            test "none" { input: ([1, 2, 3], 5); expect: 0 }
            test "mixed" { input: ([1, 10, 2, 20], 5); expect: 2 }
        }
        }
        "#;
        let module = crate::spec::parser::parse(src).unwrap().functions;
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seed = constraint_seed(&engine, &module[0], "forall_fold");
        assert_eq!(
            format!("{:?}", seed.body),
            format!(
                "{:?}",
                forall_fold_expected_body("values", "limit", IRBinOp::Gt, true)
            ),
        );
        // The flagship name over a scalar signature does not
        // fire: keying is the shape, never the name.
        let src = r#"
        T {
        spec count_matching {
            input: x: Int
            output: Int
            constraint: output >= 0
            test "one" { input: 1; expect: 1 }
        }
        }
        "#;
        let module = crate::spec::parser::parse(src).unwrap().functions;
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        assert!(
            !engine
                .seed_from_constraints(&module[0])
                .iter()
                .any(|c| matches!(&c.provenance, Provenance::ConstraintInduced { kind } if kind == "forall_fold")),
            "a scalar spec named count_matching must not gain a forall_fold seed"
        );
    }

    #[test]
    fn forall_fold_seed_mirrored_predicate_keeps_orientation() {
        // The scalar on the left, the element on the right:
        // the construction preserves the written comparison
        // (it does not normalize the operator).
        let src = r#"
        T {
        spec count_at_least {
            input: values: List<Int>, limit: Int
            output: Int
            constraint: forall x: Int in values:
                limit >= x implies output > 0
            test "some" { input: ([1, 10, 2], 5); expect: 2 }
        }
        }
        "#;
        let module = crate::spec::parser::parse(src).unwrap().functions;
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seed = constraint_seed(&engine, &module[0], "forall_fold");
        assert_eq!(
            format!("{:?}", seed.body),
            format!(
                "{:?}",
                forall_fold_expected_body("values", "limit", IRBinOp::Ge, false)
            ),
        );
    }

    #[test]
    fn forall_fold_near_misses_get_no_seed() {
        let head = "input: values: List<Int>, limit: Int\n            output: Int\n";
        let tests = "            test \"a\" { input: ([10, 20], 5); expect: 2 }\n";
        let cases: Vec<(&str, String)> = vec![
            (
                "literal threshold (not a scalar parameter)",
                format!(
                    "{}{}            constraint: forall x: Int in values:\n                x > 5 implies output > 0\n{}",
                    head, "", tests
                ),
            ),
            (
                "compound predicate (a conjunction, not one comparison)",
                format!(
                    "{}            constraint: forall x: Int in values:\n                x > limit && x < 100 implies output > 0\n{}",
                    head, tests
                ),
            ),
            (
                "consequent silent about the output",
                format!(
                    "{}            constraint: forall x: Int in values:\n                x > limit implies limit > 0\n{}",
                    head, tests
                ),
            ),
            (
                "element on both sides of the comparison",
                format!(
                    "{}            constraint: forall x: Int in values:\n                x > x implies output > 0\n{}",
                    head, tests
                ),
            ),
            (
                "forall without the implication",
                format!(
                    "{}            constraint: forall x: Int in values:\n                x > limit\n{}",
                    head, tests
                ),
            ),
            (
                "exists instead of forall",
                format!(
                    "{}            constraint: exists x: Int in values:\n                x > limit implies output > 0\n{}",
                    head, tests
                ),
            ),
            (
                "a deviating first forall blocks a matching second",
                format!(
                    "{}            constraint: forall x: Int in values:\n                x > 5 implies output > 0\n            constraint: forall y: Int in values:\n                y > limit implies output > 0\n{}",
                    head, tests
                ),
            ),
            (
                "scalar parameter of a different type than the element",
                "input: values: List<Int>, limit: Float\n            output: Int\n            constraint: forall x: Int in values:\n                x > limit implies output > 0\n            test \"a\" { input: ([10, 20], 5); expect: 2 }\n".to_string(),
            ),
            (
                "two list parameters (the iterated list is ambiguous)",
                "input: values: List<Int>, others: List<Int>, limit: Int\n            output: Int\n            constraint: forall x: Int in values:\n                x > limit implies output > 0\n            test \"a\" { input: ([10, 20], [1], 5); expect: 2 }\n".to_string(),
            ),
            (
                "no in-spec tests (nothing for the compiled vehicle to judge)",
                format!(
                    "{}            constraint: forall x: Int in values:\n                x > limit implies output > 0\n",
                    head
                ),
            ),
            (
                "a test the interpreter cannot bind (a variable input)",
                format!(
                    "{}            constraint: forall x: Int in values:\n                x > limit implies output > 0\n            test \"a\" {{ input: ([10, 20], limit); expect: 2 }}\n",
                    head
                ),
            ),
        ];
        for (label, body) in cases {
            let src = format!("\n        T {{\n        spec near_miss {{\n            {}\n        }}\n        }}\n        ", body);
            let module = crate::spec::parser::parse(&src).unwrap().functions;
            let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
            let seeds = engine.seed_from_constraints(&module[0]);
            assert!(
                !seeds.iter().any(|c| matches!(
                    &c.provenance,
                    Provenance::ConstraintInduced { kind } if kind == "forall_fold"
                )),
                "{}: must not gain a forall_fold seed (got {:?})",
                label,
                seeds.iter().map(|c| format!("{:?}", c.provenance)).collect::<Vec<_>>()
            );
        }
        // Generic element: the comparison cannot render
        // under the bound-free generated signature (the
        // templates' recorded discipline).
        let src = r#"
        T {
        spec count_generic<T> {
            input: values: List<T>, limit: T
            output: Int
            constraint: forall x: T in values:
                x > limit implies output > 0
            test "a" { input: ([10, 20], 5); expect: 2 }
        }
        }
        "#;
        let module = crate::spec::parser::parse(src).unwrap().functions;
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        assert!(
            !engine
                .seed_from_constraints(&module[0])
                .iter()
                .any(|c| matches!(&c.provenance, Provenance::ConstraintInduced { kind } if kind == "forall_fold")),
            "generic element: must not gain a forall_fold seed"
        );
    }

    #[test]
    fn forall_fold_yields_to_earlier_families() {
        // Precedence (the family's territory rule, enforced
        // in seed_from_constraints): a spec an earlier
        // family can state — here an encodable equation —
        // belongs to that family even though it also
        // carries the forall-fold shape.
        let src = r#"
        T {
        spec count_or_const {
            input: values: List<Int>, limit: Int
            output: Int
            constraint: output == limit
            constraint: forall x: Int in values:
                x > limit implies output > 0
            test "a" { input: ([10, 20], 5); expect: 2 }
        }
        }
        "#;
        let module = crate::spec::parser::parse(src).unwrap().functions;
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&module[0]);
        assert_eq!(seeds.len(), 1, "exactly the earlier family's seed");
        let _ = constraint_seed(&engine, &module[0], "equation");
    }

    #[test]
    fn forall_fold_fires_nowhere_else_in_the_corpus() {
        // Territory disjointness over every module the
        // instruments measure: no M2/M3/tokenizer spec and
        // none of the other standalone specs may gain the
        // seed. (The full 142-spec inventory, standalone
        // files included, is the Phase 60 instrument's
        // Part A.)
        let mut modules: Vec<Vec<FunctionSpec>> = ["validator", "printer"]
            .iter()
            .map(|f| load_m2_module(f))
            .collect();
        modules.push(load_codegen_module());
        modules.push(load_tokenizer_module());
        for module in &modules {
            let engine = SynthesisEngine::with_module(SynthesisConfig::default(), module);
            for spec in module {
                assert!(
                    !engine
                        .seed_from_constraints(spec)
                        .iter()
                        .any(|c| matches!(&c.provenance, Provenance::ConstraintInduced { kind } if kind == "forall_fold")),
                    "{}: must not gain a forall_fold seed",
                    spec.name
                );
            }
        }
        for (rel, name) in [
            ("examples/sort.morph", "sort"),
            ("examples/binary_search.morph", "binary_search"),
            ("examples/self_host/is_sorted.morph", "is_sorted"),
        ] {
            let spec = load_example_spec(rel, name);
            let engine = SynthesisEngine::new(SynthesisConfig::default());
            assert!(
                !engine
                    .seed_from_constraints(&spec)
                    .iter()
                    .any(|c| matches!(&c.provenance, Provenance::ConstraintInduced { kind } if kind == "forall_fold")),
                "{}: must not gain a forall_fold seed",
                name
            );
        }
    }

    /// Guard generalizations (regression): the generalizations
    /// must not move any body the builders already produced.
    /// The golden fixture (tests/seed_body_golden.txt) was
    /// captured from the builders BEFORE the generalizations —
    /// the resolver-visible body (SpecIndex::callee_body) of
    /// every previously recognized spec: the 26 seeded Codegen
    /// specs and all 14 M2 specs. Byte-identical means the
    /// Debug rendering matches exactly. Phase 28 extended the
    /// fixture with the two composition-guard bodies
    /// (rust_ident_suffix, rust_generics_text): 28 Codegen
    /// bodies now. Phase 32 extends it with the three
    /// expression-pin guard-table bodies (rust_unary_text,
    /// rust_concat_right_text, rust_match_scrutinee_text):
    /// 31 Codegen bodies, 45 total — the 42 prior bodies are
    /// asserted byte-identical below, unchanged. Phase 33
    /// extends it with the three pair-keyed guard-table
    /// bodies (rust_concat_left_owned, rust_concat_right_form,
    /// rust_match_scrutinee_bare): 34 Codegen bodies, 48
    /// total — the 45 prior bodies are asserted
    /// byte-identical below, unchanged. The same phase's
    /// builder also claims rust_string_ownership — its rows
    /// are a pair table as written (see the Phase 33 builder
    /// tests) — so its body joins the fixture too: 35
    /// Codegen bodies, 49 total, the 48 prior bodies
    /// asserted byte-identical below, unchanged. Phase 34
    /// extends it with the multi-guard truth-table body
    /// (rust_match_scrutinee_form) and the composition
    /// precedence body (rust_concat_left_text): 37 Codegen
    /// bodies, 51 total — the 49 prior bodies are asserted
    /// byte-identical below, unchanged, which is also the
    /// proof that the multi-guard generalization reduces to
    /// nothing at one guard (it requires two) and that the
    /// precedence amendment moved no earlier family's body.
    /// Phase 37 extends it with the three guard-table bodies
    /// the range-row recovery CORRECTED (rust_stmt_form,
    /// rust_expectation_source, rust_stmt_child_indent):
    /// their pre-fix bodies were the silently-wrong ones of
    /// Phase 36's Finding 1 — refuted in every run — so they
    /// were never fixture material before; the corrected
    /// bodies join now: 40 Codegen bodies, 54 total. The 51
    /// prior bodies are asserted byte-identical below,
    /// unchanged — the recovery materializes arms only for
    /// cells the old builder dropped, and the survey diff
    /// over every spec's resolver-visible body shows exactly
    /// the three corrections and nothing else. Phase 38
    /// extends it with the mixed pair-keyed guard-table
    /// body (rust_stmt_seq_form): 41 Codegen bodies, 55
    /// total — the 54 prior bodies are asserted
    /// byte-identical below, unchanged, which is also the
    /// proof that the eighth family claims only the
    /// (Int, Bool) key signature and moves no earlier
    /// family's body. Phase 49 extends it with the three
    /// Char classifiers of the tokenizer module
    /// (is_whitespace, is_ident_start, is_ident_continue):
    /// 55 prior bodies plus the tokenizer's 3, 58
    /// total — the 55 prior bodies are asserted
    /// byte-identical below, unchanged.
    /// The classifiers were always resolver-visible (their
    /// first equation body is what a call denotes — for
    /// is_ident_continue, the raw body with its Call, like
    /// validate_kind_tag's); what Phase 49 adds is their
    /// SEED (the char_equation family admits the inlined
    /// body the gate kept Z3 from judging), and the
    /// resolver-visible diff over all 138 specs shows the
    /// new family moved no resolver body anywhere.
    /// Phase 55 REPLACES three bodies in place and appends
    /// four: the spec amendments changed the guard-table
    /// bodies of spec_binop_symbol (six rows now carry
    /// their real symbols), rust_expr_form (Lambda leaves
    /// the fallback family for the closure family) and
    /// rust_expr_fixed_text (the Lambda row is gone) —
    /// amended specs' bodies legitimately change, and the
    /// replacement lines are exactly the amended specs'
    /// resolver-visible bodies. The four appends are the
    /// phase's three new equation templates
    /// (rust_expr_lambda_text, rust_expr_list_text,
    /// rust_expr_tuple_text) and rust_file_text, whose
    /// equation the placement amendment reordered — a body
    /// corrected by a phase joins the fixture, as in
    /// Phase 37: 45 Codegen bodies, 62 total. The other 55
    /// bodies are asserted byte-identical below, unchanged.
    /// (rust_expr_call_form, the phase's fourth new spec,
    /// has no seed body — String-equality guards are the
    /// recorded unadmitted family, like rust_target_route
    /// and rust_ident_empty, which are likewise absent.)
    ///
    /// Phase 62 extension (2026-11-21): ONE append —
    /// rust_generic_param_text, the Clone-bound amendment's
    /// new per-generic template spec, whose equation seed
    /// body joins the fixture by the Phase 55 append
    /// precedent for new equation specs: 46 Codegen bodies,
    /// 63 total. The other 62 bodies are asserted
    /// byte-identical below, unchanged (rust_generics_text's
    /// own body did not change — only its doc and its
    /// driver's join elements did).
    ///
    /// Phase 78 extension (2026-11-22): the M1-features
    /// kind-row amendments. Two bodies are REPLACED in
    /// place — rust_expr_form (Field leaves the fallback
    /// family for family 8; MethodCall and RecordLit join
    /// as families 9 and 10) and rust_expr_fixed_text (the
    /// Field row is gone, the MethodCall fallback row is
    /// in) — the Phase 55 replacement precedent for amended
    /// specs. Six bodies are APPENDED: the phase's five new
    /// equation templates (rust_expr_field_text,
    /// rust_expr_method_text, rust_expr_record_text,
    /// rust_field_access_text, rust_record_lit_text) and
    /// rust_tail_form, whose guard-table body this phase
    /// amended (the FieldAccess/RecordLit value rows) and
    /// which joins by the Phase 37 precedent for bodies a
    /// phase corrects: 52 Codegen bodies, 69 total. The
    /// other 61 bodies are asserted byte-identical below,
    /// unchanged. (rust_expr_method_form, the phase's sixth
    /// new spec, has no seed body — String-equality guards
    /// are the recorded unadmitted family, like
    /// rust_expr_call_form, rust_target_route and
    /// rust_ident_empty, which are likewise absent.
    /// rust_stmt_form's constraints did not move — its
    /// amendment is doc and tests only — so it has no
    /// fixture consequence.)
    #[test]
    fn constraint_seed_bodies_byte_identical_for_recognized_specs() {
        let root = env!("CARGO_MANIFEST_DIR");
        let golden = std::fs::read_to_string(format!("{}/tests/seed_body_golden.txt", root))
            .expect("golden fixture must exist");
        let mut expected: std::collections::HashMap<(String, String), String> =
            std::collections::HashMap::new();
        for line in golden.lines() {
            let (key, body) = line.split_once('\t').expect("golden line shape");
            let (file, name) = key.split_once("::").expect("golden key shape");
            expected.insert((file.to_string(), name.to_string()), body.to_string());
        }
        assert_eq!(expected.len(), 69, "52 Codegen + 14 M2 + 3 tokenizer golden bodies");
        let mut modules: Vec<(String, Vec<FunctionSpec>)> = ["validator", "printer", "codegen"]
            .iter()
            .map(|f| (f.to_string(), load_m2_module(f)))
            .collect();
        modules.push(("tokenizer".to_string(), load_tokenizer_module()));
        for (file, module) in &modules {
            let index = SpecIndex::from_specs(module);
            for func in module {
                let Some(want) = expected.get(&(file.clone(), func.name.clone())) else {
                    continue;
                };
                let body = index
                    .callee_body(&func.name)
                    .unwrap_or_else(|| panic!("{}::{}: body no longer recognized", file, func.name));
                assert_eq!(
                    format!("{:?}", body),
                    *want,
                    "{}::{}: seed body changed",
                    file,
                    func.name
                );
            }
        }
    }

    // ── Phase 28: composition-guard seeds ─────────────────
    //
    // The two seedless shapes of the Phase 27 measurement
    // that a builder can close: rows guarded by a Bool-valued
    // expression that is not a lone parameter — a CALL to
    // another spec (rust_ident_suffix, guarded by
    // rust_keyword(name)) — or by a plain Bool parameter,
    // whose output pins are EXPRESSIONS over the parameters
    // (rust_generics_text: a literal on one row, a Concat on
    // the other). Generation at opt 1 never proposes a Call,
    // so without a builder these pools hold zero Verified
    // candidates and selection crowns Inconclusive lotteries.
    // The verifier already encodes both shapes (the Phase 27
    // refutations flow through the inlined keyword set); only
    // the guard-row machinery refused them: guard leaves had
    // to compare a parameter VARIABLE against a literal, and
    // output pins had to be literals or one bare Var.

    /// Composition-guard seeds (i): rust_ident_suffix's rows
    /// are guarded by `rust_keyword(name)` — a call to an
    /// indexed Bool spec — and pin the output to `name ++ "_"`
    /// (keyword) and `name` (anything else). The seed must
    /// exist (kind `composition_guard`), its resolver-visible
    /// body must be the match over the call itself, the
    /// admitted body must be that match with the call RESOLVED
    /// (the interpreter cannot execute spec calls, so a raw
    /// call body could not be measured — the equation seed's
    /// Phase 18 precedent), it must be Verified in module
    /// context, execute to full pinned agreement (every one of
    /// the 51 keywords suffixed, non-keywords identical), and
    /// win module-aware synthesis deterministically.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_composition_guard_seed_for_ident_suffix() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_ident_suffix");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the composition-guard seed");
        let seed = constraint_seed(&engine, &spec, "composition_guard");
        assert_eq!(count_holes(&seed.body), 0);

        let arms = vec![
            (
                IRPattern::Literal(IRLiteral::Bool(true)),
                IRNode::BinOp(
                    IRBinOp::Concat,
                    Box::new(IRNode::Var("name".into())),
                    Box::new(IRNode::Literal(IRLiteral::String("_".into()))),
                ),
            ),
            (
                IRPattern::Literal(IRLiteral::Bool(false)),
                IRNode::Var("name".into()),
            ),
            (
                IRPattern::Wildcard,
                IRNode::Literal(IRLiteral::String(String::new())),
            ),
        ];
        // The resolver-visible body: a match over the call,
        // evaluated once, one arm per pinned call value.
        let index = SpecIndex::from_specs(&module);
        let raw = index
            .callee_body("rust_ident_suffix")
            .expect("the composition-guard body must be resolver-visible");
        let expected_raw = IRNode::Match {
            scrutinee: Box::new(IRNode::Call {
                function: "rust_keyword".into(),
                args: vec![IRNode::Var("name".into())],
            }),
            cases: arms.clone(),
        };
        assert_eq!(format!("{:?}", raw), format!("{:?}", expected_raw));

        // The admitted seed: same arms, scrutinee resolved to
        // the callee's own body (the keyword disjunction over
        // `name`), so the interpreter can execute it.
        let IRNode::Match { scrutinee, cases } = &seed.body else {
            panic!("seed body must be a match: {:?}", seed.body);
        };
        assert!(
            !matches!(**scrutinee, IRNode::Call { .. }),
            "seed scrutinee must be resolved, not a call: {:?}",
            scrutinee
        );
        assert!(
            format!("{:?}", scrutinee)
                .contains("BinOp(Eq, Var(\"name\"), Literal(String(\"type\")))"),
            "seed scrutinee must be the inlined keyword predicate: {:?}",
            scrutinee
        );
        assert_eq!(format!("{:?}", cases), format!("{:?}", arms));

        // Executed over the pinned domain: all 51 keywords
        // take the suffix; non-keyword names pass through.
        const KEYWORDS: [&str; 51] = [
            "as", "break", "const", "continue", "crate", "else", "enum",
            "extern", "false", "fn", "for", "if", "impl", "in", "let",
            "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
            "self", "Self", "static", "struct", "super", "trait", "true",
            "type", "unsafe", "use", "where", "while", "async", "await",
            "dyn", "abstract", "become", "box", "do", "final", "macro",
            "override", "priv", "typeof", "unsized", "virtual", "yield",
            "try",
        ];
        for kw in KEYWORDS {
            let v = eval_seed_body(
                &seed.body,
                &[("name", crate::synthesis::eval::Value::Str(kw.into()))],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Str(format!("{}_", kw)),
                "keyword {:?} must be suffixed",
                kw
            );
        }
        for plain in ["sort", "type_", "_", "hello", "gen", "union", "main"] {
            let v = eval_seed_body(
                &seed.body,
                &[("name", crate::synthesis::eval::Value::Str(plain.into()))],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Str(plain.into()),
                "non-keyword {:?} must pass through",
                plain
            );
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        // Deterministic across draws: the seed wins in every
        // independent synthesis run (Phase 27 drew Inconclusive
        // identity bodies and a constant).
        for _ in 0..3 {
            let pool = synthesize_with_module(&spec, 1, &module)
                .expect("opt-1 synthesis must succeed");
            let verified = verifier.verify_all(pool, &spec).expect("verify_all");
            let winner =
                crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
            assert!(winner.verified, "the composition-guard seed must win verified");
            assert!(
                (winner.scores.test_pass_ratio - 1.0).abs() < 1e-9,
                "winner tpr was {}",
                winner.scores.test_pass_ratio
            );
            assert_eq!(
                format!("{:?}", winner.body),
                format!("{:?}", seed.body),
                "the winning body must be the composition-guard seed in every draw"
            );
        }
    }

    /// Composition-guard seeds (ii): rust_generics_text's rows
    /// are guarded by the plain Bool parameter `present` and
    /// pin a literal (`""`) and a Concat
    /// (`"<" ++ joined ++ ">"`) — a guarded-identity deviation
    /// (a literal row) and a guard-table deviation (two
    /// params, a non-literal pin) at once. The seed must exist
    /// (kind `composition_guard`), be exactly the match over
    /// `present`, be Verified in module context, execute the
    /// pinned domain exactly, and win deterministically (the
    /// spec has no in-spec tests, so the win rides on the
    /// verdict, as the guarded-identity seed's did).
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_composition_guard_seed_for_generics_text() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_generics_text");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the composition-guard seed");
        let seed = constraint_seed(&engine, &spec, "composition_guard");
        assert_eq!(count_holes(&seed.body), 0);

        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("present".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Bool(false)),
                    IRNode::Literal(IRLiteral::String(String::new())),
                ),
                (
                    IRPattern::Literal(IRLiteral::Bool(true)),
                    IRNode::BinOp(
                        IRBinOp::Concat,
                        Box::new(IRNode::BinOp(
                            IRBinOp::Concat,
                            Box::new(IRNode::Literal(IRLiteral::String("<".into()))),
                            Box::new(IRNode::Var("joined".into())),
                        )),
                        Box::new(IRNode::Literal(IRLiteral::String(">".into()))),
                    ),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::String(String::new())),
                ),
            ],
        };
        assert_eq!(format!("{:?}", seed.body), format!("{:?}", expected));
        // No call to resolve: the resolver-visible body is the
        // same match.
        let index = SpecIndex::from_specs(&module);
        let raw = index
            .callee_body("rust_generics_text")
            .expect("the composition-guard body must be resolver-visible");
        assert_eq!(format!("{:?}", raw), format!("{:?}", expected));

        // Executed over the pinned domain.
        for (present, joined, want) in [
            (false, "T", ""),
            (false, "", ""),
            (true, "T", "<T>"),
            (true, "", "<>"),
            (true, "A, B", "<A, B>"),
        ] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("joined", crate::synthesis::eval::Value::Str(joined.into())),
                    ("present", crate::synthesis::eval::Value::Bool(present)),
                ],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Str(want.into()),
                "present={} joined={:?}",
                present,
                joined
            );
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        for _ in 0..3 {
            let pool = synthesize_with_module(&spec, 1, &module)
                .expect("opt-1 synthesis must succeed");
            let verified = verifier.verify_all(pool, &spec).expect("verify_all");
            let winner =
                crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
            assert!(winner.verified, "the composition-guard seed must win verified");
            assert_eq!(
                format!("{:?}", winner.body),
                format!("{:?}", seed.body),
                "the winning body must be the composition-guard seed in every draw"
            );
        }
    }

    /// Composition-guard seeds (negatives): near-miss shapes
    /// still get NO constraint seed — the generalization is as
    /// conservative as the builders it extends, and the two
    /// Phase 27 shapes recorded as separate generalizations
    /// stay unadmitted. Each spec below deviates in exactly
    /// one way.
    #[test]
    fn constraint_composition_guard_near_misses_get_no_seed() {
        let concat = |l: Expr, r: &str| {
            Expr::BinOp(
                BinOp::Concat,
                Box::new(l),
                Box::new(Expr::StringLit(r.into())),
            )
        };
        let mut cases: Vec<(SynthesisEngine, FunctionSpec)> = Vec::new();
        let plain = || SynthesisEngine::new(SynthesisConfig::default());

        // The output pin mentions `output` itself (inside a
        // compound pin, so the shape is otherwise squarely in
        // composition territory).
        cases.push((
            plain(),
            guard_spec(
                "comp_pin_mentions_output",
                vec![("flag", TypeRef::Bool)],
                TypeRef::String,
                vec![or_row(vec![
                    eq_expr(var_expr("flag"), Expr::BoolLit(true)),
                    eq_expr(var_expr("output"), concat(var_expr("output"), "!")),
                ])],
            ),
        ));

        // A pin calls a spec the module index does not hold.
        cases.push((
            plain(),
            guard_spec(
                "comp_pin_calls_unindexed",
                vec![("name", TypeRef::String), ("flag", TypeRef::Bool)],
                TypeRef::String,
                vec![
                    or_row(vec![
                        eq_expr(var_expr("flag"), Expr::BoolLit(false)),
                        eq_expr(var_expr("output"), concat(var_expr("name"), "!")),
                    ]),
                    or_row(vec![
                        eq_expr(var_expr("flag"), Expr::BoolLit(true)),
                        eq_expr(
                            var_expr("output"),
                            Expr::Call("render".into(), vec![var_expr("name")]),
                        ),
                    ]),
                ],
            ),
        ));

        // Two rows guarded by DIFFERENT expressions (two Bool
        // params). Until Phase 34 the recorded rationale was
        // composition's one-shared-guard requirement; the
        // shape is now the multi-guard family's territory,
        // and THIS instance stays seedless on that family's
        // own predicate: the rows jointly pin the
        // (flag, other) == (true, true) cell to two different
        // values (`a ++ "!"` by the first row, `a ++ "?"` by
        // the second) — a conflict, not a truth table.
        cases.push((
            plain(),
            guard_spec(
                "comp_mixed_guard_expressions",
                vec![
                    ("a", TypeRef::String),
                    ("flag", TypeRef::Bool),
                    ("other", TypeRef::Bool),
                ],
                TypeRef::String,
                vec![
                    or_row(vec![
                        eq_expr(var_expr("flag"), Expr::BoolLit(true)),
                        eq_expr(var_expr("output"), concat(var_expr("a"), "!")),
                    ]),
                    or_row(vec![
                        eq_expr(var_expr("other"), Expr::BoolLit(true)),
                        eq_expr(var_expr("output"), concat(var_expr("a"), "?")),
                    ]),
                ],
            ),
        ));

        // The guard is a call to a spec the index does not
        // hold at all.
        cases.push((
            plain(),
            guard_spec(
                "comp_guard_call_unindexed",
                vec![("name", TypeRef::String)],
                TypeRef::String,
                vec![
                    or_row(vec![
                        eq_expr(
                            Expr::Call("ghost".into(), vec![var_expr("name")]),
                            Expr::BoolLit(true),
                        ),
                        eq_expr(var_expr("output"), concat(var_expr("name"), "_")),
                    ]),
                    or_row(vec![
                        eq_expr(
                            Expr::Call("ghost".into(), vec![var_expr("name")]),
                            Expr::BoolLit(false),
                        ),
                        eq_expr(var_expr("output"), var_expr("name")),
                    ]),
                ],
            ),
        ));

        // The guard call resolves, but the callee does not
        // return Bool — its value has no finite {false, true}
        // domain to pin against.
        let echo = guard_spec(
            "echo",
            vec![("name", TypeRef::String)],
            TypeRef::String,
            vec![Constraint::Eq(var_expr("output"), var_expr("name"))],
        );
        let not_bool_target = guard_spec(
            "comp_guard_call_not_bool",
            vec![("name", TypeRef::String)],
            TypeRef::String,
            vec![
                or_row(vec![
                    eq_expr(
                        Expr::Call("echo".into(), vec![var_expr("name")]),
                        Expr::BoolLit(true),
                    ),
                    eq_expr(var_expr("output"), concat(var_expr("name"), "_")),
                ]),
                or_row(vec![
                    eq_expr(
                        Expr::Call("echo".into(), vec![var_expr("name")]),
                        Expr::BoolLit(false),
                    ),
                    eq_expr(var_expr("output"), var_expr("name")),
                ]),
            ],
        );
        cases.push((
            SynthesisEngine::with_module(
                SynthesisConfig::default(),
                &[echo, not_bool_target.clone()],
            ),
            not_bool_target,
        ));

        for (engine, spec) in &cases {
            assert!(
                engine.seed_from_constraints(spec).is_empty(),
                "{}: near-miss shape must get no constraint seed",
                spec.name
            );
        }

        // The recorded non-goal, on the real spec:
        // rust_ident_empty's String-equality leaf family
        // stays unadmitted. (rust_string_ownership stood
        // here until Phase 33: its two-parameter rows were
        // recorded as REGION rows, but as written they are
        // a pair-keyed guard table — a single true cell and
        // two half-plane false rows — and the pair-keyed
        // family claims them on its stated predicate; see
        // the Phase 33 builder tests. Genuinely regional
        // rows — a leaf comparing two parameters — remain
        // unadmitted: guard_leaf parses only
        // parameter-vs-constant leaves.)
        let module = load_codegen_module();
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        for name in ["rust_ident_empty"] {
            let spec = module_spec(&module, name);
            assert!(
                engine.seed_from_constraints(&spec).is_empty(),
                "{}: recorded non-goal must stay seedless",
                name
            );
        }
    }

    // ── Phase 32: expression-pin guard-table seeds ────────
    //
    // The fifth builder family, specified by the Phase 31
    // measurement: rows guarded on ONE Int key parameter
    // whose output pins are EXPRESSIONS over the parameters
    // — the shape of rust_unary_text (ten Concat pins under
    // `op`), rust_concat_right_text (a bare-Var pin and two
    // Concat pins under `form`) and rust_match_scrutinee_text
    // (a bare-Var pin and one Concat pin under `form`). Each
    // earlier family refused them on a measured predicate:
    // the guard table requires the pin to be an
    // IRNode::Literal; guarded_identity requires the pin to
    // be exactly the one remaining parameter; composition
    // guard's `guard_leaf_on_expr` is Bool-domain only. Their
    // pools held zero Verified candidates in every Phase 31
    // run and the crowned winners scored executed-pinned
    // 0/10, at best 1/4, and 0/3.

    /// One Int-keyed guard row in the spec dialect:
    /// `key<v || key>v || output==pin` — the row binds
    /// exactly at key == v.
    fn int_key_row(key: &str, v: i64, pin: Expr) -> Constraint {
        or_row(vec![
            Expr::BinOp(
                BinOp::Lt,
                Box::new(var_expr(key)),
                Box::new(Expr::IntLit(v)),
            ),
            Expr::BinOp(
                BinOp::Gt,
                Box::new(var_expr(key)),
                Box::new(Expr::IntLit(v)),
            ),
            eq_expr(var_expr("output"), pin),
        ])
    }

    fn concat_pin(l: Expr, r: &str) -> Expr {
        Expr::BinOp(
            BinOp::Concat,
            Box::new(l),
            Box::new(Expr::StringLit(r.into())),
        )
    }

    /// Expression-pin guard table (i): rust_unary_text's ten
    /// rows pin Concat templates under the Int key `op`. The
    /// seed must exist (kind `expr_pin_guard`), be one match
    /// over `op` with an arm per row plus the typed-literal
    /// wildcard, execute to the pinned template at every op
    /// code, be Verified in module context, and win
    /// module-aware synthesis deterministically.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_expr_pin_guard_seed_for_unary_text() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_unary_text");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the expression-pin seed");
        let seed = constraint_seed(&engine, &spec, "expr_pin_guard");
        assert_eq!(count_holes(&seed.body), 0);

        let IRNode::Match { scrutinee, cases } = &seed.body else {
            panic!("seed body must be a match, got {:?}", seed.body);
        };
        assert_eq!(
            format!("{:?}", **scrutinee),
            format!("{:?}", IRNode::Var("op".into()))
        );
        assert_eq!(cases.len(), 11, "ten row arms plus the wildcard");
        for (i, (pat, _)) in cases.iter().enumerate().take(10) {
            assert_eq!(
                format!("{:?}", pat),
                format!("{:?}", IRPattern::Literal(IRLiteral::Int(i as i64))),
                "arm {} must pin op == {}",
                i,
                i
            );
        }
        assert_eq!(
            format!("{:?}", cases[10]),
            format!(
                "{:?}",
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::String(String::new()))
                )
            ),
            "the wildcard carries the typed-literal default"
        );

        // Every pinned op code executes to its template; an
        // off-table code falls to the wildcard default.
        let expected = [
            "(- x)",
            "(! x)",
            "x.abs()",
            "x.len()",
            "(x)[0]",
            "(x)[1..].to_vec()",
            "(x)[..(x).len() - 1].to_vec()",
            "(x)[(x).len() - 1]",
            "x.reverse()",
            "x.clone()",
        ];
        for (op, want) in expected.iter().enumerate() {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("op", crate::synthesis::eval::Value::Int(op as i64)),
                    ("inner", crate::synthesis::eval::Value::Str("x".into())),
                ],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Str((*want).into()),
                "op {} must render its pinned template",
                op
            );
        }
        let v = eval_seed_body(
            &seed.body,
            &[
                ("op", crate::synthesis::eval::Value::Int(42)),
                ("inner", crate::synthesis::eval::Value::Str("x".into())),
            ],
        );
        assert_eq!(v, crate::synthesis::eval::Value::Str(String::new()));

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        // Deterministic across draws: the same winning body
        // in three independent synthesis runs.
        for _ in 0..3 {
            let pool = synthesize_with_module(&spec, 1, &module)
                .expect("opt-1 synthesis must succeed");
            let verified = verifier.verify_all(pool, &spec).expect("verify_all");
            let winner =
                crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
            assert!(winner.verified, "the expression-pin seed must win verified");
            assert_eq!(
                format!("{:?}", winner.body),
                format!("{:?}", seed.body),
                "the winning body must be the expression-pin seed in every draw"
            );
        }
    }

    /// Expression-pin guard table (ii): the two mixed-pin
    /// specs — a bare-Var pin alongside Concat pins under the
    /// Int key `form`. Mixed pin sets are admitted wholesale
    /// (see the synthetic test below): the Var rows are just
    /// pins that happen to be variables.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_expr_pin_guard_seed_for_mixed_pin_specs() {
        let module = load_codegen_module();
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let verifier = crate::verify::verifier::Verifier::with_specs(&module);

        let spec = module_spec(&module, "rust_concat_right_text");
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the expression-pin seed");
        let seed = constraint_seed(&engine, &spec, "expr_pin_guard");
        assert_eq!(count_holes(&seed.body), 0);
        for (form, want) in [(0, "r"), (1, "&(r)"), (2, "&r"), (9, "")] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("form", crate::synthesis::eval::Value::Int(form)),
                    ("rendered", crate::synthesis::eval::Value::Str("r".into())),
                ],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Str(want.into()),
                "form {} must render its pinned text",
                form
            );
        }
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        let spec = module_spec(&module, "rust_match_scrutinee_text");
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the expression-pin seed");
        let seed = constraint_seed(&engine, &spec, "expr_pin_guard");
        assert_eq!(count_holes(&seed.body), 0);
        for (form, want) in [(0, "r"), (1, "r.as_str()"), (9, "")] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("form", crate::synthesis::eval::Value::Int(form)),
                    ("rendered", crate::synthesis::eval::Value::Str("r".into())),
                ],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Str(want.into()),
                "form {} must render its pinned text",
                form
            );
        }
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);
    }

    /// Expression-pin guard table (iii): the admission
    /// semantics on synthetic shapes. Mixed pin sets are
    /// admitted WHOLESALE — a literal pin or a bare-Var pin
    /// among compound pins is just a pin — provided at least
    /// one pin is a compound expression; that single compound
    /// requirement is the territory rule (all-literal sets
    /// stay with the guard tables and the recorded pair-keyed
    /// generalization, all-Var sets with guarded identity).
    /// A pin calling an indexed spec is admitted and resolved
    /// before admission, like a composition pin.
    #[test]
    fn constraint_expr_pin_guard_seed_synthetic_shapes() {
        // Basic: two Concat pins under one Int key.
        let spec = guard_spec(
            "expr_pin_basic",
            vec![("code", TypeRef::Int), ("text", TypeRef::String)],
            TypeRef::String,
            vec![
                int_key_row(
                    "code",
                    0,
                    Expr::BinOp(
                        BinOp::Concat,
                        Box::new(Expr::StringLit("<".into())),
                        Box::new(var_expr("text")),
                    ),
                ),
                int_key_row("code", 1, concat_pin(var_expr("text"), ">")),
            ],
        );
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let seed = constraint_seed(&engine, &spec, "expr_pin_guard");
        for (code, want) in [(0, "<m"), (1, "m>"), (5, "")] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("code", crate::synthesis::eval::Value::Int(code)),
                    ("text", crate::synthesis::eval::Value::Str("m".into())),
                ],
            );
            assert_eq!(v, crate::synthesis::eval::Value::Str(want.into()));
        }

        // Mixed wholesale: a literal pin beside a Concat pin.
        let spec = guard_spec(
            "expr_pin_mixed_literal",
            vec![("code", TypeRef::Int), ("text", TypeRef::String)],
            TypeRef::String,
            vec![
                int_key_row("code", 0, Expr::StringLit("none".into())),
                int_key_row("code", 1, concat_pin(var_expr("text"), "!")),
            ],
        );
        let seed = constraint_seed(&engine, &spec, "expr_pin_guard");
        for (code, want) in [(0, "none"), (1, "m!")] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("code", crate::synthesis::eval::Value::Int(code)),
                    ("text", crate::synthesis::eval::Value::Str("m".into())),
                ],
            );
            assert_eq!(v, crate::synthesis::eval::Value::Str(want.into()));
        }

        // A pin calling an indexed spec: admitted, and the
        // admitted body has the call resolved (the
        // composition seed's precedent — a raw call body
        // could not be executed by the interpreter).
        let wrap = guard_spec(
            "wrap_q",
            vec![("s", TypeRef::String)],
            TypeRef::String,
            vec![Constraint::Eq(
                var_expr("output"),
                concat_pin(var_expr("s"), "?"),
            )],
        );
        let spec = guard_spec(
            "expr_pin_call",
            vec![("code", TypeRef::Int), ("text", TypeRef::String)],
            TypeRef::String,
            vec![
                int_key_row(
                    "code",
                    0,
                    Expr::Call("wrap_q".into(), vec![var_expr("text")]),
                ),
                int_key_row("code", 1, concat_pin(var_expr("text"), ">")),
            ],
        );
        let engine = SynthesisEngine::with_module(
            SynthesisConfig::default(),
            &[wrap, spec.clone()],
        );
        let seed = constraint_seed(&engine, &spec, "expr_pin_guard");
        assert_eq!(count_holes(&seed.body), 0);
        let v = eval_seed_body(
            &seed.body,
            &[
                ("code", crate::synthesis::eval::Value::Int(0)),
                ("text", crate::synthesis::eval::Value::Str("m".into())),
            ],
        );
        assert_eq!(
            v,
            crate::synthesis::eval::Value::Str("m?".into()),
            "the call pin must be resolved to the callee's body"
        );
    }

    /// Expression-pin guard table (iv): near misses. Each
    /// shape is refused by a stated predicate — territory
    /// (no compound pin), pin admissibility (a pin mentioning
    /// `output`, converting through a Hole, or calling an
    /// unindexed spec), or key discipline (rows guarded on
    /// two different Int parameters, one row spanning two
    /// keys, a row pinning a range rather than one value).
    #[test]
    fn constraint_expr_pin_guard_near_misses_get_no_seed() {
        let plain = || SynthesisEngine::new(SynthesisConfig::default());
        let mut empty_cases: Vec<FunctionSpec> = Vec::new();

        // All-literal pins under one Int key, at arity two:
        // not a guard table (its arity is one), not this
        // family (no compound pin — the pin test keeps the
        // families disjoint; single-key literal tables at
        // other arities belong to the recorded pair-keyed
        // generalization's territory discussion, not here).
        empty_cases.push(guard_spec(
            "expr_pin_all_literal",
            vec![("code", TypeRef::Int), ("text", TypeRef::String)],
            TypeRef::String,
            vec![
                int_key_row("code", 0, Expr::StringLit("a".into())),
                int_key_row("code", 1, Expr::StringLit("b".into())),
            ],
        ));

        // Bare-Var and literal pins only, no compound pin:
        // guarded identity deviates on the literal row, and
        // this family requires a compound pin.
        empty_cases.push(guard_spec(
            "expr_pin_var_and_literal",
            vec![("code", TypeRef::Int), ("text", TypeRef::String)],
            TypeRef::String,
            vec![
                int_key_row("code", 0, var_expr("text")),
                int_key_row("code", 1, Expr::StringLit("x".into())),
            ],
        ));

        // The pin mentions `output` itself.
        empty_cases.push(guard_spec(
            "expr_pin_mentions_output",
            vec![("code", TypeRef::Int), ("text", TypeRef::String)],
            TypeRef::String,
            vec![int_key_row(
                "code",
                0,
                concat_pin(var_expr("output"), "!"),
            )],
        ));

        // The pin converts through a Hole (a Length the
        // expression converter drops): not a whole pin.
        empty_cases.push(guard_spec(
            "expr_pin_hole",
            vec![("code", TypeRef::Int), ("text", TypeRef::String)],
            TypeRef::String,
            vec![int_key_row(
                "code",
                0,
                concat_pin(Expr::Length(Box::new(var_expr("text"))), "!"),
            )],
        ));

        // The pin calls a spec the index does not hold.
        empty_cases.push(guard_spec(
            "expr_pin_calls_unindexed",
            vec![("code", TypeRef::Int), ("text", TypeRef::String)],
            TypeRef::String,
            vec![int_key_row(
                "code",
                0,
                Expr::Call("ghost".into(), vec![var_expr("text")]),
            )],
        ));

        // Rows guarded on two DIFFERENT Int keys: the table
        // has one key or none.
        empty_cases.push(guard_spec(
            "expr_pin_two_keys",
            vec![
                ("a", TypeRef::Int),
                ("b", TypeRef::Int),
                ("text", TypeRef::String),
            ],
            TypeRef::String,
            vec![
                int_key_row("a", 0, concat_pin(var_expr("text"), "!")),
                int_key_row("b", 1, concat_pin(var_expr("text"), "?")),
            ],
        ));

        // One row whose guard leaves span two keys.
        empty_cases.push(guard_spec(
            "expr_pin_row_spans_keys",
            vec![
                ("a", TypeRef::Int),
                ("b", TypeRef::Int),
                ("text", TypeRef::String),
            ],
            TypeRef::String,
            vec![or_row(vec![
                Expr::BinOp(
                    BinOp::Lt,
                    Box::new(var_expr("a")),
                    Box::new(Expr::IntLit(0)),
                ),
                Expr::BinOp(
                    BinOp::Lt,
                    Box::new(var_expr("b")),
                    Box::new(Expr::IntLit(0)),
                ),
                eq_expr(var_expr("output"), concat_pin(var_expr("text"), "!")),
            ])],
        ));

        // A row pinning a range, not a single key value:
        // `code<0||code>2` is falsified by 0, 1 and 2 alike.
        empty_cases.push(guard_spec(
            "expr_pin_range",
            vec![("code", TypeRef::Int), ("text", TypeRef::String)],
            TypeRef::String,
            vec![or_row(vec![
                Expr::BinOp(
                    BinOp::Lt,
                    Box::new(var_expr("code")),
                    Box::new(Expr::IntLit(0)),
                ),
                Expr::BinOp(
                    BinOp::Gt,
                    Box::new(var_expr("code")),
                    Box::new(Expr::IntLit(2)),
                ),
                eq_expr(var_expr("output"), concat_pin(var_expr("text"), "!")),
            ])],
        ));

        for spec in &empty_cases {
            assert!(
                plain().seed_from_constraints(spec).is_empty(),
                "{}: near-miss shape must get no constraint seed",
                spec.name
            );
        }

        // All-bare-Var pins under an Int key: guarded
        // identity's exact shape — it claims the spec, and
        // this family must not add a second seed.
        let spec = guard_spec(
            "expr_pin_all_var",
            vec![("code", TypeRef::Int), ("text", TypeRef::String)],
            TypeRef::String,
            vec![
                int_key_row("code", 0, var_expr("text")),
                int_key_row("code", 1, var_expr("text")),
            ],
        );
        let seeds = plain().seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the guarded-identity seed");
        assert!(
            matches!(
                &seeds[0].provenance,
                Provenance::ConstraintInduced { kind } if kind == "guarded_identity"
            ),
            "the one seed must be guarded identity's, got {:?}",
            seeds[0].provenance
        );
    }

    // ── Phase 33: pair-keyed guard-table seeds ────────────
    //
    // The Phase 31 measurement's sibling shape (i): rows
    // whose guard leaves constrain TWO Int parameters (a
    // (kind, lit) pair) with LITERAL pins —
    // rust_concat_left_owned, rust_concat_right_form and the
    // by-design partial rust_match_scrutinee_bare (G2).
    // guard_table_body's arity is one, so all three were
    // seedless and their winners lotteries over a 363-cell
    // grid (a near-constant draw scores 362/363 by missing
    // only the (9, 3) cell). The generalization is the guard
    // table's own machinery on two axes: the pinned CELL is
    // recovered by joint falsify-every-leaf over the pair —
    // a cell is pinned exactly where the rows jointly
    // determine one output value, rows pinning whole
    // rows/columns (scrutinee_bare's kind-only and lit-only
    // rows) pin their cells by the same joint falsification,
    // and cells no row decides (scrutinee_bare's kind-22
    // column) stay unpinned and fall to the wildcard.
    // Literal pins only: the pin test is what keeps this
    // family disjoint from the expression-pin guard table.

    fn cmp_leaf(param: &str, op: BinOp, v: i64) -> Expr {
        Expr::BinOp(op, Box::new(var_expr(param)), Box::new(Expr::IntLit(v)))
    }

    /// One pair-keyed guard row: the Or of the given guard
    /// leaves plus the output pin.
    fn pair_row(leaves: Vec<Expr>, pin: Expr) -> Constraint {
        let mut leaves = leaves;
        leaves.push(eq_expr(var_expr("output"), pin));
        or_row(leaves)
    }

    /// The nested-match body the PARTIAL Bool pair table
    /// (rust_match_scrutinee_bare) takes: `match kind { 9 =>
    /// match lit { 3 => true, _ => false }, _ => false }` —
    /// the (9, 3) cell armed, its column's remaining pinned
    /// cells on the inner wildcard, and every other column
    /// on the outer wildcard, whose value is the pinned
    /// fall-through value (false), NOT the typed-literal
    /// default (Bool's menu starts at true).
    /// rust_concat_left_owned's body shared this shape
    /// until Phase 39, when its spec gained the Var row:
    /// its body carries one more arm (kind 10 → true)
    /// before the wildcard, built inline in its test.
    fn pair_bool_table_body() -> IRNode {
        IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(9)),
                    IRNode::Match {
                        scrutinee: Box::new(IRNode::Var("lit".into())),
                        cases: vec![
                            (
                                IRPattern::Literal(IRLiteral::Int(3)),
                                IRNode::Literal(IRLiteral::Bool(true)),
                            ),
                            (
                                IRPattern::Wildcard,
                                IRNode::Literal(IRLiteral::Bool(false)),
                            ),
                        ],
                    },
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Bool(false)),
                ),
            ],
        }
    }

    /// Pair-keyed guard table (i): rust_concat_left_owned's
    /// rows pin true exactly at (kind 9, lit 3) and at every
    /// kind-10 (Var) cell — Phase 39 added the Var row when
    /// the emitter's left-operand treatment grew the clone
    /// arm — and false everywhere else: a total (kind, lit)
    /// table. The seed must exist (kind `pair_guard_table`)
    /// as the spec's only constraint seed, be the nested
    /// match (the pair_bool_table_body shape plus the
    /// kind-10 arm), execute to the spec's rule over the
    /// whole cross-validation grid, be Verified in module
    /// context, and win module-aware synthesis
    /// deterministically.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_pair_guard_table_seed_for_left_owned() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_concat_left_owned");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the pair-keyed seed");
        let seed = constraint_seed(&engine, &spec, "pair_guard_table");
        assert_eq!(count_holes(&seed.body), 0);
        // The shared pair-table shape plus the Phase 39
        // Var arm (kind 10 → true) before the wildcard.
        let mut expected = pair_bool_table_body();
        if let IRNode::Match { cases, .. } = &mut expected {
            let wildcard = cases.pop().expect("wildcard arm");
            cases.push((
                IRPattern::Literal(IRLiteral::Int(10)),
                IRNode::Literal(IRLiteral::Bool(true)),
            ));
            cases.push(wildcard);
        }
        assert_eq!(
            format!("{:?}", seed.body),
            format!("{:?}", expected),
            "the seed must be the nested pair table"
        );

        // Executed over the whole 33 x 11 grid the
        // cross-validation harness sweeps.
        for kind in -2..=30 {
            for lit in -2..=8 {
                let v = eval_seed_body(
                    &seed.body,
                    &[
                        ("kind", crate::synthesis::eval::Value::Int(kind)),
                        ("lit", crate::synthesis::eval::Value::Int(lit)),
                    ],
                );
                assert_eq!(
                    v,
                    crate::synthesis::eval::Value::Bool(
                        kind == 10 || (kind == 9 && lit == 3)
                    ),
                    "({kind}, {lit}) must follow the spec's table"
                );
            }
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        // Deterministic across draws: the same winning body
        // in three independent synthesis runs.
        for _ in 0..3 {
            let pool = synthesize_with_module(&spec, 1, &module)
                .expect("opt-1 synthesis must succeed");
            let verified = verifier.verify_all(pool, &spec).expect("verify_all");
            let winner =
                crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
            assert!(winner.verified, "the pair-keyed seed must win verified");
            assert_eq!(
                format!("{:?}", winner.body),
                format!("{:?}", seed.body),
                "the winning body must be the pair-keyed seed in every draw"
            );
        }
    }

    /// Pair-keyed guard table (ii): rust_match_scrutinee_bare
    /// is PARTIAL by design (G2) — no row decides the kind-22
    /// column, so those cells stay unpinned and fall to the
    /// wildcard (which carries false, the value the pinned
    /// fall-through cells determine; on the unpinned column
    /// that is the documented guess, not a spec claim). A
    /// partial table is NOT a refusal: the seed exists, its
    /// pinned cells all follow the rows, and it is Verified
    /// — the constraints never mention kind 22's output.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_pair_guard_table_seed_for_scrutinee_bare() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_match_scrutinee_bare");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the pair-keyed seed");
        let seed = constraint_seed(&engine, &spec, "pair_guard_table");
        assert_eq!(count_holes(&seed.body), 0);
        assert_eq!(
            format!("{:?}", seed.body),
            format!("{:?}", pair_bool_table_body()),
            "the partial table takes the same nested shape"
        );

        // Every pinned cell follows the rows; the kind-22
        // column is spec-silent and lands on the wildcard.
        for kind in -2..=30 {
            for lit in -2..=8 {
                let v = eval_seed_body(
                    &seed.body,
                    &[
                        ("kind", crate::synthesis::eval::Value::Int(kind)),
                        ("lit", crate::synthesis::eval::Value::Int(lit)),
                    ],
                );
                if kind == 22 {
                    assert_eq!(
                        v,
                        crate::synthesis::eval::Value::Bool(false),
                        "({kind}, {lit}) is unpinned: the wildcard's documented guess"
                    );
                } else {
                    assert_eq!(
                        v,
                        crate::synthesis::eval::Value::Bool(kind == 9 && lit == 3),
                        "({kind}, {lit}) is pinned by the rows"
                    );
                }
            }
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        let pool = synthesize_with_module(&spec, 1, &module)
            .expect("opt-1 synthesis must succeed");
        let verified = verifier.verify_all(pool, &spec).expect("verify_all");
        let winner =
            crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
        assert!(winner.verified, "the pair-keyed seed must win verified");
        assert_eq!(
            format!("{:?}", winner.body),
            format!("{:?}", seed.body),
            "the winning body must be the pair-keyed seed"
        );
    }

    /// Pair-keyed guard table (iii): rust_concat_right_form
    /// — thirteen rows over (kind, lit): (9, 3) → 0, the ten
    /// block-like kinds → 1 at every lit, and a catch-all
    /// row → 2 for every remaining pair. Columns whose cells
    /// all pin the fall-through value (2) need no arm: the
    /// outer wildcard already yields it. The seed is the
    /// nested match with eleven kind arms plus the wildcard,
    /// Verified, executing to the mirror's rule over the
    /// whole grid, and winning synthesis.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_pair_guard_table_seed_for_right_form() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_concat_right_form");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the pair-keyed seed");
        let seed = constraint_seed(&engine, &spec, "pair_guard_table");
        assert_eq!(count_holes(&seed.body), 0);

        let IRNode::Match { scrutinee, cases } = &seed.body else {
            panic!("seed body must be a match, got {:?}", seed.body);
        };
        assert_eq!(
            format!("{:?}", **scrutinee),
            format!("{:?}", IRNode::Var("kind".into()))
        );
        assert_eq!(cases.len(), 12, "eleven kind arms plus the wildcard");
        let armed: Vec<i64> = cases[..11]
            .iter()
            .map(|(p, _)| match p {
                IRPattern::Literal(IRLiteral::Int(v)) => *v,
                other => panic!("arm pattern must be an Int literal, got {:?}", other),
            })
            .collect();
        assert_eq!(armed, vec![1, 2, 3, 4, 5, 6, 9, 13, 14, 24, 26]);
        for (pat, body) in &cases[..11] {
            if matches!(pat, IRPattern::Literal(IRLiteral::Int(9))) {
                // The kind-9 column splits on lit: (9, 3) is
                // family 0, every other lit family 2.
                assert_eq!(
                    format!("{:?}", body),
                    format!(
                        "{:?}",
                        IRNode::Match {
                            scrutinee: Box::new(IRNode::Var("lit".into())),
                            cases: vec![
                                (
                                    IRPattern::Literal(IRLiteral::Int(3)),
                                    IRNode::Literal(IRLiteral::Int(0)),
                                ),
                                (
                                    IRPattern::Wildcard,
                                    IRNode::Literal(IRLiteral::Int(2)),
                                ),
                            ],
                        }
                    ),
                    "the kind-9 arm must be the inner lit match"
                );
            } else {
                assert_eq!(
                    format!("{:?}", body),
                    format!("{:?}", IRNode::Literal(IRLiteral::Int(1))),
                    "block-like kind arms render family 1 directly"
                );
            }
        }
        assert_eq!(
            format!("{:?}", cases[11]),
            format!(
                "{:?}",
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Int(2))
                )
            ),
            "the wildcard carries the catch-all row's family 2"
        );

        // Executed over the whole grid, against the
        // cross-validation mirror's rule.
        for kind in -2..=30 {
            for lit in -2..=8 {
                let want = if kind == 9 && lit == 3 {
                    0
                } else if matches!(kind, 1 | 2 | 3 | 4 | 5 | 6 | 13 | 14 | 24 | 26) {
                    1
                } else {
                    2
                };
                let v = eval_seed_body(
                    &seed.body,
                    &[
                        ("kind", crate::synthesis::eval::Value::Int(kind)),
                        ("lit", crate::synthesis::eval::Value::Int(lit)),
                    ],
                );
                assert_eq!(
                    v,
                    crate::synthesis::eval::Value::Int(want),
                    "({kind}, {lit}) must follow the spec's table"
                );
            }
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        let pool = synthesize_with_module(&spec, 1, &module)
            .expect("opt-1 synthesis must succeed");
        let verified = verifier.verify_all(pool, &spec).expect("verify_all");
        let winner =
            crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
        assert!(winner.verified, "the pair-keyed seed must win verified");
        assert_eq!(
            format!("{:?}", winner.body),
            format!("{:?}", seed.body),
            "the winning body must be the pair-keyed seed"
        );
    }

    /// Pair-keyed guard table (iv): rust_string_ownership —
    /// the side effect this phase did not target and must
    /// not hide. Its rows were recorded in Phase 27 as the
    /// larger separate generalization ("two-parameter
    /// region rows; pinned-value recovery would be a
    /// coverage problem") and asserted seedless as a
    /// recorded non-goal. As WRITTEN, though, the rows are
    /// a pair table: `lit<3||lit>3||expected<3||
    /// expected>3||output==true` pins the single cell
    /// (3, 3), and the two half-plane rows pin false on
    /// lit != 3 and on expected != 3 — jointly, every cell
    /// of the (lit, expected) grid is determined, exactly
    /// rust_concat_left_owned's three-row pattern at a
    /// different constant. The "coverage problem" was the
    /// SINGLE-key recovery problem (each row alone pins a
    /// half-plane, no single value); joint falsify-every-
    /// leaf over the pair recovers the whole table. The
    /// builder's admission predicate — the guard family's
    /// rows, leaves over exactly two Int parameters,
    /// literal pins — is satisfied exactly, so the family
    /// claims it; a name-based carve-out would break the
    /// predicate discipline every family here is built on.
    /// What stays unadmitted is the genuinely regional
    /// shape: leaves comparing a parameter against another
    /// PARAMETER (guard_leaf parses only parameter-vs-
    /// constant leaves), which no measured spec writes.
    /// The seed is Verified, executes to the spec's rule
    /// over the whole 33 x 33 grid the cross-validation
    /// harness sweeps (all 1089 cases pinned — the spec is
    /// total), and wins synthesis.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_pair_guard_table_seed_for_string_ownership() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_string_ownership");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the pair-keyed seed");
        let seed = constraint_seed(&engine, &spec, "pair_guard_table");
        assert_eq!(count_holes(&seed.body), 0);
        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("lit".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(3)),
                    IRNode::Match {
                        scrutinee: Box::new(IRNode::Var("expected".into())),
                        cases: vec![
                            (
                                IRPattern::Literal(IRLiteral::Int(3)),
                                IRNode::Literal(IRLiteral::Bool(true)),
                            ),
                            (
                                IRPattern::Wildcard,
                                IRNode::Literal(IRLiteral::Bool(false)),
                            ),
                        ],
                    },
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Bool(false)),
                ),
            ],
        };
        assert_eq!(
            format!("{:?}", seed.body),
            format!("{:?}", expected),
            "the seed must be the nested pair table"
        );

        for lit in -2..=30 {
            for expected_v in -2..=30 {
                let v = eval_seed_body(
                    &seed.body,
                    &[
                        ("lit", crate::synthesis::eval::Value::Int(lit)),
                        ("expected", crate::synthesis::eval::Value::Int(expected_v)),
                    ],
                );
                assert_eq!(
                    v,
                    crate::synthesis::eval::Value::Bool(lit == 3 && expected_v == 3),
                    "({lit}, {expected_v}) must follow the spec's table"
                );
            }
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        let pool = synthesize_with_module(&spec, 1, &module)
            .expect("opt-1 synthesis must succeed");
        let verified = verifier.verify_all(pool, &spec).expect("verify_all");
        let winner =
            crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
        assert!(winner.verified, "the pair-keyed seed must win verified");
        assert_eq!(
            format!("{:?}", winner.body),
            format!("{:?}", seed.body),
            "the winning body must be the pair-keyed seed"
        );
    }

    /// Pair-keyed guard table (v): the admission semantics
    /// on synthetic shapes — a total table whose rows pin
    /// whole rows/columns beside the one cell row, and a
    /// partial table whose undecided column falls to the
    /// wildcard without refusing the seed.
    #[test]
    fn constraint_pair_guard_table_seed_synthetic_shapes() {
        // Total: (2, 3) -> 7 by the cell row; a != 2 -> 0 by
        // the kind-only row; a == 2 at any other lit -> 0 by
        // the mixed row.
        let spec = guard_spec(
            "pair_basic",
            vec![("a", TypeRef::Int), ("b", TypeRef::Int)],
            TypeRef::Int,
            vec![
                pair_row(
                    vec![
                        cmp_leaf("a", BinOp::Lt, 2),
                        cmp_leaf("a", BinOp::Gt, 2),
                        cmp_leaf("b", BinOp::Lt, 3),
                        cmp_leaf("b", BinOp::Gt, 3),
                    ],
                    Expr::IntLit(7),
                ),
                pair_row(
                    vec![eq_expr(var_expr("a"), Expr::IntLit(2))],
                    Expr::IntLit(0),
                ),
                pair_row(
                    vec![
                        cmp_leaf("a", BinOp::Lt, 2),
                        cmp_leaf("a", BinOp::Gt, 2),
                        eq_expr(var_expr("b"), Expr::IntLit(3)),
                    ],
                    Expr::IntLit(0),
                ),
            ],
        );
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let seed = constraint_seed(&engine, &spec, "pair_guard_table");
        for (a, b, want) in [(2, 3, 7), (2, 0, 0), (0, 3, 0), (9, 9, 0), (-5, -5, 0)] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("a", crate::synthesis::eval::Value::Int(a)),
                    ("b", crate::synthesis::eval::Value::Int(b)),
                ],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Int(want),
                "({a}, {b}) must follow the table"
            );
        }

        // Partial: no row decides the a == 3 column (both
        // false rows except it by name), so the column falls
        // to the wildcard — the seed is still admitted.
        let spec = guard_spec(
            "pair_partial",
            vec![("a", TypeRef::Int), ("b", TypeRef::Int)],
            TypeRef::Int,
            vec![
                pair_row(
                    vec![
                        cmp_leaf("a", BinOp::Lt, 1),
                        cmp_leaf("a", BinOp::Gt, 1),
                        cmp_leaf("b", BinOp::Lt, 2),
                        cmp_leaf("b", BinOp::Gt, 2),
                    ],
                    Expr::IntLit(7),
                ),
                pair_row(
                    vec![
                        eq_expr(var_expr("a"), Expr::IntLit(1)),
                        eq_expr(var_expr("a"), Expr::IntLit(3)),
                    ],
                    Expr::IntLit(0),
                ),
                pair_row(
                    vec![
                        eq_expr(var_expr("b"), Expr::IntLit(2)),
                        eq_expr(var_expr("a"), Expr::IntLit(3)),
                    ],
                    Expr::IntLit(0),
                ),
            ],
        );
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(
            seeds.len(),
            1,
            "a partial table is admitted, not refused: exactly one seed"
        );
        let seed = constraint_seed(&engine, &spec, "pair_guard_table");
        for (a, b, want) in [(1, 2, 7), (1, 1, 0), (0, 0, 0), (3, 2, 0), (3, 0, 0)] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("a", crate::synthesis::eval::Value::Int(a)),
                    ("b", crate::synthesis::eval::Value::Int(b)),
                ],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Int(want),
                "({a}, {b}): pinned cells follow the rows; (3, *) is the wildcard's guess"
            );
        }
    }

    /// Pair-keyed guard table (vi): near misses. Each shape
    /// is refused by a stated predicate — key discipline
    /// (rows over ONE key are the single-key families'
    /// shapes, refused here on the two-key requirement; rows
    /// over THREE keys exceed the pair), the pin test (a
    /// compound pin is the expression-pin family's shape and
    /// a bare-Var pin guarded identity's — both span two
    /// keys here, so no family states them), joint
    /// consistency (two rows pinning one cell to different
    /// values is not a table), and pin admissibility (a pin
    /// converting through a Hole is not a literal).
    #[test]
    fn constraint_pair_guard_table_near_misses_get_no_seed() {
        let plain = || SynthesisEngine::new(SynthesisConfig::default());
        let mut empty_cases: Vec<FunctionSpec> = Vec::new();

        // Rows over ONE key at arity two: the guard table's
        // arity is one and this family requires two keys.
        empty_cases.push(guard_spec(
            "pair_single_key",
            vec![("a", TypeRef::Int), ("text", TypeRef::String)],
            TypeRef::Int,
            vec![
                int_key_row("a", 0, Expr::IntLit(1)),
                int_key_row("a", 1, Expr::IntLit(2)),
            ],
        ));

        // Two-key rows with a compound pin: the
        // expression-pin family's pin shape, but its key
        // discipline is one key — no family states this.
        empty_cases.push(guard_spec(
            "pair_compound_pin",
            vec![("a", TypeRef::Int), ("b", TypeRef::Int)],
            TypeRef::String,
            vec![pair_row(
                vec![
                    cmp_leaf("a", BinOp::Lt, 1),
                    cmp_leaf("a", BinOp::Gt, 1),
                    cmp_leaf("b", BinOp::Lt, 2),
                    cmp_leaf("b", BinOp::Gt, 2),
                ],
                concat_pin(var_expr("a"), "!"),
            )],
        ));

        // Two-key rows with a bare-Var pin: guarded
        // identity's pin shape, but its guards range over
        // one parameter — no family states this either.
        empty_cases.push(guard_spec(
            "pair_var_pin",
            vec![("a", TypeRef::Int), ("b", TypeRef::Int)],
            TypeRef::Int,
            vec![pair_row(
                vec![
                    cmp_leaf("a", BinOp::Lt, 1),
                    cmp_leaf("a", BinOp::Gt, 1),
                    cmp_leaf("b", BinOp::Lt, 2),
                    cmp_leaf("b", BinOp::Gt, 2),
                ],
                var_expr("b"),
            )],
        ));

        // Rows over THREE keys: beyond the pair.
        empty_cases.push(guard_spec(
            "pair_three_keys",
            vec![
                ("a", TypeRef::Int),
                ("b", TypeRef::Int),
                ("c", TypeRef::Int),
            ],
            TypeRef::Int,
            vec![pair_row(
                vec![
                    cmp_leaf("a", BinOp::Lt, 1),
                    cmp_leaf("b", BinOp::Lt, 2),
                    cmp_leaf("c", BinOp::Lt, 3),
                ],
                Expr::IntLit(1),
            )],
        ));

        // Two rows pinning one cell to different values:
        // (1, 1) is 5 by the cell row and 6 by the a == 1
        // row — jointly inconsistent, not a table.
        empty_cases.push(guard_spec(
            "pair_conflict",
            vec![("a", TypeRef::Int), ("b", TypeRef::Int)],
            TypeRef::Int,
            vec![
                pair_row(
                    vec![
                        cmp_leaf("a", BinOp::Lt, 1),
                        cmp_leaf("a", BinOp::Gt, 1),
                        cmp_leaf("b", BinOp::Lt, 1),
                        cmp_leaf("b", BinOp::Gt, 1),
                    ],
                    Expr::IntLit(5),
                ),
                pair_row(
                    vec![cmp_leaf("a", BinOp::Lt, 1), cmp_leaf("a", BinOp::Gt, 1)],
                    Expr::IntLit(6),
                ),
            ],
        ));

        // A pin converting through a Hole (a Length has no
        // literal IR form): not a literal pin.
        empty_cases.push(guard_spec(
            "pair_hole_pin",
            vec![("a", TypeRef::Int), ("b", TypeRef::Int)],
            TypeRef::Int,
            vec![pair_row(
                vec![
                    cmp_leaf("a", BinOp::Lt, 1),
                    cmp_leaf("a", BinOp::Gt, 1),
                    cmp_leaf("b", BinOp::Lt, 2),
                    cmp_leaf("b", BinOp::Gt, 2),
                ],
                Expr::Length(Box::new(var_expr("a"))),
            )],
        ));

        for spec in &empty_cases {
            assert!(
                plain().seed_from_constraints(spec).is_empty(),
                "{}: near-miss shape must get no constraint seed",
                spec.name
            );
        }
    }

    // ── Phase 38: mixed pair-keyed guard tables ─────────
    //
    // The Phase 36 measurement's item (d): rust_stmt_seq_
    // form's rows range jointly over an Int key and a Bool
    // key — a mixed two-key table every admitted family
    // refused on a stated predicate (guard_table: arity;
    // pair_guard_table: both keys Int-family; composition
    // and multi-guard: Bool-domain leaves only, and the
    // rows carry Lt/Gt leaves on the Int key; expression-
    // pin guard table: one Int key, and its territory
    // requires a compound pin where these pins are
    // literal). Its pools held zero Verified candidates in
    // 60 draws; the winners were Inconclusive lotteries
    // missing single cells. The eighth family states the
    // shape directly: joint falsify-every-leaf over the
    // Int span crossed with the Bool domain, rows
    // contributing regions, literal pins only.

    /// The nested-match body rust_stmt_seq_form's table
    /// takes: `match position { 1 => match last { true =>
    /// 1, _ => 0 }, _ => 0 }` — the (1, true) cell armed
    /// through the inner match, every other cell on the
    /// wildcards, which carry the pinned fall-through
    /// value 0.
    fn mixed_seq_form_body() -> IRNode {
        IRNode::Match {
            scrutinee: Box::new(IRNode::Var("position".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(1)),
                    IRNode::Match {
                        scrutinee: Box::new(IRNode::Var("last".into())),
                        cases: vec![
                            (
                                IRPattern::Literal(IRLiteral::Bool(true)),
                                IRNode::Literal(IRLiteral::Int(1)),
                            ),
                            (
                                IRPattern::Wildcard,
                                IRNode::Literal(IRLiteral::Int(0)),
                            ),
                        ],
                    },
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Int(0)),
                ),
            ],
        }
    }

    /// Mixed pair-keyed guard table (i): rust_stmt_seq_
    /// form's rows pin 1 exactly at (position 1, last
    /// true) and 0 everywhere else — a total (Int, Bool)
    /// table. The seed must exist (kind
    /// `mixed_pair_guard_table`) as the spec's only
    /// constraint seed, be the nested match, execute to
    /// the spec's rule over the cross-validation sweep,
    /// be Verified in module context, and win module-aware
    /// synthesis deterministically.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_mixed_pair_guard_table_seed_for_seq_form() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_stmt_seq_form");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the mixed pair-keyed seed");
        let seed = constraint_seed(&engine, &spec, "mixed_pair_guard_table");
        assert_eq!(count_holes(&seed.body), 0);
        assert_eq!(
            format!("{:?}", seed.body),
            format!("{:?}", mixed_seq_form_body()),
            "the seed must be the nested mixed table"
        );

        // Executed over the whole sweep the
        // cross-validation harness uses: positions -2..=3
        // crossed with the full Bool domain (12 cases).
        for position in -2..=3 {
            for last in [false, true] {
                let v = eval_seed_body(
                    &seed.body,
                    &[
                        ("position", crate::synthesis::eval::Value::Int(position)),
                        ("last", crate::synthesis::eval::Value::Bool(last)),
                    ],
                );
                assert_eq!(
                    v,
                    crate::synthesis::eval::Value::Int(if position == 1 && last {
                        1
                    } else {
                        0
                    }),
                    "({position}, {last}) must follow the spec's table"
                );
            }
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        // Deterministic across draws: the same winning
        // body in three independent synthesis runs.
        for _ in 0..3 {
            let pool = synthesize_with_module(&spec, 1, &module)
                .expect("opt-1 synthesis must succeed");
            let verified = verifier.verify_all(pool, &spec).expect("verify_all");
            let winner =
                crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
            assert!(winner.verified, "the mixed pair-keyed seed must win verified");
            assert_eq!(
                format!("{:?}", winner.body),
                format!("{:?}", seed.body),
                "the winning body must be the mixed pair-keyed seed in every draw"
            );
        }
    }

    /// Mixed pair-keyed guard table (ii): synthetic
    /// shapes — a BAND row on the Int key pinning its
    /// whole band across the Bool split (region recovery
    /// working through the mixed grid), and a PARTIAL
    /// table: no row decides the position <= 0 columns, so
    /// they fall to the wildcard, which carries the
    /// pinned fall-through value (the band's 7, from the
    /// tail column) rather than the typed default.
    #[test]
    fn constraint_mixed_pair_guard_table_seed_synthetic_shapes() {
        // (n >= 2, *) -> 7 by the band row; (1, true) -> 5
        // and (1, false) -> 3 by the two mixed cell rows;
        // (n <= 0, *) undecided — the wildcard's guess.
        let spec = guard_spec(
            "mixed_band_partial",
            vec![("n", TypeRef::Int), ("flag", TypeRef::Bool)],
            TypeRef::Int,
            vec![
                pair_row(vec![cmp_leaf("n", BinOp::Lt, 2)], Expr::IntLit(7)),
                pair_row(
                    vec![
                        cmp_leaf("n", BinOp::Lt, 1),
                        cmp_leaf("n", BinOp::Gt, 1),
                        eq_expr(var_expr("flag"), Expr::BoolLit(false)),
                    ],
                    Expr::IntLit(5),
                ),
                pair_row(
                    vec![
                        cmp_leaf("n", BinOp::Lt, 1),
                        cmp_leaf("n", BinOp::Gt, 1),
                        eq_expr(var_expr("flag"), Expr::BoolLit(true)),
                    ],
                    Expr::IntLit(3),
                ),
            ],
        );
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the mixed pair-keyed seed");
        let seed = constraint_seed(&engine, &spec, "mixed_pair_guard_table");
        for (n, flag, want) in [
            (3, true, 7),
            (2, false, 7),
            (9, true, 7),
            (1, true, 5),
            (1, false, 3),
            (0, true, 7), // unpinned: the wildcard's documented guess
            (0, false, 7),
            (-4, false, 7),
        ] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("n", crate::synthesis::eval::Value::Int(n)),
                    ("flag", crate::synthesis::eval::Value::Bool(flag)),
                ],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Int(want),
                "({n}, {flag}) must follow the table (band + cells; <= 0 unpinned)"
            );
        }

        // The key signature is by TYPE, not position: the
        // same table with the Bool parameter first still
        // takes the nested match over the Int key.
        let spec = guard_spec(
            "mixed_reversed_signature",
            vec![("flag", TypeRef::Bool), ("n", TypeRef::Int)],
            TypeRef::Int,
            vec![
                pair_row(
                    vec![
                        cmp_leaf("n", BinOp::Lt, 1),
                        cmp_leaf("n", BinOp::Gt, 1),
                        eq_expr(var_expr("flag"), Expr::BoolLit(false)),
                    ],
                    Expr::IntLit(1),
                ),
                pair_row(
                    vec![eq_expr(var_expr("n"), Expr::IntLit(1))],
                    Expr::IntLit(0),
                ),
                pair_row(
                    vec![eq_expr(var_expr("flag"), Expr::BoolLit(true))],
                    Expr::IntLit(0),
                ),
            ],
        );
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the mixed pair-keyed seed");
        let seed = constraint_seed(&engine, &spec, "mixed_pair_guard_table");
        match &seed.body {
            IRNode::Match { scrutinee, .. } => {
                assert_eq!(
                    format!("{:?}", scrutinee),
                    format!("{:?}", IRNode::Var("n".into())),
                    "the outer match is over the Int key regardless of signature order"
                );
            }
            other => panic!("the seed must be a match, got {other:?}"),
        }
        for (n, flag, want) in [(1, true, 1), (1, false, 0), (0, true, 0), (5, false, 0)] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("flag", crate::synthesis::eval::Value::Bool(flag)),
                    ("n", crate::synthesis::eval::Value::Int(n)),
                ],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Int(want),
                "({n}, {flag}) must follow the table"
            );
        }
    }

    /// Mixed pair-keyed guard table (iii): near misses.
    /// Each shape is refused on a stated predicate — the
    /// key signature (two Int keys are the pair family's,
    /// two Bool keys the multi-guard family's), the
    /// two-key requirement (one-key row sets belong to the
    /// earlier families), the pin test (a compound pin is
    /// the expression-pin family's shape, spanning one
    /// key only; a pin converting through a Hole is not a
    /// literal), joint consistency, and arity (a third
    /// parameter, or a leaf on a parameter that is not a
    /// key, is outside the family).
    #[test]
    fn constraint_mixed_pair_guard_table_near_misses() {
        let plain = || SynthesisEngine::new(SynthesisConfig::default());

        // Shapes NO family states: the whole seed set is
        // empty.
        let mut empty_cases: Vec<FunctionSpec> = Vec::new();

        // Rows over the Int key only, at (Int, Bool)
        // arity: the guard table's arity is one and this
        // family requires both keys.
        empty_cases.push(guard_spec(
            "mixed_int_key_only",
            vec![("n", TypeRef::Int), ("flag", TypeRef::Bool)],
            TypeRef::Int,
            vec![
                int_key_row("n", 0, Expr::IntLit(1)),
                int_key_row("n", 1, Expr::IntLit(2)),
            ],
        ));

        // Mixed rows with a compound pin: the
        // expression-pin family's pin shape, but its key
        // discipline is one key — no family states this.
        empty_cases.push(guard_spec(
            "mixed_compound_pin",
            vec![("n", TypeRef::Int), ("flag", TypeRef::Bool)],
            TypeRef::String,
            vec![pair_row(
                vec![
                    cmp_leaf("n", BinOp::Lt, 1),
                    cmp_leaf("n", BinOp::Gt, 1),
                    eq_expr(var_expr("flag"), Expr::BoolLit(false)),
                ],
                concat_pin(var_expr("n"), "!"),
            )],
        ));

        // Two rows pinning one cell to different values:
        // (1, true) is 5 by the cell row and 6 by the
        // flag-only row — jointly inconsistent, not a
        // table.
        empty_cases.push(guard_spec(
            "mixed_conflict",
            vec![("n", TypeRef::Int), ("flag", TypeRef::Bool)],
            TypeRef::Int,
            vec![
                pair_row(
                    vec![
                        cmp_leaf("n", BinOp::Lt, 1),
                        cmp_leaf("n", BinOp::Gt, 1),
                        eq_expr(var_expr("flag"), Expr::BoolLit(false)),
                    ],
                    Expr::IntLit(5),
                ),
                pair_row(
                    vec![eq_expr(var_expr("flag"), Expr::BoolLit(false))],
                    Expr::IntLit(6),
                ),
            ],
        ));

        // A pin converting through a Hole (a Length has
        // no literal IR form): not a literal pin.
        empty_cases.push(guard_spec(
            "mixed_hole_pin",
            vec![("n", TypeRef::Int), ("flag", TypeRef::Bool)],
            TypeRef::Int,
            vec![pair_row(
                vec![
                    cmp_leaf("n", BinOp::Lt, 1),
                    cmp_leaf("n", BinOp::Gt, 1),
                    eq_expr(var_expr("flag"), Expr::BoolLit(false)),
                ],
                Expr::Length(Box::new(var_expr("n"))),
            )],
        ));

        for spec in &empty_cases {
            assert!(
                plain().seed_from_constraints(spec).is_empty(),
                "{}: near-miss shape must get no constraint seed",
                spec.name
            );
            assert!(
                mixed_pair_guard_table_body(spec).is_none(),
                "{}: the mixed builder itself must refuse it",
                spec.name
            );
        }

        // Shapes another family rightfully claims, or
        // that fall outside the signature: the MIXED
        // builder refuses them (its own predicate), which
        // is the disjointness statement.
        let mut refused_cases: Vec<FunctionSpec> = Vec::new();

        // Two Int keys: the pair family's signature.
        refused_cases.push(guard_spec(
            "mixed_two_int_keys",
            vec![("a", TypeRef::Int), ("b", TypeRef::Int)],
            TypeRef::Int,
            vec![pair_row(
                vec![
                    cmp_leaf("a", BinOp::Lt, 1),
                    cmp_leaf("b", BinOp::Lt, 2),
                ],
                Expr::IntLit(1),
            )],
        ));

        // Two Bool keys: the multi-guard family's
        // signature.
        refused_cases.push(guard_spec(
            "mixed_two_bool_keys",
            vec![("f", TypeRef::Bool), ("g", TypeRef::Bool)],
            TypeRef::Int,
            vec![pair_row(
                vec![
                    eq_expr(var_expr("f"), Expr::BoolLit(false)),
                    eq_expr(var_expr("g"), Expr::BoolLit(true)),
                ],
                Expr::IntLit(1),
            )],
        ));

        // Rows over the Bool key only: an earlier
        // family's shape (the composition family's
        // precedence territory), refused here on the
        // two-key requirement.
        refused_cases.push(guard_spec(
            "mixed_bool_key_only",
            vec![("n", TypeRef::Int), ("flag", TypeRef::Bool)],
            TypeRef::Int,
            vec![
                pair_row(
                    vec![eq_expr(var_expr("flag"), Expr::BoolLit(false))],
                    Expr::IntLit(1),
                ),
                pair_row(
                    vec![eq_expr(var_expr("flag"), Expr::BoolLit(true))],
                    Expr::IntLit(0),
                ),
            ],
        ));

        // Three parameters: beyond the pair, whatever
        // the rows range over.
        refused_cases.push(guard_spec(
            "mixed_three_params",
            vec![
                ("n", TypeRef::Int),
                ("flag", TypeRef::Bool),
                ("m", TypeRef::Int),
            ],
            TypeRef::Int,
            vec![pair_row(
                vec![
                    cmp_leaf("n", BinOp::Lt, 1),
                    eq_expr(var_expr("flag"), Expr::BoolLit(false)),
                ],
                Expr::IntLit(1),
            )],
        ));

        // A guard leaf on a variable that is not a key:
        // a deviation, not a skippable leaf.
        refused_cases.push(guard_spec(
            "mixed_leaf_on_third_var",
            vec![("n", TypeRef::Int), ("flag", TypeRef::Bool)],
            TypeRef::Int,
            vec![pair_row(
                vec![
                    cmp_leaf("n", BinOp::Lt, 1),
                    cmp_leaf("other", BinOp::Lt, 2),
                    eq_expr(var_expr("flag"), Expr::BoolLit(false)),
                ],
                Expr::IntLit(1),
            )],
        ));

        for spec in &refused_cases {
            assert!(
                mixed_pair_guard_table_body(spec).is_none(),
                "{}: the mixed builder must refuse this key discipline",
                spec.name
            );
        }
    }

    // ── Phase 34: multi-guard truth tables + the ────────
    // composition precedence amendment ──────────────────
    //
    // The last two seedless Codegen specs, on their Phase
    // 31 predicates:
    //
    // (ii) rust_match_scrutinee_form's rows guard on TWO
    // different Bool expressions (has_string_pat, bare),
    // where composition_guard_row requires ONE shared
    // guard. The multi-guard family lifts exactly that
    // requirement: rows may constrain any subset of a SET
    // of shared Bool expressions, and the pinned cell of
    // the joint {false, true}^n table is recovered by
    // falsify-every-leaf over the rows — the pair-keyed
    // family's joint falsification on the Bool domain.
    // Territory by construction: fewer than two distinct
    // guard expressions is the composition family's shape
    // (its bodies stay byte-identical), more than
    // MAX_MULTI_GUARD_EXPRESSIONS is refused — the table
    // doubles per guard.
    //
    // (iii) rust_concat_left_text's rows pin bare variables
    // under a plain Bool-parameter guard at arity three:
    // composition_guard_body's territory rule refused the
    // shape (bare-variable pins are guarded identity's),
    // but guarded_identity_body caps at arity two and the
    // guard table at literal pins, so the shape sat
    // seedless BETWEEN the families. The amendment restates
    // the refusal as a PRECEDENCE: the composition builder
    // claims a plain-variable guard with all literal /
    // bare-variable pins exactly when NEITHER earlier
    // family admits the spec — right of first refusal, so
    // disjointness holds by construction — and a
    // bare-variable pin names a parameter of the return
    // type (guarded identity's own condition on its
    // identity parameter: the only way the pin can BE the
    // output).

    /// Multi-guard truth tables (i): rust_match_scrutinee_
    /// form's rows pin output 1 exactly at (has_string_pat,
    /// bare) == (true, false) and 0 on the other three
    /// cells, guarding on the two Bool parameters jointly —
    /// row one constrains both, rows two and three one
    /// each. The seed must exist (kind `multi_guard`) as
    /// the spec's only constraint seed, be the nested match
    /// over the two guards in first-appearance order,
    /// execute the whole Bool x Bool domain, be Verified in
    /// module context, and win module-aware synthesis
    /// deterministically.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_multi_guard_seed_for_scrutinee_form() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_match_scrutinee_form");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the multi-guard seed");
        let seed = constraint_seed(&engine, &spec, "multi_guard");
        assert_eq!(count_holes(&seed.body), 0);

        // The (false, *) column is constant 0 and renders
        // directly; the (true, *) column splits on bare.
        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("has_string_pat".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Bool(false)),
                    IRNode::Literal(IRLiteral::Int(0)),
                ),
                (
                    IRPattern::Literal(IRLiteral::Bool(true)),
                    IRNode::Match {
                        scrutinee: Box::new(IRNode::Var("bare".into())),
                        cases: vec![
                            (
                                IRPattern::Literal(IRLiteral::Bool(false)),
                                IRNode::Literal(IRLiteral::Int(1)),
                            ),
                            (
                                IRPattern::Literal(IRLiteral::Bool(true)),
                                IRNode::Literal(IRLiteral::Int(0)),
                            ),
                            (
                                IRPattern::Wildcard,
                                IRNode::Literal(IRLiteral::Int(0)),
                            ),
                        ],
                    },
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::Int(0)),
                ),
            ],
        };
        assert_eq!(
            format!("{:?}", seed.body),
            format!("{:?}", expected),
            "the seed must be the nested truth-table match"
        );
        // No call to resolve: the resolver-visible body is
        // the same nested match.
        let index = SpecIndex::from_specs(&module);
        let raw = index
            .callee_body("rust_match_scrutinee_form")
            .expect("the multi-guard body must be resolver-visible");
        assert_eq!(format!("{:?}", raw), format!("{:?}", expected));

        // Executed over the complete Bool x Bool domain —
        // the cross-validation sweep's four pinned cases.
        for (hsp, bare, want) in [
            (false, false, 0),
            (false, true, 0),
            (true, false, 1),
            (true, true, 0),
        ] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("has_string_pat", crate::synthesis::eval::Value::Bool(hsp)),
                    ("bare", crate::synthesis::eval::Value::Bool(bare)),
                ],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Int(want),
                "({hsp}, {bare}) must follow the spec's truth table"
            );
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        for _ in 0..3 {
            let pool = synthesize_with_module(&spec, 1, &module)
                .expect("opt-1 synthesis must succeed");
            let verified = verifier.verify_all(pool, &spec).expect("verify_all");
            let winner =
                crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
            assert!(winner.verified, "the multi-guard seed must win verified");
            // No tpr assertion: the spec has no in-spec
            // tests, so the win rides on the verdict (the
            // generics_text precedent).
            assert_eq!(
                format!("{:?}", winner.body),
                format!("{:?}", seed.body),
                "the winning body must be the multi-guard seed in every draw"
            );
        }
    }

    /// Multi-guard truth tables (ii): a guard set may mix a
    /// CALL guard with a Bool parameter — each guard is
    /// validated the way the composition family validates
    /// its scrutinee, the raw body matches over the call
    /// itself, and the admitted seed has the call resolved
    /// (the composition seed's admission precedent).
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_multi_guard_seed_with_call_guard() {
        let is_high = guard_spec(
            "is_high",
            vec![("n", TypeRef::Int)],
            TypeRef::Bool,
            vec![Constraint::Eq(
                var_expr("output"),
                Expr::BinOp(
                    BinOp::Gt,
                    Box::new(var_expr("n")),
                    Box::new(Expr::IntLit(10)),
                ),
            )],
        );
        // (call, flag): 1 exactly at (true, false), else 0 —
        // row one constrains both guards, the other rows one
        // each, as in rust_match_scrutinee_form.
        let caller = guard_spec(
            "multi_call_guard",
            vec![("n", TypeRef::Int), ("flag", TypeRef::Bool)],
            TypeRef::Int,
            vec![
                or_row(vec![
                    eq_expr(
                        Expr::Call("is_high".into(), vec![var_expr("n")]),
                        Expr::BoolLit(false),
                    ),
                    eq_expr(var_expr("flag"), Expr::BoolLit(true)),
                    eq_expr(var_expr("output"), Expr::IntLit(1)),
                ]),
                or_row(vec![
                    eq_expr(
                        Expr::Call("is_high".into(), vec![var_expr("n")]),
                        Expr::BoolLit(true),
                    ),
                    eq_expr(var_expr("output"), Expr::IntLit(0)),
                ]),
                or_row(vec![
                    eq_expr(var_expr("flag"), Expr::BoolLit(false)),
                    eq_expr(var_expr("output"), Expr::IntLit(0)),
                ]),
            ],
        );
        let module = vec![is_high, caller.clone()];
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&caller);
        assert_eq!(seeds.len(), 1, "exactly the multi-guard seed");
        let seed = constraint_seed(&engine, &caller, "multi_guard");
        assert_eq!(count_holes(&seed.body), 0);

        // The resolver-visible body matches over the call
        // itself; the admitted seed's scrutinee is the
        // resolved callee body, so the interpreter can
        // execute it.
        let index = SpecIndex::from_specs(&module);
        let raw = index
            .callee_body("multi_call_guard")
            .expect("the multi-guard body must be resolver-visible");
        let IRNode::Match { scrutinee, .. } = &raw else {
            panic!("raw body must be a match: {:?}", raw);
        };
        assert!(
            matches!(**scrutinee, IRNode::Call { .. }),
            "raw scrutinee must be the call itself: {:?}",
            scrutinee
        );
        let IRNode::Match { scrutinee, .. } = &seed.body else {
            panic!("seed body must be a match: {:?}", seed.body);
        };
        assert!(
            !matches!(**scrutinee, IRNode::Call { .. }),
            "seed scrutinee must be resolved, not a call: {:?}",
            scrutinee
        );

        for (n, flag, want) in [(20, false, 1), (20, true, 0), (5, false, 0), (5, true, 0)] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("n", crate::synthesis::eval::Value::Int(n)),
                    ("flag", crate::synthesis::eval::Value::Bool(flag)),
                ],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Int(want),
                "(n={n}, flag={flag}) must follow the truth table"
            );
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &caller);
        assert_eq!(result.status, VerificationStatus::Verified);
    }

    /// Multi-guard truth tables (negatives): near-miss
    /// shapes still get NO constraint seed. Each case
    /// deviates in exactly one way.
    #[test]
    fn constraint_multi_guard_near_misses_get_no_seed() {
        let plain = || SynthesisEngine::new(SynthesisConfig::default());
        let mut cases: Vec<FunctionSpec> = Vec::new();

        // Five distinct guard expressions: past the cap —
        // the truth table doubles per guard.
        cases.push(guard_spec(
            "multi_over_cap",
            vec![
                ("f1", TypeRef::Bool),
                ("f2", TypeRef::Bool),
                ("f3", TypeRef::Bool),
                ("f4", TypeRef::Bool),
                ("f5", TypeRef::Bool),
            ],
            TypeRef::Int,
            vec![or_row(vec![
                eq_expr(var_expr("f1"), Expr::BoolLit(false)),
                eq_expr(var_expr("f2"), Expr::BoolLit(false)),
                eq_expr(var_expr("f3"), Expr::BoolLit(false)),
                eq_expr(var_expr("f4"), Expr::BoolLit(false)),
                eq_expr(var_expr("f5"), Expr::BoolLit(false)),
                eq_expr(var_expr("output"), Expr::IntLit(1)),
            ])],
        ));

        // Two rows jointly pin the (true, true) cell to
        // different values: not a truth table.
        cases.push(guard_spec(
            "multi_conflicting_cell",
            vec![("f", TypeRef::Bool), ("g", TypeRef::Bool)],
            TypeRef::Int,
            vec![
                or_row(vec![
                    eq_expr(var_expr("f"), Expr::BoolLit(false)),
                    eq_expr(var_expr("output"), Expr::IntLit(1)),
                ]),
                or_row(vec![
                    eq_expr(var_expr("g"), Expr::BoolLit(false)),
                    eq_expr(var_expr("output"), Expr::IntLit(2)),
                ]),
            ],
        ));

        // A pin mentioning `output` itself.
        cases.push(guard_spec(
            "multi_pin_mentions_output",
            vec![("f", TypeRef::Bool), ("g", TypeRef::Bool)],
            TypeRef::String,
            vec![
                or_row(vec![
                    eq_expr(var_expr("f"), Expr::BoolLit(false)),
                    eq_expr(
                        var_expr("output"),
                        Expr::BinOp(
                            BinOp::Concat,
                            Box::new(var_expr("output")),
                            Box::new(Expr::StringLit("!".into())),
                        ),
                    ),
                ]),
                or_row(vec![
                    eq_expr(var_expr("g"), Expr::BoolLit(false)),
                    eq_expr(var_expr("output"), Expr::StringLit("x".into())),
                ]),
            ],
        ));

        // A pin converting through a Hole (a Length has no
        // IR form): not an admissible pin.
        cases.push(guard_spec(
            "multi_hole_pin",
            vec![
                ("f", TypeRef::Bool),
                ("g", TypeRef::Bool),
                ("a", TypeRef::String),
            ],
            TypeRef::Int,
            vec![
                or_row(vec![
                    eq_expr(var_expr("f"), Expr::BoolLit(false)),
                    eq_expr(
                        var_expr("output"),
                        Expr::Length(Box::new(var_expr("a"))),
                    ),
                ]),
                or_row(vec![
                    eq_expr(var_expr("g"), Expr::BoolLit(false)),
                    eq_expr(var_expr("output"), Expr::IntLit(0)),
                ]),
            ],
        ));

        // A pin calling a spec the index does not hold.
        cases.push(guard_spec(
            "multi_pin_calls_unindexed",
            vec![
                ("f", TypeRef::Bool),
                ("g", TypeRef::Bool),
                ("a", TypeRef::String),
            ],
            TypeRef::String,
            vec![
                or_row(vec![
                    eq_expr(var_expr("f"), Expr::BoolLit(false)),
                    eq_expr(
                        var_expr("output"),
                        Expr::Call("ghost".into(), vec![var_expr("a")]),
                    ),
                ]),
                or_row(vec![
                    eq_expr(var_expr("g"), Expr::BoolLit(false)),
                    eq_expr(var_expr("output"), var_expr("a")),
                ]),
            ],
        ));

        // A row guarded by an Int comparison among the Bool
        // rows: the Int leaves stay with the Int families —
        // a mixed row set is no family's shape.
        cases.push(guard_spec(
            "multi_mixed_int_row",
            vec![
                ("f", TypeRef::Bool),
                ("g", TypeRef::Bool),
                ("code", TypeRef::Int),
            ],
            TypeRef::Int,
            vec![
                or_row(vec![
                    eq_expr(var_expr("f"), Expr::BoolLit(false)),
                    eq_expr(var_expr("g"), Expr::BoolLit(false)),
                    eq_expr(var_expr("output"), Expr::IntLit(1)),
                ]),
                or_row(vec![
                    Expr::BinOp(
                        BinOp::Lt,
                        Box::new(var_expr("code")),
                        Box::new(Expr::IntLit(0)),
                    ),
                    eq_expr(var_expr("output"), Expr::IntLit(0)),
                ]),
            ],
        ));

        for spec in &cases {
            assert!(
                plain().seed_from_constraints(spec).is_empty(),
                "{}: near-miss shape must get no constraint seed",
                spec.name
            );
        }
    }

    /// The precedence amendment (i): rust_concat_left_
    /// text's rows pin the two String parameters under the
    /// Bool parameter `owned` — owned_text when owned,
    /// rendered when not. Guarded identity cannot state it
    /// (arity three), the guard table cannot state it
    /// (bare-variable pins), and the composition builder's
    /// old territory rule refused it for exactly that
    /// combination. Under the precedence it is the
    /// composition family's: the seed must exist (kind
    /// `composition_guard`) as the spec's only constraint
    /// seed, be the match over `owned`, execute the pinned
    /// domain exactly, be Verified in module context, and
    /// win module-aware synthesis deterministically.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_composition_precedence_seed_for_left_text() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_concat_left_text");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the composition-guard seed");
        let seed = constraint_seed(&engine, &spec, "composition_guard");
        assert_eq!(count_holes(&seed.body), 0);

        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("owned".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Bool(true)),
                    IRNode::Var("owned_text".into()),
                ),
                (
                    IRPattern::Literal(IRLiteral::Bool(false)),
                    IRNode::Var("rendered".into()),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Literal(IRLiteral::String(String::new())),
                ),
            ],
        };
        assert_eq!(
            format!("{:?}", seed.body),
            format!("{:?}", expected),
            "the seed must be the match over `owned`"
        );

        // The cross-validation battery's four pinned cases.
        for (owned, rendered, owned_text, want) in [
            (true, "x", "\"a\"", "\"a\""),
            (true, "(a + b)", "\"q\"", "\"q\""),
            (false, "x", "\"a\"", "x"),
            (false, "\"lit\"", "ignored", "\"lit\""),
        ] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("owned", crate::synthesis::eval::Value::Bool(owned)),
                    ("rendered", crate::synthesis::eval::Value::Str(rendered.into())),
                    (
                        "owned_text",
                        crate::synthesis::eval::Value::Str(owned_text.into()),
                    ),
                ],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Str(want.into()),
                "owned={owned} rendered={rendered:?} owned_text={owned_text:?}"
            );
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        for _ in 0..3 {
            let pool = synthesize_with_module(&spec, 1, &module)
                .expect("opt-1 synthesis must succeed");
            let verified = verifier.verify_all(pool, &spec).expect("verify_all");
            let winner =
                crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
            assert!(winner.verified, "the precedence seed must win verified");
            // No tpr assertion: the spec has no in-spec
            // tests, so the win rides on the verdict (the
            // generics_text precedent).
            assert_eq!(
                format!("{:?}", winner.body),
                format!("{:?}", seed.body),
                "the winning body must be the composition-guard seed in every draw"
            );
        }
    }

    /// The precedence amendment (ii): both directions on
    /// synthetic shapes.
    ///
    /// Direction one — an arity-two guarded-identity shape
    /// still yields ONLY the guarded-identity seed: the
    /// earlier family claims first and the composition
    /// builder still refuses (also pinned on the real spec
    /// by constraint_guarded_identity_seed_for_ctor_name,
    /// which asserts exactly one seed).
    ///
    /// Direction two — the shapes the precedence newly
    /// claims, each refused by both earlier families on its
    /// own predicate. Three of them stood in
    /// constraint_seed_near_misses_get_no_seed until this
    /// phase, recorded there against GUARDED IDENTITY's
    /// predicate (a literal row deviates; the pin is the
    /// guard parameter, not the remaining one; three
    /// parameters exceed its arity). Those refusals stand —
    /// guarded identity still admits none of them. What
    /// changes is only that the composition family, gated
    /// by the earlier families' refusal, can state them,
    /// and each body below honors its rows exactly.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_composition_precedence_both_directions() {
        let plain = || SynthesisEngine::new(SynthesisConfig::default());

        // Direction two, case A: a literal row beside an
        // identity row (guarded identity deviates on the
        // literal row; the guard table's arity is one).
        let mixed = guard_spec(
            "precedence_mixed_literal_row",
            vec![("name", TypeRef::String), ("has_subs", TypeRef::Bool)],
            TypeRef::String,
            vec![
                or_row(vec![
                    eq_expr(var_expr("has_subs"), Expr::BoolLit(true)),
                    eq_expr(var_expr("output"), var_expr("name")),
                ]),
                or_row(vec![
                    eq_expr(var_expr("has_subs"), Expr::BoolLit(false)),
                    eq_expr(var_expr("output"), Expr::StringLit("bare".into())),
                ]),
            ],
        );
        let seeds = plain().seed_from_constraints(&mixed);
        assert_eq!(seeds.len(), 1, "exactly the composition-guard seed");
        let seed = constraint_seed(&plain(), &mixed, "composition_guard");
        for (has_subs, want) in [(false, "ctor"), (true, "bare")] {
            let v = eval_seed_body(
                &seed.body,
                &[
                    ("name", crate::synthesis::eval::Value::Str("ctor".into())),
                    ("has_subs", crate::synthesis::eval::Value::Bool(has_subs)),
                ],
            );
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Str(want.into()),
                "has_subs={has_subs} must follow the rows"
            );
        }
        let result = crate::verify::verifier::Verifier::new().verify_one(&seed, &mixed);
        assert_eq!(result.status, VerificationStatus::Verified);

        // Direction two, case B: the pin is the guard
        // parameter itself (guarded identity requires the
        // REMAINING parameter; here the pin's type is the
        // return type, so the pin can be the output).
        let self_pin = guard_spec(
            "precedence_pin_is_guard_param",
            vec![("flag", TypeRef::Bool), ("name", TypeRef::String)],
            TypeRef::Bool,
            vec![or_row(vec![
                eq_expr(var_expr("flag"), Expr::BoolLit(true)),
                eq_expr(var_expr("output"), var_expr("flag")),
            ])],
        );
        let seeds = plain().seed_from_constraints(&self_pin);
        assert_eq!(seeds.len(), 1, "exactly the composition-guard seed");
        let seed = constraint_seed(&plain(), &self_pin, "composition_guard");
        let v = eval_seed_body(
            &seed.body,
            &[
                ("flag", crate::synthesis::eval::Value::Bool(false)),
                ("name", crate::synthesis::eval::Value::Str("n".into())),
            ],
        );
        assert_eq!(v, crate::synthesis::eval::Value::Bool(false));
        let result = crate::verify::verifier::Verifier::new().verify_one(&seed, &self_pin);
        assert_eq!(result.status, VerificationStatus::Verified);

        // Direction two, case C: rust_concat_left_text's
        // shape in miniature — a bare-variable pin at arity
        // three (guarded identity's arity cap is the only
        // earlier refusal left standing).
        let extra = guard_spec(
            "precedence_two_extra_params",
            vec![
                ("a", TypeRef::String),
                ("b", TypeRef::String),
                ("flag", TypeRef::Bool),
            ],
            TypeRef::String,
            vec![or_row(vec![
                eq_expr(var_expr("flag"), Expr::BoolLit(true)),
                eq_expr(var_expr("output"), var_expr("a")),
            ])],
        );
        let seeds = plain().seed_from_constraints(&extra);
        assert_eq!(seeds.len(), 1, "exactly the composition-guard seed");
        let seed = constraint_seed(&plain(), &extra, "composition_guard");
        let v = eval_seed_body(
            &seed.body,
            &[
                ("a", crate::synthesis::eval::Value::Str("A".into())),
                ("b", crate::synthesis::eval::Value::Str("B".into())),
                ("flag", crate::synthesis::eval::Value::Bool(false)),
            ],
        );
        assert_eq!(v, crate::synthesis::eval::Value::Str("A".into()));
        let result = crate::verify::verifier::Verifier::new().verify_one(&seed, &extra);
        assert_eq!(result.status, VerificationStatus::Verified);
    }

    /// replace_hole must actually substitute the hole wherever
    /// find_hole_positions found it. The path walker behind it
    /// used to implement only the Block step — every other
    /// PathElem fell into a no-op arm — so a hole inside a match
    /// arm (the TestInduced seed's wildcard), an If branch, or a
    /// Let body was silently left in place: "expanded" MCTS
    /// children were clones of their parent, still holed, never
    /// terminal, never harvested, and rollouts could not fill
    /// such holes either.
    #[test]
    fn replace_hole_fills_holes_beyond_block_paths() {
        let filling = IRNode::Literal(IRLiteral::Bool(false));
        let hole = || IRNode::Hole { id: 7, expected_type: None };

        // Path [MatchCase(1)] — the seed's wildcard arm.
        let mut body = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(0)),
                    IRNode::Literal(IRLiteral::Bool(true)),
                ),
                (IRPattern::Wildcard, hole()),
            ],
        };
        let holes = find_hole_positions(&body);
        assert_eq!(holes.len(), 1);
        replace_hole(&mut body, &holes[0], &filling);
        assert_eq!(count_holes(&body), 0, "match-arm hole left: {:?}", body);

        // Path [IfElse].
        let mut body = IRNode::If {
            condition: Box::new(IRNode::Literal(IRLiteral::Bool(true))),
            then_branch: Box::new(IRNode::Literal(IRLiteral::Bool(true))),
            else_branch: Box::new(hole()),
        };
        let holes = find_hole_positions(&body);
        assert_eq!(holes.len(), 1);
        replace_hole(&mut body, &holes[0], &filling);
        assert_eq!(count_holes(&body), 0, "if-else hole left: {:?}", body);

        // Path [LetBody].
        let mut body = IRNode::Let {
            name: "x".into(),
            value: Box::new(IRNode::Literal(IRLiteral::Int(1))),
            body: Box::new(hole()),
        };
        let holes = find_hole_positions(&body);
        assert_eq!(holes.len(), 1);
        replace_hole(&mut body, &holes[0], &filling);
        assert_eq!(count_holes(&body), 0, "let-body hole left: {:?}", body);

        // Nested path [MatchCase(0), BinOpR]: only the addressed
        // hole is replaced; a second hole elsewhere survives.
        let mut body = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![
                (
                    IRPattern::Wildcard,
                    IRNode::BinOp(
                        IRBinOp::Add,
                        Box::new(IRNode::Literal(IRLiteral::Int(1))),
                        Box::new(hole()),
                    ),
                ),
                (IRPattern::Literal(IRLiteral::Int(0)), hole()),
            ],
        };
        let holes = find_hole_positions(&body);
        assert_eq!(holes.len(), 2);
        replace_hole(&mut body, &holes[0], &IRNode::Literal(IRLiteral::Int(9)));
        assert_eq!(count_holes(&body), 1, "exactly one hole must remain: {:?}", body);
    }

    /// Seed-root harvest, pipeline level: the final pool contains
    /// a HOLE-FREE completion of the TestInduced seed — the seed's
    /// own match shape (one literal arm per test, wildcard arm
    /// last) with the wildcard hole filled — measuring the full
    /// 1.0 test pass ratio with no hole decay. The spec's tests
    /// only reach the literal arms, so ANY filling of the wildcard
    /// completes a body that passes every test. Before the
    /// seed-rooted MCTS pass existed, the single MCTS tree was
    /// rooted at one random population member, so seed completions
    /// reached the pool only when the seed itself happened to be
    /// picked as that root — the holed original (tpr 0.5, see the
    /// test above) was otherwise the seed's only presence.
    #[test]
    fn synthesized_pool_contains_hole_free_seed_completion() {
        let spec = is_literal_kind_spec();
        let pool = synthesize(&spec, 1).expect("opt-1 synthesis must succeed");
        let completion = pool.iter().find(|c| {
            if count_holes(&c.body) != 0
                || (c.scores.test_pass_ratio - 1.0).abs() >= 1e-9
            {
                return false;
            }
            // The seed's shape: a match whose leading arms are the
            // per-test literal arms and whose last arm is the
            // (now filled) wildcard.
            match &c.body {
                IRNode::Match { cases, .. } => {
                    cases.len() == spec.tests.len() + 1
                        && cases[..spec.tests.len()]
                            .iter()
                            .all(|(p, _)| matches!(p, IRPattern::Literal(_)))
                        && matches!(cases[spec.tests.len()].0, IRPattern::Wildcard)
                }
                _ => false,
            }
        });
        assert!(
            completion.is_some(),
            "pool must contain a hole-free completion of the test-induced seed; \
             pool (holes, tpr): {:?}",
            pool.iter()
                .map(|c| (count_holes(&c.body), c.scores.test_pass_ratio))
                .collect::<Vec<_>>()
        );
    }

    /// F4 end-to-end (Phase 45): a Char classifier stated
    /// with char literals — range constraint plus char-literal
    /// tests — goes through the real build path
    /// (synthesize_with_module → verify_all → select_best,
    /// the main.rs sequence). Two measured facts, both
    /// asserted:
    ///
    /// 1. The crowned winner is hole-free, passes every test
    ///    (tpr 1.000), and agrees with the hand mirror on
    ///    every TESTED char — char literals flow through
    ///    parse → synthesis → test execution exactly.
    /// 2. The pool also contains the SPEC-EXACT completion
    ///    of the test-induced seed (one Char arm per test,
    ///    wildcard filled with `false`), and THAT body,
    ///    executed on the IR interpreter, agrees with the
    ///    mirror over a battery far wider than the tests.
    ///
    /// The two are different candidates, and the gap is a
    /// recorded finding, not a fudge: both completions score
    /// tpr 1.000 (no test exercises the wildcard arm — every
    /// tested input has its own arm), and Char is Opaque to
    /// the Z3 verifier, so no constraint seed and no proof
    /// can prefer the exact one; selection's tie-break
    /// crowns the wildcard-`true` completion, which is wrong
    /// on every untested char. That is the guard-table
    /// wildcard caveat without its usual Z3 backstop — the
    /// exactness S4 can claim for Char classifiers today is
    /// "the exact body is in the pool and executes exactly",
    /// not "the build crowns it".
    #[test]
    fn char_literal_classifier_builds_and_executes() {
        let src = r#"
        T {
        spec is_digit {
            input: c: Char
            output: Bool
            constraint: output == (c >= '0' && c <= '9')
            test "d0" { input: '0'; expect: true }
            test "d1" { input: '1'; expect: true }
            test "d2" { input: '2'; expect: true }
            test "d3" { input: '3'; expect: true }
            test "d4" { input: '4'; expect: true }
            test "d5" { input: '5'; expect: true }
            test "d6" { input: '6'; expect: true }
            test "d7" { input: '7'; expect: true }
            test "d8" { input: '8'; expect: true }
            test "d9" { input: '9'; expect: true }
            test "below" { input: '/'; expect: false }
            test "above" { input: ':'; expect: false }
            test "letter" { input: 'a'; expect: false }
            test "space" { input: ' '; expect: false }
            test "newline" { input: '\n'; expect: false }
        }
        }
        "#;
        let module = crate::spec::parser::parse(src).expect("char spec must parse");
        let spec = module.functions[0].clone();
        let pool = synthesize_with_module(&spec, 1, &module.functions)
            .expect("synthesis must succeed");

        let run_on = |body: &IRNode, c: char| -> crate::synthesis::eval::Value {
            let mut env = crate::synthesis::eval::Env::new();
            env.insert("c".into(), crate::synthesis::eval::Value::Char(c));
            crate::synthesis::eval::eval(body, &env)
                .unwrap_or_else(|e| panic!("body not evaluable on {:?}: {:?}", c, e))
        };
        let mirror = |c: char| crate::synthesis::eval::Value::Bool(('0'..='9').contains(&c));
        let tested: Vec<char> = spec
            .tests
            .iter()
            .map(|t| match t.input {
                Expr::CharLit(c) => c,
                ref other => panic!("test input is not a char literal: {:?}", other),
            })
            .collect();

        // Fact 2's candidate, found in the pool BEFORE
        // verification/selection consume it: the test-induced
        // shape whose wildcard arm is the `false` literal.
        let exact = pool
            .iter()
            .find(|cand| match &cand.body {
                IRNode::Match { cases, .. } => {
                    count_holes(&cand.body) == 0
                        && cases.len() == spec.tests.len() + 1
                        && matches!(
                            cases[spec.tests.len()],
                            (IRPattern::Wildcard, IRNode::Literal(IRLiteral::Bool(false)))
                        )
                }
                _ => false,
            })
            .expect("pool must contain the spec-exact test-induced completion");
        let mut battery: Vec<char> = ('\u{0}'..='\u{2FF}').collect();
        battery.extend(['\u{660}', '\u{6F0}', '\u{B2}', '\u{FF10}', '😀', '\u{10FFFF}']);
        for c in battery {
            assert_eq!(
                run_on(&exact.body, c),
                mirror(c),
                "spec-exact completion disagrees with the mirror on {:?}",
                c
            );
        }

        // Fact 1: the crowned winner, judged on the signal
        // the pipeline can actually see — its own tests.
        let verified = crate::verify::verifier::Verifier::with_specs(&module.functions)
            .verify_all(pool, &spec)
            .expect("verification must succeed");
        let winner = crate::synthesis::selector::select_best(verified, &spec)
            .expect("selection must crown a winner");
        assert_eq!(count_holes(&winner.body), 0, "winner must be hole-free");
        assert!(
            (winner.scores.test_pass_ratio - 1.0).abs() < 1e-9,
            "winner must pass every test, tpr = {}",
            winner.scores.test_pass_ratio
        );
        for c in tested {
            assert_eq!(
                run_on(&winner.body, c),
                mirror(c),
                "winner disagrees with the mirror on tested {:?}",
                c
            );
        }
    }

    /// Hole penalty: correctness credit halves per unfilled hole,
    /// so a holed candidate ranks strictly below an otherwise-equal
    /// hole-free one. The dummy spec has no tests, so the neutral
    /// test score is 0.5 and the placeholder constraint score 0.7.
    #[test]
    fn evaluate_candidate_penalizes_holes() {
        let spec = dummy_spec();
        let free = make_candidate(IRNode::Literal(IRLiteral::Int(1)), &spec);
        let one_hole = make_candidate(
            IRNode::Block(vec![
                IRNode::Literal(IRLiteral::Int(1)),
                IRNode::Hole { id: 1, expected_type: None },
            ]),
            &spec,
        );
        let two_holes = make_candidate(
            IRNode::Block(vec![
                IRNode::Hole { id: 1, expected_type: None },
                IRNode::Hole { id: 2, expected_type: None },
            ]),
            &spec,
        );

        let free_scores = evaluate_candidate(&free, &spec);
        assert!((free_scores.test_pass_ratio - 0.5).abs() < 1e-9);
        assert!((free_scores.constraint_score - 0.7).abs() < 1e-9);

        let one_scores = evaluate_candidate(&one_hole, &spec);
        assert!((one_scores.test_pass_ratio - 0.25).abs() < 1e-9);
        assert!((one_scores.constraint_score - 0.35).abs() < 1e-9);
        assert!(one_scores.test_pass_ratio < free_scores.test_pass_ratio);
        assert!(one_scores.constraint_score < free_scores.constraint_score);

        let two_scores = evaluate_candidate(&two_holes, &spec);
        assert!((two_scores.test_pass_ratio - 0.125).abs() < 1e-9);
        assert!(two_scores.test_pass_ratio < one_scores.test_pass_ratio);
    }

    /// A spec with real, mappable tests — mirrors
    /// examples/self_host/validator.morph `is_literal_kind`
    /// (input kind: Int, output Bool, output == (kind == 0)).
    fn is_literal_kind_spec() -> FunctionSpec {
        let mut spec = dummy_spec();
        spec.name = "is_literal_kind".into();
        spec.params = vec![Param {
            name: "kind".into(),
            ty: TypeRef::Int,
            annotations: HashMap::new(),
        }];
        spec.return_type = TypeRef::Bool;
        let test = |input: i64, expected: bool| Test {
            name: None,
            input: Expr::IntLit(input),
            expected_output: Expr::BoolLit(expected),
            timeout_ms: None,
            property: false,
        };
        spec.tests = vec![test(0, true), test(2, false), test(4, false)];
        spec
    }

    /// Truth signal: a candidate that actually implements the
    /// spec scores a measured 1.0 — and a wrong one scores
    /// strictly less, by exactly the tests it fails. Before the
    /// IR interpreter landed, both scored 1.0 (the filter in
    /// evaluate_candidate unconditionally returned true).
    #[test]
    fn evaluate_candidate_measures_real_test_pass_ratio() {
        let spec = is_literal_kind_spec();

        // Correct: kind == 0.
        let correct = make_candidate(
            IRNode::BinOp(
                IRBinOp::Eq,
                Box::new(IRNode::Var("kind".into())),
                Box::new(IRNode::Literal(IRLiteral::Int(0))),
            ),
            &spec,
        );
        let scores = evaluate_candidate(&correct, &spec);
        assert!((scores.test_pass_ratio - 1.0).abs() < 1e-9);

        // Wrong: always false — passes exactly 2 of 3 tests.
        let wrong = make_candidate(IRNode::Literal(IRLiteral::Bool(false)), &spec);
        let scores = evaluate_candidate(&wrong, &spec);
        assert!((scores.test_pass_ratio - 2.0 / 3.0).abs() < 1e-9);

        // The classic random-tree failure mode: an unbound
        // Var("result") body. It never compiled — and now it
        // also measures 0 instead of a free 1.0.
        let unbound = make_candidate(IRNode::Var("result".into()), &spec);
        let scores = evaluate_candidate(&unbound, &spec);
        assert_eq!(scores.test_pass_ratio, 0.0);
    }

    /// A two-param spec with tuple-form tests — the parser
    /// shape of examples/self_host/count_matching.morph's
    /// tests, on an (a, b) -> a + b semantics whose correct
    /// body the interpreter can execute.
    fn tuple_test_spec() -> FunctionSpec {
        let mut spec = dummy_spec();
        spec.name = "add_pair".into();
        spec.params = ["a", "b"]
            .iter()
            .map(|name| Param {
                name: name.to_string(),
                ty: TypeRef::Int,
                annotations: HashMap::new(),
            })
            .collect();
        spec.return_type = TypeRef::Int;
        let test = |x: i64, y: i64, expected: i64| Test {
            name: None,
            input: Expr::Call(
                "__tuple__".into(),
                vec![Expr::IntLit(x), Expr::IntLit(y)],
            ),
            expected_output: Expr::IntLit(expected),
            timeout_ms: None,
            property: false,
        };
        spec.tests = vec![test(1, 2, 3), test(10, 5, 15), test(0, 0, 0)];
        spec
    }

    /// Phase 53: tuple-form test inputs bind elementwise, so
    /// a multi-param spec's tpr is MEASURED, not the FALLBACK.
    /// Before the fix, run_spec_tests reported evaluable == 0
    /// for this spec and the correct and trivial bodies below
    /// BOTH scored the fallback 1.0 (the Phase 52 finding).
    #[test]
    fn evaluate_candidate_tuple_tests_discriminate() {
        let spec = tuple_test_spec();

        // Correct: a + b earns its 1.0 (3/3 tests).
        let correct = make_candidate(
            IRNode::BinOp(
                IRBinOp::Add,
                Box::new(IRNode::Var("a".into())),
                Box::new(IRNode::Var("b".into())),
            ),
            &spec,
        );
        let scores = evaluate_candidate(&correct, &spec);
        assert!((scores.test_pass_ratio - 1.0).abs() < 1e-9);

        // Trivial: returns the first parameter — passes only
        // the (0, 0) test, so 1/3, strictly below the correct
        // body. This assertion FAILED before the fix (the
        // trivial body scored the fallback 1.0).
        let trivial = make_candidate(IRNode::Var("a".into()), &spec);
        let scores = evaluate_candidate(&trivial, &spec);
        assert!((scores.test_pass_ratio - 1.0 / 3.0).abs() < 1e-9);
    }

    /// Phase 53: the FALLBACK keeps its legitimate domain,
    /// stated explicitly. A spec whose tests exist but NONE
    /// can be mapped — here sort.morph's shape: tuple inputs
    /// carrying a lambda for a fn-typed parameter, which the
    /// evaluator cannot represent as a value — still scores
    /// every candidate the fallback 1.0, exactly as before
    /// the tuple fix.
    #[test]
    fn evaluate_candidate_unmappable_tests_keep_fallback() {
        let mut spec = dummy_spec();
        spec.name = "sort_shape".into();
        spec.params = vec![
            Param {
                name: "list".into(),
                ty: TypeRef::List(Box::new(TypeRef::Int)),
                annotations: HashMap::new(),
            },
            Param {
                name: "cmp".into(),
                ty: TypeRef::Function(
                    vec![TypeRef::Int, TypeRef::Int],
                    Box::new(TypeRef::Bool),
                ),
                annotations: HashMap::new(),
            },
        ];
        spec.return_type = TypeRef::List(Box::new(TypeRef::Int));
        spec.tests = vec![Test {
            name: None,
            input: Expr::Call(
                "__tuple__".into(),
                vec![
                    Expr::Call(
                        "List".into(),
                        vec![Expr::IntLit(3), Expr::IntLit(1), Expr::IntLit(2)],
                    ),
                    Expr::Lambda(
                        vec![
                            ("a".into(), TypeRef::Named("_".into())),
                            ("b".into(), TypeRef::Named("_".into())),
                        ],
                        Box::new(Expr::BinOp(
                            BinOp::Lt,
                            Box::new(Expr::Var("a".into())),
                            Box::new(Expr::Var("b".into())),
                        )),
                    ),
                ],
            ),
            expected_output: Expr::Call(
                "List".into(),
                vec![Expr::IntLit(1), Expr::IntLit(2), Expr::IntLit(3)],
            ),
            timeout_ms: None,
            property: false,
        }];
        // The identity body — the class Phase 52 measured
        // being crowned for sort — still gets the fallback
        // 1.0: no test could be mapped, so there is no truth
        // signal to measure, by the recorded FALLBACK policy.
        let identity = make_candidate(IRNode::Var("list".into()), &spec);
        let scores = evaluate_candidate(&identity, &spec);
        assert!((scores.test_pass_ratio - 1.0).abs() < 1e-9);
    }

    /// The TestInduced shape — a match on the input with one
    /// literal arm per test plus exactly one wildcard Hole —
    /// passes every test its arms reach, then the hole decay
    /// halves its credit: measured 1.0 × 0.5 decay = 0.5.
    #[test]
    fn evaluate_candidate_test_induced_shape_scores_measured_ratio_times_hole_decay() {
        let spec = is_literal_kind_spec();
        let body = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![
                (
                    IRPattern::Literal(IRLiteral::Int(0)),
                    IRNode::Literal(IRLiteral::Bool(true)),
                ),
                (
                    IRPattern::Literal(IRLiteral::Int(2)),
                    IRNode::Literal(IRLiteral::Bool(false)),
                ),
                (
                    IRPattern::Literal(IRLiteral::Int(4)),
                    IRNode::Literal(IRLiteral::Bool(false)),
                ),
                (
                    IRPattern::Wildcard,
                    IRNode::Hole { id: 99, expected_type: None },
                ),
            ],
        };
        let candidate = make_candidate(body, &spec);
        let scores = evaluate_candidate(&candidate, &spec);
        assert!((scores.test_pass_ratio - 0.5).abs() < 1e-9);
    }

    /// Walk `node` tracking lexical scope the way the language
    /// binds names (Let / For / Lambda / Closure / match-arm
    /// variable patterns) and assert every `Var` refers to a name
    /// actually in scope at that point.
    fn assert_vars_in_scope(node: &IRNode, scope: &[String]) {
        match node {
            IRNode::Var(name) => assert!(
                scope.iter().any(|s| s == name),
                "free variable Var({:?}) emitted with scope {:?}",
                name,
                scope
            ),
            IRNode::Block(stmts) => stmts.iter().for_each(|s| assert_vars_in_scope(s, scope)),
            IRNode::Let { name, value, body } => {
                assert_vars_in_scope(value, scope);
                let mut inner = scope.to_vec();
                inner.push(name.clone());
                assert_vars_in_scope(body, &inner);
            }
            IRNode::If { condition, then_branch, else_branch } => {
                assert_vars_in_scope(condition, scope);
                assert_vars_in_scope(then_branch, scope);
                assert_vars_in_scope(else_branch, scope);
            }
            IRNode::While { condition, body, .. } => {
                assert_vars_in_scope(condition, scope);
                assert_vars_in_scope(body, scope);
            }
            IRNode::For { var, start, end, body } => {
                assert_vars_in_scope(start, scope);
                assert_vars_in_scope(end, scope);
                let mut inner = scope.to_vec();
                inner.push(var.clone());
                assert_vars_in_scope(body, &inner);
            }
            IRNode::Match { scrutinee, cases } => {
                assert_vars_in_scope(scrutinee, scope);
                for (pattern, body) in cases {
                    let mut inner = scope.to_vec();
                    pattern_bindings(pattern, &mut inner);
                    assert_vars_in_scope(body, &inner);
                }
            }
            IRNode::Call { args, .. } => {
                args.iter().for_each(|a| assert_vars_in_scope(a, scope))
            }
            IRNode::Return(e) => assert_vars_in_scope(e, scope),
            IRNode::BinOp(_, l, r) => {
                assert_vars_in_scope(l, scope);
                assert_vars_in_scope(r, scope);
            }
            IRNode::UnaryOp(_, e) => assert_vars_in_scope(e, scope),
            IRNode::Lambda { params, body, .. } | IRNode::Closure { params, body, .. } => {
                let mut inner = scope.to_vec();
                inner.extend(params.iter().map(|(n, _)| n.clone()));
                assert_vars_in_scope(body, &inner);
            }
            IRNode::Collection { elements, .. } => {
                elements.iter().for_each(|e| assert_vars_in_scope(e, scope))
            }
            IRNode::Map { function, over } | IRNode::Filter { predicate: function, over } => {
                assert_vars_in_scope(function, scope);
                assert_vars_in_scope(over, scope);
            }
            IRNode::Fold { operation, initial, over } => {
                assert_vars_in_scope(operation, scope);
                assert_vars_in_scope(initial, scope);
                assert_vars_in_scope(over, scope);
            }
            IRNode::Sort { comparator, collection } => {
                assert_vars_in_scope(comparator, scope);
                assert_vars_in_scope(collection, scope);
            }
            IRNode::Alloc { initial, .. } => assert_vars_in_scope(initial, scope),
            IRNode::Assign { target, value } => {
                assert_vars_in_scope(target, scope);
                assert_vars_in_scope(value, scope);
            }
            IRNode::Typed { node, .. } => assert_vars_in_scope(node, scope),
            IRNode::Index(c, i) => {
                assert_vars_in_scope(c, scope);
                assert_vars_in_scope(i, scope);
            }
            IRNode::Loop { body, .. } => assert_vars_in_scope(body, scope),
            IRNode::Break { value } => {
                if let Some(v) = value {
                    assert_vars_in_scope(v, scope);
                }
            }
            IRNode::StructDef { body, .. } => assert_vars_in_scope(body, scope),
            IRNode::TryPropagate(e) => assert_vars_in_scope(e, scope),
            // M1 features (Phase 73).
            IRNode::FieldAccess { base, .. } => assert_vars_in_scope(base, scope),
            IRNode::RecordLit { fields, .. } => {
                fields.iter().for_each(|(_, v)| assert_vars_in_scope(v, scope))
            }
            IRNode::Hole { .. } | IRNode::Literal(_) => {}
        }
    }

    /// Scope-aware generation: every `Var` leaf the random tree
    /// generator emits must name a variable that is actually in
    /// scope there — a spec parameter, or a name the generator
    /// itself bound on the way down (its `For` variable). The old
    /// generator hardcoded `Var("result")` as a leaf, which is
    /// bound nowhere: such trees can never evaluate or compile.
    /// Checked against a spec with a parameter and one without
    /// (empty scope → no `Var` leaves at all).
    #[test]
    fn random_ir_tree_only_emits_in_scope_vars() {
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        for spec in [dummy_spec(), is_literal_kind_spec()] {
            let scope: Vec<String> = spec.params.iter().map(|p| p.name.clone()).collect();
            for depth in 0..=4 {
                for _ in 0..200 {
                    let tree = random_ir_tree(&spec, depth, &engine);
                    assert_vars_in_scope(&tree, &scope);
                }
            }
        }
    }

    /// `scope_at_path` starts from the spec parameters and adds
    /// exactly the binders enclosing the position: a `Let` name
    /// counts in its body but not its value; a `For` variable in
    /// its body; a match-arm variable pattern in that arm only.
    #[test]
    fn scope_at_path_collects_enclosing_binders() {
        let tree = IRNode::Let {
            name: "y".into(),
            value: Box::new(IRNode::Hole { id: 1, expected_type: None }),
            body: Box::new(IRNode::For {
                var: "i".into(),
                start: Box::new(IRNode::Literal(IRLiteral::Int(0))),
                end: Box::new(IRNode::Literal(IRLiteral::Int(3))),
                body: Box::new(IRNode::Match {
                    scrutinee: Box::new(IRNode::Var("kind".into())),
                    cases: vec![
                        (
                            IRPattern::Variable("m".into()),
                            IRNode::Hole { id: 2, expected_type: None },
                        ),
                        (
                            IRPattern::Wildcard,
                            IRNode::Hole { id: 3, expected_type: None },
                        ),
                    ],
                }),
            }),
        };
        let params = vec!["kind".to_string()];
        let holes = find_hole_positions(&tree);
        assert_eq!(holes.len(), 3);
        // Hole 1 sits in the Let's *value*: "y" is not yet bound.
        assert_eq!(scope_at_path(&tree, &holes[0].path, &params), vec!["kind"]);
        // Hole 2 sits in the variable-pattern arm inside the loop.
        assert_eq!(
            scope_at_path(&tree, &holes[1].path, &params),
            vec!["kind", "y", "i", "m"]
        );
        // Hole 3 sits in the wildcard arm: no pattern binding.
        assert_eq!(
            scope_at_path(&tree, &holes[2].path, &params),
            vec!["kind", "y", "i"]
        );
    }

    /// Rollout fillings obey the same rule as tree generation:
    /// with a non-empty scope every emitted `Var` comes from it;
    /// with an empty scope no `Var` is emitted at all (the old
    /// code returned Var("result") / Var("output") here).
    #[test]
    fn random_filling_only_emits_in_scope_vars() {
        let hole = HoleRef { path: Vec::new(), expected_type: None };
        let scope = vec!["kind".to_string(), "y".to_string()];
        for _ in 0..500 {
            let filling = random_filling(&hole, &scope);
            assert_vars_in_scope(&filling, &scope);
        }
        for _ in 0..500 {
            let filling = random_filling(&hole, &[]);
            assert_vars_in_scope(&filling, &[]);
        }
    }

    /// MCTS expansion fillings are computed against the scope at
    /// the hole: inside `let y = … in □` for a spec with parameter
    /// `kind`, fillings may reference `kind` and `y` — and nothing
    /// else (the old code hardcoded Var("x") and `a + b`).
    #[test]
    fn generate_hole_fillings_only_emit_in_scope_vars() {
        let spec = is_literal_kind_spec();
        let node = MCTSNode {
            state: IRNode::Let {
                name: "y".into(),
                value: Box::new(IRNode::Literal(IRLiteral::Int(0))),
                body: Box::new(IRNode::Hole { id: 7, expected_type: None }),
            },
            parent: None,
            children: Vec::new(),
            visits: 0,
            total_reward: 0.0,
            is_terminal: false,
            hole_count: 1,
        };
        let holes = find_hole_positions(&node.state);
        assert_eq!(holes.len(), 1);
        let scope = vec!["kind".to_string(), "y".to_string()];
        for _ in 0..100 {
            for filling in generate_hole_fillings(&holes[0], &node, &spec) {
                assert_vars_in_scope(&filling, &scope);
            }
        }
    }

    fn mcts_node_with(state: IRNode) -> MCTSNode {
        let hole_count = count_holes(&state);
        MCTSNode {
            state,
            parent: None,
            children: Vec::new(),
            visits: 0,
            total_reward: 0.0,
            is_terminal: false,
            hole_count,
        }
    }

    /// Every filling must evaluate (in an empty environment —
    /// typed fillings are literals, so nothing may be unbound)
    /// to a value of the hole's type.
    fn assert_fillings_eval_to(fillings: &[IRNode], what: &str, ok: impl Fn(&crate::synthesis::eval::Value) -> bool) {
        assert!(!fillings.is_empty(), "no fillings generated for {}", what);
        for filling in fillings {
            let value = crate::synthesis::eval::eval(filling, &HashMap::new()).unwrap_or_else(|e| {
                panic!("filling {:?} for {} must evaluate: {:?}", filling, what, e)
            });
            assert!(
                ok(&value),
                "filling {:?} for {} evaluated to {:?} — wrong type",
                filling, what, value
            );
        }
    }

    /// Type-directed expansion: a hole that carries an expected
    /// type (the TestInduced seed's wildcard hole carries the
    /// spec's return type) must only be filled with expressions
    /// of that type. The legacy menu always led with Int(0) —
    /// which is why winning seed completions put `_ => 0` in
    /// String and Bool functions and never compiled (E0308).
    #[test]
    fn generate_hole_fillings_respect_bool_hole_type() {
        let spec = is_literal_kind_spec(); // returns Bool
        let node = mcts_node_with(IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![(
                IRPattern::Wildcard,
                IRNode::Hole { id: 7, expected_type: Some(TypeRef::Bool) },
            )],
        });
        let holes = find_hole_positions(&node.state);
        assert_eq!(holes.len(), 1);
        let fillings = generate_hole_fillings(&holes[0], &node, &spec);
        assert_fillings_eval_to(&fillings, "Bool hole", |v| {
            matches!(v, crate::synthesis::eval::Value::Bool(_))
        });
    }

    #[test]
    fn generate_hole_fillings_respect_string_hole_type() {
        let mut spec = dummy_spec();
        spec.name = "name_of".into();
        spec.params = vec![Param {
            name: "kind".into(),
            ty: TypeRef::Int,
            annotations: HashMap::new(),
        }];
        spec.return_type = TypeRef::String;
        spec.tests = vec![Test {
            name: None,
            input: Expr::IntLit(0),
            expected_output: Expr::StringLit("Literal".into()),
            timeout_ms: None,
            property: false,
        }];
        let node = mcts_node_with(IRNode::Hole {
            id: 1,
            expected_type: Some(TypeRef::String),
        });
        let holes = find_hole_positions(&node.state);
        let fillings = generate_hole_fillings(&holes[0], &node, &spec);
        assert_fillings_eval_to(&fillings, "String hole", |v| {
            matches!(v, crate::synthesis::eval::Value::Str(_))
        });
        // Values the spec's tests expect are return-typed by
        // construction, so they belong in the menu too.
        assert!(
            fillings.iter().any(|f| matches!(
                f,
                IRNode::Literal(IRLiteral::String(s)) if s == "Literal"
            )),
            "test output \"Literal\" missing from String fillings: {:?}",
            fillings
        );
    }

    #[test]
    fn generate_hole_fillings_respect_int_hole_type() {
        let mut spec = dummy_spec();
        spec.name = "arity_of".into();
        spec.return_type = TypeRef::Int;
        spec.tests = vec![Test {
            name: None,
            input: Expr::IntLit(0),
            expected_output: Expr::IntLit(2),
            timeout_ms: None,
            property: false,
        }];
        let node = mcts_node_with(IRNode::Hole {
            id: 1,
            expected_type: Some(TypeRef::Int),
        });
        let holes = find_hole_positions(&node.state);
        let fillings = generate_hole_fillings(&holes[0], &node, &spec);
        assert_fillings_eval_to(&fillings, "Int hole", |v| {
            matches!(v, crate::synthesis::eval::Value::Int(_))
        });
        assert!(
            fillings
                .iter()
                .any(|f| matches!(f, IRNode::Literal(IRLiteral::Int(2)))),
            "test output 2 missing from Int fillings: {:?}",
            fillings
        );
    }

    #[test]
    fn generate_hole_fillings_respect_list_hole_type() {
        let mut spec = dummy_spec();
        spec.return_type = TypeRef::List(Box::new(TypeRef::Int));
        let node = mcts_node_with(IRNode::Hole {
            id: 1,
            expected_type: Some(TypeRef::List(Box::new(TypeRef::Int))),
        });
        let holes = find_hole_positions(&node.state);
        let fillings = generate_hole_fillings(&holes[0], &node, &spec);
        assert_fillings_eval_to(&fillings, "List hole", |v| {
            matches!(v, crate::synthesis::eval::Value::List(_))
        });
    }

    /// Regression guard: a hole with NO expected type keeps the
    /// legacy type-blind menu exactly — typed fillings are an
    /// addition for typed holes, not a behavior change for the
    /// untyped majority (random trees, templates).
    #[test]
    fn generate_hole_fillings_untyped_hole_keeps_legacy_menu() {
        let spec = is_literal_kind_spec();
        let node = mcts_node_with(IRNode::Hole { id: 3, expected_type: None });
        let holes = find_hole_positions(&node.state);
        let fillings = generate_hole_fillings(&holes[0], &node, &spec);
        assert!(
            fillings
                .iter()
                .any(|f| matches!(f, IRNode::Literal(IRLiteral::Int(0)))),
            "legacy menu lost its Int(0) filling: {:?}",
            fillings
        );
        assert!(
            fillings
                .iter()
                .any(|f| matches!(f, IRNode::Literal(IRLiteral::Bool(true)))),
            "legacy menu lost its Bool(true) filling: {:?}",
            fillings
        );
    }

    /// Rollout fillings obey the same type discipline as the
    /// expansion menu: a draw for a Bool-typed hole is always a
    /// Bool literal, never Int(0) / a Return wrapper / a fresh
    /// Hole (the legacy arms).
    #[test]
    fn random_filling_respects_hole_type() {
        let tree = IRNode::Hole {
            id: 5,
            expected_type: Some(TypeRef::Bool),
        };
        let holes = find_hole_positions(&tree);
        assert_eq!(holes.len(), 1);
        let scope = vec!["kind".to_string()];
        for _ in 0..500 {
            let filling = random_filling(&holes[0], &scope);
            let value = crate::synthesis::eval::eval(&filling, &HashMap::new()).unwrap_or_else(|e| {
                panic!("rollout filling {:?} must evaluate: {:?}", filling, e)
            });
            assert!(
                matches!(value, crate::synthesis::eval::Value::Bool(_)),
                "rollout filling {:?} evaluated to {:?} — wrong type for a Bool hole",
                filling, value
            );
        }
    }

    /// Regression: `random_ir_tree` used to emit
    /// `Call { function: "_", args: [] }` as a leaf — a call to a
    /// nonexistent function that codegen happily emitted. An
    /// unfilled leaf must be an honest `Hole` instead.
    #[test]
    fn random_ir_tree_never_emits_placeholder_call() {
        let spec = dummy_spec();
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        for depth in 0..=4 {
            for _ in 0..200 {
                let tree = random_ir_tree(&spec, depth, &engine);
                assert!(
                    !contains_placeholder_call(&tree),
                    "random_ir_tree(depth={}) emitted a placeholder Call to \"_\"",
                    depth
                );
            }
        }
    }

    // ── Phase 37: range rows in the single-key guard ──────
    //    table (Phase 36, Finding 1)
    //
    // `pin_int_value` recovers a guard row's pin only when
    // exactly ONE value falsifies every guard leaf. Slice 5
    // writes BAND rows — `site<0||site>4||output==0` pins
    // {0..=4} at once — and an Eq-leaf CATCH-ALL pinning the
    // complement of the single-value rows. For such a row
    // `guard_row` returned None — the NON-row outcome, not a
    // deviation — so the builder silently SKIPPED it and the
    // wildcard carried the typed-literal default,
    // contradicting the skipped rows wherever they pin a
    // non-default value. Three slice-5 seeds were therefore
    // Z3-refuted in every Phase 36 run (tpr 0.833 / 0.667 /
    // 0.667) and their specs crowned Inconclusive lotteries.
    // The fix lifts the pair family's cell recovery to one
    // key: rows contribute their whole falsifying REGION to
    // a joint table over the candidate span, and a
    // row-shaped postcondition the recovered table does not
    // subsume is a deviation, never a skip.

    /// Finding 1, spec (i): rust_expectation_source's bands
    /// {0..=4} → 0 and {5, 6} → 1 beside the single row
    /// {7} → 2. Before the fix the seed was {7 → 2, _ → 0} —
    /// sites 5 and 6 contradicted — and Z3 refuted it.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_guard_table_seed_for_expectation_source() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_expectation_source");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the guard-table seed");
        let seed = constraint_seed(&engine, &spec, "guard_table");
        assert_eq!(count_holes(&seed.body), 0);

        // The {5, 6} band materializes as arms; the {0..=4}
        // band pins the wildcard's own value (the tails are
        // unpinned, so the wildcard is the typed default 0)
        // and needs no arm.
        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("site".into())),
            cases: vec![
                (IRPattern::Literal(IRLiteral::Int(5)), IRNode::Literal(IRLiteral::Int(1))),
                (IRPattern::Literal(IRLiteral::Int(6)), IRNode::Literal(IRLiteral::Int(1))),
                (IRPattern::Literal(IRLiteral::Int(7)), IRNode::Literal(IRLiteral::Int(2))),
                (IRPattern::Wildcard, IRNode::Literal(IRLiteral::Int(0))),
            ],
        };
        assert_eq!(format!("{:?}", seed.body), format!("{:?}", expected));

        for (site, want) in [(-1, 0), (0, 0), (4, 0), (5, 1), (6, 1), (7, 2), (8, 0)] {
            let v = eval_int_body(&seed.body, "site", site);
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Int(want),
                "site {} must follow the spec's bands",
                site
            );
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        for _ in 0..3 {
            let pool = synthesize_with_module(&spec, 1, &module)
                .expect("opt-1 synthesis must succeed");
            let verified = verifier.verify_all(pool, &spec).expect("verify_all");
            let winner =
                crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
            assert!(winner.verified, "the guard-table seed must win verified");
            assert_eq!(
                format!("{:?}", winner.body),
                format!("{:?}", seed.body),
                "the winning body must be the guard-table seed in every draw"
            );
        }
    }

    /// Finding 1, spec (ii): rust_stmt_form's Eq-leaf
    /// catch-all pins the complement of {0..=8, 15..=22} to
    /// 17 — both tails included — so the wildcard must carry
    /// the pinned fall-through value 17, not the typed
    /// default 0 the skipped row left behind (the seed's
    /// in-spec loop-fallback test, kind 24 → 17, failed:
    /// tpr 0.833, refuted every run).
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_guard_table_seed_for_stmt_form() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_stmt_form");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the guard-table seed");
        let seed = constraint_seed(&engine, &spec, "guard_table");
        assert_eq!(count_holes(&seed.body), 0);

        // Every pinned kind, in the spec's row order, with
        // the catch-all riding the wildcard at 17.
        let want = |kind: i64| match kind {
            0..=8 => kind,
            15 => 11,
            16 => 12,
            17 => 13,
            18 => 14,
            19 => 15,
            20 => 9,
            21 => 10,
            22 => 16,
            _ => 17,
        };
        for kind in -2..=30 {
            let v = eval_int_body(&seed.body, "kind", kind);
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Int(want(kind)),
                "kind {} must follow the spec's dispatch",
                kind
            );
        }
        // The wildcard is the fall-through value itself.
        let IRNode::Match { cases, .. } = &seed.body else {
            panic!("the seed body must be a match");
        };
        let (last_pat, last_body) = cases.last().expect("a match has a last arm");
        assert!(
            matches!(last_pat, IRPattern::Wildcard),
            "the last arm must be the wildcard"
        );
        assert_eq!(
            format!("{:?}", last_body),
            format!("{:?}", IRNode::Literal(IRLiteral::Int(17))),
            "the wildcard must carry the catch-all's pinned value"
        );

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        for _ in 0..3 {
            let pool = synthesize_with_module(&spec, 1, &module)
                .expect("opt-1 synthesis must succeed");
            let verified = verifier.verify_all(pool, &spec).expect("verify_all");
            let winner =
                crate::synthesis::selector::select_best(verified, &spec).expect("select_best");
            assert!(winner.verified, "the guard-table seed must win verified");
            assert_eq!(
                format!("{:?}", winner.body),
                format!("{:?}", seed.body),
                "the winning body must be the guard-table seed in every draw"
            );
        }
    }

    /// Finding 1, spec (iii): rust_stmt_child_indent's bands
    /// {0, 1} → 0, {7, 8} → 0 and {2..=5} → 1 beside the
    /// single row {6} → 2. Before the fix only {6} → 2 was
    /// armed; the {2..=5} band fell to the default 0 and Z3
    /// refuted the seed (tpr 0.667).
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_guard_table_seed_for_stmt_child_indent() {
        let module = load_codegen_module();
        let spec = module_spec(&module, "rust_stmt_child_indent");
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let seeds = engine.seed_from_constraints(&spec);
        assert_eq!(seeds.len(), 1, "exactly the guard-table seed");
        let seed = constraint_seed(&engine, &spec, "guard_table");
        assert_eq!(count_holes(&seed.body), 0);

        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("site".into())),
            cases: vec![
                (IRPattern::Literal(IRLiteral::Int(2)), IRNode::Literal(IRLiteral::Int(1))),
                (IRPattern::Literal(IRLiteral::Int(3)), IRNode::Literal(IRLiteral::Int(1))),
                (IRPattern::Literal(IRLiteral::Int(4)), IRNode::Literal(IRLiteral::Int(1))),
                (IRPattern::Literal(IRLiteral::Int(5)), IRNode::Literal(IRLiteral::Int(1))),
                (IRPattern::Literal(IRLiteral::Int(6)), IRNode::Literal(IRLiteral::Int(2))),
                (IRPattern::Wildcard, IRNode::Literal(IRLiteral::Int(0))),
            ],
        };
        assert_eq!(format!("{:?}", seed.body), format!("{:?}", expected));

        for (site, want) in [
            (-1, 0), (0, 0), (1, 0), (2, 1), (3, 1), (4, 1), (5, 1),
            (6, 2), (7, 0), (8, 0), (9, 0),
        ] {
            let v = eval_int_body(&seed.body, "site", site);
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Int(want),
                "site {} must follow the spec's bands",
                site
            );
        }

        let verifier = crate::verify::verifier::Verifier::with_specs(&module);
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);
    }

    /// The two catch-alls that escaped Finding 1 by luck —
    /// rust_typed_form's and rust_tail_form's complement rows
    /// pin 0, the typed default their wildcards already
    /// carried. The joint recovery must reproduce their
    /// bodies BYTE-IDENTICALLY: the fall-through value the
    /// tails determine is 0, the catch-all's cells all pin
    /// the wildcard's own value and materialize no arm, and
    /// the single-value rows arm in row order as always.
    /// (Phase 78: rust_tail_form's expected body carries the
    /// amendment's two new arms — kinds 28/29 pin 1 by their
    /// own rows, appended in row order after kind 27's.)
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_guard_table_catch_all_at_default_unchanged() {
        let module = load_codegen_module();
        let engine = SynthesisEngine::with_module(SynthesisConfig::default(), &module);
        let verifier = crate::verify::verifier::Verifier::with_specs(&module);

        let spec = module_spec(&module, "rust_typed_form");
        let seed = constraint_seed(&engine, &spec, "guard_table");
        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases: vec![
                (IRPattern::Literal(IRLiteral::Int(9)), IRNode::Literal(IRLiteral::Int(1))),
                (IRPattern::Literal(IRLiteral::Int(22)), IRNode::Literal(IRLiteral::Int(2))),
                (IRPattern::Wildcard, IRNode::Literal(IRLiteral::Int(0))),
            ],
        };
        assert_eq!(format!("{:?}", seed.body), format!("{:?}", expected));
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);

        let spec = module_spec(&module, "rust_tail_form");
        let seed = constraint_seed(&engine, &spec, "guard_table");
        let mut cases: Vec<(IRPattern, IRNode)> = [
            (1, 2), (22, 3), (7, 1), (9, 1), (10, 1), (11, 1), (12, 1), (13, 1),
            (14, 1), (15, 1), (16, 1), (17, 1), (18, 1), (19, 1), (23, 1), (27, 1),
            (28, 1), (29, 1),
        ]
        .into_iter()
        .map(|(k, v)| {
            (
                IRPattern::Literal(IRLiteral::Int(k)),
                IRNode::Literal(IRLiteral::Int(v)),
            )
        })
        .collect();
        cases.push((IRPattern::Wildcard, IRNode::Literal(IRLiteral::Int(0))));
        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("kind".into())),
            cases,
        };
        assert_eq!(format!("{:?}", seed.body), format!("{:?}", expected));
        let result = verifier.verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);
    }

    /// Range recovery on synthetic shapes: a band row and an
    /// Eq-leaf catch-all in one table. The catch-all pins
    /// both tails, so the wildcard carries ITS value (9);
    /// the band's cells pin 5 and materialize as arms.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_guard_table_bands_and_catch_all_recovered() {
        let spec = guard_spec(
            "banded_catch_all",
            vec![("x", TypeRef::Int)],
            TypeRef::Int,
            vec![
                // {0, 1, 2} → 5
                pair_row(
                    vec![cmp_leaf("x", BinOp::Lt, 0), cmp_leaf("x", BinOp::Gt, 2)],
                    Expr::IntLit(5),
                ),
                // complement of {0, 1, 2} → 9
                pair_row(
                    vec![
                        cmp_leaf("x", BinOp::Eq, 0),
                        cmp_leaf("x", BinOp::Eq, 1),
                        cmp_leaf("x", BinOp::Eq, 2),
                    ],
                    Expr::IntLit(9),
                ),
            ],
        );
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let seed = constraint_seed(&engine, &spec, "guard_table");
        let expected = IRNode::Match {
            scrutinee: Box::new(IRNode::Var("x".into())),
            cases: vec![
                (IRPattern::Literal(IRLiteral::Int(0)), IRNode::Literal(IRLiteral::Int(5))),
                (IRPattern::Literal(IRLiteral::Int(1)), IRNode::Literal(IRLiteral::Int(5))),
                (IRPattern::Literal(IRLiteral::Int(2)), IRNode::Literal(IRLiteral::Int(5))),
                (IRPattern::Wildcard, IRNode::Literal(IRLiteral::Int(9))),
            ],
        };
        assert_eq!(format!("{:?}", seed.body), format!("{:?}", expected));
        for (x, want) in [(-100, 9), (-1, 9), (0, 5), (1, 5), (2, 5), (3, 9), (100, 9)] {
            let v = eval_int_body(&seed.body, "x", x);
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Int(want),
                "x {} must follow the band/catch-all table",
                x
            );
        }
        let result = crate::verify::verifier::Verifier::new().verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);
    }

    /// Conflicting band pins are not a table: two bands
    /// overlapping on {2, 3, 4} with different pins jointly
    /// determine no value there — no seed, not a first-wins
    /// body.
    #[test]
    fn constraint_guard_table_conflicting_bands_refused() {
        let spec = guard_spec(
            "conflicting_bands",
            vec![("x", TypeRef::Int)],
            TypeRef::Int,
            vec![
                pair_row(
                    vec![cmp_leaf("x", BinOp::Lt, 0), cmp_leaf("x", BinOp::Gt, 4)],
                    Expr::IntLit(1),
                ),
                pair_row(
                    vec![cmp_leaf("x", BinOp::Lt, 2), cmp_leaf("x", BinOp::Gt, 6)],
                    Expr::IntLit(2),
                ),
            ],
        );
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        assert!(
            engine.seed_from_constraints(&spec).is_empty(),
            "overlapping bands with different pins must get no constraint seed"
        );
    }

    /// The structural kill: a row-shaped postcondition the
    /// recovered table does not subsume is a DEVIATION (no
    /// seed), never a silent skip — where before Phase 37
    /// each spec below still seeded from its one good row,
    /// with the deviating row dropped unremarked.
    #[test]
    fn constraint_guard_table_row_shaped_deviations_refused() {
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let mut cases: Vec<FunctionSpec> = Vec::new();

        // A good single-value row beside a row-shaped row
        // whose pin is not a literal (`output == x` at
        // x == 5): the guard table's pin test refuses it,
        // and no other family states the pair (the
        // expression-pin family's territory requires a
        // compound pin; a bare Var is not one).
        cases.push(guard_spec(
            "row_with_var_pin",
            vec![("x", TypeRef::Int)],
            TypeRef::Int,
            vec![
                int_key_row("x", 0, Expr::IntLit(1)),
                pair_row(
                    vec![cmp_leaf("x", BinOp::Lt, 5), cmp_leaf("x", BinOp::Gt, 5)],
                    var_expr("x"),
                ),
            ],
        ));

        // A good row beside a row-shaped row whose guard leaf
        // does not parse on the parameter (`x == x`).
        cases.push(guard_spec(
            "row_with_unparseable_leaf",
            vec![("x", TypeRef::Int)],
            TypeRef::Int,
            vec![
                int_key_row("x", 0, Expr::IntLit(1)),
                or_row(vec![
                    eq_expr(var_expr("x"), var_expr("x")),
                    eq_expr(var_expr("output"), Expr::IntLit(2)),
                ]),
            ],
        ));

        // Bool domain: a good row beside a row-shaped row
        // whose pin is the parameter itself (not a literal).
        cases.push(guard_spec(
            "bool_row_with_var_pin",
            vec![("value", TypeRef::Bool)],
            TypeRef::String,
            vec![
                or_row(vec![
                    eq_expr(var_expr("value"), Expr::BoolLit(true)),
                    eq_expr(var_expr("output"), Expr::StringLit("a".into())),
                ]),
                or_row(vec![
                    eq_expr(var_expr("value"), Expr::BoolLit(false)),
                    eq_expr(var_expr("output"), var_expr("value")),
                ]),
            ],
        ));

        // Bool domain: two rows pinning the SAME domain value
        // to different literals — a conflict, not first-wins.
        // (The composition family's duplicate handling is the
        // same rule; see the Phase 37 audit note on its
        // builder.)
        cases.push(guard_spec(
            "bool_conflicting_rows",
            vec![("value", TypeRef::Bool)],
            TypeRef::String,
            vec![
                or_row(vec![
                    eq_expr(var_expr("value"), Expr::BoolLit(true)),
                    eq_expr(var_expr("output"), Expr::StringLit("a".into())),
                ]),
                or_row(vec![
                    eq_expr(var_expr("value"), Expr::BoolLit(true)),
                    eq_expr(var_expr("output"), Expr::StringLit("b".into())),
                ]),
            ],
        ));

        for spec in &cases {
            assert!(
                engine.seed_from_constraints(spec).is_empty(),
                "{}: a row-shaped deviation must get no constraint seed",
                spec.name
            );
        }
    }

    /// A genuinely NON-row postcondition stays skipped, as
    /// before — it is not a deviation. node_arity's
    /// `output >= 0` floor is the measured case; here a band
    /// row carries the same floor beside it.
    #[test]
    #[cfg(feature = "z3-support")] // Z3 verdict assertion (Phase 66 config gate)
    fn constraint_guard_table_band_row_with_non_row_postcondition() {
        let spec = guard_spec(
            "band_with_floor",
            vec![("x", TypeRef::Int)],
            TypeRef::Int,
            vec![
                // {0, 1} → 3
                pair_row(
                    vec![cmp_leaf("x", BinOp::Lt, 0), cmp_leaf("x", BinOp::Gt, 1)],
                    Expr::IntLit(3),
                ),
                Constraint::Expr(Expr::BinOp(
                    BinOp::Ge,
                    Box::new(var_expr("output")),
                    Box::new(Expr::IntLit(0)),
                )),
            ],
        );
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let seed = constraint_seed(&engine, &spec, "guard_table");
        for (x, want) in [(-5, 0), (0, 3), (1, 3), (2, 0)] {
            let v = eval_int_body(&seed.body, "x", x);
            assert_eq!(
                v,
                crate::synthesis::eval::Value::Int(want),
                "x {} must follow the band, with the floor skipped as a non-row",
                x
            );
        }
        let result = crate::verify::verifier::Verifier::new().verify_one(&seed, &spec);
        assert_eq!(result.status, VerificationStatus::Verified);
    }

    /// The Phase 37 audit's two dedup hardenings: in the
    /// composition and expression-pin builders a second row
    /// pinning an already-pinned guard/key value was dropped
    /// first-wins even when its pin DIFFERED — a row-shaped
    /// row contributing no arm and not subsumed, the
    /// Finding 1 disease in miniature. An identical pin is
    /// subsumed and still skipped; a different one is now a
    /// deviation.
    #[test]
    fn constraint_duplicate_pin_conflicts_refused() {
        let engine = SynthesisEngine::new(SynthesisConfig::default());

        // Expression-pin: two rows pin key 0 to different
        // Concat templates.
        let spec = guard_spec(
            "expr_pin_duplicate_key",
            vec![("code", TypeRef::Int), ("text", TypeRef::String)],
            TypeRef::String,
            vec![
                int_key_row("code", 0, concat_pin(var_expr("text"), "a")),
                int_key_row("code", 0, concat_pin(var_expr("text"), "b")),
            ],
        );
        assert!(
            engine.seed_from_constraints(&spec).is_empty(),
            "conflicting duplicate key rows must get no constraint seed"
        );

        // Composition: two rows pin guard value false (under
        // a plain Bool parameter) to different Concat pins.
        let spec = guard_spec(
            "composition_duplicate_value",
            vec![("flag", TypeRef::Bool), ("text", TypeRef::String)],
            TypeRef::String,
            vec![
                or_row(vec![
                    eq_expr(var_expr("flag"), Expr::BoolLit(true)),
                    eq_expr(var_expr("output"), concat_pin(var_expr("text"), "a")),
                ]),
                or_row(vec![
                    eq_expr(var_expr("flag"), Expr::BoolLit(true)),
                    eq_expr(var_expr("output"), concat_pin(var_expr("text"), "b")),
                ]),
            ],
        );
        assert!(
            engine.seed_from_constraints(&spec).is_empty(),
            "conflicting duplicate guard-value rows must get no constraint seed"
        );
    }

    // ── Generation-side type discipline (Phase 12) ─────────
    //
    // The Phase 11 return-type gate can only crown what the
    // pool contains: in roughly half of draws a test-less
    // spec's hole-free pool was uniformly mistyped, because
    // random seed bodies were generated type-blind — `(actual
    // + expected)` or `return actual;` as a Bool body, a bare
    // `for` loop as a Bool body. The seeds are the pool's
    // foundation (MCTS completions inherit their root shape),
    // so generation itself must be type-aware: a body drawn
    // for a typed spec must never be PROVABLY mistyped under
    // selector::return_type_verdict.

    fn gen_spec(
        name: &str,
        params: Vec<(&str, TypeRef)>,
        return_type: TypeRef,
    ) -> FunctionSpec {
        let mut spec = dummy_spec();
        spec.name = name.into();
        spec.params = params
            .into_iter()
            .map(|(n, ty)| Param {
                name: n.into(),
                ty,
                annotations: HashMap::new(),
            })
            .collect();
        spec.return_type = return_type;
        spec
    }

    /// Mirrors the M2 self-host specs' signatures (all-Int
    /// params; Bool/Int/String returns), including both
    /// test-less multi-param specs (tags_compatible,
    /// needs_parens).
    fn m2_signature_specs() -> Vec<FunctionSpec> {
        vec![
            gen_spec("is_known_type_tag", vec![("tag", TypeRef::Int)], TypeRef::Bool),
            gen_spec("node_arity", vec![("kind", TypeRef::Int)], TypeRef::Int),
            gen_spec("type_tag_name", vec![("tag", TypeRef::Int)], TypeRef::String),
            gen_spec(
                "tags_compatible",
                vec![("expected", TypeRef::Int), ("actual", TypeRef::Int)],
                TypeRef::Bool,
            ),
            gen_spec(
                "needs_parens",
                vec![("op", TypeRef::Int), ("parent_op", TypeRef::Int)],
                TypeRef::Bool,
            ),
        ]
    }

    /// Property: random seed bodies (the production seeding
    /// path, `seed_random`) are never provably mistyped for
    /// their spec — and generation has not degenerated to
    /// only-unprovable shapes: some draws are provably right.
    #[test]
    fn random_seed_bodies_are_never_provably_mistyped() {
        use crate::synthesis::selector::{return_type_verdict, TypeVerdict};
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        for spec in m2_signature_specs() {
            let (mut mismatched, mut matches_ct, mut unknown) = (0usize, 0usize, 0usize);
            const N: usize = 200;
            for _ in 0..N {
                let candidate = engine.seed_random(&spec);
                match return_type_verdict(&candidate.body, &spec) {
                    TypeVerdict::Mismatched => mismatched += 1,
                    TypeVerdict::Matches => matches_ct += 1,
                    TypeVerdict::Unknown => unknown += 1,
                }
            }
            assert_eq!(
                mismatched, 0,
                "{}: {}/{} random seed bodies provably mistyped ({} Matches, {} Unknown)",
                spec.name, mismatched, N, matches_ct, unknown
            );
            assert!(
                matches_ct > 0,
                "{}: no provably-matching seed body in {} draws — generation degenerated to unprovable shapes",
                spec.name, N
            );
        }
    }

    /// Regression: the legacy untyped generator keeps its
    /// historical behavior — it is type-blind, so over many
    /// draws it still produces the shapes the typed path
    /// forbids as bodies for a non-Unit spec (statement-only
    /// roots, provably mistyped bodies). Callers without a
    /// type context are untouched by the typed path.
    #[test]
    fn random_ir_tree_legacy_path_stays_type_blind() {
        use crate::synthesis::selector::{return_type_verdict, TypeVerdict};
        let spec = gen_spec("legacy_bool", vec![("tag", TypeRef::Int)], TypeRef::Bool);
        let engine = SynthesisEngine::new(SynthesisConfig::default());
        let mut legacy_shapes = 0usize;
        for _ in 0..200 {
            let tree = random_ir_tree(&spec, 3, &engine);
            if matches!(tree, IRNode::For { .. } | IRNode::While { .. })
                || return_type_verdict(&tree, &spec) == TypeVerdict::Mismatched
            {
                legacy_shapes += 1;
            }
        }
        assert!(
            legacy_shapes > 0,
            "legacy generator produced no type-blind shapes in 200 draws — the untyped path changed"
        );
    }

    // ── expr_to_ir lowerings for the M1 features (Phase 73) ──
    //
    // Before Phase 73, `expr_to_ir` lowered Field and Index
    // into the Hole arm — the direct cause of Phase 48's
    // "no constraint seed reached any pool (24/24 seedless)"
    // for the record specs (M1_FEATURES_AUDIT section 2.1).
    // Under D2a/D3a the lowering is now real for Field (to
    // IRNode::FieldAccess, kind 28), Index (to the existing
    // IRNode::Index, kind 23 — the node existed; only the
    // lowering refused it) and RecordLit (to IRNode::RecordLit,
    // kind 29). MethodCall lowering stays a Hole this phase:
    // the IR kind table has no method-call kind (audit section
    // 4 appends only 28/29), and method semantics land in the
    // typechecker (Phase 74) and the Z3 encoder (Phase 76),
    // which both work on the spec Expr directly. The Hole is
    // pinned here so the deferral is a recorded fact, not a
    // silent drop.

    #[test]
    fn expr_to_ir_lowers_field_access() {
        let expr = Expr::Field(Box::new(Expr::Var("state".into())), "pos".into());
        let ir = expr_to_ir(&expr);
        assert!(
            matches!(&ir, IRNode::FieldAccess { base, field }
                if field == "pos" && matches!(&**base, IRNode::Var(v) if v == "state")),
            "state.pos must lower to FieldAccess, got {:?}",
            ir
        );
    }

    #[test]
    fn expr_to_ir_lowers_index_over_field() {
        // state.source[state.pos] — peek's indexing shape:
        // Index over a FieldAccess over a Var, with a
        // FieldAccess index.
        let expr = Expr::Index(
            Box::new(Expr::Field(Box::new(Expr::Var("state".into())), "source".into())),
            Box::new(Expr::Field(Box::new(Expr::Var("state".into())), "pos".into())),
        );
        let ir = expr_to_ir(&expr);
        assert!(
            matches!(&ir, IRNode::Index(coll, idx)
                if matches!(&**coll, IRNode::FieldAccess { field, .. } if field == "source")
                    && matches!(&**idx, IRNode::FieldAccess { field, .. } if field == "pos")),
            "state.source[state.pos] must lower to Index(FieldAccess, FieldAccess), got {:?}",
            ir
        );
    }

    #[test]
    fn expr_to_ir_lowers_record_literal() {
        let expr = Expr::RecordLit {
            type_name: "Token".into(),
            fields: vec![
                ("kind".into(), Expr::IntLit(0)),
                ("lexeme".into(), Expr::StringLit("spec".into())),
            ],
        };
        let ir = expr_to_ir(&expr);
        let IRNode::RecordLit { type_name, fields } = &ir else {
            panic!("record literal must lower to IRNode::RecordLit, got {:?}", ir);
        };
        assert_eq!(type_name, "Token");
        assert_eq!(fields.len(), 2);
        assert!(
            matches!(&fields[0].1, IRNode::Literal(IRLiteral::Int(0))),
            "kind field must lower to an Int literal, got {:?}",
            fields[0].1
        );
        assert!(
            matches!(&fields[1].1, IRNode::Literal(IRLiteral::String(s)) if s == "spec"),
            "lexeme field must lower to a String literal, got {:?}",
            fields[1].1
        );
    }

    #[test]
    fn expr_to_ir_method_call_lowers_to_hole_no_ir_kind() {
        // STANDING, re-pinned by Phase 76 (was
        // `..._pending_74_76`): the deferral it named is
        // complete — the typechecker resolves methods
        // against the D1a table (Phase 74) and the Z3
        // encoder encodes MethodCall on the spec Expr
        // against the Result/Option tag+payload forms
        // (Phase 76), as does the interpreter's
        // value_of_expr (Phase 75). The lowering STILL
        // produces the Hole, now as a design fact rather
        // than a deferral: the IR kind table has no
        // method-call kind (the audit's section 4 appended
        // only FieldAccess/RecordLit, kinds 28/29), and
        // every consumer of method semantics works on the
        // spec Expr directly. A constraint-derived seed
        // built from a method-call constraint therefore
        // carries a hole where the call was — recorded
        // here so a future phase that adds an IR
        // method-call kind updates this test deliberately.
        let expr = Expr::MethodCall {
            receiver: Box::new(Expr::Var("output".into())),
            name: "is_err".into(),
            args: vec![],
        };
        let ir = expr_to_ir(&expr);
        assert!(
            matches!(ir, IRNode::Hole { .. }),
            "method call lowering is the Hole (no IR kind), got {:?}",
            ir
        );
    }
}

