// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC AST — Abstract Syntax Tree                       │
// │  The internal representation of a Morphic specification   │
// └──────────────────────────────────────────────────────────┘

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A complete Morphic specification file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub name: String,
    pub imports: Vec<Import>,
    pub functions: Vec<FunctionSpec>,
    pub types: Vec<TypeDef>,
    pub invariants: Vec<GlobalInvariant>,
}

/// Import declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    pub path: String,
    pub alias: Option<String>,
}

/// A single function specification
///
/// This is the core unit of Morphic. Users declare WHAT a function does,
/// not HOW. The compiler synthesizes the HOW.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSpec {
    pub name: String,
    pub doc: Option<String>,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_type: TypeRef,
    pub preconditions: Vec<Constraint>,
    pub postconditions: Vec<Constraint>,
    pub invariants: Vec<Invariant>,
    pub complexity: Option<ComplexityBound>,
    pub resource: Option<ResourceBound>,
    pub tests: Vec<Test>,
    pub annotations: HashMap<String, String>,
}

/// A function parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: TypeRef,
    pub annotations: HashMap<String, String>,
}

/// A named type reference
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TypeRef {
    /// Built-in primitive
    Int, Float, Bool, String, Char, Unit,
    /// Fixed-width integers
    I8, I16, I32, I64,
    U8, U16, U32, U64,
    /// Generic type parameter
    Generic(String),
    /// User-defined named type
    Named(String),
    /// Container types
    List(Box<TypeRef>),
    Set(Box<TypeRef>),
    Map(Box<TypeRef>, Box<TypeRef>),
    Option(Box<TypeRef>),
    Result(Box<TypeRef>, Box<TypeRef>),
    Tuple(Vec<TypeRef>),
    Array(Box<TypeRef>, usize),
    /// Function type
    Function(Vec<TypeRef>, Box<TypeRef>),
    /// Reference / pointer
    Ref(Box<TypeRef>),
    /// Stream / iterator
    Stream(Box<TypeRef>),
}

impl std::fmt::Display for TypeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeRef::Int => write!(f, "Int"),
            TypeRef::Float => write!(f, "Float"),
            TypeRef::Bool => write!(f, "Bool"),
            TypeRef::String => write!(f, "String"),
            TypeRef::Char => write!(f, "Char"),
            TypeRef::Unit => write!(f, "()"),
            TypeRef::I8 => write!(f, "I8"),
            TypeRef::I16 => write!(f, "I16"),
            TypeRef::I32 => write!(f, "I32"),
            TypeRef::I64 => write!(f, "I64"),
            TypeRef::U8 => write!(f, "U8"),
            TypeRef::U16 => write!(f, "U16"),
            TypeRef::U32 => write!(f, "U32"),
            TypeRef::U64 => write!(f, "U64"),
            TypeRef::Generic(n) => write!(f, "{}", n),
            TypeRef::Named(n) => write!(f, "{}", n),
            TypeRef::List(t) => write!(f, "List<{}>", t),
            TypeRef::Set(t) => write!(f, "Set<{}>", t),
            TypeRef::Map(k, v) => write!(f, "Map<{}, {}>", k, v),
            TypeRef::Option(t) => write!(f, "Option<{}>", t),
            TypeRef::Result(ok, err) => write!(f, "Result<{}, {}>", ok, err),
            TypeRef::Tuple(ts) => {
                write!(f, "(")?;
                for (i, t) in ts.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", t)?;
                }
                write!(f, ")")
            }
            TypeRef::Array(t, n) => write!(f, "[{}; {}]", t, n),
            TypeRef::Function(args, ret) => {
                write!(f, "fn(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", a)?;
                }
                write!(f, ") -> {}", ret)
            }
            TypeRef::Ref(t) => write!(f, "&{}", t),
            TypeRef::Stream(t) => write!(f, "Stream<{}>", t),
        }
    }
}

/// A generic type parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<TypeBound>,
}

/// A type constraint for generics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeBound {
    Trait(String),
    Sized,
    Copy,
    Send,
    Sync,
}

