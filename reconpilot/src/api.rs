use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;
use walkdir::WalkDir;

use crate::{
    auth, classifiers, jsintel,
    models::{
        ApiEndpoint, ApiGraphSummary, ApiObject, ApiRelationship, ApiSchema, AuthObservation,
        CorrelationEvidence, GraphEdge, GraphNode, GraphQlObservation, JsObservation,
        RelationshipType, SemanticTag,
    },
    schema, utils,
};

#[derive(Debug, Clone)]
pub struct ApiIntelOutcome {
    pub api_endpoints_path: PathBuf,
    pub api_objects_path: PathBuf,
    pub api_relationships_path: PathBuf,
    pub auth_observations_path: PathBuf,
    pub js_observations_path: PathBuf,
    pub schemas_path: PathBuf,
    pub graphql_observations_path: PathBuf,
    pub api_graph_path: PathBuf,
    pub api_summary_path: PathBuf,
    pub summary: ApiGraphSummary,
}

#[derive(Debug, Clone)]
struct ApiLayout {
    root: PathBuf,
    raw: PathBuf,
    maps: PathBuf,
    js: Option<PathBuf>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize)]
struct ImportedGraphDocument {
    #[serde(default)]
    nodes: Vec<GraphNode>,
    #[serde(default)]
    edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone)]
struct LocalArtifact {
    location: String,
    content: String,
}

#[derive(Debug, Clone)]
struct EndpointSeed {
    method: String,
    path: String,
    source: String,
}

#[derive(Debug, Clone, Serialize)]
struct ApiGraphDocument {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    summary: ApiGraphSummary,
}

pub fn run_api_intelligence(input_root: &Path, out: &Path) -> Result<ApiIntelOutcome> {
    let layout = validate_api_layout(input_root)?;
    utils::ensure_directory(out)?;

    let imported_graph = load_graph_document(&layout.maps.join("graph.json"))?;
    let katana_urls = load_katana_urls(&layout.raw.join("katana"))?;
    let local_artifacts = load_local_artifacts(&layout)?;
    let js_observations = load_js_observations(layout.js.as_deref())?;
    let schemas = load_schemas(&local_artifacts);
    let mut graphql_observations = load_graphql_observations(&local_artifacts);
    augment_graphql_from_js(&mut graphql_observations, &js_observations);

    let mut endpoint_seeds = collect_endpoint_seeds(&imported_graph.nodes, &katana_urls);
    endpoint_seeds.extend(collect_js_endpoint_seeds(&js_observations));
    endpoint_seeds.extend(collect_schema_endpoint_seeds(&schemas));
    endpoint_seeds.extend(collect_graphql_endpoint_seeds(&graphql_observations));

    let api_endpoints = build_api_endpoints(endpoint_seeds, &schemas);
    let api_objects = build_api_objects(&api_endpoints, &schemas);
    let auth_observations = build_auth_observations(
        &api_endpoints,
        &schemas,
        &graphql_observations,
        &js_observations,
    );
    let api_relationships = build_api_relationships(
        &api_endpoints,
        &api_objects,
        &schemas,
        &auth_observations,
        &js_observations,
    );
    let (api_graph, summary) = build_api_graph(
        &api_endpoints,
        &api_objects,
        &schemas,
        &auth_observations,
        &js_observations,
        &graphql_observations,
        &api_relationships,
    );

    let api_endpoints_path = out.join("api-endpoints.json");
    let api_objects_path = out.join("api-objects.json");
    let api_relationships_path = out.join("api-relationships.json");
    let auth_observations_path = out.join("auth-observations.json");
    let js_observations_path = out.join("js-observations.json");
    let schemas_path = out.join("schemas.json");
    let graphql_observations_path = out.join("graphql-observations.json");
    let api_graph_path = out.join("api-graph.json");
    let api_summary_path = out.join("api-summary.md");

    utils::write_json_pretty(&api_endpoints_path, &api_endpoints)?;
    utils::write_json_pretty(&api_objects_path, &api_objects)?;
    utils::write_json_pretty(&api_relationships_path, &api_relationships)?;
    utils::write_json_pretty(&auth_observations_path, &auth_observations)?;
    utils::write_json_pretty(&js_observations_path, &js_observations)?;
    utils::write_json_pretty(&schemas_path, &schemas)?;
    utils::write_json_pretty(&graphql_observations_path, &graphql_observations)?;
    utils::write_json_pretty(&api_graph_path, &api_graph)?;
    utils::write_string(
        &api_summary_path,
        &render_api_summary(
            &summary,
            &api_endpoints,
            &api_objects,
            &auth_observations,
            &graphql_observations,
            &js_observations,
            &schemas,
        ),
    )?;

    Ok(ApiIntelOutcome {
        api_endpoints_path,
        api_objects_path,
        api_relationships_path,
        auth_observations_path,
        js_observations_path,
        schemas_path,
        graphql_observations_path,
        api_graph_path,
        api_summary_path,
        summary,
    })
}

pub fn print_api_summary(input_root: &Path, out: &Path, outcome: &ApiIntelOutcome) {
    println!("ReconPilot API intelligence summary");
    println!("Input root: {}", input_root.display());
    println!("Output root: {}", out.display());
    println!("Endpoints: {}", outcome.summary.endpoint_count);
    println!("Objects: {}", outcome.summary.object_count);
    println!("Schemas: {}", outcome.summary.schema_count);
    println!(
        "Auth observations: {} | GraphQL observations: {} | JS observations: {}",
        outcome.summary.auth_observation_count,
        outcome.summary.graphql_observation_count,
        outcome.summary.js_observation_count
    );
    println!("Outputs:");
    println!("  - {}", outcome.api_endpoints_path.display());
    println!("  - {}", outcome.api_objects_path.display());
    println!("  - {}", outcome.api_relationships_path.display());
    println!("  - {}", outcome.auth_observations_path.display());
    println!("  - {}", outcome.js_observations_path.display());
    println!("  - {}", outcome.schemas_path.display());
    println!("  - {}", outcome.graphql_observations_path.display());
    println!("  - {}", outcome.api_graph_path.display());
    println!("  - {}", outcome.api_summary_path.display());
}

