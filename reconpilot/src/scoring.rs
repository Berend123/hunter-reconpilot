use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result};
use serde::Deserialize;
use url::Url;

use crate::models::{
    AssetCluster, FindingRecord, GraphEdge, GraphNode, RelationshipType, ScoreBreakdown,
};

#[derive(Debug, Clone, Default)]
pub struct GraphRiskContext {
    pub internal_hosts: BTreeSet<String>,
    pub legacy_hosts: BTreeSet<String>,
    pub dashboard_hosts: BTreeSet<String>,
    pub shared_admin_cluster_hosts: BTreeSet<String>,
    pub shared_infrastructure_hosts: BTreeSet<String>,
    pub production_like_hosts: BTreeSet<String>,
    pub host_to_technologies: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Deserialize)]
struct GraphDocument {
    #[serde(default)]
    nodes: Vec<GraphNode>,
    #[serde(default)]
    edges: Vec<GraphEdge>,
    #[serde(default)]
    clusters: Vec<AssetCluster>,
}

#[derive(Debug, Deserialize)]
struct AnomalyDocument {
    #[serde(default)]
    anomalies: Vec<GraphAnomalyRecord>,
}

#[derive(Debug, Deserialize)]
struct GraphAnomalyRecord {
    kind: String,
    #[serde(default)]
    related_nodes: Vec<String>,
    #[serde(default)]
    indicators: Vec<String>,
}

pub fn load_findings(path: &Path) -> Result<Vec<FindingRecord>> {
    // TODO: Replace the loose loader with explicit schema-versioned adapters per phase.
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read findings input: {}", path.display()))?;
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if trimmed.starts_with('[') {
        let records = serde_json::from_str::<Vec<FindingRecord>>(trimmed)
            .with_context(|| format!("failed to parse JSON array: {}", path.display()))?;
        return Ok(records);
    }

    if trimmed.starts_with('{') {
        let value = serde_json::from_str::<serde_json::Value>(trimmed)
            .with_context(|| format!("failed to parse JSON object: {}", path.display()))?;

        if let Some(records) = value.get("scored_findings") {
            let findings = serde_json::from_value::<Vec<FindingRecord>>(records.clone())
                .with_context(|| {
                    format!("failed to parse scored_findings in {}", path.display())
                })?;
            return Ok(findings);
        }

        if let Some(records) = value.get("normalized_findings") {
            let findings = serde_json::from_value::<Vec<FindingRecord>>(records.clone())
                .with_context(|| {
                    format!("failed to parse normalized_findings in {}", path.display())
                })?;
            return Ok(findings);
        }
    }

    let mut findings = Vec::new();
    for (index, line) in trimmed.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let record = serde_json::from_str::<FindingRecord>(line).with_context(|| {
            format!(
                "failed to parse JSONL finding at line {} in {}",
                index + 1,
                path.display()
            )
        })?;
        findings.push(record);
    }

    Ok(findings)
}

#[allow(dead_code)]
pub fn score_findings(findings: Vec<FindingRecord>, keywords: &[String]) -> Vec<FindingRecord> {
    score_findings_with_graph(findings, keywords, None)
}

pub fn score_findings_with_graph(
    mut findings: Vec<FindingRecord>,
    keywords: &[String],
    graph: Option<&GraphRiskContext>,
) -> Vec<FindingRecord> {
    for finding in &mut findings {
        let breakdown = score_finding(finding, keywords, graph);
        finding.score = Some(breakdown);
    }

    findings
}

