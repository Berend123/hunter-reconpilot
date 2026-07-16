use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    models::{
        ApiEndpoint, ApiGraphSummary, ApiIntelBundle, ApiObject, ApiSchema, AssetCluster,
        AssetRole, AuthObservation, EnrichedAsset, EnrichedGraph, EnvironmentType, EvidenceItem,
        GraphQlObservation, GraphSummary, JsObservation, LlmAssetContext, LlmContextPack,
        LlmPackSummary, LlmPromptTemplate, LlmReasoningItem, ReviewItem, ReviewSummary,
        RiskExplanation, SemanticObservation,
    },
    utils,
};

#[derive(Debug, Clone)]
pub struct LlmPackOutcome {
    pub asset_contexts_dir: PathBuf,
    pub prompts_dir: PathBuf,
    pub reasoning_queue_json_path: PathBuf,
    pub reasoning_queue_markdown_path: PathBuf,
    pub analyst_brief_path: PathBuf,
    pub pack_summary_path: PathBuf,
    pub summary: LlmPackSummary,
}

#[derive(Debug, Clone, Deserialize)]
struct PriorityQueueDocument {
    summary: ReviewSummary,
    #[serde(default)]
    items: Vec<ReviewItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct EvidenceIndexEntry {
    review_item: String,
    review_rank: usize,
    #[serde(flatten)]
    evidence: EvidenceItem,
}

#[derive(Debug, Clone, Deserialize)]
struct EvidenceIndexDocument {
    summary: ReviewSummary,
    #[serde(default)]
    items: Vec<EvidenceIndexEntry>,
}

#[derive(Debug, Clone, Serialize)]
struct ReasoningQueueDocument {
    summary: LlmPackSummary,
    items: Vec<LlmReasoningItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ImportedClustersDocument {
    #[serde(default)]
    clusters: Vec<AssetCluster>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ImportedApiGraphDocument {
    #[serde(default)]
    summary: Option<ApiGraphSummary>,
}

#[derive(Debug, Clone)]
struct ReviewBundle {
    summary: ReviewSummary,
    items: Vec<ReviewItem>,
    evidence_entries: Vec<EvidenceIndexEntry>,
    executive_summary: Option<String>,
}

#[derive(Debug, Clone)]
struct EnrichmentBundle {
    assets: Vec<EnrichedAsset>,
    observations: Vec<SemanticObservation>,
    risks: Vec<RiskExplanation>,
    summary_markdown: String,
}

#[derive(Debug, Clone, Default)]
struct GraphSupport {
    summary: Option<GraphSummary>,
    clusters: Vec<AssetCluster>,
}

#[derive(Debug, Clone, Default)]
struct ApiAssetContext {
    endpoints: Vec<ApiEndpoint>,
    objects: Vec<ApiObject>,
    auth_observations: Vec<AuthObservation>,
    js_observations: Vec<JsObservation>,
    schemas: Vec<ApiSchema>,
    graphql_observations: Vec<GraphQlObservation>,
}

#[derive(Debug, Clone)]
struct ContextRecord {
    review_item: ReviewItem,
    asset: EnrichedAsset,
    context: LlmAssetContext,
    context_file: String,
    observation_count: usize,
}

pub fn run_llm_pack(
    input_root: &Path,
    out: &Path,
    max_context_chars: usize,
) -> Result<LlmPackOutcome> {
    if max_context_chars == 0 {
        bail!("max_context_chars must be greater than zero");
    }

    validate_llm_pack_input(input_root)?;
    utils::ensure_directory(out)?;

    let mut warnings = Vec::new();
    let review = load_review_bundle(input_root)?;
    let enrichment = load_enrichment_bundle(input_root)?;
    let api_bundle = load_optional_api_bundle(input_root, &mut warnings)?;
    let graph_support = load_optional_graph_support(input_root, &mut warnings)?;

    let prompts = prompt_templates();
    let mut records = build_context_records(
        &review,
        &enrichment,
        api_bundle.as_ref(),
        &graph_support,
        max_context_chars,
    );
    let mut reasoning_queue = build_reasoning_queue(&records);
    re_rank_reasoning_queue(&mut reasoning_queue);

    let summary = build_pack_summary(
        &records,
        &reasoning_queue,
        &prompts,
        max_context_chars,
        api_bundle.is_some(),
        graph_support.summary.is_some(),
        &graph_support,
        &warnings,
    );

    write_outputs(
        out,
        &prompts,
        &mut records,
        &reasoning_queue,
        &review,
        &enrichment,
        &summary,
        max_context_chars,
    )
}

pub fn print_llm_pack_summary(input: &Path, out: &Path, outcome: &LlmPackOutcome) {
    println!("ReconPilot LLM pack summary");
    println!("Input root: {}", input.display());
    println!("Output pack: {}", out.display());
    println!("Asset contexts: {}", outcome.summary.asset_context_count);
    println!("Reasoning items: {}", outcome.summary.reasoning_item_count);
    println!(
        "Prompt templates: {} | Truncated contexts: {}",
        outcome.summary.prompt_template_count, outcome.summary.truncated_context_count
    );
    println!(
        "API-intel present: {} | Graph summary present: {}",
        outcome.summary.api_intel_present, outcome.summary.graph_summary_present
    );
    println!("Outputs:");
    println!("  - {}", outcome.asset_contexts_dir.display());
    println!("  - {}", outcome.prompts_dir.display());
    println!("  - {}", outcome.reasoning_queue_json_path.display());
    println!("  - {}", outcome.reasoning_queue_markdown_path.display());
    println!("  - {}", outcome.analyst_brief_path.display());
    println!("  - {}", outcome.pack_summary_path.display());
    for warning in &outcome.summary.warnings {
        println!("Warning: {warning}");
    }
}

fn validate_llm_pack_input(input_root: &Path) -> Result<()> {
    if !input_root.exists() {
        bail!(
            "LLM pack input root does not exist: {}",
            input_root.display()
        );
    }
    if !input_root.is_dir() {
        bail!(
            "LLM pack input root is not a directory: {}",
            input_root.display()
        );
    }

    for relative in [
        "enrichment/semantic-assets.json",
        "enrichment/semantic-observations.json",
        "enrichment/risk-explanations.json",
        "enrichment/enriched-graph.json",
        "enrichment/enrichment-summary.md",
        "review/priority-queue.json",
        "review/evidence-index.json",
    ] {
        let path = input_root.join(relative);
        if !path.exists() {
            bail!(
                "required LLM pack input is missing: {}. Run `reconpilot enrich` and `reconpilot review` first.",
                path.display()
            );
        }
    }

    Ok(())
}

fn load_review_bundle(input_root: &Path) -> Result<ReviewBundle> {
    let review_root = input_root.join("review");
    let queue = load_json::<PriorityQueueDocument>(&review_root.join("priority-queue.json"))?;
    let evidence = load_json::<EvidenceIndexDocument>(&review_root.join("evidence-index.json"))?;
    let executive_summary = fs::read_to_string(review_root.join("executive-summary.md")).ok();

    let summary = if queue.summary.total_assets > 0 {
        queue.summary
    } else {
        evidence.summary
    };

    Ok(ReviewBundle {
        summary,
        items: queue.items,
        evidence_entries: evidence.items,
        executive_summary,
    })
}

fn load_enrichment_bundle(input_root: &Path) -> Result<EnrichmentBundle> {
    let enrichment_root = input_root.join("enrichment");
    let assets = load_json::<Vec<EnrichedAsset>>(&enrichment_root.join("semantic-assets.json"))?;
    let observations =
        load_json::<Vec<SemanticObservation>>(&enrichment_root.join("semantic-observations.json"))?;
    let risks = load_json::<Vec<RiskExplanation>>(&enrichment_root.join("risk-explanations.json"))?;
    let enriched_graph = load_json::<EnrichedGraph>(&enrichment_root.join("enriched-graph.json"))?;
    let summary_markdown = fs::read_to_string(enrichment_root.join("enrichment-summary.md"))
        .with_context(|| {
            format!(
                "failed to read enrichment summary markdown at {}",
                enrichment_root.join("enrichment-summary.md").display()
            )
        })?;

    let assets = if assets.is_empty() {
        enriched_graph.assets.clone()
    } else {
        assets
    };

    Ok(EnrichmentBundle {
        assets,
        observations,
        risks,
        summary_markdown,
    })
}

fn load_optional_api_bundle(
    input_root: &Path,
    warnings: &mut Vec<String>,
) -> Result<Option<ApiIntelBundle>> {
    let api_root = input_root.join("api-intel");
    if !api_root.exists() || !api_root.is_dir() {
        return Ok(None);
    }

    let mut bundle = ApiIntelBundle::default();
    bundle.endpoints = load_optional_json_with_warning(
        &api_root,
        "api-endpoints.json",
        warnings,
        "API intelligence artifact",
    )?;
    bundle.objects = load_optional_json_with_warning(
        &api_root,
        "api-objects.json",
        warnings,
        "API intelligence artifact",
    )?;
    bundle.relationships = load_optional_json_with_warning(
        &api_root,
        "api-relationships.json",
        warnings,
        "API intelligence artifact",
    )?;
    bundle.auth_observations = load_optional_json_with_warning(
        &api_root,
        "auth-observations.json",
        warnings,
        "API intelligence artifact",
    )?;
    bundle.js_observations = load_optional_json_with_warning(
        &api_root,
        "js-observations.json",
        warnings,
        "API intelligence artifact",
    )?;
    bundle.schemas = load_optional_json_with_warning(
        &api_root,
        "schemas.json",
        warnings,
        "API intelligence artifact",
    )?;
    bundle.graphql_observations = load_optional_json_with_warning(
        &api_root,
        "graphql-observations.json",
        warnings,
        "API intelligence artifact",
    )?;

    let api_graph_path = api_root.join("api-graph.json");
    if api_graph_path.exists() {
        let raw = fs::read_to_string(&api_graph_path).with_context(|| {
            format!(
                "failed to read API intelligence graph input: {}",
                api_graph_path.display()
            )
        })?;
        let document =
            serde_json::from_str::<ImportedApiGraphDocument>(&raw).with_context(|| {
                format!(
                    "failed to parse API intelligence graph input: {}",
                    api_graph_path.display()
                )
            })?;
        bundle.summary = document.summary;
    } else {
        warnings.push(format!(
            "Optional API intelligence artifact missing: {}",
            api_graph_path.display()
        ));
    }

    let api_summary_path = api_root.join("api-summary.md");
    if api_summary_path.exists() {
        bundle.summary_markdown =
            Some(fs::read_to_string(&api_summary_path).with_context(|| {
                format!(
                    "failed to read API intelligence summary input: {}",
                    api_summary_path.display()
                )
            })?);
    } else {
        warnings.push(format!(
            "Optional API intelligence artifact missing: {}",
            api_summary_path.display()
        ));
    }

    bundle.warnings = warnings.clone();
    Ok(Some(bundle))
}

fn load_optional_graph_support(
    input_root: &Path,
    warnings: &mut Vec<String>,
) -> Result<GraphSupport> {
    let maps_root = input_root.join("maps");
    if !maps_root.exists() || !maps_root.is_dir() {
        return Ok(GraphSupport::default());
    }

    let summary_path = maps_root.join("graph-summary.json");
    let summary = if summary_path.exists() {
        Some(load_json::<GraphSummary>(&summary_path)?)
    } else {
        warnings.push(format!(
            "Optional graph summary artifact missing: {}",
            summary_path.display()
        ));
        None
    };

    let clusters_path = maps_root.join("clusters.json");
    let clusters = if clusters_path.exists() {
        load_json::<ImportedClustersDocument>(&clusters_path)?.clusters
    } else {
        warnings.push(format!(
            "Optional graph cluster artifact missing: {}",
            clusters_path.display()
        ));
        Vec::new()
    };

    Ok(GraphSupport { summary, clusters })
}

fn build_context_records(
    review: &ReviewBundle,
    enrichment: &EnrichmentBundle,
    api_bundle: Option<&ApiIntelBundle>,
    graph_support: &GraphSupport,
    max_context_chars: usize,
) -> Vec<ContextRecord> {
    let asset_map = enrichment
        .assets
        .iter()
        .cloned()
        .map(|asset| (asset.asset.clone(), asset))
        .collect::<BTreeMap<_, _>>();
    let observation_map = enrichment.observations.iter().cloned().fold(
        BTreeMap::<String, Vec<SemanticObservation>>::new(),
        |mut acc, observation| {
            acc.entry(observation.asset.clone())
                .or_default()
                .push(observation);
            acc
        },
    );
    let risk_map = enrichment
        .risks
        .iter()
        .cloned()
        .map(|risk| (risk.asset.clone(), risk))
        .collect::<BTreeMap<_, _>>();
    let evidence_map = review.evidence_entries.iter().cloned().fold(
        BTreeMap::<String, Vec<EvidenceIndexEntry>>::new(),
        |mut acc, entry| {
            acc.entry(entry.review_item.clone())
                .or_default()
                .push(entry);
            acc
        },
    );

    let mut records = Vec::new();

    for item in &review.items {
        let asset = asset_map
            .get(&item.asset)
            .cloned()
            .unwrap_or_else(|| fallback_asset(item, &risk_map));
        let observations = observation_map
            .get(&item.asset)
            .cloned()
            .unwrap_or_default();
        let evidence_entries =
            dedupe_evidence_entries(evidence_map.get(&item.asset).cloned().unwrap_or_default());
        let api_context = build_effective_api_asset_context(&asset, api_bundle);
        let context_file = format!(
            "asset-contexts/{:03}-{}.json",
            item.rank,
            sanitize_asset_filename_stem(&item.asset)
        );
        let mut context = build_asset_context(
            item,
            &asset,
            &observations,
            &evidence_entries,
            &api_context,
            graph_support,
        );
        apply_token_budget(&mut context, max_context_chars);

        records.push(ContextRecord {
            review_item: item.clone(),
            asset,
            context,
            context_file,
            observation_count: observations.len(),
        });
    }

    records
}

fn build_asset_context(
    review_item: &ReviewItem,
    asset: &EnrichedAsset,
    observations: &[SemanticObservation],
    evidence_entries: &[EvidenceIndexEntry],
    api_context: &ApiAssetContext,
    graph_support: &GraphSupport,
) -> LlmAssetContext {
    let graph_neighborhood_summary =
        build_graph_neighborhood_summary(asset, observations, graph_support);
    let evidence_refs = dedupe_strings(
        review_item
            .evidence_refs
            .iter()
            .cloned()
            .chain(
                evidence_entries
                    .iter()
                    .map(|entry| entry.evidence.evidence_id.clone()),
            )
            .collect(),
    );
    let evidence_highlights = build_evidence_highlights(evidence_entries);
    let api_observations = build_api_observation_lines(api_context);
    let api_object_candidates = build_api_object_lines(api_context);
    let auth_observations = build_auth_observation_lines(api_context);
    let js_observations = build_js_observation_lines(api_context);
    let schema_observations = build_schema_observation_lines(api_context);
    let graphql_observations = build_graphql_observation_lines(api_context);
    let cautious_next_step_questions =
        build_cautious_questions(review_item, asset, api_context, &graph_neighborhood_summary);

    let mut context = LlmAssetContext {
        asset: review_item.asset.clone(),
        risk_level: review_item.risk_level.clone(),
        score: review_item.score,
        confidence: review_item.confidence,
        semantic_roles: if review_item.semantic_roles.is_empty() {
            asset.roles.clone()
        } else {
            review_item.semantic_roles.clone()
        },
        environments: if review_item.environments.is_empty() {
            asset.environments.clone()
        } else {
            review_item.environments.clone()
        },
        graph_neighborhood_summary,
        api_observations,
        api_object_candidates,
        auth_observations,
        js_observations,
        schema_observations,
        graphql_observations,
        evidence_refs,
        evidence_highlights,
        cautious_next_step_questions,
        context_markdown: String::new(),
        estimated_chars: 0,
        truncated: false,
        truncation_notes: Vec::new(),
    };
    refresh_context_markdown(&mut context);
    context
}

fn build_reasoning_queue(records: &[ContextRecord]) -> Vec<LlmReasoningItem> {
    let mut items = records
        .iter()
        .map(|record| {
            let (reasoning_score, why_llm_review) = reasoning_score(record);
            LlmReasoningItem {
                rank: 0,
                asset: record.review_item.asset.clone(),
                review_rank: record.review_item.rank,
                risk_level: record.review_item.risk_level.clone(),
                score: record.review_item.score,
                confidence: record.review_item.confidence,
                reasoning_score,
                suggested_prompt_template: choose_prompt_template(record),
                context_file: record.context_file.clone(),
                why_llm_review,
                evidence_refs: record.context.evidence_refs.clone(),
            }
        })
        .collect::<Vec<_>>();

    items.sort_by(|left, right| {
        right
            .reasoning_score
            .cmp(&left.reasoning_score)
            .then_with(|| left.review_rank.cmp(&right.review_rank))
            .then_with(|| left.asset.cmp(&right.asset))
    });

    items
}

fn re_rank_reasoning_queue(items: &mut [LlmReasoningItem]) {
    for (index, item) in items.iter_mut().enumerate() {
        item.rank = index + 1;
    }
}

fn build_pack_summary(
    records: &[ContextRecord],
    reasoning_queue: &[LlmReasoningItem],
    prompts: &[LlmPromptTemplate],
    max_context_chars: usize,
    api_intel_present: bool,
    graph_summary_present: bool,
    graph_support: &GraphSupport,
    warnings: &[String],
) -> LlmPackSummary {
    let mut theme_counts = BTreeMap::new();
    let mut api_counts = BTreeMap::new();
    let mut unknowns = Vec::new();
    let mut total_evidence_refs = 0usize;
    let mut truncated_context_count = 0usize;

    for record in records {
        total_evidence_refs += record.context.evidence_refs.len();
        if record.context.truncated {
            truncated_context_count += 1;
        }

        if record
            .asset
            .roles
            .iter()
            .any(|role| matches!(role, AssetRole::Authentication | AssetRole::AdminDashboard))
        {
            *theme_counts
                .entry("privileged or authentication surface".to_string())
                .or_insert(0usize) += 1;
        }
        if record.asset.environments.iter().any(|environment| {
            matches!(
                environment,
                EnvironmentType::Internal | EnvironmentType::Staging | EnvironmentType::Legacy
            )
        }) {
            *theme_counts
                .entry("non-production or internal environment hints".to_string())
                .or_insert(0usize) += 1;
        }
        if !record.context.api_observations.is_empty() {
            *api_counts
                .entry("API surface candidates".to_string())
                .or_insert(0usize) += 1;
        }
        if !record.context.auth_observations.is_empty() {
            *api_counts
                .entry("Auth-related application surfaces".to_string())
                .or_insert(0usize) += 1;
        }
        if !record.context.schema_observations.is_empty() {
            *api_counts
                .entry("Schema or documentation exposure candidates".to_string())
                .or_insert(0usize) += 1;
        }
        if !record.context.graphql_observations.is_empty() {
            *api_counts
                .entry("GraphQL-related surfaces".to_string())
                .or_insert(0usize) += 1;
        }
        if !record.context.js_observations.is_empty() {
            *api_counts
                .entry("JavaScript-derived hidden route context".to_string())
                .or_insert(0usize) += 1;
        }
        if record.context.semantic_roles.is_empty()
            || record
                .context
                .semantic_roles
                .iter()
                .all(|role| *role == AssetRole::Unknown)
        {
            unknowns.push(format!(
                "{} has limited role classification and may benefit from evidence-backed model triage.",
                record.review_item.asset
            ));
        }
        if record.context.evidence_refs.len() <= 2 {
            unknowns.push(format!(
                "{} has sparse evidence and may need careful hypothesis generation rather than hard conclusions.",
                record.review_item.asset
            ));
        }
    }

    let top_graph_clusters = if !graph_support.clusters.is_empty() {
        graph_support
            .clusters
            .iter()
            .take(5)
            .map(|cluster| {
                format!(
                    "{} [{}] with {} related nodes",
                    cluster.cluster_id,
                    cluster.cluster_type,
                    cluster.related_nodes.len()
                )
            })
            .collect()
    } else if let Some(summary) = &graph_support.summary {
        if !summary.largest_clusters.is_empty() {
            summary.largest_clusters.clone()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    LlmPackSummary {
        generated_at: Utc::now(),
        asset_context_count: records.len(),
        prompt_template_count: prompts.len(),
        reasoning_item_count: reasoning_queue.len(),
        max_context_chars,
        total_evidence_refs,
        truncated_context_count,
        api_intel_present,
        graph_summary_present,
        top_review_themes: top_counts(theme_counts),
        top_api_auth_areas: top_counts(api_counts),
        top_graph_clusters,
        top_unknowns: dedupe_strings(unknowns).into_iter().take(5).collect(),
        suggested_review_order: reasoning_queue
            .iter()
            .take(5)
            .map(|item| {
                format!(
                    "{} -> {} using {}",
                    item.rank, item.asset, item.suggested_prompt_template
                )
            })
            .collect(),
        warnings: warnings.to_vec(),
    }
}

fn write_outputs(
    out: &Path,
    prompts: &[LlmPromptTemplate],
    records: &mut [ContextRecord],
    reasoning_queue: &[LlmReasoningItem],
    review: &ReviewBundle,
    enrichment: &EnrichmentBundle,
    summary: &LlmPackSummary,
    max_context_chars: usize,
) -> Result<LlmPackOutcome> {
    let asset_contexts_dir = out.join("asset-contexts");
    let prompts_dir = out.join("prompts");
    let reasoning_queue_json_path = out.join("reasoning-queue.json");
    let reasoning_queue_markdown_path = out.join("reasoning-queue.md");
    let analyst_brief_path = out.join("analyst-brief.md");
    let pack_summary_path = out.join("pack-summary.json");

    utils::ensure_directory(&asset_contexts_dir)?;
    utils::ensure_directory(&prompts_dir)?;

    let mut asset_context_files = Vec::new();
    for record in records.iter() {
        let path = out.join(&record.context_file);
        utils::write_json_pretty(&path, &record.context)?;
        asset_context_files.push(record.context_file.clone());
    }

    for prompt in prompts {
        utils::write_string(
            &prompts_dir.join(&prompt.file_name),
            &prompt.template_markdown,
        )?;
    }

    utils::write_json_pretty(
        &reasoning_queue_json_path,
        &ReasoningQueueDocument {
            summary: summary.clone(),
            items: reasoning_queue.to_vec(),
        },
    )?;
    utils::write_string(
        &reasoning_queue_markdown_path,
        &render_reasoning_queue_markdown(reasoning_queue),
    )?;
    utils::write_string(
        &analyst_brief_path,
        &render_analyst_brief(review, enrichment, summary, reasoning_queue),
    )?;
    utils::write_json_pretty(
        &pack_summary_path,
        &LlmContextPack {
            generated_at: Utc::now(),
            max_context_chars,
            asset_context_files,
            prompts: prompts.to_vec(),
            reasoning_queue: reasoning_queue.to_vec(),
            summary: summary.clone(),
        },
    )?;

    Ok(LlmPackOutcome {
        asset_contexts_dir,
        prompts_dir,
        reasoning_queue_json_path,
        reasoning_queue_markdown_path,
        analyst_brief_path,
        pack_summary_path,
        summary: summary.clone(),
    })
}

fn render_reasoning_queue_markdown(items: &[LlmReasoningItem]) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot LLM Reasoning Queue\n\n");
    output.push_str("These entries are local prompt-pack candidates for analyst-controlled model review. They are not vulnerability claims.\n\n");

    if items.is_empty() {
        output.push_str("No reasoning items were generated because the review queue is empty.\n");
        return output;
    }

    for item in items {
        output.push_str(&format!("## {}. `{}`\n\n", item.rank, item.asset));
        output.push_str(&format!(
            "- Review priority: rank {} | risk {} | score {} | confidence {:.2}\n",
            item.review_rank, item.risk_level, item.score, item.confidence
        ));
        output.push_str(&format!(
            "- Why it deserves LLM review: {}\n",
            join_limited(&item.why_llm_review, 3)
        ));
        output.push_str(&format!(
            "- Suggested prompt template: `{}`\n",
            item.suggested_prompt_template
        ));
        output.push_str(&format!("- Context file: `{}`\n\n", item.context_file));
    }

    output
}

fn render_analyst_brief(
    review: &ReviewBundle,
    enrichment: &EnrichmentBundle,
    summary: &LlmPackSummary,
    reasoning_queue: &[LlmReasoningItem],
) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot Analyst Brief\n\n");
    output.push_str("This pack prepares deterministic, local-only context for analyst-controlled model reasoning. It does not execute any model, contact targets, or claim vulnerabilities.\n\n");
    output.push_str(&format!(
        "- Review items: {}\n- Asset contexts: {}\n- Prompt templates: {}\n- Truncated contexts: {}\n\n",
        review.summary.total_assets,
        summary.asset_context_count,
        summary.prompt_template_count,
        summary.truncated_context_count
    ));

    output.push_str("## Top Review Themes\n\n");
    write_markdown_list(&mut output, &summary.top_review_themes);

    output.push_str("\n## Top API/Auth Areas\n\n");
    write_markdown_list(&mut output, &summary.top_api_auth_areas);

    output.push_str("\n## Top Graph Clusters\n\n");
    write_markdown_list(&mut output, &summary.top_graph_clusters);

    output.push_str("\n## Top Unknowns\n\n");
    write_markdown_list(&mut output, &summary.top_unknowns);

    output.push_str("\n## Suggested Order For Model-Assisted Review\n\n");
    if reasoning_queue.is_empty() {
        output.push_str("No review items are currently queued for model-assisted reasoning.\n");
    } else {
        for item in reasoning_queue.iter().take(5) {
            output.push_str(&format!(
                "- `{}` using `{}` because {}\n",
                item.asset,
                item.suggested_prompt_template,
                join_limited(&item.why_llm_review, 2)
            ));
        }
    }

    if let Some(executive_summary) = &review.executive_summary {
        output.push_str("\n## Existing Review Summary Context\n\n");
        output.push_str(&truncate_text(executive_summary, 900));
        output.push('\n');
    }

    output.push_str("\n## Existing Enrichment Summary Context\n\n");
    output.push_str(&truncate_text(&enrichment.summary_markdown, 900));
    output.push('\n');
    output.push_str(
        "\n## Reminder\n\nUse this pack to generate evidence-backed hypotheses, cautious prioritization notes, or concise analyst summaries. Any candidate still requires manual validation before it should influence reporting or testing decisions.\n",
    );
    output
}

fn prompt_templates() -> Vec<LlmPromptTemplate> {
    vec![
        prompt_template(
            "asset_triage_prompt",
            "asset_triage_prompt.md",
            "Prioritize a single asset for careful manual review without suggesting exploitation.",
            &["general asset triage", "mixed graph and semantic context"],
            &[
                "Do not claim a vulnerability.",
                "Use wording such as candidate, interesting, worth review, and requires validation.",
                "Do not suggest destructive testing, credential attacks, or out-of-scope assumptions.",
                "Every conclusion must cite evidence from the provided context.",
            ],
            r#"# Asset Triage Prompt

You are assisting with recon-only analyst triage.

Safety rules:
- Prioritize, do not exploit.
- Produce hypotheses, not confirmed vulnerabilities.
- Use evidence-backed reasoning only.
- Use wording such as candidate, interesting, worth review, potentially sensitive, and requires validation.
- Do not suggest destructive testing, credential attacks, authentication attacks, or out-of-scope assumptions.

Input context:
- Asset context JSON: `{{asset_context_json}}`
- Reasoning queue item JSON: `{{reasoning_item_json}}`

Tasks:
1. Summarize why this asset deserves review first.
2. Identify the two or three strongest evidence-backed hypotheses.
3. Note what remains unknown.
4. Suggest the safest next manual review questions.

Return Markdown with sections:
- Priority Summary
- Evidence-Backed Hypotheses
- Unknowns
- Safe Next Questions
"#,
        ),
        prompt_template(
            "api_surface_reasoning_prompt",
            "api_surface_reasoning_prompt.md",
            "Reason about API surfaces, schema exposure, and object models using deterministic local evidence.",
            &["API endpoints", "schemas", "GraphQL indicators", "sensitive objects"],
            &[
                "Do not call anything a confirmed issue.",
                "Prioritize exposed documentation, schema, object, and route relationships cautiously.",
                "Do not recommend fuzzing, mutation testing, or auth bypass attempts.",
                "Keep outputs grounded in the provided evidence IDs and source artifacts.",
            ],
            r#"# API Surface Reasoning Prompt

You are reviewing API-related recon context only.

Safety rules:
- Do not claim vulnerabilities.
- Do not recommend exploitation, fuzzing, mutation testing, or auth bypass attempts.
- Stay inside the supplied evidence and context files.
- Use requires validation language.

Input context:
- Asset context JSON: `{{asset_context_json}}`
- Optional analyst brief excerpt: `{{analyst_brief_excerpt}}`

Tasks:
1. Identify the most interesting API surface candidates.
2. Explain whether schema or documentation artifacts change prioritization.
3. Highlight object-model evidence that appears potentially sensitive.
4. Suggest safe manual review questions only.

Return Markdown with sections:
- API Surface Priorities
- Schema And Documentation Notes
- Sensitive Object Candidates
- Safe Manual Review Questions
"#,
        ),
        prompt_template(
            "auth_flow_review_prompt",
            "auth_flow_review_prompt.md",
            "Reason about authentication-related observations and trust-boundary questions without testing them.",
            &["auth surfaces", "JWT", "OAuth", "SSO", "session and token indicators"],
            &[
                "Do not suggest credential attacks or authentication testing.",
                "Do not assume a flaw because auth terminology exists.",
                "Focus on evidence-backed trust-boundary questions and manual validation priorities.",
                "Use cautious language throughout.",
            ],
            r#"# Auth Flow Review Prompt

You are assisting with authentication-surface review in a recon-only workflow.

Safety rules:
- Do not suggest credential attacks, password guessing, token replay, or auth bypass testing.
- Do not call anything vulnerable.
- Use candidate and requires validation language.
- Every claim must cite the supplied evidence.

Input context:
- Asset context JSON: `{{asset_context_json}}`
- Reasoning queue item JSON: `{{reasoning_item_json}}`

Tasks:
1. Summarize the observed auth-related surface.
2. Explain why the auth context may matter for prioritization.
3. List the main unknowns that require safe manual review.
4. Suggest non-destructive review questions only.

Return Markdown with sections:
- Auth Surface Summary
- Why It Matters
- Unknowns
- Safe Manual Review Questions
"#,
        ),
        prompt_template(
            "js_intelligence_review_prompt",
            "js_intelligence_review_prompt.md",
            "Review JavaScript-derived routes, feature flags, and hidden functionality hints without assuming reachability.",
            &["JavaScript intelligence", "hidden routes", "feature flags", "frontend/backend relationships"],
            &[
                "Do not treat JS-derived strings as confirmed reachable routes.",
                "Do not recommend active abuse or destructive testing.",
                "Use evidence-backed hypotheses and call out uncertainty clearly.",
                "Keep suggested next steps non-destructive and in-scope.",
            ],
            r#"# JavaScript Intelligence Review Prompt

You are assisting with local JavaScript-derived recon review.

Safety rules:
- Do not assume that hidden routes are reachable.
- Do not suggest exploitation, mutation, or credential attacks.
- Use cautious, evidence-backed language.
- Highlight uncertainty explicitly.

Input context:
- Asset context JSON: `{{asset_context_json}}`

Tasks:
1. Summarize the most interesting JS-derived hidden routes or feature flags.
2. Explain how JS-derived context changes prioritization.
3. Identify which observations remain speculative.
4. Suggest safe manual review questions only.

Return Markdown with sections:
- JS-Derived Highlights
- Prioritization Impact
- Uncertainty Notes
- Safe Manual Review Questions
"#,
        ),
        prompt_template(
            "report_draft_prompt",
            "report_draft_prompt.md",
            "Draft a cautious analyst-facing summary from the local reasoning queue and briefs.",
            &["analyst summary", "executive draft", "queue synthesis"],
            &[
                "Do not turn prioritized candidates into confirmed vulnerabilities.",
                "Do not recommend destructive testing or out-of-scope actions.",
                "Preserve evidence-backed reasoning and requires validation language.",
                "Focus on synthesis and prioritization only.",
            ],
            r#"# Report Draft Prompt

You are drafting an analyst-facing summary from recon prioritization artifacts.

Safety rules:
- Do not claim confirmed vulnerabilities.
- Do not recommend exploitation, credential attacks, or destructive testing.
- Preserve evidence-backed reasoning and requires validation language.
- Keep the output scoped to prioritization and manual review planning.

Input context:
- Analyst brief Markdown: `{{analyst_brief_markdown}}`
- Reasoning queue JSON: `{{reasoning_queue_json}}`
- Optional selected asset contexts: `{{selected_asset_contexts}}`

Tasks:
1. Summarize the top review themes.
2. Explain which assets or clusters deserve attention first.
3. Highlight the main unknowns and why they matter.
4. Draft safe next-step questions for analysts.

Return Markdown with sections:
- Executive Prioritization Summary
- Top Review Targets
- Key Unknowns
- Safe Next Questions
"#,
        ),
    ]
}

fn prompt_template(
    name: &str,
    file_name: &str,
    purpose: &str,
    recommended_for: &[&str],
    safety_constraints: &[&str],
    template_markdown: &str,
) -> LlmPromptTemplate {
    LlmPromptTemplate {
        name: name.to_string(),
        file_name: file_name.to_string(),
        purpose: purpose.to_string(),
        recommended_for: recommended_for
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        safety_constraints: safety_constraints
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        template_markdown: template_markdown.to_string(),
    }
}

fn fallback_asset(
    item: &ReviewItem,
    risk_map: &BTreeMap<String, RiskExplanation>,
) -> EnrichedAsset {
    let risk = risk_map
        .get(&item.asset)
        .cloned()
        .unwrap_or_else(|| RiskExplanation {
            asset: item.asset.clone(),
            risk_level: item.risk_level.clone(),
            score: item.score,
            explanation: "Interesting candidate worth manual review.".to_string(),
            contributing_factors: item.reasons.clone(),
            recommended_next_steps: item.recommended_next_steps.clone(),
        });
    EnrichedAsset {
        asset: item.asset.clone(),
        semantic_tags: Vec::new(),
        roles: item.semantic_roles.clone(),
        environments: item.environments.clone(),
        risk_explanations: vec![risk],
        related_nodes: Vec::new(),
        api_endpoints: Vec::new(),
        api_objects: Vec::new(),
        auth_observations: Vec::new(),
        js_observations: Vec::new(),
        schema_observations: Vec::new(),
        graphql_observations: Vec::new(),
        neighborhood_summary: "No notable graph-neighborhood observations were captured yet."
            .to_string(),
    }
}

fn build_graph_neighborhood_summary(
    asset: &EnrichedAsset,
    observations: &[SemanticObservation],
    graph_support: &GraphSupport,
) -> String {
    let mut summary = asset.neighborhood_summary.clone();
    let cluster_ids = clusters_for_asset(asset, graph_support)
        .into_iter()
        .take(3)
        .collect::<Vec<_>>();
    if !cluster_ids.is_empty() {
        let suffix = format!(" Cluster context: {}.", cluster_ids.join(", "));
        if !summary.contains("Cluster context:") {
            summary.push_str(&suffix);
        }
    }
    if observations.iter().any(|observation| {
        matches!(
            observation.observation_type.as_str(),
            "neighborhood" | "cluster" | "operational-tooling"
        )
    }) && !summary.contains("semantic observations")
    {
        summary.push_str(" Related semantic observations reinforce the surrounding graph context.");
    }
    summary
}

fn clusters_for_asset(asset: &EnrichedAsset, graph_support: &GraphSupport) -> Vec<String> {
    let related = asset.related_nodes.iter().cloned().collect::<BTreeSet<_>>();
    graph_support
        .clusters
        .iter()
        .filter(|cluster| {
            cluster
                .related_nodes
                .iter()
                .any(|node| related.contains(node))
        })
        .map(|cluster| format!("{} [{}]", cluster.cluster_id, cluster.cluster_type))
        .collect()
}

fn build_evidence_highlights(entries: &[EvidenceIndexEntry]) -> Vec<String> {
    let values = entries
        .iter()
        .map(|entry| {
            format!(
                "[{}] {} :: {}",
                entry.evidence.evidence_id, entry.evidence.source, entry.evidence.description
            )
        })
        .collect::<Vec<_>>();
    dedupe_strings(values)
}

fn build_api_observation_lines(context: &ApiAssetContext) -> Vec<String> {
    dedupe_strings(
        context
            .endpoints
            .iter()
            .map(|endpoint| {
                let tags = endpoint
                    .semantic_tags
                    .iter()
                    .map(|tag| tag.tag.clone())
                    .collect::<Vec<_>>();
                format!(
                    "{} {} [{}] tags={} auth_indicators={}",
                    endpoint.method,
                    endpoint.path,
                    endpoint.normalized_path,
                    display_or_none(&tags),
                    display_or_none(&endpoint.auth_indicators)
                )
            })
            .collect(),
    )
}

fn build_api_object_lines(context: &ApiAssetContext) -> Vec<String> {
    dedupe_strings(
        context
            .objects
            .iter()
            .map(|object| {
                format!(
                    "{} [{} sensitivity] endpoints={}",
                    object.object_name,
                    object.inferred_sensitivity,
                    display_or_none(&object.related_endpoints)
                )
            })
            .collect(),
    )
}

fn build_auth_observation_lines(context: &ApiAssetContext) -> Vec<String> {
    dedupe_strings(
        context
            .auth_observations
            .iter()
            .map(|observation| {
                format!(
                    "{} indicators={} confidence={:.2}",
                    observation.auth_type,
                    display_or_none(&observation.indicators),
                    observation.confidence
                )
            })
            .collect(),
    )
}

fn build_js_observation_lines(context: &ApiAssetContext) -> Vec<String> {
    dedupe_strings(
        context
            .js_observations
            .iter()
            .map(|observation| {
                format!(
                    "{} endpoints={} feature_flags={} roles={}",
                    observation.js_file,
                    observation.discovered_endpoints.len(),
                    display_or_none(&observation.discovered_feature_flags),
                    display_or_none(&observation.discovered_roles)
                )
            })
            .collect(),
    )
}

fn build_schema_observation_lines(context: &ApiAssetContext) -> Vec<String> {
    dedupe_strings(
        context
            .schemas
            .iter()
            .map(|schema| {
                format!(
                    "{} [{}] auth_methods={} objects={}",
                    schema.schema_location,
                    schema.schema_type,
                    display_or_none(&schema.auth_methods),
                    display_or_none(&schema.objects)
                )
            })
            .collect(),
    )
}

fn build_graphql_observation_lines(context: &ApiAssetContext) -> Vec<String> {
    dedupe_strings(
        context
            .graphql_observations
            .iter()
            .map(|observation| {
                format!(
                    "{} indicators={} auth={}",
                    observation.endpoint,
                    display_or_none(&observation.schema_indicators),
                    display_or_none(&observation.auth_indicators)
                )
            })
            .collect(),
    )
}

fn build_cautious_questions(
    review_item: &ReviewItem,
    asset: &EnrichedAsset,
    api_context: &ApiAssetContext,
    graph_neighborhood_summary: &str,
) -> Vec<String> {
    let mut questions = Vec::new();

    questions.push(
        "Which evidence-backed candidate from this context is worth validating first, and what safe manual check would clarify it?".to_string(),
    );
    if asset
        .roles
        .iter()
        .any(|role| matches!(role, AssetRole::Authentication | AssetRole::AdminDashboard))
        || !api_context.auth_observations.is_empty()
    {
        questions.push(
            "Does the observed auth-related or privileged surface appear intentionally exposed within the expected trust boundary?".to_string(),
        );
    }
    if !api_context.schemas.is_empty() {
        questions.push(
            "Do the local schema or documentation artifacts describe potentially sensitive workflows, objects, or auth methods that are worth manual review?".to_string(),
        );
    }
    if !api_context.graphql_observations.is_empty() {
        questions.push(
            "Do the GraphQL indicators suggest an application surface that deserves careful manual mapping before any testing decisions are made?".to_string(),
        );
    }
    if !api_context.js_observations.is_empty() {
        questions.push(
            "Do JavaScript-derived hidden routes or feature flags imply additional internal or privileged functionality that requires manual validation?".to_string(),
        );
    }
    if !api_context.objects.is_empty() {
        questions.push(
            "Do the inferred object models point to potentially sensitive business entities such as users, accounts, organizations, billing, or payments?".to_string(),
        );
    }
    if !graph_neighborhood_summary.starts_with("No notable graph-neighborhood") {
        questions.push(
            "How does the surrounding graph neighborhood change the priority or interpretation of this asset?".to_string(),
        );
    }
    for step in review_item.recommended_next_steps.iter().take(2) {
        questions.push(question_from_step(step));
    }

    dedupe_strings(questions).into_iter().take(5).collect()
}

fn question_from_step(step: &str) -> String {
    let lowered = step.to_ascii_lowercase();
    if lowered.contains("documentation") || lowered.contains("schema") {
        return "Does the available documentation or schema appear intentionally exposed, and what evidence supports that interpretation?".to_string();
    }
    if lowered.contains("auth") {
        return "What auth-related unknowns remain, and which safe manual review question would resolve them without attempting authentication attacks?".to_string();
    }
    if lowered.contains("graphql") {
        return "Which GraphQL-related indicators are strongest, and what still requires manual validation?".to_string();
    }
    if lowered.contains("javascript") || lowered.contains("hidden routes") {
        return "Which JavaScript-derived routes look most interesting, and what is the safest way to validate their relevance later?".to_string();
    }
    format!(
        "How should an analyst validate the following step safely and cautiously: {}?",
        step.trim_end_matches('.')
    )
}

fn apply_token_budget(context: &mut LlmAssetContext, max_context_chars: usize) {
    refresh_context_markdown(context);
    if context.context_markdown.len() <= max_context_chars {
        return;
    }

    context.truncated = true;
    context.truncation_notes.push(format!(
        "The rendered context exceeded the configured max_context_chars budget of {max_context_chars} characters."
    ));

    clamp_list_entries(&mut context.evidence_highlights, 8, 240);
    clamp_list_entries(&mut context.api_observations, 8, 220);
    clamp_list_entries(&mut context.api_object_candidates, 6, 180);
    clamp_list_entries(&mut context.auth_observations, 6, 180);
    clamp_list_entries(&mut context.js_observations, 6, 220);
    clamp_list_entries(&mut context.schema_observations, 6, 220);
    clamp_list_entries(&mut context.graphql_observations, 6, 220);
    clamp_list_entries(&mut context.cautious_next_step_questions, 5, 220);
    context.graph_neighborhood_summary = truncate_text(&context.graph_neighborhood_summary, 700);
    refresh_context_markdown(context);

    while context.context_markdown.len() > max_context_chars
        && context.evidence_highlights.len() > 3
    {
        context.evidence_highlights.pop();
        refresh_context_markdown(context);
    }
    while context.context_markdown.len() > max_context_chars && context.api_observations.len() > 3 {
        context.api_observations.pop();
        refresh_context_markdown(context);
    }
    while context.context_markdown.len() > max_context_chars && context.js_observations.len() > 2 {
        context.js_observations.pop();
        refresh_context_markdown(context);
    }
    while context.context_markdown.len() > max_context_chars
        && context.schema_observations.len() > 2
    {
        context.schema_observations.pop();
        refresh_context_markdown(context);
    }
    while context.context_markdown.len() > max_context_chars
        && context.graphql_observations.len() > 2
    {
        context.graphql_observations.pop();
        refresh_context_markdown(context);
    }

    if context.context_markdown.len() > max_context_chars {
        context.context_markdown = truncate_text(&context.context_markdown, max_context_chars);
        context.truncation_notes.push(
            "Rendered context body was clipped to the configured budget while preserving evidence_refs separately.".to_string(),
        );
    }
    context.estimated_chars = context.context_markdown.len();
}

fn refresh_context_markdown(context: &mut LlmAssetContext) {
    context.context_markdown = render_asset_context_markdown(context);
    context.estimated_chars = context.context_markdown.len();
}

fn render_asset_context_markdown(context: &LlmAssetContext) -> String {
    let mut output = String::new();
    output.push_str(&format!("# Asset Context: `{}`\n\n", context.asset));
    output.push_str(&format!(
        "- Risk level: {}\n- Score: {}\n- Confidence: {:.2}\n- Roles: {}\n- Environments: {}\n\n",
        context.risk_level,
        context.score,
        context.confidence,
        display_roles(&context.semantic_roles),
        display_environments(&context.environments)
    ));

    output.push_str("## Graph Neighborhood Summary\n\n");
    output.push_str(&format!("{}\n\n", context.graph_neighborhood_summary));

    output.push_str("## API Observations\n\n");
    write_markdown_list(&mut output, &context.api_observations);

    output.push_str("\n## API Object Candidates\n\n");
    write_markdown_list(&mut output, &context.api_object_candidates);

    output.push_str("\n## Auth Observations\n\n");
    write_markdown_list(&mut output, &context.auth_observations);

    output.push_str("\n## JavaScript Observations\n\n");
    write_markdown_list(&mut output, &context.js_observations);

    output.push_str("\n## Schema Observations\n\n");
    write_markdown_list(&mut output, &context.schema_observations);

    output.push_str("\n## GraphQL Observations\n\n");
    write_markdown_list(&mut output, &context.graphql_observations);

    output.push_str("\n## Evidence Highlights\n\n");
    write_markdown_list(&mut output, &context.evidence_highlights);

    output.push_str("\n## Evidence References\n\n");
    write_markdown_list(&mut output, &context.evidence_refs);

    output.push_str("\n## Cautious Next-Step Questions\n\n");
    write_markdown_list(&mut output, &context.cautious_next_step_questions);

    if context.truncated && !context.truncation_notes.is_empty() {
        output.push_str("\n## Truncation Notes\n\n");
        write_markdown_list(&mut output, &context.truncation_notes);
    }

    output
}

fn reasoning_score(record: &ContextRecord) -> (i32, Vec<String>) {
    let mut total = record.review_item.score;
    let mut reasons = vec![format!(
        "Review score {} and review rank {} already indicate analyst interest.",
        record.review_item.score, record.review_item.rank
    )];

    let review_rank_boost = (25_i32 - (record.review_item.rank as i32 * 2)).max(0);
    total += review_rank_boost;
    if review_rank_boost > 0 {
        reasons.push(
            "Earlier review-queue placement increases model-assisted triage value.".to_string(),
        );
    }

    let semantic_richness = ((record.context.semantic_roles.len() as i32) * 4)
        + ((record.context.environments.len() as i32) * 3)
        + (record.asset.semantic_tags.len().min(8) as i32)
        + (record.observation_count.min(8) as i32);
    total += semantic_richness;
    if semantic_richness > 8 {
        reasons.push(
            "Richer semantic context gives a local model more structured evidence to reason over."
                .to_string(),
        );
    }

    let api_auth_relevance = ((record.context.api_observations.len().min(5) as i32) * 3)
        + ((record.context.auth_observations.len().min(4) as i32) * 5)
        + ((record.context.schema_observations.len().min(4) as i32) * 4)
        + ((record.context.graphql_observations.len().min(3) as i32) * 4)
        + ((record.context.api_object_candidates.len().min(4) as i32) * 3);
    total += api_auth_relevance;
    if api_auth_relevance > 0 {
        reasons.push("API, auth, schema, GraphQL, or object-model evidence makes this a strong reasoning candidate.".to_string());
    }

    let graph_complexity = if record
        .context
        .graph_neighborhood_summary
        .starts_with("No notable graph-neighborhood")
    {
        0
    } else {
        8 + record.asset.related_nodes.len().min(6) as i32
    };
    total += graph_complexity;
    if graph_complexity > 0 {
        reasons.push("Graph-neighborhood complexity adds relationship context that benefits hypothesis generation.".to_string());
    }

    let evidence_count_boost = record.context.evidence_refs.len().min(12) as i32;
    total += evidence_count_boost;
    if evidence_count_boost >= 4 {
        reasons.push(
            "Multiple evidence references make it easier to demand evidence-backed model output."
                .to_string(),
        );
    }

    if record.context.truncated {
        reasons.push(
            "The context was compacted to fit the local token-budget style limit.".to_string(),
        );
    }

    reasons.extend(record.review_item.reasons.iter().take(2).cloned());
    (
        total.clamp(0, 200),
        dedupe_strings(reasons).into_iter().take(5).collect(),
    )
}

fn choose_prompt_template(record: &ContextRecord) -> String {
    if !record.context.auth_observations.is_empty() {
        "auth_flow_review_prompt.md".to_string()
    } else if !record.context.api_observations.is_empty()
        || !record.context.schema_observations.is_empty()
        || !record.context.graphql_observations.is_empty()
        || !record.context.api_object_candidates.is_empty()
    {
        "api_surface_reasoning_prompt.md".to_string()
    } else if !record.context.js_observations.is_empty() {
        "js_intelligence_review_prompt.md".to_string()
    } else {
        "asset_triage_prompt.md".to_string()
    }
}

fn dedupe_evidence_entries(entries: Vec<EvidenceIndexEntry>) -> Vec<EvidenceIndexEntry> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for entry in entries {
        let key = (
            entry.review_item.clone(),
            entry.review_rank,
            entry.evidence.evidence_id.clone(),
            entry.evidence.source.clone(),
            entry.evidence.description.clone(),
        );
        if seen.insert(key) {
            deduped.push(entry);
        }
    }
    deduped
}

fn build_effective_api_asset_context(
    asset: &EnrichedAsset,
    api_bundle: Option<&ApiIntelBundle>,
) -> ApiAssetContext {
    let Some(api_bundle) = api_bundle else {
        return ApiAssetContext {
            endpoints: asset.api_endpoints.clone(),
            objects: asset.api_objects.clone(),
            auth_observations: asset.auth_observations.clone(),
            js_observations: asset.js_observations.clone(),
            schemas: asset.schema_observations.clone(),
            graphql_observations: asset.graphql_observations.clone(),
        };
    };

    let fallback = build_api_asset_context(asset, api_bundle);
    let use_enriched = !asset.api_endpoints.is_empty()
        || !asset.api_objects.is_empty()
        || !asset.auth_observations.is_empty()
        || !asset.js_observations.is_empty()
        || !asset.schema_observations.is_empty()
        || !asset.graphql_observations.is_empty();

    if !use_enriched {
        return fallback;
    }

    ApiAssetContext {
        endpoints: if asset.api_endpoints.is_empty() {
            fallback.endpoints
        } else {
            asset.api_endpoints.clone()
        },
        objects: if asset.api_objects.is_empty() {
            fallback.objects
        } else {
            asset.api_objects.clone()
        },
        auth_observations: if asset.auth_observations.is_empty() {
            fallback.auth_observations
        } else {
            asset.auth_observations.clone()
        },
        js_observations: if asset.js_observations.is_empty() {
            fallback.js_observations
        } else {
            asset.js_observations.clone()
        },
        schemas: if asset.schema_observations.is_empty() {
            fallback.schemas
        } else {
            asset.schema_observations.clone()
        },
        graphql_observations: if asset.graphql_observations.is_empty() {
            fallback.graphql_observations
        } else {
            asset.graphql_observations.clone()
        },
    }
}

fn build_api_asset_context(asset: &EnrichedAsset, api_bundle: &ApiIntelBundle) -> ApiAssetContext {
    let endpoints = api_bundle
        .endpoints
        .iter()
        .filter(|endpoint| endpoint_matches_asset(&asset.asset, endpoint))
        .cloned()
        .collect::<Vec<_>>();
    let normalized_paths = endpoints
        .iter()
        .map(|endpoint| endpoint.normalized_path.clone())
        .collect::<BTreeSet<_>>();

    let mut objects = api_bundle
        .objects
        .iter()
        .filter(|object| {
            object
                .related_endpoints
                .iter()
                .any(|path| normalized_paths.contains(path))
        })
        .cloned()
        .collect::<Vec<_>>();

    let related_object_names = api_bundle
        .relationships
        .iter()
        .filter(|relationship| {
            endpoints
                .iter()
                .any(|endpoint| endpoint.endpoint_id == relationship.source_endpoint)
        })
        .filter(|relationship| relationship.relationship_type == "returns_object")
        .map(|relationship| relationship.target_object.clone())
        .collect::<BTreeSet<_>>();
    for object in &api_bundle.objects {
        if related_object_names.contains(&object.object_name)
            && !objects
                .iter()
                .any(|existing| existing.object_name == object.object_name)
        {
            objects.push(object.clone());
        }
    }
    objects.sort_by(|left, right| left.object_name.cmp(&right.object_name));

    let schemas = api_bundle
        .schemas
        .iter()
        .filter(|schema| {
            schema_matches_asset(&asset.asset, schema)
                || schema
                    .endpoints
                    .iter()
                    .any(|path| normalized_paths.contains(&normalize_schema_endpoint(path)))
        })
        .cloned()
        .collect::<Vec<_>>();
    let auth_observations = api_bundle
        .auth_observations
        .iter()
        .filter(|observation| auth_observation_matches_asset(&asset.asset, observation, &endpoints))
        .cloned()
        .collect::<Vec<_>>();
    let js_observations = api_bundle
        .js_observations
        .iter()
        .filter(|observation| js_observation_matches_asset(&asset.asset, observation, &endpoints))
        .cloned()
        .collect::<Vec<_>>();
    let graphql_observations = api_bundle
        .graphql_observations
        .iter()
        .filter(|observation| graphql_matches_asset(&asset.asset, observation, &endpoints))
        .cloned()
        .collect::<Vec<_>>();

    ApiAssetContext {
        endpoints,
        objects,
        auth_observations,
        js_observations,
        schemas,
        graphql_observations,
    }
}

fn endpoint_matches_asset(asset: &str, endpoint: &ApiEndpoint) -> bool {
    let asset_lc = asset.to_ascii_lowercase();
    endpoint.path.to_ascii_lowercase().contains(&asset_lc)
        || extract_host_from_string(&endpoint.path)
            .map(|host| host.eq_ignore_ascii_case(asset))
            .unwrap_or(false)
        || endpoint.normalized_path == asset
}

fn schema_matches_asset(asset: &str, schema: &ApiSchema) -> bool {
    schema
        .schema_location
        .to_ascii_lowercase()
        .contains(&asset.to_ascii_lowercase())
        || extract_host_from_string(&schema.schema_location)
            .map(|host| host.eq_ignore_ascii_case(asset))
            .unwrap_or(false)
}

fn auth_observation_matches_asset(
    asset: &str,
    observation: &AuthObservation,
    endpoints: &[ApiEndpoint],
) -> bool {
    observation.asset.eq_ignore_ascii_case(asset)
        || extract_host_from_string(&observation.asset)
            .map(|host| host.eq_ignore_ascii_case(asset))
            .unwrap_or(false)
        || endpoints.iter().any(|endpoint| {
            extract_host_from_string(&endpoint.path)
                .map(|host| host.eq_ignore_ascii_case(&observation.asset))
                .unwrap_or(false)
        })
}

fn js_observation_matches_asset(
    asset: &str,
    observation: &JsObservation,
    endpoints: &[ApiEndpoint],
) -> bool {
    observation.discovered_endpoints.iter().any(|endpoint| {
        endpoint
            .to_ascii_lowercase()
            .contains(&asset.to_ascii_lowercase())
            || extract_host_from_string(endpoint)
                .map(|host| host.eq_ignore_ascii_case(asset))
                .unwrap_or(false)
    }) || observation.evidence.iter().any(|evidence| {
        evidence
            .to_ascii_lowercase()
            .contains(&asset.to_ascii_lowercase())
    }) || endpoints.iter().any(|endpoint| {
        observation
            .discovered_endpoints
            .iter()
            .any(|candidate| normalize_schema_endpoint(candidate) == endpoint.normalized_path)
    })
}

fn graphql_matches_asset(
    asset: &str,
    observation: &GraphQlObservation,
    endpoints: &[ApiEndpoint],
) -> bool {
    extract_host_from_string(&observation.endpoint)
        .map(|host| host.eq_ignore_ascii_case(asset))
        .unwrap_or(false)
        || observation
            .endpoint
            .to_ascii_lowercase()
            .contains(&asset.to_ascii_lowercase())
        || endpoints.iter().any(|endpoint| {
            endpoint.normalized_path == normalize_schema_endpoint(&observation.endpoint)
        })
}

fn normalize_schema_endpoint(value: &str) -> String {
    let trimmed = value.trim();
    let candidate = trimmed.split_whitespace().nth(1).unwrap_or(trimmed);
    let path = if candidate.starts_with("http://") || candidate.starts_with("https://") {
        Url::parse(candidate)
            .ok()
            .map(|parsed| parsed.path().to_string())
            .unwrap_or_else(|| candidate.to_string())
    } else {
        candidate.to_string()
    };
    let segments = path
        .split('?')
        .next()
        .unwrap_or_default()
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            if segment.starts_with('{') && segment.ends_with('}') {
                "{id}".to_string()
            } else if segment.starts_with(':')
                || segment.chars().all(|character| character.is_ascii_digit())
            {
                "{id}".to_string()
            } else {
                segment.to_ascii_lowercase()
            }
        })
        .collect::<Vec<_>>();
    if segments.is_empty() {
        String::new()
    } else {
        format!("/{}", segments.join("/"))
    }
}

