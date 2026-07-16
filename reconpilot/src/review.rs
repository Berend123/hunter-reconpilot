use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, BTreeSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::{
    models::{
        ApiEndpoint, ApiGraphSummary, ApiObject, ApiRelationship, ApiSchema, AssetCard, AssetRole,
        AuthObservation, EnrichedAsset, EnrichedGraph, EnvironmentType, EvidenceItem,
        GraphQlObservation, JsObservation, ReviewChecklist, ReviewItem, ReviewSummary,
        RiskExplanation, SemanticObservation,
    },
    utils,
};

#[derive(Debug, Clone)]
pub struct ReviewWorkspaceOutcome {
    pub priority_queue_markdown_path: PathBuf,
    pub priority_queue_json_path: PathBuf,
    pub asset_cards_dir: PathBuf,
    pub review_checklist_path: PathBuf,
    pub executive_summary_path: PathBuf,
    pub evidence_index_path: PathBuf,
    pub summary: ReviewSummary,
}

#[derive(Debug, Clone)]
struct ReviewAssetBundle {
    asset: EnrichedAsset,
    observations: Vec<SemanticObservation>,
    primary_risk: RiskExplanation,
    evidence: Vec<EvidenceItem>,
    adjusted_score: i32,
    confidence: f32,
    reasons: Vec<String>,
    recommended_next_steps: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct ApiIntelData {
    endpoints: Vec<ApiEndpoint>,
    objects: Vec<ApiObject>,
    relationships: Vec<ApiRelationship>,
    auth_observations: Vec<AuthObservation>,
    js_observations: Vec<JsObservation>,
    schemas: Vec<ApiSchema>,
    graphql_observations: Vec<GraphQlObservation>,
    summary: Option<ApiGraphSummary>,
    summary_markdown: Option<String>,
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

#[derive(Debug, Serialize)]
struct PriorityQueueDocument {
    summary: ReviewSummary,
    items: Vec<ReviewItem>,
}

#[derive(Debug, Serialize)]
struct EvidenceIndexEntry {
    review_item: String,
    review_rank: usize,
    #[serde(flatten)]
    evidence: EvidenceItem,
}

#[derive(Debug, Serialize)]
struct EvidenceIndexDocument {
    summary: ReviewSummary,
    items: Vec<EvidenceIndexEntry>,
}

pub fn run_review_workspace(input: &Path, out: &Path) -> Result<ReviewWorkspaceOutcome> {
    validate_review_input(input)?;
    utils::ensure_directory(out)?;
    let api_intel = load_api_intel_data(input)?;

    let semantic_assets = load_json::<Vec<EnrichedAsset>>(&input.join("semantic-assets.json"))?;
    let observations =
        load_json::<Vec<SemanticObservation>>(&input.join("semantic-observations.json"))?;
    let risk_explanations =
        load_json::<Vec<RiskExplanation>>(&input.join("risk-explanations.json"))?;
    let enriched_graph = load_json::<EnrichedGraph>(&input.join("enriched-graph.json"))?;
    let enrichment_summary =
        fs::read_to_string(input.join("enrichment-summary.md")).with_context(|| {
            format!(
                "failed to read enrichment summary markdown at {}",
                input.join("enrichment-summary.md").display()
            )
        })?;

    let semantic_assets = if semantic_assets.is_empty() {
        enriched_graph.assets.clone()
    } else {
        semantic_assets
    };

    let mut bundles = build_review_bundles(
        &semantic_assets,
        &observations,
        &risk_explanations,
        &enriched_graph,
        &api_intel,
    );
    sort_review_bundles(&mut bundles);

    let review_items = build_review_items(&bundles);
    let review_summary = build_review_summary(&review_items, &bundles, &observations);
    let asset_cards = build_asset_cards(&bundles);
    let checklist = build_review_checklist();
    let evidence_index = build_evidence_index(&review_items, &bundles, &review_summary);

    write_review_outputs(
        out,
        &review_items,
        &asset_cards,
        &checklist,
        &review_summary,
        &evidence_index,
        &bundles,
        &enrichment_summary,
        &api_intel,
    )
}

pub fn print_review_summary(input: &Path, out: &Path, outcome: &ReviewWorkspaceOutcome) {
    println!("ReconPilot review workspace summary");
    println!("Input enrichment: {}", input.display());
    println!("Output review: {}", out.display());
    println!("Assets queued: {}", outcome.summary.total_assets);
    println!(
        "High priority: {} | Medium priority: {}",
        outcome.summary.high_priority_count, outcome.summary.medium_priority_count
    );
    println!("Top roles: {}", display_or_none(&outcome.summary.top_roles));
    println!(
        "Top environments: {}",
        display_or_none(&outcome.summary.top_environments)
    );
    println!("Outputs:");
    println!("  - {}", outcome.priority_queue_markdown_path.display());
    println!("  - {}", outcome.priority_queue_json_path.display());
    println!("  - {}", outcome.asset_cards_dir.display());
    println!("  - {}", outcome.review_checklist_path.display());
    println!("  - {}", outcome.executive_summary_path.display());
    println!("  - {}", outcome.evidence_index_path.display());
}

fn validate_review_input(input: &Path) -> Result<()> {
    if !input.exists() {
        bail!("review input directory does not exist: {}", input.display());
    }
    if !input.is_dir() {
        bail!("review input path is not a directory: {}", input.display());
    }

    for file_name in [
        "semantic-assets.json",
        "semantic-observations.json",
        "risk-explanations.json",
        "enriched-graph.json",
        "enrichment-summary.md",
    ] {
        let path = input.join(file_name);
        if !path.exists() {
            bail!(
                "required review input is missing: {}. Run `reconpilot enrich --input output/maps/ --out output/enrichment/` first.",
                path.display()
            );
        }
    }

    Ok(())
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read JSON input: {}", path.display()))?;
    serde_json::from_str::<T>(&raw)
        .with_context(|| format!("failed to parse JSON input: {}", path.display()))
}

fn load_optional_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T> {
    if !path.exists() {
        return Ok(T::default());
    }
    load_json(path)
}

fn load_api_intel_data(input: &Path) -> Result<ApiIntelData> {
    let Some(output_root) = input.parent() else {
        return Ok(ApiIntelData::default());
    };
    let api_root = output_root.join("api-intel");
    if !api_root.exists() || !api_root.is_dir() {
        return Ok(ApiIntelData::default());
    }

    let summary = if api_root.join("api-graph.json").exists() {
        let raw = fs::read_to_string(api_root.join("api-graph.json")).with_context(|| {
            format!(
                "failed to read API graph input: {}",
                api_root.join("api-graph.json").display()
            )
        })?;
        serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|value| value.get("summary").cloned())
            .map(serde_json::from_value)
            .transpose()
            .with_context(|| {
                format!(
                    "failed to parse API graph summary from {}",
                    api_root.join("api-graph.json").display()
                )
            })?
    } else {
        None
    };

    Ok(ApiIntelData {
        endpoints: load_optional_json(&api_root.join("api-endpoints.json"))?,
        objects: load_optional_json(&api_root.join("api-objects.json"))?,
        relationships: load_optional_json(&api_root.join("api-relationships.json"))?,
        auth_observations: load_optional_json(&api_root.join("auth-observations.json"))?,
        js_observations: load_optional_json(&api_root.join("js-observations.json"))?,
        schemas: load_optional_json(&api_root.join("schemas.json"))?,
        graphql_observations: load_optional_json(&api_root.join("graphql-observations.json"))?,
        summary,
        summary_markdown: fs::read_to_string(api_root.join("api-summary.md")).ok(),
    })
}

fn build_review_bundles(
    semantic_assets: &[EnrichedAsset],
    observations: &[SemanticObservation],
    risk_explanations: &[RiskExplanation],
    enriched_graph: &EnrichedGraph,
    api_intel: &ApiIntelData,
) -> Vec<ReviewAssetBundle> {
    let observation_map = observations.iter().cloned().fold(
        BTreeMap::<String, Vec<SemanticObservation>>::new(),
        |mut acc, observation| {
            acc.entry(observation.asset.clone())
                .or_default()
                .push(observation);
            acc
        },
    );
    let risk_map = risk_explanations
        .iter()
        .cloned()
        .map(|risk| (risk.asset.clone(), risk))
        .collect::<BTreeMap<_, _>>();

    let mut bundles = Vec::new();

    for asset in semantic_assets {
        let api_context = build_effective_api_asset_context(asset, api_intel);
        let asset_observations = observation_map
            .get(&asset.asset)
            .cloned()
            .unwrap_or_default();
        let primary_risk = risk_map
            .get(&asset.asset)
            .cloned()
            .or_else(|| asset.risk_explanations.first().cloned())
            .unwrap_or_else(|| fallback_risk(&asset.asset));
        let evidence = build_evidence_items(
            asset,
            &asset_observations,
            &primary_risk,
            enriched_graph,
            &api_context,
        );
        let confidence = calculate_review_confidence(asset, &asset_observations, &evidence);
        let reasons = build_review_reasons(asset, &asset_observations, &primary_risk, &api_context);
        let recommended_next_steps = build_recommended_steps(asset, &primary_risk, &api_context);
        let adjusted_score = calculate_adjusted_score(
            asset,
            &asset_observations,
            &primary_risk,
            &reasons,
            &api_context,
        );

        bundles.push(ReviewAssetBundle {
            asset: asset.clone(),
            observations: asset_observations,
            primary_risk,
            evidence,
            adjusted_score,
            confidence,
            reasons,
            recommended_next_steps,
        });
    }

    bundles
}

fn build_effective_api_asset_context(
    asset: &EnrichedAsset,
    api_intel: &ApiIntelData,
) -> ApiAssetContext {
    let fallback = build_api_asset_context(asset, api_intel);
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

fn build_api_asset_context(asset: &EnrichedAsset, api_intel: &ApiIntelData) -> ApiAssetContext {
    let endpoints = api_intel
        .endpoints
        .iter()
        .filter(|endpoint| endpoint_matches_asset(&asset.asset, endpoint))
        .cloned()
        .collect::<Vec<_>>();
    let endpoint_ids = endpoints
        .iter()
        .map(|endpoint| endpoint.endpoint_id.clone())
        .collect::<BTreeSet<_>>();
    let normalized_paths = endpoints
        .iter()
        .map(|endpoint| endpoint.normalized_path.clone())
        .collect::<BTreeSet<_>>();

    let objects = api_intel
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
    let schemas = api_intel
        .schemas
        .iter()
        .filter(|schema| {
            schema_matches_asset(&asset.asset, schema)
                || schema
                    .endpoints
                    .iter()
                    .map(|endpoint| normalize_schema_endpoint_for_match(endpoint))
                    .any(|path| normalized_paths.contains(&path))
        })
        .cloned()
        .collect::<Vec<_>>();
    let auth_observations = api_intel
        .auth_observations
        .iter()
        .filter(|observation| auth_observation_matches_asset(&asset.asset, observation, &endpoints))
        .cloned()
        .collect::<Vec<_>>();
    let js_observations = api_intel
        .js_observations
        .iter()
        .filter(|observation| js_observation_matches_asset(&asset.asset, observation, &endpoints))
        .cloned()
        .collect::<Vec<_>>();
    let graphql_observations = api_intel
        .graphql_observations
        .iter()
        .filter(|observation| graphql_matches_asset(&asset.asset, observation, &endpoints))
        .cloned()
        .collect::<Vec<_>>();

    let mut context = ApiAssetContext {
        endpoints,
        objects,
        auth_observations,
        js_observations,
        schemas,
        graphql_observations,
    };

    // Pull in objects that are related through explicit API relationship links.
    let related_object_names = api_intel
        .relationships
        .iter()
        .filter(|relationship| endpoint_ids.contains(&relationship.source_endpoint))
        .filter_map(|relationship| {
            if relationship.relationship_type == "returns_object" {
                Some(relationship.target_object.clone())
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>();
    for object in &api_intel.objects {
        if related_object_names.contains(&object.object_name)
            && !context
                .objects
                .iter()
                .any(|existing| existing.object_name == object.object_name)
        {
            context.objects.push(object.clone());
        }
    }
    context
        .objects
        .sort_by(|left, right| left.object_name.cmp(&right.object_name));

    context
}

fn endpoint_matches_asset(asset: &str, endpoint: &ApiEndpoint) -> bool {
    let asset_lc = asset.to_ascii_lowercase();
    if endpoint.path.to_ascii_lowercase().contains(&asset_lc) {
        return true;
    }
    extract_host_from_string(&endpoint.path)
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
        endpoint_matches_asset(asset, &js_endpoint_to_api(endpoint, &observation.js_file))
    }) || observation.evidence.iter().any(|evidence| {
        evidence
            .to_ascii_lowercase()
            .contains(&asset.to_ascii_lowercase())
    }) || endpoints.iter().any(|endpoint| {
        observation.discovered_endpoints.iter().any(|candidate| {
            normalize_schema_endpoint_for_match(candidate) == endpoint.normalized_path
        })
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
            endpoint.normalized_path == normalize_schema_endpoint_for_match(&observation.endpoint)
        })
}

fn js_endpoint_to_api(endpoint: &str, source: &str) -> ApiEndpoint {
    ApiEndpoint {
        endpoint_id: format!("temp:{}", endpoint),
        method: "GET".to_string(),
        path: endpoint.to_string(),
        normalized_path: normalize_schema_endpoint_for_match(endpoint),
        parameters: Vec::new(),
        auth_indicators: Vec::new(),
        inferred_objects: Vec::new(),
        semantic_tags: Vec::new(),
        source: source.to_string(),
    }
}

fn build_evidence_items(
    asset: &EnrichedAsset,
    observations: &[SemanticObservation],
    risk: &RiskExplanation,
    enriched_graph: &EnrichedGraph,
    api_context: &ApiAssetContext,
) -> Vec<EvidenceItem> {
    let node_ids = asset_node_ids(asset, enriched_graph);
    let related_edges = related_edge_descriptions(&node_ids, &asset.related_nodes, enriched_graph);
    let mut items = Vec::new();
    let mut counter = 0usize;

    for tag in &asset.semantic_tags {
        counter += 1;
        items.push(EvidenceItem {
            evidence_id: evidence_id(&asset.asset, counter),
            asset: asset.asset.clone(),
            source: "semantic-assets.json".to_string(),
            evidence_type: format!("semantic_tag:{}", tag.category),
            description: format!(
                "Semantic tag '{}' [{}] with evidence: {}",
                tag.tag,
                tag.category,
                display_or_none(&tag.evidence)
            ),
            related_nodes: asset.related_nodes.clone(),
            related_edges: related_edges.clone(),
        });
    }

    for observation in observations {
        counter += 1;
        items.push(EvidenceItem {
            evidence_id: evidence_id(&asset.asset, counter),
            asset: asset.asset.clone(),
            source: "semantic-observations.json".to_string(),
            evidence_type: observation.observation_type.clone(),
            description: format!(
                "{} Evidence: {}",
                observation.description,
                display_or_none(&observation.evidence)
            ),
            related_nodes: if observation.related_nodes.is_empty() {
                asset.related_nodes.clone()
            } else {
                observation.related_nodes.clone()
            },
            related_edges: related_edges.clone(),
        });
    }

    for factor in &risk.contributing_factors {
        counter += 1;
        items.push(EvidenceItem {
            evidence_id: evidence_id(&asset.asset, counter),
            asset: asset.asset.clone(),
            source: "risk-explanations.json".to_string(),
            evidence_type: "risk_factor".to_string(),
            description: factor.clone(),
            related_nodes: asset.related_nodes.clone(),
            related_edges: related_edges.clone(),
        });
    }

    if !asset
        .neighborhood_summary
        .starts_with("No notable graph-neighborhood")
    {
        counter += 1;
        items.push(EvidenceItem {
            evidence_id: evidence_id(&asset.asset, counter),
            asset: asset.asset.clone(),
            source: "enriched-graph.json".to_string(),
            evidence_type: "graph_neighborhood".to_string(),
            description: asset.neighborhood_summary.clone(),
            related_nodes: asset.related_nodes.clone(),
            related_edges: related_edges.clone(),
        });
    }

    for endpoint in &api_context.endpoints {
        counter += 1;
        items.push(EvidenceItem {
            evidence_id: evidence_id(&asset.asset, counter),
            asset: asset.asset.clone(),
            source: "api-endpoints.json".to_string(),
            evidence_type: "api_endpoint".to_string(),
            description: format!(
                "API endpoint candidate '{}' [{}] with auth indicators: {}",
                endpoint.path,
                endpoint.method,
                display_or_none(&endpoint.auth_indicators)
            ),
            related_nodes: asset.related_nodes.clone(),
            related_edges: related_edges.clone(),
        });
    }

    for object in &api_context.objects {
        counter += 1;
        items.push(EvidenceItem {
            evidence_id: evidence_id(&asset.asset, counter),
            asset: asset.asset.clone(),
            source: "api-objects.json".to_string(),
            evidence_type: "api_object".to_string(),
            description: format!(
                "API object '{}' was inferred as {} sensitivity",
                object.object_name, object.inferred_sensitivity
            ),
            related_nodes: asset.related_nodes.clone(),
            related_edges: related_edges.clone(),
        });
    }

    for observation in &api_context.auth_observations {
        counter += 1;
        items.push(EvidenceItem {
            evidence_id: evidence_id(&asset.asset, counter),
            asset: asset.asset.clone(),
            source: "auth-observations.json".to_string(),
            evidence_type: format!("auth:{}", observation.auth_type),
            description: format!(
                "Auth observation '{}' with indicators: {}",
                observation.auth_type,
                display_or_none(&observation.indicators)
            ),
            related_nodes: asset.related_nodes.clone(),
            related_edges: related_edges.clone(),
        });
    }

    for schema in &api_context.schemas {
        counter += 1;
        items.push(EvidenceItem {
            evidence_id: evidence_id(&asset.asset, counter),
            asset: asset.asset.clone(),
            source: "schemas.json".to_string(),
            evidence_type: format!("schema:{}", schema.schema_type),
            description: format!(
                "Schema or documentation candidate '{}' with auth methods: {}",
                schema.schema_location,
                display_or_none(&schema.auth_methods)
            ),
            related_nodes: asset.related_nodes.clone(),
            related_edges: related_edges.clone(),
        });
    }

    for observation in &api_context.graphql_observations {
        counter += 1;
        items.push(EvidenceItem {
            evidence_id: evidence_id(&asset.asset, counter),
            asset: asset.asset.clone(),
            source: "graphql-observations.json".to_string(),
            evidence_type: "graphql".to_string(),
            description: format!(
                "GraphQL candidate '{}' with indicators: {}",
                observation.endpoint,
                display_or_none(&observation.schema_indicators)
            ),
            related_nodes: asset.related_nodes.clone(),
            related_edges: related_edges.clone(),
        });
    }

    for observation in &api_context.js_observations {
        counter += 1;
        items.push(EvidenceItem {
            evidence_id: evidence_id(&asset.asset, counter),
            asset: asset.asset.clone(),
            source: "js-observations.json".to_string(),
            evidence_type: "javascript".to_string(),
            description: format!(
                "JavaScript artifact '{}' referenced {} endpoint candidate{}",
                observation.js_file,
                observation.discovered_endpoints.len(),
                plural(observation.discovered_endpoints.len())
            ),
            related_nodes: asset.related_nodes.clone(),
            related_edges: related_edges.clone(),
        });
    }

    dedupe_evidence_items(items)
}

fn calculate_review_confidence(
    asset: &EnrichedAsset,
    observations: &[SemanticObservation],
    evidence: &[EvidenceItem],
) -> f32 {
    let tag_avg = if asset.semantic_tags.is_empty() {
        0.55
    } else {
        asset
            .semantic_tags
            .iter()
            .map(|tag| tag.confidence)
            .sum::<f32>()
            / asset.semantic_tags.len() as f32
    };
    let observation_avg = if observations.is_empty() {
        0.55
    } else {
        observations
            .iter()
            .map(|observation| observation.confidence)
            .sum::<f32>()
            / observations.len() as f32
    };
    let corroboration = (distinct_sources(evidence).len() as f32 / 4.0).min(1.0);
    round_confidence((tag_avg * 0.45) + (observation_avg * 0.35) + (corroboration * 0.20))
}

fn calculate_adjusted_score(
    asset: &EnrichedAsset,
    observations: &[SemanticObservation],
    risk: &RiskExplanation,
    reasons: &[String],
    api_context: &ApiAssetContext,
) -> i32 {
    let mut score = risk.score;

    if asset.roles.len() > 1 {
        score += ((asset.roles.len() - 1) as i32 * 5).min(10);
    }
    if asset
        .roles
        .iter()
        .any(|role| matches!(role, AssetRole::Authentication | AssetRole::AdminDashboard))
    {
        score += 10;
    }
    if asset.roles.iter().any(|role| {
        matches!(
            role,
            AssetRole::Monitoring | AssetRole::Logging | AssetRole::CICD
        )
    }) {
        score += 7;
    }
    if asset.environments.contains(&EnvironmentType::Internal) {
        score += 10;
    }
    if asset.environments.contains(&EnvironmentType::Legacy) {
        score += 8;
    }
    if asset.environments.contains(&EnvironmentType::Staging) {
        score += 6;
    }
    if asset.environments.iter().any(|environment| {
        matches!(
            environment,
            EnvironmentType::Development | EnvironmentType::Testing
        )
    }) {
        score += 3;
    }
    if !asset
        .neighborhood_summary
        .starts_with("No notable graph-neighborhood")
        || observations.iter().any(|observation| {
            matches!(
                observation.observation_type.as_str(),
                "neighborhood" | "cluster" | "operational-tooling"
            )
        })
    {
        score += 6;
    }
    if reasons.len() > 3 {
        score += 3;
    }
    if !api_context.auth_observations.is_empty() {
        score += 8;
    }
    if !api_context.graphql_observations.is_empty() {
        score += 6;
    }
    if api_context
        .objects
        .iter()
        .any(|object| object.inferred_sensitivity == "high")
    {
        score += 7;
    }
    if !api_context.schemas.is_empty() {
        score += 5;
    }
    if !api_context.js_observations.is_empty() && !api_context.endpoints.is_empty() {
        score += 4;
    }

    score.clamp(0, 100)
}

fn build_review_reasons(
    asset: &EnrichedAsset,
    observations: &[SemanticObservation],
    risk: &RiskExplanation,
    api_context: &ApiAssetContext,
) -> Vec<String> {
    let mut reasons = risk.contributing_factors.clone();
    reasons.extend(
        observations
            .iter()
            .map(|observation| observation.description.clone()),
    );

    if asset.roles.len() > 1 {
        reasons.push("Multiple semantic roles increase review value".to_string());
    }
    if asset
        .roles
        .iter()
        .any(|role| matches!(role, AssetRole::Authentication | AssetRole::AdminDashboard))
    {
        reasons.push("Privileged role indicators increase review priority".to_string());
    }
    if asset.environments.iter().any(|environment| {
        matches!(
            environment,
            EnvironmentType::Internal | EnvironmentType::Legacy | EnvironmentType::Staging
        )
    }) {
        reasons.push("Non-standard environment indicators increase review priority".to_string());
    }
    if !asset
        .neighborhood_summary
        .starts_with("No notable graph-neighborhood")
    {
        reasons.push("Graph-neighborhood evidence adds context for manual review".to_string());
    }
    if !api_context.auth_observations.is_empty() {
        reasons.push("Auth-related API observations increase review priority".to_string());
    }
    if api_context
        .objects
        .iter()
        .any(|object| object.inferred_sensitivity == "high")
    {
        reasons.push("Sensitive object modeling increases review value".to_string());
    }
    if !api_context.schemas.is_empty() {
        reasons.push("API documentation or schema exposure merits careful review".to_string());
    }
    if !api_context.graphql_observations.is_empty() {
        reasons.push("GraphQL indicators suggest an additional application surface".to_string());
    }
    if !api_context.js_observations.is_empty() {
        reasons.push("JavaScript-derived routes add hidden application context".to_string());
    }

    reasons.sort();
    reasons.dedup();
    reasons
}

fn build_recommended_steps(
    asset: &EnrichedAsset,
    risk: &RiskExplanation,
    api_context: &ApiAssetContext,
) -> Vec<String> {
    let mut steps = risk.recommended_next_steps.clone();

    if asset.roles.contains(&AssetRole::Documentation) {
        steps.push(
            "Review API documentation carefully and note whether it appears intentionally exposed."
                .to_string(),
        );
    }
    if asset.roles.contains(&AssetRole::Authentication) {
        steps.push(
            "Check authentication requirements manually later without assuming a flaw.".to_string(),
        );
    }
    if asset.roles.contains(&AssetRole::AdminDashboard) {
        steps.push(
            "Review administrative surfaces carefully and confirm whether exposure is expected."
                .to_string(),
        );
    }
    if asset.environments.contains(&EnvironmentType::Internal) {
        steps.push("Confirm that internal-looking assets remain within scope and expected exposure boundaries.".to_string());
    }
    if asset.environments.contains(&EnvironmentType::Staging) {
        steps.push("Compare staging-like assets with production naming and confirm whether the surface is intended to be reachable.".to_string());
    }
    if asset.environments.contains(&EnvironmentType::Legacy) {
        steps.push("Compare legacy-like assets against current systems to determine whether the surface should still exist.".to_string());
    }
    if !api_context.auth_observations.is_empty() {
        steps.push(
            "Review auth flow references carefully and confirm whether the observed headers, tokens, or login routes are expected."
                .to_string(),
        );
    }
    if !api_context.schemas.is_empty() {
        steps.push(
            "Review local API schema or documentation artifacts carefully and note whether exposure appears intentional."
                .to_string(),
        );
    }
    if !api_context.graphql_observations.is_empty() {
        steps.push(
            "Review GraphQL-related artifacts manually and confirm whether the surface, tooling references, or introspection hints are expected."
                .to_string(),
        );
    }
    if !api_context.objects.is_empty() {
        steps.push(
            "Map interesting API objects to business context and confirm whether high-sensitivity models deserve deeper manual review later."
                .to_string(),
        );
    }
    if !api_context.js_observations.is_empty() {
        steps.push(
            "Compare JavaScript-derived hidden routes with the visible application flow before escalating any candidate."
                .to_string(),
        );
    }

    steps.sort();
    steps.dedup();
    steps
}

fn sort_review_bundles(bundles: &mut Vec<ReviewAssetBundle>) {
    bundles.sort_by(|left, right| {
        right
            .adjusted_score
            .cmp(&left.adjusted_score)
            .then_with(|| {
                right
                    .confidence
                    .partial_cmp(&left.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.asset.asset.cmp(&right.asset.asset))
    });
}

fn build_review_items(bundles: &[ReviewAssetBundle]) -> Vec<ReviewItem> {
    bundles
        .iter()
        .enumerate()
        .map(|(index, bundle)| ReviewItem {
            rank: index + 1,
            asset: bundle.asset.asset.clone(),
            risk_level: bundle.primary_risk.risk_level.clone(),
            score: bundle.adjusted_score,
            confidence: bundle.confidence,
            semantic_roles: bundle.asset.roles.clone(),
            environments: bundle.asset.environments.clone(),
            reasons: bundle.reasons.clone(),
            evidence_refs: bundle
                .evidence
                .iter()
                .map(|item| item.evidence_id.clone())
                .collect(),
            recommended_next_steps: bundle.recommended_next_steps.clone(),
        })
        .collect()
}

fn build_asset_cards(bundles: &[ReviewAssetBundle]) -> Vec<AssetCard> {
    bundles
        .iter()
        .map(|bundle| AssetCard {
            asset: bundle.asset.asset.clone(),
            overview: format!(
                "{} is a {} review candidate with score {} and confidence {:.2}.",
                bundle.asset.asset,
                bundle.primary_risk.risk_level,
                bundle.adjusted_score,
                bundle.confidence
            ),
            semantic_tags: bundle.asset.semantic_tags.clone(),
            roles: bundle.asset.roles.clone(),
            environments: bundle.asset.environments.clone(),
            graph_neighborhood_summary: bundle.asset.neighborhood_summary.clone(),
            risk_explanations: vec![bundle.primary_risk.clone()],
            evidence: bundle.evidence.clone(),
            suggested_review_steps: bundle.recommended_next_steps.clone(),
            caution_note: "These notes describe recon prioritization candidates and require manual validation before any report or testing decision.".to_string(),
        })
        .collect()
}

fn build_review_checklist() -> ReviewChecklist {
    ReviewChecklist {
        title: "ReconPilot Review Checklist".to_string(),
        items: vec![
            "Confirm the asset is in scope before any deeper review.".to_string(),
            "Check whether authentication is required before assuming exposure matters.".to_string(),
            "Review API documentation carefully and note whether it appears intentionally published.".to_string(),
            "Verify access control manually later without bypass attempts or destructive actions.".to_string(),
            "Check published rate limits and avoid noisy or repetitive requests.".to_string(),
            "Avoid destructive tests, high-volume actions, and business-impacting workflows.".to_string(),
            "Document evidence clearly with timestamps, screenshots, and source artifacts.".to_string(),
            "Validate findings before reporting; prioritization results are not vulnerability confirmations.".to_string(),
        ],
        caution_note:
            "This checklist is written for legal bug bounty or authorized review workflows."
                .to_string(),
    }
}

fn build_review_summary(
    items: &[ReviewItem],
    bundles: &[ReviewAssetBundle],
    observations: &[SemanticObservation],
) -> ReviewSummary {
    let high_priority_count = items.iter().filter(|item| item.score >= 70).count();
    let medium_priority_count = items
        .iter()
        .filter(|item| item.score >= 40 && item.score < 70)
        .count();

    let mut role_counts = BTreeMap::new();
    let mut environment_counts = BTreeMap::new();
    for bundle in bundles {
        for role in &bundle.asset.roles {
            if *role != AssetRole::Unknown {
                *role_counts
                    .entry(role.as_str().to_string())
                    .or_insert(0usize) += 1;
            }
        }
        for environment in &bundle.asset.environments {
            if *environment != EnvironmentType::Unknown {
                *environment_counts
                    .entry(environment.as_str().to_string())
                    .or_insert(0usize) += 1;
            }
        }
    }

    ReviewSummary {
        total_assets: items.len(),
        total_observations: observations.len(),
        high_priority_count,
        medium_priority_count,
        top_roles: top_counts(role_counts),
        top_environments: top_counts(environment_counts),
        top_review_targets: items
            .iter()
            .take(5)
            .map(|item| format!("{} [{}:{}]", item.asset, item.risk_level, item.score))
            .collect(),
    }
}

fn build_evidence_index(
    review_items: &[ReviewItem],
    bundles: &[ReviewAssetBundle],
    review_summary: &ReviewSummary,
) -> EvidenceIndexDocument {
    let bundle_map = bundles
        .iter()
        .map(|bundle| (bundle.asset.asset.clone(), bundle))
        .collect::<BTreeMap<_, _>>();

    let mut items = Vec::new();
    for review_item in review_items {
        if let Some(bundle) = bundle_map.get(&review_item.asset) {
            for evidence in &bundle.evidence {
                items.push(EvidenceIndexEntry {
                    review_item: review_item.asset.clone(),
                    review_rank: review_item.rank,
                    evidence: evidence.clone(),
                });
            }
        }
    }

    EvidenceIndexDocument {
        summary: review_summary.clone(),
        items,
    }
}

fn write_review_outputs(
    out: &Path,
    review_items: &[ReviewItem],
    asset_cards: &[AssetCard],
    checklist: &ReviewChecklist,
    review_summary: &ReviewSummary,
    evidence_index: &EvidenceIndexDocument,
    bundles: &[ReviewAssetBundle],
    enrichment_summary: &str,
    api_intel: &ApiIntelData,
) -> Result<ReviewWorkspaceOutcome> {
    let priority_queue_markdown_path = out.join("priority-queue.md");
    let priority_queue_json_path = out.join("priority-queue.json");
    let asset_cards_dir = out.join("asset-cards");
    let review_checklist_path = out.join("review-checklist.md");
    let executive_summary_path = out.join("executive-summary.md");
    let evidence_index_path = out.join("evidence-index.json");

    utils::ensure_directory(&asset_cards_dir)?;

    utils::write_string(
        &priority_queue_markdown_path,
        &render_priority_queue_markdown(review_items),
    )?;
    utils::write_json_pretty(
        &priority_queue_json_path,
        &PriorityQueueDocument {
            summary: review_summary.clone(),
            items: review_items.to_vec(),
        },
    )?;

    for (index, card) in asset_cards.iter().enumerate() {
        let filename = format!("{:03}-{}", index + 1, sanitize_asset_filename(&card.asset));
        utils::write_string(
            &asset_cards_dir.join(filename),
            &render_asset_card_markdown(card),
        )?;
    }

    utils::write_string(
        &review_checklist_path,
        &render_review_checklist_markdown(checklist),
    )?;
    utils::write_string(
        &executive_summary_path,
        &render_executive_summary_markdown(
            review_summary,
            review_items,
            bundles,
            enrichment_summary,
            api_intel,
        ),
    )?;
    utils::write_json_pretty(&evidence_index_path, evidence_index)?;

    Ok(ReviewWorkspaceOutcome {
        priority_queue_markdown_path,
        priority_queue_json_path,
        asset_cards_dir,
        review_checklist_path,
        executive_summary_path,
        evidence_index_path,
        summary: review_summary.clone(),
    })
}

fn render_priority_queue_markdown(items: &[ReviewItem]) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot Priority Queue\n\n");
    output.push_str(
        "These entries are prioritization candidates for manual review. They are not vulnerability claims.\n\n",
    );

    if items.is_empty() {
        output.push_str("No review items were generated.\n");
        return output;
    }

    for item in items {
        output.push_str(&format!("## {}. `{}`\n\n", item.rank, item.asset));
        output.push_str(&format!(
            "- Score: {}\n- Confidence: {:.2}\n- Risk level: {}\n- Roles: {}\n- Environments: {}\n",
            item.score,
            item.confidence,
            item.risk_level,
            display_roles(&item.semantic_roles),
            display_environments(&item.environments)
        ));
        output.push_str(&format!(
            "- Why it matters: {}\n",
            item.reasons
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        ));
        output.push_str(&format!(
            "- Suggested next review step: {}\n\n",
            item.recommended_next_steps
                .first()
                .cloned()
                .unwrap_or_else(
                    || "Validate the surrounding evidence manually before escalating.".to_string()
                )
        ));
    }

    output
}

fn render_asset_card_markdown(card: &AssetCard) -> String {
    let mut output = String::new();
    output.push_str(&format!("# Asset Card: `{}`\n\n", card.asset));
    output.push_str(&format!("{}\n\n", card.overview));

    output.push_str("## Semantic Classification\n\n");
    output.push_str(&format!(
        "- Roles: {}\n- Environments: {}\n- Tags: {}\n\n",
        display_roles(&card.roles),
        display_environments(&card.environments),
        card.semantic_tags
            .iter()
            .map(|tag| format!("{} [{}]", tag.tag, tag.category))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    output.push_str("## Graph Neighborhood Summary\n\n");
    output.push_str(&format!("{}\n\n", card.graph_neighborhood_summary));

    output.push_str("## Evidence\n\n");
    if card.evidence.is_empty() {
        output.push_str("No evidence items were generated.\n");
    } else {
        for evidence in &card.evidence {
            output.push_str(&format!(
                "- `{}` [{}] {} | Source: {}\n",
                evidence.evidence_id, evidence.evidence_type, evidence.description, evidence.source
            ));
        }
    }

    output.push_str("\n## Risk Explanation\n\n");
    if card.risk_explanations.is_empty() {
        output.push_str("No risk explanation was generated.\n");
    } else {
        for explanation in &card.risk_explanations {
            output.push_str(&format!(
                "- Score {} [{}]: {}\n",
                explanation.score, explanation.risk_level, explanation.explanation
            ));
        }
    }

    output.push_str("\n## Suggested Manual Validation Steps\n\n");
    if card.suggested_review_steps.is_empty() {
        output.push_str("- Validate the surrounding evidence manually before escalating.\n");
    } else {
        for step in &card.suggested_review_steps {
            output.push_str(&format!("- {}\n", step));
        }
    }

    output.push_str("\n## Caution Note\n\n");
    output.push_str(&format!("{}\n", card.caution_note));
    output
}

fn render_review_checklist_markdown(checklist: &ReviewChecklist) -> String {
    let mut output = String::new();
    output.push_str(&format!("# {}\n\n", checklist.title));
    for item in &checklist.items {
        output.push_str(&format!("- {}\n", item));
    }
    output.push_str(&format!("\n{}\n", checklist.caution_note));
    output
}

fn render_executive_summary_markdown(
    summary: &ReviewSummary,
    review_items: &[ReviewItem],
    bundles: &[ReviewAssetBundle],
    enrichment_summary: &str,
    api_intel: &ApiIntelData,
) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot Executive Summary\n\n");
    output.push_str(&format!(
        "ReconPilot analyzed {} enriched asset{} and produced {} review prioritization candidate{}.\n\n",
        summary.total_assets,
        plural(summary.total_assets),
        review_items.len(),
        plural(review_items.len())
    ));
    output.push_str("## What Was Analyzed\n\n");
    output.push_str(
        "This workspace used local enrichment artifacts, semantic classifications, graph-neighborhood observations, and deterministic risk explanations.\n\n",
    );

    output.push_str("## Most Interesting Categories\n\n");
    write_markdown_list(&mut output, &summary.top_roles);

    output.push_str("\n## Top Environments\n\n");
    write_markdown_list(&mut output, &summary.top_environments);

    output.push_str("\n## Top Areas For Review\n\n");
    write_markdown_list(&mut output, &summary.top_review_targets);

    let notable_observations = bundles
        .iter()
        .flat_map(|bundle| {
            bundle
                .observations
                .iter()
                .map(|observation| observation.description.clone())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(5)
        .collect::<Vec<_>>();
    output.push_str("\n## Notable Observations\n\n");
    write_markdown_list(&mut output, &notable_observations);

    output.push_str("\n## Prior Enrichment Context\n\n");
    let summary_line = enrichment_summary
        .lines()
        .find(|line| line.starts_with("- Assets:") || line.starts_with("- Observations:"))
        .unwrap_or("Semantic enrichment summary was included as input.");
    output.push_str(&format!("{summary_line}\n\n"));

    if api_intel.summary.is_some()
        || !api_intel.auth_observations.is_empty()
        || !api_intel.schemas.is_empty()
        || !api_intel.graphql_observations.is_empty()
    {
        output.push_str("## API Intelligence Context\n\n");
        output.push_str(&format!(
            "- Auth observations: {}\n- Schemas or docs: {}\n- GraphQL indicators: {}\n- JS observations: {}\n",
            api_intel.auth_observations.len(),
            api_intel.schemas.len(),
            api_intel.graphql_observations.len(),
            api_intel.js_observations.len()
        ));
        if let Some(summary) = &api_intel.summary {
            output.push_str(&format!(
                "- API endpoints: {}\n- API objects: {}\n- Privileged endpoint candidates: {}\n",
                summary.endpoint_count, summary.object_count, summary.privileged_endpoint_count
            ));
        }
        if let Some(markdown) = &api_intel.summary_markdown {
            if let Some(line) = markdown.lines().find(|line| {
                line.starts_with("This phase used local") || line.starts_with("- Endpoints:")
            }) {
                output.push_str(&format!("\n{line}\n"));
            }
        }
        output.push('\n');
    }

    output.push_str("## Important Note\n\n");
    output.push_str(
        "These results are recon-prioritization outputs only. They describe interesting candidates and areas worth review, not confirmed vulnerabilities.\n",
    );
    output
}

fn asset_node_ids(asset: &EnrichedAsset, enriched_graph: &EnrichedGraph) -> Vec<String> {
    let mut node_ids = enriched_graph
        .original_nodes
        .iter()
        .filter(|node| node.value == asset.asset)
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();

    for node_id in &asset.related_nodes {
        if !node_ids.iter().any(|existing| existing == node_id) {
            node_ids.push(node_id.clone());
        }
    }
    node_ids.sort();
    node_ids
}

fn related_edge_descriptions(
    asset_node_ids: &[String],
    related_nodes: &[String],
    enriched_graph: &EnrichedGraph,
) -> Vec<String> {
    let mut relevant_nodes = asset_node_ids.iter().cloned().collect::<BTreeSet<_>>();
    relevant_nodes.extend(related_nodes.iter().cloned());

    let mut edges = enriched_graph
        .original_edges
        .iter()
        .filter(|edge| {
            relevant_nodes.contains(&edge.source) || relevant_nodes.contains(&edge.target)
        })
        .map(|edge| {
            format!(
                "{} -> {} ({:?})",
                edge.source, edge.target, edge.relationship
            )
        })
        .collect::<Vec<_>>();
    edges.sort();
    edges.dedup();
    edges
}

fn distinct_sources(evidence: &[EvidenceItem]) -> BTreeSet<String> {
    evidence.iter().map(|item| item.source.clone()).collect()
}

fn dedupe_evidence_items(items: Vec<EvidenceItem>) -> Vec<EvidenceItem> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for item in items {
        let key = (
            item.source.clone(),
            item.evidence_type.clone(),
            item.description.clone(),
        );
        if seen.insert(key) {
            deduped.push(item);
        }
    }
    deduped
}

fn display_roles(roles: &[AssetRole]) -> String {
    let values = roles
        .iter()
        .filter(|role| **role != AssetRole::Unknown)
        .map(|role| role.as_str().to_string())
        .collect::<Vec<_>>();
    display_or_none(&values)
}

fn display_environments(environments: &[EnvironmentType]) -> String {
    let values = environments
        .iter()
        .filter(|environment| **environment != EnvironmentType::Unknown)
        .map(|environment| environment.as_str().to_string())
        .collect::<Vec<_>>();
    display_or_none(&values)
}

fn display_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
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

fn normalize_schema_endpoint_for_match(value: &str) -> String {
    let trimmed = value.trim();
    let candidate = trimmed
        .split_whitespace()
        .nth(1)
        .unwrap_or(trimmed)
        .to_string();
    let path = if candidate.starts_with("http://") || candidate.starts_with("https://") {
        url::Url::parse(&candidate)
            .ok()
            .map(|parsed| parsed.path().to_string())
            .unwrap_or(candidate)
    } else {
        candidate
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
            } else if segment.starts_with(':') {
                "{id}".to_string()
            } else if segment.chars().all(|character| character.is_ascii_digit()) {
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
        return url::Url::parse(value)
            .ok()
            .and_then(|parsed| parsed.host_str().map(ToOwned::to_owned));
    }
    None
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

fn evidence_id(asset: &str, ordinal: usize) -> String {
    format!("evidence:{}:{ordinal:03}", sanitize_asset_stem(asset))
}

fn fallback_risk(asset: &str) -> RiskExplanation {
    RiskExplanation {
        asset: asset.to_string(),
        risk_level: "informational".to_string(),
        score: 0,
        explanation:
            "This asset is currently an informational candidate and requires manual validation."
                .to_string(),
        contributing_factors: vec![
            "No deterministic risk explanation was present in the enrichment layer.".to_string(),
        ],
        recommended_next_steps: vec![
            "Validate the surrounding evidence manually before escalating.".to_string(),
        ],
    }
}

fn round_confidence(value: f32) -> f32 {
    (((value.clamp(0.35, 0.99)) * 100.0).round()) / 100.0
}

fn sanitize_asset_filename(asset: &str) -> String {
    format!("{}.md", sanitize_asset_stem(asset))
}

fn sanitize_asset_stem(asset: &str) -> String {
    let lowered = asset
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_ascii_lowercase();
    let mut stem = lowered
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    stem = stem.trim_matches('-').to_string();
    if stem.is_empty() {
        stem = "asset".to_string();
    }
    if stem.len() > 64 {
        stem.truncate(64);
        stem = stem.trim_matches('-').to_string();
    }

    let mut hasher = DefaultHasher::new();
    asset.hash(&mut hasher);
    let hash = format!("{:08x}", hasher.finish() as u32);
    format!("{stem}-{hash}")
}

fn plural(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
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
    use serde_json::Value;

    use super::{run_review_workspace, sanitize_asset_filename};

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
                "reconpilot-review-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            Ok(Self { root })
        }

        fn enrichment_dir(&self) -> PathBuf {
            self.root.join("output").join("enrichment")
        }

        fn review_dir(&self) -> PathBuf {
            self.root.join("output").join("review")
        }

        fn write_file(&self, relative: &str, content: &str) -> Result<()> {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, content)?;
            Ok(())
        }

        fn seed_minimal_inputs(&self) -> Result<()> {
            self.write_file(
                "output/enrichment/semantic-assets.json",
                r#"[{"asset":"internal-admin.example.com","semantic_tags":[{"tag":"internal","category":"environment","confidence":0.9,"evidence":["Matched keyword 'internal'"]},{"tag":"admin_dashboard","category":"role","confidence":0.9,"evidence":["Matched keyword 'admin'"]}],"roles":["admin_dashboard","authentication"],"environments":["internal","staging"],"risk_explanations":[{"asset":"internal-admin.example.com","risk_level":"high","score":62,"explanation":"Interesting candidate worth review.","contributing_factors":["Internal environment indicators","Administrative surface indicators"],"recommended_next_steps":["Review administrative surfaces carefully."]}],"related_nodes":["host:internal-admin","cluster:admin-surface"],"neighborhood_summary":"This asset shares infrastructure with 2 related hosts and belongs to admin-surface."},{"asset":"app.example.com","semantic_tags":[{"tag":"customer_app","category":"role","confidence":0.6,"evidence":["Matched keyword 'app'"]}],"roles":["customer_app"],"environments":["production"],"risk_explanations":[{"asset":"app.example.com","risk_level":"low","score":18,"explanation":"Interesting but lower priority candidate.","contributing_factors":["Customer-facing application indicators"],"recommended_next_steps":["Validate manually before escalating."]}],"related_nodes":["host:app"],"neighborhood_summary":"No notable graph-neighborhood relationships were observed beyond the base graph."}]"#,
            )?;
            self.write_file(
                "output/enrichment/semantic-observations.json",
                r#"[{"observation_id":"observation:internal-admin:001","asset":"internal-admin.example.com","observation_type":"neighborhood","description":"Shares infrastructure with privileged/admin-like asset","evidence":["Neighbor admin.example.com appears privileged"],"confidence":0.84,"related_nodes":["host:internal-admin","host:admin"]},{"observation_id":"observation:docs:001","asset":"app.example.com","observation_type":"documentation","description":"API schema/documentation candidate","evidence":["Endpoint '/swagger' matched API documentation intent"],"confidence":0.9,"related_nodes":["url:swagger"]}]"#,
            )?;
            self.write_file(
                "output/enrichment/risk-explanations.json",
                r#"[{"asset":"internal-admin.example.com","risk_level":"high","score":62,"explanation":"Interesting candidate worth review.","contributing_factors":["Internal environment indicators","Administrative surface indicators"],"recommended_next_steps":["Review administrative surfaces carefully."]},{"asset":"app.example.com","risk_level":"low","score":18,"explanation":"Interesting but lower priority candidate.","contributing_factors":["Customer-facing application indicators"],"recommended_next_steps":["Validate manually before escalating."]}]"#,
            )?;
            self.write_file(
                "output/enrichment/enriched-graph.json",
                r#"{"assets":[{"asset":"internal-admin.example.com","semantic_tags":[{"tag":"internal","category":"environment","confidence":0.9,"evidence":["Matched keyword 'internal'"]}],"roles":["admin_dashboard","authentication"],"environments":["internal","staging"],"risk_explanations":[{"asset":"internal-admin.example.com","risk_level":"high","score":62,"explanation":"Interesting candidate worth review.","contributing_factors":["Internal environment indicators"],"recommended_next_steps":["Review administrative surfaces carefully."]}],"related_nodes":["host:internal-admin","cluster:admin-surface"],"neighborhood_summary":"This asset shares infrastructure with 2 related hosts and belongs to admin-surface."},{"asset":"app.example.com","semantic_tags":[{"tag":"customer_app","category":"role","confidence":0.6,"evidence":["Matched keyword 'app'"]}],"roles":["customer_app"],"environments":["production"],"risk_explanations":[{"asset":"app.example.com","risk_level":"low","score":18,"explanation":"Interesting but lower priority candidate.","contributing_factors":["Customer-facing application indicators"],"recommended_next_steps":["Validate manually before escalating."]}],"related_nodes":["host:app"],"neighborhood_summary":"No notable graph-neighborhood relationships were observed beyond the base graph."}],"observations":[],"risk_explanations":[],"original_graph_summary":{"generated_at":"2026-05-14T09:00:00Z","node_count":4,"edge_count":3,"cluster_count":1,"anomaly_count":1,"top_technologies":[],"largest_clusters":[],"shared_infrastructure":[],"suspicious_naming":[],"likely_staging_systems":[],"likely_internal_systems":[],"redirect_chain_count":0},"original_nodes":[{"id":"host:internal-admin","node_type":"host","value":"internal-admin.example.com","tags":[],"metadata":{},"source_tools":["httpx"],"timestamps":["2026-05-14T09:00:00Z"]},{"id":"host:app","node_type":"host","value":"app.example.com","tags":[],"metadata":{},"source_tools":["httpx"],"timestamps":["2026-05-14T09:00:00Z"]},{"id":"host:admin","node_type":"host","value":"admin.example.com","tags":[],"metadata":{},"source_tools":["httpx"],"timestamps":["2026-05-14T09:00:00Z"]},{"id":"url:swagger","node_type":"url","value":"https://app.example.com/swagger","tags":[],"metadata":{},"source_tools":["katana"],"timestamps":["2026-05-14T09:00:00Z"]}],"original_edges":[{"source":"host:internal-admin","target":"host:admin","relationship":"shares_ip","confidence":0.95,"evidence":[],"timestamps":["2026-05-14T09:00:00Z"]},{"source":"host:app","target":"url:swagger","relationship":"hosts","confidence":0.95,"evidence":[],"timestamps":["2026-05-14T09:00:00Z"]},{"source":"host:internal-admin","target":"cluster:admin-surface","relationship":"belongs_to_cluster","confidence":0.9,"evidence":[],"timestamps":["2026-05-14T09:00:00Z"]}],"original_clusters":[{"cluster_id":"cluster:admin-surface","cluster_type":"admin-surface","related_nodes":["host:internal-admin","host:admin"],"shared_indicators":["admin-like"],"risk_score":70}],"semantic_summary":{"generated_at":"2026-05-14T09:00:00Z","asset_count":2,"observation_count":2,"risk_explanation_count":2,"top_roles":["admin_dashboard (1)"],"top_environments":["internal (1)"],"highest_priority_assets":["internal-admin.example.com [high:62]"],"notable_neighborhood_observations":["Shares infrastructure with privileged/admin-like asset"],"recommended_next_steps":["Review administrative surfaces carefully."]}}"#,
            )?;
            self.write_file(
                "output/enrichment/enrichment-summary.md",
                "# ReconPilot Semantic Enrichment Summary\n\n- Assets: 2\n- Observations: 2\n",
            )?;
            Ok(())
        }

        fn seed_api_intel_inputs(&self) -> Result<()> {
            self.write_file(
                "output/api-intel/api-endpoints.json",
                r#"[{"endpoint_id":"endpoint:get:/swagger","method":"GET","path":"https://app.example.com/swagger","normalized_path":"/swagger","parameters":[],"auth_indicators":["bearer_token"],"inferred_objects":[],"semantic_tags":[{"tag":"api_documentation","category":"endpoint_intent","confidence":0.95,"evidence":["Matched path segment '/swagger'"]}],"source":"graph.json"}]"#,
            )?;
            self.write_file(
                "output/api-intel/api-objects.json",
                r#"[{"object_name":"User","related_endpoints":["/users/{id}"],"related_parameters":["id"],"inferred_sensitivity":"high"}]"#,
            )?;
            self.write_file(
                "output/api-intel/api-relationships.json",
                r#"[{"source_endpoint":"endpoint:get:/swagger","target_object":"auth_flow:jwt_bearer","relationship_type":"related_to_auth_flow","confidence":0.8,"evidence":["Auth flow matched bearer references"]}]"#,
            )?;
            self.write_file(
                "output/api-intel/auth-observations.json",
                r#"[{"asset":"app.example.com","auth_type":"jwt_bearer","indicators":["bearer_token","jwt"],"confidence":0.82,"evidence":["Endpoint '/swagger' matched bearer references"]}]"#,
            )?;
            self.write_file(
                "output/api-intel/js-observations.json",
                r#"[{"js_file":"output/js/app.js","discovered_endpoints":["https://app.example.com/graphql"],"discovered_roles":["api_gateway"],"discovered_auth_indicators":["jwt"],"discovered_feature_flags":["beta"],"evidence":["JavaScript referenced endpoint '/graphql'"]}]"#,
            )?;
            self.write_file(
                "output/api-intel/schemas.json",
                r#"[{"schema_type":"openapi","schema_location":"https://app.example.com/swagger.json","detected_version":"3.0.2","endpoints":["GET /users/{id}"],"auth_methods":["bearerAuth"],"objects":["User"]}]"#,
            )?;
            self.write_file(
                "output/api-intel/graphql-observations.json",
                r#"[{"endpoint":"https://app.example.com/graphql","introspection_detected":false,"schema_indicators":["javascript-reference"],"auth_indicators":["jwt"],"notes":["JS referenced GraphQL endpoint"]}]"#,
            )?;
            self.write_file(
                "output/api-intel/api-graph.json",
                r#"{"nodes":[],"edges":[],"summary":{"generated_at":"2026-05-14T09:00:00Z","endpoint_count":1,"object_count":1,"relationship_count":1,"schema_count":1,"auth_observation_count":1,"graphql_observation_count":1,"js_observation_count":1,"api_family_count":1,"privileged_endpoint_count":1,"top_auth_styles":["jwt_bearer (1)"],"likely_sensitive_objects":["User (high)"],"hidden_route_candidates":["https://app.example.com/graphql"]}}"#,
            )?;
            self.write_file(
                "output/api-intel/api-summary.md",
                "# ReconPilot API & JavaScript Intelligence Summary\n\nThis phase used local ReconPilot artifacts only.\n",
            )?;
            Ok(())
        }

        fn seed_enrichment_with_embedded_api_evidence(&self) -> Result<()> {
            self.write_file(
                "output/enrichment/semantic-assets.json",
                r#"[{"asset":"app.example.com","semantic_tags":[{"tag":"api_documentation","category":"endpoint_intent","confidence":0.95,"evidence":["Matched '/swagger'"]}],"roles":["documentation","api_gateway","authentication"],"environments":["production"],"risk_explanations":[{"asset":"app.example.com","risk_level":"medium","score":48,"explanation":"Interesting candidate worth manual review.","contributing_factors":["API documentation or schema exposure indicators","Auth-related API surface indicators"],"recommended_next_steps":["Review API documentation carefully.","Review local auth-surface evidence carefully."]}],"related_nodes":["host:app"],"api_endpoints":[{"endpoint_id":"endpoint:get:/swagger","method":"GET","path":"https://app.example.com/swagger","normalized_path":"/swagger","parameters":[],"auth_indicators":["bearer_token"],"inferred_objects":["User"],"semantic_tags":[{"tag":"api_documentation","category":"endpoint_intent","confidence":0.95,"evidence":["Matched '/swagger'"]}],"source":"graph.json"}],"api_objects":[{"object_name":"User","related_endpoints":["/swagger"],"related_parameters":[],"inferred_sensitivity":"high"}],"auth_observations":[{"asset":"app.example.com","auth_type":"jwt_bearer","indicators":["bearer_token","jwt"],"confidence":0.82,"evidence":["Endpoint '/swagger' matched bearer references"]}],"js_observations":[{"js_file":"output/js/app.js","discovered_endpoints":["https://app.example.com/graphql"],"discovered_roles":["api_gateway"],"discovered_auth_indicators":["jwt"],"discovered_feature_flags":["beta"],"evidence":["JavaScript referenced endpoint '/graphql'"]}],"schema_observations":[{"schema_type":"openapi","schema_location":"https://app.example.com/swagger.json","detected_version":"3.0.2","endpoints":["GET /swagger"],"auth_methods":["bearerAuth"],"objects":["User"]}],"graphql_observations":[{"endpoint":"https://app.example.com/graphql","introspection_detected":false,"schema_indicators":["javascript-reference"],"auth_indicators":["jwt"],"notes":["JS referenced GraphQL endpoint"]}],"neighborhood_summary":"This asset references 1 API endpoint candidate and includes auth flow indicators."}]"#,
            )?;
            self.write_file("output/enrichment/semantic-observations.json", "[]")?;
            self.write_file(
                "output/enrichment/risk-explanations.json",
                r#"[{"asset":"app.example.com","risk_level":"medium","score":48,"explanation":"Interesting candidate worth manual review.","contributing_factors":["API documentation or schema exposure indicators","Auth-related API surface indicators"],"recommended_next_steps":["Review API documentation carefully.","Review local auth-surface evidence carefully."]}]"#,
            )?;
            self.write_file(
                "output/enrichment/enriched-graph.json",
                r#"{"assets":[{"asset":"app.example.com","semantic_tags":[{"tag":"api_documentation","category":"endpoint_intent","confidence":0.95,"evidence":["Matched '/swagger'"]}],"roles":["documentation","api_gateway","authentication"],"environments":["production"],"risk_explanations":[{"asset":"app.example.com","risk_level":"medium","score":48,"explanation":"Interesting candidate worth manual review.","contributing_factors":["API documentation or schema exposure indicators","Auth-related API surface indicators"],"recommended_next_steps":["Review API documentation carefully.","Review local auth-surface evidence carefully."]}],"related_nodes":["host:app"],"api_endpoints":[{"endpoint_id":"endpoint:get:/swagger","method":"GET","path":"https://app.example.com/swagger","normalized_path":"/swagger","parameters":[],"auth_indicators":["bearer_token"],"inferred_objects":["User"],"semantic_tags":[{"tag":"api_documentation","category":"endpoint_intent","confidence":0.95,"evidence":["Matched '/swagger'"]}],"source":"graph.json"}],"api_objects":[{"object_name":"User","related_endpoints":["/swagger"],"related_parameters":[],"inferred_sensitivity":"high"}],"auth_observations":[{"asset":"app.example.com","auth_type":"jwt_bearer","indicators":["bearer_token","jwt"],"confidence":0.82,"evidence":["Endpoint '/swagger' matched bearer references"]}],"js_observations":[{"js_file":"output/js/app.js","discovered_endpoints":["https://app.example.com/graphql"],"discovered_roles":["api_gateway"],"discovered_auth_indicators":["jwt"],"discovered_feature_flags":["beta"],"evidence":["JavaScript referenced endpoint '/graphql'"]}],"schema_observations":[{"schema_type":"openapi","schema_location":"https://app.example.com/swagger.json","detected_version":"3.0.2","endpoints":["GET /swagger"],"auth_methods":["bearerAuth"],"objects":["User"]}],"graphql_observations":[{"endpoint":"https://app.example.com/graphql","introspection_detected":false,"schema_indicators":["javascript-reference"],"auth_indicators":["jwt"],"notes":["JS referenced GraphQL endpoint"]}],"neighborhood_summary":"This asset references 1 API endpoint candidate and includes auth flow indicators."}],"observations":[],"risk_explanations":[{"asset":"app.example.com","risk_level":"medium","score":48,"explanation":"Interesting candidate worth manual review.","contributing_factors":["API documentation or schema exposure indicators","Auth-related API surface indicators"],"recommended_next_steps":["Review API documentation carefully.","Review local auth-surface evidence carefully."]}],"original_graph_summary":{"generated_at":"2026-05-14T09:00:00Z","node_count":1,"edge_count":0,"cluster_count":0,"anomaly_count":0,"top_technologies":[],"largest_clusters":[],"shared_infrastructure":[],"suspicious_naming":[],"likely_staging_systems":[],"likely_internal_systems":[],"redirect_chain_count":0},"original_nodes":[{"id":"host:app","node_type":"host","value":"app.example.com","tags":[],"metadata":{},"source_tools":["httpx"],"timestamps":["2026-05-14T09:00:00Z"]}],"original_edges":[],"original_clusters":[],"semantic_summary":{"generated_at":"2026-05-14T09:00:00Z","asset_count":1,"observation_count":0,"risk_explanation_count":1,"api_endpoint_count":1,"auth_surface_count":1,"js_observation_count":1,"schema_observation_count":1,"graphql_observation_count":1,"top_roles":["api_gateway (1)"],"top_environments":["production (1)"],"highest_priority_assets":["app.example.com [medium:48]"],"notable_neighborhood_observations":[],"sensitive_object_candidates":["app.example.com -> User (high)"],"api_intelligence_warnings":[],"recommended_next_steps":["Review API documentation carefully."]}}"#,
            )?;
            self.write_file(
                "output/enrichment/enrichment-summary.md",
                "# ReconPilot Semantic Enrichment Summary\n\n- Assets: 1\n- Observations: 0\n",
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
    fn review_queue_generation_creates_outputs() -> Result<()> {
        let workspace = TestWorkspace::new("queue")?;
        workspace.seed_minimal_inputs()?;

        let outcome = run_review_workspace(&workspace.enrichment_dir(), &workspace.review_dir())?;
        assert!(outcome.priority_queue_markdown_path.exists());
        assert!(outcome.priority_queue_json_path.exists());
        assert!(outcome.asset_cards_dir.exists());
        assert!(outcome.review_checklist_path.exists());
        assert!(outcome.executive_summary_path.exists());
        assert!(outcome.evidence_index_path.exists());
        Ok(())
    }

    #[test]
    fn ranking_behavior_prioritizes_privileged_assets() -> Result<()> {
        let workspace = TestWorkspace::new("ranking")?;
        workspace.seed_minimal_inputs()?;

        let outcome = run_review_workspace(&workspace.enrichment_dir(), &workspace.review_dir())?;
        let queue = fs::read_to_string(outcome.priority_queue_json_path)?;
        let value: Value = serde_json::from_str(&queue)?;
        let first_asset = value["items"][0]["asset"]
            .as_str()
            .expect("first ranked asset should exist");
        assert_eq!(first_asset, "internal-admin.example.com");
        Ok(())
    }

    #[test]
    fn asset_card_generation_creates_sanitized_files() -> Result<()> {
        let workspace = TestWorkspace::new("cards")?;
        workspace.seed_minimal_inputs()?;

        let outcome = run_review_workspace(&workspace.enrichment_dir(), &workspace.review_dir())?;
        let files = fs::read_dir(outcome.asset_cards_dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(files
            .iter()
            .any(|file| file.contains("internal-admin-example-com")));
        let card_content = fs::read_to_string(
            workspace.review_dir().join("asset-cards").join(
                files
                    .into_iter()
                    .find(|file| file.contains("internal-admin-example-com"))
                    .expect("sanitized asset card filename should exist"),
            ),
        )?;
        assert!(card_content.contains("Suggested Manual Validation Steps"));
        Ok(())
    }

    #[test]
    fn evidence_index_generation_contains_review_mapping() -> Result<()> {
        let workspace = TestWorkspace::new("evidence")?;
        workspace.seed_minimal_inputs()?;

        let outcome = run_review_workspace(&workspace.enrichment_dir(), &workspace.review_dir())?;
        let evidence = fs::read_to_string(outcome.evidence_index_path)?;
        let value: Value = serde_json::from_str(&evidence)?;
        assert!(value["items"]
            .as_array()
            .expect("items should be an array")
            .iter()
            .any(|item| item["review_item"] == "internal-admin.example.com"));
        Ok(())
    }

    #[test]
    fn review_workspace_absorbs_api_intelligence_artifacts() -> Result<()> {
        let workspace = TestWorkspace::new("api-intel")?;
        workspace.seed_minimal_inputs()?;
        workspace.seed_api_intel_inputs()?;

        let outcome = run_review_workspace(&workspace.enrichment_dir(), &workspace.review_dir())?;
        let queue = fs::read_to_string(outcome.priority_queue_markdown_path)?;
        assert!(queue.contains("Auth-related API observations increase review priority"));

        let cards = fs::read_dir(workspace.review_dir().join("asset-cards"))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        let app_card = cards
            .iter()
            .find(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().contains("app-example-com"))
                    .unwrap_or(false)
            })
            .expect("app asset card should exist");
        let content = fs::read_to_string(app_card)?;
        assert!(content.contains("auth-observations.json"));
        assert!(content.contains("schemas.json"));
        Ok(())
    }

    #[test]
    fn review_avoids_duplicate_api_evidence_when_enrichment_already_contains_it() -> Result<()> {
        let workspace = TestWorkspace::new("dedupe-api-evidence")?;
        workspace.seed_enrichment_with_embedded_api_evidence()?;
        workspace.seed_api_intel_inputs()?;

        let outcome = run_review_workspace(&workspace.enrichment_dir(), &workspace.review_dir())?;
        let cards = fs::read_dir(outcome.asset_cards_dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        let app_card = cards
            .iter()
            .find(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().contains("app-example-com"))
                    .unwrap_or(false)
            })
            .expect("app asset card should exist");
        let content = fs::read_to_string(app_card)?;
        assert_eq!(content.matches("auth-observations.json").count(), 1);
        assert_eq!(content.matches("schemas.json").count(), 1);
        Ok(())
    }

    #[test]
    fn missing_input_handling_returns_error() -> Result<()> {
        let workspace = TestWorkspace::new("missing")?;
        fs::create_dir_all(workspace.enrichment_dir())?;
        let result = run_review_workspace(&workspace.enrichment_dir(), &workspace.review_dir());
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn empty_enrichment_handling_still_writes_workspace() -> Result<()> {
        let workspace = TestWorkspace::new("empty")?;
        workspace.write_file("output/enrichment/semantic-assets.json", "[]")?;
        workspace.write_file("output/enrichment/semantic-observations.json", "[]")?;
        workspace.write_file("output/enrichment/risk-explanations.json", "[]")?;
        workspace.write_file(
            "output/enrichment/enriched-graph.json",
            r#"{"assets":[],"observations":[],"risk_explanations":[],"original_graph_summary":{"generated_at":"2026-05-14T09:00:00Z","node_count":0,"edge_count":0,"cluster_count":0,"anomaly_count":0,"top_technologies":[],"largest_clusters":[],"shared_infrastructure":[],"suspicious_naming":[],"likely_staging_systems":[],"likely_internal_systems":[],"redirect_chain_count":0},"original_nodes":[],"original_edges":[],"original_clusters":[],"semantic_summary":{"generated_at":"2026-05-14T09:00:00Z","asset_count":0,"observation_count":0,"risk_explanation_count":0,"top_roles":[],"top_environments":[],"highest_priority_assets":[],"notable_neighborhood_observations":[],"recommended_next_steps":[]}}"#,
        )?;
        workspace.write_file(
            "output/enrichment/enrichment-summary.md",
            "# ReconPilot Semantic Enrichment Summary\n\nNone observed yet.\n",
        )?;

        let outcome = run_review_workspace(&workspace.enrichment_dir(), &workspace.review_dir())?;
        let queue = fs::read_to_string(outcome.priority_queue_markdown_path)?;
        assert!(queue.contains("No review items were generated"));
        Ok(())
    }

    #[test]
    fn filename_sanitization_is_safe() {
        let filename =
            sanitize_asset_filename("https://Portal.Example.com/admin/login?next=/billing");
        assert!(filename.ends_with(".md"));
        assert!(!filename.contains('/'));
        assert!(!filename.contains('\\'));
        assert!(!filename.contains(':'));
        assert!(filename.contains("portal-example-com-admin-login-next--billing"));
    }
}
