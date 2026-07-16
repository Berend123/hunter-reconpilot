import { describe, expect, it } from "vitest";

import { mockWorkspaceSnapshot } from "../mock";
import {
  explainMissingArtifacts,
  findAssetCard,
  friendlyWorkspaceStatus,
  getLinkedCodexDetails
} from "./workspace";

describe("workspace helpers", () => {
  it("finds a matching asset card", () => {
    const snapshot = mockWorkspaceSnapshot();
    const card = findAssetCard(snapshot.assetCards, "auth.example.com");
    expect(card?.markdown).toContain("# auth.example.com");
  });

  it("builds friendly missing artifact explanations", () => {
    const snapshot = mockWorkspaceSnapshot();
    const messages = explainMissingArtifacts(snapshot, "review");
    expect(messages[0]).toContain("Review artifacts");
    expect(messages[1]).toContain("priority-queue.json");
  });

  it("summarizes workspace health safely", () => {
    const snapshot = mockWorkspaceSnapshot();
    const status = friendlyWorkspaceStatus(snapshot.workspaceHealth);
    expect(status.label).toBe("Healthy");
    expect(status.tone).toBe("safe");
  });

  it("links codex details by asset", () => {
    const snapshot = mockWorkspaceSnapshot();
    const details = getLinkedCodexDetails(
      "auth.example.com",
      snapshot.codexSummary,
      snapshot.codexReview
    );

    expect(details.insightResults.length).toBeGreaterThan(0);
    expect(details.reviewWarnings.length).toBeGreaterThan(0);
  });
});
