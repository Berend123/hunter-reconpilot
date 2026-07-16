import type {
  AppMode,
  CommandPreview,
  CustomProfile,
  GuiConfig
} from "../types";

export function createPipelinePreview(input: {
  scopePath: string;
  outDir: string;
  profile: CustomProfile;
  execute: boolean;
  includeCodex: boolean;
  executeCodex: boolean;
  mode: AppMode;
}): CommandPreview {
  const exactCommand = [
    "reconpilot pipeline",
    `--scope ${quoteIfNeeded(input.scopePath)}`,
    `--profile ${input.profile.name}`,
    `--out ${quoteIfNeeded(input.outDir)}`,
    input.execute ? "--execute" : "",
    input.includeCodex ? "--include-codex" : "",
    input.executeCodex ? "--execute-codex" : ""
  ]
    .filter(Boolean)
    .join(" ");

  const blockedReasons: string[] = [];
  const safetyNotices = [
    "GUI executes only the ReconPilot binary, never arbitrary tools directly.",
    "Normal --execute never implies --execute-codex.",
    "The exact command is shown before execution."
  ];

  if (!input.scopePath.trim()) {
    blockedReasons.push("A scope file is required before running the pipeline.");
  }

  if (input.executeCodex && !input.includeCodex) {
    blockedReasons.push("--execute-codex requires --include-codex.");
  }

  if (input.mode === "beginner") {
    safetyNotices.push("Beginner Mode keeps dry-run first and shows stronger warnings.");
  } else {
    safetyNotices.push("Advanced Mode still requires scope, previews, and audit logging.");
  }

  const phasePreview = [...input.profile.phases];
  if (input.includeCodex) {
    const llmIndex = phasePreview.findIndex((phase) => phase.includes("llm-pack"));
    if (llmIndex >= 0) {
      phasePreview.splice(
        llmIndex + 1,
        0,
        input.executeCodex ? "codex-run (execute)" : "codex-run (plan-only)"
      );
    } else {
      safetyNotices.push(
        "Selected profile has no llm-pack phase, so Codex inclusion may be ignored."
      );
    }
  }

  const targetTouching = input.execute && input.profile.allowTargetContact;
  return {
    exactCommand,
    dryRun: !input.execute && !input.executeCodex,
    targetTouching,
    localOnly: !targetTouching,
    requiresScope: true,
    blockedReasons,
    safetyNotices,
    phasePreview
  };
}

export function createCodexPreview(input: {
  packDir: string;
  outDir: string;
  executeCodex: boolean;
  limit: number;
  template: string;
}): CommandPreview {
  const blockedReasons: string[] = [];
  if (!input.packDir.trim()) {
    blockedReasons.push("llm-pack path is required.");
  }
  if (input.limit < 1) {
    blockedReasons.push("Codex limit must be at least 1.");
  }

  return {
    exactCommand: [
      "reconpilot codex-run",
      `--pack ${quoteIfNeeded(input.packDir)}`,
      `--out ${quoteIfNeeded(input.outDir)}`,
      `--limit ${input.limit}`,
      `--template ${input.template}`,
      input.executeCodex ? "--execute-codex" : ""
    ]
      .filter(Boolean)
      .join(" "),
    dryRun: !input.executeCodex,
    targetTouching: false,
    localOnly: true,
    requiresScope: false,
    blockedReasons,
    safetyNotices: [
      "Codex execution is optional and explicit-only.",
      "Codex --yolo and dangerous bypass flags are blocked.",
      "Results remain hypotheses and require validation."
    ],
    phasePreview: [input.executeCodex ? "codex-run (execute)" : "codex-run (plan-only)"]
  };
}

export function createStaticPreview(
  exactCommand: string,
  localOnly: boolean,
  safetyNotices: string[],
  phasePreview: string[]
): CommandPreview {
  return {
    exactCommand,
    dryRun: true,
    targetTouching: false,
    localOnly,
    requiresScope: false,
    blockedReasons: [],
    safetyNotices,
    phasePreview
  };
}

export function inspectGuiConfig(config: GuiConfig): string[] {
  const warnings: string[] = [];

  if (config.rateLimits.httpRequestsPerSecond > 20) {
    warnings.push(
      "HTTP request rate is high for a safety-first default; review before execution."
    );
  }
  if (config.rateLimits.dnsQueriesPerSecond > 100) {
    warnings.push(
      "DNS query rate is high; confirm this remains safe for the intended program."
    );
  }
  if (config.rateLimits.screenshotConcurrency > 6) {
    warnings.push("Screenshot concurrency is high and may create unnecessary load.");
  }
  for (const entry of config.customToolArgs) {
    const joined = entry.args.join(" ");
    if (
      joined.includes("--yolo") ||
      joined.includes("--dangerously-bypass-approvals-and-sandbox")
    ) {
      warnings.push(
        `Custom args for ${entry.tool} contain a forbidden Codex flag and should be removed.`
      );
    }
    if (/sqlmap|metasploit|xsstrike|nuclei/i.test(joined)) {
      warnings.push(
        `Custom args for ${entry.tool} reference unsupported tooling outside ReconPilot scope.`
      );
    }
  }

  return warnings;
}

export function quoteIfNeeded(value: string): string {
  return value.includes(" ") ? `"${value}"` : value;
}