fn validate_api_layout(input_root: &Path) -> Result<ApiLayout> {
    if !input_root.exists() {
        bail!(
            "api-intel input root does not exist: {}",
            input_root.display()
        );
    }
    if !input_root.is_dir() {
        bail!(
            "api-intel input root is not a directory: {}",
            input_root.display()
        );
    }

    let raw = input_root.join("raw");
    let maps = input_root.join("maps");
    for path in [&raw, &maps] {
        if !path.exists() || !path.is_dir() {
            bail!(
                "required api-intel input directory is missing: {}",
                path.display()
            );
        }
    }

    let js_path = input_root.join("js");
    let js = if js_path.exists() && js_path.is_dir() {
        Some(js_path)
    } else {
        None
    };

    Ok(ApiLayout {
        root: input_root.to_path_buf(),
        raw,
        maps,
        js,
    })
}

fn load_graph_document(path: &Path) -> Result<ImportedGraphDocument> {
    if !path.exists() {
        return Ok(ImportedGraphDocument::default());
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read graph input at {}", path.display()))?;
    serde_json::from_str::<ImportedGraphDocument>(&raw)
        .with_context(|| format!("failed to parse graph input at {}", path.display()))
}

fn load_katana_urls(path: &Path) -> Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut urls = BTreeSet::new();
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let raw = fs::read_to_string(entry.path())
            .with_context(|| format!("failed to read artifact {}", entry.path().display()))?;
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('{') {
                if let Ok(value) = serde_json::from_str::<Value>(line) {
                    if let Some(url) = value
                        .get("url")
                        .and_then(Value::as_str)
                        .or_else(|| value.get("endpoint").and_then(Value::as_str))
                    {
                        urls.insert(url.to_string());
                    }
                }
            } else if looks_like_url(line) {
                urls.insert(line.to_string());
            }
        }
    }

    Ok(urls.into_iter().collect())
}

fn load_local_artifacts(layout: &ApiLayout) -> Result<Vec<LocalArtifact>> {
    let mut artifacts = Vec::new();
    for root in [Some(layout.raw.as_path()), layout.js.as_deref()] {
        let Some(root) = root else {
            continue;
        };

        for entry in WalkDir::new(root)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
        {
            let extension = entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase())
                .unwrap_or_default();
            if !matches!(
                extension.as_str(),
                "json" | "jsonl" | "js" | "txt" | "html" | "md"
            ) {
                continue;
            }
            let content = fs::read_to_string(entry.path()).with_context(|| {
                format!("failed to read local artifact {}", entry.path().display())
            })?;
            artifacts.push(LocalArtifact {
                location: relative_artifact_location(&layout.root, entry.path()),
                content,
            });
        }
    }

    artifacts.sort_by(|left, right| left.location.cmp(&right.location));
    Ok(artifacts)
}

fn load_js_observations(js_root: Option<&Path>) -> Result<Vec<JsObservation>> {
    let Some(js_root) = js_root else {
        return Ok(Vec::new());
    };

    let mut observations = Vec::new();
    for entry in WalkDir::new(js_root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let extension = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        if !matches!(extension.as_str(), "js" | "json" | "txt" | "html") {
            continue;
        }

        let content = fs::read_to_string(entry.path())
            .with_context(|| format!("failed to read JS artifact {}", entry.path().display()))?;
        observations.push(jsintel::analyze_javascript(
            &entry.path().display().to_string(),
            &content,
        ));
    }

    observations.sort_by(|left, right| left.js_file.cmp(&right.js_file));
    Ok(observations)
}

fn load_schemas(artifacts: &[LocalArtifact]) -> Vec<ApiSchema> {
    let mut schemas = BTreeMap::new();
    for artifact in artifacts {
        if let Some(schema) = schema::parse_schema_artifact(&artifact.location, &artifact.content) {
            schemas.insert(schema.schema_location.clone(), schema);
        }
    }
    schemas.into_values().collect()
}

fn load_graphql_observations(artifacts: &[LocalArtifact]) -> Vec<GraphQlObservation> {
    let mut observations = Vec::new();
    for artifact in artifacts {
        observations.extend(schema::detect_graphql_observations(
            &artifact.location,
            &artifact.content,
        ));
    }
    dedupe_graphql_observations(&mut observations);
    observations
}

fn augment_graphql_from_js(
    graphql_observations: &mut Vec<GraphQlObservation>,
    js_observations: &[JsObservation],
) {
    for observation in js_observations {
        for endpoint in &observation.discovered_endpoints {
            if !endpoint.to_ascii_lowercase().contains("graphql") {
                continue;
            }

            graphql_observations.push(GraphQlObservation {
                endpoint: endpoint.clone(),
                introspection_detected: false,
                schema_indicators: vec!["javascript-reference".to_string()],
                auth_indicators: observation.discovered_auth_indicators.clone(),
                notes: vec![
                    format!(
                        "JavaScript artifact {} referenced a GraphQL-like endpoint",
                        observation.js_file
                    ),
                    "No GraphQL request was executed.".to_string(),
                ],
            });
        }
    }
    dedupe_graphql_observations(graphql_observations);
}

fn collect_endpoint_seeds(graph_nodes: &[GraphNode], katana_urls: &[String]) -> Vec<EndpointSeed> {
    let mut seeds = Vec::new();

    for node in graph_nodes.iter().filter(|node| node.node_type == "url") {
        if is_interesting_endpoint(&node.value) {
            seeds.push(EndpointSeed {
                method: "GET".to_string(),
                path: node.value.clone(),
                source: "graph.json".to_string(),
            });
        }
    }

    for url in katana_urls {
        if is_interesting_endpoint(url) {
            seeds.push(EndpointSeed {
                method: "GET".to_string(),
                path: url.clone(),
                source: "katana".to_string(),
            });
        }
    }

    seeds
}

fn collect_js_endpoint_seeds(js_observations: &[JsObservation]) -> Vec<EndpointSeed> {
    let mut seeds = Vec::new();
    for observation in js_observations {
        for endpoint in &observation.discovered_endpoints {
            seeds.push(EndpointSeed {
                method: "GET".to_string(),
                path: endpoint.clone(),
                source: format!("js:{}", observation.js_file),
            });
        }
    }
    seeds
}

fn collect_schema_endpoint_seeds(schemas: &[ApiSchema]) -> Vec<EndpointSeed> {
    let mut seeds = Vec::new();
    for schema in schemas {
        for endpoint in &schema.endpoints {
            let (method, path) = parse_schema_endpoint(endpoint);
            seeds.push(EndpointSeed {
                method,
                path,
                source: schema.schema_location.clone(),
            });
        }
    }
    seeds
}

fn collect_graphql_endpoint_seeds(observations: &[GraphQlObservation]) -> Vec<EndpointSeed> {
    observations
        .iter()
        .map(|observation| EndpointSeed {
            method: "POST".to_string(),
            path: observation.endpoint.clone(),
            source: "graphql-observation".to_string(),
        })
        .collect()
}

