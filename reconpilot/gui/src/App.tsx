import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  browserRuntimeLabel,
  listProfiles,
  loadGuiConfig,
  loadWorkspaceSnapshot,
  runGuiCommand,
  saveGuiConfig
} from "./lib/backend";
import {
  createCodexPreview,
  createPipelinePreview,
  createStaticPreview,
  inspectGuiConfig
} from "./lib/commanding";
import { parseMarkdownBlocks } from "./lib/markdown";
import { redactText, redactUnknown } from "./lib/redaction";
import { collectReviewFacets, filterReviewItems, type ReviewFilters } from "./lib/review";
import {
  assetTitle,
  explainMissingArtifacts,
  findAssetCard,
  friendlyWorkspaceStatus,
  getLinkedCodexDetails,
  groupValidation,
  sortAuditTimeline
} from "./lib/workspace";
import { BUILTIN_PROFILES, DEFAULT_GUI_CONFIG } from "./mock";
import type {
  AppMode,
  AssetCardLike,
  AuditEventLike,
  CommandPreview,
  CustomProfile,
  GuiConfig,
  GuiCommandRequest,
  GuiCommandResult,
  ReviewItemLike,
  ScreenDefinition,
  ScreenId,
  WorkspaceHealthCheck,
  WorkspaceSnapshot
} from "./types";

const DEFAULT_WORKSPACE = ".";

const SCREEN_DEFINITIONS: ScreenDefinition[] = [
  { id: "dashboard", label: "Dashboard", category: "workspace" },
  { id: "workspace", label: "Workspace Selector", category: "workspace" },
  { id: "scope", label: "Scope Manager", category: "workspace" },
  { id: "pipeline", label: "Pipeline Runner", category: "control" },
  { id: "profiles", label: "Profile Editor", category: "control" },
  { id: "tools", label: "Tool Settings", category: "control" },
  { id: "review", label: "Review Queue", category: "analysis" },
  { id: "asset-detail", label: "Asset Detail", category: "analysis" },
  { id: "graph", label: "Graph Viewer", category: "analysis" },
  { id: "api-intel", label: "API Intelligence", category: "analysis" },
  { id: "enrichment", label: "Enrichment Viewer", category: "analysis" },
  { id: "llm-pack", label: "LLM Pack Viewer", category: "reasoning" },
  { id: "codex-runner", label: "Codex Runner", category: "reasoning" },
  { id: "codex-insights", label: "Codex Insights", category: "reasoning" },
  { id: "codex-review", label: "Codex Review", category: "reasoning" },
  { id: "validation", label: "Validation / Audit", category: "workspace" },
  { id: "settings", label: "Settings", category: "control" }
];

const TOOL_NAMES = [
  "subfinder",
  "httpx",
  "katana",
  "gau",
  "dnsx",
  "gowitness",
  "WhatWeb",
  "codex"
];

const DEFAULT_REVIEW_FILTERS: ReviewFilters = {
  search: "",
  riskLevel: "all",
  role: "all",
  environment: "all",
  sortBy: "rank"
};

