use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use url::Url;
use walkdir::WalkDir;

use crate::{
    correlation::{self, CorrelationView, GraphAnomaly},
    models::{
        AppMapEdge, AppMapNode, AssetCluster, CorrelationEvidence, DnsRecord, GraphEdge, GraphNode,
        GraphSummary, RelationshipType, ScreenshotRecord, TechFingerprint,
    },
    utils,
};

#[derive(Debug, Clone)]
pub struct GraphOutcome {
    pub plan_path: PathBuf,
    pub preview_path: PathBuf,
    pub graph_json_path: Option<PathBuf>,
    pub graph_markdown_path: Option<PathBuf>,
    pub clusters_json_path: Option<PathBuf>,
    pub clusters_markdown_path: Option<PathBuf>,
    pub anomalies_json_path: Option<PathBuf>,
    pub summary_json_path: Option<PathBuf>,
    pub executed: bool,
}

#[derive(Debug, Clone)]
struct GraphInputLayout {
    root: PathBuf,
    raw: PathBuf,
    dns: PathBuf,
    tech: PathBuf,
    maps: PathBuf,
    screenshots: PathBuf,
    plans: PathBuf,
    examples: Option<PathBuf>,
    js: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct GraphPlan {
    generated_at: DateTime<Utc>,
    input_root: PathBuf,
    output_root: PathBuf,
    execute_requested: bool,
    required_directories: Vec<String>,
    discovered_inputs: Vec<String>,
    planned_outputs: Vec<String>,
    notes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GraphDocument {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    clusters: Vec<AssetCluster>,
    summary: GraphSummary,
}

#[derive(Debug, Serialize)]
struct ClusterDocument {
    generated_at: DateTime<Utc>,
    clusters: Vec<AssetCluster>,
}

#[derive(Debug, Serialize)]
struct AnomalyDocument {
    generated_at: DateTime<Utc>,
    anomalies: Vec<GraphAnomaly>,
}

#[derive(Debug, Clone, Default)]
struct ArtifactBundle {
    httpx: Vec<HttpxRecord>,
    katana: Vec<KatanaRecord>,
    dns: Vec<DnsRecord>,
    tech: Vec<TechFingerprint>,
    screenshots: Vec<ScreenshotRecord>,
    js_references: Vec<JsReferenceRecord>,
    app_map_nodes: Vec<AppMapNode>,
    app_map_edges: Vec<AppMapEdge>,
}

#[derive(Debug, Clone)]
struct GraphBuildResult {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    clusters: Vec<AssetCluster>,
    anomalies: Vec<GraphAnomaly>,
    summary: GraphSummary,
}

#[derive(Debug, Clone, Default)]
struct GraphBuilder {
    nodes: BTreeMap<String, GraphNode>,
    edges: BTreeMap<(String, String, RelationshipType), GraphEdge>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ImportedMapDocument {
    #[serde(default)]
    nodes: Vec<AppMapNode>,
    #[serde(default)]
    edges: Vec<AppMapEdge>,
}

#[derive(Debug, Clone, Default)]
struct HttpxRecord {
    host: String,
    url: Option<String>,
    title: Option<String>,
    technologies: Vec<String>,
    ip_addresses: Vec<String>,
    port: Option<u16>,
    redirect_target: Option<String>,
    favicon_hash: Option<String>,
    status_code: Option<u16>,
    timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
struct KatanaRecord {
    url: String,
    source: String,
    content_type: Option<String>,
    timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
struct JsReferenceRecord {
    script_url: String,
    source_url: Option<String>,
    endpoint_url: String,
    parameters: Vec<String>,
    timestamp: Option<DateTime<Utc>>,
}

#[allow(dead_code)]
pub(crate) trait GraphStorageAdapter {
    // TODO: add SQLite sink support once graph schemas stabilize.
    // TODO: add DuckDB export support for local analytics workflows.
    // TODO: add Neo4j export support for richer relationship exploration.
    fn persist_graph(&self, _document: &GraphDocument) -> Result<()> {
        Ok(())
    }
}

pub fn print_graph_summary(input_root: &Path, output_root: &Path, outcome: &GraphOutcome) {
    println!("ReconPilot graph summary");
    println!("Input root: {}", input_root.display());
    println!("Output maps: {}", output_root.display());
    println!(
        "Mode: {}",
        if outcome.executed {
            "execute"
        } else {
            "dry-run (default)"
        }
    );
    println!("Plan: {}", outcome.plan_path.display());
    println!("Preview: {}", outcome.preview_path.display());

    if let Some(path) = &outcome.graph_json_path {
        println!("Graph JSON: {}", path.display());
    }
    if let Some(path) = &outcome.graph_markdown_path {
        println!("Graph markdown: {}", path.display());
    }
    if let Some(path) = &outcome.clusters_json_path {
        println!("Clusters JSON: {}", path.display());
    }
    if let Some(path) = &outcome.clusters_markdown_path {
        println!("Clusters markdown: {}", path.display());
    }
    if let Some(path) = &outcome.anomalies_json_path {
        println!("Anomalies JSON: {}", path.display());
    }
    if let Some(path) = &outcome.summary_json_path {
        println!("Summary JSON: {}", path.display());
    }
}

pub fn run_graph_engine(
    input_root: &Path,
    output_root: &Path,
    execute: bool,
) -> Result<GraphOutcome> {
    let layout = validate_graph_layout(input_root, output_root)?;
    let bundle = load_artifacts(&layout)?;
    let preview = render_graph_preview(&layout, output_root, &bundle, execute);
    let preview_path = output_root.join("graph-preview.md");
    utils::write_string(&preview_path, &preview)?;

    let plan = GraphPlan {
        generated_at: Utc::now(),
        input_root: layout.root.clone(),
        output_root: output_root.to_path_buf(),
        execute_requested: execute,
        required_directories: vec![
            layout.raw.display().to_string(),
            layout.dns.display().to_string(),
            layout.tech.display().to_string(),
            layout.maps.display().to_string(),
            layout.screenshots.display().to_string(),
        ],
        discovered_inputs: discovered_input_labels(&bundle),
        planned_outputs: vec![
            output_root.join("graph.json").display().to_string(),
            output_root.join("graph.md").display().to_string(),
            output_root.join("clusters.json").display().to_string(),
            output_root.join("clusters.md").display().to_string(),
            output_root.join("anomalies.json").display().to_string(),
            output_root.join("graph-summary.json").display().to_string(),
        ],
        notes: vec![
            "Graph generation is dry-run by default.".to_string(),
            "Graph execution is local-only and does not contact targets.".to_string(),
            "Future storage adapters can export the graph to SQLite, DuckDB, or Neo4j.".to_string(),
        ],
    };
    let plan_path = layout.plans.join("graph-plan.json");
    utils::write_json_pretty(&plan_path, &plan)?;

    if !execute {
        return Ok(GraphOutcome {
            plan_path,
            preview_path,
            graph_json_path: None,
            graph_markdown_path: None,
            clusters_json_path: None,
            clusters_markdown_path: None,
            anomalies_json_path: None,
            summary_json_path: None,
            executed: false,
        });
    }

    let build = build_graph(&bundle);
    let graph_json_path = output_root.join("graph.json");
    let graph_markdown_path = output_root.join("graph.md");
    let clusters_json_path = output_root.join("clusters.json");
    let clusters_markdown_path = output_root.join("clusters.md");
    let anomalies_json_path = output_root.join("anomalies.json");
    let summary_json_path = output_root.join("graph-summary.json");

    let graph_document = GraphDocument {
        nodes: build.nodes.clone(),
        edges: build.edges.clone(),
        clusters: build.clusters.clone(),
        summary: build.summary.clone(),
    };
    utils::write_json_pretty(&graph_json_path, &graph_document)?;
    utils::write_string(
        &graph_markdown_path,
        &render_graph_markdown(&graph_document, &build.anomalies),
    )?;
    utils::write_json_pretty(
        &clusters_json_path,
        &ClusterDocument {
            generated_at: Utc::now(),
            clusters: build.clusters.clone(),
        },
    )?;
    utils::write_string(
        &clusters_markdown_path,
        &render_clusters_markdown(&build.clusters),
    )?;
    utils::write_json_pretty(
        &anomalies_json_path,
        &AnomalyDocument {
            generated_at: Utc::now(),
            anomalies: build.anomalies.clone(),
        },
    )?;
    utils::write_json_pretty(&summary_json_path, &build.summary)?;

    Ok(GraphOutcome {
        plan_path,
        preview_path,
        graph_json_path: Some(graph_json_path),
        graph_markdown_path: Some(graph_markdown_path),
        clusters_json_path: Some(clusters_json_path),
        clusters_markdown_path: Some(clusters_markdown_path),
        anomalies_json_path: Some(anomalies_json_path),
        summary_json_path: Some(summary_json_path),
        executed: true,
    })
}

fn validate_graph_layout(input_root: &Path, output_root: &Path) -> Result<GraphInputLayout> {
    if !input_root.exists() {
        bail!("graph input root does not exist: {}", input_root.display());
    }
    if !input_root.is_dir() {
        bail!(
            "graph input root is not a directory: {}",
            input_root.display()
        );
    }

    let raw = input_root.join("raw");
    let dns = input_root.join("dns");
    let tech = input_root.join("tech");
    let maps = input_root.join("maps");
    let screenshots = input_root.join("screenshots");

    for path in [&raw, &dns, &tech, &maps, &screenshots] {
        if !path.exists() || !path.is_dir() {
            bail!(
                "required graph input directory is missing: {}",
                path.display()
            );
        }
    }

    utils::ensure_directory(output_root)?;
    let plans = input_root.join("plans");
    utils::ensure_directory(&plans)?;

    let examples = input_root
        .parent()
        .map(|parent| parent.join("examples"))
        .filter(|path| path.exists() && path.is_dir());
    let js_path = input_root.join("js");
    let js = if js_path.exists() && js_path.is_dir() {
        Some(js_path)
    } else {
        None
    };

    Ok(GraphInputLayout {
        root: input_root.to_path_buf(),
        raw,
        dns,
        tech,
        maps,
        screenshots,
        plans,
        examples,
        js,
    })
}

fn load_artifacts(layout: &GraphInputLayout) -> Result<ArtifactBundle> {
    let app_map = load_app_map(layout)?;
    Ok(ArtifactBundle {
        httpx: load_httpx_records(layout)?,
        katana: load_katana_records(layout)?,
        dns: load_dns_records(layout)?,
        tech: load_whatweb_records(layout)?,
        screenshots: load_screenshot_records(layout)?,
        js_references: load_js_reference_records(layout)?,
        app_map_nodes: app_map.nodes,
        app_map_edges: app_map.edges,
    })
}

fn load_httpx_records(layout: &GraphInputLayout) -> Result<Vec<HttpxRecord>> {
    let mut records = Vec::new();
    let httpx_dir = layout.raw.join("httpx");

    if httpx_dir.exists() {
        for entry in WalkDir::new(&httpx_dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
        {
            records.extend(parse_httpx_file(entry.path())?);
        }
    }

    if records.is_empty() {
        if let Some(examples) = &layout.examples {
            let sample = examples.join("sample-httpx.json");
            if sample.exists() {
                records.extend(parse_httpx_file(&sample)?);
            }
        }
    }

    dedupe_httpx_records(&mut records);
    Ok(records)
}

fn load_katana_records(layout: &GraphInputLayout) -> Result<Vec<KatanaRecord>> {
    let mut records = Vec::new();
    let katana_dir = layout.raw.join("katana");

    if katana_dir.exists() {
        for entry in WalkDir::new(&katana_dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
        {
            records.extend(parse_katana_file(entry.path())?);
        }
    }

    if records.is_empty() {
        if let Some(examples) = &layout.examples {
            let sample = examples.join("sample-katana.jsonl");
            if sample.exists() {
                records.extend(parse_katana_file(&sample)?);
            }
        }
    }

    dedupe_katana_records(&mut records);
    Ok(records)
}

fn load_dns_records(layout: &GraphInputLayout) -> Result<Vec<DnsRecord>> {
    let path = layout.dns.join("dnsx.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for (index, line) in fs::read_to_string(&path)
        .with_context(|| format!("failed to read dnsx output at {}", path.display()))?
        .lines()
        .enumerate()
    {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let value = serde_json::from_str::<Value>(line).with_context(|| {
            format!(
                "failed to parse dnsx JSONL line {} in {}",
                index + 1,
                path.display()
            )
        })?;
        let name = json_string(&value, &["host", "input", "name"]);
        if name.is_empty() {
            continue;
        }

        let record_type = json_string(&value, &["type"]).if_empty_then("A".to_string());
        let values = json_array_strings(&value, &["a", "answers", "response", "value"]);
        let notes = vec!["Parsed from dnsx JSONL output.".to_string()];
        records.push(DnsRecord {
            name,
            record_type,
            values,
            source_tool: "dnsx".to_string(),
            notes,
        });
    }

    Ok(records)
}

fn load_whatweb_records(layout: &GraphInputLayout) -> Result<Vec<TechFingerprint>> {
    let path = layout.tech.join("whatweb.json");
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read WhatWeb output at {}", path.display()))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let values = if trimmed.starts_with('[') {
        serde_json::from_str::<Vec<Value>>(trimmed).unwrap_or_default()
    } else if trimmed.starts_with('{') {
        vec![serde_json::from_str::<Value>(trimmed).unwrap_or_default()]
    } else {
        Vec::new()
    };

    let mut fingerprints = Vec::new();
    for value in values {
        if value.is_null() {
            continue;
        }

        let target = json_string(&value, &["target", "url", "host"]);
        if target.is_empty() {
            continue;
        }

        let technologies = json_object_keys(&value, &["plugins"]);
        fingerprints.push(TechFingerprint {
            target,
            url: optional_json_string(&value, &["url"]),
            technologies,
            categories: vec!["web-tech".to_string()],
            source_tool: "WhatWeb".to_string(),
            confidence: None,
            notes: vec!["Parsed from WhatWeb JSON output.".to_string()],
        });
    }

    Ok(fingerprints)
}

fn load_screenshot_records(layout: &GraphInputLayout) -> Result<Vec<ScreenshotRecord>> {
    let mut records = Vec::new();
    let jsonl_path = layout.screenshots.join("gowitness.jsonl");

    if jsonl_path.exists() {
        for (index, line) in fs::read_to_string(&jsonl_path)
            .with_context(|| {
                format!(
                    "failed to read gowitness output at {}",
                    jsonl_path.display()
                )
            })?
            .lines()
            .enumerate()
        {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let value = serde_json::from_str::<Value>(line).with_context(|| {
                format!(
                    "failed to parse gowitness JSONL line {} in {}",
                    index + 1,
                    jsonl_path.display()
                )
            })?;
            let target = json_string(&value, &["url", "target", "final_url"]);
            if target.is_empty() {
                continue;
            }

            records.push(ScreenshotRecord {
                target,
                image_path: optional_json_string(&value, &["screenshot"]).map(PathBuf::from),
                source_tool: "gowitness".to_string(),
                title: optional_json_string(&value, &["title"]),
                captured_at: optional_json_datetime(&value, &["timestamp", "captured_at"]),
                notes: vec![
                    "Parsed from gowitness JSONL output.".to_string(),
                    "Visual similarity and favicon clustering are placeholders in this phase."
                        .to_string(),
                ],
            });
        }
    }

    for entry in WalkDir::new(&layout.screenshots)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let extension = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());

        if matches!(extension.as_deref(), Some("png" | "jpg" | "jpeg" | "webp")) {
            records.push(ScreenshotRecord {
                target: entry.file_name().to_string_lossy().to_string(),
                image_path: Some(entry.path().to_path_buf()),
                source_tool: "gowitness".to_string(),
                title: None,
                captured_at: None,
                notes: vec![
                    "Discovered screenshot artifact on disk.".to_string(),
                    "Visual clustering remains heuristic-only in this phase.".to_string(),
                ],
            });
        }
    }

    Ok(records)
}

fn load_js_reference_records(layout: &GraphInputLayout) -> Result<Vec<JsReferenceRecord>> {
    let Some(js_dir) = &layout.js else {
        return Ok(Vec::new());
    };

    let mut records = Vec::new();
    for entry in WalkDir::new(js_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let raw = fs::read_to_string(entry.path()).with_context(|| {
            format!(
                "failed to read JS reference file {}",
                entry.path().display()
            )
        })?;
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                if let Some(record) = parse_js_reference_value(&value) {
                    records.push(record);
                }
                continue;
            }
        }
    }

    Ok(records)
}

fn load_app_map(layout: &GraphInputLayout) -> Result<ImportedMapDocument> {
    let path = layout.maps.join("app-map.json");
    if !path.exists() {
        return Ok(ImportedMapDocument::default());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read app-map input at {}", path.display()))?;
    let document = serde_json::from_str::<ImportedMapDocument>(&raw)
        .with_context(|| format!("failed to parse app-map input at {}", path.display()))?;
    Ok(document)
}

fn build_graph(bundle: &ArtifactBundle) -> GraphBuildResult {
    let mut builder = GraphBuilder::default();
    let mut correlation_view = CorrelationView::default();
    let now = Utc::now();

    import_app_map(bundle, &mut builder, now);
    ingest_httpx(bundle, &mut builder, &mut correlation_view, now);
    ingest_dns(bundle, &mut builder, &mut correlation_view, now);
    ingest_technology_fingerprints(bundle, &mut builder, &mut correlation_view, now);
    ingest_screenshots(bundle, &mut builder, &mut correlation_view, now);
    ingest_katana(bundle, &mut builder, now);
    ingest_js_references(bundle, &mut builder, now);

    let correlated = correlation::correlate(&correlation_view);
    for edge in correlated.edges.clone() {
        builder.add_edge(edge);
    }

    let cluster_edges = correlation::cluster_membership_edges(&correlated.clusters);
    for cluster in &correlated.clusters {
        let metadata = BTreeMap::from([
            ("risk_score".to_string(), json!(cluster.risk_score)),
            (
                "shared_indicators".to_string(),
                json!(cluster.shared_indicators.clone()),
            ),
        ]);
        builder.upsert_node(
            GraphNode {
                id: cluster.cluster_id.clone(),
                node_type: "cluster".to_string(),
                value: cluster.cluster_type.clone(),
                tags: vec![cluster.cluster_type.clone()],
                metadata,
                source_tools: vec!["correlation-engine".to_string()],
                timestamps: vec![now],
            },
            "correlation-engine",
        );
    }
    for edge in cluster_edges {
        builder.add_edge(edge);
    }

    let nodes = builder.nodes_vec();
    let edges = builder.edges_vec();
    let summary = build_summary(
        &nodes,
        &edges,
        &correlated.clusters,
        &correlated.anomalies,
        &correlation_view,
    );

    GraphBuildResult {
        nodes,
        edges,
        clusters: correlated.clusters,
        anomalies: correlated.anomalies,
        summary,
    }
}

fn import_app_map(bundle: &ArtifactBundle, builder: &mut GraphBuilder, timestamp: DateTime<Utc>) {
    for node in &bundle.app_map_nodes {
        let metadata = BTreeMap::from([
            ("references".to_string(), json!(node.references.clone())),
            ("notes".to_string(), json!(node.notes.clone())),
        ]);
        builder.upsert_node(
            GraphNode {
                id: format!("map:{}", slugify(&node.id)),
                node_type: node.kind.clone(),
                value: node.label.clone(),
                tags: vec!["app-map-import".to_string()],
                metadata,
                source_tools: vec!["app-map".to_string()],
                timestamps: vec![timestamp],
            },
            "app-map",
        );
    }

    for edge in &bundle.app_map_edges {
        builder.add_edge(GraphEdge {
            source: format!("map:{}", slugify(&edge.from)),
            target: format!("map:{}", slugify(&edge.to)),
            relationship: relationship_from_label(&edge.relationship),
            confidence: 0.5,
            evidence: vec![CorrelationEvidence {
                source_tool: "app-map".to_string(),
                description: format!(
                    "Imported placeholder app-map relationship {}",
                    edge.relationship
                ),
                weight: 0.5,
            }],
            timestamps: vec![timestamp],
        });
    }
}

fn ingest_httpx(
    bundle: &ArtifactBundle,
    builder: &mut GraphBuilder,
    correlation_view: &mut CorrelationView,
    timestamp: DateTime<Utc>,
) {
    for record in &bundle.httpx {
        let mut metadata = BTreeMap::new();
        if let Some(url) = &record.url {
            metadata.insert("url".to_string(), json!(url));
        }
        if let Some(title) = &record.title {
            metadata.insert("title".to_string(), json!(title));
            correlation_view
                .host_to_titles
                .insert(record.host.clone(), title.clone());
        }
        if let Some(status_code) = record.status_code {
            metadata.insert("status_code".to_string(), json!(status_code));
        }
        if let Some(favicon_hash) = &record.favicon_hash {
            metadata.insert("favicon_hash".to_string(), json!(favicon_hash));
        }
        builder.upsert_node(
            GraphNode {
                id: host_node_id(&record.host),
                node_type: "host".to_string(),
                value: record.host.clone(),
                tags: vec!["live-host".to_string()],
                metadata,
                source_tools: vec!["httpx".to_string()],
                timestamps: vec![record.timestamp.unwrap_or(timestamp)],
            },
            "httpx",
        );

        if let Some(port) = record.port {
            correlation_view
                .host_to_ports
                .entry(record.host.clone())
                .or_default()
                .insert(port);
        }

        if let Some(url) = &record.url {
            builder.upsert_node(
                GraphNode {
                    id: url_node_id(url),
                    node_type: "url".to_string(),
                    value: url.clone(),
                    tags: Vec::new(),
                    metadata: BTreeMap::from([("host".to_string(), json!(record.host.clone()))]),
                    source_tools: vec!["httpx".to_string()],
                    timestamps: vec![record.timestamp.unwrap_or(timestamp)],
                },
                "httpx",
            );
            builder.add_edge(GraphEdge {
                source: host_node_id(&record.host),
                target: url_node_id(url),
                relationship: RelationshipType::Hosts,
                confidence: 0.95,
                evidence: vec![CorrelationEvidence {
                    source_tool: "httpx".to_string(),
                    description: format!("Observed live URL {url} for host {}", record.host),
                    weight: 0.95,
                }],
                timestamps: vec![record.timestamp.unwrap_or(timestamp)],
            });
        }

        for ip in &record.ip_addresses {
            correlation_view
                .host_to_ips
                .entry(record.host.clone())
                .or_default()
                .insert(ip.clone());
            builder.upsert_node(
                GraphNode {
                    id: ip_node_id(ip),
                    node_type: "ip".to_string(),
                    value: ip.clone(),
                    tags: vec!["network".to_string()],
                    metadata: BTreeMap::new(),
                    source_tools: vec!["httpx".to_string()],
                    timestamps: vec![record.timestamp.unwrap_or(timestamp)],
                },
                "httpx",
            );
            builder.add_edge(GraphEdge {
                source: host_node_id(&record.host),
                target: ip_node_id(ip),
                relationship: RelationshipType::ResolvesTo,
                confidence: 0.85,
                evidence: vec![CorrelationEvidence {
                    source_tool: "httpx".to_string(),
                    description: format!("Observed IP association {ip} for host {}", record.host),
                    weight: 0.85,
                }],
                timestamps: vec![record.timestamp.unwrap_or(timestamp)],
            });
        }

        for technology in &record.technologies {
            correlation_view
                .host_to_technologies
                .entry(record.host.clone())
                .or_default()
                .insert(technology.clone());
            builder.upsert_node(
                GraphNode {
                    id: technology_node_id(technology),
                    node_type: "technology".to_string(),
                    value: technology.clone(),
                    tags: vec!["fingerprint".to_string()],
                    metadata: BTreeMap::new(),
                    source_tools: vec!["httpx".to_string()],
                    timestamps: vec![record.timestamp.unwrap_or(timestamp)],
                },
                "httpx",
            );
            builder.add_edge(GraphEdge {
                source: host_node_id(&record.host),
                target: technology_node_id(technology),
                relationship: RelationshipType::UsesTechnology,
                confidence: 0.8,
                evidence: vec![CorrelationEvidence {
                    source_tool: "httpx".to_string(),
                    description: format!(
                        "httpx metadata suggests host {} uses {technology}",
                        record.host
                    ),
                    weight: 0.8,
                }],
                timestamps: vec![record.timestamp.unwrap_or(timestamp)],
            });
        }

        if let Some(redirect_target) = &record.redirect_target {
            if let Some(target_host) = host_from_target(redirect_target) {
                correlation_view
                    .redirects
                    .push((record.host.clone(), target_host.clone()));
                builder.upsert_node(
                    GraphNode {
                        id: host_node_id(&target_host),
                        node_type: "host".to_string(),
                        value: target_host.clone(),
                        tags: vec!["redirect-target".to_string()],
                        metadata: BTreeMap::new(),
                        source_tools: vec!["httpx".to_string()],
                        timestamps: vec![record.timestamp.unwrap_or(timestamp)],
                    },
                    "httpx",
                );
                builder.add_edge(GraphEdge {
                    source: host_node_id(&record.host),
                    target: host_node_id(&target_host),
                    relationship: RelationshipType::RedirectsTo,
                    confidence: 0.9,
                    evidence: vec![CorrelationEvidence {
                        source_tool: "httpx".to_string(),
                        description: format!(
                            "httpx observed redirect from {} to {redirect_target}",
                            record.host
                        ),
                        weight: 0.9,
                    }],
                    timestamps: vec![record.timestamp.unwrap_or(timestamp)],
                });
            }
        }
    }
}

fn ingest_dns(
    bundle: &ArtifactBundle,
    builder: &mut GraphBuilder,
    correlation_view: &mut CorrelationView,
    timestamp: DateTime<Utc>,
) {
    for record in &bundle.dns {
        builder.upsert_node(
            GraphNode {
                id: host_node_id(&record.name),
                node_type: "host".to_string(),
                value: record.name.clone(),
                tags: vec!["dns-observed".to_string()],
                metadata: BTreeMap::from([
                    ("record_type".to_string(), json!(record.record_type.clone())),
                    ("resolver_notes".to_string(), json!(record.notes.clone())),
                ]),
                source_tools: vec![record.source_tool.clone()],
                timestamps: vec![timestamp],
            },
            &record.source_tool,
        );

        for value in &record.values {
            let relationship = RelationshipType::ResolvesTo;
            if is_ip_address(value) {
                correlation_view
                    .host_to_ips
                    .entry(record.name.clone())
                    .or_default()
                    .insert(value.clone());
                builder.upsert_node(
                    GraphNode {
                        id: ip_node_id(value),
                        node_type: "ip".to_string(),
                        value: value.clone(),
                        tags: vec!["dns".to_string()],
                        metadata: BTreeMap::new(),
                        source_tools: vec![record.source_tool.clone()],
                        timestamps: vec![timestamp],
                    },
                    &record.source_tool,
                );
                builder.add_edge(GraphEdge {
                    source: host_node_id(&record.name),
                    target: ip_node_id(value),
                    relationship,
                    confidence: 0.95,
                    evidence: vec![CorrelationEvidence {
                        source_tool: record.source_tool.clone(),
                        description: format!(
                            "DNS record {} for {} resolves to {value}",
                            record.record_type, record.name
                        ),
                        weight: 0.95,
                    }],
                    timestamps: vec![timestamp],
                });
            } else {
                builder.upsert_node(
                    GraphNode {
                        id: host_node_id(value),
                        node_type: "host".to_string(),
                        value: value.clone(),
                        tags: vec!["dns-target".to_string()],
                        metadata: BTreeMap::new(),
                        source_tools: vec![record.source_tool.clone()],
                        timestamps: vec![timestamp],
                    },
                    &record.source_tool,
                );
                builder.add_edge(GraphEdge {
                    source: host_node_id(&record.name),
                    target: host_node_id(value),
                    relationship,
                    confidence: 0.8,
                    evidence: vec![CorrelationEvidence {
                        source_tool: record.source_tool.clone(),
                        description: format!(
                            "DNS record {} for {} points to {value}",
                            record.record_type, record.name
                        ),
                        weight: 0.8,
                    }],
                    timestamps: vec![timestamp],
                });
            }
        }
    }
}

fn ingest_technology_fingerprints(
    bundle: &ArtifactBundle,
    builder: &mut GraphBuilder,
    correlation_view: &mut CorrelationView,
    timestamp: DateTime<Utc>,
) {
    for fingerprint in &bundle.tech {
        let Some(host) = host_from_target(&fingerprint.target) else {
            continue;
        };
        builder.upsert_node(
            GraphNode {
                id: host_node_id(&host),
                node_type: "host".to_string(),
                value: host.clone(),
                tags: vec!["fingerprinted".to_string()],
                metadata: BTreeMap::new(),
                source_tools: vec![fingerprint.source_tool.clone()],
                timestamps: vec![timestamp],
            },
            &fingerprint.source_tool,
        );

        for technology in &fingerprint.technologies {
            correlation_view
                .host_to_technologies
                .entry(host.clone())
                .or_default()
                .insert(technology.clone());
            builder.upsert_node(
                GraphNode {
                    id: technology_node_id(technology),
                    node_type: "technology".to_string(),
                    value: technology.clone(),
                    tags: fingerprint.categories.clone(),
                    metadata: BTreeMap::new(),
                    source_tools: vec![fingerprint.source_tool.clone()],
                    timestamps: vec![timestamp],
                },
                &fingerprint.source_tool,
            );
            builder.add_edge(GraphEdge {
                source: host_node_id(&host),
                target: technology_node_id(technology),
                relationship: RelationshipType::UsesTechnology,
                confidence: fingerprint.confidence.unwrap_or(0.9),
                evidence: vec![CorrelationEvidence {
                    source_tool: fingerprint.source_tool.clone(),
                    description: format!(
                        "Technology fingerprint associates {host} with {technology}"
                    ),
                    weight: fingerprint.confidence.unwrap_or(0.9),
                }],
                timestamps: vec![timestamp],
            });
        }
    }
}

fn ingest_screenshots(
    bundle: &ArtifactBundle,
    builder: &mut GraphBuilder,
    correlation_view: &mut CorrelationView,
    timestamp: DateTime<Utc>,
) {
    for screenshot in &bundle.screenshots {
        let Some(host) = host_from_target(&screenshot.target) else {
            continue;
        };
        if let Some(title) = &screenshot.title {
            correlation_view
                .host_to_titles
                .entry(host.clone())
                .or_insert_with(|| title.clone());
        }

        let mut metadata = BTreeMap::new();
        if let Some(image_path) = &screenshot.image_path {
            metadata.insert(
                "image_path".to_string(),
                json!(image_path.display().to_string()),
            );
        }
        if let Some(title) = &screenshot.title {
            metadata.insert("title".to_string(), json!(title));
        }
        metadata.insert(
            "placeholder_notes".to_string(),
            json!([
                "favicon clustering not implemented yet",
                "visual similarity not implemented yet"
            ]),
        );

        builder.upsert_node(
            GraphNode {
                id: screenshot_node_id(&screenshot.target),
                node_type: "screenshot".to_string(),
                value: screenshot.target.clone(),
                tags: vec!["visual".to_string()],
                metadata,
                source_tools: vec![screenshot.source_tool.clone()],
                timestamps: vec![screenshot.captured_at.unwrap_or(timestamp)],
            },
            &screenshot.source_tool,
        );
        builder.upsert_node(
            GraphNode {
                id: host_node_id(&host),
                node_type: "host".to_string(),
                value: host.clone(),
                tags: vec!["screenshotted".to_string()],
                metadata: BTreeMap::new(),
                source_tools: vec![screenshot.source_tool.clone()],
                timestamps: vec![screenshot.captured_at.unwrap_or(timestamp)],
            },
            &screenshot.source_tool,
        );
        builder.add_edge(GraphEdge {
            source: host_node_id(&host),
            target: screenshot_node_id(&screenshot.target),
            relationship: RelationshipType::References,
            confidence: 0.75,
            evidence: vec![CorrelationEvidence {
                source_tool: screenshot.source_tool.clone(),
                description: format!("Screenshot metadata is associated with host {host}"),
                weight: 0.75,
            }],
            timestamps: vec![screenshot.captured_at.unwrap_or(timestamp)],
        });
    }
}

fn ingest_katana(bundle: &ArtifactBundle, builder: &mut GraphBuilder, timestamp: DateTime<Utc>) {
    for record in &bundle.katana {
        let Some(parsed) = Url::parse(&record.url).ok() else {
            continue;
        };
        let Some(host) = parsed.host_str().map(ToOwned::to_owned) else {
            continue;
        };
        let ts = record.timestamp.unwrap_or(timestamp);

        builder.upsert_node(
            GraphNode {
                id: host_node_id(&host),
                node_type: "host".to_string(),
                value: host.clone(),
                tags: vec!["crawled".to_string()],
                metadata: BTreeMap::new(),
                source_tools: vec![record.source.clone()],
                timestamps: vec![ts],
            },
            &record.source,
        );
        let mut url_metadata = BTreeMap::new();
        if let Some(content_type) = &record.content_type {
            url_metadata.insert("content_type".to_string(), json!(content_type));
        }
        builder.upsert_node(
            GraphNode {
                id: url_node_id(&record.url),
                node_type: "url".to_string(),
                value: record.url.clone(),
                tags: inferred_url_tags(&record.url, record.content_type.as_deref()),
                metadata: url_metadata,
                source_tools: vec![record.source.clone()],
                timestamps: vec![ts],
            },
            &record.source,
        );
        builder.add_edge(GraphEdge {
            source: host_node_id(&host),
            target: url_node_id(&record.url),
            relationship: RelationshipType::Hosts,
            confidence: 0.9,
            evidence: vec![CorrelationEvidence {
                source_tool: record.source.clone(),
                description: format!("Crawler observed URL {}", record.url),
                weight: 0.9,
            }],
            timestamps: vec![ts],
        });

        for parameter in parsed.query_pairs().map(|(key, _)| key.to_string()) {
            builder.upsert_node(
                GraphNode {
                    id: parameter_node_id(&parameter),
                    node_type: "parameter".to_string(),
                    value: parameter.clone(),
                    tags: Vec::new(),
                    metadata: BTreeMap::new(),
                    source_tools: vec![record.source.clone()],
                    timestamps: vec![ts],
                },
                &record.source,
            );
            builder.add_edge(GraphEdge {
                source: url_node_id(&record.url),
                target: parameter_node_id(&parameter),
                relationship: RelationshipType::ContainsParameter,
                confidence: 0.9,
                evidence: vec![CorrelationEvidence {
                    source_tool: record.source.clone(),
                    description: format!("URL {} contains parameter {parameter}", record.url),
                    weight: 0.9,
                }],
                timestamps: vec![ts],
            });
        }

        if parsed.path().to_ascii_lowercase().ends_with(".js") {
            builder.upsert_node(
                GraphNode {
                    id: js_node_id(&record.url),
                    node_type: "js-file".to_string(),
                    value: record.url.clone(),
                    tags: vec!["javascript".to_string()],
                    metadata: BTreeMap::new(),
                    source_tools: vec![record.source.clone()],
                    timestamps: vec![ts],
                },
                &record.source,
            );
            builder.add_edge(GraphEdge {
                source: host_node_id(&host),
                target: js_node_id(&record.url),
                relationship: RelationshipType::Hosts,
                confidence: 0.8,
                evidence: vec![CorrelationEvidence {
                    source_tool: record.source.clone(),
                    description: format!("Host {host} exposes JavaScript file {}", record.url),
                    weight: 0.8,
                }],
                timestamps: vec![ts],
            });
        }
    }
}

fn ingest_js_references(
    bundle: &ArtifactBundle,
    builder: &mut GraphBuilder,
    timestamp: DateTime<Utc>,
) {
    for record in &bundle.js_references {
        let ts = record.timestamp.unwrap_or(timestamp);
        builder.upsert_node(
            GraphNode {
                id: js_node_id(&record.script_url),
                node_type: "js-file".to_string(),
                value: record.script_url.clone(),
                tags: vec!["javascript".to_string()],
                metadata: BTreeMap::new(),
                source_tools: vec!["js-parser".to_string()],
                timestamps: vec![ts],
            },
            "js-parser",
        );
        builder.upsert_node(
            GraphNode {
                id: url_node_id(&record.endpoint_url),
                node_type: "url".to_string(),
                value: record.endpoint_url.clone(),
                tags: inferred_url_tags(&record.endpoint_url, None),
                metadata: BTreeMap::new(),
                source_tools: vec!["js-parser".to_string()],
                timestamps: vec![ts],
            },
            "js-parser",
        );
        builder.add_edge(GraphEdge {
            source: js_node_id(&record.script_url),
            target: url_node_id(&record.endpoint_url),
            relationship: RelationshipType::References,
            confidence: 0.8,
            evidence: vec![CorrelationEvidence {
                source_tool: "js-parser".to_string(),
                description: format!(
                    "JavaScript file {} references endpoint {}",
                    record.script_url, record.endpoint_url
                ),
                weight: 0.8,
            }],
            timestamps: vec![ts],
        });

        if let Some(source_url) = &record.source_url {
            builder.upsert_node(
                GraphNode {
                    id: url_node_id(source_url),
                    node_type: "url".to_string(),
                    value: source_url.clone(),
                    tags: Vec::new(),
                    metadata: BTreeMap::new(),
                    source_tools: vec!["js-parser".to_string()],
                    timestamps: vec![ts],
                },
                "js-parser",
            );
            builder.add_edge(GraphEdge {
                source: url_node_id(source_url),
                target: js_node_id(&record.script_url),
                relationship: RelationshipType::LoadsScript,
                confidence: 0.8,
                evidence: vec![CorrelationEvidence {
                    source_tool: "js-parser".to_string(),
                    description: format!(
                        "Page {source_url} appears to load script {}",
                        record.script_url
                    ),
                    weight: 0.8,
                }],
                timestamps: vec![ts],
            });
        }

        for parameter in &record.parameters {
            builder.upsert_node(
                GraphNode {
                    id: parameter_node_id(parameter),
                    node_type: "parameter".to_string(),
                    value: parameter.clone(),
                    tags: Vec::new(),
                    metadata: BTreeMap::new(),
                    source_tools: vec!["js-parser".to_string()],
                    timestamps: vec![ts],
                },
                "js-parser",
            );
            builder.add_edge(GraphEdge {
                source: url_node_id(&record.endpoint_url),
                target: parameter_node_id(parameter),
                relationship: RelationshipType::ContainsParameter,
                confidence: 0.75,
                evidence: vec![CorrelationEvidence {
                    source_tool: "js-parser".to_string(),
                    description: format!(
                        "Referenced endpoint {} uses parameter {parameter}",
                        record.endpoint_url
                    ),
                    weight: 0.75,
                }],
                timestamps: vec![ts],
            });
        }
    }
}

fn build_summary(
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    clusters: &[AssetCluster],
    anomalies: &[GraphAnomaly],
    correlation_view: &CorrelationView,
) -> GraphSummary {
    let generated_at = Utc::now();

    let mut technology_counts: BTreeMap<String, usize> = BTreeMap::new();
    for technologies in correlation_view.host_to_technologies.values() {
        for technology in technologies {
            *technology_counts.entry(technology.clone()).or_default() += 1;
        }
    }

    let mut top_technologies = technology_counts.into_iter().collect::<Vec<_>>();
    top_technologies.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    let mut largest_clusters = clusters
        .iter()
        .map(|cluster| {
            (
                cluster.cluster_id.clone(),
                cluster.cluster_type.clone(),
                cluster.related_nodes.len(),
            )
        })
        .collect::<Vec<_>>();
    largest_clusters.sort_by(|left, right| right.2.cmp(&left.2).then_with(|| left.0.cmp(&right.0)));

    let shared_infrastructure = clusters
        .iter()
        .filter(|cluster| cluster.cluster_type == "shared-infrastructure")
        .map(|cluster| {
            format!(
                "{} [{}]",
                cluster.cluster_id,
                cluster.shared_indicators.join(", ")
            )
        })
        .collect();

    let suspicious_naming = anomalies
        .iter()
        .filter(|anomaly| anomaly.kind == "suspicious-hostname")
        .map(|anomaly| anomaly.description.clone())
        .collect();

    let likely_staging_systems =
        extract_hosts_by_keywords(nodes, &["staging", "dev", "test", "preview"]);
    let likely_internal_systems = extract_hosts_by_keywords(nodes, &["internal", "intra", "corp"]);
    let redirect_chain_count = edges
        .iter()
        .filter(|edge| edge.relationship == RelationshipType::RedirectsTo)
        .count();

    GraphSummary {
        generated_at,
        node_count: nodes.len(),
        edge_count: edges.len(),
        cluster_count: clusters.len(),
        anomaly_count: anomalies.len(),
        top_technologies: top_technologies
            .into_iter()
            .take(5)
            .map(|(technology, count)| format!("{technology} ({count})"))
            .collect(),
        largest_clusters: largest_clusters
            .into_iter()
            .take(5)
            .map(|(cluster_id, cluster_type, size)| {
                format!("{cluster_id} [{cluster_type}] ({size} nodes)")
            })
            .collect(),
        shared_infrastructure,
        suspicious_naming,
        likely_staging_systems,
        likely_internal_systems,
        redirect_chain_count,
    }
}

fn render_graph_preview(
    layout: &GraphInputLayout,
    output_root: &Path,
    bundle: &ArtifactBundle,
    execute: bool,
) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot Graph Preview\n\n");
    output.push_str(&format!(
        "- Mode: {}\n",
        if execute {
            "execute requested"
        } else {
            "dry-run (default)"
        }
    ));
    output.push_str(&format!("- Input root: `{}`\n", layout.root.display()));
    output.push_str(&format!("- Output maps: `{}`\n", output_root.display()));
    output.push_str("- Safety: graph execution is local-only and does not contact targets.\n");
    output.push_str(
        "- Placeholder note: screenshot similarity and favicon clustering remain future work.\n\n",
    );
    output.push_str("## Available Inputs\n\n");
    output.push_str(&format!("- httpx records: {}\n", bundle.httpx.len()));
    output.push_str(&format!("- katana records: {}\n", bundle.katana.len()));
    output.push_str(&format!("- dns records: {}\n", bundle.dns.len()));
    output.push_str(&format!("- tech fingerprints: {}\n", bundle.tech.len()));
    output.push_str(&format!(
        "- screenshot records: {}\n",
        bundle.screenshots.len()
    ));
    output.push_str(&format!(
        "- js reference records: {}\n",
        bundle.js_references.len()
    ));
    output.push_str(&format!(
        "- imported app-map nodes: {}\n",
        bundle.app_map_nodes.len()
    ));
    output.push_str(&format!(
        "- imported app-map edges: {}\n",
        bundle.app_map_edges.len()
    ));
    output
}

fn render_graph_markdown(document: &GraphDocument, anomalies: &[GraphAnomaly]) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot Graph Summary\n\n");
    output.push_str(&format!(
        "- Nodes: {}\n- Edges: {}\n- Clusters: {}\n- Anomalies: {}\n\n",
        document.summary.node_count,
        document.summary.edge_count,
        document.summary.cluster_count,
        document.summary.anomaly_count
    ));

    output.push_str("## Top Technologies\n\n");
    write_markdown_list(&mut output, &document.summary.top_technologies);

    output.push_str("\n## Largest Clusters\n\n");
    write_markdown_list(&mut output, &document.summary.largest_clusters);

    output.push_str("\n## Shared Infrastructure\n\n");
    write_markdown_list(&mut output, &document.summary.shared_infrastructure);

    output.push_str("\n## Suspicious Naming\n\n");
    write_markdown_list(&mut output, &document.summary.suspicious_naming);

    output.push_str("\n## Likely Staging Systems\n\n");
    write_markdown_list(&mut output, &document.summary.likely_staging_systems);

    output.push_str("\n## Likely Internal Systems\n\n");
    write_markdown_list(&mut output, &document.summary.likely_internal_systems);

    output.push_str("\n## Redirect Chains\n\n");
    if document.summary.redirect_chain_count == 0 {
        output.push_str("No redirect relationships were observed.\n");
    } else {
        output.push_str(&format!(
            "Observed {} redirect relationships.\n",
            document.summary.redirect_chain_count
        ));
    }

    output.push_str("\n## Anomaly Candidates\n\n");
    if anomalies.is_empty() {
        output.push_str("No anomaly candidates were generated.\n");
    } else {
        for anomaly in anomalies {
            output.push_str(&format!(
                "- `{}` [{}] {}\n",
                anomaly.kind, anomaly.severity, anomaly.description
            ));
        }
    }

    output
}

fn render_clusters_markdown(clusters: &[AssetCluster]) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot Cluster Summary\n\n");

    if clusters.is_empty() {
        output.push_str("No clusters were generated.\n");
        return output;
    }

    for cluster in clusters {
        output.push_str(&format!(
            "## {} [{}]\n\n",
            cluster.cluster_id, cluster.cluster_type
        ));
        output.push_str(&format!(
            "- Risk score: {}\n- Related nodes: {}\n- Shared indicators: {}\n\n",
            cluster.risk_score,
            cluster.related_nodes.len(),
            if cluster.shared_indicators.is_empty() {
                "none".to_string()
            } else {
                cluster.shared_indicators.join(", ")
            }
        ));
    }

    output
}

fn discovered_input_labels(bundle: &ArtifactBundle) -> Vec<String> {
    vec![
        format!("httpx: {}", bundle.httpx.len()),
        format!("katana: {}", bundle.katana.len()),
        format!("dns: {}", bundle.dns.len()),
        format!("tech: {}", bundle.tech.len()),
        format!("screenshots: {}", bundle.screenshots.len()),
        format!("js references: {}", bundle.js_references.len()),
        format!("app-map nodes: {}", bundle.app_map_nodes.len()),
        format!("app-map edges: {}", bundle.app_map_edges.len()),
    ]
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

fn parse_httpx_file(path: &Path) -> Result<Vec<HttpxRecord>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read httpx artifact {}", path.display()))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    if matches!(extension.as_deref(), Some("txt")) {
        let mut records = Vec::new();
        for line in trimmed.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let host = host_from_target(line).unwrap_or_else(|| line.to_string());
            records.push(HttpxRecord {
                host,
                url: if line.starts_with("http://") || line.starts_with("https://") {
                    Some(line.to_string())
                } else {
                    Some(format!("https://{line}"))
                },
                ..Default::default()
            });
        }
        return Ok(records);
    }

    if trimmed.starts_with('[') {
        let values = serde_json::from_str::<Vec<Value>>(trimmed)
            .with_context(|| format!("failed to parse JSON array from {}", path.display()))?;
        return Ok(values.iter().filter_map(parse_httpx_value).collect());
    }

    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            if let Some(record) = parse_httpx_value(&value) {
                return Ok(vec![record]);
            }

            let mut records = Vec::new();
            for (index, line) in trimmed.lines().enumerate() {
                let value = serde_json::from_str::<Value>(line).with_context(|| {
                    format!(
                        "failed to parse httpx JSONL line {} in {}",
                        index + 1,
                        path.display()
                    )
                })?;
                if let Some(record) = parse_httpx_value(&value) {
                    records.push(record);
                }
            }
            return Ok(records);
        }
    }

    Ok(Vec::new())
}

