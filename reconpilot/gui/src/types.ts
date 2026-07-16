export type AppMode = "beginner" | "advanced";

export type ScreenId =
  | "dashboard"
  | "workspace"
  | "scope"
  | "pipeline"
  | "profiles"
  | "tools"
  | "review"
  | "asset-detail"
  | "graph"
  | "api-intel"
  | "enrichment"
  | "llm-pack"
  | "codex-runner"
  | "codex-insights"
  | "codex-review"
  | "validation"
  | "settings";

export interface ScreenDefinition {
  id: ScreenId;
  label: string;
  category: "workspace" | "analysis" | "reasoning" | "control";
}

export interface CustomProfile {
  name: string;
  description: string;
  phases: string[];
  allowTargetContact: boolean;
  notes: string[];
}

export interface ToolArgConfig {
  tool: string;
  args: string[];
}

export interface GuiConfig {
  mode: AppMode;
  redactionEnabled: boolean;
  rememberedWorkspace?: string;
  acknowledgements: {
    targetContactUnderstood: boolean;
    advancedModeUnderstood: boolean;
    codexExecutionUnderstood: boolean;
  };
  rateLimits: {
    httpRequestsPerSecond: number;
    dnsQueriesPerSecond: number;
    screenshotConcurrency: number;
  };
  concurrency: {
    maxPhases: number;
    maxArtifactsPerView: number;
  };
  customProfiles: CustomProfile[];
  customToolArgs: ToolArgConfig[];
}

export interface ManifestLike {
  version?: string;
  command?: string;
  timestamp?: string;
  scope_file_hash?: string;
  config_hash?: string;
  warnings?: string[];
  errors?: string[];
  artifact_counts?: Record<string, number>;
}

export interface ValidationReportLike {
  success?: boolean;
  warnings?: string[];
  errors?: string[];
  checks?: Array<Record<string, unknown>>;
}

export interface AuditEventLike {
  timestamp?: string;
  phase?: string;
  event_type?: string;
  message?: string;
  details?: Record<string, unknown>;
}

export interface ReviewItemLike {
  rank: number;
  asset: string;
  risk_level: string;
  score: number;
  confidence: number;
  semantic_roles: string[];
  environments: string[];
  reasons: string[];
  evidence_refs: string[];
  recommended_next_steps: string[];
}

export interface ReviewQueueLike {
  summary?: Record<string, unknown>;
  items: ReviewItemLike[];
}

export interface AssetCardLike {
  asset: string;
  path: string;
  markdown: string;
}

export interface GraphLike {
  nodes?: Array<Record<string, unknown>>;
  edges?: Array<Record<string, unknown>>;
  clusters?: Array<Record<string, unknown>>;
  summary?: Record<string, unknown>;
}

export interface ApiIntelBundleLike {
  summaryMarkdown?: string;
  endpoints?: Array<Record<string, unknown>>;
  objects?: Array<Record<string, unknown>>;
  relationships?: Array<Record<string, unknown>>;
  authObservations?: Array<Record<string, unknown>>;
  jsObservations?: Array<Record<string, unknown>>;
  schemas?: Array<Record<string, unknown>>;
  graphqlObservations?: Array<Record<string, unknown>>;
}

export interface EnrichmentBundleLike {
  semanticAssets?: Array<Record<string, unknown>>;
  observations?: Array<Record<string, unknown>>;
  riskExplanations?: Array<Record<string, unknown>>;
  summaryMarkdown?: string;
}

export interface LlmPackBundleLike {
  reasoningQueue?: Array<Record<string, unknown>>;
  summary?: Record<string, unknown>;
  promptNames?: string[];
}

export interface CodexSummaryLike {
  planned_count?: number;
  executed_count?: number;
  success_count?: number;
  failure_count?: number;
  warnings?: string[];
  results?: Array<Record<string, unknown>>;
}

export interface CodexReviewLike {
  items?: Array<Record<string, unknown>>;
  unsupportedClaims?: Array<Record<string, unknown>>;
  evidenceGaps?: Array<Record<string, unknown>>;
  wordingWarnings?: Array<Record<string, unknown>>;
  summaryMarkdown?: string;
}

export interface WorkspaceHealthCheck {
  key: string;
  label: string;
  path: string;
  present: boolean;
  required: boolean;
  message: string;
}

export interface WorkspaceHealth {
  detectedFrom: "project-root" | "output-dir" | "config-dir" | "docs-dir" | "unknown";
  status: "healthy" | "partial" | "invalid";
  rootPath: string;
  outputPath: string;
  configPath: string;
  docsPath: string;
  messages: string[];
  checks: WorkspaceHealthCheck[];
}

export interface WorkspaceSnapshot {
  rootPath: string;
  browserFallback: boolean;
  warnings: string[];
  workspaceHealth: WorkspaceHealth;
  manifest?: ManifestLike;
  validation?: ValidationReportLike;
  auditEvents: AuditEventLike[];
  scopeText?: string;
  exclusionText?: string;
  assetCards?: AssetCardLike[];
  reviewQueue?: ReviewQueueLike;
  graph?: GraphLike;
  apiIntel?: ApiIntelBundleLike;
  enrichment?: EnrichmentBundleLike;
  llmPack?: LlmPackBundleLike;
  codexSummary?: CodexSummaryLike;
  codexReview?: CodexReviewLike;
  guiExecutionLog?: AuditEventLike[];
}

export type CommandKind =
  | "doctor"
  | "pipeline"
  | "validate"
  | "codex-run"
  | "codex-review";

export interface GuiCommandRequest {
  workspacePath: string;
  kind: CommandKind;
  scopePath?: string;
  profileName?: string;
  outDir?: string;
  inputDir?: string;
  packDir?: string;
  execute?: boolean;
  includeCodex?: boolean;
  executeCodex?: boolean;
  limit?: number;
  template?: string;
}

export interface CommandPreview {
  exactCommand: string;
  dryRun: boolean;
  targetTouching: boolean;
  localOnly: boolean;
  requiresScope: boolean;
  blockedReasons: string[];
  safetyNotices: string[];
  phasePreview: string[];
}

export interface GuiCommandResult {
  exactCommand: string;
  executed: boolean;
  dryRun: boolean;
  success: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  guiLogPath: string;
  warnings: string[];
}
