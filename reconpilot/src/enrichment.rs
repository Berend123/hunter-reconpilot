use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::Deserialize;

use crate::{
    correlation::GraphAnomaly,
    models::{
        ApiGraphSummary, ApiIntelBundle, AssetCluster, GraphEdge, GraphNode, GraphSummary,
        SemanticSummary,
    },
    semantic::{self, SemanticAnalysis},
    utils,
};

#[derive(Debug, Clone)]
pub struct EnrichmentOutcome {
    pub semantic_assets_path: PathBuf,
    pub observations_path: PathBuf,
    pub risk_explanations_path: PathBuf,
    pub enriched_graph_path: PathBuf,
    pub summary_path: PathBuf,
    pub warnings: Vec<String>,
    pub summary: SemanticSummary,
}

#[derive(Debug, Deserialize, Default)]
struct ImportedGraphDocument {
    #[serde(default)]
    nodes: Vec<GraphNode>,
    #[serde(default)]
    edges: Vec<GraphEdge>,
    #[serde(default)]
    clusters: Vec<AssetCluster>,
    #[serde(default)]
    summary: Option<GraphSummary>,
}

#[derive(Debug, Deserialize, Default)]
struct ImportedClustersDocument {
    #[serde(default)]
    clusters: Vec<AssetCluster>,
}

#[derive(Debug, Deserialize, Default)]
struct ImportedAnomaliesDocument {
    #[serde(default)]
    anomalies: Vec<GraphAnomaly>,
}

#[derive(Debug, Deserialize, Default)]
struct ImportedApiGraphDocument {
    #[serde(default)]
    summary: Option<ApiGraphSummary>,
}

pub fn run_enrichment_engine(
    input: &Path,
    api_intel: Option<&Path>,
    out: &Path,
) -> Result<EnrichmentOutcome> {
    validate_enrichment_input(input)?;
    utils::ensure_directory(out)?;

    let graph = load_graph_document(input)?;
    let clusters = load_clusters(input, &graph)?;
    let anomalies = load_anomalies(input)?;
    let summary = load_graph_summary(input, &graph, &clusters, &anomalies);
    let api_bundle = load_api_intel_bundle(api_intel)?;
    let analysis = if let Some(api_bundle) = api_bundle.as_ref() {
        semantic::analyze_graph_with_api(
            &graph.nodes,
            &graph.edges,
            &clusters,
            &anomalies,
            &summary,
            Some(api_bundle),
        )
    } else {
        semantic::analyze_graph(&graph.nodes, &graph.edges, &clusters, &anomalies, &summary)
    };

    write_outputs(out, &analysis)
}

pub fn print_enrichment_summary(
    input: &Path,
    api_intel: Option<&Path>,
    out: &Path,
    outcome: &EnrichmentOutcome,
) {
    println!("ReconPilot enrichment summary");
    println!("Input maps: {}", input.display());
    if let Some(api_intel) = api_intel {
        println!("API intelligence: {}", api_intel.display());
    }
    println!("Output enrichment: {}", out.display());
    println!("Assets enriched: {}", outcome.summary.asset_count);
    println!("Observations: {}", outcome.summary.observation_count);
    println!(
        "Risk explanations: {}",
        outcome.summary.risk_explanation_count
    );
    println!("Top roles: {}", display_or_none(&outcome.summary.top_roles));
    println!(
        "Top environments: {}",
        display_or_none(&outcome.summary.top_environments)
    );
    println!("Outputs:");
    println!("  - {}", outcome.semantic_assets_path.display());
    println!("  - {}", outcome.observations_path.display());
    println!("  - {}", outcome.risk_explanations_path.display());
    println!("  - {}", outcome.enriched_graph_path.display());
    println!("  - {}", outcome.summary_path.display());
    for warning in &outcome.warnings {
        println!("Warning: {warning}");
    }
}

fn validate_enrichment_input(input: &Path) -> Result<()> {
    if !input.exists() {
        bail!(
            "enrichment input directory does not exist: {}",
            input.display()
        );
    }
    if !input.is_dir() {
        bail!(
            "enrichment input path is not a directory: {}",
            input.display()
        );
    }

    let graph_path = input.join("graph.json");
    if !graph_path.exists() {
        bail!(
            "required graph input is missing: {}. Run `reconpilot graph --input output/ --out output/maps/ --execute` first.",
            graph_path.display()
        );
    }

    Ok(())
}

fn load_graph_document(input: &Path) -> Result<ImportedGraphDocument> {
    let path = input.join("graph.json");
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read graph input: {}", path.display()))?;
    serde_json::from_str::<ImportedGraphDocument>(&raw)
        .with_context(|| format!("failed to parse graph input: {}", path.display()))
}

