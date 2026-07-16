use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use tokio::process::Command;

use crate::{
    config::ReconPilotConfig,
    models::ReconToolRun,
    scope::ScopeDefinition,
    utils::{self, OutputLayout},
};

#[derive(Debug, Clone)]
pub struct PlannedPhase {
    pub name: String,
    pub tools: Vec<String>,
    pub active: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub scope_targets: Vec<String>,
    pub output_root: String,
    pub phases: Vec<PlannedPhase>,
}

#[derive(Debug, Clone)]
struct AdapterDefinition {
    order: usize,
    tool: &'static str,
    binary_name: &'static str,
    build: fn(&ScopeDefinition, &OutputLayout) -> Result<AdapterCommand>,
}

#[derive(Debug, Clone)]
struct AdapterCommand {
    phase: String,
    program: String,
    arguments: Vec<String>,
    planned_inputs: Vec<PathBuf>,
    output_files: Vec<PathBuf>,
    primary_output_path: Option<PathBuf>,
    stdout_path: Option<PathBuf>,
    stderr_path: Option<PathBuf>,
    notes: Vec<String>,
}

pub fn build_execution_plan(
    scope: &ScopeDefinition,
    config: &ReconPilotConfig,
    output: &OutputLayout,
) -> ExecutionPlan {
    let mut phases = vec![
        PlannedPhase {
            name: "Scope validation".to_string(),
            tools: vec!["native".to_string()],
            active: false,
            detail: "Validate scope before any command generation or execution.".to_string(),
        },
        PlannedPhase {
            name: "Phase 1 adapters".to_string(),
            tools: vec![
                "subfinder".to_string(),
                "httpx".to_string(),
                "katana".to_string(),
                "gau".to_string(),
            ],
            active: true,
            detail: "Generate safe dry-run command plans for the first supported tool adapters."
                .to_string(),
        },
        PlannedPhase {
            name: "Normalization".to_string(),
            tools: vec!["uro".to_string(), "jq".to_string(), "native".to_string()],
            active: false,
            detail: "Canonicalize and deduplicate collected records.".to_string(),
        },
        PlannedPhase {
            name: "Enrichment".to_string(),
            tools: vec!["native".to_string()],
            active: false,
            detail: "Attach metadata, route families, and classification hints.".to_string(),
        },
        PlannedPhase {
            name: "LLM scoring".to_string(),
            tools: vec!["native".to_string()],
            active: false,
            detail: "Future stage for explainable prioritization and analyst assistance."
                .to_string(),
        },
        PlannedPhase {
            name: "Reports".to_string(),
            tools: vec!["native".to_string()],
            active: false,
            detail: "Render machine-readable and analyst-readable summaries.".to_string(),
        },
    ];

    if config.passive_only {
        for phase in &mut phases {
            if phase.active {
                phase
                    .detail
                    .push_str(" `run` still defaults to dry-run unless `--execute` is passed.");
            }
        }
    }

    ExecutionPlan {
        scope_targets: scope.probe_targets(),
        output_root: output.root.display().to_string(),
        phases,
    }
}

pub fn planned_command_previews(plan: &ExecutionPlan) -> Vec<String> {
    vec![
        format!(
            "subfinder -dL {}/plans/subfinder-input.txt -silent -o {}/raw/subfinder/subdomains.txt",
            plan.output_root, plan.output_root
        ),
        format!(
            "httpx -l {}/plans/httpx-input.txt -silent -o {}/raw/httpx/live-hosts.txt",
            plan.output_root, plan.output_root
        ),
        format!(
            "katana -list {}/plans/katana-input.txt -jsonl -o {}/raw/katana/katana.jsonl",
            plan.output_root, plan.output_root
        ),
        format!(
            "gau --subs --json --o {}/raw/gau/gau.jsonl <scope-domains...>",
            plan.output_root
        ),
    ]
}

pub fn print_execution_plan(plan: &ExecutionPlan) {
    println!("ReconPilot execution plan");
    println!("Scope targets: {}", plan.scope_targets.join(", "));
    println!("Output root: {}", plan.output_root);
    println!(
        "Run mode: dry-run by default; use `reconpilot run --execute` to actually launch adapters."
    );
    println!();

    for (index, phase) in plan.phases.iter().enumerate() {
        println!(
            "{}. {} [{}]",
            index + 1,
            phase.name,
            if phase.active {
                "active-capable"
            } else {
                "passive"
            }
        );
        println!("   Tools : {}", phase.tools.join(", "));
        println!("   Detail: {}", phase.detail);
    }

    println!();
    println!("Phase 1 command previews:");
    for preview in planned_command_previews(plan) {
        println!("  - {}", preview);
    }
}