fn build_api_endpoints(seeds: Vec<EndpointSeed>, schemas: &[ApiSchema]) -> Vec<ApiEndpoint> {
    let mut endpoints = BTreeMap::<(String, String), ApiEndpoint>::new();

    for seed in seeds {
        let normalized_path = normalize_endpoint_path(&seed.path);
        if normalized_path.is_empty() {
            continue;
        }
        let parameters = extract_endpoint_parameters(&seed.path);
        let auth_indicators =
            auth::detect_auth_indicators(&format!("{} {}", seed.path, seed.source));
        let inferred_objects = infer_objects(&seed.path, &parameters);
        let mut semantic_tags = classifiers::classify_endpoint_intents(&seed.path);
        merge_tags(
            &mut semantic_tags,
            classifiers::classify_parameter_intents(&parameters),
        );

        let key = (seed.method.clone(), normalized_path.clone());
        let entry = endpoints.entry(key.clone()).or_insert_with(|| ApiEndpoint {
            endpoint_id: format!(
                "endpoint:{}:{}",
                slugify(&seed.method),
                slugify(&normalized_path)
            ),
            method: seed.method.clone(),
            path: seed.path.clone(),
            normalized_path: normalized_path.clone(),
            parameters: parameters.clone(),
            auth_indicators: auth_indicators.clone(),
            inferred_objects: inferred_objects.clone(),
            semantic_tags: semantic_tags.clone(),
            source: seed.source.clone(),
        });

        merge_strings(&mut entry.parameters, parameters);
        merge_strings(&mut entry.auth_indicators, auth_indicators);
        merge_strings(&mut entry.inferred_objects, inferred_objects);
        merge_tags(&mut entry.semantic_tags, semantic_tags);
        merge_sources(&mut entry.source, &seed.source);
    }

    for schema in schemas {
        for endpoint in endpoints.values_mut() {
            if schema.endpoints.iter().any(|candidate| {
                normalize_endpoint_path(&parse_schema_endpoint(candidate).1)
                    == endpoint.normalized_path
            }) {
                merge_strings(&mut endpoint.auth_indicators, schema.auth_methods.clone());
            }
        }
    }

    let mut values = endpoints.into_values().collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.normalized_path
            .cmp(&right.normalized_path)
            .then_with(|| left.method.cmp(&right.method))
    });
    values
}

fn build_api_objects(endpoints: &[ApiEndpoint], schemas: &[ApiSchema]) -> Vec<ApiObject> {
    let mut objects = BTreeMap::<String, ApiObject>::new();

    for endpoint in endpoints {
        for object_name in &endpoint.inferred_objects {
            let entry = objects
                .entry(object_name.clone())
                .or_insert_with(|| ApiObject {
                    object_name: object_name.clone(),
                    related_endpoints: Vec::new(),
                    related_parameters: Vec::new(),
                    inferred_sensitivity: infer_object_sensitivity(object_name),
                });
            push_string(
                &mut entry.related_endpoints,
                endpoint.normalized_path.clone(),
            );
            merge_strings(&mut entry.related_parameters, endpoint.parameters.clone());
        }
    }

    for schema in schemas {
        for object_name in &schema.objects {
            let entry = objects
                .entry(object_name.clone())
                .or_insert_with(|| ApiObject {
                    object_name: object_name.clone(),
                    related_endpoints: Vec::new(),
                    related_parameters: Vec::new(),
                    inferred_sensitivity: infer_object_sensitivity(object_name),
                });
            for endpoint in &schema.endpoints {
                push_string(
                    &mut entry.related_endpoints,
                    normalize_endpoint_path(&parse_schema_endpoint(endpoint).1),
                );
            }
        }
    }

    let mut values = objects.into_values().collect::<Vec<_>>();
    values.sort_by(|left, right| left.object_name.cmp(&right.object_name));
    values
}

fn build_auth_observations(
    endpoints: &[ApiEndpoint],
    schemas: &[ApiSchema],
    graphql_observations: &[GraphQlObservation],
    js_observations: &[JsObservation],
) -> Vec<AuthObservation> {
    let mut observations = BTreeMap::<(String, String), AuthObservation>::new();

    for endpoint in endpoints {
        let asset = endpoint_asset(endpoint);
        let texts = vec![
            endpoint.path.clone(),
            endpoint.normalized_path.clone(),
            endpoint.source.clone(),
            endpoint.auth_indicators.join(" "),
        ];
        let evidence = endpoint
            .auth_indicators
            .iter()
            .map(|indicator| {
                format!(
                    "Endpoint '{}' matched auth indicator '{}'",
                    endpoint.normalized_path, indicator
                )
            })
            .collect::<Vec<_>>();
        if let Some(observation) = auth::build_auth_observation(&asset, &texts, &evidence) {
            observations.insert(
                (observation.asset.clone(), observation.auth_type.clone()),
                observation,
            );
        }
    }

    for schema in schemas {
        let asset =
            extract_host(&schema.schema_location).unwrap_or_else(|| schema.schema_location.clone());
        if let Some(observation) = auth::build_auth_observation(
            &asset,
            &[
                schema.schema_location.clone(),
                schema.auth_methods.join(" "),
            ],
            &[format!(
                "Schema '{}' declared auth methods: {}",
                schema.schema_location,
                display_or_none(&schema.auth_methods)
            )],
        ) {
            observations.insert(
                (observation.asset.clone(), observation.auth_type.clone()),
                observation,
            );
        }
    }

    for observation in graphql_observations {
        if observation.auth_indicators.is_empty() {
            continue;
        }
        let asset =
            extract_host(&observation.endpoint).unwrap_or_else(|| observation.endpoint.clone());
        observations.insert(
            (asset.clone(), "graphql_auth".to_string()),
            AuthObservation {
                asset,
                auth_type: "graphql_auth".to_string(),
                indicators: observation.auth_indicators.clone(),
                confidence: 0.75,
                evidence: observation
                    .notes
                    .iter()
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>(),
            },
        );
    }

    for observation in js_observations {
        if observation.discovered_auth_indicators.is_empty() {
            continue;
        }
        let asset = observation.js_file.clone();
        observations.insert(
            (asset.clone(), "javascript_auth_reference".to_string()),
            AuthObservation {
                asset,
                auth_type: "javascript_auth_reference".to_string(),
                indicators: observation.discovered_auth_indicators.clone(),
                confidence: 0.7,
                evidence: observation.evidence.clone(),
            },
        );
    }

    observations.into_values().collect()
}

