// ┌────────────────────────────────────────────────────────────┐
// │  MORPHIC — Self-Synthesizing Programming Language         │
// │  "Write specs. The compiler writes the implementation."   │
// │                                                            │
// │  CLI shell (Phase 66, v0.6 W1): the compiler core lives   │
// │  in the library (src/lib.rs); this binary is argument     │
// │  parsing, the filesystem contract, and presentation       │
// │  over it. The core modules are re-exported at this root   │
// │  so the test/measurement modules declared below keep      │
// │  their `crate::…` paths unchanged. The `llm` module       │
// │  stays in this target: nothing in the pipeline imports    │
// │  it, and its HTTP client must never enter the library's   │
// │  (hence the browser's) dependency graph.                  │
// └────────────────────────────────────────────────────────────┘

pub use morphic::{codegen, spec, synthesis, verify};

#[cfg(feature = "llm-client")]
mod llm;
mod m2_synthesis_probe;
mod m2_cross_validation;
mod m3_synthesis_probe;
mod m3_cross_validation;
mod tokenizer_synthesis_probe;
mod tokenizer_cross_validation;
mod tokenizer_benchmark;
mod tokenizer_profile;
mod phase53_measurement;
// The Phase 58–62 measurement instruments measure the
// compiled-runner stage; without that stage (browser
// profile) they have nothing to measure and are not
// compiled.
#[cfg(feature = "compiled-runner")]
mod phase58_measurement;
#[cfg(feature = "compiled-runner")]
mod phase59_measurement;
#[cfg(feature = "compiled-runner")]
mod phase60_measurement;
#[cfg(feature = "compiled-runner")]
mod phase61_measurement;
#[cfg(feature = "compiled-runner")]
mod phase62_measurement;


use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "morphic")]
#[command(about = "Self-Synthesizing Programming Language Compiler")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile a Morphic specification into an optimized implementation
    Build {
        /// Path to .morph spec file
        input: Option<PathBuf>,

        /// Assemble the npm package (@thvtzy/morphic) from
        /// the standard library's shipped bodies instead of
        /// compiling a spec file (Phase 122: the assembly
        /// re-runs the crowning checks and refuses on a
        /// moved crown; takes no input file)
        #[arg(long)]
        package: bool,

        /// Assemble the Rust facade crate (crates.io
        /// `morphic`) from the standard library's shipped
        /// bodies instead of compiling a spec file
        /// (Phase 123: the assembly re-runs the crowning
        /// checks and refuses on a moved crown; takes no
        /// input file)
        #[arg(long)]
        facade: bool,

        /// Directory the assembled package is written to
        /// (with --package or --facade; default: the
        /// package's repo home, packages/npm resp.
        /// packages/rust)
        #[arg(long)]
        out: Option<PathBuf>,

        /// Output language (rust, c, wasm)
        #[arg(short, long, default_value = "rust")]
        target: String,

        /// Optimization level (1-4)
        #[arg(short = 'O', long, default_value = "3")]
        opt_level: u8,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Verify a spec without generating implementation
    Check {
        /// Path to .morph spec file
        input: PathBuf,
    },

    /// Synthesize interactively — see candidates in real-time
    Synthesize {
        /// Path to .morph spec file
        input: PathBuf,

        /// Maximum number of synthesis iterations
        #[arg(short, long, default_value = "10000")]
        iterations: usize,
    },

    /// Start the Morphic language server (LSP)
    Lsp,

    /// Initialize a new Morphic project
    Init {
        /// Project name
        name: String,
    },

    /// Print the standard library as a JSON asset
    /// (playground build tooling, Phase 112: regenerates
    /// playground/stdlib-asset.bin from the embedded
    /// library data)
    #[command(hide = true)]
    StdlibAsset,
}