pub async fn run_tool_adapters(
    scope: &ScopeDefinition,
    output: &OutputLayout,
    execute: bool,
) -> Result<Vec<ReconToolRun>> {
    validate_scope_for_tooling(scope)?;

    let mut runs = Vec::new();
    let mut had_execution_errors = false;

    for adapter in adapter_registry() {
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
            if run.binary_exists {
                if let Err(error) = execute_tool_run(&mut run).await {
                    had_execution_errors = true;
                    run.success = Some(false);
                    run.notes.push(format!("Execution error: {error:#}"));
                }
            } else {
                had_execution_errors = true;
                run.notes
                    .push("Binary not found on PATH; command was not executed.".to_string());
            }
        } else {
            run.notes
                .push("Dry-run mode: command was planned but not executed.".to_string());
        }

        write_plan_files(output, adapter.order, &run)?;
        runs.push(run);
    }

    utils::write_json_pretty(&output.plans.join("tool-runs.json"), &runs)?;

    if execute && had_execution_errors {
        bail!(
            "one or more tool adapters could not execute successfully; inspect {} and {}",
            output.plans.display(),
            output.raw.display()
        );
    }

    Ok(runs)
}

pub fn print_tool_run_summary(
    scope: &ScopeDefinition,
    config: &ReconPilotConfig,
    output: &OutputLayout,
    runs: &[ReconToolRun],
    execute: bool,
) {
    println!("ReconPilot run summary");
    println!("Scope source: {}", scope.source_path.display());
    println!("Scope targets: {}", scope.probe_targets().join(", "));
    println!("Profile: {}", config.profile_name);
    println!("Output root: {}", output.root.display());
    println!(
        "Mode: {}",
        if execute {
            "execute"
        } else {
            "dry-run (default)"
        }
    );
    println!("Safety notice: external recon adapters only launch when --execute is passed.");
    println!("Plans: {}", output.plans.display());
    println!("Raw output: {}", output.raw.display());
    println!();

    for run in runs {
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
        if let Some(code) = run.exit_code {
            println!("  Exit code: {}", code);
        }
        if !run.notes.is_empty() {
            println!("  Notes: {}", run.notes.join(" | "));
        }
    }
}

fn adapter_registry() -> [AdapterDefinition; 4] {
    [
        AdapterDefinition {
            order: 1,
            tool: "subfinder",
            binary_name: "subfinder",
            build: build_subfinder_command,
        },
        AdapterDefinition {
            order: 2,
            tool: "httpx",
            binary_name: "httpx",
            build: build_httpx_command,
        },
        AdapterDefinition {
            order: 3,
            tool: "katana",
            binary_name: "katana",
            build: build_katana_command,
        },
        AdapterDefinition {
            order: 4,
            tool: "gau",
            binary_name: "gau",
            build: build_gau_command,
        },
    ]
}

fn validate_scope_for_tooling(scope: &ScopeDefinition) -> Result<()> {
    if scope.is_empty() {
        bail!("scope validation failed: scope entries cannot be empty");
    }

    if scope.domain_targets().is_empty() {
        bail!("scope validation failed: no domain or URL targets could be derived");
    }

    Ok(())
}

fn build_subfinder_command(
    scope: &ScopeDefinition,
    output: &OutputLayout,
) -> Result<AdapterCommand> {
    validate_scope_for_tooling(scope)?;

    let domains = scope.domain_targets();
    let input_file = output.plans.join("subfinder-input.txt");
    utils::write_lines(&input_file, &domains)?;

    let tool_dir = output.raw.join("subfinder");
    utils::ensure_directory(&tool_dir)?;

    let output_file = tool_dir.join("subdomains.txt");
    let stdout_file = tool_dir.join("stdout.txt");
    let stderr_file = tool_dir.join("stderr.txt");

    Ok(AdapterCommand {
        phase: "subdomain-discovery".to_string(),
        program: "subfinder".to_string(),
        arguments: vec![
            "-dL".to_string(),
            input_file.display().to_string(),
            "-silent".to_string(),
            "-o".to_string(),
            output_file.display().to_string(),
        ],
        planned_inputs: vec![input_file],
        output_files: vec![
            output_file.clone(),
            stdout_file.clone(),
            stderr_file.clone(),
        ],
        primary_output_path: Some(output_file),
        stdout_path: Some(stdout_file),
        stderr_path: Some(stderr_file),
        notes: vec![
            "Wildcard scope entries are reduced to their base domains for subfinder input."
                .to_string(),
        ],
    })
}