fn build_api_relationships(
    endpoints: &[ApiEndpoint],
    objects: &[ApiObject],
    schemas: &[ApiSchema],
    auth_observations: &[AuthObservation],
    js_observations: &[JsObservation],
) -> Vec<ApiRelationship> {
    let object_set = objects
        .iter()
        .map(|object| object.object_name.clone())
        .collect::<BTreeSet<_>>();
    let auth_by_asset = auth_observations.iter().fold(
        BTreeMap::<String, Vec<String>>::new(),
        |mut acc, observation| {
            acc.entry(observation.asset.clone())
                .or_default()
                .push(observation.auth_type.clone());
            acc
        },
    );

    let mut relationships = Vec::new();

    for endpoint in endpoints {
        for object_name in &endpoint.inferred_objects {
            if object_set.contains(object_name) {
                relationships.push(ApiRelationship {
                    source_endpoint: endpoint.endpoint_id.clone(),
                    target_object: object_name.clone(),
                    relationship_type: "returns_object".to_string(),
                    confidence: 0.82,
                    evidence: vec![format!(
                        "Normalized path '{}' inferred object '{}'",
                        endpoint.normalized_path, object_name
                    )],
                });
            }
        }

        for parameter in &endpoint.parameters {
            relationships.push(ApiRelationship {
                source_endpoint: endpoint.endpoint_id.clone(),
                target_object: format!("parameter:{parameter}"),
                relationship_type: "references_parameter".to_string(),
                confidence: 0.78,
                evidence: vec![format!(
                    "Endpoint '{}' references parameter '{}'",
                    endpoint.normalized_path, parameter
                )],
            });
        }

        let family = infer_api_family(&endpoint.normalized_path);
        relationships.push(ApiRelationship {
            source_endpoint: endpoint.endpoint_id.clone(),
            target_object: family.clone(),
            relationship_type: "belongs_to_api".to_string(),
            confidence: 0.75,
            evidence: vec![format!(
                "Endpoint '{}' was grouped into API family '{}'",
                endpoint.normalized_path, family
            )],
        });

        if let Some(asset_auth) = auth_by_asset.get(&endpoint_asset(endpoint)) {
            for auth_type in asset_auth {
                relationships.push(ApiRelationship {
                    source_endpoint: endpoint.endpoint_id.clone(),
                    target_object: format!("auth_flow:{auth_type}"),
                    relationship_type: "related_to_auth_flow".to_string(),
                    confidence: 0.76,
                    evidence: vec![format!(
                        "Asset '{}' also produced auth observation '{}'",
                        endpoint_asset(endpoint),
                        auth_type
                    )],
                });
            }
        }

        for schema in schemas {
            if schema.endpoints.iter().any(|candidate| {
                normalize_endpoint_path(&parse_schema_endpoint(candidate).1)
                    == endpoint.normalized_path
            }) {
                relationships.push(ApiRelationship {
                    source_endpoint: endpoint.endpoint_id.clone(),
                    target_object: format!("schema:{}", slugify(&schema.schema_location)),
                    relationship_type: "references_schema".to_string(),
                    confidence: 0.86,
                    evidence: vec![format!(
                        "Schema '{}' documented endpoint '{}'",
                        schema.schema_location, endpoint.normalized_path
                    )],
                });
            }
        }

        if endpoint
            .auth_indicators
            .iter()
            .any(|indicator| indicator.contains("token") || indicator == "jwt")
        {
            relationships.push(ApiRelationship {
                source_endpoint: endpoint.endpoint_id.clone(),
                target_object: "token:authorization".to_string(),
                relationship_type: "uses_token".to_string(),
                confidence: 0.8,
                evidence: vec![format!(
                    "Endpoint '{}' matched token-related auth indicators",
                    endpoint.normalized_path
                )],
            });
        }
    }

    for observation in js_observations {
        for endpoint in &observation.discovered_endpoints {
            let normalized = normalize_endpoint_path(endpoint);
            if normalized.is_empty() {
                continue;
            }
            relationships.push(ApiRelationship {
                source_endpoint: format!("js:{}", slugify(&observation.js_file)),
                target_object: format!("endpoint:{}:{}", slugify("GET"), slugify(&normalized)),
                relationship_type: "loads_endpoint".to_string(),
                confidence: 0.8,
                evidence: vec![format!(
                    "JavaScript artifact '{}' referenced endpoint '{}'",
                    observation.js_file, endpoint
                )],
            });
        }
    }

    relationships.sort_by(|left, right| {
        (
            left.source_endpoint.as_str(),
            left.target_object.as_str(),
            left.relationship_type.as_str(),
        )
            .cmp(&(
                right.source_endpoint.as_str(),
                right.target_object.as_str(),
                right.relationship_type.as_str(),
            ))
    });
    relationships.dedup_by(|left, right| {
        left.source_endpoint == right.source_endpoint
            && left.target_object == right.target_object
            && left.relationship_type == right.relationship_type
    });
    relationships
}

