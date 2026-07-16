use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use tokio::process::Command;
use walkdir::WalkDir;

use crate::{
    config::ReconPilotConfig,
    models::{
        AppMapEdge, AppMapNode, DnsRecord, ReconAsset, ReconToolRun, ScreenshotRecord,
        TechFingerprint,
    },
    scope::ScopeDefinition,
    utils::{self, OutputLayout},
};

#[derive(Debug, Clone)]
pub struct MappingOutcome {
    pub runs: Vec<ReconToolRun>,
    pub map_json_path: PathBuf,
    pub map_markdown_path: PathBuf,
}

#[derive(Debug, Clone)]
struct MappingAdapterDefinition {
    tool: &'static str,
    binary_name: &'static str,
    plan_filename: &'static str,
    build: fn(&ScopeDefinition, &OutputLayout) -> Result<MappingAdapterCommand>,
}

#[derive(Debug, Clone)]
struct MappingAdapterCommand {
    phase: String,
    program: String,
    arguments: Vec<String>,
    planned_inputs: Vec<PathBuf>,
    output_files: Vec<PathBuf>,
    primary_output_path: Option<PathBuf>,
    stdout_path: Option<PathBuf>,
    stderr_path: Option<PathBuf>,
    can_execute: bool,
    notes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AppMapDocument {
    generated_at: DateTime<Utc>,
    placeholder: bool,
    notes: Vec<String>,
    nodes: Vec<AppMapNode>,
    edges: Vec<AppMapEdge>,
    assets: Vec<ReconAsset>,
    dns_records: Vec<DnsRecord>,
    screenshots: Vec<ScreenshotRecord>,
    tech_fingerprints: Vec<TechFingerprint>,
}

pub async fn run_mapping_layer(
    scope: &ScopeDefinition,
    output: &OutputLayout,
    execute: bool,
) -> Result<MappingOutcome> {
    validate_scope_for_mapping(scope)?;
    ensure_mapping_directories(output)?;

    let mut runs = Vec::new();

    for adapter in mapping_adapter_registry() {
        let command = (adapter.build)(scope, output)?;
        let binary_path = resolve_binary_path(adapter.binary_name);
        let mut run = ReconToolRun {
            tool: adapter.tool.to_string(),
            phase: command.phase,
            binary_name: adapter.binary_name.to_string(),
            binary_path: binary_path.clone(),
            binary_exists: binary_path.is_some(),
            program: command.program.clone(),
            arguments: command.arguments.clone(),
            command_line: render_command(&command.program, &command.arguments),
            execute_requested: execute,
            executed: false,
            success: None,
            exit_code: None,
            scope_source: scope.source_path.clone(),
            planned_inputs: command.planned_inputs,
            output_files: command.output_files,
            primary_output_path: command.primary_output_path,
            stdout_path: command.stdout_path,
            stderr_path: command.stderr_path,
            planned_at: Utc::now(),
            started_at: None,
            finished_at: None,
            notes: command.notes,
        };

        if execute {
            if !run.binary_exists {
                run.success = Some(false);
                run.notes
                    .push("Binary not found on PATH; execution skipped.".to_string());
            } else if !command.can_execute {
                run.success = Some(false);
                run.notes
                    .push("Required input was unavailable; execution skipped.".to_string());
            } else if let Err(error) = execute_tool_run(&mut run).await {
                run.success = Some(false);
                run.notes.push(format!("Execution error: {error:#}"));
            }
        } else {
            run.notes
                .push("Dry-run mode: command was planned but not executed.".to_string());
        }

        utils::write_json_pretty(&output.plans.join(adapter.plan_filename), &run)?;
        runs.push(run);
    }

    let app_map = build_placeholder_app_map(scope, output)?;
    let map_json_path = output.maps.join("app-map.json");
    let map_markdown_path = output.maps.join("app-map.md");
    utils::write_json_pretty(&map_json_path, &app_map)?;
    utils::write_string(&map_markdown_path, &render_app_map_markdown(&app_map))?;

    Ok(MappingOutcome {
        runs,
        map_json_path,
        map_markdown_path,
    })
}

pub fn print_mapping_summary(
    scope: &ScopeDefinition,
    config: &ReconPilotConfig,
    output: &OutputLayout,
    outcome: &MappingOutcome,
    execute: bool,
) {
    println!("ReconPilot mapping summary");
    println!("Scope source: {}", scope.source_path.display());
    println!("Scope targets: {}", scope.probe_targets().join(", "));
    println!("Profile: {}", config.profile_name);
    println!(
        "Mode: {}",
        if execute {
            "execute"
        } else {
            "dry-run (default)"
        }
    );
    println!(
        "Safety notice: mapping adapters can touch targets and remain gated behind --execute."
    );
    println!("Plans: {}", output.plans.display());
    println!("DNS output: {}", output.dns.display());
    println!("Screenshots: {}", output.screenshots.display());
    println!("Technology output: {}", output.tech.display());
    println!("Maps: {}", output.maps.display());
    println!();

    for run in &outcome.runs {
        println!(
            "- {} [{}] binary:{} executed:{}",
            run.tool,
            run.phase,
            if run.binary_exists {
                "found"
            } else {
                "missing"
            },
            run.executed
        );
        println!("  Command: {}", run.command_line);
        if !run.notes.is_empty() {
            println!("  Notes: {}", run.notes.join(" | "));
        }
    }

    println!();
    println!("Map outputs:");
    println!("  - {}", outcome.map_json_path.display());
    println!("  - {}", outcome.map_markdown_path.display());
}

fn mapping_adapter_registry() -> [MappingAdapterDefinition; 3] {
    [
        MappingAdapterDefinition {
            tool: "dnsx",
            binary_name: "dnsx",
            plan_filename: "dnsx-plan.json",
            build: build_dnsx_command,
        },
        MappingAdapterDefinition {
            tool: "gowitness",
            binary_name: "gowitness",
            plan_filename: "gowitness-plan.json",
            build: build_gowitness_command,
        },
        MappingAdapterDefinition {
            tool: "WhatWeb",
            binary_name: "whatweb",
            plan_filename: "whatweb-plan.json",
            build: build_whatweb_command,
        },
    ]
}

fn ensure_mapping_directories(output: &OutputLayout) -> Result<()> {
    for path in [
        &output.dns,
        &output.screenshots,
        &output.tech,
        &output.maps,
        &output.plans,
    ] {
        utils::ensure_directory(path)?;
    }

    Ok(())
}

fn validate_scope_for_mapping(scope: &ScopeDefinition) -> Result<()> {
    if scope.is_empty() {
        anyhow::bail!("scope validation failed: scope entries cannot be empty");
    }

    if scope.domain_targets().is_empty() {
        anyhow::bail!("scope validation failed: no mapping targets could be derived");
    }

    Ok(())
}

fn build_dnsx_command(
    scope: &ScopeDefinition,
    output: &OutputLayout,
) -> Result<MappingAdapterCommand> {
    validate_scope_for_mapping(scope)?;

    let subfinder_output = output.raw.join("subfinder").join("subdomains.txt");
    let scope_derived_input = output.plans.join("dnsx-input.txt");

    let (input_records, source_note) = if file_has_nonempty_lines(&subfinder_output)? {
        (
            utils::read_trimmed_lines(&subfinder_output)?,
            format!(
                "Using discovered subdomains from {} as dnsx input.",
                subfinder_output.display()
            ),
        )
    } else {
        (
            scope.domain_targets(),
            "No discovered subdomains were available; falling back to scope-derived domains."
                .to_string(),
        )
    };

    utils::write_lines(&scope_derived_input, &input_records)?;

    let dnsx_output = output.dns.join("dnsx.jsonl");
    let stdout_path = output.dns.join("dnsx.stdout.txt");
    let stderr_path = output.dns.join("dnsx.stderr.txt");

    Ok(MappingAdapterCommand {
        phase: "dns-mapping".to_string(),
        program: "dnsx".to_string(),
        arguments: vec![
            "-l".to_string(),
            scope_derived_input.display().to_string(),
            "-silent".to_string(),
            "-j".to_string(),
            "-omit-raw".to_string(),
            "-rl".to_string(),
            "25".to_string(),
            "-o".to_string(),
            dnsx_output.display().to_string(),
        ],
        planned_inputs: vec![scope_derived_input],
        output_files: vec![
            dnsx_output.clone(),
            stdout_path.clone(),
            stderr_path.clone(),
        ],
        primary_output_path: Some(dnsx_output),
        stdout_path: Some(stdout_path),
        stderr_path: Some(stderr_path),
        can_execute: true,
        notes: vec![
            source_note,
            "Purpose: DNS resolution and relationship mapping.".to_string(),
            "Rate-limit placeholder: dnsx is planned with a conservative `-rl 25` default."
                .to_string(),
        ],
    })
}

fn build_gowitness_command(
    scope: &ScopeDefinition,
    output: &OutputLayout,
) -> Result<MappingAdapterCommand> {
    validate_scope_for_mapping(scope)?;

    let live_hosts_input = output.raw.join("httpx").join("live-hosts.txt");
    let input_ready = file_has_nonempty_lines(&live_hosts_input)?;
    let jsonl_output = output.screenshots.join("gowitness.jsonl");
    let stdout_path = output.screenshots.join("gowitness.stdout.txt");
    let stderr_path = output.screenshots.join("gowitness.stderr.txt");

    let mut notes = vec![
        "Purpose: screenshots and visual application mapping.".to_string(),
        "Safety: screenshots still make HTTP requests and must obey program rules, scope, and rate constraints.".to_string(),
        "Rate-limit placeholder: add an explicit browser concurrency cap before enabling broader execution profiles."
            .to_string(),
    ];

    if !input_ready {
        notes.push(format!(
            "Live-host input is missing or empty at {}; command is planned against the expected future file but will not execute until it exists.",
            live_hosts_input.display()
        ));
    }

    Ok(MappingAdapterCommand {
        phase: "visual-mapping".to_string(),
        program: "gowitness".to_string(),
        arguments: vec![
            "scan".to_string(),
            "file".to_string(),
            "-f".to_string(),
            live_hosts_input.display().to_string(),
            "--screenshot-path".to_string(),
            output.screenshots.display().to_string(),
            "--write-jsonl".to_string(),
            jsonl_output.display().to_string(),
        ],
        planned_inputs: vec![live_hosts_input],
        output_files: vec![
            jsonl_output.clone(),
            stdout_path.clone(),
            stderr_path.clone(),
        ],
        primary_output_path: Some(jsonl_output),
        stdout_path: Some(stdout_path),
        stderr_path: Some(stderr_path),
        can_execute: input_ready,
        notes,
    })
}

fn build_whatweb_command(
    scope: &ScopeDefinition,
    output: &OutputLayout,
) -> Result<MappingAdapterCommand> {
    validate_scope_for_mapping(scope)?;

    let live_hosts_input = output.raw.join("httpx").join("live-hosts.txt");
    let input_ready = file_has_nonempty_lines(&live_hosts_input)?;
    let whatweb_output = output.tech.join("whatweb.json");
    let stdout_path = output.tech.join("whatweb.stdout.txt");
    let stderr_path = output.tech.join("whatweb.stderr.txt");

    let mut notes = vec![
        "Purpose: technology fingerprinting.".to_string(),
        "Safety: WhatWeb should remain on low-aggression settings and must stay within approved hosts."
            .to_string(),
        "Rate-limit placeholder: use low aggression and conservative threads for any future execution profile."
            .to_string(),
    ];

    if !input_ready {
        notes.push(format!(
            "Live-host input is missing or empty at {}; command is planned against the expected future file but will not execute until it exists.",
            live_hosts_input.display()
        ));
    }

    Ok(MappingAdapterCommand {
        phase: "tech-fingerprinting".to_string(),
        program: "whatweb".to_string(),
        arguments: vec![
            "-i".to_string(),
            live_hosts_input.display().to_string(),
            "-a".to_string(),
            "1".to_string(),
            "-t".to_string(),
            "5".to_string(),
            "--no-errors".to_string(),
            format!("--log-json={}", whatweb_output.display()),
        ],
        planned_inputs: vec![live_hosts_input],
        output_files: vec![
            whatweb_output.clone(),
            stdout_path.clone(),
            stderr_path.clone(),
        ],
        primary_output_path: Some(whatweb_output),
        stdout_path: Some(stdout_path),
        stderr_path: Some(stderr_path),
        can_execute: input_ready,
        notes,
    })
}

async fn execute_tool_run(run: &mut ReconToolRun) -> Result<()> {
    let binary_path = run
        .binary_path
        .clone()
        .context("binary path was not resolved before execution")?;

    run.started_at = Some(Utc::now());

    let mut command = Command::new(binary_path);
    command
        .args(&run.arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output = command
        .output()
        .await
        .with_context(|| format!("failed to execute {}", run.tool))?;

    if let Some(stdout_path) = &run.stdout_path {
        if let Some(parent) = stdout_path.parent() {
            utils::ensure_directory(parent)?;
        }
        fs::write(stdout_path, &output.stdout)
            .with_context(|| format!("failed to write stdout capture for {}", run.tool))?;
    }

    if let Some(stderr_path) = &run.stderr_path {
        if let Some(parent) = stderr_path.parent() {
            utils::ensure_directory(parent)?;
        }
        fs::write(stderr_path, &output.stderr)
            .with_context(|| format!("failed to write stderr capture for {}", run.tool))?;
    }

    run.executed = true;
    run.finished_at = Some(Utc::now());
    run.success = Some(output.status.success());
    run.exit_code = output.status.code();

    if !output.status.success() {
        run.notes
            .push("Tool exited with a non-zero status.".to_string());
    }

    Ok(())
}

fn build_placeholder_app_map(
    scope: &ScopeDefinition,
    output: &OutputLayout,
) -> Result<AppMapDocument> {
    let assets = collect_assets(scope, output)?;
    let dns_records = load_dns_records(output)?;
    let screenshots = load_screenshot_records(output)?;
    let tech_fingerprints = load_tech_fingerprints(output)?;
    let (nodes, edges, notes) = assemble_placeholder_map(
        scope,
        &assets,
        &dns_records,
        &screenshots,
        &tech_fingerprints,
    );

    Ok(AppMapDocument {
        generated_at: Utc::now(),
        placeholder: true,
        notes,
        nodes,
        edges,
        assets,
        dns_records,
        screenshots,
        tech_fingerprints,
    })
}

fn collect_assets(scope: &ScopeDefinition, output: &OutputLayout) -> Result<Vec<ReconAsset>> {
    let mut assets = Vec::new();

    for target in scope.probe_targets() {
        assets.push(ReconAsset {
            asset: target.clone(),
            host: host_from_target(&target),
            source_tools: vec!["scope".to_string()],
            tags: vec!["scope".to_string()],
            notes: vec!["Derived directly from validated scope.".to_string()],
            live: false,
            first_seen: Some(Utc::now()),
        });
    }

    let live_hosts_path = output.raw.join("httpx").join("live-hosts.txt");
    if file_has_nonempty_lines(&live_hosts_path)? {
        for host in utils::read_trimmed_lines(&live_hosts_path)? {
            assets.push(ReconAsset {
                asset: host.clone(),
                host: host_from_target(&host),
                source_tools: vec!["httpx".to_string()],
                tags: vec!["live-host".to_string()],
                notes: vec!["Derived from httpx live-host output.".to_string()],
                live: true,
                first_seen: Some(Utc::now()),
            });
        }
    }

    Ok(assets)
}

fn load_dns_records(output: &OutputLayout) -> Result<Vec<DnsRecord>> {
    let path = output.dns.join("dnsx.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut records = Vec::new();
    for line in fs::read_to_string(&path)
        .with_context(|| format!("failed to read dnsx output at {}", path.display()))?
        .lines()
    {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let value = match serde_json::from_str::<Value>(line) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let name = json_string(&value, &["host", "input", "name"]);
        if name.is_empty() {
            continue;
        }

        let record_type = json_string(&value, &["type"]).if_empty_then("A".to_string());
        let values = json_array_strings(&value, &["a", "answers", "response"]);

        records.push(DnsRecord {
            name,
            record_type,
            values,
            source_tool: "dnsx".to_string(),
            notes: vec!["Parsed from dnsx JSONL output.".to_string()],
        });
    }

    Ok(records)
}

fn load_screenshot_records(output: &OutputLayout) -> Result<Vec<ScreenshotRecord>> {
    let mut records = Vec::new();
    let jsonl_path = output.screenshots.join("gowitness.jsonl");

    if jsonl_path.exists() {
        for line in fs::read_to_string(&jsonl_path)
            .with_context(|| {
                format!(
                    "failed to read gowitness output at {}",
                    jsonl_path.display()
                )
            })?
            .lines()
        {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let value = match serde_json::from_str::<Value>(line) {
                Ok(value) => value,
                Err(_) => continue,
            };

            let target = json_string(&value, &["url", "target", "final_url"]);
            if target.is_empty() {
                continue;
            }

            records.push(ScreenshotRecord {
                target,
                image_path: None,
                source_tool: "gowitness".to_string(),
                title: optional_json_string(&value, &["title"]),
                captured_at: None,
                notes: vec!["Parsed from gowitness JSONL output.".to_string()],
            });
        }
    }

    for entry in WalkDir::new(&output.screenshots)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let extension = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());

        if matches!(extension.as_deref(), Some("png" | "jpg" | "jpeg" | "webp")) {
            records.push(ScreenshotRecord {
                target: entry.file_name().to_string_lossy().to_string(),
                image_path: Some(entry.path().to_path_buf()),
                source_tool: "gowitness".to_string(),
                title: None,
                captured_at: None,
                notes: vec!["Discovered screenshot artifact on disk.".to_string()],
            });
        }
    }

    Ok(records)
}

fn load_tech_fingerprints(output: &OutputLayout) -> Result<Vec<TechFingerprint>> {
    let path = output.tech.join("whatweb.json");
    if !path.exists() {
        return Ok(Vec::new());
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read WhatWeb output at {}", path.display()))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let values = if trimmed.starts_with('[') {
        serde_json::from_str::<Vec<Value>>(trimmed).unwrap_or_default()
    } else if trimmed.starts_with('{') {
        vec![serde_json::from_str::<Value>(trimmed).unwrap_or_default()]
    } else {
        Vec::new()
    };

    let mut fingerprints = Vec::new();
    for value in values {
        if value.is_null() {
            continue;
        }

        let target = json_string(&value, &["target", "url", "host"]);
        if target.is_empty() {
            continue;
        }

        let technologies = json_object_keys(&value, &["plugins"]);
        fingerprints.push(TechFingerprint {
            target,
            url: optional_json_string(&value, &["url"]),
            technologies,
            categories: vec!["web-tech".to_string()],
            source_tool: "WhatWeb".to_string(),
            confidence: None,
            notes: vec!["Parsed from WhatWeb JSON output.".to_string()],
        });
    }

    Ok(fingerprints)
}

fn assemble_placeholder_map(
    scope: &ScopeDefinition,
    assets: &[ReconAsset],
    dns_records: &[DnsRecord],
    screenshots: &[ScreenshotRecord],
    tech_fingerprints: &[TechFingerprint],
) -> (Vec<AppMapNode>, Vec<AppMapEdge>, Vec<String>) {
    let mut nodes_by_id = BTreeMap::new();
    let mut edges = Vec::new();
    let mut notes = vec![
        "Placeholder app map generated from currently available scope and mapping artifacts."
            .to_string(),
        "This map is intentionally conservative and does not infer exploitation paths.".to_string(),
    ];

    if dns_records.is_empty() {
        notes.push(
            "DNS mapping data is not present yet; dnsx output will enrich this layer later."
                .to_string(),
        );
    }
    if screenshots.is_empty() {
        notes.push("Screenshot data is not present yet; gowitness output will enrich visual mapping later.".to_string());
    }
    if tech_fingerprints.is_empty() {
        notes.push("Technology fingerprint data is not present yet; WhatWeb output will enrich tech mapping later.".to_string());
    }

    for asset in assets {
        let node_id = format!("asset:{}", normalize_node_key(&asset.asset));
        nodes_by_id.insert(
            node_id.clone(),
            AppMapNode {
                id: node_id,
                label: asset.asset.clone(),
                kind: if asset.live {
                    "live-host".to_string()
                } else {
                    "scope-asset".to_string()
                },
                references: vec![asset.host.clone().unwrap_or_else(|| asset.asset.clone())],
                notes: asset.notes.clone(),
            },
        );
    }

    for record in dns_records {
        let node_id = format!(
            "dns:{}:{}",
            normalize_node_key(&record.name),
            normalize_node_key(&record.record_type)
        );
        nodes_by_id.insert(
            node_id.clone(),
            AppMapNode {
                id: node_id.clone(),
                label: format!("{} {}", record.name, record.record_type),
                kind: "dns-record".to_string(),
                references: record.values.clone(),
                notes: record.notes.clone(),
            },
        );

        let asset_id = format!("asset:{}", normalize_node_key(&record.name));
        edges.push(AppMapEdge {
            from: asset_id,
            to: node_id,
            relationship: "resolves-to".to_string(),
            notes: vec!["Derived from dnsx output.".to_string()],
        });
    }

    for fingerprint in tech_fingerprints {
        let node_id = format!("tech:{}", normalize_node_key(&fingerprint.target));
        nodes_by_id.insert(
            node_id.clone(),
            AppMapNode {
                id: node_id.clone(),
                label: if fingerprint.technologies.is_empty() {
                    format!("Fingerprint for {}", fingerprint.target)
                } else {
                    fingerprint.technologies.join(", ")
                },
                kind: "technology".to_string(),
                references: fingerprint
                    .url
                    .clone()
                    .map(|value| vec![value])
                    .unwrap_or_default(),
                notes: fingerprint.notes.clone(),
            },
        );

        edges.push(AppMapEdge {
            from: format!("asset:{}", normalize_node_key(&fingerprint.target)),
            to: node_id,
            relationship: "fingerprinted-as".to_string(),
            notes: vec!["Derived from WhatWeb output.".to_string()],
        });
    }

    for screenshot in screenshots {
        let node_id = format!("screenshot:{}", normalize_node_key(&screenshot.target));
        nodes_by_id.insert(
            node_id.clone(),
            AppMapNode {
                id: node_id.clone(),
                label: format!("Screenshot for {}", screenshot.target),
                kind: "screenshot".to_string(),
                references: screenshot
                    .image_path
                    .as_ref()
                    .map(|path| vec![path.display().to_string()])
                    .unwrap_or_default(),
                notes: screenshot.notes.clone(),
            },
        );

        edges.push(AppMapEdge {
            from: format!("asset:{}", normalize_node_key(&screenshot.target)),
            to: node_id,
            relationship: "captured-as".to_string(),
            notes: vec![
                "Screenshots touch target systems and must remain inside program rules."
                    .to_string(),
            ],
        });
    }

    if nodes_by_id.is_empty() {
        for target in scope.probe_targets() {
            let node_id = format!("asset:{}", normalize_node_key(&target));
            nodes_by_id.insert(
                node_id.clone(),
                AppMapNode {
                    id: node_id,
                    label: target,
                    kind: "scope-asset".to_string(),
                    references: Vec::new(),
                    notes: vec![
                        "Fallback node created from scope because no other artifacts were present."
                            .to_string(),
                    ],
                },
            );
        }
    }

    (nodes_by_id.into_values().collect(), edges, notes)
}

fn render_app_map_markdown(document: &AppMapDocument) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot App Map\n\n");
    output.push_str("- Mode: placeholder\n");
    output.push_str(&format!(
        "- Generated at: {}\n- Nodes: {}\n- Edges: {}\n\n",
        document.generated_at.to_rfc3339(),
        document.nodes.len(),
        document.edges.len()
    ));

