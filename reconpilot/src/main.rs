mod api;
mod audit;
mod auth;
mod classifiers;
mod cli;
mod codex_review;
mod codex_runner;
mod config;
mod correlation;
mod doctor;
mod enrichment;
mod graph;
mod jsintel;
mod llm_pack;
mod manifest;
mod mapping;
mod models;
mod normalize;
mod pipeline;
mod profiles;
mod report;
mod review;
mod runner;
mod schema;
mod scope;
mod scoring;
mod semantic;
mod tools;
mod utils;
mod validation;

use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Result};
use chrono::Utc;
use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::models::ReconToolRun;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let command_line = current_command_line();

    match cli.command {
        Commands::Init => {
            let output = utils::ensure_output_structure(Path::new("output"))?;
            utils::ensure_directory(Path::new("data"))?;
            println!("ReconPilot workspace initialized.");
            println!("Timestamp: {}", Utc::now().to_rfc3339());
            println!("Output root: {}", output.root.display());
            println!(
                "Existing output files: {}",
                utils::count_files(&output.root)
            );
        }
        Commands::CheckTools => {
            let registry = tools::registry();
            println!("Registered {} supported tools.", registry.len());
            for tool in registry {
                println!(
                    "- {} [{}] {} | {} | {}",
                    tool.name,
                    if tool.core { "core" } else { "optional" },
                    tool.category,
                    tool.output_format,
                    tool.safe_usage
                );
                println!("  Purpose: {}", tool.purpose);
            }
        }
        Commands::Doctor => {
            let report = doctor::run_doctor()?;
            doctor::print_doctor_summary(&report);
            if !report.errors.is_empty() {
                bail!(
                    "doctor found {} blocking issue(s); review the doctor summary before local MVP use.",
                    report.errors.len()
                );
            }
        }
        Commands::Plan { scope } => {
            let scope = scope::load_scope(&scope)?;
            let config = config::load_default_or_file(None)?;
            let consistency_warnings = config::validate_scope_exclusion_consistency(
                &scope,
                config::default_exclusion_path_if_exists().as_deref(),
            )?;
            let output = utils::ensure_output_structure(Path::new(&config.output_root))?;
            let plan = runner::build_execution_plan(&scope, &config, &output);
            runner::print_execution_plan(&plan);
            for warning in consistency_warnings {
                println!("Warning: {warning}");
            }
        }
        Commands::Run {
            scope,
            out,
            execute,
        } => {
            append_phase_start(&out, "run", &[scope.clone(), out.clone()]);
            let result: Result<_> = async {
                let scope_definition = scope::load_scope(&scope)?;
                let config_warnings = config::validate_scope_exclusion_consistency(
                    &scope_definition,
                    config::default_exclusion_path_if_exists().as_deref(),
                )?;
                let mut config = config::load_default_or_file(None)?;
                config.output_root = out.display().to_string();
                let output = utils::ensure_output_structure(&out)?;
                let tool_runs =
                    runner::run_tool_adapters(&scope_definition, &output, execute).await?;
                runner::print_tool_run_summary(
                    &scope_definition,
                    &config,
                    &output,
                    &tool_runs,
                    execute,
                );
                Ok((scope_definition, output, tool_runs, config_warnings))
            }
            .await;

            match result {
                Ok((scope_definition, output, tool_runs, config_warnings)) => {
                    let warnings = collect_run_warnings(&tool_runs);
                    let mut all_warnings = warnings.clone();
                    all_warnings.extend(config_warnings);
                    let output_paths = vec![
                        output.plans.join("tool-runs.json"),
                        output.plans.clone(),
                        output.raw.clone(),
                    ];
                    append_artifact_events(&out, "run", &output_paths);
                    if !execute {
                        append_dry_run_event(
                            &out,
                            "run",
                            "Dry-run plan created for recon adapters.",
                            vec![output.plans.display().to_string()],
                        );
                    }
                    append_warning_events(&out, "run", &all_warnings);
                    append_phase_completed(
                        &out,
                        "run",
                        "Recon run phase completed.",
                        &output_paths,
                    );
                    write_command_manifest(
                        &out,
                        &command_line,
                        vec![scope.clone()],
                        output_paths,
                        Some(scope_definition.source_path.clone()),
                        manifest::default_config_path_if_exists(),
                        all_warnings,
                        Vec::new(),
                    );
                }
                Err(error) => {
                    append_error_event(&out, "run", &error);
                    write_command_manifest(
                        &out,
                        &command_line,
                        vec![scope.clone()],
                        vec![out.join("plans"), out.join("raw")],
                        Some(scope.clone()),
                        manifest::default_config_path_if_exists(),
                        Vec::new(),
                        vec![format!("{error:#}")],
                    );
                    return Err(error);
                }
            }
        }
        Commands::Pipeline {
            scope,
            profile,
            out,
            execute,
            include_codex,
            execute_codex,
        } => {
            let scope_definition = scope::load_scope(&scope)?;
            let result = pipeline::run_pipeline(
                &scope,
                &profile,
                &out,
                execute,
                include_codex,
                execute_codex,
            )
            .await?;
            pipeline::print_pipeline_summary(&scope_definition, &out, &result);
            if !result.errors.is_empty() {
                bail!(
                    "pipeline '{}' completed with {} error(s); inspect {} and {}",
                    result.profile_name,
                    result.errors.len(),
                    result.manifest_path.display(),
                    out.join("validation-report.md").display()
                );
            }
        }
        Commands::Map {
            scope,
            out,
            execute,
        } => {
            append_phase_start(&out, "map", &[scope.clone(), out.clone()]);
            let result: Result<_> = async {
                let scope_definition = scope::load_scope(&scope)?;
                let config_warnings = config::validate_scope_exclusion_consistency(
                    &scope_definition,
                    config::default_exclusion_path_if_exists().as_deref(),
                )?;
                let mut config = config::load_default_or_file(None)?;
                config.output_root = out.display().to_string();
                let output = utils::ensure_output_structure(&out)?;
                let outcome =
                    mapping::run_mapping_layer(&scope_definition, &output, execute).await?;
                mapping::print_mapping_summary(
                    &scope_definition,
                    &config,
                    &output,
                    &outcome,
                    execute,
                );
                Ok((scope_definition, output, outcome, config_warnings))
            }
            .await;

            match result {
                Ok((scope_definition, output, outcome, config_warnings)) => {
                    let warnings = collect_run_warnings(&outcome.runs);
                    let mut all_warnings = warnings.clone();
                    all_warnings.extend(config_warnings);
                    let output_paths = vec![
                        outcome.map_json_path.clone(),
                        outcome.map_markdown_path.clone(),
                        output.plans.clone(),
                    ];
                    append_artifact_events(&out, "map", &output_paths);
                    if !execute {
                        append_dry_run_event(
                            &out,
                            "map",
                            "Dry-run mapping plans created.",
                            vec![output.plans.display().to_string()],
                        );
                    }
                    append_warning_events(&out, "map", &all_warnings);
                    append_phase_completed(&out, "map", "Mapping phase completed.", &output_paths);
                    write_command_manifest(
                        &out,
                        &command_line,
                        vec![scope.clone()],
                        output_paths,
                        Some(scope_definition.source_path.clone()),
                        manifest::default_config_path_if_exists(),
                        all_warnings,
                        Vec::new(),
                    );
                }
                Err(error) => {
                    append_error_event(&out, "map", &error);
                    write_command_manifest(
                        &out,
                        &command_line,
                        vec![scope.clone()],
                        vec![out.join("plans"), out.join("maps")],
                        Some(scope.clone()),
                        manifest::default_config_path_if_exists(),
                        Vec::new(),
                        vec![format!("{error:#}")],
                    );
                    return Err(error);
                }
            }
        }
        Commands::Graph {
            input,
            out,
            execute,
        } => {
            append_phase_start(&input, "graph", &[input.clone(), out.clone()]);
            match graph::run_graph_engine(&input, &out, execute) {
                Ok(outcome) => {
                    graph::print_graph_summary(&input, &out, &outcome);
                    let output_paths = collect_graph_output_paths(&outcome);
                    append_artifact_events(&input, "graph", &output_paths);
                    if !outcome.executed {
                        append_dry_run_event(
                            &input,
                            "graph",
                            "Dry-run graph plan created.",
                            vec![outcome.plan_path.display().to_string()],
                        );
                    }
                    append_phase_completed(
                        &input,
                        "graph",
                        "Graph phase completed.",
                        &output_paths,
                    );
                    write_command_manifest(
                        &input,
                        &command_line,
                        vec![input.clone()],
                        output_paths,
                        None,
                        manifest::default_config_path_if_exists(),
                        Vec::new(),
                        Vec::new(),
                    );
                }
                Err(error) => {
                    append_error_event(&input, "graph", &error);
                    write_command_manifest(
                        &input,
                        &command_line,
                        vec![input.clone()],
                        vec![out.clone()],
                        None,
                        manifest::default_config_path_if_exists(),
                        Vec::new(),
                        vec![format!("{error:#}")],
                    );
                    return Err(error);
                }
            }
        }
        Commands::Enrich {
            input,
            api_intel,
            out,
        } => {
            let output_root = input.parent().unwrap_or(&out).to_path_buf();
            append_phase_start(
                &output_root,
                "enrich",
                &collect_optional_paths(&[
                    Some(input.clone()),
                    api_intel.clone(),
                    Some(out.clone()),
                ]),
            );
            match enrichment::run_enrichment_engine(&input, api_intel.as_deref(), &out) {
                Ok(outcome) => {
                    enrichment::print_enrichment_summary(
                        &input,
                        api_intel.as_deref(),
                        &out,
                        &outcome,
                    );
                    let output_paths = vec![
                        outcome.semantic_assets_path.clone(),
                        outcome.observations_path.clone(),
                        outcome.risk_explanations_path.clone(),
                        outcome.enriched_graph_path.clone(),
                        outcome.summary_path.clone(),
                    ];
                    append_artifact_events(&output_root, "enrich", &output_paths);
                    append_warning_events(&output_root, "enrich", &outcome.warnings);
                    append_phase_completed(
                        &output_root,
                        "enrich",
                        "Enrichment phase completed.",
                        &output_paths,
                    );
                    write_command_manifest(
                        &output_root,
                        &command_line,
                        collect_optional_paths(&[Some(input.clone()), api_intel.clone()]),
                        output_paths,
                        None,
                        manifest::default_config_path_if_exists(),
                        outcome.warnings.clone(),
                        Vec::new(),
                    );
                }
                Err(error) => {
                    append_error_event(&output_root, "enrich", &error);
                    write_command_manifest(
                        &output_root,
                        &command_line,
                        collect_optional_paths(&[Some(input.clone()), api_intel.clone()]),
                        vec![out.clone()],
                        None,
                        manifest::default_config_path_if_exists(),
                        Vec::new(),
                        vec![format!("{error:#}")],
                    );
                    return Err(error);
                }
            }
        }
        Commands::Review { input, out } => {
            let output_root = input.parent().unwrap_or(&out).to_path_buf();
            append_phase_start(&output_root, "review", &[input.clone(), out.clone()]);
            match review::run_review_workspace(&input, &out) {
                Ok(outcome) => {
                    review::print_review_summary(&input, &out, &outcome);
                    let output_paths = vec![
                        outcome.priority_queue_markdown_path.clone(),
                        outcome.priority_queue_json_path.clone(),
                        outcome.asset_cards_dir.clone(),
                        outcome.review_checklist_path.clone(),
                        outcome.executive_summary_path.clone(),
                        outcome.evidence_index_path.clone(),
                    ];
                    append_artifact_events(&output_root, "review", &output_paths);
                    append_phase_completed(
                        &output_root,
                        "review",
                        "Review phase completed.",
                        &output_paths,
                    );
                    write_command_manifest(
                        &output_root,
                        &command_line,
                        vec![input.clone()],
                        output_paths,
                        None,
                        manifest::default_config_path_if_exists(),
                        Vec::new(),
                        Vec::new(),
                    );
                }
                Err(error) => {
                    append_error_event(&output_root, "review", &error);
                    write_command_manifest(
                        &output_root,
                        &command_line,
                        vec![input.clone()],
                        vec![out.clone()],
                        None,
                        manifest::default_config_path_if_exists(),
                        Vec::new(),
                        vec![format!("{error:#}")],
                    );
                    return Err(error);
                }
            }
        }
        Commands::LlmPack {
            input,
            out,
            max_context_chars,
        } => {
            append_phase_start(&input, "llm-pack", &[input.clone(), out.clone()]);
            match llm_pack::run_llm_pack(&input, &out, max_context_chars) {
                Ok(outcome) => {
                    llm_pack::print_llm_pack_summary(&input, &out, &outcome);
                    let output_paths = vec![
                        outcome.asset_contexts_dir.clone(),
                        outcome.prompts_dir.clone(),
                        outcome.reasoning_queue_json_path.clone(),
                        outcome.reasoning_queue_markdown_path.clone(),
                        outcome.analyst_brief_path.clone(),
                        outcome.pack_summary_path.clone(),
                    ];
                    append_artifact_events(&input, "llm-pack", &output_paths);
                    append_warning_events(&input, "llm-pack", &outcome.summary.warnings);
                    append_phase_completed(
                        &input,
                        "llm-pack",
                        "LLM pack phase completed.",
                        &output_paths,
                    );
                    write_command_manifest(
                        &input,
                        &command_line,
                        vec![input.clone()],
                        output_paths,
                        None,
                        manifest::default_config_path_if_exists(),
                        outcome.summary.warnings.clone(),
                        Vec::new(),
                    );
                }
                Err(error) => {
                    append_error_event(&input, "llm-pack", &error);
                    write_command_manifest(
                        &input,
                        &command_line,
                        vec![input.clone()],
                        vec![out.clone()],
                        None,
                        manifest::default_config_path_if_exists(),
                        Vec::new(),
                        vec![format!("{error:#}")],
                    );
                    return Err(error);
                }
            }
        }
        Commands::CodexRun {
            pack,
            out,
            execute_codex,
            limit,
            template,
        } => {
            let output_root = pack.parent().unwrap_or(&out).to_path_buf();
            append_phase_start(
                &output_root,
                "codex-run",
                &collect_optional_paths(&[Some(pack.clone()), Some(out.clone())]),
            );
            match codex_runner::run_codex_runner(
                &pack,
                &out,
                execute_codex,
                limit,
                template.clone(),
            ) {
                Ok(outcome) => {
                    codex_runner::print_codex_runner_summary(&pack, &out, &outcome);
                    let output_paths = vec![
                        outcome.plan_json_path.clone(),
                        outcome.plan_markdown_path.clone(),
                        outcome.results_dir.clone(),
                        outcome.logs_dir.clone(),
                        outcome.summary_markdown_path.clone(),
                        outcome.summary_json_path.clone(),
                    ];
                    append_artifact_events(&output_root, "codex-run", &output_paths);
                    if !execute_codex {
                        append_dry_run_event(
                            &output_root,
                            "codex-run",
                            "Dry-run Codex command plan created.",
                            vec![outcome.plan_json_path.display().to_string()],
                        );
                    }
                    append_warning_events(&output_root, "codex-run", &outcome.summary.warnings);
                    append_phase_completed(
                        &output_root,
                        "codex-run",
                        "Codex reasoning phase completed.",
                        &output_paths,
                    );
                    write_command_manifest(
                        &output_root,
                        &command_line,
                        vec![pack.clone()],
                        output_paths,
                        None,
                        manifest::default_config_path_if_exists(),
                        outcome.summary.warnings.clone(),
                        Vec::new(),
                    );
                }
                Err(error) => {
                    append_error_event(&output_root, "codex-run", &error);
                    write_command_manifest(
                        &output_root,
                        &command_line,
                        vec![pack.clone()],
                        vec![out.clone()],
                        None,
                        manifest::default_config_path_if_exists(),
                        Vec::new(),
                        vec![format!("{error:#}")],
                    );
                    return Err(error);
                }
            }
        }
        Commands::CodexReview { input, out } => {
            let output_root = input.parent().unwrap_or(&out).to_path_buf();
            append_phase_start(&output_root, "codex-review", &[input.clone(), out.clone()]);
            match codex_review::run_codex_review(&input, &out) {
                Ok(outcome) => {
                    codex_review::print_codex_review_summary(&input, &out, &outcome);
                    let output_paths = vec![
                        outcome.queue_markdown_path.clone(),
                        outcome.queue_json_path.clone(),
                        outcome.unsupported_claims_path.clone(),
                        outcome.evidence_gaps_path.clone(),
                        outcome.wording_warnings_path.clone(),
                        outcome.summary_markdown_path.clone(),
                    ];
                    append_artifact_events(&output_root, "codex-review", &output_paths);
                    append_warning_events(&output_root, "codex-review", &outcome.summary.warnings);
                    append_phase_completed(
                        &output_root,
                        "codex-review",
                        "Codex review phase completed.",
                        &output_paths,
                    );
                    write_command_manifest(
                        &output_root,
                        &command_line,
                        vec![input.clone()],
                        output_paths,
                        None,
                        manifest::default_config_path_if_exists(),
                        outcome.summary.warnings.clone(),
                        Vec::new(),
                    );
                }
                Err(error) => {
                    append_error_event(&output_root, "codex-review", &error);
                    write_command_manifest(
                        &output_root,
                        &command_line,
                        vec![input.clone()],
                        vec![out.clone()],
                        None,
                        manifest::default_config_path_if_exists(),
                        Vec::new(),
                        vec![format!("{error:#}")],
                    );
                    return Err(error);
                }
            }
        }
        Commands::ApiIntel { input, out } => {
            append_phase_start(&input, "api-intel", &[input.clone(), out.clone()]);
            match api::run_api_intelligence(&input, &out) {
                Ok(outcome) => {
                    api::print_api_summary(&input, &out, &outcome);
                    let output_paths = vec![
                        outcome.api_endpoints_path.clone(),
                        outcome.api_objects_path.clone(),
                        outcome.api_relationships_path.clone(),
                        outcome.auth_observations_path.clone(),
                        outcome.js_observations_path.clone(),
                        outcome.schemas_path.clone(),
                        outcome.graphql_observations_path.clone(),
                        outcome.api_graph_path.clone(),
                        outcome.api_summary_path.clone(),
                    ];
                    append_artifact_events(&input, "api-intel", &output_paths);
                    append_phase_completed(
                        &input,
                        "api-intel",
                        "API intelligence phase completed.",
                        &output_paths,
                    );
                    write_command_manifest(
                        &input,
                        &command_line,
                        vec![input.clone()],
                        output_paths,
                        None,
                        manifest::default_config_path_if_exists(),
                        Vec::new(),
                        Vec::new(),
                    );
                }
                Err(error) => {
                    append_error_event(&input, "api-intel", &error);
                    write_command_manifest(
                        &input,
                        &command_line,
                        vec![input.clone()],
                        vec![out.clone()],
                        None,
                        manifest::default_config_path_if_exists(),
                        Vec::new(),
                        vec![format!("{error:#}")],
                    );
                    return Err(error);
                }
            }
        }
        Commands::Validate { input } => {
            append_phase_start(&input, "validate", std::slice::from_ref(&input));
            let outcome = validation::run_validation(&input)?;
            validation::print_validation_summary(&input, &outcome);
            let output_paths = vec![outcome.markdown_path.clone(), outcome.json_path.clone()];
            append_artifact_events(&input, "validate", &output_paths);
            append_warning_events(&input, "validate", &outcome.report.warnings);
            if !outcome.report.passed {
                append_error_event(
                    &input,
                    "validate",
                    &anyhow!(
                        "validation failed with {} error(s)",
                        outcome.report.errors.len()
                    ),
                );
                write_command_manifest(
                    &input,
                    &command_line,
                    vec![input.clone()],
                    output_paths.clone(),
                    None,
                    manifest::default_config_path_if_exists(),
                    outcome.report.warnings.clone(),
                    outcome.report.errors.clone(),
                );
                bail!(
                    "validation failed with {} error(s); inspect {}",
                    outcome.report.errors.len(),
                    outcome.markdown_path.display()
                );
            }
            append_phase_completed(
                &input,
                "validate",
                "Validation phase completed.",
                &output_paths,
            );
            write_command_manifest(
                &input,
                &command_line,
                vec![input.clone()],
                output_paths,
                None,
                manifest::default_config_path_if_exists(),
                outcome.report.warnings.clone(),
                Vec::new(),
            );
        }
        Commands::Normalize { input } => {
            let normalized = normalize::normalize_urls_from_file(&input)?;
            println!("Normalized {} URL records.", normalized.len());
            for record in normalized.iter().take(5) {
                println!("- {}", record.normalized_url);
            }
            if normalized.is_empty() {
                println!("No valid URLs were found in the provided input.");
            }
        }
        Commands::Score { input } => {
            let config = config::load_default_or_file(None)?;
            let findings = scoring::load_findings(&input)?;
            let graph_context = scoring::load_graph_context_for_input(&input)?;
            let scored = scoring::score_findings_with_graph(
                findings,
                &config.score_keywords,
                graph_context.as_ref(),
            );
            println!("Scored {} findings.", scored.len());
            for finding in scored.iter().take(5) {
                let total = finding
                    .score
                    .as_ref()
                    .map(|score| score.total)
                    .unwrap_or_default();
                println!("- [{}] {} ({})", total, finding.title, finding.id);
            }
            if scored.is_empty() {
                println!("No findings were supplied for scoring.");
            }
        }
        Commands::Report { input } => {
            let findings = scoring::load_findings(&input)?;
            let markdown = report::build_markdown_report(&findings);
            let json = report::build_json_report(&findings)?;
            println!("{markdown}");
            println!();
            println!("JSON preview:");
            println!("{json}");
        }
    }

    Ok(())
}

