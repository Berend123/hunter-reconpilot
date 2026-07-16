use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::{
    api, audit, codex_runner,
    config::{self, ReconPilotConfig},
    enrichment, graph, llm_pack, manifest, mapping,
    models::{PhaseStatus, PipelinePlan, PipelineResult, ReconToolRun},
    profiles::{self, PhaseDefinition, PhaseKey},
    review, runner,
    scope::{self, ScopeDefinition},
    utils::{self, OutputLayout},
    validation,
};

#[derive(Debug, Clone)]
struct PhaseExecutionOutcome {
    status: PhaseStatus,
    outputs: Vec<PathBuf>,
    warnings: Vec<String>,
    errors: Vec<String>,
    notes: Vec<String>,
    dry_run: bool,
}

pub async fn run_pipeline(
    scope_path: &Path,
    profile_name: &str,
    out: &Path,
    execute: bool,
    include_codex: bool,
    execute_codex: bool,
) -> Result<PipelineResult> {
    let scope = scope::load_scope(scope_path)?;
    let mut config = config::load_default_or_file(None)?;
    config.profile_name = profile_name.to_string();
    config.output_root = out.display().to_string();

    let output = utils::ensure_output_structure(out)?;
    let scope_consistency_warnings = config::validate_scope_exclusion_consistency(
        &scope,
        config::default_exclusion_path_if_exists().as_deref(),
    )?;
    let profile = profiles::resolve_profile(
        profile_name,
        scope_path,
        &output.root,
        execute,
        include_codex,
        execute_codex,
        config.max_context_chars,
    )?;

    let plan = PipelinePlan {
        generated_at: chrono::Utc::now(),
        profile: profile.profile.clone(),
        scope_path: scope_path.display().to_string(),
        output_root: output.root.display().to_string(),
        execute_requested: execute,
        warnings: {
            let mut warnings = scope_consistency_warnings.clone();
            warnings.extend(profile.warnings.clone());
            if !execute {
                warnings.push(
                "External-tool phases remain dry-run until --execute is passed; local-only phases may still generate outputs."
                    .to_string(),
                );
            }
            if include_codex && !execute_codex {
                warnings.push(
                    "The optional codex-run pipeline phase will stay plan-only until --execute-codex is passed."
                        .to_string(),
                );
            }
            warnings
        },
    };
    let plan_path = output.plans.join("pipeline-plan.json");
    let plan_markdown_path = output.plans.join("pipeline-plan.md");
    utils::write_json_pretty(&plan_path, &plan)?;
    utils::write_string(&plan_markdown_path, &render_plan_markdown(&plan))?;

    let audit_log_path = output.root.join("audit-log.jsonl");
    append_phase_event(
        &output.root,
        "phase_started",
        "pipeline",
        format!("Pipeline profile '{}' started.", profile.profile.name),
        vec![
            scope_path.display().to_string(),
            output.root.display().to_string(),
        ],
        BTreeMap::from([
            ("profile".to_string(), profile.profile.name.clone()),
            ("execute_requested".to_string(), execute.to_string()),
            ("include_codex".to_string(), include_codex.to_string()),
            ("execute_codex".to_string(), execute_codex.to_string()),
        ]),
    );
    append_artifact_events(
        &output.root,
        "pipeline",
        &[plan_path.clone(), plan_markdown_path.clone()],
    );

    let mut phase_results = profile.profile.phases.clone();
    let mut warnings = plan.warnings.clone();
    let mut errors = Vec::new();
    let mut all_output_paths = vec![plan_path.clone(), plan_markdown_path.clone()];

    for (index, phase_definition) in profile.phases.iter().enumerate() {
        append_phase_event(
            &output.root,
            "phase_started",
            phase_definition.key.as_str(),
            format!("Pipeline phase '{}' started.", phase_definition.phase.name),
            phase_definition.phase.required_inputs.clone(),
            BTreeMap::from([
                (
                    "phase_type".to_string(),
                    phase_definition.phase.phase_type.clone(),
                ),
                (
                    "execute_phase".to_string(),
                    phase_definition.phase.execute_phase.to_string(),
                ),
            ]),
        );

        let outcome = execute_phase(phase_definition, &scope, &config, &output).await;
        let phase = &mut phase_results[index];
        phase.status = outcome.status.clone();
        phase.notes.extend(outcome.notes.clone());
        phase.notes.extend(outcome.warnings.clone());
        phase.notes.extend(outcome.errors.clone());

        append_artifact_events(
            &output.root,
            phase_definition.key.as_str(),
            &outcome.outputs,
        );
        if outcome.dry_run {
            append_dry_run_event(
                &output.root,
                phase_definition.key.as_str(),
                format!(
                    "Dry-run plan created for pipeline phase '{}'.",
                    phase_definition.phase.name
                ),
                outcome
                    .outputs
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect(),
            );
        }
        append_warning_events(
            &output.root,
            phase_definition.key.as_str(),
            &outcome.warnings,
        );
        for error in &outcome.errors {
            append_error_event(&output.root, phase_definition.key.as_str(), error);
        }
        append_phase_event(
            &output.root,
            "phase_completed",
            phase_definition.key.as_str(),
            format!(
                "Pipeline phase '{}' completed with status '{}'.",
                phase_definition.phase.name,
                status_label(&outcome.status)
            ),
            outcome
                .outputs
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            BTreeMap::from([(
                "status".to_string(),
                status_label(&outcome.status).to_string(),
            )]),
        );

        warnings.extend(outcome.warnings);
        errors.extend(outcome.errors);
        all_output_paths.extend(outcome.outputs);
    }

    dedupe_strings(&mut warnings);
    dedupe_strings(&mut errors);
    dedupe_paths(&mut all_output_paths);

    let manifest_path = manifest::write_manifest(
        &output.root,
        manifest::ManifestInput {
            command_executed: std::env::args().collect::<Vec<_>>().join(" "),
            input_paths: vec![scope.source_path.clone()],
            output_paths: all_output_paths.clone(),
            scope_path: Some(scope.source_path.clone()),
            config_path: manifest::default_config_path_if_exists(),
            warnings: warnings.clone(),
            errors: errors.clone(),
            pipeline_profile: Some(profile.profile.name.clone()),
            phase_results: phase_results.clone(),
        },
    )?;

    append_artifact_events(
        &output.root,
        "pipeline",
        std::slice::from_ref(&manifest_path),
    );
    append_phase_event(
        &output.root,
        "phase_completed",
        "pipeline",
        format!("Pipeline profile '{}' completed.", profile.profile.name),
        vec![
            manifest_path.display().to_string(),
            audit_log_path.display().to_string(),
        ],
        BTreeMap::from([
            ("profile".to_string(), profile.profile.name.clone()),
            ("errors".to_string(), errors.len().to_string()),
            ("warnings".to_string(), warnings.len().to_string()),
        ]),
    );

    Ok(PipelineResult {
        profile_name: profile.profile.name,
        plan_path,
        plan_markdown_path,
        manifest_path,
        audit_log_path,
        phase_results,
        warnings,
        errors,
    })
}