fn build_api_graph(
    endpoints: &[ApiEndpoint],
    objects: &[ApiObject],
    schemas: &[ApiSchema],
    auth_observations: &[AuthObservation],
    js_observations: &[JsObservation],
    graphql_observations: &[GraphQlObservation],
    relationships: &[ApiRelationship],
) -> (ApiGraphDocument, ApiGraphSummary) {
    let mut nodes = BTreeMap::<String, GraphNode>::new();
    let mut edges = BTreeMap::<(String, String, RelationshipType), GraphEdge>::new();
    let mut api_families = BTreeSet::new();

    for endpoint in endpoints {
        insert_node(
            &mut nodes,
            GraphNode {
                id: endpoint.endpoint_id.clone(),
                node_type: "api_endpoint".to_string(),
                value: endpoint.path.clone(),
                tags: endpoint
                    .semantic_tags
                    .iter()
                    .map(|tag| tag.tag.clone())
                    .collect(),
                metadata: endpoint_metadata(endpoint),
                source_tools: endpoint
                    .source
                    .split(',')
                    .map(|value| value.trim().to_string())
                    .collect(),
                timestamps: vec![Utc::now()],
            },
        );

        if let Some(host) = extract_host(&endpoint.path) {
            let host_id = format!("host:{}", slugify(&host));
            insert_node(
                &mut nodes,
                GraphNode {
                    id: host_id.clone(),
                    node_type: "host".to_string(),
                    value: host.clone(),
                    tags: vec!["api_related".to_string()],
                    metadata: BTreeMap::new(),
                    source_tools: vec!["api-intel".to_string()],
                    timestamps: vec![Utc::now()],
                },
            );
            insert_edge(
                &mut edges,
                GraphEdge {
                    source: host_id,
                    target: endpoint.endpoint_id.clone(),
                    relationship: RelationshipType::Hosts,
                    confidence: 0.8,
                    evidence: vec![CorrelationEvidence {
                        source_tool: "api-intel".to_string(),
                        description: format!(
                            "Host '{}' appears to expose endpoint '{}'",
                            host, endpoint.path
                        ),
                        weight: 0.8,
                    }],
                    timestamps: vec![Utc::now()],
                },
            );
        }
    }

    for object in objects {
        insert_node(
            &mut nodes,
            GraphNode {
                id: format!("object:{}", slugify(&object.object_name)),
                node_type: "api_object".to_string(),
                value: object.object_name.clone(),
                tags: vec![format!("sensitivity:{}", object.inferred_sensitivity)],
                metadata: BTreeMap::new(),
                source_tools: vec!["api-intel".to_string()],
                timestamps: vec![Utc::now()],
            },
        );
    }

    for schema in schemas {
        insert_node(
            &mut nodes,
            GraphNode {
                id: format!("schema:{}", slugify(&schema.schema_location)),
                node_type: "api_schema".to_string(),
                value: schema.schema_location.clone(),
                tags: vec![schema.schema_type.clone()],
                metadata: BTreeMap::new(),
                source_tools: vec!["api-intel".to_string()],
                timestamps: vec![Utc::now()],
            },
        );
    }

    for observation in auth_observations {
        insert_node(
            &mut nodes,
            GraphNode {
                id: format!("auth-flow:{}", slugify(&observation.auth_type)),
                node_type: "auth_flow".to_string(),
                value: observation.auth_type.clone(),
                tags: observation.indicators.clone(),
                metadata: BTreeMap::new(),
                source_tools: vec!["api-intel".to_string()],
                timestamps: vec![Utc::now()],
            },
        );
    }

    for observation in js_observations {
        insert_node(
            &mut nodes,
            GraphNode {
                id: format!("js:{}", slugify(&observation.js_file)),
                node_type: "js_asset".to_string(),
                value: observation.js_file.clone(),
                tags: observation.discovered_roles.clone(),
                metadata: BTreeMap::new(),
                source_tools: vec!["api-intel".to_string()],
                timestamps: vec![Utc::now()],
            },
        );
    }

    for relationship in relationships {
        match relationship.relationship_type.as_str() {
            "returns_object" => {
                insert_edge(
                    &mut edges,
                    graph_edge(
                        &relationship.source_endpoint,
                        &format!("object:{}", slugify(&relationship.target_object)),
                        RelationshipType::ReturnsObject,
                        relationship.confidence,
                        &relationship.evidence,
                    ),
                );
            }
            "references_parameter" => {
                let parameter = relationship
                    .target_object
                    .trim_start_matches("parameter:")
                    .to_string();
                let parameter_id = format!("param:{}", slugify(&parameter));
                insert_node(
                    &mut nodes,
                    GraphNode {
                        id: parameter_id.clone(),
                        node_type: "parameter".to_string(),
                        value: parameter.clone(),
                        tags: Vec::new(),
                        metadata: BTreeMap::new(),
                        source_tools: vec!["api-intel".to_string()],
                        timestamps: vec![Utc::now()],
                    },
                );
                insert_edge(
                    &mut edges,
                    graph_edge(
                        &relationship.source_endpoint,
                        &parameter_id,
                        RelationshipType::ReferencesParameter,
                        relationship.confidence,
                        &relationship.evidence,
                    ),
                );
            }
            "belongs_to_api" => {
                let family_id = format!("api-family:{}", slugify(&relationship.target_object));
                api_families.insert(relationship.target_object.clone());
                insert_node(
                    &mut nodes,
                    GraphNode {
                        id: family_id.clone(),
                        node_type: "api_family".to_string(),
                        value: relationship.target_object.clone(),
                        tags: Vec::new(),
                        metadata: BTreeMap::new(),
                        source_tools: vec!["api-intel".to_string()],
                        timestamps: vec![Utc::now()],
                    },
                );
                insert_edge(
                    &mut edges,
                    graph_edge(
                        &relationship.source_endpoint,
                        &family_id,
                        RelationshipType::BelongsToApi,
                        relationship.confidence,
                        &relationship.evidence,
                    ),
                );
            }
            "references_schema" => {
                insert_edge(
                    &mut edges,
                    graph_edge(
                        &relationship.source_endpoint,
                        &relationship.target_object,
                        RelationshipType::ReferencesSchema,
                        relationship.confidence,
                        &relationship.evidence,
                    ),
                );
            }
            "related_to_auth_flow" => {
                let flow_id = format!(
                    "auth-flow:{}",
                    slugify(relationship.target_object.trim_start_matches("auth_flow:"))
                );
                insert_edge(
                    &mut edges,
                    graph_edge(
                        &relationship.source_endpoint,
                        &flow_id,
                        RelationshipType::RelatedToAuthFlow,
                        relationship.confidence,
                        &relationship.evidence,
                    ),
                );
                insert_edge(
                    &mut edges,
                    graph_edge(
                        &relationship.source_endpoint,
                        &flow_id,
                        RelationshipType::RequiresAuth,
                        relationship.confidence - 0.03,
                        &relationship.evidence,
                    ),
                );
            }
            "uses_token" => {
                let token_id = format!(
                    "token:{}",
                    slugify(relationship.target_object.trim_start_matches("token:"))
                );
                insert_node(
                    &mut nodes,
                    GraphNode {
                        id: token_id.clone(),
                        node_type: "token".to_string(),
                        value: relationship.target_object.clone(),
                        tags: vec!["auth".to_string()],
                        metadata: BTreeMap::new(),
                        source_tools: vec!["api-intel".to_string()],
                        timestamps: vec![Utc::now()],
                    },
                );
                insert_edge(
                    &mut edges,
                    graph_edge(
                        &relationship.source_endpoint,
                        &token_id,
                        RelationshipType::UsesToken,
                        relationship.confidence,
                        &relationship.evidence,
                    ),
                );
            }
            "loads_endpoint" => {
                insert_edge(
                    &mut edges,
                    graph_edge(
                        &relationship.source_endpoint,
                        &relationship.target_object,
                        RelationshipType::LoadsEndpoint,
                        relationship.confidence,
                        &relationship.evidence,
                    ),
                );
            }
            _ => {}
        }
    }

    for observation in graphql_observations {
        let endpoint_id = format!(
            "endpoint:{}:{}",
            slugify("POST"),
            slugify(&normalize_endpoint_path(&observation.endpoint))
        );
        if nodes.contains_key(&endpoint_id) {
            insert_node(
                &mut nodes,
                GraphNode {
                    id: format!("graphql:{}", slugify(&observation.endpoint)),
                    node_type: "graphql_surface".to_string(),
                    value: observation.endpoint.clone(),
                    tags: observation.schema_indicators.clone(),
                    metadata: BTreeMap::new(),
                    source_tools: vec!["api-intel".to_string()],
                    timestamps: vec![Utc::now()],
                },
            );
            insert_edge(
                &mut edges,
                graph_edge(
                    &endpoint_id,
                    &format!("graphql:{}", slugify(&observation.endpoint)),
                    RelationshipType::References,
                    0.72,
                    &observation.notes,
                ),
            );
        }
    }

    let mut node_values = nodes.into_values().collect::<Vec<_>>();
    node_values.sort_by(|left, right| left.id.cmp(&right.id));
    let mut edge_values = edges.into_values().collect::<Vec<_>>();
    edge_values.sort_by(|left, right| {
        (
            left.source.as_str(),
            left.target.as_str(),
            &left.relationship,
        )
            .cmp(&(
                right.source.as_str(),
                right.target.as_str(),
                &right.relationship,
            ))
    });

    let summary = ApiGraphSummary {
        generated_at: Utc::now(),
        endpoint_count: endpoints.len(),
        object_count: objects.len(),
        relationship_count: relationships.len(),
        schema_count: schemas.len(),
        auth_observation_count: auth_observations.len(),
        graphql_observation_count: graphql_observations.len(),
        js_observation_count: js_observations.len(),
        api_family_count: api_families.len(),
        privileged_endpoint_count: endpoints
            .iter()
            .filter(|endpoint| {
                endpoint.semantic_tags.iter().any(|tag| {
                    matches!(
                        tag.tag.as_str(),
                        "admin_surface" | "internal_surface" | "api_documentation"
                    )
                }) || !endpoint.auth_indicators.is_empty()
            })
            .count(),
        top_auth_styles: top_counts(auth_observations.iter().fold(
            BTreeMap::new(),
            |mut acc, observation| {
                *acc.entry(observation.auth_type.clone()).or_insert(0usize) += 1;
                acc
            },
        )),
        likely_sensitive_objects: objects
            .iter()
            .filter(|object| object.inferred_sensitivity != "low")
            .take(8)
            .map(|object| format!("{} ({})", object.object_name, object.inferred_sensitivity))
            .collect(),
        hidden_route_candidates: js_observations
            .iter()
            .flat_map(|observation| observation.discovered_endpoints.iter().cloned())
            .take(8)
            .collect(),
    };

    (
        ApiGraphDocument {
            nodes: node_values,
            edges: edge_values,
            summary: summary.clone(),
        },
        summary,
    )
}

