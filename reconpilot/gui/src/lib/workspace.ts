import type {
  AssetCardLike,
  AuditEventLike,
  CodexReviewLike,
  CodexSummaryLike,
  ValidationReportLike,
  WorkspaceHealth,
  WorkspaceSnapshot
} from "../types";

export function friendlyWorkspaceStatus(
  health: WorkspaceHealth
): { label: string; tone: "safe" | "warning" | "danger" } {
  if (health.status === "healthy") {
    return { label: "Healthy", tone: "safe" };
  }
  if (health.status === "partial") {
    return { label: "Partial", tone: "warning" };
  }
  return { label: "Needs Attention", tone: "danger" };
}

export function explainMissingArtifacts(
  snapshot: WorkspaceSnapshot,
  screen:
    | "review"
    | "graph"
    | "api-intel"
    | "enrichment"
    | "llm-pack"
    | "codex-insights"
    | "codex-review"
): string[] {
  const outputPath = snapshot.workspaceHealth.outputPath;
  switch (screen) {
    case "review":
      return [
        "Review artifacts were not found.",
        `Expected: ${outputPath}/review/priority-queue.json`,
        "Run enrich and review, or use the passive pipeline profile."
      ];
    case "graph":
      return [
        "Graph artifacts were not found.",
        `Expected: ${outputPath}/maps/graph.json`,
        "Run graph after planning or mapping artifacts exist."
      ];
    case "api-intel":
      return [
        "API intelligence artifacts were not found.",
        `Expected: ${outputPath}/api-intel/api-endpoints.json`,
        "Run api-intel against existing local artifacts."
      ];
    case "enrichment":
      return [
        "Enrichment artifacts were not found.",
        `Expected: ${outputPath}/enrichment/semantic-assets.json`,
        "Run enrich after graph or API-aware graph workflows."
      ];
    case "llm-pack":
      return [
        "LLM pack artifacts were not found.",
        `Expected: ${outputPath}/llm-pack/reasoning-queue.json`,
        "Run llm-pack after review artifacts exist."
      ];
    case "codex-insights":
      return [
        "Codex insight artifacts were not found.",
        `Expected: ${outputPath}/codex-insights/codex-summary.json`,
        "Run codex-run in plan-only mode first, then execute only if appropriate."
      ];
    case "codex-review":
      return [
        "Codex review artifacts were not found.",
        `Expected: ${outputPath}/codex-review/codex-review-queue.json`,
        "Run codex-review after codex-insights exists."
      ];
  }
}

export function findAssetCard(
  assetCards: AssetCardLike[] | undefined,
  asset: string
): AssetCardLike | null {
  if (!assetCards?.length) {
    return null;
  }

  const assetKey = normalizeAsset(asset);
  return (
    assetCards.find((card) => normalizeAsset(card.asset) === assetKey) ??
    assetCards.find((card) => normalizeAsset(card.path).includes(assetKey)) ??
    null
  );
}

export function getLinkedCodexDetails(
  asset: string,
  codexSummary?: CodexSummaryLike,
  codexReview?: CodexReviewLike
): {
  insightResults: Array<Record<string, unknown>>;
  reviewItems: Array<Record<string, unknown>>;
  reviewWarnings: Array<Record<string, unknown>>;
} {
  const assetKey = normalizeAsset(asset);
  const insightResults = (codexSummary?.results ?? []).filter((item) =>
    normalizeAsset(String(item.asset ?? "")).includes(assetKey)
  );
  const reviewItems = (codexReview?.items ?? []).filter((item) =>
    normalizeAsset(String(item.asset ?? "")).includes(assetKey)
  );
  const reviewWarnings = [
    ...(codexReview?.unsupportedClaims ?? []),
    ...(codexReview?.evidenceGaps ?? []),
    ...(codexReview?.wordingWarnings ?? [])
  ].filter((item) => normalizeAsset(String(item.asset ?? "")).includes(assetKey));

  return { insightResults, reviewItems, reviewWarnings };
}

export function groupValidation(validation?: ValidationReportLike): {
  warnings: string[];
  errors: string[];
} {
  return {
    warnings: validation?.warnings ?? [],
    errors: validation?.errors ?? []
  };
}

export function sortAuditTimeline(events: AuditEventLike[]): AuditEventLike[] {
  return [...events].sort((left, right) =>
    String(right.timestamp ?? "").localeCompare(String(left.timestamp ?? ""))
  );
}

export function assetTitle(asset: string): string {
  return asset.replace(/[-_]/g, ".");
}

function normalizeAsset(value: string): string {
  return value.trim().toLowerCase().replace(/[^a-z0-9]+/g, ".");
}
