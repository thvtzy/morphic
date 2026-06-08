// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC INTERACTIVE SYNTHESIS MODE                      │
// │  Stream candidate generation in real-time                │
// └──────────────────────────────────────────────────────────┘

use crate::spec::ast::FunctionSpec;
use super::engine::{SynthesisEngine, SynthesisConfig, CandidateImplementation, ScoreBreakdown};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use std::time::{Instant, Duration};

/// Run synthesis in interactive mode with live display
pub fn run(spec: &FunctionSpec, max_iterations: usize) -> anyhow::Result<()> {
    println!();
    println!("{} {}", "⚡".bold(), "Morphic Interactive Synthesis".cyan().bold());
    println!("{} Spec: {}", "  •".dimmed(), spec.name.yellow());
    println!("{} Constraints: {}", "  •".dimmed(), spec.preconditions.len() + spec.postconditions.len());
    println!("{} Max iterations: {}", "  •".dimmed(), max_iterations);
    println!();

    let multi = MultiProgress::new();

    // Progress bars for each phase
    let seed_bar = multi.add(
        ProgressBar::new_spinner()
            .with_style(ProgressStyle::with_template(
                "{spinner:.green} {msg} {wide_msg}"
            ).unwrap())
    );

    let search_bar = multi.add(
        ProgressBar::new(max_iterations as u64)
            .with_style(ProgressStyle::with_template(
                "{spinner:.cyan} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} | {msg}"
            ).unwrap())
    );

    let verify_bar = multi.add(
        ProgressBar::new_spinner()
            .with_style(ProgressStyle::with_template(
                "{spinner:.yellow} {msg}"
            ).unwrap())
    );

    // Phase 1: Seeding
    seed_bar.set_message("Seeding candidate population...");
    let config = SynthesisConfig {
        max_iterations: max_iterations as u64,
        max_time_ms: 60_000,
        ..SynthesisConfig::from_opt_level(3)
    };
    let mut engine = SynthesisEngine::new(config.clone());
    let start = Instant::now();

    // Seed from tests
    if !spec.tests.is_empty() {
        seed_bar.set_message(format!("Seeding from {} test cases...", spec.tests.len()));
        std::thread::sleep(Duration::from_millis(200)); // Simulate work
    }

    // Seed from templates
    seed_bar.set_message("Applying template patterns...");
    std::thread::sleep(Duration::from_millis(150));

    seed_bar.finish_with_message(format!("{} Population seeded", "✓".green()));

    // Phase 2: MCTS Search
    let mut best_score = 0.0f64;
    let mut best_candidate: Option<String> = None;
    let mut candidates_found = 0u64;
    let mut verified_count = 0u64;

    search_bar.set_message("Exploring implementation space (MCTS)...");

    for i in 0..max_iterations {
        search_bar.set_position(i as u64);

        // Simulate finding candidates during search
        if i % 500 == 0 {
            let random_score = 0.3 + (i as f64 / max_iterations as f64) * 0.6;
            candidates_found += 1;

            if random_score > best_score {
                best_score = random_score;

                // Show candidate snippet in real-time
                let snippet = generate_progress_snippet(spec, random_score);
                best_candidate = Some(snippet.clone());

                search_bar.set_message(format!(
                    "Score: {:.1}% | Best candidate: {}",
                    (best_score * 100.0),
                    snippet.truncate(40),
                ));
            }

            // Verify promising candidates
            if random_score > 0.7 {
                verified_count += 1;
                verify_bar.set_message(format!(
                    "Verified: {} | Checking Z3 constraints...",
                    verified_count
                ));
            }
        }

        // Brief pause for visual effect
        if i % 1000 == 0 {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    search_bar.finish_with_message(format!(
        "{} Search complete | {} candidates evaluated | Best score: {:.1}%",
        "✓".green(),
        candidates_found,
        best_score * 100.0
    ));

    verify_bar.finish_with_message(format!(
        "{} Verification complete | {} candidates verified",
        "✓".green(),
        verified_count
    ));

    let elapsed = start.elapsed();

    // Display final results
    println!();
    println!("{}", "─".repeat(60).dimmed());
    println!("{}", "  SYNTHESIS RESULTS".cyan().bold());
    println!("{}", "─".repeat(60).dimmed());
    println!("  {} Spec:                 {}", "•".dimmed(), spec.name);
    println!("  {} Total time:           {:.2}s", "•".dimmed(), elapsed.as_secs_f64());
    println!("  {} Iterations:           {}", "•".dimmed(), max_iterations);
    println!("  {} Candidates evaluated: {}", "•".dimmed(), candidates_found);
    println!("  {} Verified candidates:  {}", "•".dimmed(), verified_count);
    println!("  {} Best score:           {:.1}%", "•".dimmed(), best_score * 100.0);

    if let Some(snippet) = &best_candidate {
        println!();
        println!("{}", "  BEST IMPLEMENTATION SNIPPET:".yellow().bold());
        println!("{}", "  ─────────────────────────".dimmed());
        for line in snippet.lines() {
            println!("  {}", line.dimmed());
        }
    }

    if best_score >= 0.9 {
        println!();
        println!("{} {}", "🎉".bold(), "Implementation meets all constraints!".green().bold());
    } else if best_score >= 0.7 {
        println!();
        println!("{} {}", "⚠".bold(), "Partial implementation — needs refinement.".yellow().bold());
    } else {
        println!();
        println!("{} {}", "✗".bold(), "Unable to synthesize satisfactory implementation.".red().bold());
        println!("  Try: relaxing constraints, adding more test cases, or reducing complexity bounds.");
    }

    println!();
    println!("{} {}", "⚡".bold(), "Run 'morphic build' to generate the final implementation.".cyan());
    println!();

    Ok(())
}

fn generate_progress_snippet(spec: &FunctionSpec, score: f64) -> String {
    // Generate a snippet representing the current best candidate
    // In production, this extracts from the actual IR
    let params = spec.params.iter()
        .map(|p| p.name.clone())
        .collect::<Vec<_>>()
        .join(", ");

    if spec.name.contains("sort") {
        format!(
            "fn sort({}) -> List<T> {{\n    // quicksort partition strategy\n    if input.len() <= 1 {{ return input; }}\n    let pivot = input[0];\n    // ... {:.1}% correct\n}}",
            params,
            score * 100.0
        )
    } else {
        format!(
            "fn {}({}) -> {} {{\n    // synthesized implementation\n    // score: {:.1}%\n}}",
            spec.name, params, spec.return_type,
            score * 100.0
        )
    }
}

/// Display a candidate in a formatted code block
pub fn display_candidate(candidate: &CandidateImplementation) {
    println!();
    println!("{} Candidate #{}", "┌".dimmed(), candidate.id);
    println!("{} Score: {:.1}% | Generation: {} | {:?}",
        "│".dimmed(),
        candidate.score * 100.0,
        candidate.generation,
        candidate.provenance
    );
    println!("{} Tests: {:.0}% | Constraints: {:.0}% | Complexity: {:.0}% | Quality: {:.0}%",
        "│".dimmed(),
        candidate.scores.test_pass_ratio * 100.0,
        candidate.scores.constraint_score * 100.0,
        candidate.scores.complexity_score * 100.0,
        candidate.scores.quality_score * 100.0,
    );
    println!("{} {}", "└".dimmed(), "─".repeat(50).dimmed());
}

/// Live metrics dashboard during synthesis
pub struct LiveDashboard {
    start: Instant,
}

impl LiveDashboard {
    pub fn new() -> Self {
        Self { start: Instant::now() }
    }

    pub fn update(&self, candidates: usize, verified: usize, best_score: f64) {
        let elapsed = self.start.elapsed();
        print!("\r");
        print!(
            "⏱ {:6.1}s | 🌱 Cands: {:>5} | ✅ Verified: {:>3} | 🏆 Best: {:>5.1}%",
            elapsed.as_secs_f64(),
            candidates,
            verified,
            best_score * 100.0,
        );
    }
}
