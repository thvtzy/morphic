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
//  body, shipped as data (STDLIB_AUDIT §4.3). Tier-1 bodies
//  are crowned at library-build time — the production
//  pipeline, seeded profile, seed 42, measured in the phase
//  that ships the entry — and transcribed here as IR
//  constructions (the Phase 92 denotation precedent); the
//  Tier-2 List bodies (Phase 115) are the recorded hand
//  bodies at their tier's executed standard, the pipeline
//  crowning no Verified body for them. The registry below
//  is the ONLY source of shipped bodies; `SpecIndex::
//  callee_body` consults it before any row-derived
//  denotation, because the shipped body is what the entry's
//  name denotes. The pins: each Tier-1 shipped body judges
//  Z3-Verified ∧ golden-exact against its own spec and
//  equals the body the pipeline crowns for that entry
//  (tests/phase110_prelude.rs); each Tier-2 body is
//  executed-exact on its full battery through both
//  executed judges (tests/phase115_list.rs).
//
//  THE ASSET FORM (Phase 112): the WASM playground bundle
//  stood 1,290 bytes from its frozen budget (771,072 B) with
//  the List family still to ship, so under the
//  `wasm-playground` feature the library's DATA — the module
//  sources and the shipped bodies — is not compiled into the
//  bundle at all: the prelude is built from a text asset
//  (LibraryAsset below) that the page fetches and installs
//  via `set_library_asset` before any other call. Native
//  builds (CLI, API, every test configuration) keep the
//  embedded form below, byte-for-byte the behaviour they
//  always were. The asset is generated from the native data
//  by the hidden `morphic stdlib-asset` CLI subcommand and
//  checked in at playground/stdlib-asset.bin; the pin
//  (tests/phase112_stdlib_asset.rs) regenerates it in memory
//  and asserts the checked-in file matches byte-for-byte,
//  so the two forms cannot drift.
//
//  THE INSTALLED LAYER (Phase 165, DISEBAR_17_SCOPE §7):
//  registry-installed bundles (Phase 164) join the prelude
//  as a third layer behind this same funnel —
//  `prelude_specs` yields the stdlib's specs then the
//  installed set's, and `shipped_body` answers stdlib
//  names from the registry below and installed names from
//  the installed layer, never across the layer line.
//  Precedence is user > stdlib > installed (§6): the
//  merges above already let a user definition shadow the
//  whole prelude, and the installed layer never yields a
//  name the stdlib holds (a hand-planted stdlib-named
//  install is inert, named in the layer's diagnostics).
//  Native builds only: the layer reads the install
//  directory once per process, and the playground build —
//  whose prelude arrives as the runtime asset — has no
//  filesystem and no installed layer. Discovery does NOT
//  re-verify (install ran the proof; `registry verify`
//  re-checks it): see load_installed_layer's trust note.
// ─────────────────────────────────────────────────────────────

use std::sync::OnceLock;

use crate::spec::ast::{FunctionSpec, Spec, TypeRef};
use crate::synthesis::engine::{
    CollectionKind, IRBinOp, IRLiteral, IRNode, IRPattern, IRUnaryOp,
};

/// The bundled library modules: (module name, source text).
/// Sources live in the repo's `stdlib/` directory and are
/// embedded into native builds by `include_str!`, the
/// codebase's standing embedding mechanism. One module per
/// family (STDLIB_SCOPE §2): the Int module (Phase 110's
/// spine, completed in Phase 111), the Char module
/// (Phase 111), the String / Result / Option modules
/// (Phase 112), and the List module (Phase 115). Under the `wasm-playground` feature this
/// table does not exist — the same sources arrive in the
/// runtime asset instead (the banner comment above).
#[cfg(not(feature = "wasm-playground"))]
pub const LIBRARY_MODULES: &[(&str, &str)] = &[
    ("int", include_str!("../stdlib/int.morph")),
    ("char", include_str!("../stdlib/char.morph")),
    ("string", include_str!("../stdlib/string.morph")),
    ("result", include_str!("../stdlib/result.morph")),
    ("option", include_str!("../stdlib/option.morph")),
    ("list", include_str!("../stdlib/list.morph")),
];

// ── The asset form (Phase 112) ───────────────────────────

/// The standard library as portable data: the module
/// sources (parsed once at load, exactly as the embedded
/// form is) and the shipped bodies, encoded as IR. This
/// is the playground's separately-loaded asset format;
/// native builds generate it from the embedded data
/// ([`library_asset`]).
#[derive(Debug, Clone)]
pub struct LibraryAsset {
    pub modules: Vec<AssetModule>,
    /// (entry name, shipped body) in prelude order.
    pub bodies: Vec<(String, IRNode)>,
}

/// One bundled module in asset form.
#[derive(Debug, Clone)]
pub struct AssetModule {
    pub name: String,
    pub source: String,
}

/// The installed runtime asset (playground builds).
/// Set once, before any prelude use; native builds
/// never install one and never consult the slot.
fn asset_slot() -> &'static OnceLock<LibraryAsset> {
    static SLOT: OnceLock<LibraryAsset> = OnceLock::new();
    &SLOT
}

/// Install the standard-library asset from its wire
/// bytes. The playground shell calls this once, right
/// after the WASM initializes and before any check or
/// synthesis call; installing twice, or installing
/// malformed bytes, is an error. Native builds accept
/// the call (the slot exists) but never consult it —
/// their prelude is the embedded data.
pub fn set_library_asset(bytes: &[u8]) -> Result<(), String> {
    let asset = parse_library_asset(bytes)?;
    asset_slot()
        .set(asset)
        .map_err(|_| "stdlib asset already installed".to_string())
}

/// The installed asset, if any (playground builds).
#[cfg(feature = "wasm-playground")]
fn installed_asset() -> Option<&'static LibraryAsset> {
    asset_slot().get()
}

/// The library asset built from this build's own data:
/// natively, the embedded modules and shipped bodies;
/// in a playground build, a round-trip of the installed
/// asset. The generator surface for
/// `playground/stdlib-asset.bin`.
pub fn library_asset() -> LibraryAsset {
    LibraryAsset {
        modules: module_sources()
            .into_iter()
            .map(|(name, source)| AssetModule { name, source })
            .collect(),
        // The asset is the STDLIB's alone (Phase 165):
        // it is built from the stdlib layer's specs, so
        // a machine's installed entries — a native,
        // per-user layer — never enter the playground's
        // checked-in asset.
        bodies: stdlib_specs()
            .iter()
            .filter_map(|spec| shipped_body(&spec.name).map(|body| (spec.name.clone(), body)))
            .collect(),
    }
}

/// [`library_asset`] in its wire form — the checked-in
/// asset file is exactly these bytes (a pin test holds
/// them equal). The format is a small hand-rolled BINARY
/// codec, not JSON/serde and not a text grammar — the
/// measured reasons are on IRNode's docs in
/// synthesis/engine.rs: a serde deserialization graph
/// costs 161,717 bundle bytes, and a text recursive-
/// descent parser over the IR's 30 node kinds costs
/// 30,189. What fits the bundle is a stack machine:
/// bodies are postfix opcode streams that one tight
/// builder loop interprets — no string matching, no
/// formatted errors, every name an index into a shared
/// string table.
///
/// Wire layout: the magic line ASSET_MAGIC, then
/// sections (one tag byte + u32 LE payload length +
/// payload):
///   'S' string table — u32 count, then per string a
///       u32 byte length + UTF-8 bytes;
///   'M' module — u32 name index, u32 source byte
///       length + source bytes;
///   'B' body — u32 name index, u32 encoding byte
///       length + the encoding;
///   'E' end (no length, no payload).
/// Inside an encoding, children precede their opcode
/// (postfix); the opcode's immediates (counts, string
/// indices, inline type streams) follow it. Patterns
/// build on a second stack inside the same stream.
/// The writer and the builder share the opcode
/// constants and op tables below, so the two sides
/// cannot disagree about numbering.
#[cfg(not(feature = "wasm-playground"))]
pub fn library_asset_bytes() -> Vec<u8> {
    let asset = library_asset();
    let mut writer = AssetWriter::default();
    let mut sections: Vec<u8> = Vec::new();
    for module in &asset.modules {
        let name_idx = writer.intern(&module.name);
        let mut payload = Vec::new();
        push_u32(&mut payload, name_idx);
        push_u32(&mut payload, module.source.len() as u32);
        payload.extend_from_slice(module.source.as_bytes());
        push_section(&mut sections, b'M', &payload);
    }
    for (name, body) in &asset.bodies {
        let name_idx = writer.intern(name);
        let mut encoding: Vec<u8> = Vec::new();
        write_node(body, &mut writer, &mut encoding);
        let mut payload = Vec::new();
        push_u32(&mut payload, name_idx);
        push_u32(&mut payload, encoding.len() as u32);
        payload.extend_from_slice(&encoding);
        push_section(&mut sections, b'B', &payload);
    }
    let mut table_payload = Vec::new();
    push_u32(&mut table_payload, writer.strings.len() as u32);
    for string in &writer.strings {
        push_u32(&mut table_payload, string.len() as u32);
        table_payload.extend_from_slice(string.as_bytes());
    }
    let mut out = Vec::new();
    out.extend_from_slice(ASSET_MAGIC);
    push_section(&mut out, b'S', &table_payload);
    out.extend_from_slice(&sections);
    out.push(b'E');
    out
}

const ASSET_MAGIC: &[u8] = b"MORPHIC-STDLIB-ASSET 1\n";

