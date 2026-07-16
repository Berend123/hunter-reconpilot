use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconAsset {
    pub asset: String,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub source_tools: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub live: bool,
    #[serde(default)]
    pub first_seen: Option<DateTime<Utc>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconUrl {
    pub url: String,
    pub normalized_url: String,
    pub source: String,
    #[serde(default)]
    pub host: Option<String>,
    pub path: String,
    #[serde(default)]
    pub parameters: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskScore {
    pub total: i32,
    #[serde(default)]
    pub reasons: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconFinding {
    pub id: String,
    #[serde(default)]
    pub asset: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub normalized_url: Option<String>,
    #[serde(default)]
    pub parameters: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub technology: Vec<String>,
    #[serde(default)]
    pub source_tools: Vec<String>,
    #[serde(default)]
    pub score: Option<RiskScore>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconToolRun {
    pub tool: String,
    pub phase: String,
    pub binary_name: String,
    #[serde(default)]
    pub binary_path: Option<std::path::PathBuf>,
    pub binary_exists: bool,
    pub program: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    pub command_line: String,
    pub execute_requested: bool,
    pub executed: bool,
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    pub scope_source: std::path::PathBuf,
    #[serde(default)]
    pub planned_inputs: Vec<std::path::PathBuf>,
    #[serde(default)]
    pub output_files: Vec<std::path::PathBuf>,
    #[serde(default)]
    pub primary_output_path: Option<std::path::PathBuf>,
    #[serde(default)]
    pub stdout_path: Option<std::path::PathBuf>,
    #[serde(default)]
    pub stderr_path: Option<std::path::PathBuf>,
    pub planned_at: DateTime<Utc>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechFingerprint {
    pub target: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub technologies: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    pub source_tool: String,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub name: String,
    pub record_type: String,
    #[serde(default)]
    pub values: Vec<String>,
    pub source_tool: String,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotRecord {
    pub target: String,
    #[serde(default)]
    pub image_path: Option<std::path::PathBuf>,
    pub source_tool: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub captured_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMapNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMapEdge {
    pub from: String,
    pub to: String,
    pub relationship: String,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    pub generated_at: DateTime<Utc>,
    pub total_findings: usize,
    pub scored_findings: usize,
    #[serde(default)]
    pub highest_risk_titles: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipType {
    ResolvesTo,
    RedirectsTo,
    UsesTechnology,
    SharesIp,
    SharesTitle,
    SharesFavicon,
    References,
    LoadsScript,
    ContainsParameter,
    Hosts,
    BelongsToCluster,
    RequiresAuth,
    ReturnsObject,
    ReferencesParameter,
    BelongsToApi,
    UsesToken,
    ReferencesSchema,
    LoadsEndpoint,
    RelatedToAuthFlow,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationEvidence {
    pub source_tool: String,
    pub description: String,
    pub weight: f32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub value: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
    #[serde(default)]
    pub source_tools: Vec<String>,
    #[serde(default)]
    pub timestamps: Vec<DateTime<Utc>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub relationship: RelationshipType,
    pub confidence: f32,
    #[serde(default)]
    pub evidence: Vec<CorrelationEvidence>,
    #[serde(default)]
    pub timestamps: Vec<DateTime<Utc>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetCluster {
    pub cluster_id: String,
    pub cluster_type: String,
    #[serde(default)]
    pub related_nodes: Vec<String>,
    #[serde(default)]
    pub shared_indicators: Vec<String>,
    pub risk_score: i32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSummary {
    pub generated_at: DateTime<Utc>,
    pub node_count: usize,
    pub edge_count: usize,
    pub cluster_count: usize,
    pub anomaly_count: usize,
    #[serde(default)]
    pub top_technologies: Vec<String>,
    #[serde(default)]
    pub largest_clusters: Vec<String>,
    #[serde(default)]
    pub shared_infrastructure: Vec<String>,
    #[serde(default)]
    pub suspicious_naming: Vec<String>,
    #[serde(default)]
    pub likely_staging_systems: Vec<String>,
    #[serde(default)]
    pub likely_internal_systems: Vec<String>,
    pub redirect_chain_count: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticTag {
    pub tag: String,
    pub category: String,
    pub confidence: f32,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticObservation {
    pub observation_id: String,
    pub asset: String,
    pub observation_type: String,
    pub description: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub confidence: f32,
    #[serde(default)]
    pub related_nodes: Vec<String>,
}

#[allow(dead_code)]
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum AssetRole {
    Authentication,
    AdminDashboard,
    Monitoring,
    Logging,
    CICD,
    ApiGateway,
    Storage,
    Analytics,
    Documentation,
    CustomerApp,
    #[default]
    Unknown,
}

impl AssetRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Authentication => "authentication",
            Self::AdminDashboard => "admin_dashboard",
            Self::Monitoring => "monitoring",
            Self::Logging => "logging",
            Self::CICD => "cicd",
            Self::ApiGateway => "api_gateway",
            Self::Storage => "storage",
            Self::Analytics => "analytics",
            Self::Documentation => "documentation",
            Self::CustomerApp => "customer_app",
            Self::Unknown => "unknown",
        }
    }
}

#[allow(dead_code)]
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Copy, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentType {
    Production,
    Staging,
    Development,
    Testing,
    Internal,
    Legacy,
    #[default]
    Unknown,
}

impl EnvironmentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Staging => "staging",
            Self::Development => "development",
            Self::Testing => "testing",
            Self::Internal => "internal",
            Self::Legacy => "legacy",
            Self::Unknown => "unknown",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskExplanation {
    pub asset: String,
    pub risk_level: String,
    pub score: i32,
    pub explanation: String,
    #[serde(default)]
    pub contributing_factors: Vec<String>,
    #[serde(default)]
    pub recommended_next_steps: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedAsset {
    pub asset: String,
    #[serde(default)]
    pub semantic_tags: Vec<SemanticTag>,
    #[serde(default)]
    pub roles: Vec<AssetRole>,
    #[serde(default)]
    pub environments: Vec<EnvironmentType>,
    #[serde(default)]
    pub risk_explanations: Vec<RiskExplanation>,
    #[serde(default)]
    pub related_nodes: Vec<String>,
    #[serde(default)]
    pub api_endpoints: Vec<ApiEndpoint>,
    #[serde(default)]
    pub api_objects: Vec<ApiObject>,
    #[serde(default)]
    pub auth_observations: Vec<AuthObservation>,
    #[serde(default)]
    pub js_observations: Vec<JsObservation>,
    #[serde(default)]
    pub schema_observations: Vec<ApiSchema>,
    #[serde(default)]
    pub graphql_observations: Vec<GraphQlObservation>,
    pub neighborhood_summary: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedGraph {
    #[serde(default)]
    pub assets: Vec<EnrichedAsset>,
    #[serde(default)]
    pub observations: Vec<SemanticObservation>,
    #[serde(default)]
    pub risk_explanations: Vec<RiskExplanation>,
    pub original_graph_summary: GraphSummary,
    #[serde(default)]
    pub original_nodes: Vec<GraphNode>,
    #[serde(default)]
    pub original_edges: Vec<GraphEdge>,
    #[serde(default)]
    pub original_clusters: Vec<AssetCluster>,
    pub semantic_summary: SemanticSummary,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticSummary {
    pub generated_at: DateTime<Utc>,
    pub asset_count: usize,
    pub observation_count: usize,
    pub risk_explanation_count: usize,
    #[serde(default)]
    pub api_endpoint_count: usize,
    #[serde(default)]
    pub auth_surface_count: usize,
    #[serde(default)]
    pub js_observation_count: usize,
    #[serde(default)]
    pub schema_observation_count: usize,
    #[serde(default)]
    pub graphql_observation_count: usize,
    #[serde(default)]
    pub top_roles: Vec<String>,
    #[serde(default)]
    pub top_environments: Vec<String>,
    #[serde(default)]
    pub highest_priority_assets: Vec<String>,
    #[serde(default)]
    pub notable_neighborhood_observations: Vec<String>,
    #[serde(default)]
    pub sensitive_object_candidates: Vec<String>,
    #[serde(default)]
    pub api_intelligence_warnings: Vec<String>,
    #[serde(default)]
    pub recommended_next_steps: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    pub rank: usize,
    pub asset: String,
    pub risk_level: String,
    pub score: i32,
    pub confidence: f32,
    #[serde(default)]
    pub semantic_roles: Vec<AssetRole>,
    #[serde(default)]
    pub environments: Vec<EnvironmentType>,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub recommended_next_steps: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetCard {
    pub asset: String,
    pub overview: String,
    #[serde(default)]
    pub semantic_tags: Vec<SemanticTag>,
    #[serde(default)]
    pub roles: Vec<AssetRole>,
    #[serde(default)]
    pub environments: Vec<EnvironmentType>,
    pub graph_neighborhood_summary: String,
    #[serde(default)]
    pub risk_explanations: Vec<RiskExplanation>,
    #[serde(default)]
    pub evidence: Vec<EvidenceItem>,
    #[serde(default)]
    pub suggested_review_steps: Vec<String>,
    pub caution_note: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub evidence_id: String,
    pub asset: String,
    pub source: String,
    pub evidence_type: String,
    pub description: String,
    #[serde(default)]
    pub related_nodes: Vec<String>,
    #[serde(default)]
    pub related_edges: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewChecklist {
    pub title: String,
    #[serde(default)]
    pub items: Vec<String>,
    pub caution_note: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSummary {
    pub total_assets: usize,
    pub total_observations: usize,
    pub high_priority_count: usize,
    pub medium_priority_count: usize,
    #[serde(default)]
    pub top_roles: Vec<String>,
    #[serde(default)]
    pub top_environments: Vec<String>,
    #[serde(default)]
    pub top_review_targets: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub endpoint_id: String,
    pub method: String,
    pub path: String,
    pub normalized_path: String,
    #[serde(default)]
    pub parameters: Vec<String>,
    #[serde(default)]
    pub auth_indicators: Vec<String>,
    #[serde(default)]
    pub inferred_objects: Vec<String>,
    #[serde(default)]
    pub semantic_tags: Vec<SemanticTag>,
    pub source: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiObject {
    pub object_name: String,
    #[serde(default)]
    pub related_endpoints: Vec<String>,
    #[serde(default)]
    pub related_parameters: Vec<String>,
    pub inferred_sensitivity: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRelationship {
    pub source_endpoint: String,
    pub target_object: String,
    pub relationship_type: String,
    pub confidence: f32,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSchema {
    pub schema_type: String,
    pub schema_location: String,
    #[serde(default)]
    pub detected_version: Option<String>,
    #[serde(default)]
    pub endpoints: Vec<String>,
    #[serde(default)]
    pub auth_methods: Vec<String>,
    #[serde(default)]
    pub objects: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthObservation {
    pub asset: String,
    pub auth_type: String,
    #[serde(default)]
    pub indicators: Vec<String>,
    pub confidence: f32,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsObservation {
    pub js_file: String,
    #[serde(default)]
    pub discovered_endpoints: Vec<String>,
    #[serde(default)]
    pub discovered_roles: Vec<String>,
    #[serde(default)]
    pub discovered_auth_indicators: Vec<String>,
    #[serde(default)]
    pub discovered_feature_flags: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQlObservation {
    pub endpoint: String,
    pub introspection_detected: bool,
    #[serde(default)]
    pub schema_indicators: Vec<String>,
    #[serde(default)]
    pub auth_indicators: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiGraphSummary {
    pub generated_at: DateTime<Utc>,
    pub endpoint_count: usize,
    pub object_count: usize,
    pub relationship_count: usize,
    pub schema_count: usize,
    pub auth_observation_count: usize,
    pub graphql_observation_count: usize,
    pub js_observation_count: usize,
    pub api_family_count: usize,
    pub privileged_endpoint_count: usize,
    #[serde(default)]
    pub top_auth_styles: Vec<String>,
    #[serde(default)]
    pub likely_sensitive_objects: Vec<String>,
    #[serde(default)]
    pub hidden_route_candidates: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiIntelBundle {
    #[serde(default)]
    pub endpoints: Vec<ApiEndpoint>,
    #[serde(default)]
    pub objects: Vec<ApiObject>,
    #[serde(default)]
    pub relationships: Vec<ApiRelationship>,
    #[serde(default)]
    pub auth_observations: Vec<AuthObservation>,
    #[serde(default)]
    pub js_observations: Vec<JsObservation>,
    #[serde(default)]
    pub schemas: Vec<ApiSchema>,
    #[serde(default)]
    pub graphql_observations: Vec<GraphQlObservation>,
    #[serde(default)]
    pub summary: Option<ApiGraphSummary>,
    #[serde(default)]
    pub summary_markdown: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiDerivedObservation {
    pub asset: String,
    pub observation_type: String,
    pub description: String,
    pub confidence: f32,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub related_nodes: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmContextPack {
    pub generated_at: DateTime<Utc>,
    pub max_context_chars: usize,
    #[serde(default)]
    pub asset_context_files: Vec<String>,
    #[serde(default)]
    pub prompts: Vec<LlmPromptTemplate>,
    #[serde(default)]
    pub reasoning_queue: Vec<LlmReasoningItem>,
    pub summary: LlmPackSummary,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAssetContext {
    pub asset: String,
    pub risk_level: String,
    pub score: i32,
    pub confidence: f32,
    #[serde(default)]
    pub semantic_roles: Vec<AssetRole>,
    #[serde(default)]
    pub environments: Vec<EnvironmentType>,
    pub graph_neighborhood_summary: String,
    #[serde(default)]
    pub api_observations: Vec<String>,
    #[serde(default)]
    pub api_object_candidates: Vec<String>,
    #[serde(default)]
    pub auth_observations: Vec<String>,
    #[serde(default)]
    pub js_observations: Vec<String>,
    #[serde(default)]
    pub schema_observations: Vec<String>,
    #[serde(default)]
    pub graphql_observations: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub evidence_highlights: Vec<String>,
    #[serde(default)]
    pub cautious_next_step_questions: Vec<String>,
    pub context_markdown: String,
    pub estimated_chars: usize,
    pub truncated: bool,
    #[serde(default)]
    pub truncation_notes: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPromptTemplate {
    pub name: String,
    pub file_name: String,
    pub purpose: String,
    #[serde(default)]
    pub recommended_for: Vec<String>,
    #[serde(default)]
    pub safety_constraints: Vec<String>,
    pub template_markdown: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmReasoningItem {
    pub rank: usize,
    pub asset: String,
    pub review_rank: usize,
    pub risk_level: String,
    pub score: i32,
    pub confidence: f32,
    pub reasoning_score: i32,
    pub suggested_prompt_template: String,
    pub context_file: String,
    #[serde(default)]
    pub why_llm_review: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmPackSummary {
    pub generated_at: DateTime<Utc>,
    pub asset_context_count: usize,
    pub prompt_template_count: usize,
    pub reasoning_item_count: usize,
    pub max_context_chars: usize,
    pub total_evidence_refs: usize,
    pub truncated_context_count: usize,
    pub api_intel_present: bool,
    pub graph_summary_present: bool,
    #[serde(default)]
    pub top_review_themes: Vec<String>,
    #[serde(default)]
    pub top_api_auth_areas: Vec<String>,
    #[serde(default)]
    pub top_graph_clusters: Vec<String>,
    #[serde(default)]
    pub top_unknowns: Vec<String>,
    #[serde(default)]
    pub suggested_review_order: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexRunPlan {
    pub generated_at: DateTime<Utc>,
    pub pack_path: String,
    pub output_root: String,
    pub execute_requested: bool,
    pub codex_available: bool,
    pub max_prompt_chars: usize,
    pub limit: usize,
    #[serde(default)]
    pub template_filter: Option<String>,
    #[serde(default)]
    pub items: Vec<CodexRunItem>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexRunItem {
    pub rank: usize,
    pub asset: String,
    pub template: String,
    pub context_file: String,
    pub prompt_chars: usize,
    pub codex_command: String,
    pub executed: bool,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub why_selected: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexInsightResult {
    pub asset: String,
    pub template: String,
    pub codex_command: String,
    pub executed: bool,
    #[serde(default)]
    pub exit_status: Option<i32>,
    pub stdout_path: String,
    pub stderr_path: String,
    pub result_path: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexRunnerSummary {
    pub generated_at: DateTime<Utc>,
    pub pack_path: String,
    pub output_root: String,
    pub execute_requested: bool,
    pub codex_available: bool,
    pub planned_count: usize,
    pub executed_count: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub max_prompt_chars: usize,
    pub limit: usize,
    #[serde(default)]
    pub template_filter: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub results: Vec<CodexInsightResult>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexReviewItem {
    pub rank: usize,
    pub asset: String,
    pub template: String,
    pub executed: bool,
    #[serde(default)]
    pub exit_status: Option<i32>,
    pub result_path: String,
    pub sidecar_path: String,
    #[serde(default)]
    pub expected_evidence_refs: Vec<String>,
    #[serde(default)]
    pub mentioned_evidence_refs: Vec<String>,
    #[serde(default)]
    pub analyst_recommendations: Vec<String>,
    pub analyst_summary: String,
    pub requires_validation_language: bool,
    pub unsupported_claim_count: usize,
    pub evidence_gap_count: usize,
    pub wording_warning_count: usize,
    #[serde(default)]
    pub caution_notes: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsupportedClaim {
    pub asset: String,
    pub phrase: String,
    pub reason: String,
    pub source_path: String,
    pub requires_validation_language: bool,
    #[serde(default)]
    pub expected_evidence_refs: Vec<String>,
    #[serde(default)]
    pub mentioned_evidence_refs: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceGap {
    pub asset: String,
    pub description: String,
    pub source_path: String,
    #[serde(default)]
    pub expected_evidence_refs: Vec<String>,
    #[serde(default)]
    pub mentioned_evidence_refs: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordingWarning {
    pub asset: String,
    pub category: String,
    pub text: String,
    pub source_path: String,
    pub recommendation: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexReviewSummary {
    pub generated_at: DateTime<Utc>,
    pub total_results: usize,
    pub reviewed_items: usize,
    pub executed_count: usize,
    pub plan_only_count: usize,
    pub unsupported_claim_count: usize,
    pub evidence_gap_count: usize,
    pub wording_warning_count: usize,
    #[serde(default)]
    pub top_review_targets: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseStatus {
    Planned,
    Skipped,
    Completed,
    Failed,
    Warning,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelinePhase {
    pub name: String,
    pub phase_type: String,
    pub command: String,
    pub status: PhaseStatus,
    pub execute_phase: bool,
    pub touches_targets: bool,
    #[serde(default)]
    pub required_inputs: Vec<String>,
    #[serde(default)]
    pub expected_outputs: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineProfile {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub phases: Vec<PipelinePhase>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelinePlan {
    pub generated_at: DateTime<Utc>,
    pub profile: PipelineProfile,
    pub scope_path: String,
    pub output_root: String,
    pub execute_requested: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    pub profile_name: String,
    pub plan_path: std::path::PathBuf,
    pub plan_markdown_path: std::path::PathBuf,
    pub manifest_path: std::path::PathBuf,
    pub audit_log_path: std::path::PathBuf,
    #[serde(default)]
    pub phase_results: Vec<PipelinePhase>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

pub type UrlRecord = ReconUrl;
pub type ScoreBreakdown = RiskScore;
pub type FindingRecord = ReconFinding;
