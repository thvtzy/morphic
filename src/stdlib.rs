// ─────────────────────────────────────────────────────────────
//  THE BUNDLED STANDARD LIBRARY (Phase 110, decision D2a)
//
//  The 1.0 standard library ships as `.morph` sources bundled
//  with the compiler (the `stdlib/` directory, embedded below
//  exactly as the playground corpus examples are embedded in
//  the API's tests) and is delivered as a PRELUDE: the library's
//  specs are merged into every user module at the two points
//  every front door shares —
//
//    * the typechecker registers the library's signatures
//      before the module's own (spec/typeck.rs), so a call to
//      an entry typechecks with no import statement; and
//    * `SpecIndex::from_specs` / `from_module` merge the
//      library's specs into the module's index (synthesis/
//      engine.rs), so the call resolves when a body is judged.
//
//  CLI, library API and playground/WASM therefore see the same
//  library, because all three funnel through those two points.
//
//  COLLISION RULE (Phase 110, recorded in the ROAD note): a
//  user module's own definition of a name WINS over the
//  prelude entry of that name — the prelude spec is not merged
//  at all for a colliding name, and its shipped body is not
//  registered. The user's definition is never silently
//  overridden; the rule is the typechecker's standing
//  module-over-stdlib precedence, applied to the merge itself,
//  and is pinned by tests/phase110_prelude.rs.
//
//  BODIES AS DATA: a library entry is a spec + its crowned
//  body, shipped as data (STDLIB_AUDIT §4.3). Bodies are
//  crowned at library-build time — the production pipeline,
//  seeded profile, seed 42, measured in the phase that ships
//  the entry — and transcribed here as IR constructions (the
//  Phase 92 denotation precedent). The registry below is the
//  ONLY source of shipped bodies; `SpecIndex::callee_body`
//  consults it before any row-derived denotation, because the
//  shipped body is what the entry's name denotes. The pin
//  (tests/phase110_prelude.rs): each shipped body judges
//  Z3-Verified ∧ golden-exact against its own spec, and equals
//  the body the pipeline crowns for that entry.
// ─────────────────────────────────────────────────────────────

use std::sync::OnceLock;

use crate::spec::ast::{FunctionSpec, Spec};
use crate::synthesis::engine::{IRBinOp, IRLiteral, IRNode};

/// The bundled library modules: (module name, source text).
/// Sources live in the repo's `stdlib/` directory and are
/// embedded into every build — native binary and WASM alike —
/// by `include_str!`, the codebase's standing embedding
/// mechanism. One module per family (STDLIB_SCOPE §2); this
/// phase ships the Int module with its single proof entry.
pub const LIBRARY_MODULES: &[(&str, &str)] = &[("int", include_str!("../stdlib/int.morph"))];

/// The bundled library, parsed once per process and merged
/// into one module. A bundled source that fails to parse is
/// a compiler bug, not a user error — the phase tests pin
/// that every module parses and typechecks standalone, so
/// the panic here is unreachable on a green tree.
fn library_module() -> &'static Spec {
    static LIBRARY: OnceLock<Spec> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let mut functions = Vec::new();
        let mut types = Vec::new();
        let mut imports = Vec::new();
        for (name, source) in LIBRARY_MODULES {
            let module = crate::spec::parser::parse(source).unwrap_or_else(|e| {
                panic!("bundled stdlib module '{name}' must parse: {e}")
            });
            functions.extend(module.functions);
            types.extend(module.types);
            imports.extend(module.imports);
        }
        Spec {
            name: "Stdlib".to_string(),
            imports,
            functions,
            types,
            invariants: Vec::new(),
        }
    })
}

/// The library's specs — the prelude's signature surface.
/// The typechecker registers these in every module's
/// environment; the SpecIndex merge inserts the same specs
/// into every module's index.
pub fn prelude_specs() -> Vec<FunctionSpec> {
    library_module().functions.clone()
}

/// The crowned body shipped for the library entry `name`,
/// or None when the entry carries no shipped body (its name
/// then denotes whatever its spec's rows derive, like any
/// spec — no entry ships that way this phase).
pub fn shipped_body(name: &str) -> Option<IRNode> {
    match name {
        // Crowned at library-build time in Phase 110 (seeded
        // pipeline, seed 42): Z3-Verified, 4/4 evaluable
        // in-spec tests passed, the Int-minimum point
        // spec-silent under the entry's precondition.
        "int_abs" => Some(IRNode::If {
            condition: Box::new(IRNode::BinOp(
                IRBinOp::Lt,
                Box::new(IRNode::Var("x".into())),
                Box::new(IRNode::Literal(IRLiteral::Int(0))),
            )),
            then_branch: Box::new(IRNode::BinOp(
                IRBinOp::Sub,
                Box::new(IRNode::Literal(IRLiteral::Int(0))),
                Box::new(IRNode::Var("x".into())),
            )),
            else_branch: Box::new(IRNode::Var("x".into())),
        }),
        _ => None,
    }
}