// Node opcodes (postfix stream; see the module docs of
// library_asset_bytes).
const OP_VAR: u8 = 0x00;
const OP_INT: u8 = 0x01;
const OP_FLOAT: u8 = 0x02;
const OP_BOOL: u8 = 0x03;
const OP_CHAR: u8 = 0x04;
const OP_STR: u8 = 0x05;
const OP_UNIT: u8 = 0x06;
const OP_BINOP: u8 = 0x07;
const OP_UNOP: u8 = 0x08;
const OP_CALL: u8 = 0x09;
const OP_IF: u8 = 0x0A;
const OP_LET: u8 = 0x0B;
const OP_MATCH: u8 = 0x0C;
const OP_BLOCK: u8 = 0x0D;
const OP_RETURN: u8 = 0x0E;
const OP_HOLE: u8 = 0x0F;
const OP_WHILE: u8 = 0x10;
const OP_FOR: u8 = 0x11;
const OP_LAMBDA: u8 = 0x12;
const OP_CLOSURE: u8 = 0x13;
const OP_COLLECTION: u8 = 0x14;
const OP_MAP: u8 = 0x15;
const OP_FILTER: u8 = 0x16;
const OP_FOLD: u8 = 0x17;
const OP_SORT: u8 = 0x18;
const OP_ALLOC: u8 = 0x19;
const OP_ASSIGN: u8 = 0x1A;
const OP_TYPED: u8 = 0x1B;
const OP_INDEX: u8 = 0x1C;
const OP_LOOP: u8 = 0x1D;
const OP_BREAK: u8 = 0x1E;
const OP_STRUCTDEF: u8 = 0x1F;
const OP_TRYPROP: u8 = 0x20;
const OP_FIELD: u8 = 0x21;
const OP_RECORDLIT: u8 = 0x22;
// Pattern opcodes (the pattern stack in the same stream).
const OP_PWILD: u8 = 0x30;
const OP_PVAR: u8 = 0x31;
const OP_PLIT: u8 = 0x32;
const OP_PCTOR: u8 = 0x33;
const OP_PGUARD: u8 = 0x34;
// Type tags (inline streams inside opcodes).
const TY_INT: u8 = 0x00;
const TY_FLOAT: u8 = 0x01;
const TY_BOOL: u8 = 0x02;
const TY_STRING: u8 = 0x03;
const TY_CHAR: u8 = 0x04;
const TY_UNIT: u8 = 0x05;
const TY_I8: u8 = 0x06;
const TY_I16: u8 = 0x07;
const TY_I32: u8 = 0x08;
const TY_I64: u8 = 0x09;
const TY_U8: u8 = 0x0A;
const TY_U16: u8 = 0x0B;
const TY_U32: u8 = 0x0C;
const TY_U64: u8 = 0x0D;
const TY_GENERIC: u8 = 0x0E;
const TY_NAMED: u8 = 0x0F;
const TY_LIST: u8 = 0x10;
const TY_SET: u8 = 0x11;
const TY_MAP: u8 = 0x12;
const TY_OPTION: u8 = 0x13;
const TY_RESULT: u8 = 0x14;
const TY_TUPLE: u8 = 0x15;
const TY_ARRAY: u8 = 0x16;
const TY_FUNCTION: u8 = 0x17;
const TY_REF: u8 = 0x18;
const TY_STREAM: u8 = 0x19;

/// Op numbering shared by the codec's two sides.
const BINOP_TABLE: &[IRBinOp] = &[
    IRBinOp::Add,
    IRBinOp::Sub,
    IRBinOp::Mul,
    IRBinOp::Div,
    IRBinOp::Mod,
    IRBinOp::And,
    IRBinOp::Or,
    IRBinOp::Xor,
    IRBinOp::Eq,
    IRBinOp::Neq,
    IRBinOp::Lt,
    IRBinOp::Le,
    IRBinOp::Gt,
    IRBinOp::Ge,
    IRBinOp::Shl,
    IRBinOp::Shr,
    IRBinOp::Concat,
    IRBinOp::Append,
    IRBinOp::Merge,
];

const UNOP_TABLE: &[IRUnaryOp] = &[
    IRUnaryOp::Neg,
    IRUnaryOp::Not,
    IRUnaryOp::Abs,
    IRUnaryOp::Len,
    IRUnaryOp::Head,
    IRUnaryOp::Tail,
    IRUnaryOp::Init,
    IRUnaryOp::Last,
    IRUnaryOp::Reverse,
    IRUnaryOp::Clone,
];

const KIND_TABLE: &[CollectionKind] = &[
    CollectionKind::List,
    CollectionKind::Set,
    CollectionKind::Map,
    CollectionKind::Array,
    CollectionKind::Tuple,
];

fn push_u32(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_le_bytes());
}

