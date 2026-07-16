use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use url::Url;

use crate::{
    classifiers,
    correlation::GraphAnomaly,
    models::{
        ApiDerivedObservation, ApiEndpoint, ApiIntelBundle, ApiObject, ApiSchema, AssetCluster,
        AssetRole, AuthObservation, EnrichedAsset, EnrichedGraph, EnvironmentType, GraphEdge,
        GraphNode, GraphQlObservation, GraphSummary, JsObservation, RelationshipType,
        RiskExplanation, SemanticObservation, SemanticSummary, SemanticTag,
    },
};

#[derive(Debug, Clone)]
pub struct SemanticAnalysis {
    pub semantic_assets: Vec<EnrichedAsset>,
    pub observations: Vec<SemanticObservation>,
    pub risk_explanations: Vec<RiskExplanation>,
    pub enriched_graph: EnrichedGraph,
    pub summary: SemanticSummary,
}

#[derive(Debug, Clone)]
struct AssetSeed {
    node_id: String,
    asset: String,
    semantic_tags: Vec<SemanticTag>,
    roles: Vec<AssetRole>,
    environments: Vec<EnvironmentType>,
    related_nodes: Vec<String>,
    technologies: Vec<String>,
    endpoint_values: Vec<String>,
    shared_ip_neighbors: Vec<String>,
    shared_title_neighbors: Vec<String>,
    redirect_neighbors: Vec<String>,
    cluster_ids: Vec<String>,
    direct_relationship_counts: BTreeMap<String, usize>,
    anomaly_descriptions: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct AssetNeighborhood {
    neighbor_ids: Vec<String>,
    technologies: Vec<String>,
    parameters: Vec<String>,
    endpoint_values: Vec<String>,
    shared_ip_neighbors: Vec<String>,
    shared_title_neighbors: Vec<String>,
    redirect_neighbors: Vec<String>,
    cluster_ids: Vec<String>,
    direct_relationship_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
struct RelationshipMaps {
    node_by_id: BTreeMap<String, GraphNode>,
    edges_by_node: BTreeMap<String, Vec<GraphEdge>>,
    clusters_by_node: BTreeMap<String, Vec<AssetCluster>>,
    anomalies_by_node: BTreeMap<String, Vec<GraphAnomaly>>,
}

#[derive(Debug, Clone, Default)]
struct ApiAssetContext {
    endpoints: Vec<ApiEndpoint>,
    objects: Vec<ApiObject>,
    auth_observations: Vec<AuthObservation>,
    js_observations: Vec<JsObservation>,
    schema_observations: Vec<ApiSchema>,
    graphql_observations: Vec<GraphQlObservation>,
}

pub fn analyze_graph(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    clusters: &[AssetCluster],
    anomalies: &[GraphAnomaly],
    summary: &GraphSummary,
) -> SemanticAnalysis {
    analyze_graph_with_api(nodes, edges, clusters, anomalies, summary, None)
}

pub fn analyze_graph_with_api(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    clusters: &[AssetCluster],
    anomalies: &[GraphAnomaly],
    summary: &GraphSummary,
    api_bundle: Option<&ApiIntelBundle>,
) -> SemanticAnalysis {
    let mut analysis = analyze_graph_base(nodes, edges, clusters, anomalies, summary);
    if let Some(api_bundle) = api_bundle {
        enhance_analysis_with_api(&mut analysis, api_bundle);
    }
    analysis
}

fn analyze_graph_base(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    clusters: &[AssetCluster],
    anomalies: &[GraphAnomaly],
    summary: &GraphSummary,
) -> SemanticAnalysis {
    let maps = build_relationship_maps(nodes, edges, clusters, anomalies);
    let mut seeds = build_asset_seeds(nodes, &maps);
    let role_map = seeds
        .iter()
        .map(|seed| (seed.node_id.clone(), seed.roles.clone()))
        .collect::<BTreeMap<_, _>>();
    let environment_map = seeds
        .iter()
        .map(|seed| (seed.node_id.clone(), seed.environments.clone()))
        .collect::<BTreeMap<_, _>>();

    let mut observations = Vec::new();
    let mut risk_explanations = Vec::new();

    for seed in &mut seeds {
        let mut asset_observations =
            build_semantic_observations(seed, &maps, &role_map, &environment_map);
        let neighborhood_summary =
            build_neighborhood_summary(seed, &maps, &role_map, &environment_map);
        let risk_explanation = build_risk_explanation(
            seed,
            &asset_observations,
            &neighborhood_summary,
            &maps,
            &role_map,
            &environment_map,
        );

        let enriched_asset = EnrichedAsset {
            asset: seed.asset.clone(),
            semantic_tags: seed.semantic_tags.clone(),
            roles: seed.roles.clone(),
            environments: seed.environments.clone(),
            risk_explanations: vec![risk_explanation.clone()],
            related_nodes: seed.related_nodes.clone(),
            api_endpoints: Vec::new(),
            api_objects: Vec::new(),
            auth_observations: Vec::new(),
            js_observations: Vec::new(),
            schema_observations: Vec::new(),
            graphql_observations: Vec::new(),
            neighborhood_summary,
        };

        seed.related_nodes = enriched_asset.related_nodes.clone();
        observations.append(&mut asset_observations);
        risk_explanations.push(risk_explanation);
    }

    let mut semantic_assets = seeds
        .iter()
        .map(|seed| {
            let risk = risk_explanations
                .iter()
                .find(|risk| risk.asset == seed.asset)
                .cloned()
                .unwrap_or_else(|| informational_risk(seed.asset.clone()));
            EnrichedAsset {
                asset: seed.asset.clone(),
                semantic_tags: seed.semantic_tags.clone(),
                roles: seed.roles.clone(),
                environments: seed.environments.clone(),
                risk_explanations: vec![risk],
                related_nodes: seed.related_nodes.clone(),
                api_endpoints: Vec::new(),
                api_objects: Vec::new(),
                auth_observations: Vec::new(),
                js_observations: Vec::new(),
                schema_observations: Vec::new(),
                graphql_observations: Vec::new(),
                neighborhood_summary: build_neighborhood_summary(
                    seed,
                    &maps,
                    &role_map,
                    &environment_map,
                ),
            }
        })
        .collect::<Vec<_>>();

    sort_enriched_assets(&mut semantic_assets);
    sort_observations(&mut observations);
    sort_risk_explanations(&mut risk_explanations);

    let semantic_summary =
        build_semantic_summary(&semantic_assets, &observations, &risk_explanations);
    let enriched_graph = EnrichedGraph {
        assets: semantic_assets.clone(),
        observations: observations.clone(),
        risk_explanations: risk_explanations.clone(),
        original_graph_summary: summary.clone(),
        original_nodes: nodes.to_vec(),
        original_edges: edges.to_vec(),
        original_clusters: clusters.to_vec(),
        semantic_summary: semantic_summary.clone(),
    };

    SemanticAnalysis {
        semantic_assets,
        observations,
        risk_explanations,
        enriched_graph,
        summary: semantic_summary,
    }
}

fn enhance_analysis_with_api(analysis: &mut SemanticAnalysis, api_bundle: &ApiIntelBundle) {
    let mut risk_by_asset = analysis
        .risk_explanations
        .iter()
        .cloned()
        .map(|risk| (risk.asset.clone(), risk))
        .collect::<BTreeMap<_, _>>();
    let mut all_observations = analysis.observations.clone();

    for asset in &mut analysis.semantic_assets {
        let context = build_api_asset_context(asset, api_bundle);
        asset.api_endpoints = context.endpoints.clone();
        asset.api_objects = context.objects.clone();
        asset.auth_observations = context.auth_observations.clone();
        asset.js_observations = context.js_observations.clone();
        asset.schema_observations = context.schema_observations.clone();
        asset.graphql_observations = context.graphql_observations.clone();

        merge_api_semantic_tags(asset, &context);
        merge_api_roles_and_environments(asset, &context);

        let api_observations = build_api_semantic_observations(asset, &context);
        for observation in api_observations {
            push_observation(&mut all_observations, observation);
        }

        asset.neighborhood_summary =
            enhance_neighborhood_summary(&asset.neighborhood_summary, &context);

        let current_risk = risk_by_asset
            .remove(&asset.asset)
            .or_else(|| asset.risk_explanations.first().cloned())
            .unwrap_or_else(|| informational_risk(asset.asset.clone()));
        let asset_observations = all_observations
            .iter()
            .filter(|observation| observation.asset == asset.asset)
            .cloned()
            .collect::<Vec<_>>();
        let updated_risk =
            augment_risk_explanation(current_risk, asset, &context, &asset_observations);
        asset.risk_explanations = vec![updated_risk.clone()];
        risk_by_asset.insert(asset.asset.clone(), updated_risk);
    }

    analysis.observations = all_observations;
    analysis.risk_explanations = risk_by_asset.into_values().collect();
    sort_enriched_assets(&mut analysis.semantic_assets);
    sort_observations(&mut analysis.observations);
    sort_risk_explanations(&mut analysis.risk_explanations);

    let mut summary = build_semantic_summary(
        &analysis.semantic_assets,
        &analysis.observations,
        &analysis.risk_explanations,
    );
    summary.api_intelligence_warnings = api_bundle.warnings.clone();
    analysis.summary = summary.clone();
    analysis.enriched_graph.assets = analysis.semantic_assets.clone();
    analysis.enriched_graph.observations = analysis.observations.clone();
    analysis.enriched_graph.risk_explanations = analysis.risk_explanations.clone();
    analysis.enriched_graph.semantic_summary = summary;
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

    let schema_observations = api_bundle
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
        schema_observations,
        graphql_observations,
    }
}

fn merge_api_semantic_tags(asset: &mut EnrichedAsset, context: &ApiAssetContext) {
    let endpoint_paths = context
        .endpoints
        .iter()
        .map(|endpoint| endpoint.path.clone())
        .collect::<Vec<_>>();
    let endpoint_parameters = context
        .endpoints
        .iter()
        .flat_map(|endpoint| endpoint.parameters.iter().cloned())
        .collect::<Vec<_>>();
    let object_names = context
        .objects
        .iter()
        .map(|object| object.object_name.clone())
        .collect::<Vec<_>>();
    let auth_texts = context
        .auth_observations
        .iter()
        .flat_map(|observation| {
            let mut values = observation.indicators.clone();
            values.push(observation.auth_type.clone());
            values.extend(observation.evidence.clone());
            values
        })
        .collect::<Vec<_>>();
    let js_flags = context
        .js_observations
        .iter()
        .flat_map(|observation| observation.discovered_feature_flags.iter().cloned())
        .collect::<Vec<_>>();
    let js_texts = context
        .js_observations
        .iter()
        .flat_map(|observation| {
            let mut values = observation.discovered_endpoints.clone();
            values.extend(observation.discovered_roles.clone());
            values.extend(observation.discovered_auth_indicators.clone());
            values.extend(observation.evidence.clone());
            values
        })
        .collect::<Vec<_>>();
    let schema_texts = context
        .schema_observations
        .iter()
        .flat_map(|schema| {
            let mut values = vec![schema.schema_location.clone(), schema.schema_type.clone()];
            values.extend(schema.auth_methods.clone());
            values.extend(schema.objects.clone());
            values
        })
        .collect::<Vec<_>>();
    let mut environment_inputs = Vec::new();
    environment_inputs.extend(endpoint_paths.clone());
    environment_inputs.extend(js_flags.clone());
    environment_inputs.extend(js_texts.clone());
    environment_inputs.extend(schema_texts.clone());

    merge_tags(
        &mut asset.semantic_tags,
        classifiers::classify_api_family_tags(&endpoint_paths),
    );
    merge_tags(
        &mut asset.semantic_tags,
        classifiers::classify_auth_surface_tags(&auth_texts),
    );
    merge_tags(
        &mut asset.semantic_tags,
        classifiers::classify_object_sensitivity(&object_names),
    );
    merge_tags(
        &mut asset.semantic_tags,
        classifiers::classify_js_feature_flag_tags(&js_flags),
    );
    merge_tags(
        &mut asset.semantic_tags,
        classifiers::classify_environment_tags(&environment_inputs),
    );
    for path in endpoint_paths
        .iter()
        .chain(
            context
                .js_observations
                .iter()
                .flat_map(|observation| observation.discovered_endpoints.iter()),
        )
        .chain(
            context
                .graphql_observations
                .iter()
                .map(|observation| &observation.endpoint),
        )
    {
        merge_tags(
            &mut asset.semantic_tags,
            classifiers::classify_endpoint_intents(path),
        );
    }
    merge_tags(
        &mut asset.semantic_tags,
        classifiers::classify_parameter_intents(&endpoint_parameters),
    );
    if !context.schema_observations.is_empty() {
        merge_tags(
            &mut asset.semantic_tags,
            vec![SemanticTag {
                tag: "api_documentation".to_string(),
                category: "endpoint_intent".to_string(),
                confidence: 0.95,
                evidence: context
                    .schema_observations
                    .iter()
                    .map(|schema| {
                        format!(
                            "Schema '{}' was supplied by the API intelligence layer",
                            schema.schema_location
                        )
                    })
                    .collect(),
            }],
        );
    }
    if !context.graphql_observations.is_empty() {
        merge_tags(
            &mut asset.semantic_tags,
            vec![SemanticTag {
                tag: "graphql_surface".to_string(),
                category: "endpoint_intent".to_string(),
                confidence: 0.95,
                evidence: context
                    .graphql_observations
                    .iter()
                    .map(|observation| {
                        format!("GraphQL indicator matched '{}'", observation.endpoint)
                    })
                    .collect(),
            }],
        );
    }
}

fn merge_api_roles_and_environments(asset: &mut EnrichedAsset, context: &ApiAssetContext) {
    let mut role_inputs = context
        .endpoints
        .iter()
        .map(|endpoint| endpoint.path.clone())
        .collect::<Vec<_>>();
    role_inputs.extend(context.js_observations.iter().flat_map(|observation| {
        let mut values = observation.discovered_endpoints.clone();
        values.extend(observation.discovered_roles.clone());
        values
    }));
    role_inputs.extend(context.auth_observations.iter().flat_map(|observation| {
        let mut values = observation.indicators.clone();
        values.push(observation.auth_type.clone());
        values
    }));
    role_inputs.extend(
        context
            .schema_observations
            .iter()
            .flat_map(|schema| vec![schema.schema_location.clone(), schema.schema_type.clone()]),
    );
    role_inputs.extend(
        context
            .graphql_observations
            .iter()
            .map(|observation| observation.endpoint.clone()),
    );

    for role in classifiers::classify_roles(&role_inputs, &[]) {
        push_unique_role(&mut asset.roles, role);
    }
    for role_name in context
        .js_observations
        .iter()
        .flat_map(|observation| observation.discovered_roles.iter())
    {
        if let Some(role) = role_from_name(role_name) {
            push_unique_role(&mut asset.roles, role);
        }
    }
    if !context.auth_observations.is_empty()
        || asset.semantic_tags.iter().any(|tag| {
            matches!(
                tag.tag.as_str(),
                "auth_surface"
                    | "auth_bearer_indicator"
                    | "auth_oauth_indicator"
                    | "auth_jwt_indicator"
            )
        })
    {
        push_unique_role(&mut asset.roles, AssetRole::Authentication);
    }
    if !context.schema_observations.is_empty()
        || asset
            .semantic_tags
            .iter()
            .any(|tag| tag.tag == "api_documentation")
    {
        push_unique_role(&mut asset.roles, AssetRole::Documentation);
    }
    if !context.graphql_observations.is_empty()
        || asset
            .semantic_tags
            .iter()
            .any(|tag| matches!(tag.tag.as_str(), "graphql_surface" | "api_surface"))
    {
        push_unique_role(&mut asset.roles, AssetRole::ApiGateway);
    }
    if asset
        .semantic_tags
        .iter()
        .any(|tag| matches!(tag.tag.as_str(), "admin_surface" | "internal_surface"))
    {
        push_unique_role(&mut asset.roles, AssetRole::AdminDashboard);
    }
    if asset.roles.is_empty() {
        asset.roles.push(AssetRole::Unknown);
    }

    let mut environment_inputs = context
        .js_observations
        .iter()
        .flat_map(|observation| {
            let mut values = observation.discovered_endpoints.clone();
            values.extend(observation.discovered_feature_flags.clone());
            values.extend(observation.evidence.clone());
            values
        })
        .collect::<Vec<_>>();
    environment_inputs.extend(
        context
            .schema_observations
            .iter()
            .map(|schema| schema.schema_location.clone()),
    );
    environment_inputs.extend(
        context
            .endpoints
            .iter()
            .map(|endpoint| endpoint.path.clone()),
    );
    for environment in classifiers::classify_environments(&environment_inputs) {
        if environment != EnvironmentType::Unknown && !asset.environments.contains(&environment) {
            asset.environments.push(environment);
        }
    }
    asset.environments.sort();
    asset.environments.dedup();
    if asset.environments.is_empty() {
        asset.environments.push(EnvironmentType::Unknown);
    }
}

fn build_api_semantic_observations(
    asset: &EnrichedAsset,
    context: &ApiAssetContext,
) -> Vec<SemanticObservation> {
    let mut observations = Vec::new();
    let related_nodes = asset.related_nodes.clone();

    if context.endpoints.iter().any(|endpoint| {
        endpoint
            .semantic_tags
            .iter()
            .any(|tag| matches!(tag.tag.as_str(), "admin_surface" | "internal_surface"))
            || endpoint.path.to_ascii_lowercase().contains("/admin")
            || endpoint.path.to_ascii_lowercase().contains("/internal")
    }) {
        push_observation(
            &mut observations,
            semantic_from_api(ApiDerivedObservation {
                asset: asset.asset.clone(),
                observation_type: "api-surface".to_string(),
                description: "Privileged or internal API route candidate".to_string(),
                confidence: 0.9,
                evidence: context
                    .endpoints
                    .iter()
                    .filter(|endpoint| {
                        endpoint.path.to_ascii_lowercase().contains("/admin")
                            || endpoint.path.to_ascii_lowercase().contains("/internal")
                    })
                    .map(|endpoint| {
                        format!("Endpoint '{}' matched privileged API naming", endpoint.path)
                    })
                    .collect(),
                related_nodes: related_nodes.clone(),
            }),
        );
    }

    if !context.auth_observations.is_empty()
        || context
            .endpoints
            .iter()
            .any(|endpoint| !endpoint.auth_indicators.is_empty())
    {
        let mut evidence = context
            .auth_observations
            .iter()
            .map(|observation| {
                format!(
                    "Auth observation '{}' matched indicators: {}",
                    observation.auth_type,
                    observation.indicators.join(", ")
                )
            })
            .collect::<Vec<_>>();
        evidence.extend(
            context
                .endpoints
                .iter()
                .filter(|endpoint| !endpoint.auth_indicators.is_empty())
                .map(|endpoint| {
                    format!(
                        "Endpoint '{}' carried auth indicators: {}",
                        endpoint.path,
                        endpoint.auth_indicators.join(", ")
                    )
                }),
        );
        push_observation(
            &mut observations,
            semantic_from_api(ApiDerivedObservation {
                asset: asset.asset.clone(),
                observation_type: "auth-surface".to_string(),
                description: "Auth-related API surface candidate".to_string(),
                confidence: 0.9,
                evidence,
                related_nodes: related_nodes.clone(),
            }),
        );
    }

    if !context.schema_observations.is_empty() {
        push_observation(
            &mut observations,
            semantic_from_api(ApiDerivedObservation {
                asset: asset.asset.clone(),
                observation_type: "documentation".to_string(),
                description: "API documentation or schema exposure candidate".to_string(),
                confidence: 0.92,
                evidence: context
                    .schema_observations
                    .iter()
                    .map(|schema| {
                        format!(
                            "Schema '{}' [{}] was available for local analysis",
                            schema.schema_location, schema.schema_type
                        )
                    })
                    .collect(),
                related_nodes: related_nodes.clone(),
            }),
        );
    }

    if !context.graphql_observations.is_empty() {
        push_observation(
            &mut observations,
            semantic_from_api(ApiDerivedObservation {
                asset: asset.asset.clone(),
                observation_type: "graphql".to_string(),
                description: "GraphQL surface candidate".to_string(),
                confidence: 0.9,
                evidence: context
                    .graphql_observations
                    .iter()
                    .map(|observation| {
                        format!(
                            "GraphQL endpoint '{}' matched indicators: {}",
                            observation.endpoint,
                            join_limited(&observation.schema_indicators, 3)
                        )
                    })
                    .collect(),
                related_nodes: related_nodes.clone(),
            }),
        );
    }

    if context
        .objects
        .iter()
        .any(|object| object.inferred_sensitivity.eq_ignore_ascii_case("high"))
    {
        push_observation(
            &mut observations,
            semantic_from_api(ApiDerivedObservation {
                asset: asset.asset.clone(),
                observation_type: "api-object".to_string(),
                description: "Potentially sensitive API object candidate".to_string(),
                confidence: 0.88,
                evidence: context
                    .objects
                    .iter()
                    .filter(|object| object.inferred_sensitivity.eq_ignore_ascii_case("high"))
                    .map(|object| {
                        format!(
                            "Object '{}' was inferred as potentially sensitive",
                            object.object_name
                        )
                    })
                    .collect(),
                related_nodes: related_nodes.clone(),
            }),
        );
    }

    if js_has_hidden_internal_routes(&context.js_observations) {
        push_observation(
            &mut observations,
            semantic_from_api(ApiDerivedObservation {
                asset: asset.asset.clone(),
                observation_type: "javascript".to_string(),
                description: "JavaScript discovered hidden or internal route candidate".to_string(),
                confidence: 0.84,
                evidence: context
                    .js_observations
                    .iter()
                    .flat_map(|observation| {
                        observation
                            .discovered_endpoints
                            .iter()
                            .filter(|endpoint| is_internal_or_privileged_route(endpoint))
                            .map(|endpoint| {
                                format!(
                                    "JavaScript artifact '{}' referenced '{}'",
                                    observation.js_file, endpoint
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect(),
                related_nodes: related_nodes.clone(),
            }),
        );
    }

    if context
        .js_observations
        .iter()
        .any(|observation| !observation.discovered_feature_flags.is_empty())
    {
        push_observation(
            &mut observations,
            semantic_from_api(ApiDerivedObservation {
                asset: asset.asset.clone(),
                observation_type: "javascript-feature".to_string(),
                description: "JavaScript feature flag or privileged mode candidate".to_string(),
                confidence: 0.78,
                evidence: context
                    .js_observations
                    .iter()
                    .flat_map(|observation| {
                        observation.discovered_feature_flags.iter().map(|flag| {
                            format!(
                                "JavaScript artifact '{}' matched feature flag '{}'",
                                observation.js_file, flag
                            )
                        })
                    })
                    .collect(),
                related_nodes: related_nodes.clone(),
            }),
        );
    }

    let js_environment_hints = context
        .js_observations
        .iter()
        .flat_map(|observation| {
            let mut texts = observation.discovered_endpoints.clone();
            texts.extend(observation.discovered_feature_flags.clone());
            texts.extend(observation.evidence.clone());
            classifiers::classify_environment_tags(&texts)
        })
        .collect::<Vec<_>>();
    if !js_environment_hints.is_empty() {
        push_observation(
            &mut observations,
            semantic_from_api(ApiDerivedObservation {
                asset: asset.asset.clone(),
                observation_type: "javascript-environment".to_string(),
                description: "JavaScript environment hint candidate".to_string(),
                confidence: 0.76,
                evidence: js_environment_hints
                    .iter()
                    .flat_map(|tag| tag.evidence.clone())
                    .collect(),
                related_nodes: related_nodes.clone(),
            }),
        );
    }

    if has_token_or_header_indicators(context) {
        let mut evidence = context
            .auth_observations
            .iter()
            .flat_map(|observation| {
                observation
                    .indicators
                    .iter()
                    .filter(|indicator| is_token_or_header_indicator(indicator))
                    .map(|indicator| {
                        format!(
                            "Auth observation '{}' included '{}'",
                            observation.auth_type, indicator
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        evidence.extend(context.endpoints.iter().flat_map(|endpoint| {
            endpoint
                .auth_indicators
                .iter()
                .filter(|indicator| is_token_or_header_indicator(indicator))
                .map(|indicator| {
                    format!(
                        "Endpoint '{}' included auth indicator '{}'",
                        endpoint.path, indicator
                    )
                })
                .collect::<Vec<_>>()
        }));
        push_observation(
            &mut observations,
            semantic_from_api(ApiDerivedObservation {
                asset: asset.asset.clone(),
                observation_type: "auth-indicator".to_string(),
                description: "Token or auth-header indicator candidate".to_string(),
                confidence: 0.82,
                evidence,
                related_nodes,
            }),
        );
    }

    observations
}

fn enhance_neighborhood_summary(base: &str, context: &ApiAssetContext) -> String {
    let mut parts = Vec::new();
    if !context.endpoints.is_empty() {
        parts.push(format!(
            "references {} API endpoint candidate{}",
            context.endpoints.len(),
            plural(context.endpoints.len())
        ));
    }
    if !context.auth_observations.is_empty() {
        parts.push("includes auth flow indicators".to_string());
    }
    if !context.schema_observations.is_empty() {
        parts.push("links to local schema or documentation artifacts".to_string());
    }
    if !context.graphql_observations.is_empty() {
        parts.push("includes GraphQL indicators".to_string());
    }
    if context
        .objects
        .iter()
        .any(|object| object.inferred_sensitivity.eq_ignore_ascii_case("high"))
    {
        parts.push("references potentially sensitive API objects".to_string());
    }
    if js_has_hidden_internal_routes(&context.js_observations) {
        parts.push("includes JavaScript-derived hidden or internal routes".to_string());
    }

    if parts.is_empty() {
        return base.to_string();
    }

    if base.starts_with("No notable graph-neighborhood") {
        format!("This asset {}.", parts.join(", "))
    } else {
        format!("{base} It also {}.", parts.join(", "))
    }
}

fn augment_risk_explanation(
    mut risk: RiskExplanation,
    asset: &EnrichedAsset,
    context: &ApiAssetContext,
    observations: &[SemanticObservation],
) -> RiskExplanation {
    let mut score = risk.score;
    let mut contributing_factors = risk.contributing_factors.clone();
    let mut recommended_next_steps = risk.recommended_next_steps.clone();

    if !context.auth_observations.is_empty()
        || context
            .endpoints
            .iter()
            .any(|endpoint| !endpoint.auth_indicators.is_empty())
    {
        score += 12;
        contributing_factors.push("Auth-related API surface indicators".to_string());
        recommended_next_steps.push(
            "Review local auth-surface evidence carefully and confirm whether the observed flow or token handling is expected."
                .to_string(),
        );
    }
    if context.endpoints.iter().any(|endpoint| {
        endpoint.path.to_ascii_lowercase().contains("/admin")
            || endpoint.path.to_ascii_lowercase().contains("/internal")
    }) {
        score += 12;
        contributing_factors.push("Privileged or internal API route indicators".to_string());
    }
    if !context.schema_observations.is_empty() {
        score += 10;
        contributing_factors.push("API documentation or schema exposure indicators".to_string());
        recommended_next_steps.push(
            "Review schema and documentation artifacts manually to confirm whether exposed object models and routes are intentionally reachable."
                .to_string(),
        );
    }
    if !context.graphql_observations.is_empty() {
        score += 10;
        contributing_factors.push("GraphQL surface indicators".to_string());
        recommended_next_steps.push(
            "Review GraphQL-related artifacts carefully and confirm whether the surface and local schema hints are expected."
                .to_string(),
        );
    }
    if context
        .objects
        .iter()
        .any(|object| object.inferred_sensitivity.eq_ignore_ascii_case("high"))
    {
        score += 12;
        contributing_factors.push("Potentially sensitive API object references".to_string());
    }
    if js_has_hidden_internal_routes(&context.js_observations) {
        score += 8;
        contributing_factors
            .push("JavaScript-derived hidden or internal route indicators".to_string());
    }
    if has_token_or_header_indicators(context) {
        score += 8;
        contributing_factors.push("Token or auth-header indicators".to_string());
    }
    if context
        .js_observations
        .iter()
        .any(|observation| !observation.discovered_feature_flags.is_empty())
    {
        score += 5;
        contributing_factors.push("Feature flag or privileged mode indicators".to_string());
    }
    if observations.iter().any(|observation| {
        matches!(
            observation.observation_type.as_str(),
            "api-surface" | "auth-surface" | "graphql" | "javascript" | "api-object"
        )
    }) {
        score += 4;
    }

    contributing_factors.sort();
    contributing_factors.dedup();
    recommended_next_steps.push(
        "Validate API-related findings manually; these signals describe interesting candidates and require validation.".to_string(),
    );
    recommended_next_steps.sort();
    recommended_next_steps.dedup();

    score = score.clamp(0, 100);
    let risk_level = if score >= 60 {
        "high"
    } else if score >= 35 {
        "medium"
    } else if score >= 15 {
        "low"
    } else {
        "informational"
    };
    let factor_preview = contributing_factors
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>();
    let explanation = if factor_preview.is_empty() {
        format!(
            "This asset remains an informational candidate and requires validation. {}",
            asset.neighborhood_summary
        )
    } else {
        format!(
            "This asset is an interesting candidate worth manual review because of {}. {}",
            factor_preview.join(", ").to_ascii_lowercase(),
            asset.neighborhood_summary
        )
    };

    risk.risk_level = risk_level.to_string();
    risk.score = score;
    risk.explanation = explanation;
    risk.contributing_factors = contributing_factors;
    risk.recommended_next_steps = recommended_next_steps;
    risk
}

fn semantic_from_api(observation: ApiDerivedObservation) -> SemanticObservation {
    SemanticObservation {
        observation_id: observation_id(
            &observation.asset,
            &format!(
                "api-{}",
                slugify(&format!(
                    "{}-{}",
                    observation.observation_type, observation.description
                ))
            ),
        ),
        asset: observation.asset,
        observation_type: observation.observation_type,
        description: observation.description,
        evidence: observation.evidence,
        confidence: observation.confidence,
        related_nodes: observation.related_nodes,
    }
}

fn build_relationship_maps(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    clusters: &[AssetCluster],
    anomalies: &[GraphAnomaly],
) -> RelationshipMaps {
    let node_by_id = nodes
        .iter()
        .cloned()
        .map(|node| (node.id.clone(), node))
        .collect::<BTreeMap<_, _>>();

    let mut edges_by_node: BTreeMap<String, Vec<GraphEdge>> = BTreeMap::new();
    for edge in edges {
        edges_by_node
            .entry(edge.source.clone())
            .or_default()
            .push(edge.clone());
        edges_by_node
            .entry(edge.target.clone())
            .or_default()
            .push(edge.clone());
    }

    let mut clusters_by_node: BTreeMap<String, Vec<AssetCluster>> = BTreeMap::new();
    for cluster in clusters {
        for node_id in &cluster.related_nodes {
            clusters_by_node
                .entry(node_id.clone())
                .or_default()
                .push(cluster.clone());
        }
    }

    let mut anomalies_by_node: BTreeMap<String, Vec<GraphAnomaly>> = BTreeMap::new();
    for anomaly in anomalies {
        for node_id in &anomaly.related_nodes {
            anomalies_by_node
                .entry(node_id.clone())
                .or_default()
                .push(anomaly.clone());
        }
    }

    RelationshipMaps {
        node_by_id,
        edges_by_node,
        clusters_by_node,
        anomalies_by_node,
    }
}

fn build_asset_seeds(nodes: &[GraphNode], maps: &RelationshipMaps) -> Vec<AssetSeed> {
    let mut seeds = Vec::new();

    for node in nodes
        .iter()
        .filter(|node| matches!(node.node_type.as_str(), "host" | "url"))
    {
        let neighborhood = collect_neighborhood(node, maps);
        let text_fragments = build_text_fragments(node, &neighborhood, maps);
        let mut semantic_tags = Vec::new();
        merge_tags(
            &mut semantic_tags,
            classifiers::classify_environment_tags(&text_fragments),
        );
        merge_tags(
            &mut semantic_tags,
            classifiers::classify_role_tags(&text_fragments, &neighborhood.technologies),
        );
        merge_tags(
            &mut semantic_tags,
            classifiers::classify_technology_tags(&neighborhood.technologies),
        );

        for endpoint in &neighborhood.endpoint_values {
            merge_tags(
                &mut semantic_tags,
                classifiers::classify_endpoint_intents(endpoint),
            );
        }
        merge_tags(
            &mut semantic_tags,
            classifiers::classify_parameter_intents(&neighborhood.parameters),
        );

        let mut roles = classifiers::classify_roles(&text_fragments, &neighborhood.technologies);
        if semantic_tags
            .iter()
            .any(|tag| tag.tag == "api_documentation")
        {
            push_unique_role(&mut roles, AssetRole::Documentation);
        }
        if semantic_tags
            .iter()
            .any(|tag| matches!(tag.tag.as_str(), "admin_surface" | "internal_surface"))
        {
            push_unique_role(&mut roles, AssetRole::AdminDashboard);
        }
        if semantic_tags
            .iter()
            .any(|tag| matches!(tag.tag.as_str(), "api_surface" | "graphql_surface"))
        {
            push_unique_role(&mut roles, AssetRole::ApiGateway);
        }

        let environments = classifiers::classify_environments(&text_fragments);
        let anomaly_descriptions = maps
            .anomalies_by_node
            .get(&node.id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|anomaly| anomaly.description)
            .collect::<Vec<_>>();

        seeds.push(AssetSeed {
            node_id: node.id.clone(),
            asset: node.value.clone(),
            semantic_tags,
            roles,
            environments,
            related_nodes: neighborhood.neighbor_ids.clone(),
            technologies: neighborhood.technologies,
            endpoint_values: neighborhood.endpoint_values,
            shared_ip_neighbors: neighborhood.shared_ip_neighbors,
            shared_title_neighbors: neighborhood.shared_title_neighbors,
            redirect_neighbors: neighborhood.redirect_neighbors,
            cluster_ids: neighborhood.cluster_ids,
            direct_relationship_counts: neighborhood.direct_relationship_counts,
            anomaly_descriptions,
        });
    }

    seeds.sort_by(|left, right| left.asset.cmp(&right.asset));
    seeds
}

fn collect_neighborhood(node: &GraphNode, maps: &RelationshipMaps) -> AssetNeighborhood {
    let mut neighborhood = AssetNeighborhood::default();
    let connected_edges = maps
        .edges_by_node
        .get(&node.id)
        .cloned()
        .unwrap_or_default();

    for edge in connected_edges {
        let counterpart_id = if edge.source == node.id {
            edge.target.clone()
        } else {
            edge.source.clone()
        };

        push_unique_string(&mut neighborhood.neighbor_ids, counterpart_id.clone());
        *neighborhood
            .direct_relationship_counts
            .entry(format!("{:?}", edge.relationship))
            .or_default() += 1;

        match edge.relationship {
            RelationshipType::UsesTechnology => {
                if let Some(technology) = maps.node_by_id.get(&counterpart_id) {
                    push_unique_string(&mut neighborhood.technologies, technology.value.clone());
                }
            }
            RelationshipType::ContainsParameter => {
                if let Some(parameter) = maps.node_by_id.get(&counterpart_id) {
                    push_unique_string(&mut neighborhood.parameters, parameter.value.clone());
                }
            }
            RelationshipType::Hosts => {
                if node.node_type == "host" {
                    if let Some(url_node) = maps.node_by_id.get(&counterpart_id) {
                        push_unique_string(
                            &mut neighborhood.endpoint_values,
                            url_node.value.clone(),
                        );
                    }
                } else if node.node_type == "url" {
                    if let Some(host_node) = maps.node_by_id.get(&counterpart_id) {
                        push_unique_string(&mut neighborhood.neighbor_ids, host_node.id.clone());
                    }
                }
            }
            RelationshipType::SharesIp => {
                if let Some(host) = maps.node_by_id.get(&counterpart_id) {
                    push_unique_string(&mut neighborhood.shared_ip_neighbors, host.value.clone());
                }
            }
            RelationshipType::SharesTitle => {
                if let Some(host) = maps.node_by_id.get(&counterpart_id) {
                    push_unique_string(
                        &mut neighborhood.shared_title_neighbors,
                        host.value.clone(),
                    );
                }
            }
            RelationshipType::RedirectsTo => {
                if let Some(host) = maps.node_by_id.get(&counterpart_id) {
                    push_unique_string(&mut neighborhood.redirect_neighbors, host.value.clone());
                }
            }
            _ => {}
        }
    }

    if node.node_type == "url" {
        push_unique_string(&mut neighborhood.endpoint_values, node.value.clone());
        if let Ok(parsed) = Url::parse(&node.value) {
            let parameters = parsed
                .query_pairs()
                .map(|(key, _)| key.to_string())
                .collect::<Vec<_>>();
            for parameter in parameters {
                push_unique_string(&mut neighborhood.parameters, parameter);
            }
        }
    }

    for cluster in maps
        .clusters_by_node
        .get(&node.id)
        .cloned()
        .unwrap_or_default()
    {
        push_unique_string(&mut neighborhood.cluster_ids, cluster.cluster_id);
    }

    neighborhood
}

fn build_text_fragments(
    node: &GraphNode,
    neighborhood: &AssetNeighborhood,
    maps: &RelationshipMaps,
) -> Vec<String> {
    let mut fragments = Vec::new();
    fragments.push(node.value.clone());
    fragments.extend(node.tags.clone());
    fragments.extend(node.source_tools.clone());

    for value in node.metadata.values() {
        fragments.extend(flatten_json_value(value));
    }

    for endpoint in &neighborhood.endpoint_values {
        fragments.push(endpoint.clone());
    }
    for cluster_id in &neighborhood.cluster_ids {
        fragments.push(cluster_id.clone());
        if let Some(cluster) = maps.clusters_by_node.get(&node.id).and_then(|clusters| {
            clusters
                .iter()
                .find(|cluster| &cluster.cluster_id == cluster_id)
        }) {
            fragments.push(cluster.cluster_type.clone());
            fragments.extend(cluster.shared_indicators.clone());
        }
    }

    fragments.sort();
    fragments.dedup();
    fragments
}

fn build_semantic_observations(
    seed: &AssetSeed,
    maps: &RelationshipMaps,
    role_map: &BTreeMap<String, Vec<AssetRole>>,
    environment_map: &BTreeMap<String, Vec<EnvironmentType>>,
) -> Vec<SemanticObservation> {
    let mut observations = Vec::new();

    if seed.environments.contains(&EnvironmentType::Staging)
        && seed.roles.contains(&AssetRole::Authentication)
    {
        push_observation(
            &mut observations,
            SemanticObservation {
                observation_id: observation_id(&seed.asset, "staging-authentication"),
                asset: seed.asset.clone(),
                observation_type: "environment-role".to_string(),
                description: "Likely staging authentication surface".to_string(),
                evidence: vec![
                    "Environment classification matched staging indicators".to_string(),
                    "Role classification matched authentication indicators".to_string(),
                ],
                confidence: 0.92,
                related_nodes: seed.related_nodes.clone(),
            },
        );
    }

    if seed
        .technologies
        .iter()
        .any(|technology| is_operational_tooling(technology))
    {
        push_observation(
            &mut observations,
            SemanticObservation {
                observation_id: observation_id(&seed.asset, "operational-tooling"),
                asset: seed.asset.clone(),
                observation_type: "operational-tooling".to_string(),
                description: "Exposed operational tooling candidate".to_string(),
                evidence: seed
                    .technologies
                    .iter()
                    .filter(|technology| is_operational_tooling(technology))
                    .map(|technology| {
                        format!("Technology '{technology}' suggests operational tooling")
                    })
                    .collect(),
                confidence: 0.9,
                related_nodes: seed.related_nodes.clone(),
            },
        );
    }

    if seed
        .semantic_tags
        .iter()
        .any(|tag| tag.tag == "api_documentation")
    {
        push_observation(
            &mut observations,
            SemanticObservation {
                observation_id: observation_id(&seed.asset, "api-documentation"),
                asset: seed.asset.clone(),
                observation_type: "documentation".to_string(),
                description: "API schema/documentation candidate".to_string(),
                evidence: seed
                    .endpoint_values
                    .iter()
                    .filter(|endpoint| {
                        let lowered = endpoint.to_ascii_lowercase();
                        lowered.contains("/swagger") || lowered.contains("/openapi")
                    })
                    .map(|endpoint| {
                        format!("Endpoint '{endpoint}' matched API documentation intent")
                    })
                    .collect(),
                confidence: 0.9,
                related_nodes: seed.related_nodes.clone(),
            },
        );
    }

    if seed
        .semantic_tags
        .iter()
        .any(|tag| tag.tag == "sensitive_data_operation")
    {
        push_observation(
            &mut observations,
            SemanticObservation {
                observation_id: observation_id(&seed.asset, "sensitive-operation"),
                asset: seed.asset.clone(),
                observation_type: "sensitive-endpoint".to_string(),
                description: "Sensitive data operation candidate".to_string(),
                evidence: seed
                    .endpoint_values
                    .iter()
                    .filter(|endpoint| {
                        let lowered = endpoint.to_ascii_lowercase();
                        lowered.contains("/export") || lowered.contains("/backup")
                    })
                    .map(|endpoint| {
                        format!("Endpoint '{endpoint}' matched export or backup intent")
                    })
                    .collect(),
                confidence: 0.88,
                related_nodes: seed.related_nodes.clone(),
            },
        );
    }

    if shares_privileged_infrastructure(seed, maps, role_map, environment_map) {
        push_observation(
            &mut observations,
            SemanticObservation {
                observation_id: observation_id(&seed.asset, "shared-privileged-infrastructure"),
                asset: seed.asset.clone(),
                observation_type: "neighborhood".to_string(),
                description: "Shares infrastructure with privileged/admin-like asset".to_string(),
                evidence: privileged_neighbor_evidence(seed, maps, role_map, environment_map),
                confidence: 0.84,
                related_nodes: seed.related_nodes.clone(),
            },
        );
    }

    if seed.cluster_ids.iter().any(|cluster_id| {
        cluster_id.contains("admin-surface") || cluster_id.contains("shared-admin-infrastructure")
    }) {
        push_observation(
            &mut observations,
            SemanticObservation {
                observation_id: observation_id(&seed.asset, "admin-cluster-membership"),
                asset: seed.asset.clone(),
                observation_type: "cluster".to_string(),
                description: "Belongs to an admin-like infrastructure cluster".to_string(),
                evidence: seed
                    .cluster_ids
                    .iter()
                    .filter(|cluster_id| {
                        cluster_id.contains("admin-surface")
                            || cluster_id.contains("shared-admin-infrastructure")
                    })
                    .map(|cluster_id| {
                        format!("Cluster membership '{cluster_id}' suggests privileged grouping")
                    })
                    .collect(),
                confidence: 0.85,
                related_nodes: seed.related_nodes.clone(),
            },
        );
    }

    if seed
        .semantic_tags
        .iter()
        .any(|tag| tag.tag == "secret_or_token" || tag.tag == "debug_control")
    {
        push_observation(
            &mut observations,
            SemanticObservation {
                observation_id: observation_id(&seed.asset, "interesting-parameters"),
                asset: seed.asset.clone(),
                observation_type: "parameter-intent".to_string(),
                description: "Interesting parameter intent candidate".to_string(),
                evidence: seed
                    .semantic_tags
                    .iter()
                    .filter(|tag| tag.category == "parameter_intent")
                    .flat_map(|tag| tag.evidence.clone())
                    .collect(),
                confidence: 0.8,
                related_nodes: seed.related_nodes.clone(),
            },
        );
    }

    if let Some(anomalies) = maps.anomalies_by_node.get(&seed.node_id) {
        for anomaly in anomalies {
            push_observation(
                &mut observations,
                SemanticObservation {
                    observation_id: observation_id(
                        &seed.asset,
                        &format!("anomaly-{}", slugify(&anomaly.kind)),
                    ),
                    asset: seed.asset.clone(),
                    observation_type: "anomaly".to_string(),
                    description: format!("Graph anomaly candidate: {}", anomaly.description),
                    evidence: anomaly
                        .indicators
                        .iter()
                        .map(|indicator| format!("Anomaly indicator '{indicator}'"))
                        .collect(),
                    confidence: if anomaly.severity.eq_ignore_ascii_case("high") {
                        0.88
                    } else {
                        0.72
                    },
                    related_nodes: anomaly.related_nodes.clone(),
                },
            );
        }
    }

    observations
}

fn build_neighborhood_summary(
    seed: &AssetSeed,
    maps: &RelationshipMaps,
    role_map: &BTreeMap<String, Vec<AssetRole>>,
    environment_map: &BTreeMap<String, Vec<EnvironmentType>>,
) -> String {
    let mut parts = Vec::new();

    if !seed.shared_ip_neighbors.is_empty() {
        parts.push(format!(
            "shares infrastructure with {} related host{}",
            seed.shared_ip_neighbors.len(),
            plural(seed.shared_ip_neighbors.len())
        ));
    }
    if !seed.shared_title_neighbors.is_empty() {
        parts.push(format!(
            "shares titles with {} related host{}",
            seed.shared_title_neighbors.len(),
            plural(seed.shared_title_neighbors.len())
        ));
    }
    if !seed.technologies.is_empty() {
        parts.push(format!("uses {}", join_limited(&seed.technologies, 3)));
    }
    if !seed.redirect_neighbors.is_empty() {
        parts.push(format!(
            "participates in {} redirect relationship{}",
            seed.redirect_neighbors.len(),
            plural(seed.redirect_neighbors.len())
        ));
    }

    let reference_count = ["References", "LoadsScript", "ContainsParameter"]
        .into_iter()
        .map(|key| {
            seed.direct_relationship_counts
                .get(key)
                .copied()
                .unwrap_or_default()
        })
        .sum::<usize>();
    if reference_count > 0 {
        parts.push(format!(
            "connects to {} referenced endpoint or parameter node{}",
            reference_count,
            plural(reference_count)
        ));
    }
    if !seed.cluster_ids.is_empty() {
        let cluster_labels = seed
            .cluster_ids
            .iter()
            .filter_map(|cluster_id| {
                maps.clusters_by_node
                    .get(&seed.node_id)
                    .and_then(|clusters| {
                        clusters
                            .iter()
                            .find(|cluster| &cluster.cluster_id == cluster_id)
                    })
                    .map(|cluster| cluster.cluster_type.clone())
            })
            .collect::<Vec<_>>();
        if !cluster_labels.is_empty() {
            parts.push(format!("belongs to {}", join_limited(&cluster_labels, 2)));
        }
    }
    if shares_privileged_infrastructure(seed, maps, role_map, environment_map) {
        parts.push("is adjacent to privileged or operational neighbors".to_string());
    }

    if parts.is_empty() {
        return "No notable graph-neighborhood relationships were observed beyond the base graph."
            .to_string();
    }

    format!("This asset {}.", parts.join(", "))
}

fn build_risk_explanation(
    seed: &AssetSeed,
    observations: &[SemanticObservation],
    neighborhood_summary: &str,
    maps: &RelationshipMaps,
    role_map: &BTreeMap<String, Vec<AssetRole>>,
    environment_map: &BTreeMap<String, Vec<EnvironmentType>>,
) -> RiskExplanation {
    let mut score = 0;
    let mut contributing_factors = Vec::new();
    let mut recommended_next_steps = Vec::new();

    for environment in &seed.environments {
        match environment {
            EnvironmentType::Internal => {
                score += 20;
                contributing_factors.push("Internal environment indicators".to_string());
                recommended_next_steps.push(
                    "Review whether the asset is intentionally reachable and compare it with known internal naming conventions.".to_string(),
                );
            }
            EnvironmentType::Legacy => {
                score += 15;
                contributing_factors.push("Legacy environment indicators".to_string());
                recommended_next_steps.push(
                    "Confirm whether the asset is still expected to be active and compare it against current production equivalents.".to_string(),
                );
            }
            EnvironmentType::Staging => {
                score += 10;
                contributing_factors.push("Staging environment indicators".to_string());
            }
            EnvironmentType::Development | EnvironmentType::Testing => {
                score += 5;
                contributing_factors.push("Non-production environment indicators".to_string());
            }
            EnvironmentType::Production | EnvironmentType::Unknown => {}
        }
    }

    for role in &seed.roles {
        match role {
            AssetRole::AdminDashboard => {
                score += 20;
                contributing_factors.push("Administrative surface indicators".to_string());
                recommended_next_steps.push(
                    "Review linked administrative routes and confirm whether exposure is expected."
                        .to_string(),
                );
            }
            AssetRole::Monitoring | AssetRole::Logging | AssetRole::CICD => {
                score += 15;
                contributing_factors.push("Operational tooling indicators".to_string());
                recommended_next_steps.push(
                    "Inspect surrounding operational endpoints and confirm whether tooling exposure is intended.".to_string(),
                );
            }
            AssetRole::Authentication => {
                score += 12;
                contributing_factors.push("Authentication-related indicators".to_string());
                recommended_next_steps.push(
                    "Review authentication-adjacent routes and flows without assuming weakness."
                        .to_string(),
                );
            }
            AssetRole::Documentation => {
                score += 10;
                contributing_factors
                    .push("Documentation or schema exposure indicators".to_string());
                recommended_next_steps.push(
                    "Review documentation endpoints manually to confirm what information is intentionally exposed.".to_string(),
                );
            }
            AssetRole::Storage => {
                score += 10;
                contributing_factors.push("Storage-oriented indicators".to_string());
            }
            AssetRole::ApiGateway => {
                score += 8;
                contributing_factors.push("API-oriented surface indicators".to_string());
            }
            AssetRole::Analytics => {
                score += 6;
                contributing_factors.push("Analytics or metrics surface indicators".to_string());
            }
            AssetRole::CustomerApp => {
                score += 3;
                contributing_factors.push("Customer-facing application indicators".to_string());
            }
            AssetRole::Unknown => {}
        }
    }

    for tag in &seed.semantic_tags {
        match tag.tag.as_str() {
            "api_documentation" => {
                score += 10;
                contributing_factors.push("API documentation candidate".to_string());
            }
            "sensitive_data_operation" => {
                score += 12;
                contributing_factors.push("Sensitive data operation candidate".to_string());
                recommended_next_steps.push(
                    "Inspect export or backup related routes and validate whether the workflow is intentionally exposed.".to_string(),
                );
            }
            "debug_surface" | "debug_control" => {
                score += 12;
                contributing_factors.push("Debug-oriented indicator".to_string());
            }
            "secret_or_token" => {
                score += 12;
                contributing_factors
                    .push("Interesting token or secret parameter naming".to_string());
            }
            "redirect_control" => {
                score += 8;
                contributing_factors.push("Redirect-like parameter naming".to_string());
            }
            "file_or_path_control" => {
                score += 10;
                contributing_factors.push("File or path-oriented parameter naming".to_string());
            }
            "admin_surface" | "internal_surface" => {
                score += 10;
                contributing_factors.push("Privileged endpoint naming".to_string());
            }
            _ => {}
        }
    }

    if seed.cluster_ids.iter().any(|cluster_id| {
        cluster_id.contains("admin-surface") || cluster_id.contains("shared-admin-infrastructure")
    }) {
        score += 15;
        contributing_factors.push("Admin-like cluster membership".to_string());
    }

    if shares_privileged_infrastructure(seed, maps, role_map, environment_map) {
        score += 10;
        contributing_factors
            .push("Shares infrastructure with privileged or operational assets".to_string());
        recommended_next_steps.push(
            "Compare this asset with neighboring hosts on shared infrastructure and validate whether their exposure patterns align.".to_string(),
        );
    } else if shares_production_like_infrastructure(seed, maps, environment_map) {
        score += 5;
        contributing_factors
            .push("Shares infrastructure with production-like or unknown assets".to_string());
    }

    if !seed.anomaly_descriptions.is_empty() {
        score += 8;
        contributing_factors.extend(seed.anomaly_descriptions.iter().cloned());
    }

    if observations
        .iter()
        .any(|observation| observation.observation_type == "operational-tooling")
    {
        score += 10;
    }

    contributing_factors.sort();
    contributing_factors.dedup();
    recommended_next_steps.push(
        "Validate exposure manually; this enrichment describes prioritization candidates, not vulnerabilities.".to_string(),
    );
    recommended_next_steps.sort();
    recommended_next_steps.dedup();

    let score = score.clamp(0, 100);
    let risk_level = if score >= 60 {
        "high"
    } else if score >= 35 {
        "medium"
    } else if score >= 15 {
        "low"
    } else {
        "informational"
    };

    let factor_preview = contributing_factors
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>();
    let explanation = if factor_preview.is_empty() {
        format!(
            "This asset is currently informational and worth keeping for context. {}",
            neighborhood_summary
        )
    } else {
        format!(
            "This asset is an interesting candidate for manual review because of {}. {}",
            factor_preview.join(", ").to_ascii_lowercase(),
            neighborhood_summary
        )
    };

    RiskExplanation {
        asset: seed.asset.clone(),
        risk_level: risk_level.to_string(),
        score,
        explanation,
        contributing_factors,
        recommended_next_steps,
    }
}

fn build_semantic_summary(
    assets: &[EnrichedAsset],
    observations: &[SemanticObservation],
    risk_explanations: &[RiskExplanation],
) -> SemanticSummary {
    let mut role_counts = BTreeMap::new();
    let mut environment_counts = BTreeMap::new();
    let mut recommended_next_steps = BTreeSet::new();
    let mut sensitive_object_candidates = BTreeSet::new();
    let mut api_endpoint_count = 0usize;
    let mut auth_surface_count = 0usize;
    let mut js_observation_count = 0usize;
    let mut schema_observation_count = 0usize;
    let mut graphql_observation_count = 0usize;

    for asset in assets {
        api_endpoint_count += asset.api_endpoints.len();
        auth_surface_count += asset.auth_observations.len();
        js_observation_count += asset.js_observations.len();
        schema_observation_count += asset.schema_observations.len();
        graphql_observation_count += asset.graphql_observations.len();
        for role in &asset.roles {
            if *role != AssetRole::Unknown {
                *role_counts
                    .entry(role.as_str().to_string())
                    .or_insert(0usize) += 1;
            }
        }
        for environment in &asset.environments {
            if *environment != EnvironmentType::Unknown {
                *environment_counts
                    .entry(environment.as_str().to_string())
                    .or_insert(0usize) += 1;
            }
        }
        for explanation in &asset.risk_explanations {
            for step in &explanation.recommended_next_steps {
                recommended_next_steps.insert(step.clone());
            }
        }
        for object in &asset.api_objects {
            if object.inferred_sensitivity.eq_ignore_ascii_case("high")
                || object.inferred_sensitivity.eq_ignore_ascii_case("medium")
            {
                sensitive_object_candidates.insert(format!(
                    "{} -> {} ({})",
                    asset.asset, object.object_name, object.inferred_sensitivity
                ));
            }
        }
    }

    let mut highest_priority_assets = risk_explanations
        .iter()
        .map(|explanation| {
            (
                explanation.score,
                explanation.asset.clone(),
                explanation.risk_level.clone(),
            )
        })
        .collect::<Vec<_>>();
    highest_priority_assets
        .sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));

    let notable_neighborhood_observations = observations
        .iter()
        .filter(|observation| {
            matches!(
                observation.observation_type.as_str(),
                "neighborhood" | "operational-tooling" | "cluster"
            )
        })
        .map(|observation| observation.description.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(5)
        .collect::<Vec<_>>();

    SemanticSummary {
        generated_at: Utc::now(),
        asset_count: assets.len(),
        observation_count: observations.len(),
        risk_explanation_count: risk_explanations.len(),
        api_endpoint_count,
        auth_surface_count,
        js_observation_count,
        schema_observation_count,
        graphql_observation_count,
        top_roles: top_counts(role_counts),
        top_environments: top_counts(environment_counts),
        highest_priority_assets: highest_priority_assets
            .into_iter()
            .take(5)
            .map(|(score, asset, level)| format!("{asset} [{level}:{score}]"))
            .collect(),
        notable_neighborhood_observations,
        sensitive_object_candidates: sensitive_object_candidates.into_iter().take(8).collect(),
        api_intelligence_warnings: Vec::new(),
        recommended_next_steps: recommended_next_steps.into_iter().take(6).collect(),
    }
}

fn shares_privileged_infrastructure(
    seed: &AssetSeed,
    maps: &RelationshipMaps,
    role_map: &BTreeMap<String, Vec<AssetRole>>,
    environment_map: &BTreeMap<String, Vec<EnvironmentType>>,
) -> bool {
    seed.related_nodes.iter().any(|node_id| {
        let Some(node) = maps.node_by_id.get(node_id) else {
            return false;
        };
        let roles = role_map.get(node_id).cloned().unwrap_or_default();
        let environments = environment_map.get(node_id).cloned().unwrap_or_default();
        matches!(node.node_type.as_str(), "host" | "url")
            && (roles.iter().any(|role| {
                matches!(
                    role,
                    AssetRole::AdminDashboard
                        | AssetRole::Authentication
                        | AssetRole::Monitoring
                        | AssetRole::Logging
                        | AssetRole::CICD
                )
            }) || environments.contains(&EnvironmentType::Internal)
                || node.value.to_ascii_lowercase().contains("admin"))
    })
}

fn privileged_neighbor_evidence(
    seed: &AssetSeed,
    maps: &RelationshipMaps,
    role_map: &BTreeMap<String, Vec<AssetRole>>,
    environment_map: &BTreeMap<String, Vec<EnvironmentType>>,
) -> Vec<String> {
    let mut evidence = Vec::new();
    for node_id in &seed.related_nodes {
        let Some(node) = maps.node_by_id.get(node_id) else {
            continue;
        };
        let roles = role_map.get(node_id).cloned().unwrap_or_default();
        let environments = environment_map.get(node_id).cloned().unwrap_or_default();
        if roles.iter().any(|role| {
            matches!(
                role,
                AssetRole::AdminDashboard
                    | AssetRole::Authentication
                    | AssetRole::Monitoring
                    | AssetRole::Logging
                    | AssetRole::CICD
            )
        }) || environments.contains(&EnvironmentType::Internal)
            || node.value.to_ascii_lowercase().contains("admin")
        {
            evidence.push(format!(
                "Neighbor '{}' appears privileged or operational",
                node.value
            ));
        }
    }
    if evidence.is_empty() {
        evidence.push(
            "Shared infrastructure relationship suggests adjacency to higher-interest assets"
                .to_string(),
        );
    }
    evidence.sort();
    evidence.dedup();
    evidence
}

fn shares_production_like_infrastructure(
    seed: &AssetSeed,
    maps: &RelationshipMaps,
    environment_map: &BTreeMap<String, Vec<EnvironmentType>>,
) -> bool {
    seed.related_nodes.iter().any(|node_id| {
        let Some(node) = maps.node_by_id.get(node_id) else {
            return false;
        };
        let environments = environment_map.get(node_id).cloned().unwrap_or_default();
        matches!(node.node_type.as_str(), "host" | "url")
            && (environments.contains(&EnvironmentType::Production)
                || environments.contains(&EnvironmentType::Unknown))
    })
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
    let candidate = trimmed
        .split_whitespace()
        .nth(1)
        .unwrap_or(trimmed)
        .to_string();
    let path = if candidate.starts_with("http://") || candidate.starts_with("https://") {
        Url::parse(&candidate)
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

fn role_from_name(value: &str) -> Option<AssetRole> {
    match value.to_ascii_lowercase().as_str() {
        "authentication" => Some(AssetRole::Authentication),
        "admin_dashboard" => Some(AssetRole::AdminDashboard),
        "monitoring" => Some(AssetRole::Monitoring),
        "logging" => Some(AssetRole::Logging),
        "cicd" => Some(AssetRole::CICD),
        "api_gateway" => Some(AssetRole::ApiGateway),
        "storage" => Some(AssetRole::Storage),
        "analytics" => Some(AssetRole::Analytics),
        "documentation" => Some(AssetRole::Documentation),
        "customer_app" => Some(AssetRole::CustomerApp),
        _ => None,
    }
}

fn is_internal_or_privileged_route(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    [
        "/internal",
        "/admin",
        "/debug",
        "/beta",
        "/feature",
        "/staging",
        "/test",
        "/export",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn js_has_hidden_internal_routes(observations: &[JsObservation]) -> bool {
    observations.iter().any(|observation| {
        observation
            .discovered_endpoints
            .iter()
            .any(|endpoint| is_internal_or_privileged_route(endpoint))
    })
}

fn has_token_or_header_indicators(context: &ApiAssetContext) -> bool {
    context.auth_observations.iter().any(|observation| {
        observation
            .indicators
            .iter()
            .any(|indicator| is_token_or_header_indicator(indicator))
    }) || context.endpoints.iter().any(|endpoint| {
        endpoint
            .auth_indicators
            .iter()
            .any(|indicator| is_token_or_header_indicator(indicator))
    })
}

fn is_token_or_header_indicator(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    [
        "authorization",
        "header",
        "bearer",
        "jwt",
        "token",
        "api_key",
        "x-api-key",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn flatten_json_value(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::String(value) => vec![value.clone()],
        serde_json::Value::Array(values) => values.iter().flat_map(flatten_json_value).collect(),
        serde_json::Value::Object(object) => object.values().flat_map(flatten_json_value).collect(),
        _ => Vec::new(),
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

fn informational_risk(asset: String) -> RiskExplanation {
    RiskExplanation {
        asset,
        risk_level: "informational".to_string(),
        score: 0,
        explanation: "This asset is currently informational and should be retained for context."
            .to_string(),
        contributing_factors: vec!["No deterministic enrichment factors were observed yet.".to_string()],
        recommended_next_steps: vec![
            "Validate findings manually; this enrichment describes prioritization candidates, not vulnerabilities.".to_string(),
        ],
    }
}

fn is_operational_tooling(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    [
        "grafana",
        "prometheus",
        "kibana",
        "jenkins",
        "sentry",
        "datadog",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
}

fn push_unique_string(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
        values.sort();
    }
}

fn push_unique_role(values: &mut Vec<AssetRole>, value: AssetRole) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
        values.sort();
    }
}

fn merge_tags(target: &mut Vec<SemanticTag>, tags: Vec<SemanticTag>) {
    let mut by_key = target
        .iter()
        .cloned()
        .map(|tag| ((tag.tag.clone(), tag.category.clone()), tag))
        .collect::<BTreeMap<_, _>>();

    for tag in tags {
        let key = (tag.tag.clone(), tag.category.clone());
        if let Some(existing) = by_key.get_mut(&key) {
            existing.confidence = existing.confidence.max(tag.confidence);
            for evidence in tag.evidence {
                if !existing
                    .evidence
                    .iter()
                    .any(|existing| existing == &evidence)
                {
                    existing.evidence.push(evidence);
                }
            }
            existing.evidence.sort();
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

fn push_observation(target: &mut Vec<SemanticObservation>, observation: SemanticObservation) {
    if !target
        .iter()
        .any(|existing| existing.observation_id == observation.observation_id)
    {
        target.push(observation);
    }
}

fn sort_enriched_assets(assets: &mut Vec<EnrichedAsset>) {
    assets.sort_by(|left, right| {
        let left_score = left
            .risk_explanations
            .first()
            .map(|risk| risk.score)
            .unwrap_or_default();
        let right_score = right
            .risk_explanations
            .first()
            .map(|risk| risk.score)
            .unwrap_or_default();
        right_score
            .cmp(&left_score)
            .then_with(|| left.asset.cmp(&right.asset))
    });
}

fn sort_observations(observations: &mut Vec<SemanticObservation>) {
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
}

fn sort_risk_explanations(explanations: &mut Vec<RiskExplanation>) {
    explanations.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.asset.cmp(&right.asset))
    });
}

fn join_limited(values: &[String], limit: usize) -> String {
    if values.is_empty() {
        return "none".to_string();
    }
    if values.len() <= limit {
        return values.join(", ");
    }
    let mut preview = values.iter().take(limit).cloned().collect::<Vec<_>>();
    preview.push(format!("and {} more", values.len() - limit));
    preview.join(", ")
}

fn plural(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
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

fn observation_id(asset: &str, suffix: &str) -> String {
    format!("observation:{}:{suffix}", slugify(asset))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::Utc;

    use crate::{
        correlation::GraphAnomaly,
        models::{AssetCluster, GraphEdge, GraphNode, GraphSummary, RelationshipType},
    };

    use super::analyze_graph;

    fn sample_summary() -> GraphSummary {
        GraphSummary {
            generated_at: Utc::now(),
            node_count: 0,
            edge_count: 0,
            cluster_count: 0,
            anomaly_count: 0,
            top_technologies: Vec::new(),
            largest_clusters: Vec::new(),
            shared_infrastructure: Vec::new(),
            suspicious_naming: Vec::new(),
            likely_staging_systems: Vec::new(),
            likely_internal_systems: Vec::new(),
            redirect_chain_count: 0,
        }
    }

    #[test]
    fn semantic_observation_generation_creates_expected_candidates() {
        let nodes = vec![
            GraphNode {
                id: "host:staging-auth".to_string(),
                node_type: "host".to_string(),
                value: "staging-auth.example.com".to_string(),
                tags: Vec::new(),
                metadata: BTreeMap::new(),
                source_tools: vec!["httpx".to_string()],
                timestamps: vec![Utc::now()],
            },
            GraphNode {
                id: "url:swagger".to_string(),
                node_type: "url".to_string(),
                value: "https://staging-auth.example.com/swagger".to_string(),
                tags: Vec::new(),
                metadata: BTreeMap::new(),
                source_tools: vec!["katana".to_string()],
                timestamps: vec![Utc::now()],
            },
            GraphNode {
                id: "technology:grafana".to_string(),
                node_type: "technology".to_string(),
                value: "Grafana".to_string(),
                tags: Vec::new(),
                metadata: BTreeMap::new(),
                source_tools: vec!["WhatWeb".to_string()],
                timestamps: vec![Utc::now()],
            },
        ];
        let edges = vec![
            GraphEdge {
                source: "host:staging-auth".to_string(),
                target: "url:swagger".to_string(),
                relationship: RelationshipType::Hosts,
                confidence: 0.9,
                evidence: Vec::new(),
                timestamps: vec![Utc::now()],
            },
            GraphEdge {
                source: "host:staging-auth".to_string(),
                target: "technology:grafana".to_string(),
                relationship: RelationshipType::UsesTechnology,
                confidence: 0.9,
                evidence: Vec::new(),
                timestamps: vec![Utc::now()],
            },
        ];

        let analysis = analyze_graph(&nodes, &edges, &[], &[], &sample_summary());
        assert!(analysis
            .observations
            .iter()
            .any(|observation| observation.description.contains("staging authentication")));
        assert!(analysis
            .observations
            .iter()
            .any(|observation| observation.description.contains("operational tooling")));
        assert!(analysis
            .observations
            .iter()
            .any(|observation| observation.description.contains("documentation")));
    }

    #[test]
    fn risk_explanation_generation_is_cautious_and_ranked() {
        let nodes = vec![GraphNode {
            id: "host:internal-admin".to_string(),
            node_type: "host".to_string(),
            value: "internal-admin.example.com".to_string(),
            tags: Vec::new(),
            metadata: BTreeMap::new(),
            source_tools: vec!["httpx".to_string()],
            timestamps: vec![Utc::now()],
        }];
        let anomalies = vec![GraphAnomaly {
            id: "anomaly:admin".to_string(),
            kind: "suspicious-hostname".to_string(),
            severity: "high".to_string(),
            description: "Hostname contains admin-like keyword".to_string(),
            related_nodes: vec!["host:internal-admin".to_string()],
            indicators: vec!["internal".to_string(), "admin".to_string()],
        }];

        let analysis = analyze_graph(&nodes, &[], &[], &anomalies, &sample_summary());
        let risk = analysis
            .risk_explanations
            .iter()
            .find(|risk| risk.asset == "internal-admin.example.com")
            .expect("risk explanation should exist");
        assert!(matches!(risk.risk_level.as_str(), "medium" | "high"));
        assert!(risk.explanation.contains("candidate"));
        assert!(!risk.explanation.contains("vulnerability"));
    }

    #[test]
    fn graph_neighborhood_summary_generation_mentions_clusters_and_sharing() {
        let nodes = vec![
            GraphNode {
                id: "host:portal".to_string(),
                node_type: "host".to_string(),
                value: "portal.example.com".to_string(),
                tags: Vec::new(),
                metadata: BTreeMap::new(),
                source_tools: vec!["httpx".to_string()],
                timestamps: vec![Utc::now()],
            },
            GraphNode {
                id: "host:admin".to_string(),
                node_type: "host".to_string(),
                value: "admin.example.com".to_string(),
                tags: Vec::new(),
                metadata: BTreeMap::new(),
                source_tools: vec!["httpx".to_string()],
                timestamps: vec![Utc::now()],
            },
            GraphNode {
                id: "technology:grafana".to_string(),
                node_type: "technology".to_string(),
                value: "Grafana".to_string(),
                tags: Vec::new(),
                metadata: BTreeMap::new(),
                source_tools: vec!["WhatWeb".to_string()],
                timestamps: vec![Utc::now()],
            },
        ];
        let edges = vec![
            GraphEdge {
                source: "host:portal".to_string(),
                target: "host:admin".to_string(),
                relationship: RelationshipType::SharesIp,
                confidence: 0.9,
                evidence: Vec::new(),
                timestamps: vec![Utc::now()],
            },
            GraphEdge {
                source: "host:portal".to_string(),
                target: "technology:grafana".to_string(),
                relationship: RelationshipType::UsesTechnology,
                confidence: 0.9,
                evidence: Vec::new(),
                timestamps: vec![Utc::now()],
            },
        ];
        let clusters = vec![AssetCluster {
            cluster_id: "cluster:admin-surface".to_string(),
            cluster_type: "admin-surface".to_string(),
            related_nodes: vec!["host:portal".to_string(), "host:admin".to_string()],
            shared_indicators: vec!["admin-like".to_string()],
            risk_score: 70,
        }];

        let analysis = analyze_graph(&nodes, &edges, &clusters, &[], &sample_summary());
        let asset = analysis
            .semantic_assets
            .iter()
            .find(|asset| asset.asset == "portal.example.com")
            .expect("asset should exist");
        assert!(asset.neighborhood_summary.contains("shares infrastructure"));
        assert!(asset.neighborhood_summary.contains("belongs to"));
    }
}
