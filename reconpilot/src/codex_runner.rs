use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use regex::Regex;
use serde::Deserialize;

use crate::{
    models::{
        CodexInsightResult, CodexRunItem, CodexRunPlan, CodexRunnerSummary, GraphSummary,
        LlmAssetContext, LlmContextPack, LlmReasoningItem, ReviewItem,
    },
    utils,
};

#[derive(Debug, Clone)]
pub struct CodexRunnerOutcome {
    pub plan_json_path: PathBuf,
    pub plan_markdown_path: PathBuf,
    pub results_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub summary_markdown_path: PathBuf,
    pub summary_json_path: PathBuf,
    pub summary: CodexRunnerSummary,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ReasoningQueueDocument {
    #[serde(default)]
    items: Vec<LlmReasoningItem>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PriorityQueueDocument {
    #[serde(default)]
    items: Vec<ReviewItem>,
}

#[derive(Debug, Clone, Default)]
struct SupportingArtifacts {
    review_items: BTreeMap<String, ReviewItem>,
    enrichment_summary: Option<String>,
    api_summary: Option<String>,
    graph_summary: Option<GraphSummary>,
}

#[derive(Debug, Clone)]
struct TemplateMaterial {
    file_name: String,
    markdown: String,
}

#[derive(Debug, Clone)]
struct PreparedPrompt {
    plan_item: CodexRunItem,
    prompt: String,
    template_file: String,
    result_stem: String,
}

#[derive(Debug, Clone)]
struct ExecutionCapture {
    stdout: String,
    stderr: String,
    exit_status: Option<i32>,
    success: bool,
}

pub fn run_codex_runner(
    pack: &Path,
    out: &Path,
    execute_codex: bool,
    limit: usize,
    template_filter: Option<String>,
) -> Result<CodexRunnerOutcome> {
    run_codex_runner_with_executor(
        pack,
        out,
        execute_codex,
        limit,
        template_filter,
        None,
        default_codex_executor,
    )
}

pub fn print_codex_runner_summary(pack: &Path, out: &Path, outcome: &CodexRunnerOutcome) {
    println!("ReconPilot Codex runner summary");
    println!("LLM pack: {}", pack.display());
    println!("Output root: {}", out.display());
    println!(
        "Mode: {}",
        if outcome.summary.execute_requested {
            "execute-codex"
        } else {
            "plan-only (default)"
        }
    );
    println!(
        "Codex CLI available: {} | Planned items: {} | Executed: {}",
        outcome.summary.codex_available,
        outcome.summary.planned_count,
        outcome.summary.executed_count
    );
    println!(
        "Successes: {} | Failures: {}",
        outcome.summary.success_count, outcome.summary.failure_count
    );
    println!("Safety notice: Codex reasoning is optional, local prompt execution only, and never contacts targets.");
    println!("Outputs:");
    println!("  - {}", outcome.plan_json_path.display());
    println!("  - {}", outcome.plan_markdown_path.display());
    println!("  - {}", outcome.results_dir.display());
    println!("  - {}", outcome.logs_dir.display());
    println!("  - {}", outcome.summary_markdown_path.display());
    println!("  - {}", outcome.summary_json_path.display());
    for warning in &outcome.summary.warnings {
        println!("Warning: {warning}");
    }
}

fn run_codex_runner_with_executor<F>(
    pack: &Path,
    out: &Path,
    execute_codex: bool,
    limit: usize,
    template_filter: Option<String>,
    binary_override: Option<PathBuf>,
    mut executor: F,
) -> Result<CodexRunnerOutcome>
where
    F: FnMut(&Path, &str) -> Result<ExecutionCapture>,
{
    if limit == 0 {
        bail!("codex-run limit must be greater than zero");
    }

    validate_pack_input(pack)?;
    utils::ensure_directory(out)?;
    let plans_dir = out.join("plans");
    let results_dir = out.join("results");
    let logs_dir = out.join("logs");
    utils::ensure_directory(&plans_dir)?;
    utils::ensure_directory(&results_dir)?;
    utils::ensure_directory(&logs_dir)?;

    let mut warnings = Vec::new();
    let context_pack = load_json::<LlmContextPack>(&pack.join("pack-summary.json"))?;
    let reasoning_queue = load_reasoning_queue(pack)?;
    let templates = load_templates(pack, &context_pack)?;
    let support = load_supporting_artifacts(pack, &mut warnings)?;
    let max_prompt_chars = context_pack.max_context_chars.max(1_000);
    let binary_path = resolve_codex_binary(binary_override.as_deref());
    let codex_available = binary_path.is_some();

    if !codex_available {
        warnings.push(format!(
            "Codex CLI was not found on PATH{}. Plan artifacts were still generated.",
            binary_override
                .as_ref()
                .map(|path| format!(" or at {}", path.display()))
                .unwrap_or_default()
        ));
    }

    if execute_codex && !codex_available {
        bail!(
            "Codex CLI is unavailable. Install or expose `codex` on PATH before using `--execute-codex`."
        );
    }

    let prepared = prepare_prompts(
        pack,
        &reasoning_queue,
        &templates,
        &support,
        limit,
        template_filter.clone(),
        max_prompt_chars,
    )?;
    let mut plan_items = prepared
        .iter()
        .map(|prepared| prepared.plan_item.clone())
        .collect::<Vec<_>>();

    let mut results = Vec::new();
    if execute_codex {
        let binary_path = binary_path
            .as_deref()
            .context("Codex CLI availability should have been checked earlier")?;
        for (index, prepared_item) in prepared.iter().enumerate() {
            let (result, executed) = execute_prepared_item(
                binary_path,
                prepared_item,
                &results_dir,
                &logs_dir,
                &mut executor,
            )?;
            if let Some(item) = plan_items.get_mut(index) {
                item.executed = executed;
                item.warnings = result.warnings.clone();
            }
            results.push(result);
        }
    }

    let plan = CodexRunPlan {
        generated_at: Utc::now(),
        pack_path: pack.display().to_string(),
        output_root: out.display().to_string(),
        execute_requested: execute_codex,
        codex_available,
        max_prompt_chars,
        limit,
        template_filter: template_filter.clone(),
        items: plan_items.clone(),
        warnings: warnings.clone(),
    };
    let plan_json_path = plans_dir.join("codex-command-plan.json");
    let plan_markdown_path = plans_dir.join("codex-command-plan.md");
    utils::write_json_pretty(&plan_json_path, &plan)?;
    utils::write_string(&plan_markdown_path, &render_plan_markdown(&plan))?;

    let summary = build_summary(
        pack,
        out,
        execute_codex,
        codex_available,
        max_prompt_chars,
        limit,
        template_filter,
        &warnings,
        &results,
        prepared.len(),
    );
    let summary_json_path = out.join("codex-summary.json");
    let summary_markdown_path = out.join("codex-summary.md");
    utils::write_json_pretty(&summary_json_path, &summary)?;
    utils::write_string(
        &summary_markdown_path,
        &render_summary_markdown(&summary, &plan_items, &support),
    )?;

    Ok(CodexRunnerOutcome {
        plan_json_path,
        plan_markdown_path,
        results_dir,
        logs_dir,
        summary_markdown_path,
        summary_json_path,
        summary,
    })
}

fn validate_pack_input(pack: &Path) -> Result<()> {
    if !pack.exists() {
        bail!(
            "codex-run pack directory does not exist: {}",
            pack.display()
        );
    }
    if !pack.is_dir() {
        bail!("codex-run pack path is not a directory: {}", pack.display());
    }

    for relative in [
        "reasoning-queue.json",
        "pack-summary.json",
        "asset-contexts",
        "prompts",
    ] {
        let path = pack.join(relative);
        if !path.exists() {
            bail!(
                "required codex-run input is missing: {}. Run `reconpilot llm-pack --input output/ --out output/llm-pack/` first.",
                path.display()
            );
        }
    }

    Ok(())
}

fn load_reasoning_queue(pack: &Path) -> Result<Vec<LlmReasoningItem>> {
    let document = load_json::<ReasoningQueueDocument>(&pack.join("reasoning-queue.json"))?;
    Ok(document.items)
}

fn load_templates(
    pack: &Path,
    context_pack: &LlmContextPack,
) -> Result<BTreeMap<String, TemplateMaterial>> {
    let mut templates = BTreeMap::new();

    for prompt in &context_pack.prompts {
        let file_name = prompt.file_name.clone();
        let path = pack.join("prompts").join(&file_name);
        let markdown = if path.exists() {
            fs::read_to_string(&path).with_context(|| {
                format!(
                    "failed to read llm-pack prompt template: {}",
                    path.display()
                )
            })?
        } else {
            prompt.template_markdown.clone()
        };

        let material = TemplateMaterial {
            file_name: file_name.clone(),
            markdown,
        };
        templates.insert(prompt.name.clone(), material.clone());
        templates.insert(file_name.clone(), material.clone());
        templates.insert(file_stem(&file_name), material);
    }

    if templates.is_empty() {
        bail!(
            "no prompt templates were loaded from {}; run `reconpilot llm-pack` again.",
            pack.join("prompts").display()
        );
    }

    Ok(templates)
}

fn load_supporting_artifacts(
    pack: &Path,
    warnings: &mut Vec<String>,
) -> Result<SupportingArtifacts> {
    let Some(output_root) = pack.parent() else {
        return Ok(SupportingArtifacts::default());
    };

    let mut support = SupportingArtifacts::default();

    let review_path = output_root.join("review").join("priority-queue.json");
    if review_path.exists() {
        let review = load_json::<PriorityQueueDocument>(&review_path)?;
        support.review_items = review
            .items
            .into_iter()
            .map(|item| (item.asset.clone(), item))
            .collect();
    } else {
        warnings.push(format!(
            "Optional review artifact missing for Codex context enrichment: {}",
            review_path.display()
        ));
    }

    let enrichment_summary_path = output_root.join("enrichment").join("enrichment-summary.md");
    if enrichment_summary_path.exists() {
        support.enrichment_summary = Some(
            fs::read_to_string(&enrichment_summary_path).with_context(|| {
                format!(
                    "failed to read enrichment summary for Codex context: {}",
                    enrichment_summary_path.display()
                )
            })?,
        );
    } else {
        warnings.push(format!(
            "Optional enrichment summary missing for Codex context enrichment: {}",
            enrichment_summary_path.display()
        ));
    }

    let api_summary_path = output_root.join("api-intel").join("api-summary.md");
    if api_summary_path.exists() {
        support.api_summary = Some(fs::read_to_string(&api_summary_path).with_context(|| {
            format!(
                "failed to read API summary for Codex context: {}",
                api_summary_path.display()
            )
        })?);
    } else {
        warnings.push(format!(
            "Optional API summary missing for Codex context enrichment: {}",
            api_summary_path.display()
        ));
    }

    let graph_summary_path = output_root.join("maps").join("graph-summary.json");
    if graph_summary_path.exists() {
        support.graph_summary = Some(load_json::<GraphSummary>(&graph_summary_path)?);
    } else {
        warnings.push(format!(
            "Optional graph summary missing for Codex context enrichment: {}",
            graph_summary_path.display()
        ));
    }

    Ok(support)
}

fn prepare_prompts(
    pack: &Path,
    reasoning_queue: &[LlmReasoningItem],
    templates: &BTreeMap<String, TemplateMaterial>,
    support: &SupportingArtifacts,
    limit: usize,
    template_filter: Option<String>,
    max_prompt_chars: usize,
) -> Result<Vec<PreparedPrompt>> {
    let selected_template = if let Some(value) = template_filter.as_deref() {
        Some(resolve_template(templates, value)?.with_context(|| {
            format!(
                "prompt template '{}' could not be resolved from the llm-pack prompts",
                value
            )
        })?)
    } else {
        None
    };

    let mut prepared = Vec::new();
    for item in reasoning_queue.iter().take(limit) {
        let template = if let Some(template) = selected_template.clone() {
            template
        } else {
            choose_template_for_item(item, templates).with_context(|| {
                format!(
                    "reasoning item '{}' references missing prompt template '{}'",
                    item.asset, item.suggested_prompt_template
                )
            })?
        };

        let context_path = pack.join(&item.context_file);
        let mut context = load_json::<LlmAssetContext>(&context_path)?;
        context.evidence_refs = dedupe_strings(context.evidence_refs);
        context.evidence_highlights = dedupe_strings(context.evidence_highlights);
        let (prompt, prompt_warnings) =
            build_prompt(&context, item, &template, support, max_prompt_chars);
        let result_stem = format!(
            "{:03}-{}",
            item.rank,
            sanitize_asset_filename_stem(&item.asset)
        );
        let command_line = render_command("codex", &["exec".to_string(), prompt.clone()]);

        prepared.push(PreparedPrompt {
            plan_item: CodexRunItem {
                rank: item.rank,
                asset: item.asset.clone(),
                template: template.file_name.clone(),
                context_file: item.context_file.clone(),
                prompt_chars: prompt.chars().count(),
                codex_command: command_line,
                executed: false,
                evidence_refs: dedupe_strings(item.evidence_refs.clone()),
                why_selected: item.why_llm_review.clone(),
                warnings: prompt_warnings,
            },
            prompt,
            template_file: template.file_name.clone(),
            result_stem,
        });
    }

    Ok(prepared)
}

fn execute_prepared_item<F>(
    binary_path: &Path,
    prepared: &PreparedPrompt,
    results_dir: &Path,
    logs_dir: &Path,
    executor: &mut F,
) -> Result<(CodexInsightResult, bool)>
where
    F: FnMut(&Path, &str) -> Result<ExecutionCapture>,
{
    let stdout_log = logs_dir.join("codex-stdout.log");
    let stderr_log = logs_dir.join("codex-stderr.log");
    let result_md_path = results_dir.join(format!("{}.md", prepared.result_stem));
    let sidecar_path = results_dir.join(format!("{}.json", prepared.result_stem));
    let mut warnings = prepared.plan_item.warnings.clone();

    let capture = match executor(binary_path, &prepared.prompt) {
        Ok(capture) => capture,
        Err(error) => {
            warnings.push(format!("Execution error: {error:#}"));
            let content = render_failure_markdown(&prepared.plan_item.asset, &warnings);
            utils::write_string(&result_md_path, &content)?;
            append_log_section(
                &stderr_log,
                &prepared.plan_item.asset,
                &format!("Execution error: {error:#}"),
            )?;
            let sidecar = CodexInsightResult {
                asset: prepared.plan_item.asset.clone(),
                template: prepared.template_file.clone(),
                codex_command: prepared.plan_item.codex_command.clone(),
                executed: false,
                exit_status: None,
                stdout_path: stdout_log.display().to_string(),
                stderr_path: stderr_log.display().to_string(),
                result_path: result_md_path.display().to_string(),
                timestamp: Utc::now(),
                warnings,
            };
            utils::write_json_pretty(&sidecar_path, &sidecar)?;
            return Ok((sidecar, false));
        }
    };

    append_log_section(&stdout_log, &prepared.plan_item.asset, &capture.stdout)?;
    append_log_section(&stderr_log, &prepared.plan_item.asset, &capture.stderr)?;

    if !capture.success {
        warnings.push(format!(
            "codex exec exited with status {}",
            capture
                .exit_status
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ));
    }

    let content = if capture.stdout.trim().is_empty() {
        render_failure_markdown(
            &prepared.plan_item.asset,
            &[
                "Codex returned no stdout content. Review stderr and validate manually."
                    .to_string(),
            ],
        )
    } else {
        capture.stdout.clone()
    };
    utils::write_string(&result_md_path, &content)?;

    let sidecar = CodexInsightResult {
        asset: prepared.plan_item.asset.clone(),
        template: prepared.template_file.clone(),
        codex_command: prepared.plan_item.codex_command.clone(),
        executed: true,
        exit_status: capture.exit_status,
        stdout_path: stdout_log.display().to_string(),
        stderr_path: stderr_log.display().to_string(),
        result_path: result_md_path.display().to_string(),
        timestamp: Utc::now(),
        warnings,
    };
    utils::write_json_pretty(&sidecar_path, &sidecar)?;

    Ok((sidecar, true))
}

fn build_prompt(
    context: &LlmAssetContext,
    reasoning_item: &LlmReasoningItem,
    template: &TemplateMaterial,
    support: &SupportingArtifacts,
    max_prompt_chars: usize,
) -> (String, Vec<String>) {
    let mut warnings = Vec::new();
    let redacted_context = redact_likely_secrets(&context.context_markdown, &mut warnings);
    let review_reasons = support
        .review_items
        .get(&context.asset)
        .map(|item| item.reasons.clone())
        .unwrap_or_default();

    let enrichment_snippet = support
        .enrichment_summary
        .as_deref()
        .map(|value| truncate_text(&redact_likely_secrets(value, &mut warnings), 900))
        .unwrap_or_else(|| "Enrichment summary unavailable.".to_string());
    let api_snippet = support
        .api_summary
        .as_deref()
        .map(|value| truncate_text(&redact_likely_secrets(value, &mut warnings), 700))
        .unwrap_or_else(|| "API summary unavailable.".to_string());
    let graph_snippet = support
        .graph_summary
        .as_ref()
        .map(render_graph_summary_snippet)
        .unwrap_or_else(|| "Graph summary unavailable.".to_string());
    let evidence_refs = if context.evidence_refs.is_empty() {
        "none".to_string()
    } else {
        context.evidence_refs.join(", ")
    };
    let why_selected = if reasoning_item.why_llm_review.is_empty() {
        "Structured reasoning context is available for cautious prioritization.".to_string()
    } else {
        reasoning_item.why_llm_review.join("; ")
    };
    let review_reason_line = if review_reasons.is_empty() {
        "No additional review reasons were available.".to_string()
    } else {
        review_reasons.join("; ")
    };
    let template_section = truncate_text(&template.markdown, 2_000);

    let fixed_prefix = format!(
        "You are assisting with analyst-controlled reasoning over local ReconPilot artifacts only.\n\
\n\
Safety rules:\n\
- Do not contact targets.\n\
- Do not claim vulnerabilities or confirmed impact.\n\
- Use cautious language such as candidate, interesting, worth manual review, potentially sensitive, and requires validation.\n\
- Do not suggest destructive testing, payloads, exploitation, credential attacks, brute force, or auth attacks.\n\
- Do not assume out-of-scope assets are allowed.\n\
- Base every hypothesis on cited evidence from the supplied context.\n\
\n\
Requested output schema:\n\
1. Analyst summary\n\
2. Why this asset is interesting\n\
3. Evidence-backed hypotheses\n\
4. Questions for manual validation\n\
5. Safe next review steps\n\
6. What not to conclude yet\n\
7. Confidence and uncertainty\n\
\n\
Asset metadata:\n\
- Asset: {}\n\
- Risk level: {}\n\
- Score: {}\n\
- Confidence: {:.2}\n\
- Semantic roles: {}\n\
- Environments: {}\n\
- Evidence refs: {}\n\
- Why selected: {}\n\
- Review reasons: {}\n\
\n\
Supporting summaries:\n\
Enrichment summary snippet:\n{}\n\
\n\
API summary snippet:\n{}\n\
\n\
Graph summary snippet:\n{}\n\
\n\
Prompt template guidance (follow this together with the safety rules above):\n{}\n\
\n\
Asset context:\n",
        context.asset,
        context.risk_level,
        context.score,
        context.confidence,
        render_roles(context),
        render_environments(context),
        evidence_refs,
        why_selected,
        review_reason_line,
        enrichment_snippet,
        api_snippet,
        graph_snippet,
        template_section,
    );

    let budget_for_context = max_prompt_chars.saturating_sub(fixed_prefix.chars().count() + 64);
    let mut context_body = if budget_for_context == 0 {
        warnings.push(format!(
            "Configured max_context_chars budget of {max_prompt_chars} left no room for the asset context body."
        ));
        String::new()
    } else {
        truncate_text(&redacted_context, budget_for_context)
    };

    if context_body.chars().count() < redacted_context.chars().count() {
        warnings.push(format!(
            "Prompt content for '{}' was truncated to respect the max_context_chars budget of {} characters.",
            context.asset, max_prompt_chars
        ));
        context_body
            .push_str("\n\n[Truncated for prompt budget. Evidence IDs were preserved above.]");
    }

    let mut prompt = format!("{fixed_prefix}{context_body}\n");
    if prompt.chars().count() > max_prompt_chars {
        warnings.push(format!(
            "Prompt content for '{}' exceeded the prompt budget after assembly and was truncated at the final step.",
            context.asset
        ));
        prompt = truncate_text(&prompt, max_prompt_chars);
    }

    (prompt, dedupe_strings(warnings))
}

fn build_summary(
    pack: &Path,
    out: &Path,
    execute_codex: bool,
    codex_available: bool,
    max_prompt_chars: usize,
    limit: usize,
    template_filter: Option<String>,
    warnings: &[String],
    results: &[CodexInsightResult],
    planned_count: usize,
) -> CodexRunnerSummary {
    let executed_count = results.iter().filter(|result| result.executed).count();
    let success_count = results
        .iter()
        .filter(|result| result.executed && result.exit_status.unwrap_or(1) == 0)
        .count();
    let failure_count = results.len().saturating_sub(success_count);

    CodexRunnerSummary {
        generated_at: Utc::now(),
        pack_path: pack.display().to_string(),
        output_root: out.display().to_string(),
        execute_requested: execute_codex,
        codex_available,
        planned_count,
        executed_count,
        success_count,
        failure_count,
        max_prompt_chars,
        limit,
        template_filter,
        warnings: warnings.to_vec(),
        results: results.to_vec(),
    }
}

fn render_plan_markdown(plan: &CodexRunPlan) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot Codex Command Plan\n\n");
    output.push_str(&format!(
        "- Generated at: {}\n- Pack: {}\n- Output root: {}\n- Mode: {}\n- Codex available: {}\n- Prompt budget: {}\n- Limit: {}\n",
        plan.generated_at.to_rfc3339(),
        plan.pack_path,
        plan.output_root,
        if plan.execute_requested {
            "execute-codex"
        } else {
            "plan-only"
        },
        plan.codex_available,
        plan.max_prompt_chars,
        plan.limit,
    ));
    if let Some(template) = &plan.template_filter {
        output.push_str(&format!("- Template override: {}\n", template));
    }
    output.push_str("\n## Safety\n\n");
    output.push_str(
        "- This phase only constructs or optionally runs local `codex exec` reasoning prompts.\n",
    );
    output.push_str("- No target contact occurs.\n");
    output.push_str("- Findings remain hypotheses and require validation.\n");
    output.push_str(
        "- No destructive testing, credential attacks, or exploitation guidance is requested.\n\n",
    );
    output.push_str("## Items\n\n");
    if plan.items.is_empty() {
        output.push_str("No reasoning items were available in the llm-pack queue.\n");
    } else {
        for item in &plan.items {
            output.push_str(&format!(
                "### {}. {}\n\n- Template: {}\n- Context: {}\n- Prompt chars: {}\n- Executed: {}\n- Evidence refs: {}\n- Why selected: {}\n- Command:\n\n```text\n{}\n```\n\n",
                item.rank,
                item.asset,
                item.template,
                item.context_file,
                item.prompt_chars,
                item.executed,
                if item.evidence_refs.is_empty() {
                    "none".to_string()
                } else {
                    item.evidence_refs.join(", ")
                },
                if item.why_selected.is_empty() {
                    "Structured context available.".to_string()
                } else {
                    item.why_selected.join("; ")
                },
                item.codex_command
            ));
            if !item.warnings.is_empty() {
                output.push_str("Warnings:\n");
                for warning in &item.warnings {
                    output.push_str(&format!("- {}\n", warning));
                }
                output.push('\n');
            }
        }
    }
    if !plan.warnings.is_empty() {
        output.push_str("## Warnings\n\n");
        for warning in &plan.warnings {
            output.push_str(&format!("- {}\n", warning));
        }
    }
    output
}

fn render_summary_markdown(
    summary: &CodexRunnerSummary,
    items: &[CodexRunItem],
    support: &SupportingArtifacts,
) -> String {
    let mut output = String::new();
    output.push_str("# ReconPilot Codex Runner Summary\n\n");
    output.push_str(&format!(
        "- Generated at: {}\n- Pack: {}\n- Output root: {}\n- Mode: {}\n- Codex available: {}\n- Planned items: {}\n- Executed items: {}\n- Successes: {}\n- Failures: {}\n",
        summary.generated_at.to_rfc3339(),
        summary.pack_path,
        summary.output_root,
        if summary.execute_requested {
            "execute-codex"
        } else {
            "plan-only"
        },
        summary.codex_available,
        summary.planned_count,
        summary.executed_count,
        summary.success_count,
        summary.failure_count
    ));
    output.push_str("\n## Analyst Notes\n\n");
    output.push_str("- Codex output is reasoning support only and does not validate findings.\n");
    output.push_str(
        "- Every hypothesis still requires manual review and scope-aware confirmation.\n",
    );
    output.push_str("- No target contact occurred during this phase.\n\n");

    output.push_str("## Queue Preview\n\n");
    if items.is_empty() {
        output.push_str("No llm-pack reasoning items were available.\n");
    } else {
        for item in items.iter().take(5) {
            output.push_str(&format!(
                "- {} [{}] via {}: {}\n",
                item.asset,
                item.rank,
                item.template,
                if item.why_selected.is_empty() {
                    "Structured reasoning context available.".to_string()
                } else {
                    item.why_selected.join("; ")
                }
            ));
        }
    }

    output.push_str("\n## Optional Supporting Context\n\n");
    output.push_str(&format!(
        "- Review queue present: {}\n- Enrichment summary present: {}\n- API summary present: {}\n- Graph summary present: {}\n",
        !support.review_items.is_empty(),
        support.enrichment_summary.is_some(),
        support.api_summary.is_some(),
        support.graph_summary.is_some()
    ));

    if !summary.warnings.is_empty() {
        output.push_str("\n## Warnings\n\n");
        for warning in &summary.warnings {
            output.push_str(&format!("- {}\n", warning));
        }
    }

    if !summary.results.is_empty() {
        output.push_str("\n## Result Files\n\n");
        for result in &summary.results {
            output.push_str(&format!(
                "- {} [{}] -> {}{}\n",
                result.asset,
                result.template,
                result.result_path,
                if result.executed {
                    String::new()
                } else {
                    " (execution failed or was skipped)".to_string()
                }
            ));
        }
    }

    output
}

fn render_failure_markdown(asset: &str, warnings: &[String]) -> String {
    let mut output = format!("# Codex Insight Placeholder\n\nAsset: `{asset}`\n\n");
    output.push_str("No completed Codex reasoning output was captured for this asset.\n\n");
    output.push_str("The asset still requires analyst review and validation.\n");
    if !warnings.is_empty() {
        output.push_str("\nWarnings:\n");
        for warning in warnings {
            output.push_str(&format!("- {}\n", warning));
        }
    }
    output
}

fn render_graph_summary_snippet(summary: &GraphSummary) -> String {
    format!(
        "Nodes: {}; Edges: {}; Clusters: {}; Anomalies: {}; Shared infrastructure: {}; Likely staging: {}; Likely internal: {}.",
        summary.node_count,
        summary.edge_count,
        summary.cluster_count,
        summary.anomaly_count,
        join_or_none(&summary.shared_infrastructure),
        join_or_none(&summary.likely_staging_systems),
        join_or_none(&summary.likely_internal_systems),
    )
}

fn render_roles(context: &LlmAssetContext) -> String {
    if context.semantic_roles.is_empty() {
        "unknown".to_string()
    } else {
        context
            .semantic_roles
            .iter()
            .map(|role| role.as_str().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn render_environments(context: &LlmAssetContext) -> String {
    if context.environments.is_empty() {
        "unknown".to_string()
    } else {
        context
            .environments
            .iter()
            .map(|environment| environment.as_str().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn choose_template_for_item(
    item: &LlmReasoningItem,
    templates: &BTreeMap<String, TemplateMaterial>,
) -> Option<TemplateMaterial> {
    resolve_template(templates, &item.suggested_prompt_template)
        .ok()
        .flatten()
        .or_else(|| {
            resolve_template(templates, "asset_triage_prompt")
                .ok()
                .flatten()
        })
}

fn resolve_template(
    templates: &BTreeMap<String, TemplateMaterial>,
    value: &str,
) -> Result<Option<TemplateMaterial>> {
    if let Some(template) = templates.get(value) {
        return Ok(Some(template.clone()));
    }

    let normalized = if value.ends_with(".md") {
        value.trim_end_matches(".md").to_string()
    } else {
        value.to_string()
    };
    if let Some(template) = templates.get(&normalized) {
        return Ok(Some(template.clone()));
    }
    let file_name = format!("{normalized}.md");
    if let Some(template) = templates.get(&file_name) {
        return Ok(Some(template.clone()));
    }

    bail!(
        "prompt template '{}' is unavailable in the llm-pack prompt directory",
        value
    )
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read JSON input: {}", path.display()))?;
    serde_json::from_str::<T>(&raw)
        .with_context(|| format!("failed to parse JSON input: {}", path.display()))
}

fn default_codex_executor(binary_path: &Path, prompt: &str) -> Result<ExecutionCapture> {
    let output = Command::new(binary_path)
        .arg("exec")
        .arg(prompt)
        .output()
        .with_context(|| format!("failed to execute {}", binary_path.display()))?;

    Ok(ExecutionCapture {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_status: output.status.code(),
        success: output.status.success(),
    })
}

fn resolve_codex_binary(binary_override: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = binary_override {
        if path.exists() && path.is_file() {
            return Some(path.to_path_buf());
        }
        return None;
    }

    resolve_binary_path("codex")
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

fn append_log_section(path: &Path, asset: &str, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        utils::ensure_directory(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open log file: {}", path.display()))?;
    writeln!(
        file,
        "===== {} | {} =====\n{}\n",
        Utc::now().to_rfc3339(),
        asset,
        content
    )
    .with_context(|| format!("failed to append log section to {}", path.display()))?;
    Ok(())
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

fn sanitize_asset_filename_stem(asset: &str) -> String {
    let mut value = asset.to_ascii_lowercase();
    for prefix in ["https://", "http://"] {
        if value.starts_with(prefix) {
            value = value.trim_start_matches(prefix).to_string();
        }
    }
    let mut sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while sanitized.contains("--") {
        sanitized = sanitized.replace("--", "-");
    }
    sanitized = sanitized.trim_matches('-').to_string();
    if sanitized.is_empty() {
        "asset".to_string()
    } else {
        sanitized
    }
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

fn redact_likely_secrets(value: &str, warnings: &mut Vec<String>) -> String {
    let patterns = [
        (
            Regex::new(r"(?im)^authorization\s*:\s*[^\r\n]+").expect("authorization regex"),
            "[REDACTED_AUTH_HEADER]",
            "Authorization header",
        ),
        (
            Regex::new(r"(?i)\bbearer\s+[A-Za-z0-9._\-+/=]{8,}\b").expect("bearer regex"),
            "Bearer [REDACTED_TOKEN]",
            "Bearer token",
        ),
        (
            Regex::new(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b")
                .expect("jwt regex"),
            "[REDACTED_JWT]",
            "JWT-like token",
        ),
        (
            Regex::new(
                r#"(?i)\b(api[_-]?key|access[_-]?key|secret[_-]?key)\b\s*[:=]\s*["']?[A-Za-z0-9_\-]{8,}["']?"#,
            )
            .expect("api key regex"),
            "$1=[REDACTED_KEY]",
            "API key material",
        ),
        (
            Regex::new(r"\b[a-fA-F0-9]{32,}\b").expect("hex regex"),
            "[REDACTED_HEX]",
            "long hex token",
        ),
        (
            Regex::new(r"\b[A-Za-z0-9+/]{40,}={0,2}\b").expect("base64 regex"),
            "[REDACTED_BLOB]",
            "long base64-like blob",
        ),
    ];

    let mut redacted = value.to_string();
    let mut redaction_hits = Vec::new();
    for (pattern, replacement, label) in patterns {
        let replaced = pattern.replace_all(&redacted, replacement).to_string();
        if replaced != redacted {
            redaction_hits.push(label.to_string());
            redacted = replaced;
        }
    }

    if !redaction_hits.is_empty() {
        warnings.push(format!(
            "Prompt content was redacted for likely secret material: {}",
            dedupe_strings(redaction_hits).join(", ")
        ));
    }

    redacted
}

fn truncate_text(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }

    let truncated = value
        .chars()
        .take(max_len.saturating_sub(3))
        .collect::<String>();
    format!("{truncated}...")
}

fn join_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ")
    }
}

fn file_stem(value: &str) -> String {
    Path::new(value)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;
    use serde_json::Value;

    use super::{
        redact_likely_secrets, run_codex_runner_with_executor, sanitize_asset_filename_stem,
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
                "reconpilot-codex-runner-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            Ok(Self { root })
        }

        fn output_root(&self) -> PathBuf {
            self.root.join("output")
        }

        fn pack_dir(&self) -> PathBuf {
            self.output_root().join("llm-pack")
        }

        fn insights_dir(&self) -> PathBuf {
            self.output_root().join("codex-insights")
        }

        fn write_file(&self, relative: &str, content: &str) -> Result<()> {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, content)?;
            Ok(())
        }

        fn touch_file(&self, relative: &str) -> Result<PathBuf> {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, [])?;
            Ok(path)
        }

        fn seed_llm_pack(&self) -> Result<()> {
            self.write_file(
                "output/llm-pack/reasoning-queue.json",
                r#"{"items":[{"rank":1,"asset":"auth.example.com","review_rank":1,"risk_level":"high","score":82,"confidence":0.91,"reasoning_score":95,"suggested_prompt_template":"asset_triage_prompt.md","context_file":"asset-contexts/001-auth-example-com.json","why_llm_review":["Auth-related surface with rich evidence"],"evidence_refs":["ev-1","ev-1","ev-2"]},{"rank":2,"asset":"app.example.com","review_rank":2,"risk_level":"medium","score":44,"confidence":0.67,"reasoning_score":60,"suggested_prompt_template":"auth_flow_review_prompt.md","context_file":"asset-contexts/002-app-example-com.json","why_llm_review":["JavaScript-derived internal route candidate"],"evidence_refs":["ev-3"]}]}"#,
            )?;
            self.write_file(
                "output/llm-pack/pack-summary.json",
                r#"{"generated_at":"2026-05-15T08:00:00Z","max_context_chars":1200,"asset_context_files":["asset-contexts/001-auth-example-com.json","asset-contexts/002-app-example-com.json"],"prompts":[{"name":"asset_triage_prompt","file_name":"asset_triage_prompt.md","purpose":"Asset triage","recommended_for":["high-priority assets"],"safety_constraints":["Use requires validation language"],"template_markdown":"Prioritize evidence-backed hypotheses only."},{"name":"auth_flow_review_prompt","file_name":"auth_flow_review_prompt.md","purpose":"Auth flow review","recommended_for":["auth surfaces"],"safety_constraints":["No auth attacks"],"template_markdown":"Focus on auth flow understanding and review order."}],"reasoning_queue":[],"summary":{"generated_at":"2026-05-15T08:00:00Z","asset_context_count":2,"prompt_template_count":2,"reasoning_item_count":2,"max_context_chars":1200,"total_evidence_refs":3,"truncated_context_count":0,"api_intel_present":true,"graph_summary_present":true,"top_review_themes":[],"top_api_auth_areas":[],"top_graph_clusters":[],"top_unknowns":[],"suggested_review_order":[],"warnings":[]}}"#,
            )?;
            self.write_file(
                "output/llm-pack/prompts/asset_triage_prompt.md",
                "# Asset Triage Prompt\n\nPrioritize evidence-backed hypotheses only.\n",
            )?;
            self.write_file(
                "output/llm-pack/prompts/auth_flow_review_prompt.md",
                "# Auth Flow Review Prompt\n\nReview auth and token handling carefully.\n",
            )?;
            self.write_file(
                "output/llm-pack/asset-contexts/001-auth-example-com.json",
                r##"{"asset":"auth.example.com","risk_level":"high","score":82,"confidence":0.91,"semantic_roles":["authentication","api_gateway"],"environments":["staging"],"graph_neighborhood_summary":"Shares infrastructure with a privileged cluster.","api_observations":["Auth endpoint candidate at /oauth/token"],"api_object_candidates":["User (high)"],"auth_observations":["Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.abcdef1234567890.zyx987654321"],"js_observations":["JS referenced /internal/graphql"],"schema_observations":["OpenAPI at /swagger.json"],"graphql_observations":["GraphQL candidate at /graphql"],"evidence_refs":["ev-1","ev-1","ev-2"],"evidence_highlights":["[ev-1] auth-observations.json :: Authorization: Bearer abcdefghijklmnopqrstuvwxyz0123456789","[ev-2] graphql-observations.json :: GraphQL candidate /graphql"],"cautious_next_step_questions":["Which auth flow candidate is worth manual review first?"],"context_markdown":"# Asset Context\n\nAuthorization: Bearer supersecretvalue01234567890123456789\nJWT: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.abcdef1234567890.zyx987654321\nAPI_KEY=ABCDEF1234567890ABCDEF1234567890\nNotes: auth.example.com references /oauth/token and /graphql.\n","estimated_chars":320,"truncated":false,"truncation_notes":[]}"##,
            )?;
            self.write_file(
                "output/llm-pack/asset-contexts/002-app-example-com.json",
                r##"{"asset":"app.example.com","risk_level":"medium","score":44,"confidence":0.67,"semantic_roles":["customer_app"],"environments":["production"],"graph_neighborhood_summary":"References a hidden JavaScript route candidate.","api_observations":[],"api_object_candidates":[],"auth_observations":[],"js_observations":["Feature flag betaAdmin referenced in app bundle"],"schema_observations":[],"graphql_observations":[],"evidence_refs":["ev-3"],"evidence_highlights":["[ev-3] js-observations.json :: JavaScript referenced /internal/admin/export"],"cautious_next_step_questions":["Which JS-derived route is worth manual review first?"],"context_markdown":"# Asset Context\n\nJavaScript referenced /internal/admin/export and betaAdmin.\n","estimated_chars":90,"truncated":false,"truncation_notes":[]}"##,
            )?;
            self.write_file(
                "output/review/priority-queue.json",
                r#"{"items":[{"rank":1,"asset":"auth.example.com","risk_level":"high","score":82,"confidence":0.91,"semantic_roles":["authentication","api_gateway"],"environments":["staging"],"reasons":["Auth-related surface"],"evidence_refs":["ev-1","ev-2"],"recommended_next_steps":["Review auth docs carefully."]},{"rank":2,"asset":"app.example.com","risk_level":"medium","score":44,"confidence":0.67,"semantic_roles":["customer_app"],"environments":["production"],"reasons":["JS-discovered internal route"],"evidence_refs":["ev-3"],"recommended_next_steps":["Review hidden routes carefully."]}]}"#,
            )?;
            self.write_file(
                "output/enrichment/enrichment-summary.md",
                "# Enrichment Summary\n\nAssets were enriched locally.\n",
            )?;
            self.write_file(
                "output/api-intel/api-summary.md",
                "# API Summary\n\nAuth and GraphQL candidates were derived locally.\n",
            )?;
            self.write_file(
                "output/maps/graph-summary.json",
                r#"{"generated_at":"2026-05-15T08:00:00Z","node_count":4,"edge_count":3,"cluster_count":1,"anomaly_count":1,"top_technologies":["GraphQL (1)"],"largest_clusters":["cluster:auth-admin"],"shared_infrastructure":["auth.example.com <-> admin.example.com"],"suspicious_naming":["auth.example.com"],"likely_staging_systems":["auth.example.com"],"likely_internal_systems":["admin.example.com"],"redirect_chain_count":0}"#,
            )?;
            Ok(())
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn codex_cli_missing_behavior_is_clear() -> Result<()> {
        let workspace = TestWorkspace::new("missing-cli")?;
        workspace.seed_llm_pack()?;

        let plan_only = run_codex_runner_with_executor(
            &workspace.pack_dir(),
            &workspace.insights_dir(),
            false,
            3,
            None,
            Some(workspace.root.join("missing-codex.exe")),
            |_binary, _prompt| unreachable!("plan mode should not execute"),
        )?;
        assert!(plan_only
            .summary
            .warnings
            .iter()
            .any(|warning| warning.contains("Codex CLI was not found")));

        let execute_result = run_codex_runner_with_executor(
            &workspace.pack_dir(),
            &workspace.insights_dir(),
            true,
            3,
            None,
            Some(workspace.root.join("missing-codex.exe")),
            |_binary, _prompt| unreachable!("missing binary should fail before execution"),
        );
        assert!(execute_result.is_err());
        Ok(())
    }

    #[test]
    fn plan_generation_writes_expected_outputs() -> Result<()> {
        let workspace = TestWorkspace::new("plan-generation")?;
        workspace.seed_llm_pack()?;

        let outcome = run_codex_runner_with_executor(
            &workspace.pack_dir(),
            &workspace.insights_dir(),
            false,
            3,
            None,
            Some(workspace.root.join("missing-codex.exe")),
            |_binary, _prompt| unreachable!("plan mode should not execute"),
        )?;
        assert!(outcome.plan_json_path.exists());
        assert!(outcome.plan_markdown_path.exists());
        assert!(outcome.summary_json_path.exists());
        assert!(outcome.summary_markdown_path.exists());
        Ok(())
    }

    #[test]
    fn prompt_construction_includes_safety_rules() -> Result<()> {
        let workspace = TestWorkspace::new("prompt-safety")?;
        workspace.seed_llm_pack()?;

        let outcome = run_codex_runner_with_executor(
            &workspace.pack_dir(),
            &workspace.insights_dir(),
            false,
            1,
            Some("asset_triage_prompt".to_string()),
            Some(workspace.root.join("missing-codex.exe")),
            |_binary, _prompt| unreachable!("plan mode should not execute"),
        )?;
        let raw = fs::read_to_string(outcome.plan_json_path)?;
        assert!(raw.contains("Do not claim vulnerabilities"));
        assert!(raw.contains("Do not suggest destructive testing"));
        assert!(raw.contains("requires validation"));
        Ok(())
    }

    #[test]
    fn limit_behavior_caps_planned_assets() -> Result<()> {
        let workspace = TestWorkspace::new("limit")?;
        workspace.seed_llm_pack()?;

        let outcome = run_codex_runner_with_executor(
            &workspace.pack_dir(),
            &workspace.insights_dir(),
            false,
            1,
            None,
            Some(workspace.root.join("missing-codex.exe")),
            |_binary, _prompt| unreachable!("plan mode should not execute"),
        )?;
        let raw = fs::read_to_string(outcome.plan_json_path)?;
        let value: Value = serde_json::from_str(&raw)?;
        assert_eq!(value["items"].as_array().map(|items| items.len()), Some(1));
        Ok(())
    }

    #[test]
    fn filename_sanitization_is_safe() {
        let value =
            sanitize_asset_filename_stem("https://Portal.Example.com/admin/login?next=/billing");
        assert!(!value.contains('/'));
        assert!(!value.contains('\\'));
        assert!(!value.contains(':'));
        assert!(value.contains("portal-example-com-admin-login-next-billing"));
    }

    #[test]
    fn secret_redaction_removes_likely_tokens() {
        let mut warnings = Vec::new();
        let value = redact_likely_secrets(
            "Authorization: Bearer supersecretvalue0123456789\nJWT eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.abcdef1234567890.zyx987654321\nAPI_KEY=ABCDEF1234567890ABCDEF1234567890",
            &mut warnings,
        );
        assert!(value.contains("[REDACTED_AUTH_HEADER]"));
        assert!(value.contains("[REDACTED_JWT]"));
        assert!(value.contains("[REDACTED_KEY]"));
        assert!(!warnings.is_empty());
    }

    #[test]
    fn result_sidecar_generation_writes_metadata() -> Result<()> {
        let workspace = TestWorkspace::new("sidecar")?;
        workspace.seed_llm_pack()?;
        let fake_binary = workspace.touch_file("fake-codex.exe")?;

        let outcome = run_codex_runner_with_executor(
            &workspace.pack_dir(),
            &workspace.insights_dir(),
            true,
            1,
            None,
            Some(fake_binary),
            |_binary, _prompt| {
                Ok(super::ExecutionCapture {
                    stdout: "# Analyst summary\n\nInteresting candidate.\n".to_string(),
                    stderr: String::new(),
                    exit_status: Some(0),
                    success: true,
                })
            },
        )?;

        let sidecar = fs::read_to_string(
            workspace
                .insights_dir()
                .join("results")
                .join("001-auth-example-com.json"),
        )?;
        let value: Value = serde_json::from_str(&sidecar)?;
        assert_eq!(value["asset"], "auth.example.com");
        assert_eq!(value["executed"], Value::Bool(true));
        assert!(outcome.summary.success_count >= 1);
        Ok(())
    }

    #[test]
    fn execute_mode_uses_codex_exec_not_interactive_codex() -> Result<()> {
        let workspace = TestWorkspace::new("exec-mode")?;
        workspace.seed_llm_pack()?;
        let fake_binary = workspace.touch_file("fake-codex.exe")?;
        let commands = RefCell::new(Vec::<String>::new());

        run_codex_runner_with_executor(
            &workspace.pack_dir(),
            &workspace.insights_dir(),
            true,
            1,
            None,
            Some(fake_binary),
            |_binary, _prompt| {
                Ok(super::ExecutionCapture {
                    stdout: "ok".to_string(),
                    stderr: String::new(),
                    exit_status: Some(0),
                    success: true,
                })
            },
        )?;

        let plan_raw = fs::read_to_string(
            workspace
                .insights_dir()
                .join("plans")
                .join("codex-command-plan.json"),
        )?;
        commands.borrow_mut().push(plan_raw);
        let joined = commands.borrow().join("\n");
        assert!(joined.contains("\"codex_command\""));
        assert!(joined.contains("codex exec"));
        assert!(!joined.contains("codex chat"));
        Ok(())
    }

    #[test]
    fn yolo_flag_never_appears_in_command_plan() -> Result<()> {
        let workspace = TestWorkspace::new("no-yolo")?;
        workspace.seed_llm_pack()?;

        let outcome = run_codex_runner_with_executor(
            &workspace.pack_dir(),
            &workspace.insights_dir(),
            false,
            1,
            None,
            Some(workspace.root.join("missing-codex.exe")),
            |_binary, _prompt| unreachable!("plan mode should not execute"),
        )?;
        let raw = fs::read_to_string(outcome.plan_json_path)?;
        assert!(!raw.contains("--yolo"));
        assert!(!raw.contains("--dangerously-bypass-approvals-and-sandbox"));
        Ok(())
    }
}