export default function App() {
  const [workspaceInput, setWorkspaceInput] = useState(DEFAULT_WORKSPACE);
  const [workspacePath, setWorkspacePath] = useState(DEFAULT_WORKSPACE);
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot | null>(null);
  const [config, setConfig] = useState<GuiConfig>(DEFAULT_GUI_CONFIG);
  const [selectedScreen, setSelectedScreen] = useState<ScreenId>("dashboard");
  const [selectedAsset, setSelectedAsset] = useState("auth.example.com");
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [lastResult, setLastResult] = useState<GuiCommandResult | null>(null);
  const [uiError, setUiError] = useState<string | null>(null);

  const [pipelineScopePath, setPipelineScopePath] = useState("config/scope.example.txt");
  const [pipelineOutDir, setPipelineOutDir] = useState("output/");
  const [pipelineProfileName, setPipelineProfileName] = useState("passive");
  const [pipelineExecute, setPipelineExecute] = useState(false);
  const [pipelineIncludeCodex, setPipelineIncludeCodex] = useState(false);
  const [pipelineExecuteCodex, setPipelineExecuteCodex] = useState(false);
  const [pipelineTargetAck, setPipelineTargetAck] = useState(false);
  const [pipelineCodexAck, setPipelineCodexAck] = useState(false);

  const [customProfileName, setCustomProfileName] = useState("");
  const [customProfileDescription, setCustomProfileDescription] = useState("");
  const [customProfilePhases, setCustomProfilePhases] = useState(
    "run, graph, review, validate"
  );
  const [customProfileTargetContact, setCustomProfileTargetContact] = useState(false);

  const [codexPackPath, setCodexPackPath] = useState("output/llm-pack/");
  const [codexOutDir, setCodexOutDir] = useState("output/codex-insights/");
  const [codexLimit, setCodexLimit] = useState(3);
  const [codexTemplate, setCodexTemplate] = useState("asset_triage_prompt");
  const [codexExecute, setCodexExecute] = useState(false);
  const [codexExecutionAck, setCodexExecutionAck] = useState(false);

  const [reviewFilters, setReviewFilters] = useState<ReviewFilters>(
    DEFAULT_REVIEW_FILTERS
  );

  const profiles = useMemo(() => listProfiles(config), [config]);
  const activeProfile =
    profiles.find((profile) => profile.name === pipelineProfileName) ??
    profiles[0] ??
    BUILTIN_PROFILES[0];
  const runtimeLabel = browserRuntimeLabel();
  const advancedMode = config.mode === "advanced";
  const configWarnings = useMemo(() => inspectGuiConfig(config), [config]);

  const reviewItems = snapshot?.reviewQueue?.items ?? [];
  const reviewFacets = useMemo(() => collectReviewFacets(reviewItems), [reviewItems]);
  const filteredReviewItems = useMemo(
    () => filterReviewItems(reviewItems, reviewFilters),
    [reviewItems, reviewFilters]
  );
  const activeReviewItem =
    reviewItems.find((item) => item.asset === selectedAsset) ?? reviewItems[0] ?? null;

  const pipelinePreview = useMemo(
    () =>
      createPipelinePreview({
        scopePath: pipelineScopePath,
        outDir: pipelineOutDir,
        profile: activeProfile,
        execute: pipelineExecute,
        includeCodex: pipelineIncludeCodex,
        executeCodex: pipelineExecuteCodex,
        mode: config.mode
      }),
    [
      activeProfile,
      config.mode,
      pipelineExecute,
      pipelineIncludeCodex,
      pipelineExecuteCodex,
      pipelineOutDir,
      pipelineScopePath
    ]
  );

  const codexPreview = useMemo(
    () =>
      createCodexPreview({
        packDir: codexPackPath,
        outDir: codexOutDir,
        executeCodex: codexExecute,
        limit: codexLimit,
        template: codexTemplate
      }),
    [codexExecute, codexLimit, codexOutDir, codexPackPath, codexTemplate]
  );

  useEffect(() => {
    void refreshWorkspace(workspacePath);
    void hydrateConfig(workspacePath);
  }, [workspacePath]);

  useEffect(() => {
    if (!activeReviewItem && filteredReviewItems[0]) {
      setSelectedAsset(filteredReviewItems[0].asset);
    }
  }, [activeReviewItem, filteredReviewItems]);

  async function hydrateConfig(nextWorkspacePath: string) {
    try {
      const loaded = await loadGuiConfig(nextWorkspacePath);
      setConfig(loaded);
      if (loaded.rememberedWorkspace) {
        setWorkspaceInput(loaded.rememberedWorkspace);
      }
    } catch (error) {
      setUiError(`Failed to load GUI config: ${toErrorMessage(error)}`);
    }
  }

  async function refreshWorkspace(nextWorkspacePath: string) {
    setLoading(true);
    setUiError(null);
    try {
      const nextSnapshot = await loadWorkspaceSnapshot(nextWorkspacePath);
      setSnapshot(nextSnapshot);
      const firstAsset = nextSnapshot.reviewQueue?.items?.[0]?.asset;
      if (firstAsset) {
        setSelectedAsset(firstAsset);
      }
    } catch (error) {
      setUiError(`Failed to load workspace artifacts: ${toErrorMessage(error)}`);
    } finally {
      setLoading(false);
    }
  }

  async function persistConfig(nextConfig: GuiConfig) {
    setConfig(nextConfig);
    try {
      await saveGuiConfig(workspacePath, nextConfig);
    } catch (error) {
      setUiError(`Failed to save GUI config: ${toErrorMessage(error)}`);
    }
  }

  async function updateAcknowledgement(
    key: keyof GuiConfig["acknowledgements"],
    value: boolean
  ) {
    await persistConfig({
      ...config,
      acknowledgements: {
        ...config.acknowledgements,
        [key]: value
      }
    });
  }

  async function handleOpenWorkspace() {
    const trimmed = workspaceInput.trim();
    if (!trimmed) {
      setUiError("Workspace path cannot be empty.");
      return;
    }

    setWorkspacePath(trimmed);
    await persistConfig({
      ...config,
      rememberedWorkspace: trimmed
    });
  }

  async function handleRunCommand(request: GuiCommandRequest, preview: CommandPreview) {
    if (preview.blockedReasons.length > 0) {
      setUiError(preview.blockedReasons.join(" | "));
      return;
    }

    setBusy(true);
    setUiError(null);
    try {
      const result = await runGuiCommand(request, preview);
      setLastResult(result);
      await refreshWorkspace(workspacePath);
    } catch (error) {
      setUiError(`Command failed: ${toErrorMessage(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function handlePipelineRun() {
    await handleRunCommand(
      {
        workspacePath,
        kind: "pipeline",
        scopePath: pipelineScopePath,
        profileName: activeProfile.name,
        outDir: pipelineOutDir,
        execute: pipelineExecute,
        includeCodex: pipelineIncludeCodex,
        executeCodex: pipelineExecuteCodex
      },
      pipelinePreview
    );
  }

  async function handleCodexRun() {
    await handleRunCommand(
      {
        workspacePath,
        kind: "codex-run",
        packDir: codexPackPath,
        outDir: codexOutDir,
        executeCodex: codexExecute,
        limit: codexLimit,
        template: codexTemplate
      },
      codexPreview
    );
  }

  async function handleValidate() {
    await handleRunCommand(
      {
        workspacePath,
        kind: "validate",
        inputDir: "output/"
      },
      createStaticPreview(
        "reconpilot validate --input output/",
        true,
        ["Validate local outputs only."],
        ["validate"]
      )
    );
  }

  async function handleCodexReview() {
    await handleRunCommand(
      {
        workspacePath,
        kind: "codex-review",
        inputDir: "output/codex-insights/",
        outDir: "output/codex-review/"
      },
      createStaticPreview(
        "reconpilot codex-review --input output/codex-insights/ --out output/codex-review/",
        true,
        ["Review Codex outputs locally without modifying them."],
        ["codex-review"]
      )
    );
  }

  async function handleDoctor() {
    await handleRunCommand(
      {
        workspacePath,
        kind: "doctor"
      },
      createStaticPreview(
        "reconpilot doctor",
        true,
        ["Doctor is local-only and safe to run anytime."],
        ["doctor"]
      )
    );
  }

  async function addCustomProfile() {
    if (!advancedMode) {
      setUiError("Custom profiles are available only in Advanced Mode.");
      return;
    }

    const name = customProfileName.trim();
    const phases = customProfilePhases
      .split(",")
      .map((phase) => phase.trim())
      .filter(Boolean);

    if (!name) {
      setUiError("Custom profile name cannot be empty.");
      return;
    }
    if (phases.length === 0) {
      setUiError("Custom profile phases cannot be empty.");
      return;
    }
    if (profiles.some((profile) => profile.name === name)) {
      setUiError(`A profile named '${name}' already exists.`);
      return;
    }

    await persistConfig({
      ...config,
      customProfiles: [
        ...config.customProfiles,
        {
          name,
          description: customProfileDescription.trim() || "User-defined profile",
          phases,
          allowTargetContact: customProfileTargetContact,
          notes: ["User-defined profile from Advanced Mode."]
        }
      ]
    });

    setCustomProfileName("");
    setCustomProfileDescription("");
    setCustomProfilePhases("run, graph, review, validate");
    setCustomProfileTargetContact(false);
  }

  async function updateToolArg(tool: string, rawArgs: string) {
    if (!advancedMode) {
      return;
    }

    const args = rawArgs
      .split(/\s+/)
      .map((value) => value.trim())
      .filter(Boolean);

    const nextArgs = config.customToolArgs.filter((entry) => entry.tool !== tool);
    nextArgs.push({ tool, args });
    await persistConfig({
      ...config,
      customToolArgs: nextArgs
    });
  }

  async function updateMode(nextMode: AppMode) {
    await persistConfig({
      ...config,
      mode: nextMode,
      acknowledgements: {
        ...config.acknowledgements,
        advancedModeUnderstood:
          nextMode === "advanced"
            ? config.acknowledgements.advancedModeUnderstood
            : false
      }
    });
  }

  const headerWarnings = [
    ...(snapshot?.warnings ?? []),
    ...configWarnings,
    ...(snapshot?.validation?.warnings ?? [])
  ];

  return (
    <div className="app-shell">
      <Sidebar
        current={selectedScreen}
        screens={SCREEN_DEFINITIONS}
        onSelect={setSelectedScreen}
      />
      <main className="app-main">
        <HeaderBar
          mode={config.mode}
          workspacePath={workspacePath}
          runtimeLabel={runtimeLabel}
          manifest={snapshot?.manifest}
          onOpenReview={() => setSelectedScreen("review")}
          onOpenPipeline={() => setSelectedScreen("pipeline")}
          onOpenValidation={() => setSelectedScreen("validation")}
        />

        {uiError ? <Banner tone="danger" title="GUI Error" items={[uiError]} /> : null}
        {headerWarnings.length > 0 ? (
          <Banner tone="warning" title="Warnings" items={headerWarnings.slice(0, 6)} />
        ) : null}
        {lastResult ? (
          <Panel
            title={lastResult.success ? "Last command result" : "Last command failure"}
            subtitle={
              lastResult.success
                ? "Most recent GUI-triggered ReconPilot command."
                : "Review the exact command and output before retrying."
            }
          >
            <ResultView result={lastResult} redactionEnabled={config.redactionEnabled} />
          </Panel>
        ) : null}

        {loading || snapshot === null ? (
          <Panel
            title="Loading workspace"
            subtitle="Reading local artifacts and GUI settings."
          >
            <p className="muted">Workspace: {workspacePath}</p>
          </Panel>
        ) : (
          <div className="screen-grid">{renderScreen(snapshot)}</div>
        )}
      </main>
    </div>
  );

  function renderScreen(currentSnapshot: WorkspaceSnapshot) {
    switch (selectedScreen) {
      case "dashboard":
        return renderDashboard(currentSnapshot);
      case "workspace":
        return renderWorkspace(currentSnapshot);
      case "scope":
        return renderScope(currentSnapshot);
      case "pipeline":
        return renderPipeline(currentSnapshot);
      case "profiles":
        return renderProfiles();
      case "tools":
        return renderTools();
      case "review":
        return renderReview(currentSnapshot);
      case "asset-detail":
        return renderAssetDetail(currentSnapshot);
      case "graph":
        return renderGraph(currentSnapshot);
      case "api-intel":
        return renderApiIntel(currentSnapshot);
      case "enrichment":
        return renderEnrichment(currentSnapshot);
      case "llm-pack":
        return renderLlmPack(currentSnapshot);
      case "codex-runner":
        return renderCodexRunner();
      case "codex-insights":
        return renderCodexInsights(currentSnapshot);
      case "codex-review":
        return renderCodexReview(currentSnapshot);
      case "validation":
        return renderValidation(currentSnapshot);
      case "settings":
        return renderSettings();
      default:
        return null;
    }
  }

  function renderDashboard(currentSnapshot: WorkspaceSnapshot) {
    const workspaceStatus = friendlyWorkspaceStatus(currentSnapshot.workspaceHealth);
    const validationGroups = groupValidation(currentSnapshot.validation);
    return (
      <>
        <section className="grid-three">
          <MetricCard
            label="Workspace"
            value={currentSnapshot.rootPath.split("/").pop() || "reconpilot"}
            hint={workspaceStatus.label}
          />
          <MetricCard
            label="Validation"
            value={currentSnapshot.validation?.success ? "Healthy" : "Attention"}
            hint={`${validationGroups.warnings.length} warning(s), ${validationGroups.errors.length} error(s)`}
          />
          <MetricCard
            label="Review Targets"
            value={`${reviewItems.length}`}
            hint={`${currentSnapshot.codexSummary?.planned_count ?? 0} Codex item(s) planned`}
          />
        </section>

        <Panel
          title="Run State"
          subtitle="Current reproducibility, workspace health, and safety summary."
        >
          <div className="two-column">
            <KeyValueGrid
              items={[
                ["Mode", config.mode],
                ["Workspace health", workspaceStatus.label],
                ["Scope hash", currentSnapshot.manifest?.scope_file_hash ?? "not available"],
                ["Config hash", currentSnapshot.manifest?.config_hash ?? "not available"],
                [
                  "Last command",
                  currentSnapshot.manifest?.command ?? "No manifest command recorded yet."
                ]
              ]}
            />
            <div className="stack">
              <h4>Quick actions</h4>
              <div className="button-row">
                <button className="button" onClick={() => void handleDoctor()} disabled={busy}>
                  Run Doctor
                </button>
                <button className="button" onClick={() => void handleValidate()} disabled={busy}>
                  Validate Outputs
                </button>
                <button
                  className="button button-ghost"
                  onClick={() => setSelectedScreen("pipeline")}
                >
                  Open Pipeline Runner
                </button>
              </div>
              <p className="muted">
                Validation errors should be reviewed before Codex reasoning is executed.
              </p>
            </div>
          </div>
        </Panel>

        <Panel
          title="Top Review Targets"
          subtitle="Cautious analyst queue from local artifacts."
        >
          <ReviewList
            items={filteredReviewItems.slice(0, 3)}
            onSelect={(asset) => {
              setSelectedAsset(asset);
              setSelectedScreen("asset-detail");
            }}
          />
        </Panel>

        <Panel
          title="Workspace Messages"
          subtitle="Detection and health messages from the workspace loader."
        >
          <ul className="plain-list compact">
            {currentSnapshot.workspaceHealth.messages.map((message) => (
              <li key={message}>{message}</li>
            ))}
          </ul>
        </Panel>

        <Panel
          title="Recent Audit Activity"
          subtitle="Manifest- and audit-driven visibility."
        >
          <AuditTimeline events={sortAuditTimeline(currentSnapshot.auditEvents).slice(0, 6)} />
        </Panel>
      </>
    );
  }

  function renderWorkspace(currentSnapshot: WorkspaceSnapshot) {
    const workspaceStatus = friendlyWorkspaceStatus(currentSnapshot.workspaceHealth);
    return (
      <>
        <Panel
          title="Workspace Selector"
          subtitle="Open a ReconPilot project root, output directory, config directory, or docs directory."
        >
          <div className="field-grid">
            <label className="field-span-full">
              Workspace path
              <input
                value={workspaceInput}
                onChange={(event) => setWorkspaceInput(event.target.value)}
                placeholder="C:/path/to/reconpilot or C:/path/to/reconpilot/output"
              />
            </label>
          </div>
          <div className="button-row">
            <button className="button" onClick={() => void handleOpenWorkspace()}>
              Open Workspace
            </button>
            <button
              className="button button-ghost"
              onClick={() => void refreshWorkspace(workspacePath)}
            >
              Refresh Artifacts
            </button>
          </div>
        </Panel>

        <Panel
          title="Workspace Health"
          subtitle="Project root detection, auto-resolved directories, and artifact availability."
        >
          <div className="two-column">
            <KeyValueGrid
              items={[
                ["Status", workspaceStatus.label],
                ["Detected from", currentSnapshot.workspaceHealth.detectedFrom],
                ["Root path", currentSnapshot.workspaceHealth.rootPath],
                ["Output path", currentSnapshot.workspaceHealth.outputPath],
                ["Config path", currentSnapshot.workspaceHealth.configPath],
                ["Docs path", currentSnapshot.workspaceHealth.docsPath]
              ]}
            />
            <ArtifactCheckList checks={currentSnapshot.workspaceHealth.checks} />
          </div>
        </Panel>
      </>
    );
  }

  function renderScope(currentSnapshot: WorkspaceSnapshot) {
    return (
      <Panel
        title="Scope Manager"
        subtitle="Scope-first review before any target-touching action."
      >
        <div className="two-column">
          <div>
            <h4>Scope file</h4>
            <pre className="code-block">
              {currentSnapshot.scopeText ?? "No scope file loaded."}
            </pre>
          </div>
          <div>
            <h4>Exclusions</h4>
            <pre className="code-block">
              {currentSnapshot.exclusionText ?? "No exclusion file loaded."}
            </pre>
          </div>
        </div>
        <Banner
          tone="info"
          title="Safety"
          items={[
            "A scope file remains mandatory for target-touching phases.",
            "Empty or inconsistent scope should block execution.",
            `Scope hash: ${currentSnapshot.manifest?.scope_file_hash ?? "not available"}`
          ]}
        />
      </Panel>
    );
  }

  function renderPipeline(currentSnapshot: WorkspaceSnapshot) {
    const pipelineHistory = sortAuditTimeline([
      ...(currentSnapshot.guiExecutionLog ?? []),
      ...currentSnapshot.auditEvents
    ]).filter(
      (event) =>
        event.phase === "pipeline" ||
        String(event.message ?? "").includes("reconpilot pipeline")
    );

    return (
      <>
        <Panel
          title="Pipeline Runner"
          subtitle="CLI-first orchestration with exact command preview, phase preview, and separate Codex controls."
        >
          <div className="field-grid">
            <label>
              Scope file
              <input
                value={pipelineScopePath}
                onChange={(event) => setPipelineScopePath(event.target.value)}
              />
            </label>
            <label>
              Output directory
              <input
                value={pipelineOutDir}
                onChange={(event) => setPipelineOutDir(event.target.value)}
              />
            </label>
            <label>
              Profile
              <select
                value={pipelineProfileName}
                onChange={(event) => setPipelineProfileName(event.target.value)}
              >
                {profiles.map((profile) => (
                  <option key={profile.name} value={profile.name}>
                    {profile.name}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div className="mode-badges">
            <StatusPill tone="safe">
              {pipelinePreview.dryRun ? "Dry-run default" : "Execution requested"}
            </StatusPill>
            <StatusPill tone={pipelinePreview.targetTouching ? "warning" : "safe"}>
              {pipelinePreview.targetTouching ? "Target-touching capable" : "Local-only / planning"}
            </StatusPill>
            <StatusPill tone={pipelineIncludeCodex ? "warning" : "safe"}>
              {pipelineIncludeCodex
                ? pipelineExecuteCodex
                  ? "Codex execute requested"
                  : "Codex plan-only"
                : "No Codex phase"}
            </StatusPill>
          </div>

          <div className="toggle-grid">
            <Toggle
              label="Execute external tool phases"
              checked={pipelineExecute}
              onChange={setPipelineExecute}
              help="Normal --execute controls target-touching external phases only."
            />
            <Toggle
              label="Include Codex in pipeline"
              checked={pipelineIncludeCodex}
              onChange={setPipelineIncludeCodex}
              help="Adds codex-run after llm-pack."
            />
            <Toggle
              label="Execute Codex"
              checked={pipelineExecuteCodex}
              onChange={setPipelineExecuteCodex}
              help="Separate from --execute. Never implied."
            />
          </div>

          {pipelineExecute && activeProfile.allowTargetContact ? (
            <label className="checkbox">
              <input
                type="checkbox"
                checked={
                  pipelineTargetAck ||
                  (advancedMode && config.acknowledgements.targetContactUnderstood)
                }
                onChange={(event) => {
                  setPipelineTargetAck(event.target.checked);
                  if (advancedMode && event.target.checked) {
                    void updateAcknowledgement("targetContactUnderstood", true);
                  }
                }}
              />
              I understand this contacts in-scope targets and must remain authorized.
            </label>
          ) : null}

          {pipelineIncludeCodex ? (
            <label className="checkbox">
              <input
                type="checkbox"
                checked={
                  pipelineCodexAck ||
                  (advancedMode && config.acknowledgements.codexExecutionUnderstood)
                }
                onChange={(event) => {
                  setPipelineCodexAck(event.target.checked);
                  if (advancedMode && event.target.checked) {
                    void updateAcknowledgement("codexExecutionUnderstood", true);
                  }
                }}
              />
              I understand Codex reasoning is hypothesis-only and requires validation.
            </label>
          ) : null}

          <CommandPreviewCard preview={pipelinePreview} />

          <div className="button-row">
            <button
              className="button"
              disabled={
                busy ||
                pipelinePreview.blockedReasons.length > 0 ||
                (pipelineExecute &&
                  activeProfile.allowTargetContact &&
                  !pipelineTargetAck &&
                  !(advancedMode && config.acknowledgements.targetContactUnderstood)) ||
                (pipelineExecuteCodex &&
                  (!pipelineIncludeCodex ||
                    (!pipelineCodexAck &&
                      !(advancedMode &&
                        config.acknowledgements.codexExecutionUnderstood))))
              }
              onClick={() => void handlePipelineRun()}
            >
              {pipelineExecute || pipelineExecuteCodex
                ? "Run Pipeline"
                : "Generate Plan / Local Outputs"}
            </button>
            <button
              className="button button-ghost"
              onClick={() => void handleValidate()}
              disabled={busy}
            >
              Validate After Run
            </button>
          </div>
        </Panel>

        <Panel title="Profile Preview" subtitle={activeProfile.description}>
          <div className="stack">
            <ul className="plain-list compact">
              {activeProfile.notes.map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
            <div className="phase-preview-grid">
              {pipelinePreview.phasePreview.map((phase, index) => (
                <div key={`${phase}-${index}`} className="phase-chip">
                  <span className="phase-index">{index + 1}</span>
                  <span>{phase}</span>
                </div>
              ))}
            </div>
          </div>
        </Panel>

        <Panel title="Execution History" subtitle="Recent pipeline-related audit and GUI execution activity.">
          <AuditTimeline events={pipelineHistory.slice(0, 8)} />
        </Panel>
      </>
    );
  }

  function renderProfiles() {
    return (
      <>
        <Panel
          title="Profile Editor"
          subtitle="Built-in profiles are fixed. Advanced Mode may add user-defined profiles for safe previewing."
        >
          <div className="cards-grid">
            {profiles.map((profile) => (
              <article key={profile.name} className="list-card">
                <div className="list-card-header">
                  <strong>{profile.name}</strong>
                  <StatusPill tone={profile.allowTargetContact ? "warning" : "safe"}>
                    {profile.allowTargetContact
                      ? "target-touching capable"
                      : "local / dry-run first"}
                  </StatusPill>
                </div>
                <p>{profile.description}</p>
                <ul className="plain-list compact">
                  {profile.phases.map((phase) => (
                    <li key={phase}>{phase}</li>
                  ))}
                </ul>
              </article>
            ))}
          </div>
        </Panel>

        <Panel
          title="Add Custom Profile"
          subtitle="Advanced Mode only. Custom profiles are saved to GUI config, not the core CLI."
        >
          {!advancedMode ? (
            <Banner
              tone="warning"
              title="Advanced Mode Required"
              items={[
                "Beginner Mode keeps profile editing disabled.",
                "Switch modes in Settings after acknowledging the advanced workflow."
              ]}
            />
          ) : (
            <>
              <div className="field-grid">
                <label>
                  Profile name
                  <input
                    value={customProfileName}
                    onChange={(event) => setCustomProfileName(event.target.value)}
                  />
                </label>
                <label>
                  Description
                  <input
                    value={customProfileDescription}
                    onChange={(event) =>
                      setCustomProfileDescription(event.target.value)
                    }
                  />
                </label>
                <label className="field-span-full">
                  Phases (comma separated)
                  <input
                    value={customProfilePhases}
                    onChange={(event) => setCustomProfilePhases(event.target.value)}
                  />
                </label>
              </div>
              <label className="checkbox">
                <input
                  type="checkbox"
                  checked={customProfileTargetContact}
                  onChange={(event) =>
                    setCustomProfileTargetContact(event.target.checked)
                  }
                />
                This profile may contact in-scope targets when used with --execute.
              </label>
              <button className="button" onClick={() => void addCustomProfile()}>
                Save Custom Profile
              </button>
            </>
          )}
        </Panel>
      </>
    );
  }

  function renderTools() {
    return (
      <>
        <Panel
          title="Tool Settings"
          subtitle="Advanced Mode can store controlled custom tool args and rate/concurrency placeholders."
        >
          <div className="field-grid">
            <label>
              HTTP requests / second
              <input
                type="number"
                min={1}
                value={config.rateLimits.httpRequestsPerSecond}
                onChange={(event) =>
                  void persistConfig({
                    ...config,
                    rateLimits: {
                      ...config.rateLimits,
                      httpRequestsPerSecond: Number(event.target.value)
                    }
                  })
                }
                disabled={!advancedMode}
              />
            </label>
            <label>
              DNS queries / second
              <input
                type="number"
                min={1}
                value={config.rateLimits.dnsQueriesPerSecond}
                onChange={(event) =>
                  void persistConfig({
                    ...config,
                    rateLimits: {
                      ...config.rateLimits,
                      dnsQueriesPerSecond: Number(event.target.value)
                    }
                  })
                }
                disabled={!advancedMode}
              />
            </label>
            <label>
              Screenshot concurrency
              <input
                type="number"
                min={1}
                value={config.rateLimits.screenshotConcurrency}
                onChange={(event) =>
                  void persistConfig({
                    ...config,
                    rateLimits: {
                      ...config.rateLimits,
                      screenshotConcurrency: Number(event.target.value)
                    }
                  })
                }
                disabled={!advancedMode}
              />
            </label>
          </div>
          <div className="cards-grid">
            {TOOL_NAMES.map((tool) => {
              const rawValue = (
                config.customToolArgs.find((entry) => entry.tool === tool)?.args ?? []
              ).join(" ");
              return (
                <article key={tool} className="list-card">
                  <strong>{tool}</strong>
                  <textarea
                    value={rawValue}
                    onChange={(event) => void updateToolArg(tool, event.target.value)}
                    placeholder="Stored for safe previewing in Advanced Mode."
                    disabled={!advancedMode}
                  />
                </article>
              );
            })}
          </div>
        </Panel>
        {configWarnings.length > 0 ? (
          <Banner
            tone="warning"
            title="Config Risk Warnings"
            items={configWarnings}
          />
        ) : null}
      </>
    );
  }

  function renderReview(currentSnapshot: WorkspaceSnapshot) {
    if (reviewItems.length === 0) {
      return (
        <Panel
          title="Review Queue"
          subtitle="Analyst-facing prioritization view with evidence references and cautious language."
        >
          <EmptyState title="Review queue not found" items={explainMissingArtifacts(currentSnapshot, "review")} />
        </Panel>
      );
    }

    return (
      <Panel
        title="Review Queue"
        subtitle="Filter by risk, role, environment, search, and sort order."
      >
        <div className="filter-grid">
          <label>
            Search asset
            <input
              value={reviewFilters.search}
              onChange={(event) =>
                setReviewFilters((current) => ({
                  ...current,
                  search: event.target.value
                }))
              }
              placeholder="auth.example.com"
            />
          </label>
          <label>
            Risk
            <select
              value={reviewFilters.riskLevel}
              onChange={(event) =>
                setReviewFilters((current) => ({
                  ...current,
                  riskLevel: event.target.value
                }))
              }
            >
              <option value="all">all</option>
              {reviewFacets.riskLevels.map((risk) => (
                <option key={risk} value={risk}>
                  {risk}
                </option>
              ))}
            </select>
          </label>
          <label>
            Role
            <select
              value={reviewFilters.role}
              onChange={(event) =>
                setReviewFilters((current) => ({
                  ...current,
                  role: event.target.value
                }))
              }
            >
              <option value="all">all</option>
              {reviewFacets.roles.map((role) => (
                <option key={role} value={role}>
                  {role}
                </option>
              ))}
            </select>
          </label>
          <label>
            Environment
            <select
              value={reviewFilters.environment}
              onChange={(event) =>
                setReviewFilters((current) => ({
                  ...current,
                  environment: event.target.value
                }))
              }
            >
              <option value="all">all</option>
              {reviewFacets.environments.map((environment) => (
                <option key={environment} value={environment}>
                  {environment}
                </option>
              ))}
            </select>
          </label>
          <label>
            Sort by
            <select
              value={reviewFilters.sortBy}
              onChange={(event) =>
                setReviewFilters((current) => ({
                  ...current,
                  sortBy: event.target.value as ReviewFilters["sortBy"]
                }))
              }
            >
              <option value="rank">rank</option>
              <option value="score">score</option>
              <option value="confidence">confidence</option>
            </select>
          </label>
        </div>
        <ReviewList
          items={filteredReviewItems}
          detailed
          onSelect={(asset) => {
            setSelectedAsset(asset);
            setSelectedScreen("asset-detail");
          }}
        />
      </Panel>
    );
  }

  function renderAssetDetail(currentSnapshot: WorkspaceSnapshot) {
    if (!activeReviewItem) {
      return (
        <Panel
          title="Asset Detail"
          subtitle="Cross-links semantic, graph, API, and review evidence without claiming vulnerabilities."
        >
          <EmptyState
            title="No asset selected"
            items={["Select an item from the review queue to inspect it in detail."]}
          />
        </Panel>
      );
    }

    return (
      <Panel
        title={`Asset Detail: ${activeReviewItem.asset}`}
        subtitle="Rendered asset card, evidence references, and linked Codex review context."
      >
        <AssetDetailView
          item={activeReviewItem}
          snapshot={currentSnapshot}
          redactionEnabled={config.redactionEnabled}
        />
      </Panel>
    );
  }

  function renderGraph(currentSnapshot: WorkspaceSnapshot) {
    if (!currentSnapshot.graph?.nodes?.length) {
      return (
        <Panel
          title="Graph Viewer"
          subtitle="Relationship-aware view of hosts, shared infrastructure, clusters, and technologies."
        >
          <EmptyState title="Graph artifacts not found" items={explainMissingArtifacts(currentSnapshot, "graph")} />
        </Panel>
      );
    }

    return (
      <Panel
        title="Graph Viewer"
        subtitle="Relationship-aware view of hosts, shared infrastructure, clusters, and technologies."
      >
        <GraphView
          graph={currentSnapshot.graph}
          onSelectAsset={(asset) => {
            setSelectedAsset(asset);
            setSelectedScreen("asset-detail");
          }}
        />
      </Panel>
    );
  }

  function renderApiIntel(currentSnapshot: WorkspaceSnapshot) {
    return (
      <Panel
        title="API Intelligence"
        subtitle="Schemas, auth indicators, JS-derived routes, and object sensitivity."
      >
        {currentSnapshot.apiIntel ? (
          <ApiIntelView
            apiIntel={currentSnapshot.apiIntel}
            redactionEnabled={config.redactionEnabled}
          />
        ) : (
          <EmptyState
            title="API intelligence artifacts not found"
            items={explainMissingArtifacts(currentSnapshot, "api-intel")}
          />
        )}
      </Panel>
    );
  }

  function renderEnrichment(currentSnapshot: WorkspaceSnapshot) {
    return (
      <Panel
        title="Enrichment Viewer"
        subtitle="Deterministic semantic roles, environments, and graph-neighborhood context."
      >
        {currentSnapshot.enrichment ? (
          <EnrichmentView enrichment={currentSnapshot.enrichment} />
        ) : (
          <EmptyState
            title="Enrichment artifacts not found"
            items={explainMissingArtifacts(currentSnapshot, "enrichment")}
          />
        )}
      </Panel>
    );
  }

  function renderLlmPack(currentSnapshot: WorkspaceSnapshot) {
    return (
      <Panel
        title="LLM Pack Viewer"
        subtitle="Model-ready local context bundles and prompt selection preview."
      >
        {currentSnapshot.llmPack ? (
          <LlmPackView llmPack={currentSnapshot.llmPack} />
        ) : (
          <EmptyState
            title="LLM pack artifacts not found"
            items={explainMissingArtifacts(currentSnapshot, "llm-pack")}
          />
        )}
      </Panel>
    );
  }

  function renderCodexRunner() {
    return (
      <Panel
        title="Codex Runner"
        subtitle="Optional local Codex reasoning. Plan-only by default and separate from normal execute."
      >
        <div className="field-grid">
          <label>
            llm-pack path
            <input
              value={codexPackPath}
              onChange={(event) => setCodexPackPath(event.target.value)}
            />
          </label>
          <label>
            Output directory
            <input
              value={codexOutDir}
              onChange={(event) => setCodexOutDir(event.target.value)}
            />
          </label>
          <label>
            Template
            <input
              value={codexTemplate}
              onChange={(event) => setCodexTemplate(event.target.value)}
            />
          </label>
          <label>
            Asset limit
            <input
              type="number"
              min={1}
              max={25}
              value={codexLimit}
              onChange={(event) => setCodexLimit(Number(event.target.value))}
            />
          </label>
        </div>
        <Toggle
          label="Execute Codex"
          checked={codexExecute}
          onChange={setCodexExecute}
          help="Plan-only remains the default."
        />
        {codexExecute ? (
          <label className="checkbox">
            <input
              type="checkbox"
              checked={
                codexExecutionAck ||
                (advancedMode && config.acknowledgements.codexExecutionUnderstood)
              }
              onChange={(event) => {
                setCodexExecutionAck(event.target.checked);
                if (advancedMode && event.target.checked) {
                  void updateAcknowledgement("codexExecutionUnderstood", true);
                }
              }}
            />
            I understand Codex execution is explicit, local-only, and still requires validation review.
          </label>
        ) : null}
        <CommandPreviewCard preview={codexPreview} />
        <div className="button-row">
          <button
            className="button"
            disabled={
              busy ||
              codexPreview.blockedReasons.length > 0 ||
              (codexExecute &&
                !codexExecutionAck &&
                !(advancedMode && config.acknowledgements.codexExecutionUnderstood))
            }
            onClick={() => void handleCodexRun()}
          >
            {codexExecute ? "Run Codex" : "Generate Codex Plan"}
          </button>
          <button
            className="button button-ghost"
            onClick={() => setSelectedScreen("codex-insights")}
          >
            View Codex Insights
          </button>
        </div>
      </Panel>
    );
  }

  function renderCodexInsights(currentSnapshot: WorkspaceSnapshot) {
    return (
      <Panel
        title="Codex Insights"
        subtitle="Plan-only vs executed reasoning outputs with safety context."
      >
        {currentSnapshot.codexSummary ? (
          <CodexInsightsView
            codexSummary={currentSnapshot.codexSummary}
            redactionEnabled={config.redactionEnabled}
          />
        ) : (
          <EmptyState
            title="Codex insight artifacts not found"
            items={explainMissingArtifacts(currentSnapshot, "codex-insights")}
          />
        )}
      </Panel>
    );
  }

  function renderCodexReview(currentSnapshot: WorkspaceSnapshot) {
    return (
      <>
        <Panel
          title="Codex Review"
          subtitle="Flag unsupported claims, evidence gaps, and unsafe recommendations without rewriting results."
        >
          {currentSnapshot.codexReview ? (
            <CodexReviewView codexReview={currentSnapshot.codexReview} />
          ) : (
            <EmptyState
              title="Codex review artifacts not found"
              items={explainMissingArtifacts(currentSnapshot, "codex-review")}
            />
          )}
        </Panel>
        <div className="button-row">
          <button className="button" onClick={() => void handleCodexReview()} disabled={busy}>
            Refresh Codex Review
          </button>
        </div>
      </>
    );
  }

  function renderValidation(currentSnapshot: WorkspaceSnapshot) {
    return (
      <Panel
        title="Validation / Audit"
        subtitle="Quality control, reproducibility, warnings, errors, and execution history."
      >
        <ValidationView snapshot={currentSnapshot} />
      </Panel>
    );
  }

  function renderSettings() {
    return (
      <Panel
        title="Settings"
        subtitle="Mode, redaction, and acknowledgement preferences."
      >
        <div className="toggle-grid">
          <Toggle
            label="Beginner Mode"
            checked={config.mode === "beginner"}
            onChange={(checked) => {
              if (checked) {
                void updateMode("beginner");
              }
            }}
            help="Stronger warnings, no custom profiles, no custom tool args."
          />
          <Toggle
            label="Advanced Mode"
            checked={config.mode === "advanced"}
            onChange={(checked) => {
              if (checked) {
                void updateMode("advanced");
              }
            }}
            help="Still dry-run first, but allows controlled custom profiles and tool args."
          />
          <Toggle
            label="Redaction enabled"
            checked={config.redactionEnabled}
            onChange={(checked) =>
              void persistConfig({
                ...config,
                redactionEnabled: checked
              })
            }
            help="Masks bearer tokens, JWT-like strings, API keys, and long blobs."
          />
        </div>
        {config.mode === "advanced" ? (
          <label className="checkbox">
            <input
              type="checkbox"
              checked={config.acknowledgements.advancedModeUnderstood}
              onChange={(event) =>
                void updateAcknowledgement(
                  "advancedModeUnderstood",
                  event.target.checked
                )
              }
            />
            I understand Advanced Mode reduces repeated prompts but does not remove scope, preview, logging, or Codex safety requirements.
          </label>
        ) : null}
      </Panel>
    );
  }
}

function Sidebar({
  current,
  screens,
  onSelect
}: {
  current: ScreenId;
  screens: ScreenDefinition[];
  onSelect: (screen: ScreenId) => void;
}) {
  const sections = ["workspace", "analysis", "reasoning", "control"] as const;
  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <span className="eyebrow">ReconPilot</span>
        <h1>Desktop GUI</h1>
        <p>CLI-first, artifact-driven, safety-focused.</p>
      </div>
      {sections.map((section) => (
        <div key={section} className="sidebar-section">
          <span className="sidebar-section-label">{section.replace("-", " ")}</span>
          {screens
            .filter((screen) => screen.category === section)
            .map((screen) => (
              <button
                key={screen.id}
                className={`sidebar-link${screen.id === current ? " active" : ""}`}
                onClick={() => onSelect(screen.id)}
              >
                {screen.label}
              </button>
            ))}
        </div>
      ))}
    </aside>
  );
}

function HeaderBar({
  mode,
  workspacePath,
  runtimeLabel,
  manifest,
  onOpenReview,
  onOpenPipeline,
  onOpenValidation
}: {
  mode: AppMode;
  workspacePath: string;
  runtimeLabel: string;
  manifest?: WorkspaceSnapshot["manifest"];
  onOpenReview: () => void;
  onOpenPipeline: () => void;
  onOpenValidation: () => void;
}) {
  return (
    <header className="header-bar">
      <div>
        <span className="eyebrow">Workspace</span>
        <h2>{workspacePath}</h2>
        <p className="muted">
          Mode: <strong>{mode}</strong> | Runtime: <strong>{runtimeLabel}</strong> |
          Scope hash: <strong>{manifest?.scope_file_hash ?? "not available"}</strong>
        </p>
      </div>
      <div className="header-actions">
        <button className="button button-ghost" onClick={onOpenReview}>
          Review Queue
        </button>
        <button className="button button-ghost" onClick={onOpenPipeline}>
          Pipeline Runner
        </button>
        <button className="button" onClick={onOpenValidation}>
          Validation / Audit
        </button>
      </div>
    </header>
  );
}

function Panel({
  title,
  subtitle,
  children
}: {
  title: string;
  subtitle?: string;
  children: ReactNode;
}) {
  return (
    <section className="panel">
      <div className="panel-header">
        <h3>{title}</h3>
        {subtitle ? <p>{subtitle}</p> : null}
      </div>
      <div className="panel-body">{children}</div>
    </section>
  );
}

function Banner({
  tone,
  title,
  items
}: {
  tone: "info" | "warning" | "danger";
  title: string;
  items: string[];
}) {
  return (
    <div className={`banner ${tone}`}>
      <strong>{title}</strong>
      <ul className="plain-list compact">
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </div>
  );
}

function MetricCard({
  label,
  value,
  hint
}: {
  label: string;
  value: string;
  hint: string;
}) {
  return (
    <article className="metric-card">
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{hint}</small>
    </article>
  );
}

function KeyValueGrid({ items }: { items: Array<[string, string]> }) {
  return (
    <dl className="key-value-grid">
      {items.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function Toggle({
  label,
  checked,
  onChange,
  help
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  help: string;
}) {
  return (
    <label className="toggle">
      <div>
        <strong>{label}</strong>
        <p>{help}</p>
      </div>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  );
}

function StatusPill({
  tone,
  children
}: {
  tone: "safe" | "warning" | "danger";
  children: ReactNode;
}) {
  return <span className={`status-pill ${tone}`}>{children}</span>;
}

function EmptyState({ title, items }: { title: string; items: string[] }) {
  return (
    <div className="empty-state">
      <strong>{title}</strong>
      <ul className="plain-list compact">
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </div>
  );
}

function ArtifactCheckList({ checks }: { checks: WorkspaceHealthCheck[] }) {
  return (
    <div className="stack">
      {checks.map((check) => (
        <article key={check.key} className="artifact-check">
          <div className="list-card-header">
            <strong>{check.label}</strong>
            <StatusPill tone={check.present ? "safe" : check.required ? "danger" : "warning"}>
              {check.present ? "present" : check.required ? "required missing" : "optional missing"}
            </StatusPill>
          </div>
          <p className="muted">{check.path}</p>
          <p>{check.message}</p>
        </article>
      ))}
    </div>
  );
}

function ReviewList({
  items,
  onSelect,
  detailed = false
}: {
  items: ReviewItemLike[];
  onSelect: (asset: string) => void;
  detailed?: boolean;
}) {
  if (items.length === 0) {
    return <p className="muted">No review queue was found yet.</p>;
  }

  return (
    <div className="review-list">
      {items.map((item) => (
        <article key={item.asset} className="list-card">
          <div className="list-card-header">
            <div>
              <strong>
                #{item.rank} | {item.asset}
              </strong>
              <p className="muted">
                {item.risk_level} | score {item.score} | confidence {item.confidence}
              </p>
            </div>
            <button className="button button-ghost" onClick={() => onSelect(item.asset)}>
              Open
            </button>
          </div>
          <p>{item.reasons[0] ?? "No reason recorded."}</p>
          {detailed ? (
            <>
              <div className="pill-row">
                {item.semantic_roles.map((role) => (
                  <span key={role} className="tag-pill">
                    {role}
                  </span>
                ))}
              </div>
              <div className="pill-row">
                {item.environments.map((environment) => (
                  <span key={environment} className="tag-pill muted-pill">
                    {environment}
                  </span>
                ))}
              </div>
              <p className="muted">Evidence: {item.evidence_refs.join(", ") || "None"}</p>
            </>
          ) : null}
        </article>
      ))}
    </div>
  );
}

function AuditTimeline({ events }: { events: AuditEventLike[] }) {
  if (events.length === 0) {
    return <p className="muted">No audit events loaded.</p>;
  }

  return (
    <div className="timeline">
      {events.map((event, index) => (
        <article
          key={`${event.timestamp ?? "event"}-${index}`}
          className="timeline-event"
        >
          <strong>{event.event_type ?? "event"}</strong>
          <p className="muted">
            {event.timestamp ?? "unknown time"} | {event.phase ?? "unknown phase"}
          </p>
          <p>{event.message ?? "No message"}</p>
        </article>
      ))}
    </div>
  );
}

function CommandPreviewCard({ preview }: { preview: CommandPreview }) {
  return (
    <div className="command-preview">
      <div className="two-column">
        <KeyValueGrid
          items={[
            ["Dry-run", preview.dryRun ? "yes" : "no"],
            ["Target-touching", preview.targetTouching ? "yes" : "no"],
            ["Local-only", preview.localOnly ? "yes" : "no"],
            ["Requires scope", preview.requiresScope ? "yes" : "no"]
          ]}
        />
        <div>
          <h4>Safety notices</h4>
          <ul className="plain-list compact">
            {preview.safetyNotices.map((notice) => (
              <li key={notice}>{notice}</li>
            ))}
          </ul>
        </div>
      </div>
      <h4>Exact command</h4>
      <pre className="code-block">{preview.exactCommand}</pre>
      {preview.blockedReasons.length > 0 ? (
        <Banner tone="danger" title="Blocked" items={preview.blockedReasons} />
      ) : null}
    </div>
  );
}

function AssetDetailView({
  item,
  snapshot,
  redactionEnabled
}: {
  item: ReviewItemLike;
  snapshot: WorkspaceSnapshot;
  redactionEnabled: boolean;
}) {
  const semanticMatch =
    snapshot.enrichment?.semanticAssets?.find(
      (asset) => String(asset.asset ?? "") === item.asset
    ) ?? null;
  const riskExplanation =
    (snapshot.enrichment?.riskExplanations ?? []).find(
      (entry) => String(entry.asset ?? "") === item.asset
    ) ?? null;
  const assetCard = findAssetCard(snapshot.assetCards, item.asset);
  const linkedCodex = getLinkedCodexDetails(
    item.asset,
    snapshot.codexSummary,
    snapshot.codexReview
  );

  return (
    <div className="stack">
      <KeyValueGrid
        items={[
          ["Asset", assetTitle(item.asset)],
          ["Risk level", item.risk_level],
          ["Score", String(item.score)],
          ["Confidence", String(item.confidence)],
          ["Roles", item.semantic_roles.join(", ") || "Unknown"],
          ["Environments", item.environments.join(", ") || "Unknown"]
        ]}
      />
      <div>
        <h4>Why this asset is interesting</h4>
        <ul className="plain-list compact">
          {item.reasons.map((reason) => (
            <li key={reason}>{reason}</li>
          ))}
        </ul>
      </div>
      <div>
        <h4>Evidence references</h4>
        <div className="pill-row">
          {item.evidence_refs.map((reference) => (
            <span key={reference} className="tag-pill">
              {reference}
            </span>
          ))}
        </div>
      </div>
      <div>
        <h4>Graph neighborhood summary</h4>
        <p>
          {String(
            semanticMatch?.neighborhood_summary ??
              "No neighborhood summary available."
          )}
        </p>
      </div>
      <div>
        <h4>Risk explanation</h4>
        <p>
          {String(
            riskExplanation?.explanation ?? "No risk explanation loaded."
          )}
        </p>
      </div>
      <div>
        <h4>Asset card</h4>
        {assetCard ? (
          <MarkdownCardView card={assetCard} redactionEnabled={redactionEnabled} />
        ) : (
          <EmptyState
            title="No asset card found"
            items={[
              "Review queue data is present, but no markdown asset card matched this asset.",
              "Run review to regenerate asset cards if needed."
            ]}
          />
        )}
      </div>
      <div>
        <h4>Linked Codex insights</h4>
        <pre className="code-block">
          {redactUnknown(linkedCodex.insightResults, redactionEnabled)}
        </pre>
      </div>
      <div>
        <h4>Linked Codex review warnings</h4>
        <pre className="code-block">
          {redactUnknown(
            {
              reviewItems: linkedCodex.reviewItems,
              warnings: linkedCodex.reviewWarnings
            },
            redactionEnabled
          )}
        </pre>
      </div>
      <Banner
        tone="warning"
        title="Caution"
        items={[
          "This view presents prioritization context, not vulnerability confirmation.",
          "Findings require validation before reporting."
        ]}
      />
    </div>
  );
}

function MarkdownCardView({
  card,
  redactionEnabled
}: {
  card: AssetCardLike;
  redactionEnabled: boolean;
}) {
  const blocks = parseMarkdownBlocks(redactText(card.markdown, redactionEnabled));
  return (
    <div className="markdown-card">
      <p className="muted">{card.path}</p>
      {blocks.map((block, index) => {
        if (block.type === "heading") {
          if (block.depth <= 2) {
            return <h4 key={`heading-${index}`}>{block.text}</h4>;
          }
          return <h5 key={`heading-${index}`}>{block.text}</h5>;
        }
        if (block.type === "bullet") {
          return (
            <ul key={`bullet-${index}`} className="plain-list compact">
              <li>{block.text}</li>
            </ul>
          );
        }
        return <p key={`paragraph-${index}`}>{block.text}</p>;
      })}
    </div>
  );
}

function GraphView({
  graph,
  onSelectAsset
}: {
  graph: WorkspaceSnapshot["graph"];
  onSelectAsset: (asset: string) => void;
}) {
  const nodes = graph?.nodes ?? [];
  const edges = graph?.edges ?? [];
  const clusters = graph?.clusters ?? [];
  return (
    <div className="two-column">
      <div className="stack">
        <h4>Nodes</h4>
        {nodes.map((node) => (
          <article key={String(node.id ?? Math.random())} className="list-card">
            <div className="list-card-header">
              <strong>{String(node.value ?? node.id ?? "node")}</strong>
              {String(node.value ?? "").includes(".") ? (
                <button
                  className="button button-ghost"
                  onClick={() => onSelectAsset(String(node.value))}
                >
                  Open Asset
                </button>
              ) : null}
            </div>
            <p className="muted">{String(node.node_type ?? "Unknown type")}</p>
          </article>
        ))}
      </div>
      <div className="stack">
        <h4>Edges</h4>
        <pre className="code-block">{JSON.stringify(edges, null, 2)}</pre>
        <h4>Clusters</h4>
        <pre className="code-block">{JSON.stringify(clusters, null, 2)}</pre>
      </div>
    </div>
  );
}

function ApiIntelView({
  apiIntel,
  redactionEnabled
}: {
  apiIntel: WorkspaceSnapshot["apiIntel"];
  redactionEnabled: boolean;
}) {
  return (
    <div className="stack">
      <div className="two-column">
        <div>
          <h4>Summary</h4>
          <pre className="code-block">
            {redactText(apiIntel?.summaryMarkdown ?? "No summary.", redactionEnabled)}
          </pre>
        </div>
        <div>
          <h4>Auth observations</h4>
          <pre className="code-block">
            {redactUnknown(apiIntel?.authObservations ?? [], redactionEnabled)}
          </pre>
        </div>
      </div>
      <h4>Endpoints</h4>
      <pre className="code-block">
        {redactUnknown(apiIntel?.endpoints ?? [], redactionEnabled)}
      </pre>
      <h4>Objects and GraphQL</h4>
      <pre className="code-block">
        {redactUnknown(
          {
            objects: apiIntel?.objects ?? [],
            graphql: apiIntel?.graphqlObservations ?? []
          },
          redactionEnabled
        )}
      </pre>
    </div>
  );
}

function EnrichmentView({
  enrichment
}: {
  enrichment: WorkspaceSnapshot["enrichment"];
}) {
  return (
    <div className="stack">
      <div className="two-column">
        <div>
          <h4>Semantic assets</h4>
          <pre className="code-block">
            {JSON.stringify(enrichment?.semanticAssets ?? [], null, 2)}
          </pre>
        </div>
        <div>
          <h4>Observations</h4>
          <pre className="code-block">
            {JSON.stringify(enrichment?.observations ?? [], null, 2)}
          </pre>
        </div>
      </div>
      <h4>Risk explanations</h4>
      <pre className="code-block">
        {JSON.stringify(enrichment?.riskExplanations ?? [], null, 2)}
      </pre>
      <h4>Summary</h4>
      <pre className="code-block">
        {enrichment?.summaryMarkdown ?? "No enrichment summary."}
      </pre>
    </div>
  );
}

function LlmPackView({ llmPack }: { llmPack: WorkspaceSnapshot["llmPack"] }) {
  return (
    <div className="stack">
      <KeyValueGrid
        items={[
          [
            "Max context chars",
            String(
              (llmPack?.summary as Record<string, unknown> | undefined)
                ?.max_context_chars ?? "unknown"
            )
          ],
          [
            "Queue size",
            String(
              (llmPack?.summary as Record<string, unknown> | undefined)?.total_items ??
                (llmPack?.reasoningQueue?.length ?? 0)
            )
          ]
        ]}
      />
      <h4>Prompt templates</h4>
      <ul className="plain-list compact">
        {(llmPack?.promptNames ?? []).map((prompt) => (
          <li key={prompt}>{prompt}</li>
        ))}
      </ul>
      <h4>Reasoning queue</h4>
      <pre className="code-block">
        {JSON.stringify(llmPack?.reasoningQueue ?? [], null, 2)}
      </pre>
    </div>
  );
}

function CodexInsightsView({
  codexSummary,
  redactionEnabled
}: {
  codexSummary: WorkspaceSnapshot["codexSummary"];
  redactionEnabled: boolean;
}) {
  return (
    <div className="stack">
      <KeyValueGrid
        items={[
          ["Planned", String(codexSummary?.planned_count ?? 0)],
          ["Executed", String(codexSummary?.executed_count ?? 0)],
          ["Succeeded", String(codexSummary?.success_count ?? 0)],
          ["Failed", String(codexSummary?.failure_count ?? 0)]
        ]}
      />
      <Banner
        tone="info"
        title="Codex safety"
        items={[
          "Codex is optional and explicit-only.",
          "Reasoning outputs are hypotheses and require validation.",
          "Dangerous bypass flags are blocked by design."
        ]}
      />
      <pre className="code-block">
        {redactUnknown(codexSummary?.results ?? [], redactionEnabled)}
      </pre>
    </div>
  );
}

function CodexReviewView({
  codexReview
}: {
  codexReview: WorkspaceSnapshot["codexReview"];
}) {
  return (
    <div className="stack">
      <div className="two-column">
        <div>
          <h4>Review queue</h4>
          <pre className="code-block">
            {JSON.stringify(codexReview?.items ?? [], null, 2)}
          </pre>
        </div>
        <div>
          <h4>Summary</h4>
          <pre className="code-block">
            {codexReview?.summaryMarkdown ?? "No summary."}
          </pre>
        </div>
      </div>
      <div className="two-column">
        <div>
          <h4>Unsupported claims</h4>
          <pre className="code-block">
            {JSON.stringify(codexReview?.unsupportedClaims ?? [], null, 2)}
          </pre>
        </div>
        <div>
          <h4>Evidence gaps and wording warnings</h4>
          <pre className="code-block">
            {JSON.stringify(
              {
                evidenceGaps: codexReview?.evidenceGaps ?? [],
                wordingWarnings: codexReview?.wordingWarnings ?? []
              },
              null,
              2
            )}
          </pre>
        </div>
      </div>
    </div>
  );
}

function ValidationView({ snapshot }: { snapshot: WorkspaceSnapshot }) {
  const validationGroups = groupValidation(snapshot.validation);
  const manifestCounts = Object.entries(snapshot.manifest?.artifact_counts ?? {});

  return (
    <div className="stack">
      <section className="grid-three">
        <MetricCard
          label="Validation"
          value={snapshot.validation?.success ? "Healthy" : "Attention"}
          hint={`${validationGroups.errors.length} error(s)`}
        />
        <MetricCard
          label="Audit Events"
          value={String(snapshot.auditEvents.length)}
          hint="append-only local trail"
        />
        <MetricCard
          label="GUI Commands"
          value={String(snapshot.guiExecutionLog?.length ?? 0)}
          hint="GUI execution log entries"
        />
      </section>

      <div className="two-column">
        <div>
          <h4>Validation errors</h4>
          {validationGroups.errors.length > 0 ? (
            <ul className="plain-list compact">
              {validationGroups.errors.map((error) => (
                <li key={error}>{error}</li>
              ))}
            </ul>
          ) : (
            <p className="muted">No validation errors were reported.</p>
          )}
        </div>
        <div>
          <h4>Validation warnings</h4>
          {validationGroups.warnings.length > 0 ? (
            <ul className="plain-list compact">
              {validationGroups.warnings.map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
          ) : (
            <p className="muted">No validation warnings were reported.</p>
          )}
        </div>
      </div>

      <Panel title="Manifest Summary" subtitle="High-level artifact counts from the latest run manifest.">
        {manifestCounts.length > 0 ? (
          <div className="cards-grid">
            {manifestCounts.map(([key, value]) => (
              <article key={key} className="metric-card">
                <span>{key}</span>
                <strong>{String(value)}</strong>
                <small>artifacts counted</small>
              </article>
            ))}
          </div>
        ) : (
          <p className="muted">No manifest artifact counts were found.</p>
        )}
      </Panel>

      <div className="two-column">
        <div>
          <h4>Audit timeline</h4>
          <AuditTimeline events={sortAuditTimeline(snapshot.auditEvents).slice(0, 10)} />
        </div>
        <div>
          <h4>GUI execution log</h4>
          <AuditTimeline
            events={sortAuditTimeline(snapshot.guiExecutionLog ?? []).slice(0, 10)}
          />
        </div>
      </div>
    </div>
  );
}

function ResultView({
  result,
  redactionEnabled
}: {
  result: GuiCommandResult;
  redactionEnabled: boolean;
}) {
  return (
    <div className="stack">
      <div className="mode-badges">
        <StatusPill tone={result.success ? "safe" : "danger"}>
          {result.success ? "Command succeeded" : "Command failed"}
        </StatusPill>
        <StatusPill tone={result.dryRun ? "safe" : "warning"}>
          {result.dryRun ? "Dry-run / local plan" : "Execution mode"}
        </StatusPill>
      </div>
      <KeyValueGrid
        items={[
          ["Executed", result.executed ? "yes" : "no"],
          ["Dry-run", result.dryRun ? "yes" : "no"],
          ["Success", result.success ? "yes" : "no"],
          ["Exit code", String(result.exitCode)],
          ["GUI log path", result.guiLogPath]
        ]}
      />
      <h4>Exact command</h4>
      <pre className="code-block">{result.exactCommand}</pre>
      <div className="two-column">
        <div>
          <h4>stdout</h4>
          <pre className="code-block">
            {redactText(result.stdout || "No stdout was captured.", redactionEnabled)}
          </pre>
        </div>
        <div>
          <h4>stderr</h4>
          <pre className="code-block">
            {redactText(result.stderr || "No stderr was captured.", redactionEnabled)}
          </pre>
        </div>
      </div>
      {result.warnings.length > 0 ? (
        <Banner tone="warning" title="Result warnings" items={result.warnings} />
      ) : null}
    </div>
  );
}

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