pub fn score_finding(
    finding: &FindingRecord,
    keywords: &[String],
    graph: Option<&GraphRiskContext>,
) -> ScoreBreakdown {
    // TODO: Blend heuristic scoring with future LLM-assisted reasoning once prompts and schemas are stable.
    let mut total = 0;
    let mut reasons = Vec::new();

    let searchable = format!(
        "{} {} {} {} {} {} {} {}",
        finding.title,
        finding.asset,
        finding.url.as_deref().unwrap_or_default(),
        finding.normalized_url.as_deref().unwrap_or_default(),
        finding.parameters.join(" "),
        finding.tags.join(" "),
        finding.notes.join(" "),
        finding.technology.join(" "),
    )
    .to_ascii_lowercase();

    for keyword in keywords {
        if searchable.contains(&keyword.to_ascii_lowercase()) {
            total += 10;
            reasons.push(format!("Matched keyword '{keyword}'"));
        }
    }

    if finding.source_tools.len() > 1 {
        total += 10;
        reasons.push("Observed in multiple tool sources".to_string());
    }

    if finding.kind.eq_ignore_ascii_case("parameter") && !finding.parameters.is_empty() {
        total += 5;
        reasons.push("Contains extracted parameters".to_string());
    }

    if let Some(graph) = graph {
        apply_graph_scoring(finding, graph, &mut total, &mut reasons);
    }

    if total == 0 {
        reasons.push(
            "No heuristic matches yet; keep for operator review if context demands it".to_string(),
        );
    }

    ScoreBreakdown {
        total: total.clamp(0, 100),
        reasons,
    }
}

pub fn load_graph_context_for_input(input: &Path) -> Result<Option<GraphRiskContext>> {
    let Some(root) = input
        .ancestors()
        .find(|path| path.join("maps").exists() && path.join("maps").is_dir())
    else {
        return Ok(None);
    };

    let graph_path = root.join("maps").join("graph.json");
    if !graph_path.exists() {
        return Ok(None);
    }

    let graph_raw = fs::read_to_string(&graph_path)
        .with_context(|| format!("failed to read graph document: {}", graph_path.display()))?;
    let graph = serde_json::from_str::<GraphDocument>(&graph_raw)
        .with_context(|| format!("failed to parse graph document: {}", graph_path.display()))?;

    let anomalies_path = root.join("maps").join("anomalies.json");
    let anomaly_document = if anomalies_path.exists() {
        let raw = fs::read_to_string(&anomalies_path).with_context(|| {
            format!(
                "failed to read anomaly document: {}",
                anomalies_path.display()
            )
        })?;
        serde_json::from_str::<AnomalyDocument>(&raw).with_context(|| {
            format!(
                "failed to parse anomaly document: {}",
                anomalies_path.display()
            )
        })?
    } else {
        AnomalyDocument {
            anomalies: Vec::new(),
        }
    };

    Ok(Some(build_graph_context(graph, anomaly_document)))
}

fn build_graph_context(graph: GraphDocument, anomalies: AnomalyDocument) -> GraphRiskContext {
    let mut context = GraphRiskContext::default();

    for edge in &graph.edges {
        if edge.relationship == RelationshipType::UsesTechnology {
            if let (Some(host), Some(technology)) = (
                host_value_from_node_id(&edge.source),
                technology_value_from_node_id(&edge.target),
            ) {
                context
                    .host_to_technologies
                    .entry(host)
                    .or_default()
                    .insert(technology);
            }
        }
    }

    for cluster in &graph.clusters {
        let hosts = cluster
            .related_nodes
            .iter()
            .filter_map(|node| host_value_from_node_id(node))
            .collect::<Vec<_>>();

        if cluster.cluster_type == "shared-admin-infrastructure" {
            context
                .shared_admin_cluster_hosts
                .extend(hosts.iter().cloned());
        }
        if cluster.cluster_type == "shared-infrastructure" {
            context
                .shared_infrastructure_hosts
                .extend(hosts.iter().cloned());
            if hosts
                .iter()
                .any(|host| !contains_any(host, &["staging", "dev", "test", "internal", "admin"]))
            {
                context.production_like_hosts.extend(hosts);
            }
        }
    }

    for anomaly in anomalies.anomalies {
        let hosts = anomaly
            .related_nodes
            .iter()
            .filter_map(|node| host_value_from_node_id(node))
            .collect::<Vec<_>>();

        match anomaly.kind.as_str() {
            "suspicious-hostname" => {
                if anomaly
                    .indicators
                    .iter()
                    .any(|indicator| indicator.eq_ignore_ascii_case("internal"))
                {
                    context.internal_hosts.extend(hosts);
                }
            }
            "legacy-technology" => {
                context.legacy_hosts.extend(hosts);
            }
            "exposed-dashboard" | "admin-like-surface" | "shared-admin-infrastructure" => {
                context.dashboard_hosts.extend(hosts);
            }
            _ => {}
        }
    }

    for node in graph.nodes {
        if node.node_type != "host" {
            continue;
        }

        let host = node.value.to_ascii_lowercase();
        if contains_any(&host, &["internal", "corp", "intra"]) {
            context.internal_hosts.insert(host);
        }
    }

    context
}