fn build_httpx_command(scope: &ScopeDefinition, output: &OutputLayout) -> Result<AdapterCommand> {
    validate_scope_for_tooling(scope)?;

    let targets = scope.probe_targets();
    let input_file = output.plans.join("httpx-input.txt");
    utils::write_lines(&input_file, &targets)?;

    let tool_dir = output.raw.join("httpx");
    utils::ensure_directory(&tool_dir)?;

    let output_file = tool_dir.join("live-hosts.txt");
    let stdout_file = tool_dir.join("stdout.txt");
    let stderr_file = tool_dir.join("stderr.txt");

    Ok(AdapterCommand {
        phase: "live-host-probing".to_string(),
        program: "httpx".to_string(),
        arguments: vec![
            "-l".to_string(),
            input_file.display().to_string(),
            "-silent".to_string(),
            "-o".to_string(),
            output_file.display().to_string(),
        ],
        planned_inputs: vec![input_file],
        output_files: vec![
            output_file.clone(),
            stdout_file.clone(),
            stderr_file.clone(),
        ],
        primary_output_path: Some(output_file),
        stdout_path: Some(stdout_file),
        stderr_path: Some(stderr_file),
        notes: vec![
            "Phase 1 keeps httpx output line-oriented so later adapters can reuse it.".to_string(),
        ],
    })
}

fn build_katana_command(scope: &ScopeDefinition, output: &OutputLayout) -> Result<AdapterCommand> {
    validate_scope_for_tooling(scope)?;

    let targets = scope.crawl_targets();
    let input_file = output.plans.join("katana-input.txt");
    utils::write_lines(&input_file, &targets)?;

    let tool_dir = output.raw.join("katana");
    utils::ensure_directory(&tool_dir)?;

    let output_file = tool_dir.join("katana.jsonl");
    let stdout_file = tool_dir.join("stdout.txt");
    let stderr_file = tool_dir.join("stderr.txt");

    Ok(AdapterCommand {
        phase: "crawling".to_string(),
        program: "katana".to_string(),
        arguments: vec![
            "-list".to_string(),
            input_file.display().to_string(),
            "-jsonl".to_string(),
            "-o".to_string(),
            output_file.display().to_string(),
        ],
        planned_inputs: vec![input_file],
        output_files: vec![
            output_file.clone(),
            stdout_file.clone(),
            stderr_file.clone(),
        ],
        primary_output_path: Some(output_file),
        stdout_path: Some(stdout_file),
        stderr_path: Some(stderr_file),
        notes: vec![
            "URL scope entries are preserved; domain entries become HTTPS seeds for katana."
                .to_string(),
        ],
    })
}

