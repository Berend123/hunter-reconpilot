import { describe, expect, it } from "vitest";

import { BUILTIN_PROFILES } from "../mock";
import {
  createCodexPreview,
  createPipelinePreview,
  inspectGuiConfig
} from "./commanding";

describe("command previews", () => {
  it("builds a passive pipeline preview with codex plan-only", () => {
    const preview = createPipelinePreview({
      scopePath: "config/scope.example.txt",
      outDir: "output/",
      profile: BUILTIN_PROFILES[0],
      execute: false,
      includeCodex: true,
      executeCodex: false,
      mode: "beginner"
    });

    expect(preview.exactCommand).toContain("--include-codex");
    expect(preview.exactCommand).not.toContain("--execute-codex");
    expect(preview.dryRun).toBe(true);
    expect(preview.phasePreview.some((phase) => phase.includes("codex-run"))).toBe(true);
  });

  it("blocks codex execution without include-codex", () => {
    const preview = createPipelinePreview({
      scopePath: "config/scope.example.txt",
      outDir: "output/",
      profile: BUILTIN_PROFILES[0],
      execute: false,
      includeCodex: false,
      executeCodex: true,
      mode: "advanced"
    });

    expect(preview.blockedReasons).toContain("--execute-codex requires --include-codex.");
  });

  it("builds a codex preview without dangerous flags", () => {
    const preview = createCodexPreview({
      packDir: "output/llm-pack/",
      outDir: "output/codex-insights/",
      executeCodex: true,
      limit: 3,
      template: "asset_triage_prompt"
    });

    expect(preview.exactCommand).toContain("reconpilot codex-run");
    expect(preview.exactCommand).toContain("--execute-codex");
    expect(preview.exactCommand).not.toContain("--yolo");
    expect(preview.exactCommand).not.toContain(
      "--dangerously-bypass-approvals-and-sandbox"
    );
  });

  it("warns on risky GUI config values", () => {
    const warnings = inspectGuiConfig({
      mode: "advanced",
      redactionEnabled: true,
      rememberedWorkspace: "",
      acknowledgements: {
        targetContactUnderstood: true,
        advancedModeUnderstood: true,
        codexExecutionUnderstood: true
      },
      rateLimits: {
        httpRequestsPerSecond: 50,
        dnsQueriesPerSecond: 150,
        screenshotConcurrency: 10
      },
      concurrency: {
        maxPhases: 1,
        maxArtifactsPerView: 250
      },
      customProfiles: [],
      customToolArgs: [{ tool: "codex", args: ["--yolo"] }]
    });

    expect(warnings.length).toBeGreaterThanOrEqual(4);
  });
});