fn render_api_summary(
    summary: &ApiGraphSummary,
    endpoints: &[ApiEndpoint],
    objects: &[ApiObject],
    auth_observations: &[AuthObservation],
    graphql_observations: &[GraphQlObservation],
    js_observations: &[JsObservation],
    schemas: &[ApiSchema],
) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot API & JavaScript Intelligence Summary\n\n");
    output.push_str(
        "This phase used local ReconPilot artifacts only. The results below describe candidate application capabilities and relationships that require manual validation.\n\n",
    );

    output.push_str("## Discovered API Families\n\n");
    write_markdown_list(&mut output, &top_api_families(endpoints));

    output.push_str("\n## Detected Auth Styles\n\n");
    write_markdown_list(&mut output, &summary.top_auth_styles);

    output.push_str("\n## Detected Schema And Documentation Systems\n\n");
    let schema_values = schemas
        .iter()
        .map(|schema| format!("{} [{}]", schema.schema_location, schema.schema_type))
        .collect::<Vec<_>>();
    write_markdown_list(&mut output, &schema_values);

    output.push_str("\n## Likely Sensitive Objects\n\n");
    write_markdown_list(&mut output, &summary.likely_sensitive_objects);

    output.push_str("\n## Likely Privileged Endpoints\n\n");
    let privileged = endpoints
        .iter()
        .filter(|endpoint| {
            endpoint.semantic_tags.iter().any(|tag| {
                matches!(
                    tag.tag.as_str(),
                    "admin_surface"
                        | "internal_surface"
                        | "api_documentation"
                        | "sensitive_data_operation"
                )
            }) || !endpoint.auth_indicators.is_empty()
        })
        .take(10)
        .map(|endpoint| format!("{} {}", endpoint.method, endpoint.path))
        .collect::<Vec<_>>();
    write_markdown_list(&mut output, &privileged);

    output.push_str("\n## GraphQL Indicators\n\n");
    let graphql_values = graphql_observations
        .iter()
        .map(|observation| {
            format!(
                "{} [{}]",
                observation.endpoint,
                display_or_none(&observation.schema_indicators)
            )
        })
        .collect::<Vec<_>>();
    write_markdown_list(&mut output, &graphql_values);

    output.push_str("\n## JS-Discovered Hidden Routes\n\n");
    let js_routes = js_observations
        .iter()
        .flat_map(|observation| observation.discovered_endpoints.iter().cloned())
        .take(10)
        .collect::<Vec<_>>();
    write_markdown_list(&mut output, &js_routes);

    output.push_str("\n## Local Analysis Metrics\n\n");
    output.push_str(&format!(
        "- Endpoints: {}\n- Objects: {}\n- Relationships: {}\n- Auth observations: {}\n- JS observations: {}\n",
        summary.endpoint_count,
        objects.len(),
        summary.relationship_count,
        auth_observations.len(),
        summary.js_observation_count
    ));

    output.push_str("\n## Caution\n\n");
    output.push_str(
        "These results identify interesting candidates, documentation exposure hints, auth-flow references, and possible object relationships. They are not vulnerability findings and require careful manual validation.\n",
    );
    output
}

fn parse_schema_endpoint(value: &str) -> (String, String) {
    let trimmed = value.trim();
    let mut parts = trimmed.split_whitespace();
    let first = parts.next().unwrap_or("GET");
    let second = parts.next();
    if let Some(path) = second {
        (first.to_ascii_uppercase(), path.to_string())
    } else {
        ("GET".to_string(), trimmed.to_string())
    }
}

fn is_interesting_endpoint(value: &str) -> bool {
    !classifiers::classify_endpoint_intents(value).is_empty()
        || !extract_endpoint_parameters(value).is_empty()
        || value.to_ascii_lowercase().contains("graphql")
        || value.to_ascii_lowercase().contains("swagger")
        || value.to_ascii_lowercase().contains("openapi")
}