fn parse_httpx_value(value: &Value) -> Option<HttpxRecord> {
    let url = optional_json_string(value, &["url", "final_url", "input"]);
    let host = json_string(value, &["host", "input"]);
    let host = if host.is_empty() {
        url.as_deref().and_then(host_from_target)?
    } else if host.starts_with("http://") || host.starts_with("https://") {
        host_from_target(&host)?
    } else {
        host
    };

    let mut technologies = json_array_strings(value, &["tech"]);
    let webserver = json_string(value, &["webserver"]);
    if !webserver.is_empty() {
        technologies.push(webserver);
    }
    technologies.sort();
    technologies.dedup();

    let mut ip_addresses = json_array_strings(value, &["a", "ip_addresses"]);
    let single_ip = json_string(value, &["ip"]);
    if !single_ip.is_empty() {
        ip_addresses.push(single_ip);
    }
    ip_addresses.retain(|value| is_ip_address(value));
    ip_addresses.sort();
    ip_addresses.dedup();

    let port = json_u16(value, &["port"]).or_else(|| {
        url.as_ref()
            .and_then(|candidate| Url::parse(candidate).ok())
            .and_then(|parsed| parsed.port_or_known_default())
    });
    let redirect_target =
        optional_json_string(value, &["location", "redirect_location", "final_url"]).filter(
            |target| {
                url.as_ref()
                    .map(|current| current != target)
                    .unwrap_or(true)
            },
        );
    let timestamp = optional_json_datetime(value, &["timestamp"]);

    Some(HttpxRecord {
        host,
        url,
        title: optional_json_string(value, &["title"]),
        technologies,
        ip_addresses,
        port,
        redirect_target,
        favicon_hash: optional_json_string(value, &["favicon_hash"]),
        status_code: json_u16(value, &["status_code"]),
        timestamp,
    })
}

