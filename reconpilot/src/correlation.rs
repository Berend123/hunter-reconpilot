use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::models::{
    AssetCluster, CorrelationEvidence, GraphEdge, RelationshipType, TechFingerprint,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphAnomaly {
    pub id: String,
    pub kind: String,
    pub severity: String,
    pub description: String,
    #[serde(default)]
    pub related_nodes: Vec<String>,
    #[serde(default)]
    pub indicators: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct CorrelationView {
    pub host_to_ips: BTreeMap<String, BTreeSet<String>>,
    pub host_to_titles: BTreeMap<String, String>,
    pub host_to_technologies: BTreeMap<String, BTreeSet<String>>,
    pub host_to_ports: BTreeMap<String, BTreeSet<u16>>,
    pub redirects: Vec<(String, String)>,
    pub title_sources: BTreeMap<String, String>,
    pub tech_fingerprints: Vec<TechFingerprint>,
}

#[derive(Debug, Clone, Default)]
pub struct CorrelationOutput {
    pub edges: Vec<GraphEdge>,
    pub clusters: Vec<AssetCluster>,
    pub anomalies: Vec<GraphAnomaly>,
}

pub fn correlate(view: &CorrelationView) -> CorrelationOutput {
    let mut output = CorrelationOutput::default();

    let (shared_ip_edges, shared_ip_clusters) = build_shared_ip_edges_and_clusters(view);
    output.edges.extend(shared_ip_edges);
    output.clusters.extend(shared_ip_clusters.clone());

    let (shared_title_edges, shared_title_clusters) = build_shared_title_edges_and_clusters(view);
    output.edges.extend(shared_title_edges);
    output.clusters.extend(shared_title_clusters);

    let technology_edges = build_technology_edges(view);
    output.edges.extend(technology_edges);

    let redirect_edges = build_redirect_edges(view);
    output.edges.extend(redirect_edges);

    let admin_clusters = build_admin_surface_clusters(view, &shared_ip_clusters);
    output.clusters.extend(admin_clusters);

    output.anomalies = detect_anomalies(view, &output.clusters);

    sort_edges(&mut output.edges);
    sort_clusters(&mut output.clusters);
    sort_anomalies(&mut output.anomalies);

    output
}

pub fn cluster_membership_edges(clusters: &[AssetCluster]) -> Vec<GraphEdge> {
    let mut edges = Vec::new();
    let timestamp = Utc::now();

    for cluster in clusters {
        for related_node in &cluster.related_nodes {
            edges.push(GraphEdge {
                source: related_node.clone(),
                target: cluster.cluster_id.clone(),
                relationship: RelationshipType::BelongsToCluster,
                confidence: 0.9,
                evidence: vec![CorrelationEvidence {
                    source_tool: "correlation-engine".to_string(),
                    description: format!(
                        "Node belongs to correlation cluster {}",
                        cluster.cluster_type
                    ),
                    weight: 0.9,
                }],
                timestamps: vec![timestamp],
            });
        }
    }

    sort_edges(&mut edges);
    edges
}

fn build_shared_ip_edges_and_clusters(
    view: &CorrelationView,
) -> (Vec<GraphEdge>, Vec<AssetCluster>) {
    let mut ip_to_hosts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (host, ips) in &view.host_to_ips {
        for ip in ips {
            ip_to_hosts
                .entry(ip.clone())
                .or_default()
                .push(host.clone());
        }
    }

    let mut edges = Vec::new();
    let mut clusters = Vec::new();
    let timestamp = Utc::now();

    for (ip, mut hosts) in ip_to_hosts {
        hosts.sort();
        hosts.dedup();
        if hosts.len() < 2 {
            continue;
        }

        for pair in host_pairs(&hosts) {
            edges.push(GraphEdge {
                source: host_node_id(&pair.0),
                target: host_node_id(&pair.1),
                relationship: RelationshipType::SharesIp,
                confidence: 0.95,
                evidence: vec![CorrelationEvidence {
                    source_tool: "dnsx".to_string(),
                    description: format!("Hosts resolve to the same IP address {ip}"),
                    weight: 0.95,
                }],
                timestamps: vec![timestamp],
            });
        }

        let admin_like = hosts
            .iter()
            .filter(|host| is_admin_like(host, None, None))
            .count() as i32;
        clusters.push(AssetCluster {
            cluster_id: format!("cluster:shared-ip:{}", slugify(&ip)),
            cluster_type: "shared-infrastructure".to_string(),
            related_nodes: hosts.iter().map(|host| host_node_id(host)).collect(),
            shared_indicators: vec![ip],
            risk_score: (25 + (admin_like * 10)).clamp(0, 100),
        });
    }

    (edges, clusters)
}

fn build_shared_title_edges_and_clusters(
    view: &CorrelationView,
) -> (Vec<GraphEdge>, Vec<AssetCluster>) {
    let mut title_to_hosts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (host, title) in &view.host_to_titles {
        let normalized = normalize_title(title);
        if normalized.is_empty() {
            continue;
        }
        title_to_hosts
            .entry(normalized)
            .or_default()
            .push(host.clone());
    }

    let mut edges = Vec::new();
    let mut clusters = Vec::new();
    let timestamp = Utc::now();

    for (title, mut hosts) in title_to_hosts {
        hosts.sort();
        hosts.dedup();
        if hosts.len() < 2 {
            continue;
        }

        for pair in host_pairs(&hosts) {
            edges.push(GraphEdge {
                source: host_node_id(&pair.0),
                target: host_node_id(&pair.1),
                relationship: RelationshipType::SharesTitle,
                confidence: 0.85,
                evidence: vec![CorrelationEvidence {
                    source_tool: "httpx".to_string(),
                    description: format!("Hosts share the same page title '{title}'"),
                    weight: 0.85,
                }],
                timestamps: vec![timestamp],
            });
        }

        clusters.push(AssetCluster {
            cluster_id: format!("cluster:shared-title:{}", slugify(&title)),
            cluster_type: "shared-title".to_string(),
            related_nodes: hosts.iter().map(|host| host_node_id(host)).collect(),
            shared_indicators: vec![title],
            risk_score: 20,
        });
    }

    (edges, clusters)
}

fn build_technology_edges(view: &CorrelationView) -> Vec<GraphEdge> {
    let mut edges = Vec::new();
    let timestamp = Utc::now();

    for (host, technologies) in &view.host_to_technologies {
        for technology in technologies {
            edges.push(GraphEdge {
                source: host_node_id(host),
                target: technology_node_id(technology),
                relationship: RelationshipType::UsesTechnology,
                confidence: 0.9,
                evidence: vec![CorrelationEvidence {
                    source_tool: "tech-correlation".to_string(),
                    description: format!("Host is associated with technology {technology}"),
                    weight: 0.9,
                }],
                timestamps: vec![timestamp],
            });
        }
    }

    edges
}

fn build_redirect_edges(view: &CorrelationView) -> Vec<GraphEdge> {
    let mut edges = Vec::new();
    let timestamp = Utc::now();

    for (source, target) in &view.redirects {
        edges.push(GraphEdge {
            source: host_node_id(source),
            target: host_node_id(target),
            relationship: RelationshipType::RedirectsTo,
            confidence: 0.9,
            evidence: vec![CorrelationEvidence {
                source_tool: "httpx".to_string(),
                description: format!("Observed redirect from {source} to {target}"),
                weight: 0.9,
            }],
            timestamps: vec![timestamp],
        });
    }

    edges
}

fn build_admin_surface_clusters(
    view: &CorrelationView,
    shared_ip_clusters: &[AssetCluster],
) -> Vec<AssetCluster> {
    let mut admin_hosts = BTreeSet::new();

    for host in view.host_to_titles.keys() {
        let title = view.host_to_titles.get(host).map(String::as_str);
        let technologies = view.host_to_technologies.get(host);
        if is_admin_like(host, title, technologies) {
            admin_hosts.insert(host.clone());
        }
    }

    for host in view.host_to_technologies.keys() {
        let title = view.host_to_titles.get(host).map(String::as_str);
        let technologies = view.host_to_technologies.get(host);
        if is_admin_like(host, title, technologies) {
            admin_hosts.insert(host.clone());
        }
    }

    if admin_hosts.is_empty() {
        return Vec::new();
    }

    let mut clusters = Vec::new();
    let host_nodes = admin_hosts
        .iter()
        .map(|host| host_node_id(host))
        .collect::<Vec<_>>();
    let admin_host_ids = admin_hosts
        .iter()
        .map(|host| host_node_id(host))
        .collect::<BTreeSet<_>>();
    clusters.push(AssetCluster {
        cluster_id: "cluster:admin-surface".to_string(),
        cluster_type: "admin-surface".to_string(),
        related_nodes: host_nodes,
        shared_indicators: vec!["admin-like naming or dashboard technology".to_string()],
        risk_score: 70,
    });

    for cluster in shared_ip_clusters {
        let shared_admin_nodes = cluster
            .related_nodes
            .iter()
            .filter(|node| admin_host_ids.contains(*node))
            .cloned()
            .collect::<Vec<_>>();

        if shared_admin_nodes.len() > 1 {
            clusters.push(AssetCluster {
                cluster_id: format!("{}:admin", cluster.cluster_id),
                cluster_type: "shared-admin-infrastructure".to_string(),
                related_nodes: shared_admin_nodes,
                shared_indicators: cluster.shared_indicators.clone(),
                risk_score: 85,
            });
        }
    }

    clusters
}

fn detect_anomalies(view: &CorrelationView, clusters: &[AssetCluster]) -> Vec<GraphAnomaly> {
    let mut anomalies = Vec::new();

    for host in all_hosts(view) {
        let title = view.host_to_titles.get(&host).map(String::as_str);
        let technologies = view.host_to_technologies.get(&host);

        for keyword in ["admin", "internal", "dev", "staging", "test", "legacy"] {
            if host.to_ascii_lowercase().contains(keyword) {
                anomalies.push(GraphAnomaly {
                    id: format!("anomaly:hostname:{}:{}", slugify(&host), keyword),
                    kind: "suspicious-hostname".to_string(),
                    severity: if matches!(keyword, "admin" | "internal") {
                        "high".to_string()
                    } else {
                        "medium".to_string()
                    },
                    description: format!("Hostname {host} contains suspicious keyword '{keyword}'"),
                    related_nodes: vec![host_node_id(&host)],
                    indicators: vec![keyword.to_string()],
                });
            }
        }

        if let Some(ports) = view.host_to_ports.get(&host) {
            for port in ports {
                if !matches!(*port, 80 | 443 | 8080 | 8443) {
                    anomalies.push(GraphAnomaly {
                        id: format!("anomaly:port:{}:{}", slugify(&host), port),
                        kind: "unusual-port".to_string(),
                        severity: "medium".to_string(),
                        description: format!("Host {host} exposes unusual port {port}"),
                        related_nodes: vec![host_node_id(&host)],
                        indicators: vec![port.to_string()],
                    });
                }
            }
        }

        if let Some(technologies) = technologies {
            let lowered = technologies
                .iter()
                .map(|technology| technology.to_ascii_lowercase())
                .collect::<Vec<_>>();

            for technology in &lowered {
                if is_legacy_technology(technology) {
                    anomalies.push(GraphAnomaly {
                        id: format!(
                            "anomaly:legacy-tech:{}:{}",
                            slugify(&host),
                            slugify(technology)
                        ),
                        kind: "legacy-technology".to_string(),
                        severity: "medium".to_string(),
                        description: format!(
                            "Host {host} appears to use legacy or aging technology '{technology}'"
                        ),
                        related_nodes: vec![host_node_id(&host)],
                        indicators: vec![technology.clone()],
                    });
                }

                if is_dashboard_technology(technology) {
                    anomalies.push(GraphAnomaly {
                        id: format!(
                            "anomaly:dashboard:{}:{}",
                            slugify(&host),
                            slugify(technology)
                        ),
                        kind: "exposed-dashboard".to_string(),
                        severity: "high".to_string(),
                        description: format!(
                            "Host {host} appears to expose dashboard-like technology '{technology}'"
                        ),
                        related_nodes: vec![host_node_id(&host)],
                        indicators: vec![technology.clone()],
                    });
                }
            }
        }

        if is_admin_like(&host, title, technologies) {
            anomalies.push(GraphAnomaly {
                id: format!("anomaly:admin-surface:{}", slugify(&host)),
                kind: "admin-like-surface".to_string(),
                severity: "high".to_string(),
                description: format!(
                    "Host {host} appears admin-like based on naming, title, or technology"
                ),
                related_nodes: vec![host_node_id(&host)],
                indicators: collect_admin_indicators(&host, title, technologies),
            });
        }
    }

    for cluster in clusters {
        if cluster.cluster_type == "shared-admin-infrastructure" {
            anomalies.push(GraphAnomaly {
                id: format!("anomaly:cluster:{}", slugify(&cluster.cluster_id)),
                kind: "shared-admin-infrastructure".to_string(),
                severity: "high".to_string(),
                description: "Multiple admin-like systems appear to share infrastructure"
                    .to_string(),
                related_nodes: cluster.related_nodes.clone(),
                indicators: cluster.shared_indicators.clone(),
            });
        }
    }

    anomalies
}

fn host_pairs(hosts: &[String]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for left in 0..hosts.len() {
        for right in (left + 1)..hosts.len() {
            pairs.push((hosts[left].clone(), hosts[right].clone()));
        }
    }
    pairs
}

fn all_hosts(view: &CorrelationView) -> BTreeSet<String> {
    let mut hosts = BTreeSet::new();
    hosts.extend(view.host_to_titles.keys().cloned());
    hosts.extend(view.host_to_technologies.keys().cloned());
    hosts.extend(view.host_to_ips.keys().cloned());
    hosts.extend(view.host_to_ports.keys().cloned());
    for (source, target) in &view.redirects {
        hosts.insert(source.clone());
        hosts.insert(target.clone());
    }
    hosts
}

fn is_legacy_technology(value: &str) -> bool {
    [
        "drupal 7",
        "php 5",
        "apache 2.2",
        "jquery 1",
        "iis 6",
        "asp.net 4",
    ]
    .iter()
    .any(|needle| value.contains(needle))
}

fn is_dashboard_technology(value: &str) -> bool {
    ["grafana", "jenkins", "kibana", "prometheus"]
        .iter()
        .any(|needle| value.contains(needle))
}

fn is_admin_like(host: &str, title: Option<&str>, technologies: Option<&BTreeSet<String>>) -> bool {
    let host_lc = host.to_ascii_lowercase();
    if ["admin", "internal", "console", "dashboard", "ops"]
        .iter()
        .any(|needle| host_lc.contains(needle))
    {
        return true;
    }

    if let Some(title) = title {
        let title_lc = title.to_ascii_lowercase();
        if ["admin", "dashboard", "console", "internal"]
            .iter()
            .any(|needle| title_lc.contains(needle))
        {
            return true;
        }
    }

    if let Some(technologies) = technologies {
        for technology in technologies {
            if is_dashboard_technology(&technology.to_ascii_lowercase()) {
                return true;
            }
        }
    }

    false
}

fn collect_admin_indicators(
    host: &str,
    title: Option<&str>,
    technologies: Option<&BTreeSet<String>>,
) -> Vec<String> {
    let mut indicators = Vec::new();
    let host_lc = host.to_ascii_lowercase();

    for keyword in ["admin", "internal", "console", "dashboard", "ops"] {
        if host_lc.contains(keyword) {
            indicators.push(format!("hostname:{keyword}"));
        }
    }

    if let Some(title) = title {
        let title_lc = title.to_ascii_lowercase();
        for keyword in ["admin", "dashboard", "console", "internal"] {
            if title_lc.contains(keyword) {
                indicators.push(format!("title:{keyword}"));
            }
        }
    }

    if let Some(technologies) = technologies {
        for technology in technologies {
            let technology_lc = technology.to_ascii_lowercase();
            if is_dashboard_technology(&technology_lc) {
                indicators.push(format!("technology:{technology}"));
            }
        }
    }

    indicators.sort();
    indicators.dedup();
    indicators
}

fn sort_edges(edges: &mut Vec<GraphEdge>) {
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
}

fn sort_clusters(clusters: &mut Vec<AssetCluster>) {
    clusters.sort_by(|left, right| left.cluster_id.cmp(&right.cluster_id));
}

fn sort_anomalies(anomalies: &mut Vec<GraphAnomaly>) {
    anomalies.sort_by(|left, right| left.id.cmp(&right.id));
}

fn host_node_id(host: &str) -> String {
    format!("host:{}", slugify(host))
}

fn technology_node_id(technology: &str) -> String {
    format!("technology:{}", slugify(technology))
}

fn normalize_title(value: &str) -> String {
    value.trim().to_ascii_lowercase()
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
    use std::collections::BTreeSet;

    use super::{correlate, CorrelationView};

    #[test]
    fn shared_ip_clustering_creates_edges_and_cluster() {
        let mut view = CorrelationView::default();
        view.host_to_ips.insert(
            "admin.example.com".to_string(),
            BTreeSet::from(["203.0.113.10".to_string()]),
        );
        view.host_to_ips.insert(
            "portal.example.com".to_string(),
            BTreeSet::from(["203.0.113.10".to_string()]),
        );
        view.host_to_titles
            .insert("admin.example.com".to_string(), "Admin Console".to_string());
        view.host_to_titles.insert(
            "portal.example.com".to_string(),
            "Shared Portal".to_string(),
        );

        let output = correlate(&view);
        assert!(output
            .edges
            .iter()
            .any(|edge| edge.relationship.to_string() == "shares_ip"));
        assert!(output
            .clusters
            .iter()
            .any(|cluster| cluster.cluster_type == "shared-infrastructure"));
    }

    #[test]
    fn shared_title_clustering_creates_edges_and_cluster() {
        let mut view = CorrelationView::default();
        view.host_to_titles
            .insert("app.example.com".to_string(), "Shared Title".to_string());
        view.host_to_titles
            .insert("api.example.com".to_string(), "Shared Title".to_string());
        view.host_to_ips.insert(
            "app.example.com".to_string(),
            BTreeSet::from(["203.0.113.20".to_string()]),
        );
        view.host_to_ips.insert(
            "api.example.com".to_string(),
            BTreeSet::from(["198.51.100.30".to_string()]),
        );

        let output = correlate(&view);
        assert!(output
            .edges
            .iter()
            .any(|edge| edge.relationship.to_string() == "shares_title"));
        assert!(output
            .clusters
            .iter()
            .any(|cluster| cluster.cluster_type == "shared-title"));
    }

    trait RelationshipTypeToString {
        fn to_string(&self) -> String;
    }

    impl RelationshipTypeToString for crate::models::RelationshipType {
        fn to_string(&self) -> String {
            match self {
                crate::models::RelationshipType::ResolvesTo => "resolves_to".to_string(),
                crate::models::RelationshipType::RedirectsTo => "redirects_to".to_string(),
                crate::models::RelationshipType::UsesTechnology => "uses_technology".to_string(),
                crate::models::RelationshipType::SharesIp => "shares_ip".to_string(),
                crate::models::RelationshipType::SharesTitle => "shares_title".to_string(),
                crate::models::RelationshipType::SharesFavicon => "shares_favicon".to_string(),
                crate::models::RelationshipType::References => "references".to_string(),
                crate::models::RelationshipType::LoadsScript => "loads_script".to_string(),
                crate::models::RelationshipType::ContainsParameter => {
                    "contains_parameter".to_string()
                }
                crate::models::RelationshipType::Hosts => "hosts".to_string(),
                crate::models::RelationshipType::BelongsToCluster => {
                    "belongs_to_cluster".to_string()
                }
                crate::models::RelationshipType::RequiresAuth => "requires_auth".to_string(),
                crate::models::RelationshipType::ReturnsObject => "returns_object".to_string(),
                crate::models::RelationshipType::ReferencesParameter => {
                    "references_parameter".to_string()
                }
                crate::models::RelationshipType::BelongsToApi => "belongs_to_api".to_string(),
                crate::models::RelationshipType::UsesToken => "uses_token".to_string(),
                crate::models::RelationshipType::ReferencesSchema => {
                    "references_schema".to_string()
                }
                crate::models::RelationshipType::LoadsEndpoint => "loads_endpoint".to_string(),
                crate::models::RelationshipType::RelatedToAuthFlow => {
                    "related_to_auth_flow".to_string()
                }
            }
        }
    }
}