fn normalize_endpoint_path(value: &str) -> String {
    let candidate = if looks_like_url(value) {
        Url::parse(value)
            .ok()
            .map(|url| {
                let mut path = url.path().to_string();
                if !url.query().unwrap_or_default().is_empty() {
                    path.push('?');
                    path.push_str(url.query().unwrap_or_default());
                }
                path
            })
            .unwrap_or_else(|| value.to_string())
    } else {
        value.to_string()
    };

    let (_, path) = parse_schema_endpoint(&candidate);
    let path_only = path.split('?').next().unwrap_or_default();
    let uuid_re = Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
        .expect("valid regex");
    let numeric_re = Regex::new(r"^\d+$").expect("valid regex");

    let mut segments = Vec::new();
    for segment in path_only.split('/') {
        if segment.is_empty() {
            continue;
        }
        let normalized = if segment.starts_with('{') && segment.ends_with('}') {
            "{id}".to_string()
        } else if segment.starts_with(':') {
            "{id}".to_string()
        } else if uuid_re.is_match(segment) || numeric_re.is_match(segment) {
            "{id}".to_string()
        } else {
            segment.to_ascii_lowercase()
        };
        segments.push(normalized);
    }

    if segments.is_empty() {
        String::new()
    } else {
        format!("/{}", segments.join("/"))
    }
}

fn extract_endpoint_parameters(value: &str) -> Vec<String> {
    let mut parameters = BTreeSet::new();

    let candidate = if looks_like_url(value) {
        Url::parse(value).ok()
    } else {
        None
    };
    if let Some(url) = candidate {
        for (key, _) in url.query_pairs() {
            parameters.insert(key.to_string());
        }
    }

    let brace_re = Regex::new(r"\{([A-Za-z0-9_]+)\}").expect("valid regex");
    let colon_re = Regex::new(r":([A-Za-z0-9_]+)").expect("valid regex");
    for capture in brace_re.captures_iter(value) {
        parameters.insert(capture[1].to_string());
    }
    for capture in colon_re.captures_iter(value) {
        parameters.insert(capture[1].to_string());
    }

    parameters.into_iter().collect()
}

fn infer_objects(path: &str, parameters: &[String]) -> Vec<String> {
    let mut objects = BTreeSet::new();
    let normalized = normalize_endpoint_path(path);
    for segment in normalized.split('/') {
        if segment.is_empty() || is_generic_path_segment(segment) || segment == "{id}" {
            continue;
        }

        if let Some(object_name) = normalize_object_name(segment) {
            objects.insert(object_name);
        }
    }

    for parameter in parameters {
        if let Some(object_name) = parameter
            .strip_suffix("_id")
            .or_else(|| parameter.strip_suffix("Id"))
            .and_then(normalize_object_name)
        {
            objects.insert(object_name);
        } else if parameter == "id" {
            if let Some(last_object) = objects.iter().last().cloned() {
                objects.insert(last_object);
            }
        }
    }

    objects.into_iter().collect()
}