fn parse_katana_file(path: &Path) -> Result<Vec<KatanaRecord>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read katana artifact {}", path.display()))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    if trimmed.starts_with('{') {
        for (index, line) in trimmed.lines().enumerate() {
            let value = serde_json::from_str::<Value>(line).with_context(|| {
                format!(
                    "failed to parse katana JSONL line {} in {}",
                    index + 1,
                    path.display()
                )
            })?;
            if let Some(record) = parse_katana_value(&value) {
                records.push(record);
            }
        }
        return Ok(records);
    }

    if trimmed.starts_with('[') {
        let values = serde_json::from_str::<Vec<Value>>(trimmed)
            .with_context(|| format!("failed to parse katana array in {}", path.display()))?;
        for value in values {
            if let Some(record) = parse_katana_value(&value) {
                records.push(record);
            }
        }
    }

    Ok(records)
}

fn parse_katana_value(value: &Value) -> Option<KatanaRecord> {
    let url = json_string(value, &["url", "endpoint"]);
    if url.is_empty() {
        return None;
    }

    Some(KatanaRecord {
        url,
        source: json_string(value, &["source"]).if_empty_then("katana".to_string()),
        content_type: optional_json_string(value, &["content_type"]),
        timestamp: optional_json_datetime(value, &["timestamp"]),
    })
}