pub fn print_pipeline_summary(scope: &ScopeDefinition, out: &Path, result: &PipelineResult) {
    println!("ReconPilot pipeline summary");
    println!("Profile: {}", result.profile_name);
    println!("Scope source: {}", scope.source_path.display());
    println!("Output root: {}", out.display());
    println!("Plan JSON: {}", result.plan_path.display());
    println!("Plan markdown: {}", result.plan_markdown_path.display());
    println!("Manifest: {}", result.manifest_path.display());
    println!("Audit log: {}", result.audit_log_path.display());
    println!(
        "Warnings: {} | Errors: {}",
        result.warnings.len(),
        result.errors.len()
    );
    println!(
        "Safety notice: external-tool phases remain dry-run unless the pipeline itself is invoked with --execute."
    );
    println!(
        "Codex notice: codex-run is only included with --include-codex and only executes with --execute-codex."
    );
    println!();

    for phase in &result.phase_results {
        println!(
            "- {} [{}] status:{} execute:{}",
            phase.name,
            phase.phase_type,
            status_label(&phase.status),
            phase.execute_phase
        );
        if !phase.notes.is_empty() {
            println!("  Notes: {}", phase.notes.join(" | "));
        }
    }
}

async fn execute_phase(
    definition: &PhaseDefinition,
    scope: &ScopeDefinition,
    config: &ReconPilotConfig,
    output: &OutputLayout,
) -> PhaseExecutionOutcome {
    match definition.key {
        PhaseKey::Run => {
            match runner::run_tool_adapters(scope, output, definition.phase.execute_phase).await {
                Ok(runs) => {
                    let warnings = collect_run_warnings(&runs);
                    let mut notes = vec![if definition.phase.execute_phase {
                        "External recon adapters executed from the pipeline.".to_string()
                    } else {
                        "External recon adapters stayed in dry-run planning mode.".to_string()
                    }];
                    if !definition.phase.execute_phase {
                        notes.push(
                        "Use `reconpilot pipeline ... --execute` to allow target-touching adapter execution."
                            .to_string(),
                    );
                    }
                    PhaseExecutionOutcome {
                        status: if warnings.is_empty() {
                            PhaseStatus::Completed
                        } else {
                            PhaseStatus::Warning
                        },
                        outputs: vec![
                            output.plans.join("tool-runs.json"),
                            output.plans.clone(),
                            output.raw.clone(),
                        ],
                        warnings,
                        errors: Vec::new(),
                        notes,
                        dry_run: !definition.phase.execute_phase,
                    }
                }
                Err(error) => {
                    failed_outcome(error, vec![output.plans.clone(), output.raw.clone()], "run")
                }
            }
        }
        PhaseKey::Map => {
            match mapping::run_mapping_layer(scope, output, definition.phase.execute_phase).await {
                Ok(outcome) => {
                    let warnings = collect_run_warnings(&outcome.runs);
                    PhaseExecutionOutcome {
                        status: if warnings.is_empty() {
                            PhaseStatus::Completed
                        } else {
                            PhaseStatus::Warning
                        },
                        outputs: vec![
                            output.plans.join("dnsx-plan.json"),
                            output.plans.join("gowitness-plan.json"),
                            output.plans.join("whatweb-plan.json"),
                            outcome.map_json_path,
                            outcome.map_markdown_path,
                        ],
                        warnings,
                        errors: Vec::new(),
                        notes: vec![if definition.phase.execute_phase {
                            "Mapping adapters executed from the pipeline.".to_string()
                        } else {
                            "Mapping adapters stayed in dry-run planning mode.".to_string()
                        }],
                        dry_run: !definition.phase.execute_phase,
                    }
                }
                Err(error) => failed_outcome(
                    error,
                    vec![output.plans.clone(), output.maps.clone()],
                    "map",
                ),
            }
        }
        PhaseKey::Graph => match graph::run_graph_engine(&output.root, &output.maps, true) {
            Ok(outcome) => PhaseExecutionOutcome {
                status: PhaseStatus::Completed,
                outputs: collect_graph_output_paths(&outcome),
                warnings: Vec::new(),
                errors: Vec::new(),
                notes: vec![
                    "Graph phase executed locally using existing artifacts only.".to_string(),
                ],
                dry_run: !outcome.executed,
            },
            Err(error) => failed_outcome(error, vec![output.maps.clone()], "graph"),
        },
        PhaseKey::ApiIntel => match api::run_api_intelligence(&output.root, &output.api_intel) {
            Ok(outcome) => PhaseExecutionOutcome {
                status: PhaseStatus::Completed,
                outputs: vec![
                    outcome.api_endpoints_path,
                    outcome.api_objects_path,
                    outcome.api_relationships_path,
                    outcome.auth_observations_path,
                    outcome.js_observations_path,
                    outcome.schemas_path,
                    outcome.graphql_observations_path,
                    outcome.api_graph_path,
                    outcome.api_summary_path,
                ],
                warnings: Vec::new(),
                errors: Vec::new(),
                notes: vec![
                    "API and JavaScript intelligence executed locally without contacting targets."
                        .to_string(),
                ],
                dry_run: false,
            },
            Err(error) => failed_outcome(error, vec![output.api_intel.clone()], "api-intel"),
        },
        PhaseKey::Enrich => {
            let mut required = vec![output.maps.join("graph.json")];
            if definition.enrich_with_api_intel {
                required.push(output.api_intel.clone());
            }
            if let Some(skip) = skipped_outcome("enrich", &required) {
                return skip;
            }

            match enrichment::run_enrichment_engine(
                &output.maps,
                definition.enrich_with_api_intel.then_some(output.api_intel.as_path()),
                &output.enrichment,
            ) {
                Ok(outcome) => PhaseExecutionOutcome {
                    status: if outcome.warnings.is_empty() {
                        PhaseStatus::Completed
                    } else {
                        PhaseStatus::Warning
                    },
                    outputs: vec![
                        outcome.semantic_assets_path,
                        outcome.observations_path,
                        outcome.risk_explanations_path,
                        outcome.enriched_graph_path,
                        outcome.summary_path,
                    ],
                    warnings: outcome.warnings,
                    errors: Vec::new(),
                    notes: vec![
                        "Semantic enrichment executed locally and preserved cautious prioritization language."
                            .to_string(),
                    ],
                    dry_run: false,
                },
                Err(error) => failed_outcome(error, vec![output.enrichment.clone()], "enrich"),
            }
        }
        PhaseKey::Review => {
            let required = review_required_inputs(output);
            if let Some(skip) = skipped_outcome("review", &required) {
                return skip;
            }

            match review::run_review_workspace(&output.enrichment, &output.review) {
                Ok(outcome) => PhaseExecutionOutcome {
                    status: PhaseStatus::Completed,
                    outputs: vec![
                        outcome.priority_queue_markdown_path,
                        outcome.priority_queue_json_path,
                        outcome.asset_cards_dir,
                        outcome.review_checklist_path,
                        outcome.executive_summary_path,
                        outcome.evidence_index_path,
                    ],
                    warnings: Vec::new(),
                    errors: Vec::new(),
                    notes: vec![
                        "Review workspace generated analyst-facing queues and evidence indexes."
                            .to_string(),
                    ],
                    dry_run: false,
                },
                Err(error) => failed_outcome(error, vec![output.review.clone()], "review"),
            }
        }
        PhaseKey::LlmPack => {
            let required = llm_pack_required_inputs(output);
            if let Some(skip) = skipped_outcome("llm-pack", &required) {
                return skip;
            }

            match llm_pack::run_llm_pack(&output.root, &output.llm_pack, config.max_context_chars) {
                Ok(outcome) => PhaseExecutionOutcome {
                    status: if outcome.summary.warnings.is_empty() {
                        PhaseStatus::Completed
                    } else {
                        PhaseStatus::Warning
                    },
                    outputs: vec![
                        outcome.asset_contexts_dir,
                        outcome.prompts_dir,
                        outcome.reasoning_queue_json_path,
                        outcome.reasoning_queue_markdown_path,
                        outcome.analyst_brief_path,
                        outcome.pack_summary_path,
                    ],
                    warnings: outcome.summary.warnings,
                    errors: Vec::new(),
                    notes: vec![
                        "Local LLM context packs were generated without invoking a model."
                            .to_string(),
                    ],
                    dry_run: false,
                },
                Err(error) => failed_outcome(error, vec![output.llm_pack.clone()], "llm-pack"),
            }
        }
        PhaseKey::Validate => match validation::run_validation(&output.root) {
            Ok(outcome) => {
                let mut errors = Vec::new();
                if !outcome.report.passed {
                    errors.extend(outcome.report.errors.clone());
                }
                PhaseExecutionOutcome {
                    status: if !errors.is_empty() {
                        PhaseStatus::Failed
                    } else if outcome.report.warnings.is_empty() {
                        PhaseStatus::Completed
                    } else {
                        PhaseStatus::Warning
                    },
                    outputs: vec![outcome.markdown_path, outcome.json_path],
                    warnings: outcome.report.warnings,
                    errors,
                    notes: vec![
                        "Validation checked artifact integrity, reference consistency, and output completeness."
                            .to_string(),
                    ],
                    dry_run: false,
                }
            }
            Err(error) => failed_outcome(
                error,
                vec![
                    output.root.join("validation-report.md"),
                    output.root.join("validation-report.json"),
                ],
                "validate",
            ),
        },
        PhaseKey::CodexRun => {
            let required = codex_run_required_inputs(output);
            if let Some(skip) = skipped_outcome("codex-run", &required) {
                return skip;
            }

            match codex_runner::run_codex_runner(
                &output.llm_pack,
                &output.codex_insights,
                definition.phase.execute_phase,
                3,
                None,
            ) {
                Ok(outcome) => PhaseExecutionOutcome {
                    status: if outcome.summary.warnings.is_empty() {
                        PhaseStatus::Completed
                    } else {
                        PhaseStatus::Warning
                    },
                    outputs: vec![
                        outcome.plan_json_path,
                        outcome.plan_markdown_path,
                        outcome.results_dir,
                        outcome.logs_dir,
                        outcome.summary_markdown_path,
                        outcome.summary_json_path,
                    ],
                    warnings: outcome.summary.warnings,
                    errors: Vec::new(),
                    notes: vec![if definition.phase.execute_phase {
                        "Codex reasoning executed only because --include-codex and --execute-codex were both requested."
                            .to_string()
                    } else {
                        "Codex reasoning stayed in plan-only mode because --execute-codex was not requested."
                            .to_string()
                    }],
                    dry_run: !definition.phase.execute_phase,
                },
                Err(error) => {
                    failed_outcome(error, vec![output.codex_insights.clone()], "codex-run")
                }
            }
        }
    }
}