fn push_varint(out: &mut Vec<u8>, mut n: u64) {
    loop {
        let byte = (n & 0x7F) as u8;
        n >>= 7;
        if n == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn push_zigzag(out: &mut Vec<u8>, n: i64) {
    push_varint(out, ((n << 1) ^ (n >> 63)) as u64);
}

fn push_section(out: &mut Vec<u8>, tag: u8, payload: &[u8]) {
    out.push(tag);
    push_u32(out, payload.len() as u32);
    out.extend_from_slice(payload);
}

/// The asset writer's string table: interned in
/// encounter order (deterministic — the map is only
/// ever looked up, never iterated).
#[cfg(not(feature = "wasm-playground"))]
#[derive(Default)]
struct AssetWriter {
    strings: Vec<String>,
    index: std::collections::HashMap<String, u32>,
}

#[cfg(not(feature = "wasm-playground"))]
impl AssetWriter {
    fn intern(&mut self, s: &str) -> u32 {
        if let Some(idx) = self.index.get(s) {
            return *idx;
        }
        let idx = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.index.insert(s.to_string(), idx);
        idx
    }
}

#[cfg(not(feature = "wasm-playground"))]
fn write_literal(lit: &IRLiteral, w: &mut AssetWriter, out: &mut Vec<u8>) {
    match lit {
        IRLiteral::Int(n) => {
            out.push(OP_INT);
            push_zigzag(out, *n);
        }
        IRLiteral::Float(f) => {
            out.push(OP_FLOAT);
            out.extend_from_slice(&f.to_bits().to_le_bytes());
        }
        IRLiteral::Bool(b) => {
            out.push(OP_BOOL);
            out.push(u8::from(*b));
        }
        IRLiteral::Char(c) => {
            out.push(OP_CHAR);
            push_varint(out, u64::from(*c as u32));
        }
        IRLiteral::String(s) => {
            out.push(OP_STR);
            push_varint(out, u64::from(w.intern(s)));
        }
        IRLiteral::Unit => out.push(OP_UNIT),
    }
}

#[cfg(not(feature = "wasm-playground"))]
fn write_type(ty: &TypeRef, w: &mut AssetWriter, out: &mut Vec<u8>) {
    match ty {
        TypeRef::Int => out.push(TY_INT),
        TypeRef::Float => out.push(TY_FLOAT),
        TypeRef::Bool => out.push(TY_BOOL),
        TypeRef::String => out.push(TY_STRING),
        TypeRef::Char => out.push(TY_CHAR),
        TypeRef::Unit => out.push(TY_UNIT),
        TypeRef::I8 => out.push(TY_I8),
        TypeRef::I16 => out.push(TY_I16),
        TypeRef::I32 => out.push(TY_I32),
        TypeRef::I64 => out.push(TY_I64),
        TypeRef::U8 => out.push(TY_U8),
        TypeRef::U16 => out.push(TY_U16),
        TypeRef::U32 => out.push(TY_U32),
        TypeRef::U64 => out.push(TY_U64),
        TypeRef::Generic(name) => {
            out.push(TY_GENERIC);
            push_varint(out, u64::from(w.intern(name)));
        }
        TypeRef::Named(name) => {
            out.push(TY_NAMED);
            push_varint(out, u64::from(w.intern(name)));
        }
        TypeRef::List(inner) => {
            out.push(TY_LIST);
            write_type(inner, w, out);
        }
        TypeRef::Set(inner) => {
            out.push(TY_SET);
            write_type(inner, w, out);
        }
        TypeRef::Map(k, v) => {
            out.push(TY_MAP);
            write_type(k, w, out);
            write_type(v, w, out);
        }
        TypeRef::Option(inner) => {
            out.push(TY_OPTION);
            write_type(inner, w, out);
        }
        TypeRef::Result(ok, err) => {
            out.push(TY_RESULT);
            write_type(ok, w, out);
            write_type(err, w, out);
        }
        TypeRef::Tuple(items) => {
            out.push(TY_TUPLE);
            push_varint(out, items.len() as u64);
            for item in items {
                write_type(item, w, out);
            }
        }
        TypeRef::Array(inner, size) => {
            out.push(TY_ARRAY);
            write_type(inner, w, out);
            push_varint(out, *size as u64);
        }
        TypeRef::Function(params, ret) => {
            out.push(TY_FUNCTION);
            push_varint(out, params.len() as u64);
            for param in params {
                write_type(param, w, out);
            }
            write_type(ret, w, out);
        }
        TypeRef::Ref(inner) => {
            out.push(TY_REF);
            write_type(inner, w, out);
        }
        TypeRef::Stream(inner) => {
            out.push(TY_STREAM);
            write_type(inner, w, out);
        }
    }
}

#[cfg(not(feature = "wasm-playground"))]
fn write_opt_string(value: &Option<String>, w: &mut AssetWriter, out: &mut Vec<u8>) {
    // Index-plus-one: 0 encodes None.
    match value {
        Some(s) => push_varint(out, u64::from(w.intern(s)) + 1),
        None => push_varint(out, 0),
    }
}

#[cfg(not(feature = "wasm-playground"))]
fn write_params(params: &[(String, TypeRef)], w: &mut AssetWriter, out: &mut Vec<u8>) {
    push_varint(out, params.len() as u64);
    for (name, ty) in params {
        push_varint(out, u64::from(w.intern(name)));
        write_type(ty, w, out);
    }
}

#[cfg(not(feature = "wasm-playground"))]
fn write_pattern(pattern: &IRPattern, w: &mut AssetWriter, out: &mut Vec<u8>) {
    match pattern {
        IRPattern::Wildcard => out.push(OP_PWILD),
        IRPattern::Variable(name) => {
            out.push(OP_PVAR);
            push_varint(out, u64::from(w.intern(name)));
        }
        IRPattern::Literal(lit) => {
            write_literal(lit, w, out);
            out.push(OP_PLIT);
        }
        IRPattern::Constructor(name, subs) => {
            for sub in subs {
                write_pattern(sub, w, out);
            }
            out.push(OP_PCTOR);
            push_varint(out, u64::from(w.intern(name)));
            push_varint(out, subs.len() as u64);
        }
        IRPattern::Guard(inner, condition) => {
            write_pattern(inner, w, out);
            out.push(OP_PGUARD);
            push_varint(out, u64::from(w.intern(condition)));
        }
    }
}

#[cfg(not(feature = "wasm-playground"))]
fn write_node(node: &IRNode, w: &mut AssetWriter, out: &mut Vec<u8>) {
    match node {
        IRNode::Hole { id, expected_type } => {
            out.push(OP_HOLE);
            push_varint(out, *id);
            match expected_type {
                Some(ty) => {
                    out.push(1);
                    write_type(ty, w, out);
                }
                None => out.push(0),
            }
        }
        IRNode::Block(statements) => {
            for statement in statements {
                write_node(statement, w, out);
            }
            out.push(OP_BLOCK);
            push_varint(out, statements.len() as u64);
        }
        IRNode::Let { name, value, body } => {
            write_node(value, w, out);
            write_node(body, w, out);
            out.push(OP_LET);
            push_varint(out, u64::from(w.intern(name)));
        }
        IRNode::If { condition, then_branch, else_branch } => {
            write_node(condition, w, out);
            write_node(then_branch, w, out);
            write_node(else_branch, w, out);
            out.push(OP_IF);
        }
        IRNode::While { condition, body, invariant } => {
            write_node(condition, w, out);
            write_node(body, w, out);
            out.push(OP_WHILE);
            write_opt_string(invariant, w, out);
        }
        IRNode::For { var, start, end, body } => {
            write_node(start, w, out);
            write_node(end, w, out);
            write_node(body, w, out);
            out.push(OP_FOR);
            push_varint(out, u64::from(w.intern(var)));
        }
        IRNode::Match { scrutinee, cases } => {
            write_node(scrutinee, w, out);
            for (pattern, body) in cases {
                write_pattern(pattern, w, out);
                write_node(body, w, out);
            }
            out.push(OP_MATCH);
            push_varint(out, cases.len() as u64);
        }
        IRNode::Call { function, args } => {
            for arg in args {
                write_node(arg, w, out);
            }
            out.push(OP_CALL);
            push_varint(out, u64::from(w.intern(function)));
            push_varint(out, args.len() as u64);
        }
        IRNode::Return(value) => {
            write_node(value, w, out);
            out.push(OP_RETURN);
        }
        IRNode::Literal(lit) => write_literal(lit, w, out),
        IRNode::Var(name) => {
            out.push(OP_VAR);
            push_varint(out, u64::from(w.intern(name)));
        }
        IRNode::BinOp(op, l, r) => {
            write_node(l, w, out);
            write_node(r, w, out);
            out.push(OP_BINOP);
            out.push(BINOP_TABLE.iter().position(|o| o == op).unwrap() as u8);
        }
        IRNode::UnaryOp(op, x) => {
            write_node(x, w, out);
            out.push(OP_UNOP);
            out.push(UNOP_TABLE.iter().position(|o| o == op).unwrap() as u8);
        }
        IRNode::Lambda { params, body } => {
            write_node(body, w, out);
            out.push(OP_LAMBDA);
            write_params(params, w, out);
        }
        IRNode::Closure { params, captures, body } => {
            write_node(body, w, out);
            out.push(OP_CLOSURE);
            write_params(params, w, out);
            push_varint(out, captures.len() as u64);
            for capture in captures {
                push_varint(out, u64::from(w.intern(capture)));
            }
        }
        IRNode::Collection { kind, elements } => {
            for element in elements {
                write_node(element, w, out);
            }
            out.push(OP_COLLECTION);
            out.push(KIND_TABLE.iter().position(|k| k == kind).unwrap() as u8);
            push_varint(out, elements.len() as u64);
        }
        IRNode::Map { function, over } => {
            write_node(function, w, out);
            write_node(over, w, out);
            out.push(OP_MAP);
        }
        IRNode::Filter { predicate, over } => {
            write_node(predicate, w, out);
            write_node(over, w, out);
            out.push(OP_FILTER);
        }
        IRNode::Fold { operation, initial, over } => {
            write_node(operation, w, out);
            write_node(initial, w, out);
            write_node(over, w, out);
            out.push(OP_FOLD);
        }
        IRNode::Sort { comparator, collection } => {
            write_node(comparator, w, out);
            write_node(collection, w, out);
            out.push(OP_SORT);
        }
        IRNode::Alloc { name, ty, initial } => {
            write_node(initial, w, out);
            out.push(OP_ALLOC);
            push_varint(out, u64::from(w.intern(name)));
            write_type(ty, w, out);
        }
        IRNode::Assign { target, value } => {
            write_node(target, w, out);
            write_node(value, w, out);
            out.push(OP_ASSIGN);
        }
        IRNode::Typed { node, ty } => {
            write_node(node, w, out);
            out.push(OP_TYPED);
            write_type(ty, w, out);
        }
        IRNode::Index(collection, index) => {
            write_node(collection, w, out);
            write_node(index, w, out);
            out.push(OP_INDEX);
        }
        IRNode::Loop { body, invariant } => {
            write_node(body, w, out);
            out.push(OP_LOOP);
            write_opt_string(invariant, w, out);
        }
        IRNode::Break { value } => {
            match value {
                Some(inner) => write_node(inner, w, out),
                None => {}
            }
            out.push(OP_BREAK);
            out.push(u8::from(value.is_some()));
        }
        IRNode::StructDef { name, fields, body } => {
            write_node(body, w, out);
            out.push(OP_STRUCTDEF);
            push_varint(out, u64::from(w.intern(name)));
            push_varint(out, fields.len() as u64);
            for (field, ty) in fields {
                push_varint(out, u64::from(w.intern(field)));
                write_type(ty, w, out);
            }
        }
        IRNode::TryPropagate(inner) => {
            write_node(inner, w, out);
            out.push(OP_TRYPROP);
        }
        IRNode::FieldAccess { base, field } => {
            write_node(base, w, out);
            out.push(OP_FIELD);
            push_varint(out, u64::from(w.intern(field)));
        }
        IRNode::RecordLit { type_name, fields } => {
            for (_, value) in fields {
                write_node(value, w, out);
            }
            out.push(OP_RECORDLIT);
            push_varint(out, u64::from(w.intern(type_name)));
            push_varint(out, fields.len() as u64);
            for (field, _) in fields {
                push_varint(out, u64::from(w.intern(field)));
            }
        }
    }
}

/// Parse the asset's wire form (the inverse of
/// [`library_asset_bytes`]). Used by
/// [`set_library_asset`] and by the asset pin test.
/// Errors are static strings by design: formatted
/// errors are code the bundle cannot afford, and a
/// malformed asset has exactly one remedy (regenerate
/// it) regardless of which byte is wrong.
pub fn parse_library_asset(bytes: &[u8]) -> Result<LibraryAsset, String> {
    let mut cur = ByteCursor { bytes, pos: 0 };
    let magic = cur.take(ASSET_MAGIC.len()).map_err(str::to_string)?;
    if magic != ASSET_MAGIC {
        return Err("invalid stdlib asset: bad magic".to_string());
    }
    let mut strings: Vec<String> = Vec::new();
    let mut modules = Vec::new();
    let mut bodies = Vec::new();
    loop {
        match cur.u8().map_err(str::to_string)? {
            b'S' => {
                let mut payload = cur.payload()?;
                let count = payload.u32().map_err(str::to_string)?;
                let mut table = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let len = payload.u32().map_err(str::to_string)? as usize;
                    let raw = payload.take(len).map_err(str::to_string)?;
                    table.push(
                        String::from_utf8(raw.to_vec())
                            .map_err(|_| "invalid stdlib asset: non-UTF-8 string".to_string())?,
                    );
                }
                strings = table;
            }
            b'M' => {
                let mut payload = cur.payload()?;
                let name = string_at(&strings, payload.u32().map_err(str::to_string)?)?;
                let len = payload.u32().map_err(str::to_string)? as usize;
                let raw = payload.take(len).map_err(str::to_string)?;
                let source = String::from_utf8(raw.to_vec())
                    .map_err(|_| "invalid stdlib asset: non-UTF-8 source".to_string())?;
                modules.push(AssetModule { name, source });
            }
            b'B' => {
                let mut payload = cur.payload()?;
                let name = string_at(&strings, payload.u32().map_err(str::to_string)?)?;
                let len = payload.u32().map_err(str::to_string)? as usize;
                let encoding = payload.take(len).map_err(str::to_string)?;
                let body = build_body(encoding, &strings)?;
                bodies.push((name, body));
            }
            b'E' => break,
            _ => return Err("invalid stdlib asset: unknown section".to_string()),
        }
    }
    Ok(LibraryAsset { modules, bodies })
}

fn string_at(strings: &[String], idx: u32) -> Result<String, String> {
    strings
        .get(idx as usize)
        .cloned()
        .ok_or_else(|| "invalid stdlib asset: string index out of range".to_string())
}

/// A byte cursor over the wire form. All errors are
/// the same static class strings (see parse_library_asset).
struct ByteCursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> ByteCursor<'a> {
    fn u8(&mut self) -> Result<u8, &'static str> {
        let byte = *self.bytes.get(self.pos).ok_or("invalid stdlib asset: overrun")?;
        self.pos += 1;
        Ok(byte)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], &'static str> {
        if self.pos + len > self.bytes.len() {
            return Err("invalid stdlib asset: overrun");
        }
        let slice = &self.bytes[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn u32(&mut self) -> Result<u32, &'static str> {
        let raw = self.take(4)?;
        Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
    }

    /// A section payload as its own cursor.
    fn payload(&mut self) -> Result<ByteCursor<'a>, String> {
        let len = self.u32().map_err(str::to_string)? as usize;
        let raw = self.take(len).map_err(str::to_string)?;
        Ok(ByteCursor { bytes: raw, pos: 0 })
    }

    fn varint(&mut self) -> Result<u64, &'static str> {
        let mut value: u64 = 0;
        let mut shift = 0;
        loop {
            let byte = self.u8()?;
            value |= u64::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
            if shift >= 64 {
                return Err("invalid stdlib asset: varint too long");
            }
        }
    }

    fn zigzag(&mut self) -> Result<i64, &'static str> {
        let raw = self.varint()?;
        Ok(((raw >> 1) as i64) ^ -((raw & 1) as i64))
    }

    fn idx(&mut self) -> Result<usize, &'static str> {
        Ok(self.varint()? as usize)
    }

    fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }
}

fn pop_node(nodes: &mut Vec<IRNode>) -> Result<IRNode, String> {
    nodes
        .pop()
        .ok_or_else(|| "invalid stdlib asset: body stack underflow".to_string())
}

fn pop_pattern(patterns: &mut Vec<IRPattern>) -> Result<IRPattern, String> {
    patterns
        .pop()
        .ok_or_else(|| "invalid stdlib asset: pattern stack underflow".to_string())
}

fn table<T: Copy>(table: &[T], idx: usize) -> Result<T, String> {
    table
        .get(idx)
        .copied()
        .ok_or_else(|| "invalid stdlib asset: op index out of range".to_string())
}

