import type {
  AssetCardLike,
  AuditEventLike,
  CommandPreview,
  CustomProfile,
  GuiConfig,
  GuiCommandRequest,
  GuiCommandResult,
  WorkspaceSnapshot
} from "./types";

export const DEFAULT_GUI_CONFIG: GuiConfig = {
  mode: "beginner",
  redactionEnabled: true,
  rememberedWorkspace: "",
  acknowledgements: {
    targetContactUnderstood: false,
    advancedModeUnderstood: false,
    codexExecutionUnderstood: false
  },
  rateLimits: {
    httpRequestsPerSecond: 4,
    dnsQueriesPerSecond: 20,
    screenshotConcurrency: 2
  },
  concurrency: {
    maxPhases: 1,
    maxArtifactsPerView: 250
  },
  customProfiles: [],
  customToolArgs: []
};

export const BUILTIN_PROFILES: CustomProfile[] = [
  {
    name: "passive",
    description:
      "Run planning plus local graph, API, enrichment, review, llm-pack, and validation.",
    phases: ["run", "graph", "api-intel", "enrich", "review", "llm-pack", "validate"],
    allowTargetContact: false,
    notes: ["External adapters stay dry-run unless --execute is passed."]
  },
  {
    name: "active-lite",
    description:
      "Scope-validated external recon plus local analysis. External execution still requires --execute.",
    phases: [
      "run",
      "map",
      "graph",
      "api-intel",
      "enrich --api-intel",
      "review",
      "llm-pack",
      "validate"
    ],
    allowTargetContact: true,
    notes: ["Normal --execute never implies Codex execution."]
  },
  {
    name: "api-focused",
    description: "Local API and JavaScript analysis pipeline for existing artifacts.",
    phases: ["api-intel", "enrich --api-intel", "review", "llm-pack", "validate"],
    allowTargetContact: false,
    notes: ["Uses only local artifacts."]
  },
  {
    name: "mapping-focused",
    description: "DNS, screenshot, and tech mapping followed by graph and review.",
    phases: ["map", "graph", "enrich", "review", "validate"],
    allowTargetContact: true,
    notes: ["Map remains dry-run until --execute."]
  },
  {
    name: "review-only",
    description: "Turn existing artifacts into a review workspace.",
    phases: ["enrich", "review", "validate"],
    allowTargetContact: false,
    notes: ["Safe for local-only workflows."]
  },
  {
    name: "llm-pack-only",
    description: "Build LLM context bundles from existing review outputs.",
    phases: ["llm-pack", "validate"],
    allowTargetContact: false,
    notes: ["No target contact."]
  }
];