fn extract_host_from_string(value: &str) -> Option<String> {
    if value.starts_with("http://") || value.starts_with("https://") {
        return Url::parse(value)
            .ok()
            .and_then(|parsed| parsed.host_str().map(ToOwned::to_owned));
    }
    None
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read JSON input: {}", path.display()))?;
    serde_json::from_str::<T>(&raw)
        .with_context(|| format!("failed to parse JSON input: {}", path.display()))
}

fn load_optional_json_with_warning<T: serde::de::DeserializeOwned + Default>(
    root: &Path,
    file_name: &str,
    warnings: &mut Vec<String>,
    label: &str,
) -> Result<T> {
    let path = root.join(file_name);
    if !path.exists() {
        warnings.push(format!("Optional {label} missing: {}", path.display()));
        return Ok(T::default());
    }
    load_json(&path)
}

fn display_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn display_roles(values: &[AssetRole]) -> String {
    let rendered = values
        .iter()
        .filter(|value| **value != AssetRole::Unknown)
        .map(|value| value.as_str().to_string())
        .collect::<Vec<_>>();
    if rendered.is_empty() {
        "unknown".to_string()
    } else {
        rendered.join(", ")
    }
}

fn display_environments(values: &[EnvironmentType]) -> String {
    let rendered = values
        .iter()
        .filter(|value| **value != EnvironmentType::Unknown)
        .map(|value| value.as_str().to_string())
        .collect::<Vec<_>>();
    if rendered.is_empty() {
        "unknown".to_string()
    } else {
        rendered.join(", ")
    }
}