fn build_type(cur: &mut ByteCursor, strings: &[String]) -> Result<TypeRef, String> {
    Ok(match cur.u8().map_err(str::to_string)? {
        TY_INT => TypeRef::Int,
        TY_FLOAT => TypeRef::Float,
        TY_BOOL => TypeRef::Bool,
        TY_STRING => TypeRef::String,
        TY_CHAR => TypeRef::Char,
        TY_UNIT => TypeRef::Unit,
        TY_I8 => TypeRef::I8,
        TY_I16 => TypeRef::I16,
        TY_I32 => TypeRef::I32,
        TY_I64 => TypeRef::I64,
        TY_U8 => TypeRef::U8,
        TY_U16 => TypeRef::U16,
        TY_U32 => TypeRef::U32,
        TY_U64 => TypeRef::U64,
        TY_GENERIC => {
            TypeRef::Generic(string_at(strings, cur.varint().map_err(str::to_string)? as u32)?)
        }
        TY_NAMED => {
            TypeRef::Named(string_at(strings, cur.varint().map_err(str::to_string)? as u32)?)
        }
        TY_LIST => TypeRef::List(Box::new(build_type(cur, strings)?)),
        TY_SET => TypeRef::Set(Box::new(build_type(cur, strings)?)),
        TY_MAP => {
            let k = build_type(cur, strings)?;
            let v = build_type(cur, strings)?;
            TypeRef::Map(Box::new(k), Box::new(v))
        }
        TY_OPTION => TypeRef::Option(Box::new(build_type(cur, strings)?)),
        TY_RESULT => {
            let ok = build_type(cur, strings)?;
            let err = build_type(cur, strings)?;
            TypeRef::Result(Box::new(ok), Box::new(err))
        }
        TY_TUPLE => {
            let count = cur.idx().map_err(str::to_string)?;
            let mut items = Vec::with_capacity(count);
            for _ in 0..count {
                items.push(build_type(cur, strings)?);
            }
            TypeRef::Tuple(items)
        }
        TY_ARRAY => {
            let inner = build_type(cur, strings)?;
            let size = cur.idx().map_err(str::to_string)?;
            TypeRef::Array(Box::new(inner), size)
        }
        TY_FUNCTION => {
            let count = cur.idx().map_err(str::to_string)?;
            let mut params = Vec::with_capacity(count);
            for _ in 0..count {
                params.push(build_type(cur, strings)?);
            }
            let ret = build_type(cur, strings)?;
            TypeRef::Function(params, Box::new(ret))
        }
        TY_REF => TypeRef::Ref(Box::new(build_type(cur, strings)?)),
        TY_STREAM => TypeRef::Stream(Box::new(build_type(cur, strings)?)),
        _ => return Err("invalid stdlib asset: bad type tag".to_string()),
    })
}

fn build_opt_string(
    cur: &mut ByteCursor,
    strings: &[String],
) -> Result<Option<String>, String> {
    let raw = cur.varint().map_err(str::to_string)?;
    if raw == 0 {
        Ok(None)
    } else {
        Ok(Some(string_at(strings, (raw - 1) as u32)?))
    }
}

fn build_params(
    cur: &mut ByteCursor,
    strings: &[String],
) -> Result<Vec<(String, TypeRef)>, String> {
    let count = cur.idx().map_err(str::to_string)?;
    let mut params = Vec::with_capacity(count);
    for _ in 0..count {
        let name = string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
        let ty = build_type(cur, strings)?;
        params.push((name, ty));
    }
    Ok(params)
}

/// The stack-machine builder: interpret one body's
/// postfix opcode stream. Exactly one node must remain
/// on the node stack, and the pattern stack must be
/// empty, when the stream ends.
fn build_body(encoding: &[u8], strings: &[String]) -> Result<IRNode, String> {
    let mut cur = ByteCursor { bytes: encoding, pos: 0 };
    let mut nodes: Vec<IRNode> = Vec::new();
    let mut patterns: Vec<IRPattern> = Vec::new();
    while !cur.at_end() {
        let op = cur.u8().map_err(str::to_string)?;
        match op {
            OP_VAR => {
                let name = string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                nodes.push(IRNode::Var(name));
            }
            OP_INT => nodes.push(IRNode::Literal(IRLiteral::Int(
                cur.zigzag().map_err(str::to_string)?,
            ))),
            OP_FLOAT => {
                let raw = cur.take(8).map_err(str::to_string)?;
                let bits = u64::from_le_bytes([
                    raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                ]);
                nodes.push(IRNode::Literal(IRLiteral::Float(f64::from_bits(bits))));
            }
            OP_BOOL => {
                let b = cur.u8().map_err(str::to_string)?;
                nodes.push(IRNode::Literal(IRLiteral::Bool(b != 0)));
            }
            OP_CHAR => {
                let raw = cur.varint().map_err(str::to_string)?;
                let c = char::from_u32(raw as u32)
                    .ok_or_else(|| "invalid stdlib asset: bad char".to_string())?;
                nodes.push(IRNode::Literal(IRLiteral::Char(c)));
            }
            OP_STR => {
                let s = string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                nodes.push(IRNode::Literal(IRLiteral::String(s)));
            }
            OP_UNIT => nodes.push(IRNode::Literal(IRLiteral::Unit)),
            OP_BINOP => {
                let idx = cur.u8().map_err(str::to_string)? as usize;
                let op = table(BINOP_TABLE, idx)?;
                let r = pop_node(&mut nodes)?;
                let l = pop_node(&mut nodes)?;
                nodes.push(IRNode::BinOp(op, Box::new(l), Box::new(r)));
            }
            OP_UNOP => {
                let idx = cur.u8().map_err(str::to_string)? as usize;
                let op = table(UNOP_TABLE, idx)?;
                let x = pop_node(&mut nodes)?;
                nodes.push(IRNode::UnaryOp(op, Box::new(x)));
            }
            OP_CALL => {
                let function =
                    string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                let argc = cur.idx().map_err(str::to_string)?;
                let mut args = Vec::with_capacity(argc);
                for _ in 0..argc {
                    args.push(pop_node(&mut nodes)?);
                }
                args.reverse();
                nodes.push(IRNode::Call { function, args });
            }
            OP_IF => {
                let else_branch = pop_node(&mut nodes)?;
                let then_branch = pop_node(&mut nodes)?;
                let condition = pop_node(&mut nodes)?;
                nodes.push(IRNode::If {
                    condition: Box::new(condition),
                    then_branch: Box::new(then_branch),
                    else_branch: Box::new(else_branch),
                });
            }
            OP_LET => {
                let name = string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                let body = pop_node(&mut nodes)?;
                let value = pop_node(&mut nodes)?;
                nodes.push(IRNode::Let {
                    name,
                    value: Box::new(value),
                    body: Box::new(body),
                });
            }
            OP_MATCH => {
                let count = cur.idx().map_err(str::to_string)?;
                let mut cases = Vec::with_capacity(count);
                for _ in 0..count {
                    let body = pop_node(&mut nodes)?;
                    let pattern = pop_pattern(&mut patterns)?;
                    cases.push((pattern, body));
                }
                cases.reverse();
                let scrutinee = pop_node(&mut nodes)?;
                nodes.push(IRNode::Match {
                    scrutinee: Box::new(scrutinee),
                    cases,
                });
            }
            OP_BLOCK => {
                let count = cur.idx().map_err(str::to_string)?;
                let mut statements = Vec::with_capacity(count);
                for _ in 0..count {
                    statements.push(pop_node(&mut nodes)?);
                }
                statements.reverse();
                nodes.push(IRNode::Block(statements));
            }
            OP_RETURN => {
                let value = pop_node(&mut nodes)?;
                nodes.push(IRNode::Return(Box::new(value)));
            }
            OP_HOLE => {
                let id = cur.varint().map_err(str::to_string)?;
                let has_type = cur.u8().map_err(str::to_string)?;
                let expected_type = if has_type != 0 {
                    Some(build_type(&mut cur, strings)?)
                } else {
                    None
                };
                nodes.push(IRNode::Hole { id, expected_type });
            }
            OP_WHILE => {
                let invariant = build_opt_string(&mut cur, strings)?;
                let body = pop_node(&mut nodes)?;
                let condition = pop_node(&mut nodes)?;
                nodes.push(IRNode::While {
                    condition: Box::new(condition),
                    body: Box::new(body),
                    invariant,
                });
            }
            OP_FOR => {
                let var = string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                let body = pop_node(&mut nodes)?;
                let end = pop_node(&mut nodes)?;
                let start = pop_node(&mut nodes)?;
                nodes.push(IRNode::For {
                    var,
                    start: Box::new(start),
                    end: Box::new(end),
                    body: Box::new(body),
                });
            }
            OP_LAMBDA => {
                let params = build_params(&mut cur, strings)?;
                let body = pop_node(&mut nodes)?;
                nodes.push(IRNode::Lambda {
                    params,
                    body: Box::new(body),
                });
            }
            OP_CLOSURE => {
                let params = build_params(&mut cur, strings)?;
                let count = cur.idx().map_err(str::to_string)?;
                let mut captures = Vec::with_capacity(count);
                for _ in 0..count {
                    captures.push(string_at(
                        strings,
                        cur.varint().map_err(str::to_string)? as u32,
                    )?);
                }
                let body = pop_node(&mut nodes)?;
                nodes.push(IRNode::Closure {
                    params,
                    captures,
                    body: Box::new(body),
                });
            }
            OP_COLLECTION => {
                let kind_idx = cur.u8().map_err(str::to_string)? as usize;
                let kind = table(KIND_TABLE, kind_idx)?;
                let count = cur.idx().map_err(str::to_string)?;
                let mut elements = Vec::with_capacity(count);
                for _ in 0..count {
                    elements.push(pop_node(&mut nodes)?);
                }
                elements.reverse();
                nodes.push(IRNode::Collection { kind, elements });
            }
            OP_MAP => {
                let over = pop_node(&mut nodes)?;
                let function = pop_node(&mut nodes)?;
                nodes.push(IRNode::Map {
                    function: Box::new(function),
                    over: Box::new(over),
                });
            }
            OP_FILTER => {
                let over = pop_node(&mut nodes)?;
                let predicate = pop_node(&mut nodes)?;
                nodes.push(IRNode::Filter {
                    predicate: Box::new(predicate),
                    over: Box::new(over),
                });
            }
            OP_FOLD => {
                let over = pop_node(&mut nodes)?;
                let initial = pop_node(&mut nodes)?;
                let operation = pop_node(&mut nodes)?;
                nodes.push(IRNode::Fold {
                    operation: Box::new(operation),
                    initial: Box::new(initial),
                    over: Box::new(over),
                });
            }
            OP_SORT => {
                let collection = pop_node(&mut nodes)?;
                let comparator = pop_node(&mut nodes)?;
                nodes.push(IRNode::Sort {
                    comparator: Box::new(comparator),
                    collection: Box::new(collection),
                });
            }
            OP_ALLOC => {
                let name = string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                let ty = build_type(&mut cur, strings)?;
                let initial = pop_node(&mut nodes)?;
                nodes.push(IRNode::Alloc {
                    name,
                    ty,
                    initial: Box::new(initial),
                });
            }
            OP_ASSIGN => {
                let value = pop_node(&mut nodes)?;
                let target = pop_node(&mut nodes)?;
                nodes.push(IRNode::Assign {
                    target: Box::new(target),
                    value: Box::new(value),
                });
            }
            OP_TYPED => {
                let ty = build_type(&mut cur, strings)?;
                let node = pop_node(&mut nodes)?;
                nodes.push(IRNode::Typed {
                    node: Box::new(node),
                    ty,
                });
            }
            OP_INDEX => {
                let index = pop_node(&mut nodes)?;
                let collection = pop_node(&mut nodes)?;
                nodes.push(IRNode::Index(Box::new(collection), Box::new(index)));
            }
            OP_LOOP => {
                let invariant = build_opt_string(&mut cur, strings)?;
                let body = pop_node(&mut nodes)?;
                nodes.push(IRNode::Loop {
                    body: Box::new(body),
                    invariant,
                });
            }
            OP_BREAK => {
                let has_value = cur.u8().map_err(str::to_string)?;
                let value = if has_value != 0 {
                    Some(Box::new(pop_node(&mut nodes)?))
                } else {
                    None
                };
                nodes.push(IRNode::Break { value });
            }
            OP_STRUCTDEF => {
                let name = string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                let count = cur.idx().map_err(str::to_string)?;
                let mut fields = Vec::with_capacity(count);
                for _ in 0..count {
                    let field =
                        string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                    let ty = build_type(&mut cur, strings)?;
                    fields.push((field, ty));
                }
                let body = pop_node(&mut nodes)?;
                nodes.push(IRNode::StructDef {
                    name,
                    fields,
                    body: Box::new(body),
                });
            }
            OP_TRYPROP => {
                let inner = pop_node(&mut nodes)?;
                nodes.push(IRNode::TryPropagate(Box::new(inner)));
            }
            OP_FIELD => {
                let field = string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                let base = pop_node(&mut nodes)?;
                nodes.push(IRNode::FieldAccess {
                    base: Box::new(base),
                    field,
                });
            }
            OP_RECORDLIT => {
                let type_name =
                    string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                let count = cur.idx().map_err(str::to_string)?;
                let mut names = Vec::with_capacity(count);
                for _ in 0..count {
                    names.push(string_at(
                        strings,
                        cur.varint().map_err(str::to_string)? as u32,
                    )?);
                }
                let mut values = Vec::with_capacity(count);
                for _ in 0..count {
                    values.push(pop_node(&mut nodes)?);
                }
                values.reverse();
                let fields = names.into_iter().zip(values).collect();
                nodes.push(IRNode::RecordLit {
                    type_name,
                    fields,
                });
            }
            OP_PWILD => patterns.push(IRPattern::Wildcard),
            OP_PVAR => {
                let name = string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                patterns.push(IRPattern::Variable(name));
            }
            OP_PLIT => {
                let node = pop_node(&mut nodes)?;
                match node {
                    IRNode::Literal(lit) => patterns.push(IRPattern::Literal(lit)),
                    _ => {
                        return Err(
                            "invalid stdlib asset: pattern literal over non-literal".to_string()
                        )
                    }
                }
            }
            OP_PCTOR => {
                let name = string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                let count = cur.idx().map_err(str::to_string)?;
                let mut subs = Vec::with_capacity(count);
                for _ in 0..count {
                    subs.push(pop_pattern(&mut patterns)?);
                }
                subs.reverse();
                patterns.push(IRPattern::Constructor(name, subs));
            }
            OP_PGUARD => {
                let condition =
                    string_at(strings, cur.varint().map_err(str::to_string)? as u32)?;
                let inner = pop_pattern(&mut patterns)?;
                patterns.push(IRPattern::Guard(Box::new(inner), condition));
            }
            _ => return Err("invalid stdlib asset: bad opcode".to_string()),
        }
    }
    if nodes.len() != 1 || !patterns.is_empty() {
        return Err("invalid stdlib asset: body does not reduce to one node".to_string());
    }
    Ok(nodes.pop().unwrap())
}

