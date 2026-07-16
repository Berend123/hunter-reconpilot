import { BUILTIN_PROFILES, DEFAULT_GUI_CONFIG, mockCommandResult, mockWorkspaceSnapshot } from "../mock";
import type {
  CommandPreview,
  CustomProfile,
  GuiConfig,
  GuiCommandRequest,
  GuiCommandResult,
  WorkspaceSnapshot
} from "../types";

function getInvoke() {
  return window.__TAURI__?.core?.invoke;
}

function isTauriRuntime(): boolean {
  return typeof getInvoke() === "function";
}

export async function loadWorkspaceSnapshot(workspacePath: string): Promise<WorkspaceSnapshot> {
  const invoke = getInvoke();
  if (invoke) {
    return invoke<WorkspaceSnapshot>("load_workspace_snapshot", { workspacePath });
  }

  return mockWorkspaceSnapshot(workspacePath);
}

export async function loadGuiConfig(workspacePath: string): Promise<GuiConfig> {
  const invoke = getInvoke();
  if (invoke) {
    return invoke<GuiConfig>("load_gui_config", { workspacePath });
  }

  const raw = window.localStorage.getItem(`reconpilot.gui.${workspacePath}`);
  if (!raw) {
    return DEFAULT_GUI_CONFIG;
  }

  try {
    return JSON.parse(raw) as GuiConfig;
  } catch {
    return DEFAULT_GUI_CONFIG;
  }
}

export async function saveGuiConfig(workspacePath: string, config: GuiConfig): Promise<void> {
  const invoke = getInvoke();
  if (invoke) {
    await invoke("save_gui_config", { workspacePath, config });
    return;
  }

  window.localStorage.setItem(`reconpilot.gui.${workspacePath}`, JSON.stringify(config));
}

export async function runGuiCommand(
  request: GuiCommandRequest,
  preview: CommandPreview
): Promise<GuiCommandResult> {
  const invoke = getInvoke();
  if (invoke) {
    return invoke<GuiCommandResult>("run_reconpilot_command", { request });
  }

  return mockCommandResult(request, preview);
}

export function listProfiles(config: GuiConfig): CustomProfile[] {
  return [...BUILTIN_PROFILES, ...config.customProfiles];
}

export function browserRuntimeLabel(): string {
  return isTauriRuntime() ? "tauri-runtime" : "browser-demo";
}