fn failed_outcome(
    error: anyhow::Error,
    outputs: Vec<PathBuf>,
    phase: &str,
) -> PhaseExecutionOutcome {
    PhaseExecutionOutcome {
        status: PhaseStatus::Failed,
        outputs,
        warnings: Vec::new(),
        errors: vec![format!("{phase}: {error:#}")],
        notes: vec![format!(
            "Phase '{}' failed and requires operator review.",
            phase
        )],
        dry_run: false,
    }
}

fn skipped_outcome(phase: &str, required_inputs: &[PathBuf]) -> Option<PhaseExecutionOutcome> {
    let missing = required_inputs
        .iter()
        .filter(|path| !path.exists())
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return None;
    }

    Some(PhaseExecutionOutcome {
        status: PhaseStatus::Skipped,
        outputs: Vec::new(),
        warnings: vec![format!(
            "Phase '{}' skipped because required local inputs were missing: {}",
            phase,
            missing
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )],
        errors: Vec::new(),
        notes: vec![
            "Downstream local analysis depends on artifacts that were not present yet.".to_string(),
        ],
        dry_run: false,
    })
}

fn review_required_inputs(output: &OutputLayout) -> Vec<PathBuf> {
    vec![
        output.enrichment.join("semantic-assets.json"),
        output.enrichment.join("semantic-observations.json"),
        output.enrichment.join("risk-explanations.json"),
        output.enrichment.join("enriched-graph.json"),
        output.enrichment.join("enrichment-summary.md"),
    ]
}