/// The module sources the prelude is built from, in
/// bundle order: natively the embedded table; in a
/// playground build, the installed asset's modules (or
/// none, before installation).
fn module_sources() -> Vec<(String, String)> {
    #[cfg(not(feature = "wasm-playground"))]
    return LIBRARY_MODULES
        .iter()
        .map(|(name, source)| (name.to_string(), source.to_string()))
        .collect();
    #[cfg(feature = "wasm-playground")]
    return match installed_asset() {
        Some(asset) => asset
            .modules
            .iter()
            .map(|m| (m.name.clone(), m.source.clone()))
            .collect(),
        None => Vec::new(),
    };
}

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
        for (name, source) in module_sources() {
            let module = crate::spec::parser::parse(&source).unwrap_or_else(|e| {
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

/// The standard library's own specs — the prelude's
/// first layer, alone. Consumers whose question is
/// about the stdlib itself (the asset above, the
/// registry's collision check) read this; every other
/// consumer reads the merged funnel, [`prelude_specs`].
pub fn stdlib_specs() -> Vec<FunctionSpec> {
    library_module().functions.clone()
}

/// The library's specs — the prelude's signature surface.
/// The typechecker registers these in every module's
/// environment; the SpecIndex merge inserts the same specs
/// into every module's index. Since Phase 165
/// (DISEBAR_17_SCOPE §7) the prelude is two layers: the
/// stdlib's specs, then the installed layer's — an
/// installed name the stdlib holds is not yielded (§6's
/// inertness, realized here at the funnel). Native builds
/// only: a playground build's prelude is the installed
/// asset's, and it carries no installed layer.
pub fn prelude_specs() -> Vec<FunctionSpec> {
    let mut specs = stdlib_specs();
    #[cfg(not(feature = "wasm-playground"))]
    specs.extend(
        installed_layer()
            .entries
            .iter()
            .map(|(spec, _)| spec.clone()),
    );
    specs
}

// ── The installed layer (Phase 165) ─────────────────────

/// One process's read of the registry install directory:
/// the entries discovery admits to the prelude, and the
/// named diagnostic for every entry (or the whole layer)
/// it did not admit. Read once per process — the
/// stdlib's own OnceLock pattern — so a long-lived
/// shell's prelude is stable for its life; an install or
/// a removal is a new process's fact (every CLI
/// invocation is one).
#[cfg(not(feature = "wasm-playground"))]
struct InstalledLayer {
    /// (spec, body) per admitted entry, in ledger order.
    entries: Vec<(FunctionSpec, IRNode)>,
    /// One line per refusal: who was not admitted, and
    /// why — named, never silent (§7).
    diagnostics: Vec<String>,
}

#[cfg(not(feature = "wasm-playground"))]
fn installed_layer() -> &'static InstalledLayer {
    static LAYER: OnceLock<InstalledLayer> = OnceLock::new();
    LAYER.get_or_init(load_installed_layer)
}

/// The discovery read (DISEBAR_17_SCOPE §7).
///
/// THE TRUST BOUNDARY: discovery does NOT re-verify.
/// The install ran the proof — sha256 against the
/// index, then `verify_bundle_text`'s re-judgment — and
/// the receipt records what that re-run measured.
/// Discovery trusts the receipt + the file's presence,
/// checking only the cheap identity facts: the file
/// still hashes to the receipt's sha256, the bundle
/// inside still names the receipt's entry, its spec
/// still names it too, and its body still reads through
/// the IR text reader. Each failure degrades that entry
/// by name; a corrupt ledger degrades the whole layer
/// by name; the stdlib and the user's module are
/// unaffected in every shape. `morphic registry verify`
/// is the re-check verb — it re-runs the proof.
#[cfg(not(feature = "wasm-playground"))]
fn load_installed_layer() -> InstalledLayer {
    let mut diagnostics: Vec<String> = Vec::new();
    let mut entries: Vec<(FunctionSpec, IRNode)> = Vec::new();
    let empty = |diagnostics: Vec<String>| InstalledLayer {
        entries: Vec::new(),
        diagnostics,
    };
    let dir = match crate::registry::install_dir() {
        Ok(dir) => dir,
        Err(reason) => {
            diagnostics.push(format!(
                "registry discovery: the installed layer is empty — {reason}"
            ));
            return empty(diagnostics);
        }
    };
    // An absent directory is today's behavior — no
    // layer, and no diagnostic: absence is not
    // degradation.
    if !dir.exists() {
        return empty(diagnostics);
    }
    let receipts = match crate::registry::ledger_receipts(&dir) {
        Ok(receipts) => receipts,
        Err(reason) => {
            diagnostics.push(format!(
                "registry discovery: the installed layer is empty — {reason}"
            ));
            return empty(diagnostics);
        }
    };
    let stdlib_names: std::collections::BTreeSet<&str> =
        library_module().functions.iter().map(|s| s.name.as_str()).collect();
    let mut receipted: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for receipt in &receipts {
        let name = receipt.entry.as_str();
        receipted.insert(name);
        // §6's inertness, realized at the funnel: the
        // layer order, not the directory's contents,
        // decides — a hand-planted stdlib-named install
        // (or a forged receipt for one) yields nothing.
        if stdlib_names.contains(name) {
            diagnostics.push(format!(
                "registry discovery: installed entry '{name}' is inert — \
                 '{name}' is a standard-library name, and precedence \
                 user > stdlib > installed means the installed layer \
                 never yields it (remove it with: morphic registry \
                 remove {name})"
            ));
            continue;
        }
        if entries.iter().any(|(spec, _)| spec.name == name) {
            diagnostics.push(format!(
                "registry discovery: installed entry '{name}' is skipped \
                 — the ledger carries a second receipt for it; the \
                 first stands"
            ));
            continue;
        }
        let file = dir.join(format!("{name}.bundle.json"));
        let bytes = match std::fs::read(&file) {
            Ok(bytes) => bytes,
            Err(e) => {
                diagnostics.push(format!(
                    "registry discovery: installed entry '{name}' is \
                     skipped — its bundle file at {} cannot be read: {e}",
                    file.display()
                ));
                continue;
            }
        };
        let digest = crate::registry::sha256_hex(&bytes);
        if digest != receipt.sha256 {
            diagnostics.push(format!(
                "registry discovery: installed entry '{name}' is \
                 skipped — the installed file's sha256 is {digest}, but \
                 its receipt records {}: the file changed after install \
                 (`morphic registry verify {name}` reports the same \
                 fact as a refusal)",
                receipt.sha256
            ));
            continue;
        }
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                diagnostics.push(format!(
                    "registry discovery: installed entry '{name}' is \
                     skipped — its bundle file is not UTF-8 text"
                ));
                continue;
            }
        };
        let bundle: crate::pcc::Bundle = match serde_json::from_str(&text) {
            Ok(bundle) => bundle,
            Err(e) => {
                diagnostics.push(format!(
                    "registry discovery: installed entry '{name}' is \
                     skipped — its bundle file does not parse: {e}"
                ));
                continue;
            }
        };
        if bundle.entry != name || bundle.spec.name != name {
            diagnostics.push(format!(
                "registry discovery: installed entry '{name}' is \
                 skipped — the bundle inside names entry '{}' and its \
                 spec names '{}'; a receipt and its file must agree",
                bundle.entry, bundle.spec.name
            ));
            continue;
        }
        let body = match crate::synthesis::irtext::parse_ir(&bundle.body) {
            Ok(body) => body,
            Err(reason) => {
                diagnostics.push(format!(
                    "registry discovery: installed entry '{name}' is \
                     skipped — its body's IR text does not read: {reason}"
                ));
                continue;
            }
        };
        entries.push((bundle.spec, body));
    }
    // Stray bundle files — present in the directory,
    // receipted nowhere — are not installs; named, so a
    // hand-copied bundle's absence from the prelude is
    // never silent (`registry verify`'s sweep names the
    // same files as refusals).
    if let Ok(read) = std::fs::read_dir(&dir) {
        for item in read.flatten() {
            let file_name = item.file_name().to_string_lossy().into_owned();
            if let Some(stem) = file_name.strip_suffix(".bundle.json") {
                if !receipted.contains(stem) {
                    diagnostics.push(format!(
                        "registry discovery: bundle file '{file_name}' \
                         has no receipt in {} — it is not an install and \
                         discovery ignores it (install it with: morphic \
                         registry add)",
                        crate::registry::LEDGER_FILE
                    ));
                }
            }
        }
    }
    InstalledLayer { entries, diagnostics }
}