fn parse_js_reference_value(value: &Value) -> Option<JsReferenceRecord> {
    let script_url = json_string(value, &["script", "script_url", "js_url"]);
    let endpoint_url = json_string(value, &["endpoint", "url", "endpoint_url"]);
    if script_url.is_empty() || endpoint_url.is_empty() {
        return None;
    }

    Some(JsReferenceRecord {
        script_url,
        source_url: optional_json_string(value, &["source_url", "page", "origin"]),
        endpoint_url,
        parameters: json_array_strings(value, &["params", "parameters"]),
        timestamp: optional_json_datetime(value, &["timestamp"]),
    })
}

impl GraphBuilder {
    fn upsert_node(&mut self, node: GraphNode, source_tool: &str) {
        let entry = self
            .nodes
            .entry(node.id.clone())
            .or_insert_with(|| node.clone());
        if entry.value.is_empty() {
            entry.value = node.value.clone();
        }
        if entry.node_type.is_empty() {
            entry.node_type = node.node_type.clone();
        }
        extend_unique(&mut entry.tags, node.tags);
        extend_unique(&mut entry.source_tools, node.source_tools);
        if !entry.source_tools.iter().any(|tool| tool == source_tool) {
            entry.source_tools.push(source_tool.to_string());
        }
        entry.metadata.extend(node.metadata);
        entry.timestamps.extend(node.timestamps);
        entry.timestamps.sort();
        entry.timestamps.dedup();
    }