fn llm_pack_required_inputs(output: &OutputLayout) -> Vec<PathBuf> {
    vec![
        output.enrichment.join("semantic-assets.json"),
        output.enrichment.join("semantic-observations.json"),
        output.enrichment.join("risk-explanations.json"),
        output.enrichment.join("enriched-graph.json"),
        output.enrichment.join("enrichment-summary.md"),
        output.review.join("priority-queue.json"),
        output.review.join("evidence-index.json"),
    ]
}

fn codex_run_required_inputs(output: &OutputLayout) -> Vec<PathBuf> {
    vec![
        output.llm_pack.join("reasoning-queue.json"),
        output.llm_pack.join("pack-summary.json"),
        output.llm_pack.join("asset-contexts"),
        output.llm_pack.join("prompts"),
    ]
}

fn render_plan_markdown(plan: &PipelinePlan) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot Pipeline Plan\n\n");
    output.push_str(&format!(
        "- Generated at: {}\n- Profile: {}\n- Scope: {}\n- Output root: {}\n- External execution requested: {}\n\n",
        plan.generated_at.to_rfc3339(),
        plan.profile.name,
        plan.scope_path,
        plan.output_root,
        plan.execute_requested
    ));
    output.push_str(&format!("{}\n\n", plan.profile.description));

    output.push_str("## Phases\n\n");
    for (index, phase) in plan.profile.phases.iter().enumerate() {
        output.push_str(&format!(
            "{}. `{}` [{}]\n",
            index + 1,
            phase.name,
            phase.phase_type
        ));
        output.push_str(&format!(
            "   Status: `{}` | Execute: `{}` | Touches targets: `{}`\n",
            status_label(&phase.status),
            phase.execute_phase,
            phase.touches_targets
        ));
        output.push_str(&format!("   Command: `{}`\n", phase.command));
        if !phase.required_inputs.is_empty() {
            output.push_str(&format!(
                "   Required inputs: {}\n",
                phase.required_inputs.join(", ")
            ));
        }
        if !phase.expected_outputs.is_empty() {
            output.push_str(&format!(
                "   Expected outputs: {}\n",
                phase.expected_outputs.join(", ")
            ));
        }
        for note in &phase.notes {
            output.push_str(&format!("   Note: {}\n", note));
        }
    }

    if !plan.warnings.is_empty() {
        output.push_str("\n## Warnings\n\n");
        for warning in &plan.warnings {
            output.push_str(&format!("- {}\n", warning));
        }
    }

    output
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
    dedupe_strings(&mut warnings);
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