/// The installed layer's named diagnostics (Phase 165):
/// one line per entry (or the layer) discovery did not
/// admit, and why. Empty when the layer is whole — and
/// when there is no layer at all (an absent directory is
/// not degradation). The CLI prints these on the shells
/// that report; `morphic registry verify` reports the
/// same facts as refusals. Playground builds have no
/// installed layer and always answer empty.
pub fn installed_diagnostics() -> Vec<String> {
    #[cfg(not(feature = "wasm-playground"))]
    return installed_layer().diagnostics.clone();
    #[cfg(feature = "wasm-playground")]
    return Vec::new();
}

/// The installed layer's shipped body for `name`
/// (native builds): the bundle's body, read back
/// through the IR text reader at discovery time.
#[cfg(not(feature = "wasm-playground"))]
fn installed_body(name: &str) -> Option<IRNode> {
    installed_layer()
        .entries
        .iter()
        .find(|(spec, _)| spec.name == name)
        .map(|(_, body)| body.clone())
}

/// The crowned body shipped for the library entry `name`,
/// or None when the entry carries no shipped body (its name
/// then denotes whatever its spec's rows derive, like any
/// spec — no entry ships that way). Native builds answer
/// from the transcribed registry below; playground builds
/// answer from the installed asset — the same data, loaded
/// at runtime instead of compiled in. Since Phase 165 a
/// native build answers an installed entry's name from
/// the installed layer, and only when the stdlib registry
/// does not hold the name — never across the layer line
/// (§7).
pub fn shipped_body(name: &str) -> Option<IRNode> {
    #[cfg(not(feature = "wasm-playground"))]
    return shipped_body_native(name).or_else(|| installed_body(name));
    #[cfg(feature = "wasm-playground")]
    return installed_asset()?
        .bodies
        .iter()
        .find(|(entry, _)| entry == name)
        .map(|(_, body)| body.clone());
}