    fn add_edge(&mut self, edge: GraphEdge) {
        let key = (
            edge.source.clone(),
            edge.target.clone(),
            edge.relationship.clone(),
        );

        let entry = self.edges.entry(key).or_insert_with(|| edge.clone());
        entry.confidence = entry.confidence.max(edge.confidence);
        entry.evidence.extend(edge.evidence);
        entry.timestamps.extend(edge.timestamps);
        entry.timestamps.sort();
        entry.timestamps.dedup();
    }

    fn nodes_vec(&self) -> Vec<GraphNode> {
        let mut nodes = self.nodes.values().cloned().collect::<Vec<_>>();
        nodes.sort_by(|left, right| left.id.cmp(&right.id));
        nodes
    }

    fn edges_vec(&self) -> Vec<GraphEdge> {
        let mut edges = self.edges.values().cloned().collect::<Vec<_>>();
        edges.sort_by(|left, right| {
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
        edges
    }
}

fn dedupe_httpx_records(records: &mut Vec<HttpxRecord>) {
    let mut seen = BTreeSet::new();
    records.retain(|record| {
        seen.insert((
            record.host.clone(),
            record.url.clone().unwrap_or_default(),
            record.title.clone().unwrap_or_default(),
        ))
    });
}

fn dedupe_katana_records(records: &mut Vec<KatanaRecord>) {
    let mut seen = BTreeSet::new();
    records.retain(|record| seen.insert(record.url.clone()));
}

fn extract_hosts_by_keywords(nodes: &[GraphNode], keywords: &[&str]) -> Vec<String> {
    nodes
        .iter()
        .filter(|node| node.node_type == "host")
        .filter(|node| {
            let value = node.value.to_ascii_lowercase();
            keywords.iter().any(|keyword| value.contains(keyword))
        })
        .map(|node| node.value.clone())
        .collect()
}

fn inferred_url_tags(url: &str, content_type: Option<&str>) -> Vec<String> {
    let mut tags = Vec::new();
    let url_lc = url.to_ascii_lowercase();
    if url_lc.contains("/api/") {
        tags.push("api".to_string());
    }
    if url_lc.contains("/admin") {
        tags.push("admin".to_string());
    }
    if url_lc.ends_with(".js") {
        tags.push("javascript".to_string());
    }
    if let Some(content_type) = content_type {
        if content_type.contains("json") {
            tags.push("json".to_string());
        }
    }
    tags.sort();
    tags.dedup();
    tags
}

fn relationship_from_label(value: &str) -> RelationshipType {
    match value {
        "resolves-to" => RelationshipType::ResolvesTo,
        "fingerprinted-as" => RelationshipType::UsesTechnology,
        "captured-as" => RelationshipType::References,
        _ => RelationshipType::References,
    }
}

fn host_from_target(target: &str) -> Option<String> {
    if target.starts_with("http://") || target.starts_with("https://") {
        return Url::parse(target)
            .ok()
            .and_then(|parsed| parsed.host_str().map(ToOwned::to_owned));
    }

    Some(target.trim_start_matches("*.").to_string())
}

fn json_string(value: &Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(found) = value.get(*key).and_then(|entry| entry.as_str()) {
            return found.to_string();
        }
    }
    String::new()
}

fn optional_json_string(value: &Value, keys: &[&str]) -> Option<String> {
    let found = json_string(value, keys);
    if found.is_empty() {
        None
    } else {
        Some(found)
    }
}

fn json_array_strings(value: &Value, keys: &[&str]) -> Vec<String> {
    for key in keys {
        if let Some(entry) = value.get(*key) {
            if let Some(array) = entry.as_array() {
                return array
                    .iter()
                    .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                    .collect();
            }
            if let Some(string) = entry.as_str() {
                return vec![string.to_string()];
            }
        }
    }
    Vec::new()
}

fn json_object_keys(value: &Value, keys: &[&str]) -> Vec<String> {
    for key in keys {
        if let Some(object) = value.get(*key).and_then(|entry| entry.as_object()) {
            return object.keys().cloned().collect();
        }
    }
    Vec::new()
}

fn json_u16(value: &Value, keys: &[&str]) -> Option<u16> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(|entry| entry.as_u64().and_then(|number| u16::try_from(number).ok()))
}

