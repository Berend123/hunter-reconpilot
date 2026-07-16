use std::path::PathBuf;

use clap::{Parser, Subcommand};

const CLI_HELP_FOOTER: &str = "\
Examples:
  reconpilot init
  reconpilot doctor
  reconpilot plan --scope config/scope.example.txt
  reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/
  reconpilot pipeline --scope config/scope.example.txt --profile active-lite --out output/ --execute
  reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/ --include-codex
  reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/ --include-codex --execute-codex
  reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/
  reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/ --execute-codex --limit 3
  reconpilot codex-review --input output/codex-insights/ --out output/codex-review/
  reconpilot validate --input output/

Safety:
  External-tool phases stay dry-run unless --execute is passed.
  Local-only phases never contact targets.
  Codex reasoning stays plan-only unless --execute-codex is passed.
  Pipeline --execute never implies --execute-codex.
  Scope validation is required before target-touching phases.
  ReconPilot is recon-only and does not ship exploit tooling.";

#[derive(Debug, Parser)]
#[command(
    name = "reconpilot",
    version,
    about = "Modern Rust-based recon orchestration skeleton",
    long_about = "ReconPilot coordinates recon data collection, normalization, enrichment, scoring, and reporting without shipping exploit tooling.",
    after_help = CLI_HELP_FOOTER
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Prepare local folders and confirm the workspace layout.
    Init,
    /// Show the supported tool registry and placeholder install expectations.
    CheckTools,
    /// Run a local MVP environment doctor and safety/readiness checklist.
    Doctor,
    /// Validate scope and print a planned orchestration flow.
    Plan {
        #[arg(long, value_name = "FILE")]
        scope: PathBuf,
    },
    /// Validate scope, create output folders safely, and print a placeholder run plan.
    Run {
        #[arg(long, value_name = "FILE")]
        scope: PathBuf,
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
        #[arg(long, default_value_t = false)]
        execute: bool,
    },
    /// Orchestrate named ReconPilot phase profiles in the correct order.
    Pipeline {
        #[arg(long, value_name = "FILE")]
        scope: PathBuf,
        #[arg(long, value_name = "NAME")]
        profile: String,
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
        #[arg(long, default_value_t = false)]
        execute: bool,
        #[arg(long, default_value_t = false)]
        include_codex: bool,
        #[arg(long, default_value_t = false)]
        execute_codex: bool,
    },
    /// Build safe mapping plans and a placeholder application map.
    Map {
        #[arg(long, value_name = "FILE")]
        scope: PathBuf,
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
        #[arg(long, default_value_t = false)]
        execute: bool,
    },
    /// Build graph-aware correlation plans and local graph artifacts from existing output data.
    Graph {
        #[arg(long, value_name = "DIR")]
        input: PathBuf,
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
        #[arg(long, default_value_t = false)]
        execute: bool,
    },
    /// Deterministically enrich graph artifacts into semantic overlays and risk explanations.
    Enrich {
        #[arg(long, value_name = "DIR")]
        input: PathBuf,
        #[arg(long, value_name = "DIR")]
        api_intel: Option<PathBuf>,
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
    },
    /// Build an analyst-facing review workspace from local enrichment artifacts.
    Review {
        #[arg(long, value_name = "DIR")]
        input: PathBuf,
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
    },
    /// Build local LLM-ready context bundles and prompt packs without executing any model.
    LlmPack {
        #[arg(long, value_name = "DIR")]
        input: PathBuf,
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
        #[arg(long, value_name = "CHARS", default_value_t = 12_000)]
        max_context_chars: usize,
    },
    /// Build optional Codex command plans or analyst-controlled reasoning outputs from llm-pack artifacts.
    CodexRun {
        #[arg(long, value_name = "DIR")]
        pack: PathBuf,
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
        #[arg(long, default_value_t = false)]
        execute_codex: bool,
        #[arg(long, value_name = "COUNT", default_value_t = 3)]
        limit: usize,
        #[arg(long, value_name = "NAME")]
        template: Option<String>,
    },
    /// Review and annotate Codex reasoning outputs without modifying the original artifacts.
    CodexReview {
        #[arg(long, value_name = "DIR")]
        input: PathBuf,
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
    },
    /// Analyze local API, schema, auth, and JavaScript artifacts without contacting targets.
    ApiIntel {
        #[arg(long, value_name = "DIR")]
        input: PathBuf,
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
    },
    /// Validate the local output tree, integrity references, and structured artifacts.
    Validate {
        #[arg(long, value_name = "DIR")]
        input: PathBuf,
    },
    /// Normalize raw URL input into canonical records.
    Normalize {
        #[arg(long, value_name = "FILE")]
        input: PathBuf,
    },
    /// Apply placeholder keyword-based scoring to finding records.
    Score {
        #[arg(long, value_name = "FILE")]
        input: PathBuf,
    },
    /// Build a report preview from scored findings.
    Report {
        #[arg(long, value_name = "FILE")]
        input: PathBuf,
    },
}