    output.push_str("## Notes\n\n");
    for note in &document.notes {
        output.push_str(&format!("- {}\n", note));
    }

    output.push_str("\n## Nodes\n\n");
    if document.nodes.is_empty() {
        output.push_str("No nodes are available yet.\n");
    } else {
        for node in &document.nodes {
            output.push_str(&format!("- `{}` [{}] {}\n", node.id, node.kind, node.label));
        }
    }

    output.push_str("\n## Edges\n\n");
    if document.edges.is_empty() {
        output.push_str("No edges are available yet.\n");
    } else {
        for edge in &document.edges {
            output.push_str(&format!(
                "- `{}` -> `{}` ({})\n",
                edge.from, edge.to, edge.relationship
            ));
        }
    }

    output
}

fn file_has_nonempty_lines(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }

    Ok(!utils::read_trimmed_lines(path)?.is_empty())
}

fn host_from_target(target: &str) -> Option<String> {
    if target.starts_with("http://") || target.starts_with("https://") {
        return url::Url::parse(target)
            .ok()
            .and_then(|parsed| parsed.host_str().map(ToOwned::to_owned));
    }

    Some(target.trim_start_matches("*.").to_string())
}

fn json_string(value: &Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(found) = value.get(*key).and_then(|entry| entry.as_str()) {
            return found.to_string();
        }
    }

    String::new()
}