/// A constraint (precondition, postcondition, or invariant)
///
/// Constraints are logical formulas that must hold. They can be:
/// - Simple boolean expressions
/// - Quantified formulas (forall, exists)
/// - Ordering constraints on complexity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    /// Always true — placeholder
    True,

    /// A boolean expression (string form for now; will be lowered to IR)
    Expr(Expr),

    /// Forall quantified constraint
    Forall {
        vars: Vec<(String, TypeRef)>,
        body: Box<Constraint>,
    },

    /// Exists quantified constraint
    Exists {
        vars: Vec<(String, TypeRef)>,
        body: Box<Constraint>,
    },

    /// Implication: if antecedent holds, consequent must hold
    Implies(Box<Constraint>, Box<Constraint>),

    /// Conjunction of constraints
    And(Vec<Constraint>),

    /// Disjunction of constraints
    Or(Vec<Constraint>),

    /// Negation
    Not(Box<Constraint>),

    /// Equality: a == b
    Eq(Expr, Expr),

    /// Ordering: a <= b, a < b, etc.
    Order(OrderOp, Expr, Expr),

    /// Predicate call, e.g. is_sorted(result)
    Predicate(String, Vec<Expr>),
}

/// A boolean/logical expression node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    /// Variable reference (e.g., `x`, `result`, `input`)
    Var(String),
    /// Integer literal
    IntLit(i64),
    /// Float literal
    FloatLit(f64),
    /// Boolean literal
    BoolLit(bool),
    /// String literal
    StringLit(String),
    /// Field access: expr.field
    Field(Box<Expr>, String),
    /// Index access: expr[index]
    Index(Box<Expr>, Box<Expr>),
    /// Function call: func(args...)
    Call(String, Vec<Expr>),
    /// Binary arithmetic operation
    BinOp(BinOp, Box<Expr>, Box<Expr>),
    /// Unary operation
    UnaryOp(UnaryOp, Box<Expr>),
    /// Lambda expression
    Lambda(Vec<(String, TypeRef)>, Box<Expr>),
    /// Length of a collection
    Length(Box<Expr>),
    /// Collection comprehension
    Comprehension {
        binder: String,
        collection: Box<Expr>,
        body: Box<Expr>,
    },
}

/// Binary arithmetic/comparison operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    And, Or,
    Eq, Neq, Lt, Le, Gt, Ge,
    Concat,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg, Not,
}

/// Comparison ordering operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderOp {
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

/// An invariant — a property that must always hold
///
/// Unlike pre/postconditions, invariants hold at all times.
/// For mutable data structures, this includes intermediate states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invariant {
    pub name: Option<String>,
    pub constraint: Constraint,
}

/// A global invariant — applies across multiple functions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalInvariant {
    pub name: String,
    pub constraint: Constraint,
}

/// Bounds on computational complexity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityBound {
    pub dimension: ComplexityDimension,
    pub bound: BigO,
    pub condition: Option<Constraint>, // Only enforce under certain conditions
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComplexityDimension {
    /// Time complexity
    Time,
    /// Space complexity
    Space,
    /// Amortized
    AmortizedTime,
    /// Communication complexity
    Communication,
}

/// Big-O notation representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BigO {
    Constant,
    Logarithmic,
    Linear,
    Linearithmic, // O(n log n)
    Quadratic,
    Cubic,
    Polynomial(f64),   // O(n^k)
    Exponential(f64),  // O(k^n)
    Custom(String),
}

impl std::fmt::Display for BigO {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BigO::Constant => write!(f, "O(1)"),
            BigO::Logarithmic => write!(f, "O(log n)"),
            BigO::Linear => write!(f, "O(n)"),
            BigO::Linearithmic => write!(f, "O(n log n)"),
            BigO::Quadratic => write!(f, "O(n²)"),
            BigO::Cubic => write!(f, "O(n³)"),
            BigO::Polynomial(k) => write!(f, "O(n^{})", k),
            BigO::Exponential(k) => write!(f, "O({}^n)", k),
            BigO::Custom(s) => write!(f, "O({})", s),
        }
    }
}

/// Resource bounds (memory, allocations, I/O, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceBound {
    pub resource: Resource,
    pub max_amount: Option<u64>,
    pub predicate: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Resource {
    MemoryBytes,
    Allocations,
    Syscalls,
    NetworkIO,
    DiskIO,
}

/// A test case for the specification
///
/// Tests serve double duty:
/// 1. They validate that synthesized implementations behave correctly
/// 2. They can be used as "anchors" for the synthesis engine (known input/output pairs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Test {
    pub name: Option<String>,
    pub input: Expr,
    pub expected_output: Expr,
    pub timeout_ms: Option<u64>,
    /// If true, this is a property-based test
    pub property: bool,
}

/// A user-defined type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDef {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub kind: TypeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeKind {
    /// Algebraic Data Type (enum/sum type)
    ADT {
        variants: Vec<Variant>,
    },
    /// Record (struct/product type)
    Record {
        fields: Vec<(String, TypeRef)>,
    },
    /// Type alias
    Alias(TypeRef),
    /// Newtype wrapper
    Newtype(TypeRef),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<(String, TypeRef)>,
}
