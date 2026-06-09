// ┌────────────────────────────────────────────────────────────┐
// │  MORPHIC — Self-Synthesizing Programming Language         │
// │  "Write specs. The compiler writes the implementation."    │
// └────────────────────────────────────────────────────────────┘

mod spec;
mod synthesis;
mod verify;
mod codegen;

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
        input: PathBuf,

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

    println!("{}", banner.cyan());

    let cli = Cli::parse();

    match cli.command {
        Command::Build { input, target, opt_level, output } => {
            println!("{} Compiling {}...", "⚙".bold(), input.display());
            let source = std::fs::read_to_string(&input)?;

            // Phase 1: Parse
            println!("  {} Parsing specification...", "→".dimmed());
            let spec = spec::parser::parse(&source)?;

            // Phase 2: Type-check
            println!("  {} Type checking...", "→".dimmed());
            let type_checked = spec::typeck::check(spec)?;

            // Extract the first function spec for synthesis
            // A .morph file may contain multiple specs; we synthesize one at a time
            let spec = type_checked.spec.functions.first()
                .ok_or_else(|| anyhow::anyhow!("No spec functions found"))?
                .clone();

            // Phase 3: Synthesize
            println!("  {} Synthesizing implementations...", "→".dimmed());
            let candidates = synthesis::engine::synthesize(&spec, opt_level)?;

            // Phase 4: Verify
            println!("  {} Verifying candidates...", "→".dimmed());
            let verified = verify::verifier::verify_all(candidates, &spec)?;

            if verified.is_empty() {
                anyhow::bail!("No implementation satisfied all constraints");
            }

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
            // Interactive mode: stream candidates in real time
            let source = std::fs::read_to_string(&input)?;
            let parsed = spec::parser::parse(&source)?;
            let type_checked = spec::typeck::check(parsed)?;
            let func_spec = type_checked.spec.functions.first()
                .ok_or_else(|| anyhow::anyhow!("No spec functions found"))?;
            synthesis::interactive::run(func_spec, iterations)?;
        }

        Command::Lsp => {
            println!("{} Starting Morphic Language Server...", "🔌".bold());
            // TODO: LSP implementation
        }

        Command::Init { name } => {
            let dir = PathBuf::from(&name);
            std::fs::create_dir_all(&dir)?;

            let cargo_toml = format!(r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
morphic-std = "0.1"

[[bin]]
name = "{}"
path = "main.morph"
"#, name, name);

            let spec = format!(r#"// {} — Morphic Project
// Generated by morphic init

use morphic::std::*;

spec main {{
    input: ()
    output: ()

    constraint: true

    test {{
        // Your specifications here
    }}
}}
"#, name);

            std::fs::write(dir.join("Morphic.toml"), cargo_toml)?;
            std::fs::write(dir.join("main.morph"), spec)?;

            println!("{} Created new Morphic project: {}", "✓".green().bold(), name);
            println!("  cd {}", name);
            println!("  morphic build main.morph");
        }
    }

    Ok(())
}