fn current_command_line() -> String {
    env::args().collect::<Vec<_>>().join(" ")
}

fn append_phase_start(output_root: &Path, phase: &str, inputs: &[PathBuf]) {
    let event = audit::AuditEvent::new(
        "phase_started",
        phase,
        format!("{phase} phase started."),
        inputs
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
    );
    let _ = audit::append_audit_event(output_root, &event);
}

fn append_phase_completed(output_root: &Path, phase: &str, message: &str, outputs: &[PathBuf]) {
    let event = audit::AuditEvent::new(
        "phase_completed",
        phase,
        message.to_string(),
        outputs
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
    );
    let _ = audit::append_audit_event(output_root, &event);
}

fn append_artifact_events(output_root: &Path, phase: &str, paths: &[PathBuf]) {
    for path in paths {
        let event = audit::AuditEvent::new(
            "artifact_written",
            phase,
            format!("Artifact written: {}", path.display()),
            vec![path.display().to_string()],
        );
        let _ = audit::append_audit_event(output_root, &event);
    }
}

fn append_warning_events(output_root: &Path, phase: &str, warnings: &[String]) {
    for warning in warnings {
        let event_type = if warning.to_ascii_lowercase().contains("optional") {
            "skipped_optional_input"
        } else {
            "warning"
        };
        let event = audit::AuditEvent::new(event_type, phase, warning.clone(), Vec::new());
        let _ = audit::append_audit_event(output_root, &event);
    }
}