fn write_markdown_list(output: &mut String, values: &[String]) {
    if values.is_empty() {
        output.push_str("None observed yet.\n");
        return;
    }

    for value in values {
        output.push_str(&format!("- {}\n", value));
    }
}

fn top_counts(counts: BTreeMap<String, usize>) -> Vec<String> {
    let mut values = counts.into_iter().collect::<Vec<_>>();
    values.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    values
        .into_iter()
        .take(5)
        .map(|(label, count)| format!("{label} ({count})"))
        .collect()
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

fn sanitize_asset_filename_stem(asset: &str) -> String {
    let mut value = asset.to_ascii_lowercase();
    for prefix in ["https://", "http://"] {
        if value.starts_with(prefix) {
            value = value.trim_start_matches(prefix).to_string();
        }
    }
    let mut sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while sanitized.contains("--") {
        sanitized = sanitized.replace("--", "-");
    }
    sanitized = sanitized.trim_matches('-').to_string();
    if sanitized.is_empty() {
        "asset".to_string()
    } else {
        sanitized
    }
}

fn clamp_list_entries(values: &mut Vec<String>, max_entries: usize, max_len: usize) {
    for value in values.iter_mut() {
        *value = truncate_text(value, max_len);
    }
    if values.len() > max_entries {
        values.truncate(max_entries);
    }
}

fn truncate_text(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }

    let truncated = value
        .chars()
        .take(max_len.saturating_sub(3))
        .collect::<String>();
    format!("{truncated}...")
}