fn apply_graph_scoring(
    finding: &FindingRecord,
    graph: &GraphRiskContext,
    total: &mut i32,
    reasons: &mut Vec<String>,
) {
    let host_candidates = derive_host_candidates(finding);
    let searchable = format!(
        "{} {} {} {} {}",
        finding.title,
        finding.asset,
        finding.url.as_deref().unwrap_or_default(),
        finding.normalized_url.as_deref().unwrap_or_default(),
        finding.notes.join(" ")
    )
    .to_ascii_lowercase();

    if contains_any(&searchable, &["internal"])
        || host_candidates
            .iter()
            .any(|host| graph.internal_hosts.contains(host))
    {
        *total += 20;
        reasons
            .push("Graph context suggests internal naming or internal asset context".to_string());
    }

    if host_candidates
        .iter()
        .any(|host| graph.shared_admin_cluster_hosts.contains(host))
    {
        *total += 15;
        reasons.push("Host is part of a shared admin cluster".to_string());
    }

    let uses_legacy = host_candidates.iter().any(|host| {
        graph.legacy_hosts.contains(host)
            || graph
                .host_to_technologies
                .get(host)
                .map(|technologies| {
                    technologies.iter().any(|technology| {
                        contains_any(
                            &technology.to_ascii_lowercase(),
                            &[
                                "drupal 7",
                                "php 5",
                                "apache 2.2",
                                "jquery 1",
                                "iis 6",
                                "asp.net 4",
                            ],
                        )
                    })
                })
                .unwrap_or(false)
    });
    if uses_legacy {
        *total += 10;
        reasons.push("Graph context indicates legacy technology".to_string());
    }

    let exposed_dashboard = host_candidates.iter().any(|host| {
        graph.dashboard_hosts.contains(host)
            || graph
                .host_to_technologies
                .get(host)
                .map(|technologies| {
                    technologies.iter().any(|technology| {
                        contains_any(
                            &technology.to_ascii_lowercase(),
                            &["grafana", "jenkins", "kibana", "prometheus"],
                        )
                    })
                })
                .unwrap_or(false)
    });
    if exposed_dashboard {
        *total += 15;
        reasons
            .push("Graph context suggests an exposed dashboard or admin-like surface".to_string());
    }

    if host_candidates.iter().any(|host| {
        graph.shared_infrastructure_hosts.contains(host)
            && graph.production_like_hosts.contains(host)
    }) {
        *total += 5;
        reasons.push("Host shares infrastructure with production-like systems".to_string());
    }
}

fn derive_host_candidates(finding: &FindingRecord) -> BTreeSet<String> {
    let mut hosts = BTreeSet::new();

    for candidate in [
        Some(finding.asset.as_str()),
        finding.url.as_deref(),
        finding.normalized_url.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(host) = host_from_target(candidate) {
            hosts.insert(host.to_ascii_lowercase());
        } else if candidate.contains('.') {
            hosts.insert(candidate.to_ascii_lowercase());
        }
    }

    hosts
}

fn host_from_target(value: &str) -> Option<String> {
    if value.starts_with("http://") || value.starts_with("https://") {
        return Url::parse(value)
            .ok()
            .and_then(|parsed| parsed.host_str().map(ToOwned::to_owned));
    }
    Some(value.trim_start_matches("*.").to_string())
}

fn host_value_from_node_id(node_id: &str) -> Option<String> {
    node_id
        .strip_prefix("host:")
        .map(|value| value.replace('-', "."))
        .map(|value| value.to_ascii_lowercase())
}

fn technology_value_from_node_id(node_id: &str) -> Option<String> {
    node_id
        .strip_prefix("technology:")
        .map(|value| value.replace('-', " "))
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}