/// The transcribed registry of crowned bodies (native
/// builds). Each Tier-1 arm is the body the production
/// pipeline crowned for the entry at library-build
/// time, pinned by tests/phase110_prelude.rs; the
/// Tier-2 List arms (Phase 115) are the recorded hand
/// bodies at their tier's executed standard, pinned by
/// tests/phase115_list.rs.
#[cfg(not(feature = "wasm-playground"))]
fn shipped_body_native(name: &str) -> Option<IRNode> {
    // Transcription helpers for the Phase 111 arms (the
    // Phase 110 `int_abs` arm below predates them and is
    // left in its original longhand).
    fn v(name: &str) -> IRNode {
        IRNode::Var(name.into())
    }
    fn i(n: i64) -> IRNode {
        IRNode::Literal(IRLiteral::Int(n))
    }
    fn ch(c: char) -> IRNode {
        IRNode::Literal(IRLiteral::Char(c))
    }
    fn bop(op: IRBinOp, l: IRNode, r: IRNode) -> IRNode {
        IRNode::BinOp(op, Box::new(l), Box::new(r))
    }
    fn if_(cond: IRNode, then: IRNode, els: IRNode) -> IRNode {
        IRNode::If {
            condition: Box::new(cond),
            then_branch: Box::new(then),
            else_branch: Box::new(els),
        }
    }
    // Transcription helpers for the Phase 115 arms (the
    // statement/scan vocabulary of the List bodies).
    fn boolean(b: bool) -> IRNode {
        IRNode::Literal(IRLiteral::Bool(b))
    }
    fn block(stmts: Vec<IRNode>) -> IRNode {
        IRNode::Block(stmts)
    }
    fn alloc(name: &str, ty: TypeRef, initial: IRNode) -> IRNode {
        IRNode::Alloc {
            name: name.into(),
            ty,
            initial: Box::new(initial),
        }
    }
    fn assign(name: &str, value: IRNode) -> IRNode {
        IRNode::Assign {
            target: Box::new(v(name)),
            value: Box::new(value),
        }
    }
    fn for_(var: &str, start: IRNode, end: IRNode, body: IRNode) -> IRNode {
        IRNode::For {
            var: var.into(),
            start: Box::new(start),
            end: Box::new(end),
            body: Box::new(body),
        }
    }
    /// The range-end form of `len`: the UnaryOp node,
    /// which the Rust emitter renders WITHOUT the
    /// `as i64` cast, so a For range over it binds the
    /// loop variable as `usize` — the one index the
    /// generated Rust can use (the G5 boundary).
    fn len_u(x: IRNode) -> IRNode {
        IRNode::UnaryOp(IRUnaryOp::Len, Box::new(x))
    }
    /// The value form of `len`: the Call, rendered
    /// `(...).len() as i64` — an Int wherever an Int is
    /// compared or stored.
    fn len_c(x: IRNode) -> IRNode {
        IRNode::Call {
            function: "len".into(),
            args: vec![x],
        }
    }
    fn idx(coll: IRNode, index: IRNode) -> IRNode {
        IRNode::Index(Box::new(coll), Box::new(index))
    }
    fn empty_list() -> IRNode {
        IRNode::Collection {
            kind: CollectionKind::List,
            elements: vec![],
        }
    }
    fn append_stmt(name: &str, item: IRNode) -> IRNode {
        bop(IRBinOp::Append, v(name), item)
    }
    // Phase 135 helper: an application of a function
    // parameter — the Call form both executed judges
    // reach (the interpreter applies a bound function
    // value, Phase 130; the compiled runner renders the
    // plain call against the fn-pointer parameter).
    fn call(f: &str, args: Vec<IRNode>) -> IRNode {
        IRNode::Call {
            function: f.into(),
            args,
        }
    }
    fn some(x: IRNode) -> IRNode {
        IRNode::Call {
            function: "Some".into(),
            args: vec![x],
        }
    }
    fn none() -> IRNode {
        IRNode::Call {
            function: "None".into(),
            args: vec![],
        }
    }
    fn ret(x: IRNode) -> IRNode {
        IRNode::Return(Box::new(x))
    }
    fn list_ty() -> TypeRef {
        TypeRef::List(Box::new(TypeRef::Int))
    }
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
        // ── Phase 111: the Int family completed ─────────
        // Each body below was crowned at library-build time
        // in Phase 111 (seeded pipeline, seed 42) and is
        // pinned, per entry, by tests/phase110_prelude.rs:
        // Z3-Verified ∧ golden-exact on the entry's battery.
        //
        // int_min / int_max: the complementary-guard seed —
        // If(a > b, b, a) / If(a < b, b, a).
        "int_min" => Some(if_(bop(IRBinOp::Gt, v("a"), v("b")), v("b"), v("a"))),
        "int_max" => Some(if_(bop(IRBinOp::Lt, v("a"), v("b")), v("b"), v("a"))),
        // int_clamp: the equation seed over the spec's
        // composition row — min(max(x, lo), hi) with the
        // callee bodies inlined: If(m > hi, hi, m) where
        // m = If(x < lo, lo, x), stated twice.
        "int_clamp" => {
            let m = if_(bop(IRBinOp::Lt, v("x"), v("lo")), v("lo"), v("x"));
            Some(if_(bop(IRBinOp::Gt, m.clone(), v("hi")), v("hi"), m))
        }
        // int_sign: the equation seed over the clamp
        // identity — If(m > 1, 1, m) where m is clamp's
        // max-half If(x < 0 - 1, 0 - 1, x) (the (0 - 1)
        // pins lower as Sub terms, exactly as crowned).
        "int_sign" => {
            let m = if_(
                bop(IRBinOp::Lt, v("x"), bop(IRBinOp::Sub, i(0), i(1))),
                bop(IRBinOp::Sub, i(0), i(1)),
                v("x"),
            );
            Some(if_(bop(IRBinOp::Gt, m.clone(), i(1)), i(1), m))
        }
        // int_is_even / int_is_odd: the equation seeds —
        // the rows' own right-hand sides.
        "int_is_even" => Some(bop(IRBinOp::Eq, bop(IRBinOp::Mod, v("x"), i(2)), i(0))),
        "int_is_odd" => Some(bop(IRBinOp::Neq, bop(IRBinOp::Mod, v("x"), i(2)), i(0))),
        // ── Phase 111: the Char family ──────────────────
        // The five classifiers: equation seeds — the rows'
        // own right-hand sides over the Char scalar
        // encoding.
        "char_is_digit" => Some(bop(
            IRBinOp::And,
            bop(IRBinOp::Ge, v("c"), ch('0')),
            bop(IRBinOp::Le, v("c"), ch('9')),
        )),
        "char_is_alpha" => Some(bop(
            IRBinOp::Or,
            bop(
                IRBinOp::And,
                bop(IRBinOp::Ge, v("c"), ch('A')),
                bop(IRBinOp::Le, v("c"), ch('Z')),
            ),
            bop(
                IRBinOp::And,
                bop(IRBinOp::Ge, v("c"), ch('a')),
                bop(IRBinOp::Le, v("c"), ch('z')),
            ),
        )),
        "char_is_whitespace" => Some(bop(
            IRBinOp::Or,
            bop(
                IRBinOp::Or,
                bop(
                    IRBinOp::Or,
                    bop(IRBinOp::Eq, v("c"), ch(' ')),
                    bop(IRBinOp::Eq, v("c"), ch('\t')),
                ),
                bop(IRBinOp::Eq, v("c"), ch('\n')),
            ),
            bop(IRBinOp::Eq, v("c"), ch('\r')),
        )),
        "char_is_uppercase" => Some(bop(
            IRBinOp::And,
            bop(IRBinOp::Ge, v("c"), ch('A')),
            bop(IRBinOp::Le, v("c"), ch('Z')),
        )),
        "char_is_lowercase" => Some(bop(
            IRBinOp::And,
            bop(IRBinOp::Ge, v("c"), ch('a')),
            bop(IRBinOp::Le, v("c"), ch('z')),
        )),
        // char_to_digit: the char_guard_table seed
        // (Phase 111's one seed-family addition) — the
        // rows' own match: one arm per digit, the None
        // fall-through the two boundary rows pin on both
        // tails as the wildcard.
        "char_to_digit" => {
            let mut cases: Vec<(IRPattern, IRNode)> = ('0'..='9')
                .map(|d| {
                    (
                        IRPattern::Literal(IRLiteral::Char(d)),
                        IRNode::Call {
                            function: "Some".into(),
                            args: vec![i(d as i64 - '0' as i64)],
                        },
                    )
                })
                .collect();
            cases.push((
                IRPattern::Wildcard,
                IRNode::Call { function: "None".into(), args: vec![] },
            ));
            Some(IRNode::Match {
                scrutinee: Box::new(v("c")),
                cases,
            })
        }
        // ── Phase 112: String basics ────────────────────
        // string_is_empty: the equation seed — the row's
        // own right-hand side, len(s) == 0.
        "string_is_empty" => Some(bop(
            IRBinOp::Eq,
            IRNode::Call {
                function: "len".into(),
                args: vec![v("s")],
            },
            i(0),
        )),
        // ── Phase 112: Result / Option ──────────────────
        // The match_from_rows seed (Phase 112's one seed-
        // family addition): each body is the constructor
        // Match the entry's guarded method rows state.
        "result_unwrap_or" => Some(IRNode::Match {
            scrutinee: Box::new(v("r")),
            cases: vec![
                (
                    IRPattern::Constructor("Ok".into(), vec![IRPattern::Variable("v".into())]),
                    v("v"),
                ),
                (
                    IRPattern::Constructor("Err".into(), vec![IRPattern::Wildcard]),
                    v("d"),
                ),
            ],
        }),
        "option_unwrap_or" => Some(IRNode::Match {
            scrutinee: Box::new(v("o")),
            cases: vec![
                (
                    IRPattern::Constructor("Some".into(), vec![IRPattern::Variable("v".into())]),
                    v("v"),
                ),
                (IRPattern::Constructor("None".into(), vec![]), v("d")),
            ],
        }),
        "option_ok_or" => Some(IRNode::Match {
            scrutinee: Box::new(v("o")),
            cases: vec![
                (
                    IRPattern::Constructor("Some".into(), vec![IRPattern::Variable("v".into())]),
                    IRNode::Call {
                        function: "Ok".into(),
                        args: vec![v("v")],
                    },
                ),
                (
                    IRPattern::Constructor("None".into(), vec![]),
                    IRNode::Call {
                        function: "Err".into(),
                        args: vec![v("e")],
                    },
                ),
            ],
        }),
        // ── Phase 115: the List family ──────────────────
        // list_is_empty is Tier-1: the equation seed's
        // body — the row's own right-hand side, crowned
        // by the pipeline exactly like its String sibling
        // (Z3-Verified ∧ golden-exact; the crowning pin
        // covers it with the other Tier-1 entries).
        "list_is_empty" => Some(bop(
            IRBinOp::Eq,
            IRNode::Call {
                function: "len".into(),
                args: vec![v("xs")],
            },
            i(0),
        )),
        // The eleven Tier-2 entries: HAND bodies at the
        // executed standard (STDLIB_SCOPE §2.6, D7a),
        // shipped because the pipeline crowns no
        // Verified body for any of them and its executed
        // crowns are unstable samples (measured in
        // Phase 115: `list_is_sorted`'s crown flips
        // between the template scan at 9/9 and a
        // constant-true body at 6/9 across runs), while
        // the library ships pinned, deterministic data. Each body is written in the one
        // vocabulary BOTH executed judges reach in full:
        // For scans whose loop variable is the index
        // (the compiled form's only usable index — the
        // G5 boundary: generated Rust cannot index a Vec
        // by an Int variable), Alloc/Assign accumulators,
        // statement Append, and Int counters wherever an
        // Int is compared or produced. The Phase 115 pin
        // re-proves each body's executed-exactness on the
        // full battery through both routes (the in-process
        // interpreter — For arm, Phase 115 — and the
        // compiled runner's rendering via production
        // codegen + rustc). Verdicts are Inconclusive:
        // element-level verification is the §3.9 wall,
        // the post-1.0 encoder campaign's to close, with
        // this family as its test corpus.
        "list_reverse" => Some(block(vec![
            alloc("out", list_ty(), empty_list()),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                append_stmt(
                    "out",
                    idx(
                        v("xs"),
                        bop(IRBinOp::Sub, bop(IRBinOp::Sub, len_u(v("xs")), i(1)), v("i")),
                    ),
                ),
            ),
            ret(v("out")),
        ])),
        "list_contains" => Some(block(vec![
            alloc("found", TypeRef::Bool, boolean(false)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                assign(
                    "found",
                    bop(
                        IRBinOp::Or,
                        v("found"),
                        bop(IRBinOp::Eq, idx(v("xs"), v("i")), v("x")),
                    ),
                ),
            ),
            ret(v("found")),
        ])),
        "list_sum" => Some(block(vec![
            alloc("acc", TypeRef::Int, i(0)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                assign("acc", bop(IRBinOp::Add, v("acc"), idx(v("xs"), v("i")))),
            ),
            ret(v("acc")),
        ])),
        "list_product" => Some(block(vec![
            alloc("acc", TypeRef::Int, i(1)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                assign("acc", bop(IRBinOp::Mul, v("acc"), idx(v("xs"), v("i")))),
            ),
            ret(v("acc")),
        ])),
        "list_min" => Some(if_(
            bop(IRBinOp::Eq, len_c(v("xs")), i(0)),
            none(),
            block(vec![
                alloc("m", TypeRef::Int, idx(v("xs"), i(0))),
                for_(
                    "i",
                    i(1),
                    len_u(v("xs")),
                    assign(
                        "m",
                        if_(
                            bop(IRBinOp::Lt, idx(v("xs"), v("i")), v("m")),
                            idx(v("xs"), v("i")),
                            v("m"),
                        ),
                    ),
                ),
                ret(some(v("m"))),
            ]),
        )),
        "list_max" => Some(if_(
            bop(IRBinOp::Eq, len_c(v("xs")), i(0)),
            none(),
            block(vec![
                alloc("m", TypeRef::Int, idx(v("xs"), i(0))),
                for_(
                    "i",
                    i(1),
                    len_u(v("xs")),
                    assign(
                        "m",
                        if_(
                            bop(IRBinOp::Gt, idx(v("xs"), v("i")), v("m")),
                            idx(v("xs"), v("i")),
                            v("m"),
                        ),
                    ),
                ),
                ret(some(v("m"))),
            ]),
        )),
        "list_take" => Some(block(vec![
            alloc("out", list_ty(), empty_list()),
            alloc("k", TypeRef::Int, i(0)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                block(vec![
                    if_(
                        bop(IRBinOp::Lt, v("k"), v("n")),
                        append_stmt("out", idx(v("xs"), v("i"))),
                        block(vec![]),
                    ),
                    assign("k", bop(IRBinOp::Add, v("k"), i(1))),
                ]),
            ),
            ret(v("out")),
        ])),
        "list_drop" => Some(block(vec![
            alloc("out", list_ty(), empty_list()),
            alloc("k", TypeRef::Int, i(0)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                block(vec![
                    if_(
                        bop(IRBinOp::Ge, v("k"), v("n")),
                        append_stmt("out", idx(v("xs"), v("i"))),
                        block(vec![]),
                    ),
                    assign("k", bop(IRBinOp::Add, v("k"), i(1))),
                ]),
            ),
            ret(v("out")),
        ])),
        // The AdjacentScan shape (the standing template's
        // construction, hand-written): accumulate the
        // conjunction over adjacent pairs.
        "list_is_sorted" => Some(block(vec![
            alloc("ok", TypeRef::Bool, boolean(true)),
            for_(
                "i",
                i(1),
                len_u(v("xs")),
                assign(
                    "ok",
                    bop(
                        IRBinOp::And,
                        v("ok"),
                        bop(
                            IRBinOp::Le,
                            idx(v("xs"), bop(IRBinOp::Sub, v("i"), i(1))),
                            idx(v("xs"), v("i")),
                        ),
                    ),
                ),
            ),
            ret(v("ok")),
        ])),
        // Insertion sort: each element of xs is inserted
        // into the accumulated sorted list at the first
        // position whose element exceeds it (the position
        // scan and the rebuild both count with Int
        // counters alongside the For variables).
        "list_sort" => Some(block(vec![
            alloc("out", list_ty(), empty_list()),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                block(vec![
                    alloc("x", TypeRef::Int, idx(v("xs"), v("i"))),
                    alloc("pos", TypeRef::Int, len_c(v("out"))),
                    alloc("k", TypeRef::Int, i(0)),
                    for_(
                        "j",
                        i(0),
                        len_u(v("out")),
                        block(vec![
                            if_(
                                bop(
                                    IRBinOp::And,
                                    bop(IRBinOp::Eq, v("pos"), len_c(v("out"))),
                                    bop(IRBinOp::Gt, idx(v("out"), v("j")), v("x")),
                                ),
                                assign("pos", v("k")),
                                block(vec![]),
                            ),
                            assign("k", bop(IRBinOp::Add, v("k"), i(1))),
                        ]),
                    ),
                    alloc("next", list_ty(), empty_list()),
                    alloc("k2", TypeRef::Int, i(0)),
                    for_(
                        "j",
                        i(0),
                        len_u(v("out")),
                        block(vec![
                            if_(
                                bop(IRBinOp::Eq, v("k2"), v("pos")),
                                append_stmt("next", v("x")),
                                block(vec![]),
                            ),
                            append_stmt("next", idx(v("out"), v("j"))),
                            assign("k2", bop(IRBinOp::Add, v("k2"), i(1))),
                        ]),
                    ),
                    if_(
                        bop(IRBinOp::Eq, v("pos"), len_c(v("out"))),
                        append_stmt("next", v("x")),
                        block(vec![]),
                    ),
                    assign("out", v("next")),
                ]),
            ),
            ret(v("out")),
        ])),
        // The G5 scope-around, as documented on the
        // entry: a first-match scan (the While-form
        // binary search cannot compile — generated Rust
        // has no Int-variable index). The counter k is
        // the Int the Some payload carries; the For
        // variable is only ever the index.
        "list_binary_search" => Some(block(vec![
            alloc(
                "res",
                TypeRef::Option(Box::new(TypeRef::Int)),
                none(),
            ),
            alloc("k", TypeRef::Int, i(0)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                block(vec![
                    if_(
                        bop(
                            IRBinOp::And,
                            bop(IRBinOp::Eq, idx(v("xs"), v("i")), v("x")),
                            bop(IRBinOp::Eq, v("res"), none()),
                        ),
                        assign("res", some(v("k"))),
                        block(vec![]),
                    ),
                    assign("k", bop(IRBinOp::Add, v("k"), i(1))),
                ]),
            ),
            ret(v("res")),
        ])),
        // ── Phase 135: the Amendment 2 entries ──────
        // Hand bodies in the standing Phase 115
        // vocabulary (For scans, Alloc/Assign
        // accumulators, statement Append), applying the
        // function parameter through a Call — the
        // Map/Filter/Fold IR forms do not compile in
        // Rust under the standing rendering (E0631: a
        // fn pointer is not `FnMut(&i64) -> i64`,
        // measured at Phase 135), so the For-loop form
        // is the one vocabulary both executed judges
        // reach in full. Crowned Tier-2 at Phase 135:
        // executed-exact on the Amendment 2 batteries
        // (105 pinned points) through the index-carrying
        // route; pinned in tests/phase135_amendment_
        // build.rs.
        "list_map" => Some(block(vec![
            alloc("out", list_ty(), empty_list()),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                append_stmt("out", call("f", vec![idx(v("xs"), v("i"))])),
            ),
            ret(v("out")),
        ])),
        "list_filter" => Some(block(vec![
            alloc("out", list_ty(), empty_list()),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                if_(
                    call("p", vec![idx(v("xs"), v("i"))]),
                    append_stmt("out", idx(v("xs"), v("i"))),
                    block(vec![]),
                ),
            ),
            ret(v("out")),
        ])),
        "list_fold" => Some(block(vec![
            alloc("acc", TypeRef::Int, v("init")),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                assign("acc", call("f", vec![v("acc"), idx(v("xs"), v("i"))])),
            ),
            ret(v("acc")),
        ])),
        "list_all" => Some(block(vec![
            alloc("ok", TypeRef::Bool, boolean(true)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                assign(
                    "ok",
                    bop(
                        IRBinOp::And,
                        v("ok"),
                        call("p", vec![idx(v("xs"), v("i"))]),
                    ),
                ),
            ),
            ret(v("ok")),
        ])),
        "list_any" => Some(block(vec![
            alloc("ok", TypeRef::Bool, boolean(false)),
            for_(
                "i",
                i(0),
                len_u(v("xs")),
                assign(
                    "ok",
                    bop(
                        IRBinOp::Or,
                        v("ok"),
                        call("p", vec![idx(v("xs"), v("i"))]),
                    ),
                ),
            ),
            ret(v("ok")),
        ])),
        // The probed body (Phase 127's range_upto,
        // judged 2/2 in-process) generalized by its For
        // bounds alone (Amendment 2's recorded
        // resolution): the loop variable runs lo..hi
        // and is itself the appended value; lo >= hi
        // runs zero iterations — the empty list, no
        // descending form.
        "range" => Some(block(vec![
            alloc("out", list_ty(), empty_list()),
            for_("i", v("lo"), v("hi"), append_stmt("out", v("i"))),
            ret(v("out")),
        ])),
        _ => None,
    }
}