export function mockWorkspaceSnapshot(rootPath = "."): WorkspaceSnapshot {
  const auditEvents: AuditEventLike[] = [
    {
      timestamp: "2026-05-15T18:01:00Z",
      phase: "pipeline",
      event_type: "phase_started",
      message: "Passive pipeline planned."
    },
    {
      timestamp: "2026-05-15T18:01:08Z",
      phase: "llm-pack",
      event_type: "phase_completed",
      message: "Local reasoning pack generated."
    },
    {
      timestamp: "2026-05-15T18:01:11Z",
      phase: "codex-run",
      event_type: "dry_run_plan_created",
      message: "Codex command plan created without execution."
    }
  ];

  const assetCards: AssetCardLike[] = [
    {
      asset: "auth.example.com",
      path: "output/review/asset-cards/auth-example-com.md",
      markdown:
        "# auth.example.com\n\n## Overview\n\nInteresting auth surface candidate with staging indicators.\n\n## Evidence\n\n- ev-1\n- ev-2\n- ev-7\n\n## Suggested Review Steps\n\n- Review schema exposure.\n- Compare auth routes manually.\n\n## Caution\n\nRequires validation. Do not treat as a confirmed vulnerability.\n"
    },
    {
      asset: "api.example.com",
      path: "output/review/asset-cards/api-example-com.md",
      markdown:
        "# api.example.com\n\n## Overview\n\nBilling-related API family with documentation and object-model hints.\n\n## Evidence\n\n- ev-12\n- ev-13\n\n## Suggested Review Steps\n\n- Review object sensitivity.\n- Check documentation exposure manually.\n"
    }
  ];

  return {
    rootPath,
    browserFallback: true,
    warnings: [
      "Browser demo mode is active. Switch to the Tauri shell to read real workspace artifacts.",
      "Validation should be reviewed before any Codex execution."
    ],
    workspaceHealth: {
      detectedFrom: "project-root",
      status: "healthy",
      rootPath,
      outputPath: `${rootPath}/output`,
      configPath: `${rootPath}/config`,
      docsPath: `${rootPath}/docs`,
      messages: [
        "Workspace root was detected directly.",
        "Required config and output directories were found."
      ],
      checks: [
        {
          key: "config",
          label: "Config directory",
          path: `${rootPath}/config`,
          present: true,
          required: true,
          message: "Config directory is available."
        },
        {
          key: "output",
          label: "Output directory",
          path: `${rootPath}/output`,
          present: true,
          required: true,
          message: "Output directory is available."
        },
        {
          key: "docs",
          label: "Docs directory",
          path: `${rootPath}/docs`,
          present: true,
          required: false,
          message: "Docs directory is available."
        },
        {
          key: "review-queue",
          label: "Review queue",
          path: `${rootPath}/output/review/priority-queue.json`,
          present: true,
          required: false,
          message: "Review queue artifact is available."
        }
      ]
    },
    manifest: {
      version: "0.1.0",
      command:
        "reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/ --include-codex",
      timestamp: "2026-05-15T18:01:11Z",
      scope_file_hash: "f4f2a84e9ca2a31b",
      config_hash: "1d85a2c11f9f0dd0",
      warnings: ["Codex remained plan-only because --execute-codex was not provided."],
      errors: [],
      artifact_counts: {
        maps: 6,
        enrichment: 5,
        review: 6,
        "llm-pack": 8,
        "codex-insights": 4
      }
    },
    validation: {
      success: true,
      warnings: ["Optional GraphQL artifact was not present."],
      errors: [],
      checks: [
        { name: "graph_integrity", status: "ok" },
        { name: "review_integrity", status: "ok" },
        { name: "codex_review_integrity", status: "warning" }
      ]
    },
    auditEvents,
    guiExecutionLog: auditEvents,
    scopeText:
      "# Example only. Do not scan without authorization.\nexample.com\napi.example.com\nstaging.example.com\n",
    exclusionText: "# Example exclusion list\ninternal.example.com\n",
    assetCards,
    reviewQueue: {
      summary: {
        total_assets: 3,
        high_priority_count: 2,
        medium_priority_count: 1
      },
      items: [
        {
          rank: 1,
          asset: "auth.example.com",
          risk_level: "high",
          score: 82,
          confidence: 0.84,
          semantic_roles: ["Authentication", "ApiGateway"],
          environments: ["Staging", "Internal"],
          reasons: [
            "Shares infrastructure with an admin-like asset.",
            "API documentation and auth indicators are present."
          ],
          evidence_refs: ["ev-1", "ev-2", "ev-7"],
          recommended_next_steps: [
            "Review documentation exposure.",
            "Validate auth flow assumptions manually."
          ]
        },
        {
          rank: 2,
          asset: "admin.example.com",
          risk_level: "high",
          score: 78,
          confidence: 0.79,
          semantic_roles: ["AdminDashboard", "Monitoring"],
          environments: ["Production"],
          reasons: ["Grafana-like title overlap and admin keywords were observed."],
          evidence_refs: ["ev-5", "ev-9"],
          recommended_next_steps: ["Review dashboard access requirements."]
        },
        {
          rank: 3,
          asset: "api.example.com",
          risk_level: "medium",
          score: 71,
          confidence: 0.75,
          semantic_roles: ["ApiGateway"],
          environments: ["Production"],
          reasons: ["OpenAPI endpoints and billing objects are present."],
          evidence_refs: ["ev-12", "ev-13"],
          recommended_next_steps: ["Review schema exposure and object models."]
        }
      ]
    },
    graph: {
      nodes: [
        { id: "host:auth.example.com", node_type: "Host", value: "auth.example.com" },
        { id: "host:api.example.com", node_type: "Host", value: "api.example.com" },
        { id: "ip:203.0.113.10", node_type: "Ip", value: "203.0.113.10" },
        { id: "tech:grafana", node_type: "Technology", value: "Grafana" }
      ],
      edges: [
        { source: "host:auth.example.com", target: "ip:203.0.113.10", relationship: "ResolvesTo" },
        { source: "host:api.example.com", target: "ip:203.0.113.10", relationship: "SharesIp" },
        { source: "host:auth.example.com", target: "tech:grafana", relationship: "UsesTechnology" }
      ],
      clusters: [
        {
          cluster_id: "infra-cluster-1",
          cluster_type: "SharedIp",
          related_nodes: ["host:auth.example.com", "host:api.example.com", "ip:203.0.113.10"]
        }
      ],
      summary: {
        node_count: 4,
        edge_count: 3,
        largest_cluster_size: 3
      }
    },
    apiIntel: {
      summaryMarkdown:
        "# API Summary\n\n- OpenAPI docs candidate at `/swagger`.\n- GraphQL hint at `/graphql`.\n- Sensitive object candidates: User, Account, Billing.\n",
      endpoints: [
        {
          endpoint_id: "ep-1",
          method: "GET",
          path: "/swagger",
          normalized_path: "/swagger",
          auth_indicators: ["Bearer"],
          inferred_objects: ["User"],
          source: "js-observations"
        },
        {
          endpoint_id: "ep-2",
          method: "POST",
          path: "/api/v1/billing/export",
          normalized_path: "/api/v1/billing/export",
          auth_indicators: ["JWT"],
          inferred_objects: ["Billing"],
          source: "schema"
        }
      ],
      objects: [
        { object_name: "User", inferred_sensitivity: "potentially sensitive" },
        { object_name: "Billing", inferred_sensitivity: "potentially sensitive" }
      ],
      relationships: [
        { source_endpoint: "ep-2", target_object: "Billing", relationship_type: "returns_object" }
      ],
      authObservations: [
        {
          asset: "auth.example.com",
          auth_type: "JWT",
          confidence: 0.88,
          evidence: ["Authorization: Bearer ..."]
        }
      ],
      jsObservations: [
        {
          js_file: "app.bundle.js",
          discovered_endpoints: ["/admin", "/graphql", "/feature-flags"],
          discovered_feature_flags: ["betaBilling"],
          evidence: ["window.__FEATURES__"]
        }
      ],
      schemas: [
        {
          schema_type: "OpenAPI",
          schema_location: "/swagger.json",
          endpoints: ["/api/v1/billing/export", "/api/v1/users/{id}"]
        }
      ],
      graphqlObservations: [
        {
          endpoint: "/graphql",
          introspection_detected: false,
          notes: "GraphQL route candidate from JS references."
        }
      ]
    },
    enrichment: {
      semanticAssets: [
        {
          asset: "auth.example.com",
          roles: ["Authentication", "ApiGateway"],
          environments: ["Staging", "Internal"],
          neighborhood_summary:
            "Shares infrastructure with 3 related hosts, uses Grafana, and references schema docs."
        },
        {
          asset: "admin.example.com",
          roles: ["AdminDashboard", "Monitoring"],
          environments: ["Production"],
          neighborhood_summary: "Shares title with Grafana-like host and admin-like cluster."
        }
      ],
      observations: [
        {
          observation_id: "obs-1",
          asset: "auth.example.com",
          observation_type: "auth-surface",
          description: "Likely staging authentication surface candidate."
        },
        {
          observation_id: "obs-2",
          asset: "api.example.com",
          observation_type: "schema-exposure",
          description: "API documentation candidate worth manual review."
        }
      ],
      riskExplanations: [
        {
          asset: "auth.example.com",
          risk_level: "high",
          score: 82,
          explanation:
            "Interesting auth surface candidate with API documentation and internal naming. Requires validation.",
          recommended_next_steps: ["Review docs", "Compare auth route families"]
        }
      ],
      summaryMarkdown:
        "# Enrichment Summary\n\n## API Intelligence Summary\n\n- Auth surface indicators present.\n- Schema documentation candidate present.\n"
    },
    llmPack: {
      reasoningQueue: [
        {
          asset: "auth.example.com",
          prompt_template: "asset_triage_prompt.md",
          why_selected: [
            "Rich auth evidence",
            "Graph neighborhood complexity",
            "Documentation exposure"
          ]
        },
        {
          asset: "api.example.com",
          prompt_template: "api_surface_reasoning_prompt.md",
          why_selected: ["Sensitive object candidates", "Billing endpoints"]
        }
      ],
      summary: {
        max_context_chars: 12000,
        total_items: 2
      },
      promptNames: [
        "asset_triage_prompt.md",
        "api_surface_reasoning_prompt.md",
        "auth_flow_review_prompt.md",
        "js_intelligence_review_prompt.md",
        "report_draft_prompt.md"
      ]
    },
    codexSummary: {
      planned_count: 2,
      executed_count: 0,
      success_count: 0,
      failure_count: 0,
      warnings: ["Codex remained in plan-only mode."],
      results: [
        {
          asset: "auth.example.com",
          template: "asset_triage_prompt.md",
          executed: false,
          result_path: "output/codex-insights/results/001-auth-example-com.md"
        }
      ]
    },
    codexReview: {
      items: [
        {
          rank: 1,
          asset: "auth.example.com",
          unsupported_claim_count: 1,
          evidence_gap_count: 0,
          wording_warning_count: 1,
          requires_validation_language: false
        }
      ],
      unsupportedClaims: [
        {
          asset: "auth.example.com",
          phrase: "confirmed vulnerability"
        }
      ],
      evidenceGaps: [],
      wordingWarnings: [
        {
          asset: "auth.example.com",
          text: "Attempt an auth bypass to confirm impact."
        }
      ],
      summaryMarkdown:
        "# Codex Review Summary\n\nCodex outputs are hypotheses only. Unsupported claim wording and unsafe next-step suggestions were flagged."
    }
  };
}

export function mockCommandResult(
  request: GuiCommandRequest,
  preview: CommandPreview
): GuiCommandResult {
  const phaseSummary =
    request.kind === "pipeline"
      ? "Pipeline preview created in browser demo mode."
      : "GUI browser mode does not execute commands. Use the Tauri shell to run ReconPilot.";

  return {
    exactCommand: preview.exactCommand,
    executed: false,
    dryRun: preview.dryRun,
    success: true,
    exitCode: 0,
    stdout: phaseSummary,
    stderr: "",
    guiLogPath: "output/gui-execution-log.jsonl",
    warnings: ["Browser demo mode cannot spawn the ReconPilot binary."]
  };
}