fn append_error_event(output_root: &Path, phase: &str, error: &anyhow::Error) {
    let event = audit::AuditEvent::new("error", phase, format!("{error:#}"), Vec::new());
    let _ = audit::append_audit_event(output_root, &event);
}

fn append_dry_run_event(output_root: &Path, phase: &str, message: &str, paths: Vec<String>) {
    let event = audit::AuditEvent::new("dry_run_plan_created", phase, message.to_string(), paths);
    let _ = audit::append_audit_event(output_root, &event);
}

fn write_command_manifest(
    output_root: &Path,
    command_line: &str,
    input_paths: Vec<PathBuf>,
    output_paths: Vec<PathBuf>,
    scope_path: Option<PathBuf>,
    config_path: Option<PathBuf>,
    warnings: Vec<String>,
    errors: Vec<String>,
) {
    let _ = manifest::write_manifest(
        output_root,
        manifest::ManifestInput {
            command_executed: command_line.to_string(),
            input_paths,
            output_paths,
            scope_path,
            config_path,
            warnings,
            errors,
            pipeline_profile: None,
            phase_results: Vec::new(),
        },
    );
}

fn collect_run_warnings(runs: &[ReconToolRun]) -> Vec<String> {
    let mut warnings = runs
        .iter()
        .flat_map(|run| {
            run.notes.iter().filter_map(|note| {
                if note.contains("Binary not found")
                    || note.contains("Required input was unavailable")
                    || note.contains("Execution error")
                {
                    Some(format!("{}: {}", run.tool, note))
                } else {
                    None
                }
            })
        })
        .collect::<Vec<_>>();
    warnings.sort();
    warnings.dedup();
    warnings
}

fn collect_graph_output_paths(outcome: &graph::GraphOutcome) -> Vec<PathBuf> {
    let mut paths = vec![outcome.plan_path.clone(), outcome.preview_path.clone()];
    if let Some(path) = &outcome.graph_json_path {
        paths.push(path.clone());
    }
    if let Some(path) = &outcome.graph_markdown_path {
        paths.push(path.clone());
    }
    if let Some(path) = &outcome.clusters_json_path {
        paths.push(path.clone());
    }
    if let Some(path) = &outcome.clusters_markdown_path {
        paths.push(path.clone());
    }
    if let Some(path) = &outcome.anomalies_json_path {
        paths.push(path.clone());
    }
    if let Some(path) = &outcome.summary_json_path {
        paths.push(path.clone());
    }
    paths
}

fn collect_optional_paths(values: &[Option<PathBuf>]) -> Vec<PathBuf> {
    values.iter().filter_map(Clone::clone).collect()
}