/// The stated loop invariant for a shipped body's `For`
/// loop (Phase 141, LIST_ENCODER_SCOPE §2.6): spec-syntax
/// constraint rows over the loop's environment, in the
/// Phase 96 invariant dialect (rows separated by `;`,
/// parsed by the spec parser's own constraint parser).
///
/// `IRNode::For` carries no invariant field — the IR
/// gives that field to While/Loop only — so a shipped
/// body's loop states its invariant here, authored
/// beside the bodies themselves as part of the library's
/// verification data. The verifier reaches this table
/// only through `verify_one`, keyed by spec name exactly
/// as the engine reaches `shipped_body`; the Phase 96
/// obligations (initiation / consecution / exit)
/// discharge the text against the body before any fact
/// from it is used, so a text the body does not preserve
/// fails the route and the verdict stays Inconclusive —
/// the table can state, never assert. Each entry below
/// has exactly one `For` loop; the text is that loop's
/// invariant, in the prefix form of the entry's own
/// postconditions (the counter `i` is the prefix bound;
/// `range`'s output elements are what its invariant
/// says, nothing more — §2.6's named exhibit).
///
/// The rows are written against the encoder's standing
/// semantics: a symbolic length is an Int the sessions
/// know only as a count (`>= 0` is a judge-session side
/// condition, Phase 141), so a bound the loop head
/// reaches only on the non-degenerate case is stated as
/// an implication (`len(xs) >= 0 implies i <= len(xs)`),
/// never as a bare fact the entry state cannot carry.
pub fn shipped_loop_invariant(name: &str) -> Option<&'static str> {
    match name {
        // out is the reversed prefix: its length is the
        // counter, and position j holds the element the
        // full reversal places there.
        "list_reverse" => Some(
            "len(out) == i; 0 <= i; len(xs) >= 0 implies i <= len(xs); \
             forall j: Int in range(len(out)): out[j] == xs[(len(xs) - 1) - j]",
        ),
        // found is the prefix disjunction, stated in both
        // directions: every hit in the prefix is recorded
        // (the forall form), and a recorded hit is
        // witnessed in the prefix (the exists form).
        "list_contains" => Some(
            "0 <= i; len(xs) >= 0 implies i <= len(xs); \
             forall j: Int in range(len(xs)): j >= i || xs[j] != x || found == true; \
             found == true implies (exists j: Int in range(len(xs)): j < i && xs[j] == x)",
        ),
        // m is the prefix minimum: bounded above by every
        // element seen, and attained by one of them.
        "list_min" => Some(
            "1 <= i; len(xs) >= 1 implies i <= len(xs); \
             forall j: Int in range(len(xs)): j >= i || m <= xs[j]; \
             exists j: Int in range(len(xs)): j < i && xs[j] == m",
        ),
        // m is the prefix maximum, dually.
        "list_max" => Some(
            "1 <= i; len(xs) >= 1 implies i <= len(xs); \
             forall j: Int in range(len(xs)): j >= i || m >= xs[j]; \
             exists j: Int in range(len(xs)): j < i && xs[j] == m",
        ),
        // k counts in lockstep with i; out is the prefix
        // of xs truncated at n (the three length cases
        // are the postconditions' own, at the prefix).
        "list_take" => Some(
            "k == i; 0 <= i; len(xs) >= 0 implies i <= len(xs); \
             n <= 0 implies len(out) == 0; \
             n >= i implies len(out) == i; \
             n > 0 && n < i implies len(out) == n; \
             forall j: Int in range(len(out)): out[j] == xs[j]",
        ),
        // out is the prefix of xs past n (the same three
        // cases), and its elements are xs's, shifted by n
        // on the positive case and unshifted otherwise —
        // the two element rows are the postconditions'
        // own guarded rows at the prefix.
        "list_drop" => Some(
            "k == i; 0 <= i; len(xs) >= 0 implies i <= len(xs); \
             n <= 0 implies len(out) == i; \
             n >= i implies len(out) == 0; \
             n > 0 && n < i implies len(out) == i - n; \
             forall j: Int in range(len(out)): n > 0 implies out[j] == xs[j + n]; \
             forall j: Int in range(len(out)): n <= 0 implies out[j] == xs[j]",
        ),
        // ok is the conjunction over the adjacent pairs
        // strictly inside the prefix: while ok holds,
        // every such pair is ordered; a false ok is
        // witnessed by an inverted pair inside it.
        "list_is_sorted" => Some(
            "1 <= i; len(xs) >= 1 implies i <= len(xs); \
             forall j: Int in range(len(xs)): j + 1 >= i || ok == false || xs[j] <= xs[j + 1]; \
             ok == false implies (exists j: Int in range(len(xs)): j + 1 < i && xs[j] > xs[j + 1])",
        ),
        // out is the built prefix of the range: its
        // length is the counter's distance from lo, and
        // position j holds lo + j. The empty case is
        // carried by the second row (before the first
        // iteration out is empty whatever hi says; once
        // hi > lo the loop's own progress keeps the
        // length equation exact).
        "range" => Some(
            "len(out) == i - lo; len(out) == 0 || hi > lo; \
             hi >= lo implies i <= hi; \
             forall j: Int in range(len(out)): out[j] == lo + j",
        ),
        // acc is the fold over the consumed prefix, in
        // §2.8's prefix form itself (Phase 142): the
        // fold over `list_take(xs, i)` under the
        // contract's own op. Initiation is the base row
        // (the empty prefix folds to the initial value),
        // consecution is the step row at the counter,
        // and at exit the counter is the length, where
        // the prefix is the whole list — the
        // postcondition's own fold.
        "list_sum" => Some(
            "0 <= i; len(xs) >= 0 implies i <= len(xs); \
             acc == fold(list_take(xs, i), 0, |acc, v| acc + v)",
        ),
        // The product, dually: initial value 1, the
        // multiplying op.
        "list_product" => Some(
            "0 <= i; len(xs) >= 0 implies i <= len(xs); \
             acc == fold(list_take(xs, i), 1, |acc, v| acc * v)",
        ),
        // The fold of the parameter `f` itself: the
        // prefix form over UF applications (§2.9's Fold
        // point) — congruence is all the step row
        // carries, and all the invariant states.
        "list_fold" => Some(
            "0 <= i; len(xs) >= 0 implies i <= len(xs); \
             acc == fold(list_take(xs, i), init, f)",
        ),
        // out is the mapped prefix (Phase 143, §2.9's
        // Map point): its length is the counter, and
        // position j holds the UF's application at the
        // input's element there — pointwise congruence
        // at guarded indices, nothing more.
        "list_map" => Some(
            "len(out) == i; 0 <= i; len(xs) >= 0 implies i <= len(xs); \
             forall j: Int in range(len(out)): out[j] == f(xs[j])",
        ),
        // out is a filtered prefix, stated at exactly
        // its contract's strength (§2.9's Filter point,
        // §9 R6): no longer than the consumed prefix,
        // and the predicate holds at every element it
        // holds — no order, no completeness, no
        // subsequence claim.
        "list_filter" => Some(
            "len(out) <= i; 0 <= i; len(xs) >= 0 implies i <= len(xs); \
             forall j: Int in range(len(out)): p(out[j]) == true",
        ),
        // ok is the prefix conjunction, stated in both
        // directions (§2.9's All point): a failing
        // element inside the prefix is recorded (the
        // forall form), and a recorded failure is
        // witnessed inside the prefix (the exists form).
        "list_all" => Some(
            "0 <= i; len(xs) >= 0 implies i <= len(xs); \
             forall j: Int in range(len(xs)): j >= i || p(xs[j]) == true || ok == false; \
             ok == false implies (exists j: Int in range(len(xs)): j < i && p(xs[j]) == false)",
        ),
        // ok is the prefix disjunction, dually (the Any
        // point): a passing element inside the prefix
        // is recorded, and a recorded pass is witnessed
        // inside the prefix.
        "list_any" => Some(
            "0 <= i; len(xs) >= 0 implies i <= len(xs); \
             forall j: Int in range(len(xs)): j >= i || p(xs[j]) == false || ok == true; \
             ok == true implies (exists j: Int in range(len(xs)): j < i && p(xs[j]) == true)",
        ),
        _ => None,
    }
}
