use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;

use crate::{
    models::{
        CodexInsightResult, CodexReviewItem, CodexReviewSummary, CodexRunPlan, CodexRunnerSummary,
        EnrichedAsset, EnrichedGraph, EvidenceGap, GraphEdge, GraphNode, LlmAssetContext,
        LlmContextPack, LlmReasoningItem, ReviewItem, RiskExplanation, SemanticObservation,
        UnsupportedClaim, WordingWarning,
    },
    utils,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheck {
    pub name: String,
    pub passed: bool,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub generated_at: DateTime<Utc>,
    pub input_root: String,
    pub passed: bool,
    #[serde(default)]
    pub checks: Vec<ValidationCheck>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub artifact_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct ValidationOutcome {
    pub markdown_path: PathBuf,
    pub json_path: PathBuf,
    pub report: ValidationReport,
}

#[derive(Debug, Clone, Default)]
pub struct IntegrityResult {
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PriorityQueueDocument {
    #[serde(default)]
    items: Vec<ReviewItem>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct EvidenceIndexEntry {
    review_item: String,
    #[serde(flatten)]
    evidence: crate::models::EvidenceItem,
}

#[derive(Debug, Clone, Deserialize)]
struct EvidenceIndexDocument {
    #[serde(default)]
    items: Vec<EvidenceIndexEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct ReasoningQueueDocument {
    #[serde(default)]
    items: Vec<LlmReasoningItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct GraphDocument {
    #[serde(default)]
    nodes: Vec<GraphNode>,
    #[serde(default)]
    edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Deserialize)]
struct CodexReviewQueueDocument {
    summary: CodexReviewSummary,
    #[serde(default)]
    items: Vec<CodexReviewItem>,
}

pub fn run_validation(input_root: &Path) -> Result<ValidationOutcome> {
    let report = validate_output_tree(input_root)?;
    let markdown_path = input_root.join("validation-report.md");
    let json_path = input_root.join("validation-report.json");
    utils::write_string(&markdown_path, &render_markdown_report(&report))?;
    utils::write_json_pretty(&json_path, &report)?;

    let outcome = ValidationOutcome {
        markdown_path,
        json_path,
        report,
    };

    Ok(outcome)
}

pub fn print_validation_summary(input_root: &Path, outcome: &ValidationOutcome) {
    println!("ReconPilot validation summary");
    println!("Input root: {}", input_root.display());
    println!(
        "Status: {}",
        if outcome.report.passed {
            "passed"
        } else {
            "failed"
        }
    );
    println!(
        "Warnings: {} | Errors: {}",
        outcome.report.warnings.len(),
        outcome.report.errors.len()
    );
    println!("Outputs:");
    println!("  - {}", outcome.markdown_path.display());
    println!("  - {}", outcome.json_path.display());
}

pub fn validate_output_tree(input_root: &Path) -> Result<ValidationReport> {
    if !input_root.exists() {
        bail!(
            "validation input root does not exist: {}",
            input_root.display()
        );
    }
    if !input_root.is_dir() {
        bail!(
            "validation input root is not a directory: {}",
            input_root.display()
        );
    }

    let mut report = ValidationReport {
        generated_at: Utc::now(),
        input_root: input_root.display().to_string(),
        passed: true,
        checks: Vec::new(),
        warnings: Vec::new(),
        errors: Vec::new(),
        artifact_counts: collect_artifact_counts(input_root),
    };

    check_expected_directories(input_root, &mut report);
    check_required_core_artifacts(input_root, &mut report);
    parse_structured_files(input_root, &mut report)?;
    if let Err(error) = validate_enrichment_integrity(input_root, &mut report) {
        report.errors.push(format!("{error:#}"));
    }

    match validate_graph_integrity(input_root) {
        Ok(graph) => extend_report("graph_integrity", &graph, &mut report),
        Err(error) => report.errors.push(format!("{error:#}")),
    }

    match validate_review_integrity(input_root) {
        Ok(review) => extend_report("review_integrity", &review, &mut report),
        Err(error) => report.errors.push(format!("{error:#}")),
    }

    match validate_llm_pack_integrity(input_root) {
        Ok(llm_pack) => extend_report("llm_pack_integrity", &llm_pack, &mut report),
        Err(error) => report.errors.push(format!("{error:#}")),
    }

    match validate_codex_insights_integrity(input_root) {
        Ok(codex_insights) => {
            extend_report("codex_insights_integrity", &codex_insights, &mut report)
        }
        Err(error) => report.errors.push(format!("{error:#}")),
    }

    match validate_codex_review_integrity(input_root) {
        Ok(codex_review) => extend_report("codex_review_integrity", &codex_review, &mut report),
        Err(error) => report.errors.push(format!("{error:#}")),
    }

    if let Err(error) = check_empty_high_value_outputs(input_root, &mut report) {
        report.errors.push(format!("{error:#}"));
    }
    report.passed = report.errors.is_empty();
    Ok(report)
}

pub fn validate_json_file(path: &Path) -> Result<()> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read JSON file: {}", path.display()))?;
    serde_json::from_str::<Value>(&raw)
        .with_context(|| format!("failed to parse JSON file: {}", path.display()))?;
    Ok(())
}

pub fn validate_jsonl_file(path: &Path) -> Result<()> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read JSONL file: {}", path.display()))?;
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        serde_json::from_str::<Value>(trimmed).with_context(|| {
            format!(
                "failed to parse JSONL line {} in {}",
                index + 1,
                path.display()
            )
        })?;
    }
    Ok(())
}

pub fn validate_graph_integrity(input_root: &Path) -> Result<IntegrityResult> {
    let path = input_root.join("maps").join("graph.json");
    if !path.exists() {
        return Ok(IntegrityResult {
            warnings: Vec::new(),
            errors: vec![format!(
                "Required graph artifact is missing: {}",
                path.display()
            )],
        });
    }

    let document = load_json::<GraphDocument>(&path)?;
    let mut result = IntegrityResult::default();
    let mut node_ids = BTreeSet::new();
    let mut duplicate_nodes = Vec::new();

    for node in &document.nodes {
        if !node_ids.insert(node.id.clone()) {
            duplicate_nodes.push(node.id.clone());
        }
    }
    if !duplicate_nodes.is_empty() {
        result.errors.push(format!(
            "Graph contains duplicate node IDs: {}",
            duplicate_nodes.join(", ")
        ));
    }

    for edge in &document.edges {
        if !node_ids.contains(&edge.source) {
            result.errors.push(format!(
                "Graph edge references missing source node '{}'",
                edge.source
            ));
        }
        if !node_ids.contains(&edge.target) {
            result.errors.push(format!(
                "Graph edge references missing target node '{}'",
                edge.target
            ));
        }
    }

    Ok(result)
}

pub fn validate_review_integrity(input_root: &Path) -> Result<IntegrityResult> {
    let review_root = input_root.join("review");
    let queue_path = review_root.join("priority-queue.json");
    let evidence_path = review_root.join("evidence-index.json");
    if !queue_path.exists() || !evidence_path.exists() {
        return Ok(IntegrityResult {
            warnings: Vec::new(),
            errors: vec![
                "Review artifacts are incomplete; expected priority-queue.json and evidence-index.json.".to_string(),
            ],
        });
    }

    let queue = load_json::<PriorityQueueDocument>(&queue_path)?;
    let evidence = load_json::<EvidenceIndexDocument>(&evidence_path)?;
    let mut result = IntegrityResult::default();

    let evidence_ids = evidence
        .items
        .iter()
        .map(|entry| entry.evidence.evidence_id.clone())
        .collect::<BTreeSet<_>>();
    let mut seen_assets = BTreeSet::new();
    for item in &queue.items {
        if !seen_assets.insert(item.asset.clone()) {
            result.errors.push(format!(
                "Review queue contains duplicate asset ID '{}'",
                item.asset
            ));
        }
        for evidence_ref in &item.evidence_refs {
            if !evidence_ids.contains(evidence_ref) {
                result.errors.push(format!(
                    "Review item '{}' references missing evidence ID '{}'",
                    item.asset, evidence_ref
                ));
            }
        }
    }

    for entry in &evidence.items {
        if resolve_evidence_source(input_root, &entry.evidence.source).is_none() {
            result.errors.push(format!(
                "Evidence '{}' references missing source artifact '{}'",
                entry.evidence.evidence_id, entry.evidence.source
            ));
        }
    }

    Ok(result)
}

pub fn validate_llm_pack_integrity(input_root: &Path) -> Result<IntegrityResult> {
    let llm_root = input_root.join("llm-pack");
    let queue_path = llm_root.join("reasoning-queue.json");
    let pack_path = llm_root.join("pack-summary.json");
    if !queue_path.exists() || !pack_path.exists() {
        return Ok(IntegrityResult {
            warnings: Vec::new(),
            errors: vec![
                "LLM pack artifacts are incomplete; expected reasoning-queue.json and pack-summary.json.".to_string(),
            ],
        });
    }

    let queue = load_json::<ReasoningQueueDocument>(&queue_path)?;
    let pack = load_json::<LlmContextPack>(&pack_path)?;
    let review_evidence =
        load_json::<EvidenceIndexDocument>(&input_root.join("review").join("evidence-index.json"))
            .unwrap_or(EvidenceIndexDocument { items: Vec::new() });
    let valid_evidence_ids = review_evidence
        .items
        .iter()
        .map(|entry| entry.evidence.evidence_id.clone())
        .collect::<BTreeSet<_>>();
    let mut result = IntegrityResult::default();

    for item in &queue.items {
        let context_path = llm_root.join(&item.context_file);
        if !context_path.exists() {
            result.errors.push(format!(
                "LLM reasoning item '{}' references missing context file '{}'",
                item.asset, item.context_file
            ));
        } else {
            let context = load_json::<LlmAssetContext>(&context_path)?;
            if context.asset != item.asset {
                result.errors.push(format!(
                    "LLM context '{}' does not match reasoning asset '{}'",
                    context.asset, item.asset
                ));
            }
        }

        let prompt_path = llm_root
            .join("prompts")
            .join(&item.suggested_prompt_template);
        if !prompt_path.exists() {
            result.errors.push(format!(
                "LLM reasoning item '{}' references missing prompt template '{}'",
                item.asset, item.suggested_prompt_template
            ));
        }
        for evidence_ref in &item.evidence_refs {
            if !valid_evidence_ids.contains(evidence_ref) {
                result.errors.push(format!(
                    "LLM reasoning item '{}' references missing review evidence ID '{}'",
                    item.asset, evidence_ref
                ));
            }
        }
    }

    for context_file in &pack.asset_context_files {
        if !llm_root.join(context_file).exists() {
            result.errors.push(format!(
                "LLM pack manifest references missing context file '{}'",
                context_file
            ));
        }
    }

    Ok(result)
}

pub fn validate_codex_insights_integrity(input_root: &Path) -> Result<IntegrityResult> {
    let codex_root = input_root.join("codex-insights");
    let summary_path = codex_root.join("codex-summary.json");
    let plan_path = codex_root.join("plans").join("codex-command-plan.json");
    let has_any_artifacts = path_has_files(&codex_root);

    if !has_any_artifacts {
        return Ok(IntegrityResult {
            warnings: vec![
                "Optional Codex insight artifacts were not found. Validation continued without codex-insights checks.".to_string(),
            ],
            errors: Vec::new(),
        });
    }

    let mut result = IntegrityResult::default();
    if !summary_path.exists() || !plan_path.exists() {
        result.errors.push(
            "Codex insight artifacts are incomplete; expected codex-summary.json and plans/codex-command-plan.json."
                .to_string(),
        );
        return Ok(result);
    }

    let summary = load_json::<CodexRunnerSummary>(&summary_path)?;
    let plan = load_json::<CodexRunPlan>(&plan_path)?;
    let mut planned_assets = BTreeSet::new();

    for item in &plan.items {
        if !planned_assets.insert(item.asset.clone()) {
            result.errors.push(format!(
                "Codex command plan contains duplicate asset '{}'",
                item.asset
            ));
        }
        if !item.codex_command.contains("codex exec") {
            result.errors.push(format!(
                "Codex command plan item '{}' does not use `codex exec`.",
                item.asset
            ));
        }
        if item.codex_command.contains("--yolo")
            || item
                .codex_command
                .contains("--dangerously-bypass-approvals-and-sandbox")
        {
            result.errors.push(format!(
                "Codex command plan item '{}' contains a forbidden Codex safety-bypass flag.",
                item.asset
            ));
        }

        let context_path = input_root.join("llm-pack").join(&item.context_file);
        if !context_path.exists() {
            result.errors.push(format!(
                "Codex command plan item '{}' references missing llm-pack context file '{}'.",
                item.asset, item.context_file
            ));
        }
        let template_path = input_root
            .join("llm-pack")
            .join("prompts")
            .join(&item.template);
        if !template_path.exists() {
            result.errors.push(format!(
                "Codex command plan item '{}' references missing llm-pack prompt template '{}'.",
                item.asset, item.template
            ));
        }
    }

    for summary_result in &summary.results {
        if !summary_result.codex_command.contains("codex exec") {
            result.errors.push(format!(
                "Codex result '{}' does not record a `codex exec` invocation.",
                summary_result.asset
            ));
        }
        if summary_result.codex_command.contains("--yolo")
            || summary_result
                .codex_command
                .contains("--dangerously-bypass-approvals-and-sandbox")
        {
            result.errors.push(format!(
                "Codex result '{}' records a forbidden Codex safety-bypass flag.",
                summary_result.asset
            ));
        }

        let result_path = resolve_runtime_path(input_root, &summary_result.result_path);
        if !result_path.exists() {
            result.errors.push(format!(
                "Codex result '{}' references missing markdown output '{}'.",
                summary_result.asset, summary_result.result_path
            ));
            continue;
        }

        let sidecar_path = result_path.with_extension("json");
        if !sidecar_path.exists() {
            result.errors.push(format!(
                "Codex result '{}' is missing sidecar JSON '{}'.",
                summary_result.asset,
                sidecar_path.display()
            ));
            continue;
        }

        let sidecar = load_json::<CodexInsightResult>(&sidecar_path)?;
        if sidecar.asset != summary_result.asset {
            result.errors.push(format!(
                "Codex sidecar '{}' does not match summary asset '{}'.",
                sidecar.asset, summary_result.asset
            ));
        }
    }

    Ok(result)
}

pub fn validate_codex_review_integrity(input_root: &Path) -> Result<IntegrityResult> {
    let review_root = input_root.join("codex-review");
    let queue_path = review_root.join("codex-review-queue.json");
    let unsupported_path = review_root.join("unsupported-claims.json");
    let evidence_gaps_path = review_root.join("evidence-gaps.json");
    let wording_warnings_path = review_root.join("wording-warnings.json");
    let summary_path = review_root.join("codex-review-summary.md");
    let has_any_artifacts = path_has_files(&review_root);

    if !has_any_artifacts {
        return Ok(IntegrityResult {
            warnings: vec![
                "Optional Codex review artifacts were not found. Validation continued without codex-review checks.".to_string(),
            ],
            errors: Vec::new(),
        });
    }

    let mut result = IntegrityResult::default();
    for required in [
        &queue_path,
        &unsupported_path,
        &evidence_gaps_path,
        &wording_warnings_path,
        &summary_path,
    ] {
        if !required.exists() {
            result.errors.push(format!(
                "Codex review artifact is missing: {}",
                required.display()
            ));
        }
    }
    if !result.errors.is_empty() {
        return Ok(result);
    }

    let queue = load_json::<CodexReviewQueueDocument>(&queue_path)?;
    let unsupported_claims = load_json::<Vec<UnsupportedClaim>>(&unsupported_path)?;
    let evidence_gaps = load_json::<Vec<EvidenceGap>>(&evidence_gaps_path)?;
    let wording_warnings = load_json::<Vec<WordingWarning>>(&wording_warnings_path)?;
    let codex_summary = load_optional_json::<CodexRunnerSummary>(
        &input_root.join("codex-insights").join("codex-summary.json"),
    )?
    .unwrap_or(CodexRunnerSummary {
        generated_at: Utc::now(),
        pack_path: String::new(),
        output_root: String::new(),
        execute_requested: false,
        codex_available: false,
        planned_count: 0,
        executed_count: 0,
        success_count: 0,
        failure_count: 0,
        max_prompt_chars: 0,
        limit: 0,
        template_filter: None,
        warnings: Vec::new(),
        results: Vec::new(),
    });
    let valid_assets = codex_summary
        .results
        .iter()
        .map(|result| result.asset.clone())
        .collect::<BTreeSet<_>>();

    if queue.summary.reviewed_items != queue.items.len() {
        result.errors.push(format!(
            "Codex review summary says {} items were reviewed, but the queue contains {} items.",
            queue.summary.reviewed_items,
            queue.items.len()
        ));
    }

    let mut seen_assets = BTreeSet::new();
    for item in &queue.items {
        if !seen_assets.insert(item.asset.clone()) {
            result.errors.push(format!(
                "Codex review queue contains duplicate asset '{}'.",
                item.asset
            ));
        }
        if !valid_assets.is_empty() && !valid_assets.contains(&item.asset) {
            result.errors.push(format!(
                "Codex review queue asset '{}' does not exist in codex-summary.json.",
                item.asset
            ));
        }

        let result_path = resolve_runtime_path(input_root, &item.result_path);
        if !result_path.exists() {
            result.errors.push(format!(
                "Codex review queue asset '{}' references missing result markdown '{}'.",
                item.asset, item.result_path
            ));
        }
        let sidecar_path = resolve_runtime_path(input_root, &item.sidecar_path);
        if !sidecar_path.exists() {
            result.errors.push(format!(
                "Codex review queue asset '{}' references missing sidecar '{}'.",
                item.asset, item.sidecar_path
            ));
        }
    }

    validate_codex_review_assets(
        "unsupported claims",
        unsupported_claims
            .iter()
            .map(|claim| (&claim.asset, &claim.source_path)),
        &seen_assets,
        input_root,
        &mut result,
    );
    validate_codex_review_assets(
        "evidence gaps",
        evidence_gaps
            .iter()
            .map(|gap| (&gap.asset, &gap.source_path)),
        &seen_assets,
        input_root,
        &mut result,
    );
    validate_codex_review_assets(
        "wording warnings",
        wording_warnings
            .iter()
            .map(|warning| (&warning.asset, &warning.source_path)),
        &seen_assets,
        input_root,
        &mut result,
    );

    Ok(result)
}

fn check_expected_directories(input_root: &Path, report: &mut ValidationReport) {
    let required_dirs = [
        "plans",
        "raw",
        "dns",
        "tech",
        "maps",
        "assets",
        "urls",
        "js",
        "params",
        "screenshots",
        "findings",
        "enrichment",
        "api-intel",
        "review",
        "llm-pack",
        "codex-insights",
        "codex-review",
        "reports",
    ];

    for dir in required_dirs {
        let path = input_root.join(dir);
        if path.exists() && path.is_dir() {
            report.checks.push(ValidationCheck {
                name: format!("expected_directory:{dir}"),
                passed: true,
                severity: "info".to_string(),
                message: format!("Expected directory exists: {}", path.display()),
            });
        } else {
            report.errors.push(format!(
                "Expected output directory is missing: {}",
                path.display()
            ));
            report.checks.push(ValidationCheck {
                name: format!("expected_directory:{dir}"),
                passed: false,
                severity: "error".to_string(),
                message: format!("Expected output directory is missing: {}", path.display()),
            });
        }
    }
}

fn check_required_core_artifacts(input_root: &Path, report: &mut ValidationReport) {
    let required_files = [
        "maps/graph.json",
        "maps/clusters.json",
        "maps/anomalies.json",
        "maps/graph-summary.json",
        "enrichment/semantic-assets.json",
        "enrichment/semantic-observations.json",
        "enrichment/risk-explanations.json",
        "enrichment/enriched-graph.json",
        "enrichment/enrichment-summary.md",
        "review/priority-queue.json",
        "review/evidence-index.json",
        "llm-pack/reasoning-queue.json",
        "llm-pack/pack-summary.json",
    ];

    for relative in required_files {
        let path = input_root.join(relative);
        if path.exists() && path.is_file() {
            report.checks.push(ValidationCheck {
                name: format!("required_artifact:{relative}"),
                passed: true,
                severity: "info".to_string(),
                message: format!("Required artifact exists: {}", path.display()),
            });
        } else {
            report
                .errors
                .push(format!("Required artifact is missing: {}", path.display()));
            report.checks.push(ValidationCheck {
                name: format!("required_artifact:{relative}"),
                passed: false,
                severity: "error".to_string(),
                message: format!("Required artifact is missing: {}", path.display()),
            });
        }
    }

    let llm_context_dir = input_root.join("llm-pack").join("asset-contexts");
    if !llm_context_dir.exists() || !llm_context_dir.is_dir() {
        report.errors.push(format!(
            "Required artifact directory is missing: {}",
            llm_context_dir.display()
        ));
    }

    let optional_api_files = [
        "api-intel/api-endpoints.json",
        "api-intel/api-objects.json",
        "api-intel/auth-observations.json",
        "api-intel/js-observations.json",
        "api-intel/schemas.json",
        "api-intel/graphql-observations.json",
        "api-intel/api-graph.json",
    ];
    let api_present = optional_api_files
        .iter()
        .any(|relative| input_root.join(relative).exists());
    if !api_present {
        report.warnings.push(
            "Optional API-intel artifacts were not found. Validation continued without API-specific artifact checks.".to_string(),
        );
    }
}

fn parse_structured_files(input_root: &Path, report: &mut ValidationReport) -> Result<()> {
    for entry in WalkDir::new(input_root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        match path.extension().and_then(|value| value.to_str()) {
            Some("json") => {
                if let Err(error) = validate_json_file(path) {
                    report.errors.push(format!("{error:#}"));
                }
            }
            Some("jsonl") => {
                if let Err(error) = validate_jsonl_file(path) {
                    report.errors.push(format!("{error:#}"));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_enrichment_integrity(input_root: &Path, report: &mut ValidationReport) -> Result<()> {
    let enrichment_root = input_root.join("enrichment");
    let semantic_assets =
        load_json::<Vec<EnrichedAsset>>(&enrichment_root.join("semantic-assets.json"))?;
    let observations =
        load_json::<Vec<SemanticObservation>>(&enrichment_root.join("semantic-observations.json"))?;
    let risks = load_json::<Vec<RiskExplanation>>(&enrichment_root.join("risk-explanations.json"))?;
    let enriched_graph = load_json::<EnrichedGraph>(&enrichment_root.join("enriched-graph.json"))?;

    let base_assets = if semantic_assets.is_empty() {
        enriched_graph.assets.clone()
    } else {
        semantic_assets.clone()
    };
    let asset_ids = base_assets
        .iter()
        .map(|asset| asset.asset.clone())
        .collect::<Vec<_>>();

    check_duplicate_assets("enrichment assets", &asset_ids, report);

    let asset_set = asset_ids.into_iter().collect::<BTreeSet<_>>();
    for observation in observations {
        if !asset_set.contains(&observation.asset) {
            report.errors.push(format!(
                "Semantic observation references missing asset '{}'",
                observation.asset
            ));
        }
    }
    for risk in risks {
        if !asset_set.contains(&risk.asset) {
            report.errors.push(format!(
                "Risk explanation references missing asset '{}'",
                risk.asset
            ));
        }
    }
    for asset in &enriched_graph.assets {
        if !asset_set.contains(&asset.asset) {
            report.errors.push(format!(
                "Enriched graph asset '{}' is missing from semantic-assets.json",
                asset.asset
            ));
        }
    }

    Ok(())
}

fn check_empty_high_value_outputs(input_root: &Path, report: &mut ValidationReport) -> Result<()> {
    let graph = load_json::<GraphDocument>(&input_root.join("maps").join("graph.json"))?;
    if graph.nodes.is_empty() {
        report.warnings.push(
            "High-value output maps/graph.json is empty; graph correlation may not have run or may not have produced usable records."
                .to_string(),
        );
    }

    let semantic_assets = load_json::<Vec<EnrichedAsset>>(
        &input_root.join("enrichment").join("semantic-assets.json"),
    )?;
    if semantic_assets.is_empty() {
        report
            .warnings
            .push("High-value output enrichment/semantic-assets.json is empty.".to_string());
    }

    let review =
        load_json::<PriorityQueueDocument>(&input_root.join("review").join("priority-queue.json"))?;
    if review.items.is_empty() {
        report
            .warnings
            .push("High-value output review/priority-queue.json is empty.".to_string());
    }

    let reasoning = load_json::<ReasoningQueueDocument>(
        &input_root.join("llm-pack").join("reasoning-queue.json"),
    )?;
    if reasoning.items.is_empty() {
        report
            .warnings
            .push("High-value output llm-pack/reasoning-queue.json is empty.".to_string());
    }

    let codex_summary_path = input_root.join("codex-insights").join("codex-summary.json");
    if codex_summary_path.exists() {
        let codex_summary = load_json::<CodexRunnerSummary>(&codex_summary_path)?;
        if codex_summary.results.is_empty() {
            report.warnings.push(
                "Optional output codex-insights/codex-summary.json contains no executed reasoning results."
                    .to_string(),
            );
        }
    }

    Ok(())
}

fn extend_report(prefix: &str, result: &IntegrityResult, report: &mut ValidationReport) {
    for warning in &result.warnings {
        report.warnings.push(warning.clone());
        report.checks.push(ValidationCheck {
            name: prefix.to_string(),
            passed: true,
            severity: "warning".to_string(),
            message: warning.clone(),
        });
    }
    for error in &result.errors {
        report.errors.push(error.clone());
        report.checks.push(ValidationCheck {
            name: prefix.to_string(),
            passed: false,
            severity: "error".to_string(),
            message: error.clone(),
        });
    }
    if result.warnings.is_empty() && result.errors.is_empty() {
        report.checks.push(ValidationCheck {
            name: prefix.to_string(),
            passed: true,
            severity: "info".to_string(),
            message: format!("{prefix} checks passed."),
        });
    }
}

fn resolve_evidence_source(input_root: &Path, file_name: &str) -> Option<PathBuf> {
    let candidates = [
        input_root.join("review").join(file_name),
        input_root.join("enrichment").join(file_name),
        input_root.join("api-intel").join(file_name),
        input_root.join("maps").join(file_name),
        input_root.join(file_name),
    ];
    candidates
        .into_iter()
        .find(|candidate| candidate.exists() && candidate.is_file())
}

fn collect_artifact_counts(input_root: &Path) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    counts.insert("total_files".to_string(), utils::count_files(input_root));
    for label in [
        "plans",
        "raw",
        "maps",
        "enrichment",
        "api-intel",
        "review",
        "llm-pack",
        "codex-insights",
        "codex-review",
    ] {
        counts.insert(
            label.to_string(),
            utils::count_files(&input_root.join(label)),
        );
    }
    counts
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read JSON input: {}", path.display()))?;
    serde_json::from_str::<T>(&raw)
        .with_context(|| format!("failed to parse JSON input: {}", path.display()))
}

fn load_optional_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    load_json(path).map(Some)
}

fn check_duplicate_assets(context: &str, assets: &[String], report: &mut ValidationReport) {
    let mut seen = BTreeSet::new();
    let mut duplicates = Vec::new();
    for asset in assets {
        if !seen.insert(asset.clone()) {
            duplicates.push(asset.clone());
        }
    }
    if !duplicates.is_empty() {
        report.errors.push(format!(
            "{context} contain duplicate asset IDs: {}",
            duplicates.join(", ")
        ));
    }
}

fn path_has_files(path: &Path) -> bool {
    path.exists()
        && WalkDir::new(path)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .any(|entry| entry.file_type().is_file())
}

fn resolve_runtime_path(input_root: &Path, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        return candidate;
    }

    let input_root_name = input_root.file_name().and_then(|value| value.to_str());
    if matches!(input_root_name, Some("output")) {
        let parent = input_root.parent().unwrap_or(input_root);
        let joined = parent.join(&candidate);
        if joined.exists() {
            return joined;
        }
    }

    let joined = input_root.join(&candidate);
    if joined.exists() {
        joined
    } else {
        candidate
    }
}

fn validate_codex_review_assets<'a, I>(
    label: &str,
    entries: I,
    valid_assets: &BTreeSet<String>,
    input_root: &Path,
    result: &mut IntegrityResult,
) where
    I: IntoIterator<Item = (&'a String, &'a String)>,
{
    for (asset, source_path) in entries {
        if !valid_assets.contains(asset) {
            result.errors.push(format!(
                "Codex review {} reference unknown asset '{}'.",
                label, asset
            ));
        }
        let resolved = resolve_runtime_path(input_root, source_path);
        if !resolved.exists() {
            result.errors.push(format!(
                "Codex review {} reference missing source path '{}'.",
                label, source_path
            ));
        }
    }
}

fn render_markdown_report(report: &ValidationReport) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot Validation Report\n\n");
    output.push_str(&format!(
        "- Generated at: {}\n- Input root: {}\n- Status: {}\n- Warnings: {}\n- Errors: {}\n\n",
        report.generated_at.to_rfc3339(),
        report.input_root,
        if report.passed { "passed" } else { "failed" },
        report.warnings.len(),
        report.errors.len()
    ));

    output.push_str("## Checks\n\n");
    if report.checks.is_empty() {
        output.push_str("No validation checks were recorded.\n");
    } else {
        for check in &report.checks {
            output.push_str(&format!(
                "- [{}] {}: {}\n",
                if check.passed {
                    "pass"
                } else {
                    check.severity.as_str()
                },
                check.name,
                check.message
            ));
        }
    }

    output.push_str("\n## Warnings\n\n");
    write_markdown_list(&mut output, &report.warnings);

    output.push_str("\n## Errors\n\n");
    write_markdown_list(&mut output, &report.errors);

    output.push_str("\n## Artifact Counts\n\n");
    for (label, count) in &report.artifact_counts {
        output.push_str(&format!("- {}: {}\n", label, count));
    }

    output
}

fn write_markdown_list(output: &mut String, values: &[String]) {
    if values.is_empty() {
        output.push_str("None observed.\n");
        return;
    }
    for value in values {
        output.push_str(&format!("- {}\n", value));
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use super::validate_output_tree;
    use crate::utils::ensure_output_structure;

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
                "reconpilot-validate-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            Ok(Self { root })
        }

        fn output_root(&self) -> PathBuf {
            self.root.join("output")
        }

        fn write_file(&self, relative: &str, content: &str) -> Result<()> {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, content)?;
            Ok(())
        }

        fn seed_valid_tree(&self) -> Result<()> {
            let _ = ensure_output_structure(&self.output_root())?;
            self.write_file(
                "output/maps/graph.json",
                r#"{"nodes":[{"id":"host:app","node_type":"host","value":"app.example.com","tags":[],"metadata":{},"source_tools":["httpx"],"timestamps":["2026-05-14T09:00:00Z"]},{"id":"cluster:app","node_type":"cluster","value":"app-cluster","tags":[],"metadata":{},"source_tools":["correlation-engine"],"timestamps":["2026-05-14T09:00:00Z"]}],"edges":[{"source":"host:app","target":"cluster:app","relationship":"belongs_to_cluster","confidence":0.9,"evidence":[],"timestamps":["2026-05-14T09:00:00Z"]}]}"#,
            )?;
            self.write_file(
                "output/maps/clusters.json",
                r#"{"clusters":[{"cluster_id":"cluster:app","cluster_type":"app-cluster","related_nodes":["host:app"],"shared_indicators":["app"],"risk_score":40}]}"#,
            )?;
            self.write_file("output/maps/anomalies.json", r#"{"anomalies":[]}"#)?;
            self.write_file(
                "output/maps/graph-summary.json",
                r#"{"generated_at":"2026-05-14T09:00:00Z","node_count":2,"edge_count":1,"cluster_count":1,"anomaly_count":0,"top_technologies":[],"largest_clusters":[],"shared_infrastructure":[],"suspicious_naming":[],"likely_staging_systems":[],"likely_internal_systems":[],"redirect_chain_count":0}"#,
            )?;
            self.write_file(
                "output/enrichment/semantic-assets.json",
                r#"[{"asset":"app.example.com","semantic_tags":[],"roles":["customer_app"],"environments":["production"],"risk_explanations":[{"asset":"app.example.com","risk_level":"medium","score":40,"explanation":"Interesting candidate worth manual review.","contributing_factors":["Graph-neighborhood evidence"],"recommended_next_steps":["Review route families carefully."]}],"related_nodes":["host:app"],"api_endpoints":[],"api_objects":[],"auth_observations":[],"js_observations":[],"schema_observations":[],"graphql_observations":[],"neighborhood_summary":"Interesting cluster context."}]"#,
            )?;
            self.write_file(
                "output/enrichment/semantic-observations.json",
                r#"[{"observation_id":"obs:1","asset":"app.example.com","observation_type":"neighborhood","description":"Interesting cluster context.","evidence":["Cluster evidence"],"confidence":0.7,"related_nodes":["host:app"]}]"#,
            )?;
            self.write_file(
                "output/enrichment/risk-explanations.json",
                r#"[{"asset":"app.example.com","risk_level":"medium","score":40,"explanation":"Interesting candidate worth manual review.","contributing_factors":["Graph-neighborhood evidence"],"recommended_next_steps":["Review route families carefully."]}]"#,
            )?;
            self.write_file(
                "output/enrichment/enriched-graph.json",
                r#"{"assets":[{"asset":"app.example.com","semantic_tags":[],"roles":["customer_app"],"environments":["production"],"risk_explanations":[{"asset":"app.example.com","risk_level":"medium","score":40,"explanation":"Interesting candidate worth manual review.","contributing_factors":["Graph-neighborhood evidence"],"recommended_next_steps":["Review route families carefully."]}],"related_nodes":["host:app"],"api_endpoints":[],"api_objects":[],"auth_observations":[],"js_observations":[],"schema_observations":[],"graphql_observations":[],"neighborhood_summary":"Interesting cluster context."}],"observations":[{"observation_id":"obs:1","asset":"app.example.com","observation_type":"neighborhood","description":"Interesting cluster context.","evidence":["Cluster evidence"],"confidence":0.7,"related_nodes":["host:app"]}],"risk_explanations":[{"asset":"app.example.com","risk_level":"medium","score":40,"explanation":"Interesting candidate worth manual review.","contributing_factors":["Graph-neighborhood evidence"],"recommended_next_steps":["Review route families carefully."]}],"original_graph_summary":{"generated_at":"2026-05-14T09:00:00Z","node_count":2,"edge_count":1,"cluster_count":1,"anomaly_count":0,"top_technologies":[],"largest_clusters":[],"shared_infrastructure":[],"suspicious_naming":[],"likely_staging_systems":[],"likely_internal_systems":[],"redirect_chain_count":0},"original_nodes":[{"id":"host:app","node_type":"host","value":"app.example.com","tags":[],"metadata":{},"source_tools":["httpx"],"timestamps":["2026-05-14T09:00:00Z"]}],"original_edges":[{"source":"host:app","target":"cluster:app","relationship":"belongs_to_cluster","confidence":0.9,"evidence":[],"timestamps":["2026-05-14T09:00:00Z"]}],"original_clusters":[{"cluster_id":"cluster:app","cluster_type":"app-cluster","related_nodes":["host:app"],"shared_indicators":["app"],"risk_score":40}],"semantic_summary":{"generated_at":"2026-05-14T09:00:00Z","asset_count":1,"observation_count":1,"risk_explanation_count":1,"api_endpoint_count":0,"auth_surface_count":0,"js_observation_count":0,"schema_observation_count":0,"graphql_observation_count":0,"top_roles":["customer_app (1)"],"top_environments":["production (1)"],"highest_priority_assets":["app.example.com [medium:40]"],"notable_neighborhood_observations":["Interesting cluster context."],"sensitive_object_candidates":[],"api_intelligence_warnings":[],"recommended_next_steps":["Review route families carefully."]}}"#,
            )?;
            self.write_file(
                "output/enrichment/enrichment-summary.md",
                "# Enrichment Summary\n\nValid.\n",
            )?;
            self.write_file(
                "output/review/priority-queue.json",
                r#"{"summary":{"total_assets":1,"total_observations":1,"high_priority_count":0,"medium_priority_count":1,"top_roles":["customer_app (1)"],"top_environments":["production (1)"],"top_review_targets":["app.example.com [medium:40]"]},"items":[{"rank":1,"asset":"app.example.com","risk_level":"medium","score":40,"confidence":0.7,"semantic_roles":["customer_app"],"environments":["production"],"reasons":["Graph-neighborhood evidence"],"evidence_refs":["evidence-1"],"recommended_next_steps":["Review route families carefully."]}]}"#,
            )?;
            self.write_file(
                "output/review/evidence-index.json",
                r#"{"summary":{"total_assets":1,"total_observations":1,"high_priority_count":0,"medium_priority_count":1,"top_roles":["customer_app (1)"],"top_environments":["production (1)"],"top_review_targets":["app.example.com [medium:40]"]},"items":[{"review_item":"app.example.com","review_rank":1,"evidence_id":"evidence-1","asset":"app.example.com","source":"semantic-observations.json","evidence_type":"neighborhood","description":"Interesting cluster context.","related_nodes":["host:app"],"related_edges":[]}]}"#,
            )?;
            self.write_file(
                "output/llm-pack/reasoning-queue.json",
                r#"{"summary":{"generated_at":"2026-05-14T09:00:00Z","asset_context_count":1,"prompt_template_count":1,"reasoning_item_count":1,"max_context_chars":12000,"total_evidence_refs":1,"truncated_context_count":0,"api_intel_present":false,"graph_summary_present":true,"top_review_themes":[],"top_api_auth_areas":[],"top_graph_clusters":[],"top_unknowns":[],"suggested_review_order":[],"warnings":[]},"items":[{"rank":1,"asset":"app.example.com","review_rank":1,"risk_level":"medium","score":40,"confidence":0.7,"reasoning_score":60,"suggested_prompt_template":"asset_triage_prompt.md","context_file":"asset-contexts/001-app-example-com.json","why_llm_review":["Structured context available."],"evidence_refs":["evidence-1"]}]}"#,
            )?;
            self.write_file(
                "output/llm-pack/pack-summary.json",
                r#"{"generated_at":"2026-05-14T09:00:00Z","max_context_chars":12000,"asset_context_files":["asset-contexts/001-app-example-com.json"],"prompts":[{"name":"asset_triage_prompt","file_name":"asset_triage_prompt.md","purpose":"Triage","recommended_for":[],"safety_constraints":[],"template_markdown":"Prompt"}],"reasoning_queue":[{"rank":1,"asset":"app.example.com","review_rank":1,"risk_level":"medium","score":40,"confidence":0.7,"reasoning_score":60,"suggested_prompt_template":"asset_triage_prompt.md","context_file":"asset-contexts/001-app-example-com.json","why_llm_review":["Structured context available."],"evidence_refs":["evidence-1"]}],"summary":{"generated_at":"2026-05-14T09:00:00Z","asset_context_count":1,"prompt_template_count":1,"reasoning_item_count":1,"max_context_chars":12000,"total_evidence_refs":1,"truncated_context_count":0,"api_intel_present":false,"graph_summary_present":true,"top_review_themes":[],"top_api_auth_areas":[],"top_graph_clusters":[],"top_unknowns":[],"suggested_review_order":[],"warnings":[]}}"#,
            )?;
            self.write_file(
                "output/llm-pack/asset-contexts/001-app-example-com.json",
                r#"{"asset":"app.example.com","risk_level":"medium","score":40,"confidence":0.7,"semantic_roles":["customer_app"],"environments":["production"],"graph_neighborhood_summary":"Interesting cluster context.","api_observations":[],"api_object_candidates":[],"auth_observations":[],"js_observations":[],"schema_observations":[],"graphql_observations":[],"evidence_refs":["evidence-1"],"evidence_highlights":["[evidence-1] semantic-observations.json :: Interesting cluster context."],"cautious_next_step_questions":["Which evidence-backed candidate is worth validating first?"],"context_markdown":"Context","estimated_chars":7,"truncated":false,"truncation_notes":[]}"#,
            )?;
            self.write_file(
                "output/llm-pack/prompts/asset_triage_prompt.md",
                "# Prompt\n",
            )?;
            Ok(())
        }

        fn seed_codex_outputs(&self) -> Result<()> {
            self.write_file(
                "output/codex-insights/plans/codex-command-plan.json",
                r#"{"generated_at":"2026-05-15T12:00:00Z","pack_path":"output/llm-pack","output_root":"output/codex-insights","execute_requested":false,"codex_available":false,"max_prompt_chars":12000,"limit":1,"template_filter":null,"items":[{"rank":1,"asset":"app.example.com","template":"asset_triage_prompt.md","context_file":"asset-contexts/001-app-example-com.json","prompt_chars":400,"codex_command":"codex exec \"prompt\"","executed":false,"evidence_refs":["evidence-1"],"why_selected":["Structured context available."],"warnings":[]}],"warnings":[]}"#,
            )?;
            self.write_file(
                "output/codex-insights/codex-summary.json",
                r#"{"generated_at":"2026-05-15T12:01:00Z","pack_path":"output/llm-pack","output_root":"output/codex-insights","execute_requested":true,"codex_available":true,"planned_count":1,"executed_count":1,"success_count":1,"failure_count":0,"max_prompt_chars":12000,"limit":1,"template_filter":null,"warnings":[],"results":[{"asset":"app.example.com","template":"asset_triage_prompt.md","codex_command":"codex exec \"prompt\"","executed":true,"exit_status":0,"stdout_path":"output/codex-insights/logs/codex-stdout.log","stderr_path":"output/codex-insights/logs/codex-stderr.log","result_path":"output/codex-insights/results/001-app-example-com.md","timestamp":"2026-05-15T12:01:00Z","warnings":[]}]}"#,
            )?;
            self.write_file(
                "output/codex-insights/results/001-app-example-com.md",
                "# Analyst summary\n\nInteresting candidate that requires validation.\n\n# Safe next review steps\n- Review route families carefully.\n",
            )?;
            self.write_file(
                "output/codex-insights/results/001-app-example-com.json",
                r#"{"asset":"app.example.com","template":"asset_triage_prompt.md","codex_command":"codex exec \"prompt\"","executed":true,"exit_status":0,"stdout_path":"output/codex-insights/logs/codex-stdout.log","stderr_path":"output/codex-insights/logs/codex-stderr.log","result_path":"output/codex-insights/results/001-app-example-com.md","timestamp":"2026-05-15T12:01:00Z","warnings":[]}"#,
            )?;
            self.write_file(
                "output/codex-review/codex-review-queue.json",
                r#"{"summary":{"generated_at":"2026-05-15T12:02:00Z","total_results":1,"reviewed_items":1,"executed_count":1,"plan_only_count":0,"unsupported_claim_count":0,"evidence_gap_count":0,"wording_warning_count":0,"top_review_targets":["app.example.com [claims:0 gaps:0 warnings:0]"],"warnings":[]},"items":[{"rank":1,"asset":"app.example.com","template":"asset_triage_prompt.md","executed":true,"exit_status":0,"result_path":"output/codex-insights/results/001-app-example-com.md","sidecar_path":"output/codex-insights/results/001-app-example-com.json","expected_evidence_refs":["evidence-1"],"mentioned_evidence_refs":["evidence-1"],"analyst_recommendations":["Review route families carefully."],"analyst_summary":"Interesting candidate that requires validation.","requires_validation_language":true,"unsupported_claim_count":0,"evidence_gap_count":0,"wording_warning_count":0,"caution_notes":["Codex output remained cautious, but it still requires analyst validation."]}]}"#,
            )?;
            self.write_file(
                "output/codex-review/codex-review-queue.md",
                "# Codex Review Queue\n\n- Generated.\n",
            )?;
            self.write_file("output/codex-review/unsupported-claims.json", "[]")?;
            self.write_file("output/codex-review/evidence-gaps.json", "[]")?;
            self.write_file("output/codex-review/wording-warnings.json", "[]")?;
            self.write_file(
                "output/codex-review/codex-review-summary.md",
                "# Codex Review Summary\n\nLocal annotations only.\n",
            )?;
            Ok(())
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn validation_success() -> Result<()> {
        let workspace = TestWorkspace::new("success")?;
        workspace.seed_valid_tree()?;

        let report = validate_output_tree(&workspace.output_root())?;
        assert!(report.passed);
        Ok(())
    }

    #[test]
    fn validation_failure_on_malformed_json() -> Result<()> {
        let workspace = TestWorkspace::new("malformed-json")?;
        workspace.seed_valid_tree()?;
        workspace.write_file("output/review/priority-queue.json", "{bad-json")?;

        let report = validate_output_tree(&workspace.output_root())?;
        assert!(!report.passed);
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("priority-queue.json")));
        Ok(())
    }

    #[test]
    fn graph_edge_missing_node_detection() -> Result<()> {
        let workspace = TestWorkspace::new("graph-missing-node")?;
        workspace.seed_valid_tree()?;
        workspace.write_file(
            "output/maps/graph.json",
            r#"{"nodes":[{"id":"host:app","node_type":"host","value":"app.example.com","tags":[],"metadata":{},"source_tools":["httpx"],"timestamps":["2026-05-14T09:00:00Z"]}],"edges":[{"source":"host:app","target":"missing:node","relationship":"references","confidence":0.9,"evidence":[],"timestamps":["2026-05-14T09:00:00Z"]}]}"#,
        )?;

        let report = validate_output_tree(&workspace.output_root())?;
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("missing target node")));
        Ok(())
    }

    #[test]
    fn review_evidence_reference_validation() -> Result<()> {
        let workspace = TestWorkspace::new("review-evidence")?;
        workspace.seed_valid_tree()?;
        workspace.write_file(
            "output/review/priority-queue.json",
            r#"{"summary":{"total_assets":1,"total_observations":1,"high_priority_count":0,"medium_priority_count":1,"top_roles":["customer_app (1)"],"top_environments":["production (1)"],"top_review_targets":["app.example.com [medium:40]"]},"items":[{"rank":1,"asset":"app.example.com","risk_level":"medium","score":40,"confidence":0.7,"semantic_roles":["customer_app"],"environments":["production"],"reasons":["Graph-neighborhood evidence"],"evidence_refs":["missing-evidence"],"recommended_next_steps":["Review route families carefully."]}]}"#,
        )?;

        let report = validate_output_tree(&workspace.output_root())?;
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("missing evidence ID")));
        Ok(())
    }

    #[test]
    fn llm_pack_missing_context_detection() -> Result<()> {
        let workspace = TestWorkspace::new("llm-pack-missing-context")?;
        workspace.seed_valid_tree()?;
        fs::remove_file(
            workspace
                .output_root()
                .join("llm-pack")
                .join("asset-contexts")
                .join("001-app-example-com.json"),
        )?;

        let report = validate_output_tree(&workspace.output_root())?;
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("missing context file")));
        Ok(())
    }

    #[test]
    fn empty_output_tree_handling() -> Result<()> {
        let workspace = TestWorkspace::new("empty-tree")?;
        fs::create_dir_all(workspace.output_root())?;

        let report = validate_output_tree(&workspace.output_root())?;
        assert!(!report.passed);
        assert!(!report.errors.is_empty());
        Ok(())
    }

    #[test]
    fn validation_includes_codex_review_outputs() -> Result<()> {
        let workspace = TestWorkspace::new("codex-review")?;
        workspace.seed_valid_tree()?;
        workspace.seed_codex_outputs()?;

        let report = validate_output_tree(&workspace.output_root())?;
        assert!(report.passed);
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "codex_review_integrity"));
        Ok(())
    }
}