fn optional_json_string(value: &Value, keys: &[&str]) -> Option<String> {
    let found = json_string(value, keys);
    if found.is_empty() {
        None
    } else {
        Some(found)
    }
}

fn json_array_strings(value: &Value, keys: &[&str]) -> Vec<String> {
    for key in keys {
        if let Some(entry) = value.get(*key) {
            if let Some(array) = entry.as_array() {
                return array
                    .iter()
                    .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                    .collect();
            }

            if let Some(string) = entry.as_str() {
                return vec![string.to_string()];
            }
        }
    }

    Vec::new()
}

fn json_object_keys(value: &Value, keys: &[&str]) -> Vec<String> {
    for key in keys {
        if let Some(object) = value.get(*key).and_then(|entry| entry.as_object()) {
            return object.keys().cloned().collect();
        }
    }

    Vec::new()
}

fn normalize_node_key(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

fn resolve_binary_path(binary_name: &str) -> Option<PathBuf> {
    let candidate = Path::new(binary_name);
    if candidate.components().count() > 1 && candidate.is_file() {
        return Some(candidate.to_path_buf());
    }

    let path_value = env::var_os("PATH")?;
    let path_exts = windows_path_extensions();

    for directory in env::split_paths(&path_value) {
        let direct_candidate = directory.join(binary_name);
        if direct_candidate.is_file() {
            return Some(direct_candidate);
        }

        if direct_candidate.extension().is_none() {
            for extension in &path_exts {
                let candidate = directory.join(format!("{binary_name}{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

fn windows_path_extensions() -> Vec<String> {
    let default = OsString::from(".COM;.EXE;.BAT;.CMD");
    env::var_os("PATHEXT")
        .unwrap_or(default)
        .to_string_lossy()
        .split(';')
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn render_command(program: &str, arguments: &[String]) -> String {
    let mut parts = Vec::with_capacity(arguments.len() + 1);
    parts.push(quote_argument(program));
    parts.extend(arguments.iter().map(|argument| quote_argument(argument)));
    parts.join(" ")
}

fn quote_argument(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_string();
    }

    if value
        .chars()
        .any(|character| matches!(character, ' ' | '\t' | '"'))
    {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

trait IfEmptyThen {
    fn if_empty_then(self, fallback: String) -> String;
}

impl IfEmptyThen for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.is_empty() {
            fallback
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use super::run_mapping_layer;
    use crate::{scope::load_scope, utils::ensure_output_structure};

    struct TestWorkspace {
        root: PathBuf,
    }

    impl TestWorkspace {
        fn new(label: &str) -> Result<Self> {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "reconpilot-map-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            Ok(Self { root })
        }

        fn path(&self) -> &std::path::Path {
            &self.root
        }

        fn write_scope(&self, content: &str) -> Result<PathBuf> {
            let path = self.root.join("scope.txt");
            fs::write(&path, content)?;
            Ok(path)
        }

        fn write_live_hosts(&self, output_root: &std::path::Path, content: &str) -> Result<()> {
            let live_hosts = output_root.join("raw").join("httpx").join("live-hosts.txt");
            if let Some(parent) = live_hosts.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(live_hosts, content)?;
            Ok(())
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[tokio::test]
    async fn mapping_output_directory_creation() -> Result<()> {
        let workspace = TestWorkspace::new("dirs")?;
        let scope_path = workspace.write_scope("example.com\n")?;
        let scope = load_scope(&scope_path)?;
        let output = ensure_output_structure(&workspace.path().join("output"))?;

        let _outcome = run_mapping_layer(&scope, &output, false).await?;
        assert!(output.dns.exists());
        assert!(output.screenshots.exists());
        assert!(output.tech.exists());
        assert!(output.maps.exists());
        assert!(output.plans.exists());
        Ok(())
    }

    #[tokio::test]
    async fn dry_run_mapping_command_generation() -> Result<()> {
        let workspace = TestWorkspace::new("dryrun")?;
        let scope_path = workspace.write_scope("example.com\nhttps://portal.example.org\n")?;
        let scope = load_scope(&scope_path)?;
        let output = ensure_output_structure(&workspace.path().join("output"))?;
        workspace.write_live_hosts(
            &output.root,
            "https://example.com\nhttps://portal.example.org\n",
        )?;

        let outcome = run_mapping_layer(&scope, &output, false).await?;
        assert_eq!(outcome.runs.len(), 3);
        assert!(output.plans.join("dnsx-plan.json").exists());
        assert!(output.plans.join("gowitness-plan.json").exists());
        assert!(output.plans.join("whatweb-plan.json").exists());
        assert!(outcome.runs.iter().all(|run| !run.executed));
        Ok(())
    }

    #[tokio::test]
    async fn missing_live_host_input_behavior() -> Result<()> {
        let workspace = TestWorkspace::new("missing-httpx")?;
        let scope_path = workspace.write_scope("example.com\n")?;
        let scope = load_scope(&scope_path)?;
        let output = ensure_output_structure(&workspace.path().join("output"))?;

        let outcome = run_mapping_layer(&scope, &output, false).await?;
        let gowitness = outcome
            .runs
            .iter()
            .find(|run| run.tool == "gowitness")
            .expect("gowitness plan should exist");
        let whatweb = outcome
            .runs
            .iter()
            .find(|run| run.tool == "WhatWeb")
            .expect("WhatWeb plan should exist");

        assert!(gowitness
            .notes
            .iter()
            .any(|note| note.contains("Live-host input is missing or empty")));
        assert!(whatweb
            .notes
            .iter()
            .any(|note| note.contains("Live-host input is missing or empty")));
        assert!(!gowitness.executed);
        assert!(!whatweb.executed);
        Ok(())
    }

    #[tokio::test]
    async fn app_map_placeholder_generation() -> Result<()> {
        let workspace = TestWorkspace::new("map-output")?;
        let scope_path = workspace.write_scope("example.com\napi.example.com\n")?;
        let scope = load_scope(&scope_path)?;
        let output = ensure_output_structure(&workspace.path().join("output"))?;

        let outcome = run_mapping_layer(&scope, &output, false).await?;
        assert!(outcome.map_json_path.exists());
        assert!(outcome.map_markdown_path.exists());

        let json = fs::read_to_string(&outcome.map_json_path)?;
        let markdown = fs::read_to_string(&outcome.map_markdown_path)?;
        assert!(json.contains("\"placeholder\": true"));
        assert!(json.contains("scope-asset"));
        assert!(markdown.contains("ReconPilot App Map"));
        Ok(())
    }
}