fn dedupe_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn dedupe_paths(paths: &mut Vec<PathBuf>) {
    let mut seen = BTreeSet::new();
    paths.retain(|path| seen.insert(path.clone()));
}

fn status_label(status: &PhaseStatus) -> &'static str {
    match status {
        PhaseStatus::Planned => "planned",
        PhaseStatus::Skipped => "skipped",
        PhaseStatus::Completed => "completed",
        PhaseStatus::Failed => "failed",
        PhaseStatus::Warning => "warning",
    }
}

fn append_phase_event(
    output_root: &Path,
    event_type: &str,
    phase: &str,
    message: String,
    paths: Vec<String>,
    details: BTreeMap<String, String>,
) {
    let mut event = audit::AuditEvent::new(event_type, phase, message, paths);
    event.details = details;
    let _ = audit::append_audit_event(output_root, &event);
}

fn append_artifact_events(output_root: &Path, phase: &str, paths: &[PathBuf]) {
    for path in paths {
        let _ = audit::append_audit_event(
            output_root,
            &audit::AuditEvent::new(
                "artifact_written",
                phase,
                format!("Artifact written: {}", path.display()),
                vec![path.display().to_string()],
            ),
        );
    }
}

fn append_warning_events(output_root: &Path, phase: &str, warnings: &[String]) {
    for warning in warnings {
        let event_type = if warning.to_ascii_lowercase().contains("optional")
            || warning.to_ascii_lowercase().contains("skipped")
        {
            "skipped_optional_input"
        } else {
            "warning"
        };
        let _ = audit::append_audit_event(
            output_root,
            &audit::AuditEvent::new(event_type, phase, warning.clone(), Vec::new()),
        );
    }
}