fn infer_object_sensitivity(object_name: &str) -> String {
    let lowered = object_name.to_ascii_lowercase();
    if [
        "user",
        "account",
        "organization",
        "billing",
        "payment",
        "invoice",
        "admin",
        "token",
        "secret",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
    {
        "high".to_string()
    } else if ["project", "report", "file", "document", "customer"]
        .iter()
        .any(|needle| lowered.contains(needle))
    {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

fn infer_api_family(path: &str) -> String {
    let segments = path
        .trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return "api:unknown".to_string();
    }

    let family = if segments[0] == "api" && segments.len() > 1 {
        segments[1]
    } else {
        segments[0]
    };
    format!("api:{}", family.to_ascii_lowercase())
}

fn endpoint_asset(endpoint: &ApiEndpoint) -> String {
    extract_host(&endpoint.path).unwrap_or_else(|| endpoint.normalized_path.clone())
}

fn endpoint_metadata(endpoint: &ApiEndpoint) -> BTreeMap<String, Value> {
    let mut metadata = BTreeMap::new();
    metadata.insert("method".to_string(), Value::String(endpoint.method.clone()));
    metadata.insert(
        "normalized_path".to_string(),
        Value::String(endpoint.normalized_path.clone()),
    );
    metadata.insert(
        "parameters".to_string(),
        Value::Array(
            endpoint
                .parameters
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );
    metadata
}

fn extract_host(value: &str) -> Option<String> {
    if looks_like_url(value) {
        return Url::parse(value)
            .ok()
            .and_then(|parsed| parsed.host_str().map(ToOwned::to_owned));
    }
    None
}

fn looks_like_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn is_generic_path_segment(value: &str) -> bool {
    matches!(
        value,
        "api"
            | "v1"
            | "v2"
            | "v3"
            | "swagger"
            | "openapi"
            | "docs"
            | "graphql"
            | "internal"
            | "admin"
            | "auth"
            | "login"
            | "public"
    )
}

fn normalize_object_name(value: &str) -> Option<String> {
    let cleaned = value
        .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '_');
    if cleaned.is_empty() || cleaned.len() < 2 {
        return None;
    }

    let singular = cleaned
        .trim_end_matches("ies")
        .trim_end_matches('s')
        .replace('_', " ");
    if singular.is_empty() {
        return None;
    }

    let normalized = singular
        .split_whitespace()
        .map(|segment| {
            let mut chars = segment.chars();
            let first = chars.next()?.to_ascii_uppercase();
            Some(format!("{first}{}", chars.as_str().to_ascii_lowercase()))
        })
        .collect::<Option<Vec<_>>>()?
        .join("");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn merge_tags(target: &mut Vec<SemanticTag>, incoming: Vec<SemanticTag>) {
    let mut by_key = target
        .iter()
        .cloned()
        .map(|tag| ((tag.tag.clone(), tag.category.clone()), tag))
        .collect::<BTreeMap<_, _>>();

    for tag in incoming {
        let key = (tag.tag.clone(), tag.category.clone());
        if let Some(existing) = by_key.get_mut(&key) {
            existing.confidence = existing.confidence.max(tag.confidence);
            merge_strings(&mut existing.evidence, tag.evidence);
        } else {
            by_key.insert(key, tag);
        }
    }

    *target = by_key.into_values().collect();
    target.sort_by(|left, right| {
        (left.category.as_str(), left.tag.as_str())
            .cmp(&(right.category.as_str(), right.tag.as_str()))
    });
}

fn merge_strings(target: &mut Vec<String>, incoming: Vec<String>) {
    for value in incoming {
        push_string(target, value);
    }
}

fn push_string(target: &mut Vec<String>, value: String) {
    if !target.iter().any(|existing| existing == &value) {
        target.push(value);
        target.sort();
    }
}

fn merge_sources(target: &mut String, source: &str) {
    let mut values = target
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    push_string(&mut values, source.to_string());
    *target = values.join(", ");
}

fn insert_node(nodes: &mut BTreeMap<String, GraphNode>, node: GraphNode) {
    nodes.entry(node.id.clone()).or_insert(node);
}

fn insert_edge(
    edges: &mut BTreeMap<(String, String, RelationshipType), GraphEdge>,
    edge: GraphEdge,
) {
    let key = (
        edge.source.clone(),
        edge.target.clone(),
        edge.relationship.clone(),
    );
    let entry = edges.entry(key).or_insert_with(|| edge.clone());
    entry.confidence = entry.confidence.max(edge.confidence);
    entry.evidence.extend(edge.evidence);
    entry.timestamps.extend(edge.timestamps);
    entry.timestamps.sort();
    entry.timestamps.dedup();
}

fn graph_edge(
    source: &str,
    target: &str,
    relationship: RelationshipType,
    confidence: f32,
    evidence: &[String],
) -> GraphEdge {
    GraphEdge {
        source: source.to_string(),
        target: target.to_string(),
        relationship,
        confidence,
        evidence: evidence
            .iter()
            .map(|description| CorrelationEvidence {
                source_tool: "api-intel".to_string(),
                description: description.clone(),
                weight: confidence,
            })
            .collect(),
        timestamps: vec![Utc::now()],
    }
}

fn relative_artifact_location(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
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

fn top_api_families(endpoints: &[ApiEndpoint]) -> Vec<String> {
    let counts = endpoints.iter().fold(BTreeMap::new(), |mut acc, endpoint| {
        *acc.entry(infer_api_family(&endpoint.normalized_path))
            .or_insert(0usize) += 1;
        acc
    });
    top_counts(counts)
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
        output.push_str(&format!("- {value}\n"));
    }
}

fn dedupe_graphql_observations(observations: &mut Vec<GraphQlObservation>) {
    let mut seen = BTreeSet::new();
    observations.retain(|observation| {
        seen.insert((
            observation.endpoint.clone(),
            observation.introspection_detected,
            observation.schema_indicators.join("|"),
        ))
    });
    observations.sort_by(|left, right| left.endpoint.cmp(&right.endpoint));
}

fn slugify(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use super::{build_api_relationships, infer_objects, run_api_intelligence};
    use crate::{
        models::{ApiEndpoint, ApiObject, ApiSchema, AuthObservation, JsObservation},
        utils::ensure_output_structure,
    };

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
                "reconpilot-api-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            Ok(Self { root })
        }

        fn output_root(&self) -> PathBuf {
            self.root.join("output")
        }

        fn api_out(&self) -> PathBuf {
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
    fn object_inference_matches_common_path_patterns() {
        let objects = infer_objects(
            "https://api.example.com/api/v1/users/{id}?org_id=123",
            &["id".to_string(), "org_id".to_string()],
        );
        assert!(objects.iter().any(|value| value == "User"));
        assert!(objects.iter().any(|value| value == "Org"));
    }

    #[test]
    fn api_relationship_generation_includes_object_and_auth_links() {
        let endpoints = vec![ApiEndpoint {
            endpoint_id: "endpoint:get:/users/{id}".to_string(),
            method: "GET".to_string(),
            path: "https://api.example.com/users/{id}".to_string(),
            normalized_path: "/users/{id}".to_string(),
            parameters: vec!["id".to_string(), "token".to_string()],
            auth_indicators: vec!["jwt".to_string(), "bearer_token".to_string()],
            inferred_objects: vec!["User".to_string()],
            semantic_tags: Vec::new(),
            source: "graph.json".to_string(),
        }];
        let objects = vec![ApiObject {
            object_name: "User".to_string(),
            related_endpoints: vec!["/users/{id}".to_string()],
            related_parameters: vec!["id".to_string()],
            inferred_sensitivity: "high".to_string(),
        }];
        let schemas = vec![ApiSchema {
            schema_type: "openapi".to_string(),
            schema_location: "swagger.json".to_string(),
            detected_version: Some("3.0.0".to_string()),
            endpoints: vec!["GET /users/{id}".to_string()],
            auth_methods: vec!["bearerAuth".to_string()],
            objects: vec!["User".to_string()],
        }];
        let auth = vec![AuthObservation {
            asset: "api.example.com".to_string(),
            auth_type: "jwt_bearer".to_string(),
            indicators: vec!["jwt".to_string()],
            confidence: 0.8,
            evidence: Vec::new(),
        }];
        let js = vec![JsObservation {
            js_file: "app.js".to_string(),
            discovered_endpoints: vec!["/users/{id}".to_string()],
            discovered_roles: Vec::new(),
            discovered_auth_indicators: Vec::new(),
            discovered_feature_flags: Vec::new(),
            evidence: Vec::new(),
        }];

        let relationships = build_api_relationships(&endpoints, &objects, &schemas, &auth, &js);
        assert!(relationships
            .iter()
            .any(|relationship| relationship.relationship_type == "returns_object"));
        assert!(relationships
            .iter()
            .any(|relationship| relationship.relationship_type == "related_to_auth_flow"));
        assert!(relationships
            .iter()
            .any(|relationship| relationship.relationship_type == "references_schema"));
    }

    #[test]
    fn empty_artifact_handling_writes_outputs() -> Result<()> {
        let workspace = TestWorkspace::new("empty")?;
        let output = ensure_output_structure(&workspace.output_root())?;
        let outcome = run_api_intelligence(&output.root, &workspace.api_out())?;
        assert!(outcome.api_endpoints_path.exists());
        assert!(outcome.api_graph_path.exists());
        assert_eq!(outcome.summary.endpoint_count, 0);
        Ok(())
    }

    #[test]
    fn malformed_artifact_handling_is_safe() -> Result<()> {
        let workspace = TestWorkspace::new("malformed")?;
        let output = ensure_output_structure(&workspace.output_root())?;
        workspace.write_file("output/raw/katana/katana.jsonl", "{not-json}\n%%%%\n")?;
        workspace.write_file("output/js/app.js", "const route = '/api/v1/test';")?;

        let outcome = run_api_intelligence(&output.root, &workspace.api_out())?;
        assert!(outcome.api_summary_path.exists());
        let endpoints = fs::read_to_string(outcome.api_endpoints_path)?;
        assert!(endpoints.contains("/api/v1/test"));
        Ok(())
    }
}