fn load_clusters(input: &Path, graph: &ImportedGraphDocument) -> Result<Vec<AssetCluster>> {
    let path = input.join("clusters.json");
    if !path.exists() {
        return Ok(graph.clusters.clone());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read clusters input: {}", path.display()))?;
    let document = serde_json::from_str::<ImportedClustersDocument>(&raw)
        .with_context(|| format!("failed to parse clusters input: {}", path.display()))?;
    Ok(document.clusters)
}

fn load_anomalies(input: &Path) -> Result<Vec<GraphAnomaly>> {
    let path = input.join("anomalies.json");
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read anomalies input: {}", path.display()))?;
    let document = serde_json::from_str::<ImportedAnomaliesDocument>(&raw)
        .with_context(|| format!("failed to parse anomalies input: {}", path.display()))?;
    Ok(document.anomalies)
}

fn load_graph_summary(
    input: &Path,
    graph: &ImportedGraphDocument,
    clusters: &[AssetCluster],
    anomalies: &[GraphAnomaly],
) -> GraphSummary {
    let summary_path = input.join("graph-summary.json");
    if summary_path.exists() {
        if let Ok(raw) = fs::read_to_string(&summary_path) {
            if let Ok(summary) = serde_json::from_str::<GraphSummary>(&raw) {
                return summary;
            }
        }
    }

    graph.summary.clone().unwrap_or_else(|| GraphSummary {
        generated_at: Utc::now(),
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        cluster_count: clusters.len(),
        anomaly_count: anomalies.len(),
        top_technologies: Vec::new(),
        largest_clusters: Vec::new(),
        shared_infrastructure: Vec::new(),
        suspicious_naming: Vec::new(),
        likely_staging_systems: Vec::new(),
        likely_internal_systems: Vec::new(),
        redirect_chain_count: graph
            .edges
            .iter()
            .filter(|edge| edge.relationship == crate::models::RelationshipType::RedirectsTo)
            .count(),
    })
}

fn load_api_intel_bundle(api_intel: Option<&Path>) -> Result<Option<ApiIntelBundle>> {
    let Some(api_intel) = api_intel else {
        return Ok(None);
    };
    if !api_intel.exists() {
        bail!(
            "api-intel input directory does not exist: {}",
            api_intel.display()
        );
    }
    if !api_intel.is_dir() {
        bail!(
            "api-intel input path is not a directory: {}",
            api_intel.display()
        );
    }

    let mut bundle = ApiIntelBundle::default();

    bundle.endpoints =
        load_optional_api_json(api_intel, "api-endpoints.json", &mut bundle.warnings)?;
    bundle.objects = load_optional_api_json(api_intel, "api-objects.json", &mut bundle.warnings)?;
    bundle.relationships =
        load_optional_api_json(api_intel, "api-relationships.json", &mut bundle.warnings)?;
    bundle.auth_observations =
        load_optional_api_json(api_intel, "auth-observations.json", &mut bundle.warnings)?;
    bundle.js_observations =
        load_optional_api_json(api_intel, "js-observations.json", &mut bundle.warnings)?;
    bundle.schemas = load_optional_api_json(api_intel, "schemas.json", &mut bundle.warnings)?;
    bundle.graphql_observations =
        load_optional_api_json(api_intel, "graphql-observations.json", &mut bundle.warnings)?;

    let api_graph_path = api_intel.join("api-graph.json");
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
        bundle.warnings.push(format!(
            "Optional API-intel artifact missing: {}",
            api_graph_path.display()
        ));
    }

    let api_summary_path = api_intel.join("api-summary.md");
    if api_summary_path.exists() {
        bundle.summary_markdown =
            Some(fs::read_to_string(&api_summary_path).with_context(|| {
                format!(
                    "failed to read API intelligence summary input: {}",
                    api_summary_path.display()
                )
            })?);
    } else {
        bundle.warnings.push(format!(
            "Optional API-intel artifact missing: {}",
            api_summary_path.display()
        ));
    }

    Ok(Some(bundle))
}