fn optional_json_datetime(value: &Value, keys: &[&str]) -> Option<DateTime<Utc>> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(|entry| {
            entry
                .as_str()
                .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.with_timezone(&Utc))
        })
}

fn extend_unique(target: &mut Vec<String>, values: Vec<String>) {
    for value in values {
        if !target.iter().any(|existing| existing == &value) {
            target.push(value);
        }
    }
    target.sort();
}

fn is_ip_address(value: &str) -> bool {
    value.parse::<std::net::IpAddr>().is_ok()
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

fn host_node_id(host: &str) -> String {
    format!("host:{}", slugify(host))
}

fn ip_node_id(ip: &str) -> String {
    format!("ip:{}", slugify(ip))
}

fn url_node_id(url: &str) -> String {
    format!("url:{}", slugify(url))
}

fn technology_node_id(technology: &str) -> String {
    format!("technology:{}", slugify(technology))
}

fn screenshot_node_id(target: &str) -> String {
    format!("screenshot:{}", slugify(target))
}

fn js_node_id(target: &str) -> String {
    format!("js:{}", slugify(target))
}

fn parameter_node_id(parameter: &str) -> String {
    format!("param:{}", slugify(parameter))
}

trait IfEmptyThen {
    fn if_empty_then(self, fallback: String) -> String;
}

impl IfEmptyThen for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.is_empty() {
            fallback
        } else {
            self
        }
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

    use super::run_graph_engine;
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
                "reconpilot-graph-{label}-{}-{unique}",
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
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn graph_generation_creates_artifacts() -> Result<()> {
        let workspace = TestWorkspace::new("graph-generation")?;
        let output = ensure_output_structure(&workspace.output_root())?;
        workspace.write_file(
            "output/raw/httpx/httpx.json",
            r#"[{"input":"app.example.com","host":"app.example.com","url":"https://app.example.com","title":"Example App","tech":["Next.js"],"ip":"203.0.113.10"}]"#,
        )?;
        workspace.write_file(
            "output/raw/katana/katana.jsonl",
            r#"{"url":"https://app.example.com/admin/login?returnUrl=/","source":"katana","content_type":"text/html"}"#,
        )?;
        let outcome = run_graph_engine(&output.root, &output.maps, true)?;
        assert!(outcome.executed);
        assert!(output.maps.join("graph.json").exists());
        assert!(output.maps.join("graph.md").exists());
        assert!(output.maps.join("clusters.json").exists());
        assert!(output.maps.join("anomalies.json").exists());
        Ok(())
    }

    #[test]
    fn edge_creation_captures_relationships() -> Result<()> {
        let workspace = TestWorkspace::new("edge-creation")?;
        let output = ensure_output_structure(&workspace.output_root())?;
        workspace.write_file(
            "output/raw/httpx/httpx.json",
            r#"[{"input":"app.example.com","host":"app.example.com","url":"https://app.example.com","title":"Dashboard","tech":["Grafana"],"ip":"203.0.113.10","location":"https://login.example.com"}]"#,
        )?;
        workspace.write_file(
            "output/dns/dnsx.jsonl",
            "{\"host\":\"app.example.com\",\"type\":\"A\",\"a\":[\"203.0.113.10\"]}\n",
        )?;
        run_graph_engine(&output.root, &output.maps, true)?;

        let graph = fs::read_to_string(output.maps.join("graph.json"))?;
        assert!(graph.contains("\"uses_technology\""));
        assert!(graph.contains("\"redirects_to\""));
        assert!(graph.contains("\"resolves_to\""));
        Ok(())
    }

    #[test]
    fn cluster_generation_builds_clusters() -> Result<()> {
        let workspace = TestWorkspace::new("clusters")?;
        let output = ensure_output_structure(&workspace.output_root())?;
        workspace.write_file(
            "output/raw/httpx/httpx.json",
            r#"[{"host":"admin.example.com","url":"https://admin.example.com","title":"Shared Portal","ip":"203.0.113.10"},{"host":"portal.example.com","url":"https://portal.example.com","title":"Shared Portal","ip":"203.0.113.10"}]"#,
        )?;
        run_graph_engine(&output.root, &output.maps, true)?;

        let clusters = fs::read_to_string(output.maps.join("clusters.json"))?;
        assert!(clusters.contains("shared-infrastructure"));
        assert!(clusters.contains("shared-title"));
        Ok(())
    }

    #[test]
    fn anomaly_detection_flags_admin_like_assets() -> Result<()> {
        let workspace = TestWorkspace::new("anomalies")?;
        let output = ensure_output_structure(&workspace.output_root())?;
        workspace.write_file(
            "output/raw/httpx/httpx.json",
            r#"[{"host":"internal-admin.example.com","url":"https://internal-admin.example.com:8448","title":"Ops Dashboard","tech":["Jenkins"],"ip":"203.0.113.50","port":8448}]"#,
        )?;
        run_graph_engine(&output.root, &output.maps, true)?;

        let anomalies = fs::read_to_string(output.maps.join("anomalies.json"))?;
        assert!(anomalies.contains("suspicious-hostname"));
        assert!(anomalies.contains("exposed-dashboard"));
        assert!(anomalies.contains("unusual-port"));
        Ok(())
    }

    #[test]
    fn empty_graph_handling_writes_zero_summary() -> Result<()> {
        let workspace = TestWorkspace::new("empty")?;
        let output = ensure_output_structure(&workspace.output_root())?;
        run_graph_engine(&output.root, &output.maps, true)?;

        let summary = fs::read_to_string(output.maps.join("graph-summary.json"))?;
        let summary: Value = serde_json::from_str(&summary)?;
        assert_eq!(summary.get("node_count").and_then(Value::as_u64), Some(0));
        assert_eq!(summary.get("edge_count").and_then(Value::as_u64), Some(0));
        assert_eq!(
            summary.get("cluster_count").and_then(Value::as_u64),
            Some(0)
        );
        Ok(())
    }

    #[test]
    fn dry_run_graph_generates_plan_and_preview_only() -> Result<()> {
        let workspace = TestWorkspace::new("dry-run")?;
        let output = ensure_output_structure(&workspace.output_root())?;
        workspace.write_file(
            "output/raw/httpx/httpx.json",
            r#"[{"host":"app.example.com","url":"https://app.example.com","title":"Example App"}]"#,
        )?;
        let outcome = run_graph_engine(&output.root, &output.maps, false)?;
        assert!(!outcome.executed);
        assert!(output.plans.join("graph-plan.json").exists());
        assert!(output.maps.join("graph-preview.md").exists());
        assert!(!output.maps.join("graph.json").exists());
        Ok(())
    }
}