fn join_limited(values: &[String], limit: usize) -> String {
    if values.is_empty() {
        return "none".to_string();
    }
    if values.len() <= limit {
        return values.join("; ");
    }
    let mut preview = values.iter().take(limit).cloned().collect::<Vec<_>>();
    preview.push(format!("and {} more", values.len() - limit));
    preview.join("; ")
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

    use super::run_llm_pack;

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
                "reconpilot-llm-pack-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            Ok(Self { root })
        }

        fn output_root(&self) -> PathBuf {
            self.root.join("output")
        }

        fn llm_pack_dir(&self) -> PathBuf {
            self.output_root().join("llm-pack")
        }

        fn write_file(&self, relative: &str, content: &str) -> Result<()> {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, content)?;
            Ok(())
        }

        fn seed_required_inputs(&self) -> Result<()> {
            self.write_file(
                "output/enrichment/semantic-assets.json",
                r#"[{"asset":"auth.example.com","semantic_tags":[{"tag":"auth_surface","category":"role","confidence":0.95,"evidence":["Matched auth indicators"]}],"roles":["authentication","api_gateway"],"environments":["staging"],"risk_explanations":[{"asset":"auth.example.com","risk_level":"high","score":82,"explanation":"Interesting candidate worth manual review.","contributing_factors":["Auth-related API surface indicators","GraphQL surface candidate"],"recommended_next_steps":["Review auth-related routes carefully.","Review GraphQL-related artifacts manually."]}],"related_nodes":["host:auth","url:graphql"],"api_endpoints":[{"endpoint_id":"endpoint:get:/graphql","method":"GET","path":"https://auth.example.com/graphql","normalized_path":"/graphql","parameters":["user_id"],"auth_indicators":["bearer_token","jwt"],"inferred_objects":["User"],"semantic_tags":[{"tag":"graphql_surface","category":"endpoint_intent","confidence":0.91,"evidence":["Matched '/graphql'"]}],"source":"api-intel"}],"api_objects":[{"object_name":"User","related_endpoints":["/users/{id}"],"related_parameters":["user_id"],"inferred_sensitivity":"high"}],"auth_observations":[{"asset":"auth.example.com","auth_type":"jwt_bearer","indicators":["bearer_token","jwt"],"confidence":0.91,"evidence":["Bearer references in local API artifacts"]}],"js_observations":[{"js_file":"output/js/app.js","discovered_endpoints":["https://auth.example.com/internal/graphql"],"discovered_roles":["api_gateway"],"discovered_auth_indicators":["jwt"],"discovered_feature_flags":["betaAdmin"],"evidence":["JS referenced internal GraphQL route"]}],"schema_observations":[{"schema_type":"openapi","schema_location":"https://auth.example.com/swagger.json","detected_version":"3.0.2","endpoints":["GET /users/{id}"],"auth_methods":["bearerAuth"],"objects":["User"]}],"graphql_observations":[{"endpoint":"https://auth.example.com/graphql","introspection_detected":false,"schema_indicators":["apollo"],"auth_indicators":["jwt"],"notes":["GraphQL candidate from local JS"]}],"neighborhood_summary":"This asset shares infrastructure with related hosts and belongs to a privileged cluster."},{"asset":"www.example.com","semantic_tags":[],"roles":["customer_app"],"environments":["production"],"risk_explanations":[{"asset":"www.example.com","risk_level":"medium","score":40,"explanation":"Interesting candidate worth manual review.","contributing_factors":["JavaScript-derived routes add hidden application context"],"recommended_next_steps":["Compare JavaScript-derived hidden routes with the visible application flow."]}],"related_nodes":["host:www"],"api_endpoints":[],"api_objects":[],"auth_observations":[],"js_observations":[],"schema_observations":[],"graphql_observations":[],"neighborhood_summary":"No notable graph-neighborhood observations were captured yet."}]"#,
            )?;
            self.write_file(
                "output/enrichment/semantic-observations.json",
                r#"[{"observation_id":"obs:auth","asset":"auth.example.com","observation_type":"auth-surface","description":"Auth-related API surface candidate","evidence":["Bearer token reference"],"confidence":0.9,"related_nodes":["host:auth"]},{"observation_id":"obs:js","asset":"www.example.com","observation_type":"javascript","description":"JavaScript discovered hidden route candidate","evidence":["JS referenced route"],"confidence":0.7,"related_nodes":["host:www"]}]"#,
            )?;
            self.write_file(
                "output/enrichment/risk-explanations.json",
                r#"[{"asset":"auth.example.com","risk_level":"high","score":82,"explanation":"Interesting candidate worth manual review.","contributing_factors":["Auth-related API surface indicators","GraphQL surface candidate"],"recommended_next_steps":["Review auth-related routes carefully.","Review GraphQL-related artifacts manually."]},{"asset":"www.example.com","risk_level":"medium","score":40,"explanation":"Interesting candidate worth manual review.","contributing_factors":["JavaScript-derived routes add hidden application context"],"recommended_next_steps":["Compare JavaScript-derived hidden routes with the visible application flow."]}]"#,
            )?;
            self.write_file(
                "output/enrichment/enriched-graph.json",
                r#"{"assets":[{"asset":"auth.example.com","semantic_tags":[{"tag":"auth_surface","category":"role","confidence":0.95,"evidence":["Matched auth indicators"]}],"roles":["authentication","api_gateway"],"environments":["staging"],"risk_explanations":[{"asset":"auth.example.com","risk_level":"high","score":82,"explanation":"Interesting candidate worth manual review.","contributing_factors":["Auth-related API surface indicators","GraphQL surface candidate"],"recommended_next_steps":["Review auth-related routes carefully.","Review GraphQL-related artifacts manually."]}],"related_nodes":["host:auth","url:graphql"],"api_endpoints":[{"endpoint_id":"endpoint:get:/graphql","method":"GET","path":"https://auth.example.com/graphql","normalized_path":"/graphql","parameters":["user_id"],"auth_indicators":["bearer_token","jwt"],"inferred_objects":["User"],"semantic_tags":[{"tag":"graphql_surface","category":"endpoint_intent","confidence":0.91,"evidence":["Matched '/graphql'"]}],"source":"api-intel"}],"api_objects":[{"object_name":"User","related_endpoints":["/users/{id}"],"related_parameters":["user_id"],"inferred_sensitivity":"high"}],"auth_observations":[{"asset":"auth.example.com","auth_type":"jwt_bearer","indicators":["bearer_token","jwt"],"confidence":0.91,"evidence":["Bearer references in local API artifacts"]}],"js_observations":[{"js_file":"output/js/app.js","discovered_endpoints":["https://auth.example.com/internal/graphql"],"discovered_roles":["api_gateway"],"discovered_auth_indicators":["jwt"],"discovered_feature_flags":["betaAdmin"],"evidence":["JS referenced internal GraphQL route"]}],"schema_observations":[{"schema_type":"openapi","schema_location":"https://auth.example.com/swagger.json","detected_version":"3.0.2","endpoints":["GET /users/{id}"],"auth_methods":["bearerAuth"],"objects":["User"]}],"graphql_observations":[{"endpoint":"https://auth.example.com/graphql","introspection_detected":false,"schema_indicators":["apollo"],"auth_indicators":["jwt"],"notes":["GraphQL candidate from local JS"]}],"neighborhood_summary":"This asset shares infrastructure with related hosts and belongs to a privileged cluster."},{"asset":"www.example.com","semantic_tags":[],"roles":["customer_app"],"environments":["production"],"risk_explanations":[{"asset":"www.example.com","risk_level":"medium","score":40,"explanation":"Interesting candidate worth manual review.","contributing_factors":["JavaScript-derived routes add hidden application context"],"recommended_next_steps":["Compare JavaScript-derived hidden routes with the visible application flow."]}],"related_nodes":["host:www"],"api_endpoints":[],"api_objects":[],"auth_observations":[],"js_observations":[],"schema_observations":[],"graphql_observations":[],"neighborhood_summary":"No notable graph-neighborhood observations were captured yet."}],"observations":[{"observation_id":"obs:auth","asset":"auth.example.com","observation_type":"auth-surface","description":"Auth-related API surface candidate","evidence":["Bearer token reference"],"confidence":0.9,"related_nodes":["host:auth"]},{"observation_id":"obs:js","asset":"www.example.com","observation_type":"javascript","description":"JavaScript discovered hidden route candidate","evidence":["JS referenced route"],"confidence":0.7,"related_nodes":["host:www"]}],"risk_explanations":[{"asset":"auth.example.com","risk_level":"high","score":82,"explanation":"Interesting candidate worth manual review.","contributing_factors":["Auth-related API surface indicators","GraphQL surface candidate"],"recommended_next_steps":["Review auth-related routes carefully.","Review GraphQL-related artifacts manually."]},{"asset":"www.example.com","risk_level":"medium","score":40,"explanation":"Interesting candidate worth manual review.","contributing_factors":["JavaScript-derived routes add hidden application context"],"recommended_next_steps":["Compare JavaScript-derived hidden routes with the visible application flow."]}],"original_graph_summary":{"generated_at":"2026-05-14T09:00:00Z","node_count":4,"edge_count":3,"cluster_count":1,"anomaly_count":1,"top_technologies":["GraphQL (1)"],"largest_clusters":["cluster:auth-admin [privileged-surface]"],"shared_infrastructure":["auth.example.com <-> admin.example.com"],"suspicious_naming":["auth.example.com"],"likely_staging_systems":["auth.example.com"],"likely_internal_systems":[],"redirect_chain_count":0},"original_nodes":[{"id":"host:auth","node_type":"host","value":"auth.example.com","tags":[],"metadata":{},"source_tools":["httpx"],"timestamps":["2026-05-14T09:00:00Z"]}],"original_edges":[],"original_clusters":[{"cluster_id":"cluster:auth-admin","cluster_type":"privileged-surface","related_nodes":["host:auth"],"shared_indicators":["auth"],"risk_score":78}],"semantic_summary":{"generated_at":"2026-05-14T09:00:00Z","asset_count":2,"observation_count":2,"risk_explanation_count":2,"api_endpoint_count":1,"auth_surface_count":1,"js_observation_count":1,"schema_observation_count":1,"graphql_observation_count":1,"top_roles":["authentication (1)"],"top_environments":["staging (1)"],"highest_priority_assets":["auth.example.com [high:82]"],"notable_neighborhood_observations":["Shares infrastructure with privileged/admin-like asset"],"sensitive_object_candidates":["auth.example.com -> User (high)"],"api_intelligence_warnings":[],"recommended_next_steps":["Review auth-related routes carefully."]}}"#,
            )?;
            self.write_file(
                "output/enrichment/enrichment-summary.md",
                "# ReconPilot Semantic Enrichment Summary\n\n- Assets: 2\n- Observations: 2\n",
            )?;
            self.write_file(
                "output/review/priority-queue.json",
                r#"{"summary":{"total_assets":2,"total_observations":2,"high_priority_count":1,"medium_priority_count":1,"top_roles":["authentication (1)"],"top_environments":["staging (1)"],"top_review_targets":["auth.example.com [high:82]","www.example.com [medium:40]"]},"items":[{"rank":1,"asset":"auth.example.com","risk_level":"high","score":92,"confidence":0.91,"semantic_roles":["authentication","api_gateway"],"environments":["staging"],"reasons":["Auth-related API observations increase review priority","Graph-neighborhood evidence adds context for manual review"],"evidence_refs":["evidence-auth-1","evidence-auth-1","evidence-auth-2"],"recommended_next_steps":["Review auth-related routes carefully.","Review GraphQL-related artifacts manually."]},{"rank":2,"asset":"www.example.com","risk_level":"medium","score":44,"confidence":0.67,"semantic_roles":["customer_app"],"environments":["production"],"reasons":["JavaScript-derived routes add hidden application context"],"evidence_refs":["evidence-www-1"],"recommended_next_steps":["Compare JavaScript-derived hidden routes with the visible application flow."]}]}"#,
            )?;
            self.write_file(
                "output/review/evidence-index.json",
                r#"{"summary":{"total_assets":2,"total_observations":2,"high_priority_count":1,"medium_priority_count":1,"top_roles":["authentication (1)"],"top_environments":["staging (1)"],"top_review_targets":["auth.example.com [high:82]","www.example.com [medium:40]"]},"items":[{"review_item":"auth.example.com","review_rank":1,"evidence_id":"evidence-auth-1","asset":"auth.example.com","source":"auth-observations.json","evidence_type":"auth:jwt_bearer","description":"Auth observation 'jwt_bearer' with indicators: bearer_token, jwt","related_nodes":["host:auth"],"related_edges":["host:auth -> url:graphql [loads_endpoint]"]},{"review_item":"auth.example.com","review_rank":1,"evidence_id":"evidence-auth-1","asset":"auth.example.com","source":"auth-observations.json","evidence_type":"auth:jwt_bearer","description":"Auth observation 'jwt_bearer' with indicators: bearer_token, jwt","related_nodes":["host:auth"],"related_edges":["host:auth -> url:graphql [loads_endpoint]"]},{"review_item":"auth.example.com","review_rank":1,"evidence_id":"evidence-auth-2","asset":"auth.example.com","source":"graphql-observations.json","evidence_type":"graphql","description":"GraphQL candidate 'https://auth.example.com/graphql' with indicators: apollo","related_nodes":["host:auth"],"related_edges":["host:auth -> url:graphql [loads_endpoint]"]},{"review_item":"www.example.com","review_rank":2,"evidence_id":"evidence-www-1","asset":"www.example.com","source":"js-observations.json","evidence_type":"javascript","description":"JavaScript artifact 'output/js/app.js' referenced 1 endpoint candidate","related_nodes":["host:www"],"related_edges":[]}]} "#,
            )?;
            self.write_file(
                "output/review/executive-summary.md",
                "# Review Summary\n\nThese are recon-prioritization candidates, not confirmed vulnerabilities.\n",
            )?;
            Ok(())
        }

        fn seed_optional_inputs(&self) -> Result<()> {
            self.write_file(
                "output/api-intel/api-endpoints.json",
                r#"[{"endpoint_id":"endpoint:get:/graphql","method":"GET","path":"https://auth.example.com/graphql","normalized_path":"/graphql","parameters":["user_id"],"auth_indicators":["bearer_token","jwt"],"inferred_objects":["User"],"semantic_tags":[{"tag":"graphql_surface","category":"endpoint_intent","confidence":0.91,"evidence":["Matched '/graphql'"]}],"source":"graph.json"}]"#,
            )?;
            self.write_file(
                "output/api-intel/api-objects.json",
                r#"[{"object_name":"User","related_endpoints":["/users/{id}"],"related_parameters":["user_id"],"inferred_sensitivity":"high"}]"#,
            )?;
            self.write_file(
                "output/api-intel/auth-observations.json",
                r#"[{"asset":"auth.example.com","auth_type":"jwt_bearer","indicators":["bearer_token","jwt"],"confidence":0.91,"evidence":["Bearer references in local API artifacts"]}]"#,
            )?;
            self.write_file(
                "output/api-intel/js-observations.json",
                r#"[{"js_file":"output/js/app.js","discovered_endpoints":["https://auth.example.com/internal/graphql"],"discovered_roles":["api_gateway"],"discovered_auth_indicators":["jwt"],"discovered_feature_flags":["betaAdmin"],"evidence":["JS referenced internal GraphQL route"]}]"#,
            )?;
            self.write_file(
                "output/api-intel/schemas.json",
                r#"[{"schema_type":"openapi","schema_location":"https://auth.example.com/swagger.json","detected_version":"3.0.2","endpoints":["GET /users/{id}"],"auth_methods":["bearerAuth"],"objects":["User"]}]"#,
            )?;
            self.write_file(
                "output/api-intel/graphql-observations.json",
                r#"[{"endpoint":"https://auth.example.com/graphql","introspection_detected":false,"schema_indicators":["apollo"],"auth_indicators":["jwt"],"notes":["GraphQL candidate from local JS"]}]"#,
            )?;
            self.write_file(
                "output/api-intel/api-graph.json",
                r#"{"summary":{"generated_at":"2026-05-14T09:00:00Z","endpoint_count":1,"object_count":1,"relationship_count":1,"schema_count":1,"auth_observation_count":1,"graphql_observation_count":1,"js_observation_count":1,"api_family_count":1,"privileged_endpoint_count":1,"top_auth_styles":["jwt_bearer (1)"],"likely_sensitive_objects":["User (high)"],"hidden_route_candidates":["https://auth.example.com/internal/graphql"]}}"#,
            )?;
            self.write_file(
                "output/api-intel/api-summary.md",
                "# API Summary\n\nLocal-only.\n",
            )?;
            self.write_file(
                "output/maps/graph-summary.json",
                r#"{"generated_at":"2026-05-14T09:00:00Z","node_count":4,"edge_count":3,"cluster_count":1,"anomaly_count":1,"top_technologies":["GraphQL (1)"],"largest_clusters":["cluster:auth-admin [privileged-surface]"],"shared_infrastructure":["auth.example.com <-> admin.example.com"],"suspicious_naming":["auth.example.com"],"likely_staging_systems":["auth.example.com"],"likely_internal_systems":[],"redirect_chain_count":0}"#,
            )?;
            self.write_file(
                "output/maps/clusters.json",
                r#"{"clusters":[{"cluster_id":"cluster:auth-admin","cluster_type":"privileged-surface","related_nodes":["host:auth"],"shared_indicators":["auth"],"risk_score":78}]}"#,
            )?;
            Ok(())
        }

        fn seed_long_evidence(&self) -> Result<()> {
            let long_description = "A".repeat(16_000);
            self.write_file(
                "output/review/evidence-index.json",
                &format!(
                    r#"{{"summary":{{"total_assets":1,"total_observations":1,"high_priority_count":1,"medium_priority_count":0,"top_roles":["authentication (1)"],"top_environments":["staging (1)"],"top_review_targets":["auth.example.com [high:82]"]}},"items":[{{"review_item":"auth.example.com","review_rank":1,"evidence_id":"long-evidence-1","asset":"auth.example.com","source":"semantic-observations.json","evidence_type":"observation","description":"{}","related_nodes":["host:auth"],"related_edges":[]}}]}}"#,
                    long_description
                ),
            )?;
            self.write_file(
                "output/review/priority-queue.json",
                r#"{"summary":{"total_assets":1,"total_observations":1,"high_priority_count":1,"medium_priority_count":0,"top_roles":["authentication (1)"],"top_environments":["staging (1)"],"top_review_targets":["auth.example.com [high:82]"]},"items":[{"rank":1,"asset":"auth.example.com","risk_level":"high","score":82,"confidence":0.9,"semantic_roles":["authentication"],"environments":["staging"],"reasons":["Auth-related API observations increase review priority"],"evidence_refs":["long-evidence-1"],"recommended_next_steps":["Review auth-related routes carefully."]}]}"#,
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
    fn context_pack_generation_creates_outputs() -> Result<()> {
        let workspace = TestWorkspace::new("context-pack")?;
        workspace.seed_required_inputs()?;
        workspace.seed_optional_inputs()?;

        let outcome = run_llm_pack(&workspace.output_root(), &workspace.llm_pack_dir(), 12_000)?;
        assert!(outcome.asset_contexts_dir.exists());
        assert!(outcome.reasoning_queue_json_path.exists());
        assert!(outcome.analyst_brief_path.exists());
        assert!(outcome.pack_summary_path.exists());
        Ok(())
    }

    #[test]
    fn prompt_template_generation_writes_expected_files() -> Result<()> {
        let workspace = TestWorkspace::new("prompts")?;
        workspace.seed_required_inputs()?;

        let outcome = run_llm_pack(&workspace.output_root(), &workspace.llm_pack_dir(), 12_000)?;
        let files = fs::read_dir(outcome.prompts_dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(files.len(), 5);
        let auth_prompt = fs::read_to_string(
            workspace
                .llm_pack_dir()
                .join("prompts")
                .join("auth_flow_review_prompt.md"),
        )?;
        assert!(auth_prompt.contains("Do not suggest credential attacks"));
        Ok(())
    }

    #[test]
    fn reasoning_queue_ranking_prefers_richer_auth_assets() -> Result<()> {
        let workspace = TestWorkspace::new("ranking")?;
        workspace.seed_required_inputs()?;
        workspace.seed_optional_inputs()?;

        let outcome = run_llm_pack(&workspace.output_root(), &workspace.llm_pack_dir(), 12_000)?;
        let queue = fs::read_to_string(outcome.reasoning_queue_json_path)?;
        let value: Value = serde_json::from_str(&queue)?;
        let first_asset = value["items"][0]["asset"]
            .as_str()
            .expect("first ranked reasoning item should exist");
        assert_eq!(first_asset, "auth.example.com");
        Ok(())
    }

    #[test]
    fn evidence_deduplication_preserves_unique_refs_only() -> Result<()> {
        let workspace = TestWorkspace::new("dedupe")?;
        workspace.seed_required_inputs()?;

        run_llm_pack(&workspace.output_root(), &workspace.llm_pack_dir(), 12_000)?;
        let context = fs::read_to_string(
            workspace
                .llm_pack_dir()
                .join("asset-contexts")
                .join("001-auth-example-com.json"),
        )?;
        let value: Value = serde_json::from_str(&context)?;
        assert_eq!(
            value["evidence_refs"].as_array().map(|items| items.len()),
            Some(2)
        );
        Ok(())
    }

    #[test]
    fn truncation_behavior_respects_context_budget() -> Result<()> {
        let workspace = TestWorkspace::new("truncate")?;
        workspace.seed_required_inputs()?;
        workspace.seed_long_evidence()?;

        let outcome = run_llm_pack(&workspace.output_root(), &workspace.llm_pack_dir(), 1200)?;
        let context_path = workspace
            .llm_pack_dir()
            .join("asset-contexts")
            .join("001-auth-example-com.json");
        let raw = fs::read_to_string(context_path)?;
        let value: Value = serde_json::from_str(&raw)?;
        assert_eq!(value["truncated"], Value::Bool(true));
        assert!(
            value["context_markdown"]
                .as_str()
                .expect("context markdown should be present")
                .len()
                <= 1200
        );
        assert_eq!(outcome.summary.truncated_context_count, 1);
        Ok(())
    }

    #[test]
    fn missing_optional_inputs_still_generates_pack() -> Result<()> {
        let workspace = TestWorkspace::new("missing-optional")?;
        workspace.seed_required_inputs()?;

        let outcome = run_llm_pack(&workspace.output_root(), &workspace.llm_pack_dir(), 12_000)?;
        assert!(!outcome.summary.api_intel_present);
        assert!(!outcome.summary.graph_summary_present);
        Ok(())
    }

    #[test]
    fn empty_review_queue_handling_writes_empty_reasoning_outputs() -> Result<()> {
        let workspace = TestWorkspace::new("empty-queue")?;
        workspace.write_file("output/enrichment/semantic-assets.json", "[]")?;
        workspace.write_file("output/enrichment/semantic-observations.json", "[]")?;
        workspace.write_file("output/enrichment/risk-explanations.json", "[]")?;
        workspace.write_file(
            "output/enrichment/enriched-graph.json",
            r#"{"assets":[],"observations":[],"risk_explanations":[],"original_graph_summary":{"generated_at":"2026-05-14T09:00:00Z","node_count":0,"edge_count":0,"cluster_count":0,"anomaly_count":0,"top_technologies":[],"largest_clusters":[],"shared_infrastructure":[],"suspicious_naming":[],"likely_staging_systems":[],"likely_internal_systems":[],"redirect_chain_count":0},"original_nodes":[],"original_edges":[],"original_clusters":[],"semantic_summary":{"generated_at":"2026-05-14T09:00:00Z","asset_count":0,"observation_count":0,"risk_explanation_count":0,"api_endpoint_count":0,"auth_surface_count":0,"js_observation_count":0,"schema_observation_count":0,"graphql_observation_count":0,"top_roles":[],"top_environments":[],"highest_priority_assets":[],"notable_neighborhood_observations":[],"sensitive_object_candidates":[],"api_intelligence_warnings":[],"recommended_next_steps":[]}}"#,
        )?;
        workspace.write_file(
            "output/enrichment/enrichment-summary.md",
            "# ReconPilot Semantic Enrichment Summary\n\nNone observed yet.\n",
        )?;
        workspace.write_file(
            "output/review/priority-queue.json",
            r#"{"summary":{"total_assets":0,"total_observations":0,"high_priority_count":0,"medium_priority_count":0,"top_roles":[],"top_environments":[],"top_review_targets":[]},"items":[]}"#,
        )?;
        workspace.write_file(
            "output/review/evidence-index.json",
            r#"{"summary":{"total_assets":0,"total_observations":0,"high_priority_count":0,"medium_priority_count":0,"top_roles":[],"top_environments":[],"top_review_targets":[]},"items":[]}"#,
        )?;

        let outcome = run_llm_pack(&workspace.output_root(), &workspace.llm_pack_dir(), 12_000)?;
        let queue = fs::read_to_string(outcome.reasoning_queue_markdown_path)?;
        assert!(queue.contains("No reasoning items were generated"));
        Ok(())
    }
}