fn load_optional_api_json<T: serde::de::DeserializeOwned + Default>(
    root: &Path,
    file_name: &str,
    warnings: &mut Vec<String>,
) -> Result<T> {
    let path = root.join(file_name);
    if !path.exists() {
        warnings.push(format!(
            "Optional API-intel artifact missing: {}",
            path.display()
        ));
        return Ok(T::default());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read API-intel input: {}", path.display()))?;
    serde_json::from_str::<T>(&raw)
        .with_context(|| format!("failed to parse API-intel input: {}", path.display()))
}

fn write_outputs(out: &Path, analysis: &SemanticAnalysis) -> Result<EnrichmentOutcome> {
    let semantic_assets_path = out.join("semantic-assets.json");
    let observations_path = out.join("semantic-observations.json");
    let risk_explanations_path = out.join("risk-explanations.json");
    let enriched_graph_path = out.join("enriched-graph.json");
    let summary_path = out.join("enrichment-summary.md");

    utils::write_json_pretty(&semantic_assets_path, &analysis.semantic_assets)?;
    utils::write_json_pretty(&observations_path, &analysis.observations)?;
    utils::write_json_pretty(&risk_explanations_path, &analysis.risk_explanations)?;
    utils::write_json_pretty(&enriched_graph_path, &analysis.enriched_graph)?;
    utils::write_string(&summary_path, &render_summary_markdown(analysis))?;

    Ok(EnrichmentOutcome {
        semantic_assets_path,
        observations_path,
        risk_explanations_path,
        enriched_graph_path,
        summary_path,
        warnings: analysis.summary.api_intelligence_warnings.clone(),
        summary: analysis.summary.clone(),
    })
}

fn render_summary_markdown(analysis: &SemanticAnalysis) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot Semantic Enrichment Summary\n\n");
    output.push_str(&format!(
        "- Generated at: {}\n- Assets: {}\n- Observations: {}\n- Risk explanations: {}\n\n",
        analysis.summary.generated_at.to_rfc3339(),
        analysis.summary.asset_count,
        analysis.summary.observation_count,
        analysis.summary.risk_explanation_count
    ));

    output.push_str("## Top Semantic Roles\n\n");
    write_markdown_list(&mut output, &analysis.summary.top_roles);

    output.push_str("\n## Top Environments\n\n");
    write_markdown_list(&mut output, &analysis.summary.top_environments);

    output.push_str("\n## Highest Priority Assets\n\n");
    write_markdown_list(&mut output, &analysis.summary.highest_priority_assets);

    output.push_str("\n## Notable Graph-Neighborhood Observations\n\n");
    write_markdown_list(
        &mut output,
        &analysis.summary.notable_neighborhood_observations,
    );

    output.push_str("\n## API Intelligence Summary\n\n");
    output.push_str(&format!(
        "- API endpoints: {}\n- Auth surfaces: {}\n- JavaScript observations: {}\n- Schema observations: {}\n- GraphQL observations: {}\n\n",
        analysis.summary.api_endpoint_count,
        analysis.summary.auth_surface_count,
        analysis.summary.js_observation_count,
        analysis.summary.schema_observation_count,
        analysis.summary.graphql_observation_count
    ));

    output.push_str("## Auth Surface Summary\n\n");
    write_markdown_list(
        &mut output,
        &analysis
            .semantic_assets
            .iter()
            .filter(|asset| !asset.auth_observations.is_empty())
            .map(|asset| {
                format!(
                    "{} -> {}",
                    asset.asset,
                    asset
                        .auth_observations
                        .iter()
                        .map(|observation| observation.auth_type.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>(),
    );

    output.push_str("\n## JavaScript-Derived Observations\n\n");
    write_markdown_list(
        &mut output,
        &analysis
            .semantic_assets
            .iter()
            .flat_map(|asset| {
                asset.js_observations.iter().map(|observation| {
                    format!(
                        "{} -> {} endpoint candidate{} from {}",
                        asset.asset,
                        observation.discovered_endpoints.len(),
                        plural(observation.discovered_endpoints.len()),
                        observation.js_file
                    )
                })
            })
            .collect::<Vec<_>>(),
    );

    output.push_str("\n## Schema and Documentation Observations\n\n");
    write_markdown_list(
        &mut output,
        &analysis
            .semantic_assets
            .iter()
            .flat_map(|asset| {
                asset.schema_observations.iter().map(|schema| {
                    format!(
                        "{} -> {} [{}]",
                        asset.asset, schema.schema_location, schema.schema_type
                    )
                })
            })
            .collect::<Vec<_>>(),
    );

    output.push_str("\n## GraphQL Observations\n\n");
    write_markdown_list(
        &mut output,
        &analysis
            .semantic_assets
            .iter()
            .flat_map(|asset| {
                asset
                    .graphql_observations
                    .iter()
                    .map(|observation| format!("{} -> {}", asset.asset, observation.endpoint))
            })
            .collect::<Vec<_>>(),
    );

    output.push_str("\n## Sensitive Object Candidates\n\n");
    write_markdown_list(&mut output, &analysis.summary.sensitive_object_candidates);

    output.push_str("\n## Recommended Next Manual Review Steps\n\n");
    write_markdown_list(&mut output, &analysis.summary.recommended_next_steps);

    if !analysis.summary.api_intelligence_warnings.is_empty() {
        output.push_str("\n## API Intelligence Warnings\n\n");
        write_markdown_list(&mut output, &analysis.summary.api_intelligence_warnings);
    }

    output.push_str("\n## Reminder\n\n");
    output.push_str(
        "These outputs describe interesting candidates and graph context. They are not vulnerability claims and require manual validation.\n",
    );
    output
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

fn display_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
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

    use super::run_enrichment_engine;

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
                "reconpilot-enrich-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            Ok(Self { root })
        }

        fn maps_dir(&self) -> PathBuf {
            self.root.join("output").join("maps")
        }

        fn enrichment_dir(&self) -> PathBuf {
            self.root.join("output").join("enrichment")
        }

        fn api_intel_dir(&self) -> PathBuf {
            self.root.join("output").join("api-intel")
        }

        fn write_file(&self, relative: &str, content: &str) -> Result<()> {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, content)?;
            Ok(())
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn missing_graph_input_handling_returns_error() -> Result<()> {
        let workspace = TestWorkspace::new("missing-graph")?;
        fs::create_dir_all(workspace.maps_dir())?;
        let result =
            run_enrichment_engine(&workspace.maps_dir(), None, &workspace.enrichment_dir());
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn empty_graph_handling_writes_empty_outputs() -> Result<()> {
        let workspace = TestWorkspace::new("empty-graph")?;
        workspace.write_file(
            "output/maps/graph.json",
            r#"{"nodes":[],"edges":[],"clusters":[]}"#,
        )?;

        let outcome =
            run_enrichment_engine(&workspace.maps_dir(), None, &workspace.enrichment_dir())?;
        assert!(outcome.semantic_assets_path.exists());
        assert!(outcome.enriched_graph_path.exists());

        let summary = fs::read_to_string(outcome.summary_path)?;
        assert!(summary.contains("None observed yet"));
        Ok(())
    }

    #[test]
    fn enrich_without_api_intel_remains_unchanged() -> Result<()> {
        let workspace = TestWorkspace::new("base-unchanged")?;
        workspace.write_file(
            "output/maps/graph.json",
            r#"{"nodes":[{"id":"host:app","node_type":"host","value":"app.example.com","tags":[],"metadata":{},"source_tools":["httpx"],"timestamps":["2026-05-14T09:00:00Z"]}],"edges":[],"clusters":[]}"#,
        )?;

        let outcome =
            run_enrichment_engine(&workspace.maps_dir(), None, &workspace.enrichment_dir())?;
        let assets = fs::read_to_string(outcome.semantic_assets_path)?;
        assert!(!assets.contains("\"api_endpoints\":[{"));
        Ok(())
    }

    #[test]
    fn enrich_with_valid_api_intel_adds_api_observations() -> Result<()> {
        let workspace = TestWorkspace::new("with-api")?;
        workspace.write_file(
            "output/maps/graph.json",
            r#"{"nodes":[{"id":"host:app","node_type":"host","value":"app.example.com","tags":[],"metadata":{},"source_tools":["httpx"],"timestamps":["2026-05-14T09:00:00Z"]}],"edges":[],"clusters":[]}"#,
        )?;
        workspace.write_file(
            "output/api-intel/api-endpoints.json",
            r#"[{"endpoint_id":"endpoint:get:/swagger","method":"GET","path":"https://app.example.com/swagger","normalized_path":"/swagger","parameters":[],"auth_indicators":["bearer_token"],"inferred_objects":["User"],"semantic_tags":[{"tag":"api_documentation","category":"endpoint_intent","confidence":0.95,"evidence":["Matched '/swagger'"]}],"source":"graph.json"}]"#,
        )?;
        workspace.write_file(
            "output/api-intel/api-objects.json",
            r#"[{"object_name":"User","related_endpoints":["/swagger"],"related_parameters":[],"inferred_sensitivity":"high"}]"#,
        )?;
        workspace.write_file(
            "output/api-intel/auth-observations.json",
            r#"[{"asset":"app.example.com","auth_type":"jwt_bearer","indicators":["bearer_token","jwt"],"confidence":0.88,"evidence":["Bearer token reference"]}]"#,
        )?;
        workspace.write_file(
            "output/api-intel/js-observations.json",
            r#"[{"js_file":"output/js/app.js","discovered_endpoints":["https://app.example.com/internal/graphql"],"discovered_roles":["api_gateway"],"discovered_auth_indicators":["jwt"],"discovered_feature_flags":["adminOnlyBeta"],"evidence":["JS referenced internal GraphQL route"]}]"#,
        )?;
        workspace.write_file(
            "output/api-intel/schemas.json",
            r#"[{"schema_type":"openapi","schema_location":"https://app.example.com/swagger.json","detected_version":"3.0.2","endpoints":["GET /swagger"],"auth_methods":["bearerAuth"],"objects":["User"]}]"#,
        )?;
        workspace.write_file(
            "output/api-intel/graphql-observations.json",
            r#"[{"endpoint":"https://app.example.com/internal/graphql","introspection_detected":false,"schema_indicators":["apollo"],"auth_indicators":["jwt"],"notes":["GraphQL candidate from local JS"]}]"#,
        )?;
        workspace.write_file(
            "output/api-intel/api-graph.json",
            r#"{"summary":{"generated_at":"2026-05-14T09:00:00Z","endpoint_count":1,"object_count":1,"relationship_count":0,"schema_count":1,"auth_observation_count":1,"graphql_observation_count":1,"js_observation_count":1,"api_family_count":1,"privileged_endpoint_count":1,"top_auth_styles":["jwt_bearer (1)"],"likely_sensitive_objects":["User (high)"],"hidden_route_candidates":["https://app.example.com/internal/graphql"]}}"#,
        )?;
        workspace.write_file(
            "output/api-intel/api-summary.md",
            "# API Summary\n\nLocal-only.\n",
        )?;

        let outcome = run_enrichment_engine(
            &workspace.maps_dir(),
            Some(&workspace.api_intel_dir()),
            &workspace.enrichment_dir(),
        )?;
        let observations = fs::read_to_string(outcome.observations_path)?;
        let risks = fs::read_to_string(outcome.risk_explanations_path)?;
        let assets = fs::read_to_string(outcome.semantic_assets_path)?;
        let summary = fs::read_to_string(outcome.summary_path)?;
        assert!(observations.contains("Auth-related API surface candidate"));
        assert!(observations.contains("JavaScript discovered hidden or internal route candidate"));
        assert!(observations.contains("GraphQL surface candidate"));
        assert!(risks.contains("Auth-related API surface indicators"));
        assert!(risks.contains("API documentation or schema exposure indicators"));
        assert!(risks.contains("Potentially sensitive API object references"));
        assert!(assets.contains("\"api_endpoints\""));
        assert!(summary.contains("API Intelligence Summary"));
        assert!(summary.contains("Sensitive Object Candidates"));
        Ok(())
    }

    #[test]
    fn malformed_api_intel_fails_clearly() -> Result<()> {
        let workspace = TestWorkspace::new("malformed-api")?;
        workspace.write_file(
            "output/maps/graph.json",
            r#"{"nodes":[],"edges":[],"clusters":[]}"#,
        )?;
        workspace.write_file("output/api-intel/api-endpoints.json", "{bad-json")?;

        let result = run_enrichment_engine(
            &workspace.maps_dir(),
            Some(&workspace.api_intel_dir()),
            &workspace.enrichment_dir(),
        );
        assert!(result.is_err());
        let error = format!("{}", result.expect_err("malformed API-intel should fail"));
        assert!(error.contains("api-endpoints.json"));
        Ok(())
    }

    #[test]
    fn missing_optional_api_intel_files_warn_and_skip_safely() -> Result<()> {
        let workspace = TestWorkspace::new("missing-optional-api")?;
        workspace.write_file(
            "output/maps/graph.json",
            r#"{"nodes":[{"id":"host:app","node_type":"host","value":"app.example.com","tags":[],"metadata":{},"source_tools":["httpx"],"timestamps":["2026-05-14T09:00:00Z"]}],"edges":[],"clusters":[]}"#,
        )?;
        workspace.write_file(
            "output/api-intel/api-endpoints.json",
            r#"[{"endpoint_id":"endpoint:get:/api","method":"GET","path":"https://app.example.com/api","normalized_path":"/api","parameters":[],"auth_indicators":[],"inferred_objects":[],"semantic_tags":[],"source":"graph.json"}]"#,
        )?;

        let outcome = run_enrichment_engine(
            &workspace.maps_dir(),
            Some(&workspace.api_intel_dir()),
            &workspace.enrichment_dir(),
        )?;
        assert!(!outcome.warnings.is_empty());
        let summary = fs::read_to_string(outcome.summary_path)?;
        assert!(summary.contains("API Intelligence Warnings"));
        Ok(())
    }
}