fn build_gau_command(scope: &ScopeDefinition, output: &OutputLayout) -> Result<AdapterCommand> {
    validate_scope_for_tooling(scope)?;

    let domains = scope.domain_targets();
    let input_file = output.plans.join("gau-input.txt");
    utils::write_lines(&input_file, &domains)?;

    let tool_dir = output.raw.join("gau");
    utils::ensure_directory(&tool_dir)?;

    let output_file = tool_dir.join("gau.jsonl");
    let stdout_file = tool_dir.join("stdout.txt");
    let stderr_file = tool_dir.join("stderr.txt");

    let mut arguments = vec![
        "--subs".to_string(),
        "--json".to_string(),
        "--o".to_string(),
        output_file.display().to_string(),
    ];
    arguments.extend(domains.iter().cloned());

    Ok(AdapterCommand {
        phase: "historical-urls".to_string(),
        program: "gau".to_string(),
        arguments,
        planned_inputs: vec![input_file],
        output_files: vec![
            output_file.clone(),
            stdout_file.clone(),
            stderr_file.clone(),
        ],
        primary_output_path: Some(output_file),
        stdout_path: Some(stdout_file),
        stderr_path: Some(stderr_file),
        notes: vec![
            "gau receives scope-derived domains as explicit positional arguments.".to_string(),
        ],
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

fn write_plan_files(output: &OutputLayout, order: usize, run: &ReconToolRun) -> Result<()> {
    let prefix = format!("{order:02}-{}", run.tool);
    utils::write_string(
        &output.plans.join(format!("{prefix}.cmd.txt")),
        &run.command_line,
    )?;
    utils::write_json_pretty(&output.plans.join(format!("{prefix}.json")), run)?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use super::run_tool_adapters;
    use crate::{
        scope::{load_scope, ScopeDefinition},
        utils::ensure_output_structure,
    };

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
                "reconpilot-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            Ok(Self { root })
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn write_scope(&self, content: &str) -> Result<PathBuf> {
            let path = self.root.join("scope.txt");
            fs::write(&path, content)?;
            Ok(path)
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[tokio::test]
    async fn empty_scope_rejection() -> Result<()> {
        let workspace = TestWorkspace::new("empty-scope")?;
        let output = ensure_output_structure(&workspace.path().join("output"))?;
        let empty_scope = ScopeDefinition {
            source_path: workspace.path().join("scope.txt"),
            entries: Vec::new(),
        };

        let result = run_tool_adapters(&empty_scope, &output, false).await;
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn command_generation_creates_plan_files() -> Result<()> {
        let workspace = TestWorkspace::new("command-generation")?;
        let scope_path = workspace
            .write_scope("example.com\n*.corp-example.net\nhttps://portal.example.org\n")?;
        let scope = load_scope(&scope_path)?;
        let output = ensure_output_structure(&workspace.path().join("output"))?;

        let runs = run_tool_adapters(&scope, &output, false).await?;
        assert_eq!(runs.len(), 4);

        let subfinder = runs.iter().find(|run| run.tool == "subfinder").unwrap();
        assert_eq!(subfinder.program, "subfinder");
        assert!(subfinder.arguments.iter().any(|argument| argument == "-dL"));
        assert!(subfinder.command_line.contains("subfinder"));
        assert!(subfinder.command_line.contains("subfinder-input.txt"));

        assert!(output.plans.join("01-subfinder.cmd.txt").exists());
        assert!(output.plans.join("02-httpx.cmd.txt").exists());
        assert!(output.plans.join("03-katana.cmd.txt").exists());
        assert!(output.plans.join("04-gau.cmd.txt").exists());
        assert!(output.plans.join("tool-runs.json").exists());
        Ok(())
    }

    #[tokio::test]
    async fn dry_run_does_not_execute_tools() -> Result<()> {
        let workspace = TestWorkspace::new("dry-run")?;
        let scope_path = workspace.write_scope("example.com\nhttps://portal.example.org\n")?;
        let scope = load_scope(&scope_path)?;
        let output = ensure_output_structure(&workspace.path().join("output"))?;

        let runs = run_tool_adapters(&scope, &output, false).await?;
        assert!(runs.iter().all(|run| !run.execute_requested));
        assert!(runs.iter().all(|run| !run.executed));
        assert!(runs.iter().all(|run| run
            .stdout_path
            .as_ref()
            .map(|path| !path.exists())
            .unwrap_or(true)));
        assert!(runs.iter().all(|run| run
            .stderr_path
            .as_ref()
            .map(|path| !path.exists())
            .unwrap_or(true)));
        Ok(())
    }

    #[tokio::test]
    async fn output_directory_creation_for_phase_one() -> Result<()> {
        let workspace = TestWorkspace::new("output-layout")?;
        let scope_path = workspace.write_scope("example.com\n")?;
        let scope = load_scope(&scope_path)?;
        let output = ensure_output_structure(&workspace.path().join("output"))?;

        let _runs = run_tool_adapters(&scope, &output, false).await?;
        assert!(output.raw.exists());
        assert!(output.plans.exists());
        assert!(output.raw.join("subfinder").exists());
        assert!(output.raw.join("httpx").exists());
        assert!(output.raw.join("katana").exists());
        assert!(output.raw.join("gau").exists());
        Ok(())
    }
}