fn main() -> anyhow::Result<()> {
    let banner = r#"
    ╔══════════════════════════════════════════════╗
    ║  ███╗   ███╗ ██████╗ ██████╗ ██████╗ ██╗  ██╗ ██╗ ██████╗
    ║  ████╗ ████║██╔═══██╗██╔══██╗██╔══██╗██║  ██║███║██╔════╝
    ║  ██╔████╔██║██║   ██║██████╔╝██████╔╝███████║╚██║██║
    ║  ██║╚██╔╝██║██║   ██║██╔══██╗██╔═══╝ ██╔══██║ ██║██║
    ║  ██║ ╚═╝ ██║╚██████╔╝██║  ██║██║     ██║  ██║ ██║╚██████╗
    ║  ╚═╝     ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═╝     ╚═╝  ╚═╝ ╚═╝ ╚═════╝
    ║                                                  v0.1.0
    ║     Self-Synthesizing Programming Language
    ║     "Don't write code. Write what you want."
    ╚══════════════════════════════════════════════╝
    "#;

    let cli = Cli::parse();

    // The stdlib-asset subcommand's stdout IS the asset
    // file (Phase 112) — the banner would corrupt it, so
    // that one command runs without it.
    if !matches!(cli.command, Command::StdlibAsset) {
        println!("{}", banner.cyan());
    }

    match cli.command {
        Command::Build { package: true, input, out, .. } => {
            // Phase 122: `morphic build --package` — assemble
            // the npm package (@thvtzy/morphic) from the
            // standard library's shipped bodies. The assembly
            // re-runs the crowning checks before generating
            // (codegen::package); a moved crown fails here and
            // nothing is written.
            if input.is_some() {
                anyhow::bail!(
                    "build --package assembles the standard-library package \
                     and takes no input file"
                );
            }
            // The assembly reads the EMBEDDED library data,
            // which exists only in native builds — under the
            // `wasm-playground` feature (which a workspace
            // --all-targets build unifies into this bin) the
            // command refuses by name instead, the StdlibAsset
            // precedent.
            #[cfg(not(feature = "wasm-playground"))]
            {
                let files = codegen::package::assemble_npm_package_checked()?;
                let mass = codegen::package::unpacked_mass(&files);
                if mass > codegen::package::NPM_UNPACKED_BUDGET {
                    anyhow::bail!(
                        "the assembled package is {mass} B unpacked, over the \
                         frozen budget of {} B (EMBED_SCOPE §4) — refusing to write",
                        codegen::package::NPM_UNPACKED_BUDGET
                    );
                }
                let dir =
                    out.unwrap_or_else(|| PathBuf::from(codegen::package::NPM_PACKAGE_DIR));
                std::fs::create_dir_all(&dir)?;
                for f in &files {
                    std::fs::write(dir.join(&f.path), &f.contents)?;
                }
                println!(
                    "{} Assembled {} — {} files, {mass} B unpacked (budget {} B)",
                    "✓".green().bold(),
                    codegen::bindings::PACKAGE_NAME,
                    files.len(),
                    codegen::package::NPM_UNPACKED_BUDGET
                );
                println!("  {} {}", "•".dimmed(), dir.display());
            }
            #[cfg(feature = "wasm-playground")]
            {
                let _ = out;
                anyhow::bail!(
                    "package assembly runs from a native build \
                     (this build carries no embedded library data)"
                );
            }
        }

        Command::Build { facade: true, input, out, .. } => {
            // Phase 123: `morphic build --facade` — assemble
            // the Rust facade crate (crates.io `morphic`)
            // from the standard library's shipped bodies.
            // The assembly re-runs the crowning checks
            // before generating (codegen::facade, sharing
            // codegen::package's crowning surface); a moved
            // crown fails here and nothing is written. The
            // crate carries no size budget (EMBED_SCOPE R8).
            if input.is_some() {
                anyhow::bail!(
                    "build --facade assembles the standard-library crate \
                     and takes no input file"
                );
            }
            // The assembly reads the EMBEDDED library data,
            // which exists only in native builds — under the
            // `wasm-playground` feature the command refuses
            // by name instead, the --package precedent.
            #[cfg(not(feature = "wasm-playground"))]
            {
                let files = codegen::facade::assemble_rust_facade_checked()?;
                let total: usize = files.iter().map(|f| f.contents.len()).sum();
                let dir =
                    out.unwrap_or_else(|| PathBuf::from(codegen::facade::RUST_PACKAGE_DIR));
                for f in &files {
                    let path = dir.join(&f.path);
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&path, &f.contents)?;
                }
                println!(
                    "{} Assembled {} (Rust facade crate) — {} files, {total} B \
                     (no size budget, EMBED_SCOPE R8)",
                    "✓".green().bold(),
                    codegen::facade::CRATE_NAME,
                    files.len()
                );
                println!("  {} {}", "•".dimmed(), dir.display());
            }
            #[cfg(feature = "wasm-playground")]
            {
                let _ = out;
                anyhow::bail!(
                    "facade assembly runs from a native build \
                     (this build carries no embedded library data)"
                );
            }
        }

        Command::Build { input, target, opt_level, output, .. } => {
            let input = input.ok_or_else(|| {
                anyhow::anyhow!("build requires a .morph input file (or --package)")
            })?;
            println!("{} Compiling {}...", "⚙".bold(), input.display());
            let source = std::fs::read_to_string(&input)?;

            // Phase 1: Parse
            println!("  {} Parsing specification...", "→".dimmed());
            let spec = spec::parser::parse(&source)?;

            // Phase 2: Type-check
            println!("  {} Type checking...", "→".dimmed());
            let type_checked = spec::typeck::check(spec)?;

            // The whole module stays in scope for synthesis and
            // verification: a spec's constraints may call sibling
            // specs, and those calls resolve against the module
            // (SpecIndex) — the same module-aware pipeline the M2
            // probe and cross-validation harness run. We still
            // build one spec at a time: the file's first spec.
            let functions = &type_checked.spec.functions;
            let spec = functions.first()
                .ok_or_else(|| anyhow::anyhow!("No spec functions found"))?
                .clone();

            // Phase 3: Synthesize
            println!("  {} Synthesizing implementations...", "→".dimmed());
            let candidates = synthesis::engine::synthesize_with_module(&spec, opt_level, functions)?;

            // Phase 4: Verify
            println!("  {} Verifying candidates...", "→".dimmed());
            let verified = verify::verifier::Verifier::with_specs(functions)
                .verify_all(candidates, &spec)?;

            if verified.is_empty() {
                anyhow::bail!("No implementation satisfied all constraints");
            }

            // Phase 4b (Phase 58, G1; extended Phase 60, G3):
            // compiled-runner final scoring. For specs whose
            // in-spec tests the IR interpreter cannot fully
            // bind (the lambda-carrying tuple residue — sort,
            // binary_search), the FALLBACK test_pass_ratio 1.0
            // is replaced for a bounded finalist set by a
            // measured compiled-runner ratio before selection;
            // since Phase 60 the stage likewise judges a
            // constraint-constructed seed the interpreter
            // cannot evaluate at all (the forall-fold scan —
            // count_matching), whose interpreter ratio is 0.0
            // by coverage rather than behavior. A no-op for
            // every other spec; infrastructure failure
            // degrades to the fallback behavior — it never
            // fails the build.
            let verified = synthesis::runner::rescore_finalists(verified, &spec);

            // Phase 5: Optimize & select
            println!("  {} Selecting optimal implementation...", "→".dimmed());
            let winner = synthesis::selector::select_best(verified, &spec)?;

            // Phase 6: Codegen
            println!("  {} Generating {} code...", "→".dimmed(), target);
            let code = codegen::generate(&winner, &target)?;

            // Phase 7: Output
            let out_path = output.unwrap_or_else(|| {
                input.with_extension(target)
            });
            std::fs::write(&out_path, &code)?;

            println!("\n{} {}", "✓".green().bold(), format!("Generated: {}", out_path.display()).green());
            println!("  {} {} candidates evaluated", "•".dimmed(), synthesis::engine::candidate_count());
            println!("  {} Synthesis time: {}ms", "•".dimmed(), synthesis::engine::elapsed_ms());
        }

        Command::Check { input } => {
            println!("{} Checking {}...", "✓".bold(), input.display());
            let source = std::fs::read_to_string(&input)?;
            let spec = spec::parser::parse(&source)?;
            let _spec = spec::typeck::check(spec)?;
            println!("{} Specification is valid.", "✓".green().bold());
        }

        Command::Synthesize { input, iterations } => {
            println!("{} Synthesizing interactively ({} max iterations)...",
                "🔍".bold(), iterations);
            // Interactive mode: stream candidates in real time.
            // The TUI is native-only furniture (the
            // `interactive` feature); a build without it
            // reports the fact rather than failing to compile.
            #[cfg(feature = "interactive")]
            {
                let source = std::fs::read_to_string(&input)?;
                let parsed = spec::parser::parse(&source)?;
                let type_checked = spec::typeck::check(parsed)?;
                let func_spec = type_checked.spec.functions.first()
                    .ok_or_else(|| anyhow::anyhow!("No spec functions found"))?;
                synthesis::interactive::run(func_spec, iterations)?;
            }
            #[cfg(not(feature = "interactive"))]
            {
                let _ = (input, iterations);
                anyhow::bail!(
                    "interactive synthesis is not available in this build \
                     (feature `interactive` is off)"
                );
            }
        }

        Command::Lsp => {
            println!("{} Starting Morphic Language Server...", "🔌".bold());
            // TODO: LSP implementation
        }

        Command::StdlibAsset => {
            // Phase 112: the asset generator. Stdout is
            // exactly the asset bytes (no banner, nothing
            // else): `morphic stdlib-asset >
            // playground/stdlib-asset.bin` regenerates the
            // checked-in file byte-for-byte. The generator
            // reads the EMBEDDED library data, which exists
            // only in native builds — under the
            // `wasm-playground` feature (which a workspace
            // --all-targets build unifies into this bin)
            // the subcommand refuses by name instead.
            #[cfg(not(feature = "wasm-playground"))]
            {
                use std::io::Write;
                std::io::stdout().write_all(&morphic::stdlib::library_asset_bytes())?;
            }
            #[cfg(feature = "wasm-playground")]
            {
                anyhow::bail!(
                    "stdlib-asset is generated from a native build \
                     (this build carries no embedded library data)"
                );
            }
        }

        Command::Init { name } => {
            let dir = PathBuf::from(&name);
            std::fs::create_dir_all(&dir)?;

            // Phase 110: the template corrected to reality.
            // There is no `morphic-std` crate: the standard
            // library ships with the compiler and is merged
            // into every module as a prelude (decision D2a),
            // so there is no dependency to declare and no
            // import to write — and the generated spec is a
            // real module that parses and checks, calling a
            // library entry by its plain name.
            let cargo_toml = format!(r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

# The standard library ships with the compiler (a prelude
# merged into every module) — there is no library
# dependency to declare.

[[bin]]
name = "{}"
path = "main.morph"
"#, name, name);

            let spec = format!(r#"// {} — Morphic Project
// Generated by morphic init
//
// The standard library is built in: its entries (such as
// int_abs) are available in every module by plain name —
// no import, no dependency.

{} {{
spec main {{
    input: x: Int
    output: Int

    constraint: output == int_abs(x)

    test {{
        input: (0 - 3)
        expect: 3
    }}
}}
}}
"#, name, name);

            std::fs::write(dir.join("Morphic.toml"), cargo_toml)?;
            std::fs::write(dir.join("main.morph"), spec)?;

            println!("{} Created new Morphic project: {}", "✓".green().bold(), name);
            println!("  cd {}", name);
            println!("  morphic build main.morph");
        }
    }

    Ok(())
}
