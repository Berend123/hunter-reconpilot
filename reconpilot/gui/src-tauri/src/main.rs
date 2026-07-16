use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuiConfig {
    mode: String,
    redaction_enabled: bool,
    remembered_workspace: Option<String>,
    acknowledgements: Value,
    rate_limits: Value,
    concurrency: Value,
    custom_profiles: Vec<Value>,
    custom_tool_args: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSnapshot {
    root_path: String,
    browser_fallback: bool,
    warnings: Vec<String>,
    workspace_health: WorkspaceHealth,
    manifest: Option<Value>,
    validation: Option<Value>,
    audit_events: Vec<Value>,
    scope_text: Option<String>,
    exclusion_text: Option<String>,
    asset_cards: Vec<AssetCard>,
    review_queue: Option<Value>,
    graph: Option<Value>,
    api_intel: Option<Value>,
    enrichment: Option<Value>,
    llm_pack: Option<Value>,
    codex_summary: Option<Value>,
    codex_review: Option<Value>,
    gui_execution_log: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuiCommandRequest {
    workspace_path: String,
    kind: String,
    scope_path: Option<String>,
    profile_name: Option<String>,
    out_dir: Option<String>,
    input_dir: Option<String>,
    pack_dir: Option<String>,
    execute: Option<bool>,
    include_codex: Option<bool>,
    execute_codex: Option<bool>,
    limit: Option<usize>,
    template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuiCommandResult {
    exact_command: String,
    executed: bool,
    dry_run: bool,
    success: bool,
    exit_code: i32,
    stdout: String,
    stderr: String,
    gui_log_path: String,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceHealth {
    detected_from: String,
    status: String,
    root_path: String,
    output_path: String,
    config_path: String,
    docs_path: String,
    messages: Vec<String>,
    checks: Vec<WorkspaceHealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceHealthCheck {
    key: String,
    label: String,
    path: String,
    present: bool,
    required: bool,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssetCard {
    asset: String,
    path: String,
    markdown: String,
}

#[derive(Debug, Clone)]
struct WorkspaceResolution {
    root: PathBuf,
    detected_from: String,
    messages: Vec<String>,
}

#[tauri::command]
fn load_workspace_snapshot(workspace_path: String) -> Result<WorkspaceSnapshot, String> {
    let selected = PathBuf::from(&workspace_path);
    if !selected.exists() {
        return Err(format!("workspace does not exist: {}", selected.display()));
    }

    let resolution = resolve_workspace_root(&selected);
    let root = resolution.root.clone();
    let output = root.join("output");
    let config = root.join("config");
    let docs = root.join("docs");
    let workspace_health = build_workspace_health(&root, &output, &config, &docs, &resolution);

    Ok(WorkspaceSnapshot {
        root_path: root.display().to_string(),
        browser_fallback: false,
        warnings: resolution.messages.clone(),
        workspace_health,
        manifest: load_optional_json(output.join("run-manifest.json"))?,
        validation: load_optional_json(output.join("validation-report.json"))?,
        audit_events: load_optional_jsonl(output.join("audit-log.jsonl"))?,
        scope_text: load_optional_text(config.join("scope.txt"))
            .or_else(|| load_optional_text(config.join("scope.example.txt"))),
        exclusion_text: load_optional_text(config.join("excluded.txt"))
            .or_else(|| load_optional_text(config.join("excluded.example.txt"))),
        asset_cards: load_asset_cards(&output.join("review").join("asset-cards"))?,
        review_queue: load_optional_json(output.join("review").join("priority-queue.json"))?,
        graph: load_optional_json(output.join("maps").join("graph.json"))?,
        api_intel: load_optional_json_bundle(&[
            ("summaryMarkdown", output.join("api-intel").join("api-summary.md")),
            ("endpoints", output.join("api-intel").join("api-endpoints.json")),
            ("objects", output.join("api-intel").join("api-objects.json")),
            (
                "relationships",
                output.join("api-intel").join("api-relationships.json"),
            ),
            (
                "authObservations",
                output.join("api-intel").join("auth-observations.json"),
            ),
            (
                "jsObservations",
                output.join("api-intel").join("js-observations.json"),
            ),
            ("schemas", output.join("api-intel").join("schemas.json")),
            (
                "graphqlObservations",
                output.join("api-intel").join("graphql-observations.json"),
            ),
        ])?,
        enrichment: load_optional_json_bundle(&[
            (
                "semanticAssets",
                output.join("enrichment").join("semantic-assets.json"),
            ),
            (
                "observations",
                output.join("enrichment").join("semantic-observations.json"),
            ),
            (
                "riskExplanations",
                output.join("enrichment").join("risk-explanations.json"),
            ),
            (
                "summaryMarkdown",
                output.join("enrichment").join("enrichment-summary.md"),
            ),
        ])?,
        llm_pack: load_optional_json_bundle(&[
            (
                "reasoningQueue",
                output.join("llm-pack").join("reasoning-queue.json"),
            ),
            ("summary", output.join("llm-pack").join("pack-summary.json")),
        ])?,
        codex_summary: load_optional_json(output.join("codex-insights").join("codex-summary.json"))?,
        codex_review: load_optional_json_bundle(&[
            (
                "items",
                output
                    .join("codex-review")
                    .join("codex-review-queue.json"),
            ),
            (
                "unsupportedClaims",
                output
                    .join("codex-review")
                    .join("unsupported-claims.json"),
            ),
            (
                "evidenceGaps",
                output.join("codex-review").join("evidence-gaps.json"),
            ),
            (
                "wordingWarnings",
                output
                    .join("codex-review")
                    .join("wording-warnings.json"),
            ),
            (
                "summaryMarkdown",
                output
                    .join("codex-review")
                    .join("codex-review-summary.md"),
            ),
        ])?,
        gui_execution_log: load_optional_jsonl(output.join("gui-execution-log.jsonl"))?,
    })
}

#[tauri::command]
fn load_gui_config(workspace_path: String) -> Result<GuiConfig, String> {
    let resolution = resolve_workspace_root(Path::new(&workspace_path));
    let path = resolution
        .root
        .join("config")
        .join("reconpilot.gui.json");
    if !path.exists() {
        return Ok(default_gui_config());
    }

    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    serde_json::from_str(&raw).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_gui_config(workspace_path: String, config: GuiConfig) -> Result<(), String> {
    let resolution = resolve_workspace_root(Path::new(&workspace_path));
    let path = resolution
        .root
        .join("config")
        .join("reconpilot.gui.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let raw = serde_json::to_string_pretty(&config).map_err(|error| error.to_string())?;
    fs::write(path, raw).map_err(|error| error.to_string())
}

#[tauri::command]
fn run_reconpilot_command(request: GuiCommandRequest) -> Result<GuiCommandResult, String> {
    let selected = PathBuf::from(&request.workspace_path);
    if !selected.exists() {
        return Err(format!("workspace does not exist: {}", selected.display()));
    }
    let workspace = resolve_workspace_root(&selected).root;

    let (program, args, dry_run, warnings) = build_allowed_command(&workspace, &request)?;
    let exact_command = render_command(&program, &args);
    let output = Command::new(&program)
        .args(&args)
        .current_dir(&workspace)
        .output()
        .map_err(|error| format!("failed to execute allowed command: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);
    let success = output.status.success();
    let gui_log_path = workspace.join("output").join("gui-execution-log.jsonl");

    append_gui_log(
        &gui_log_path,
        &request.kind,
        &exact_command,
        success,
        exit_code,
        &warnings,
    )?;

    Ok(GuiCommandResult {
        exact_command,
        executed: true,
        dry_run,
        success,
        exit_code,
        stdout,
        stderr,
        gui_log_path: relative_to_workspace(&workspace, &gui_log_path),
        warnings,
    })
}

fn build_allowed_command(
    workspace: &Path,
    request: &GuiCommandRequest,
) -> Result<(PathBuf, Vec<String>, bool, Vec<String>), String> {
    let binary = resolve_reconpilot_binary(workspace)?;
    let mut args = Vec::new();
    let mut warnings = Vec::new();

    match request.kind.as_str() {
        "doctor" => {
            args.push("doctor".to_string());
            Ok((binary, args, true, warnings))
        }
        "validate" => {
            args.extend([
                "validate".to_string(),
                "--input".to_string(),
                request
                    .input_dir
                    .clone()
                    .unwrap_or_else(|| "output".to_string()),
            ]);
            Ok((binary, args, true, warnings))
        }
        "codex-review" => {
            let input = request
                .input_dir
                .clone()
                .ok_or_else(|| "codex-review requires inputDir".to_string())?;
            let out = request
                .out_dir
                .clone()
                .ok_or_else(|| "codex-review requires outDir".to_string())?;
            args.extend([
                "codex-review".to_string(),
                "--input".to_string(),
                input,
                "--out".to_string(),
                out,
            ]);
            Ok((binary, args, true, warnings))
        }
        "codex-run" => {
            let pack = request
                .pack_dir
                .clone()
                .ok_or_else(|| "codex-run requires packDir".to_string())?;
            let out = request
                .out_dir
                .clone()
                .ok_or_else(|| "codex-run requires outDir".to_string())?;
            args.extend([
                "codex-run".to_string(),
                "--pack".to_string(),
                pack,
                "--out".to_string(),
                out,
                "--limit".to_string(),
                request.limit.unwrap_or(3).to_string(),
            ]);
            if let Some(template) = &request.template {
                args.extend(["--template".to_string(), template.clone()]);
            }
            if request.execute_codex.unwrap_or(false) {
                args.push("--execute-codex".to_string());
            }
            Ok((binary, args, !request.execute_codex.unwrap_or(false), warnings))
        }
        "pipeline" => {
            let scope = request
                .scope_path
                .clone()
                .ok_or_else(|| "pipeline requires scopePath".to_string())?;
            let profile = request
                .profile_name
                .clone()
                .ok_or_else(|| "pipeline requires profileName".to_string())?;
            let out = request
                .out_dir
                .clone()
                .ok_or_else(|| "pipeline requires outDir".to_string())?;
            args.extend([
                "pipeline".to_string(),
                "--scope".to_string(),
                scope,
                "--profile".to_string(),
                profile,
                "--out".to_string(),
                out,
            ]);
            if request.execute.unwrap_or(false) {
                args.push("--execute".to_string());
            }
            if request.include_codex.unwrap_or(false) {
                args.push("--include-codex".to_string());
            }
            if request.execute_codex.unwrap_or(false) {
                if request.include_codex.unwrap_or(false) {
                    args.push("--execute-codex".to_string());
                } else {
                    warnings.push(
                        "--execute-codex was ignored because --include-codex was not set."
                            .to_string(),
                    );
                }
            }
            Ok((
                binary,
                args,
                !request.execute.unwrap_or(false) && !request.execute_codex.unwrap_or(false),
                warnings,
            ))
        }
        other => Err(format!(
            "unsupported command kind '{other}'. GUI execution is restricted to the ReconPilot allowlist."
        )),
    }
}

fn resolve_reconpilot_binary(workspace: &Path) -> Result<PathBuf, String> {
    let candidates = [
        workspace.join("reconpilot.exe"),
        workspace.join("target").join("debug").join("reconpilot.exe"),
        workspace
            .join("target")
            .join("x86_64-pc-windows-gnullvm")
            .join("debug")
            .join("reconpilot.exe"),
        workspace.join("target").join("release").join("reconpilot.exe"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Ok(PathBuf::from("reconpilot"))
}

fn render_command(program: &Path, args: &[String]) -> String {
    let mut parts = vec![program.display().to_string()];
    for arg in args {
        if arg.contains(' ') {
            parts.push(format!("\"{arg}\""));
        } else {
            parts.push(arg.clone());
        }
    }
    parts.join(" ")
}

fn append_gui_log(
    path: &Path,
    kind: &str,
    command: &str,
    success: bool,
    exit_code: i32,
    warnings: &[String],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let record = serde_json::json!({
        "timestamp": Utc::now().to_rfc3339(),
        "phase": kind,
        "event_type": "gui_command_executed",
        "message": command,
        "details": {
            "success": success,
            "exit_code": exit_code,
            "warnings": warnings
        }
    });

    let mut line = serde_json::to_string(&record).map_err(|error| error.to_string())?;
    line.push('\n');
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(line.as_bytes())
        .map_err(|error| error.to_string())
}

fn load_optional_text(path: PathBuf) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn load_optional_json(path: PathBuf) -> Result<Option<Value>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let value = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    Ok(Some(value))
}

fn load_optional_jsonl(path: PathBuf) -> Result<Vec<Value>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let mut records = Vec::new();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let value = serde_json::from_str::<Value>(line).map_err(|error| error.to_string())?;
        records.push(value);
    }
    Ok(records)
}

fn load_optional_json_bundle(entries: &[(&str, PathBuf)]) -> Result<Option<Value>, String> {
    let mut bundle = BTreeMap::new();
    let mut any = false;
    for (key, path) in entries {
        if !path.exists() {
            continue;
        }
        any = true;
        if path.extension().and_then(|value| value.to_str()) == Some("md") {
            let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
            bundle.insert((*key).to_string(), Value::String(raw));
        } else if let Some(json) = load_optional_json(path.clone())? {
            bundle.insert((*key).to_string(), json);
        }
    }

    if any {
        Ok(Some(serde_json::to_value(bundle).map_err(|error| error.to_string())?))
    } else {
        Ok(None)
    }
}

fn relative_to_workspace(workspace: &Path, target: &Path) -> String {
    target
        .strip_prefix(workspace)
        .unwrap_or(target)
        .display()
        .to_string()
}

fn default_gui_config() -> GuiConfig {
    GuiConfig {
        mode: "beginner".to_string(),
        redaction_enabled: true,
        remembered_workspace: None,
        acknowledgements: serde_json::json!({
            "targetContactUnderstood": false,
            "advancedModeUnderstood": false,
            "codexExecutionUnderstood": false
        }),
        rate_limits: serde_json::json!({
            "httpRequestsPerSecond": 4,
            "dnsQueriesPerSecond": 20,
            "screenshotConcurrency": 2
        }),
        concurrency: serde_json::json!({
            "maxPhases": 1,
            "maxArtifactsPerView": 250
        }),
        custom_profiles: Vec::new(),
        custom_tool_args: Vec::new(),
    }
}

fn resolve_workspace_root(selected: &Path) -> WorkspaceResolution {
    let as_dir = if selected.is_file() {
        selected.parent().unwrap_or(selected)
    } else {
        selected
    };

    let direct = as_dir.to_path_buf();
    let parent = direct.parent().map(|path| path.to_path_buf());

    if direct.join("output").is_dir() && direct.join("config").is_dir() {
        return WorkspaceResolution {
            root: direct,
            detected_from: "project-root".to_string(),
            messages: vec!["Workspace root detected directly.".to_string()],
        };
    }

    if direct.file_name().and_then(|value| value.to_str()) == Some("output") {
        if let Some(parent) = &parent {
            return WorkspaceResolution {
                root: parent.clone(),
                detected_from: "output-dir".to_string(),
                messages: vec![
                    "An output directory was selected, so the parent project root was used."
                        .to_string(),
                ],
            };
        }
    }

    if direct.file_name().and_then(|value| value.to_str()) == Some("config") {
        if let Some(parent) = &parent {
            return WorkspaceResolution {
                root: parent.clone(),
                detected_from: "config-dir".to_string(),
                messages: vec![
                    "A config directory was selected, so the parent project root was used."
                        .to_string(),
                ],
            };
        }
    }

    if direct.file_name().and_then(|value| value.to_str()) == Some("docs") {
        if let Some(parent) = &parent {
            return WorkspaceResolution {
                root: parent.clone(),
                detected_from: "docs-dir".to_string(),
                messages: vec![
                    "A docs directory was selected, so the parent project root was used."
                        .to_string(),
                ],
            };
        }
    }

    WorkspaceResolution {
        root: direct,
        detected_from: "unknown".to_string(),
        messages: vec![
            "Workspace root was assumed from the selected path. Missing directories may limit the GUI."
                .to_string(),
        ],
    }
}

fn build_workspace_health(
    root: &Path,
    output: &Path,
    config: &Path,
    docs: &Path,
    resolution: &WorkspaceResolution,
) -> WorkspaceHealth {
    let checks = vec![
        workspace_check(
            "config",
            "Config directory",
            config,
            true,
            "Required for scope, exclusions, and GUI config.",
        ),
        workspace_check(
            "output",
            "Output directory",
            output,
            true,
            "Required for artifact browsing and local review outputs.",
        ),
        workspace_check(
            "docs",
            "Docs directory",
            docs,
            false,
            "Optional, but useful for local operator guidance.",
        ),
        workspace_check(
            "manifest",
            "Run manifest",
            &output.join("run-manifest.json"),
            false,
            "Generated after validated runs and used for GUI status summaries.",
        ),
        workspace_check(
            "validation",
            "Validation report",
            &output.join("validation-report.json"),
            false,
            "Recommended before Codex review or reasoning workflows.",
        ),
        workspace_check(
            "review-queue",
            "Review queue",
            &output.join("review").join("priority-queue.json"),
            false,
            "Generated after enrich and review phases.",
        ),
    ];

    let required_missing = checks.iter().filter(|check| check.required && !check.present).count();
    let optional_present = checks.iter().filter(|check| !check.required && check.present).count();
    let status = if required_missing > 0 {
        "invalid"
    } else if optional_present > 0 {
        "healthy"
    } else {
        "partial"
    };

    WorkspaceHealth {
        detected_from: resolution.detected_from.clone(),
        status: status.to_string(),
        root_path: root.display().to_string(),
        output_path: output.display().to_string(),
        config_path: config.display().to_string(),
        docs_path: docs.display().to_string(),
        messages: resolution.messages.clone(),
        checks,
    }
}

fn workspace_check(
    key: &str,
    label: &str,
    path: &Path,
    required: bool,
    message: &str,
) -> WorkspaceHealthCheck {
    WorkspaceHealthCheck {
        key: key.to_string(),
        label: label.to_string(),
        path: path.display().to_string(),
        present: path.exists(),
        required,
        message: message.to_string(),
    }
}

fn load_asset_cards(path: &Path) -> Result<Vec<AssetCard>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut cards = Vec::new();
    let entries = fs::read_dir(path).map_err(|error| error.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let markdown = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let asset = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("asset-card")
            .replace('-', ".");
        cards.push(AssetCard {
            asset,
            path: path.display().to_string(),
            markdown,
        });
    }
    cards.sort_by(|left, right| left.asset.cmp(&right.asset));
    Ok(cards)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_workspace_snapshot,
            load_gui_config,
            save_gui_config,
            run_reconpilot_command
        ])
        .run(tauri::generate_context!())
        .expect("failed to run ReconPilot GUI");
}