fn append_error_event(output_root: &Path, phase: &str, error: &str) {
    let _ = audit::append_audit_event(
        output_root,
        &audit::AuditEvent::new("error", phase, error.to_string(), Vec::new()),
    );
}

fn append_dry_run_event(output_root: &Path, phase: &str, message: String, paths: Vec<String>) {
    let _ = audit::append_audit_event(
        output_root,
        &audit::AuditEvent::new("dry_run_plan_created", phase, message, paths),
    );
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;
    use serde_json::Value;

    use crate::models::PhaseStatus;

    use super::run_pipeline;

    struct TestWorkspace {
        root: PathBuf,
    }

    impl TestWorkspace {
        fn new(label: &str) -> Result<Self> {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "reconpilot-pipeline-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            Ok(Self { root })
        }

        fn scope_path(&self) -> PathBuf {
            self.root.join("scope.txt")
        }

        fn output_root(&self) -> PathBuf {
            self.root.join("output")
        }

        fn write_scope(&self, content: &str) -> Result<()> {
            fs::write(self.scope_path(), content)?;
            Ok(())
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[tokio::test]
    async fn pipeline_plan_generation_creates_plan_files() -> Result<()> {
        let workspace = TestWorkspace::new("plan")?;
        workspace.write_scope("example.com\n")?;

        let result = run_pipeline(
            &workspace.scope_path(),
            "passive",
            &workspace.output_root(),
            false,
            false,
            false,
        )
        .await?;
        assert!(result.plan_path.exists());
        assert!(result.plan_markdown_path.exists());
        let raw = fs::read_to_string(result.plan_path)?;
        assert!(raw.contains("\"profile\""));
        assert!(raw.contains("\"name\": \"passive\""));
        Ok(())
    }

    #[tokio::test]
    async fn dry_run_external_phase_behavior_preserves_planning() -> Result<()> {
        let workspace = TestWorkspace::new("dry-run")?;
        workspace.write_scope("example.com\nhttps://portal.example.org\n")?;

        let _result = run_pipeline(
            &workspace.scope_path(),
            "active-lite",
            &workspace.output_root(),
            false,
            false,
            false,
        )
        .await?;
        let tool_runs =
            fs::read_to_string(workspace.output_root().join("plans").join("tool-runs.json"))?;
        let dnsx_plan =
            fs::read_to_string(workspace.output_root().join("plans").join("dnsx-plan.json"))?;
        assert!(tool_runs.contains("\"executed\": false"));
        assert!(dnsx_plan.contains("\"execute_requested\": false"));
        Ok(())
    }

    #[tokio::test]
    async fn local_phase_execution_behavior_generates_outputs() -> Result<()> {
        let workspace = TestWorkspace::new("local-exec")?;
        workspace.write_scope("example.com\n")?;

        let result = run_pipeline(
            &workspace.scope_path(),
            "passive",
            &workspace.output_root(),
            false,
            false,
            false,
        )
        .await?;
        assert!(result.errors.is_empty());
        assert!(workspace
            .output_root()
            .join("maps")
            .join("graph.json")
            .exists());
        assert!(workspace
            .output_root()
            .join("review")
            .join("priority-queue.json")
            .exists());
        assert!(workspace
            .output_root()
            .join("llm-pack")
            .join("reasoning-queue.json")
            .exists());
        Ok(())
    }

    #[tokio::test]
    async fn missing_input_skip_behavior_marks_dependent_phases() -> Result<()> {
        let workspace = TestWorkspace::new("skip")?;
        workspace.write_scope("example.com\n")?;

        let result = run_pipeline(
            &workspace.scope_path(),
            "api-focused",
            &workspace.output_root(),
            false,
            false,
            false,
        )
        .await?;
        let enrich = result
            .phase_results
            .iter()
            .find(|phase| phase.name == "enrich")
            .expect("enrich phase should exist");
        let review = result
            .phase_results
            .iter()
            .find(|phase| phase.name == "review")
            .expect("review phase should exist");
        assert!(matches!(enrich.status, PhaseStatus::Skipped));
        assert!(matches!(review.status, PhaseStatus::Skipped));
        assert!(!result.errors.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn active_lite_requires_execute_for_external_execution() -> Result<()> {
        let workspace = TestWorkspace::new("active-lite")?;
        workspace.write_scope("example.com\n")?;

        let result = run_pipeline(
            &workspace.scope_path(),
            "active-lite",
            &workspace.output_root(),
            false,
            false,
            false,
        )
        .await?;
        let run = result
            .phase_results
            .iter()
            .find(|phase| phase.name == "run")
            .expect("run phase should exist");
        let map = result
            .phase_results
            .iter()
            .find(|phase| phase.name == "map")
            .expect("map phase should exist");
        assert!(!run.execute_phase);
        assert!(!map.execute_phase);
        Ok(())
    }

    #[tokio::test]
    async fn audit_events_generated_for_pipeline() -> Result<()> {
        let workspace = TestWorkspace::new("audit")?;
        workspace.write_scope("example.com\n")?;

        let result = run_pipeline(
            &workspace.scope_path(),
            "passive",
            &workspace.output_root(),
            false,
            false,
            false,
        )
        .await?;
        let raw = fs::read_to_string(result.audit_log_path)?;
        assert!(raw.contains("\"event_type\":\"phase_started\""));
        assert!(raw.contains("\"event_type\":\"phase_completed\""));
        assert!(raw.contains("\"phase\":\"pipeline\""));
        Ok(())
    }

    #[tokio::test]
    async fn manifest_includes_profile_results() -> Result<()> {
        let workspace = TestWorkspace::new("manifest")?;
        workspace.write_scope("example.com\n")?;

        let result = run_pipeline(
            &workspace.scope_path(),
            "passive",
            &workspace.output_root(),
            false,
            false,
            false,
        )
        .await?;
        let raw = fs::read_to_string(result.manifest_path)?;
        let value: Value = serde_json::from_str(&raw)?;
        assert_eq!(
            value.get("pipeline_profile").and_then(Value::as_str),
            Some("passive")
        );
        assert!(value
            .get("phase_results")
            .and_then(Value::as_array)
            .map(|items| !items.is_empty())
            .unwrap_or(false));
        Ok(())
    }

    #[tokio::test]
    async fn pipeline_include_codex_plan_only_behavior_generates_codex_plan() -> Result<()> {
        let workspace = TestWorkspace::new("include-codex")?;
        workspace.write_scope("example.com\n")?;

        let result = run_pipeline(
            &workspace.scope_path(),
            "passive",
            &workspace.output_root(),
            false,
            true,
            false,
        )
        .await?;
        let codex_phase = result
            .phase_results
            .iter()
            .find(|phase| phase.name == "codex-run")
            .expect("codex-run phase should exist");
        assert!(!codex_phase.execute_phase);
        assert!(workspace
            .output_root()
            .join("codex-insights")
            .join("plans")
            .join("codex-command-plan.json")
            .exists());
        Ok(())
    }

    #[tokio::test]
    async fn pipeline_execute_does_not_imply_execute_codex() -> Result<()> {
        let workspace = TestWorkspace::new("execute-not-codex")?;
        workspace.write_scope("example.com\n")?;

        let result = run_pipeline(
            &workspace.scope_path(),
            "passive",
            &workspace.output_root(),
            true,
            true,
            false,
        )
        .await?;
        let codex_phase = result
            .phase_results
            .iter()
            .find(|phase| phase.name == "codex-run")
            .expect("codex-run phase should exist");
        assert!(!codex_phase.execute_phase);
        Ok(())
    }

    #[tokio::test]
    async fn pipeline_execute_codex_only_applies_when_included() -> Result<()> {
        let workspace = TestWorkspace::new("execute-codex-ignored")?;
        workspace.write_scope("example.com\n")?;

        let result = run_pipeline(
            &workspace.scope_path(),
            "passive",
            &workspace.output_root(),
            false,
            false,
            true,
        )
        .await?;
        assert!(!result
            .phase_results
            .iter()
            .any(|phase| phase.name == "codex-run"));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("--execute-codex was ignored")));
        Ok(())
    }
}
