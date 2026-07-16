use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::Serialize;

use crate::{
    models::{
        CodexInsightResult, CodexReviewItem, CodexReviewSummary, CodexRunPlan, CodexRunnerSummary,
        EvidenceGap, UnsupportedClaim, WordingWarning,
    },
    utils,
};

#[derive(Debug, Clone)]
pub struct CodexReviewOutcome {
    pub queue_markdown_path: PathBuf,
    pub queue_json_path: PathBuf,
    pub unsupported_claims_path: PathBuf,
    pub evidence_gaps_path: PathBuf,
    pub wording_warnings_path: PathBuf,
    pub summary_markdown_path: PathBuf,
    pub summary: CodexReviewSummary,
}

#[derive(Debug, Clone, Serialize)]
struct CodexReviewQueueDocument {
    summary: CodexReviewSummary,
    items: Vec<CodexReviewItem>,
}

#[derive(Debug, Clone, Default)]
struct AssetAnnotations {
    unsupported_claims: Vec<UnsupportedClaim>,
    evidence_gaps: Vec<EvidenceGap>,
    wording_warnings: Vec<WordingWarning>,
}

pub fn run_codex_review(input: &Path, out: &Path) -> Result<CodexReviewOutcome> {
    validate_codex_review_input(input)?;
    utils::ensure_directory(out)?;

    let summary = load_json::<CodexRunnerSummary>(&input.join("codex-summary.json"))?;
    let plan =
        load_optional_json::<CodexRunPlan>(&input.join("plans").join("codex-command-plan.json"))?
            .unwrap_or_else(|| CodexRunPlan {
                generated_at: Utc::now(),
                pack_path: String::new(),
                output_root: input.display().to_string(),
                execute_requested: false,
                codex_available: false,
                max_prompt_chars: 0,
                limit: 0,
                template_filter: None,
                items: Vec::new(),
                warnings: Vec::new(),
            });
    let plan_map = plan
        .items
        .iter()
        .cloned()
        .map(|item| (item.asset.clone(), item))
        .collect::<BTreeMap<_, _>>();

    let mut warnings = summary.warnings.clone();
    let mut queue_items = Vec::new();
    let mut unsupported_claims = Vec::new();
    let mut evidence_gaps = Vec::new();
    let mut wording_warnings = Vec::new();

    if summary.results.is_empty() {
        warnings.push(
            "No Codex reasoning results were present. The codex-run phase may have stayed in plan-only mode."
                .to_string(),
        );
    }

    for (index, result) in summary.results.iter().enumerate() {
        let result_markdown_path = resolve_runtime_path(input, &result.result_path);
        let markdown = load_result_markdown(&result_markdown_path)?;
        let sidecar_path = sidecar_path_for_result(&result_markdown_path);
        let _sidecar = load_json::<CodexInsightResult>(&sidecar_path)?;
        let plan_item = plan_map.get(&result.asset);

        let expected_evidence_refs = plan_item
            .map(|item| dedupe_strings(item.evidence_refs.clone()))
            .unwrap_or_default();
        let mentioned_evidence_refs = expected_evidence_refs
            .iter()
            .filter(|reference| markdown.contains(reference.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let requires_validation_language = markdown
            .to_ascii_lowercase()
            .contains("requires validation");
        let recommendations = extract_recommendations(&markdown);
        let analyst_summary = extract_analyst_summary(&markdown);
        let annotations = analyze_result_markdown(
            &result.asset,
            &result.result_path,
            &markdown,
            &expected_evidence_refs,
            &mentioned_evidence_refs,
            requires_validation_language,
        );

        let mut caution_notes = Vec::new();
        if !requires_validation_language {
            caution_notes.push(
                "Codex output did not explicitly use 'requires validation' language.".to_string(),
            );
        }
        if !annotations.unsupported_claims.is_empty() {
            caution_notes.push(
                "Overconfident or unsupported claim wording was detected and requires manual review."
                    .to_string(),
            );
        }
        if !annotations.evidence_gaps.is_empty() {
            caution_notes.push(
                "Expected evidence references were missing or incomplete in the Codex result."
                    .to_string(),
            );
        }
        if !annotations.wording_warnings.is_empty() {
            caution_notes
                .push("Safety wording warnings were detected in the Codex result.".to_string());
        }
        if caution_notes.is_empty() {
            caution_notes.push(
                "Codex output remained cautious, but it still requires analyst validation."
                    .to_string(),
            );
        }

        queue_items.push(CodexReviewItem {
            rank: index + 1,
            asset: result.asset.clone(),
            template: result.template.clone(),
            executed: result.executed,
            exit_status: result.exit_status,
            result_path: result_markdown_path.display().to_string(),
            sidecar_path: sidecar_path.display().to_string(),
            expected_evidence_refs: expected_evidence_refs.clone(),
            mentioned_evidence_refs: mentioned_evidence_refs.clone(),
            analyst_recommendations: recommendations,
            analyst_summary,
            requires_validation_language,
            unsupported_claim_count: annotations.unsupported_claims.len(),
            evidence_gap_count: annotations.evidence_gaps.len(),
            wording_warning_count: annotations.wording_warnings.len(),
            caution_notes,
        });

        unsupported_claims.extend(annotations.unsupported_claims);
        evidence_gaps.extend(annotations.evidence_gaps);
        wording_warnings.extend(annotations.wording_warnings);
    }

    queue_items.sort_by(|left, right| {
        right
            .unsupported_claim_count
            .cmp(&left.unsupported_claim_count)
            .then_with(|| right.evidence_gap_count.cmp(&left.evidence_gap_count))
            .then_with(|| right.wording_warning_count.cmp(&left.wording_warning_count))
            .then_with(|| right.executed.cmp(&left.executed))
            .then_with(|| left.asset.cmp(&right.asset))
    });
    for (index, item) in queue_items.iter_mut().enumerate() {
        item.rank = index + 1;
    }

    let review_summary = CodexReviewSummary {
        generated_at: Utc::now(),
        total_results: summary.results.len(),
        reviewed_items: queue_items.len(),
        executed_count: summary
            .results
            .iter()
            .filter(|result| result.executed)
            .count(),
        plan_only_count: summary
            .results
            .iter()
            .filter(|result| !result.executed)
            .count(),
        unsupported_claim_count: unsupported_claims.len(),
        evidence_gap_count: evidence_gaps.len(),
        wording_warning_count: wording_warnings.len(),
        top_review_targets: queue_items
            .iter()
            .take(5)
            .map(|item| {
                format!(
                    "{} [claims:{} gaps:{} warnings:{}]",
                    item.asset,
                    item.unsupported_claim_count,
                    item.evidence_gap_count,
                    item.wording_warning_count
                )
            })
            .collect(),
        warnings,
    };

    let queue_markdown_path = out.join("codex-review-queue.md");
    let queue_json_path = out.join("codex-review-queue.json");
    let unsupported_claims_path = out.join("unsupported-claims.json");
    let evidence_gaps_path = out.join("evidence-gaps.json");
    let wording_warnings_path = out.join("wording-warnings.json");
    let summary_markdown_path = out.join("codex-review-summary.md");

    utils::write_json_pretty(
        &queue_json_path,
        &CodexReviewQueueDocument {
            summary: review_summary.clone(),
            items: queue_items.clone(),
        },
    )?;
    utils::write_string(
        &queue_markdown_path,
        &render_queue_markdown(&review_summary, &queue_items),
    )?;
    utils::write_json_pretty(&unsupported_claims_path, &unsupported_claims)?;
    utils::write_json_pretty(&evidence_gaps_path, &evidence_gaps)?;
    utils::write_json_pretty(&wording_warnings_path, &wording_warnings)?;
    utils::write_string(
        &summary_markdown_path,
        &render_summary_markdown(
            &review_summary,
            &unsupported_claims,
            &evidence_gaps,
            &wording_warnings,
        ),
    )?;

    Ok(CodexReviewOutcome {
        queue_markdown_path,
        queue_json_path,
        unsupported_claims_path,
        evidence_gaps_path,
        wording_warnings_path,
        summary_markdown_path,
        summary: review_summary,
    })
}

pub fn print_codex_review_summary(input: &Path, out: &Path, outcome: &CodexReviewOutcome) {
    println!("ReconPilot Codex review summary");
    println!("Input Codex insights: {}", input.display());
    println!("Output Codex review: {}", out.display());
    println!(
        "Reviewed items: {} | Unsupported claims: {} | Evidence gaps: {} | Wording warnings: {}",
        outcome.summary.reviewed_items,
        outcome.summary.unsupported_claim_count,
        outcome.summary.evidence_gap_count,
        outcome.summary.wording_warning_count
    );
    println!("Outputs:");
    println!("  - {}", outcome.queue_markdown_path.display());
    println!("  - {}", outcome.queue_json_path.display());
    println!("  - {}", outcome.unsupported_claims_path.display());
    println!("  - {}", outcome.evidence_gaps_path.display());
    println!("  - {}", outcome.wording_warnings_path.display());
    println!("  - {}", outcome.summary_markdown_path.display());
    for warning in &outcome.summary.warnings {
        println!("Warning: {warning}");
    }
}

fn validate_codex_review_input(input: &Path) -> Result<()> {
    if !input.exists() {
        bail!(
            "codex-review input directory does not exist: {}",
            input.display()
        );
    }
    if !input.is_dir() {
        bail!(
            "codex-review input path is not a directory: {}",
            input.display()
        );
    }

    let summary_path = input.join("codex-summary.json");
    if !summary_path.exists() {
        bail!(
            "required codex-review input is missing: {}. Run `reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/` first.",
            summary_path.display()
        );
    }

    Ok(())
}

fn analyze_result_markdown(
    asset: &str,
    source_path: &str,
    markdown: &str,
    expected_evidence_refs: &[String],
    mentioned_evidence_refs: &[String],
    requires_validation_language: bool,
) -> AssetAnnotations {
    let mut annotations = AssetAnnotations::default();
    let lower = markdown.to_ascii_lowercase();
    let overconfident_phrases = [
        "confirmed vulnerability",
        "definitely vulnerable",
        "critical vulnerability",
        "can be exploited",
        "exploitable",
    ];

    for phrase in overconfident_phrases {
        if lower.contains(phrase) {
            annotations.wording_warnings.push(WordingWarning {
                asset: asset.to_string(),
                category: "overconfident_wording".to_string(),
                text: phrase.to_string(),
                source_path: source_path.to_string(),
                recommendation:
                    "Replace certainty with cautious language and keep 'requires validation' wording."
                        .to_string(),
            });
            if !requires_validation_language
                || (!expected_evidence_refs.is_empty() && mentioned_evidence_refs.is_empty())
            {
                annotations.unsupported_claims.push(UnsupportedClaim {
                    asset: asset.to_string(),
                    phrase: phrase.to_string(),
                    reason: if !requires_validation_language {
                        "The result used overconfident wording without also saying 'requires validation'."
                            .to_string()
                    } else {
                        "The result used overconfident wording without citing expected evidence references."
                            .to_string()
                    },
                    source_path: source_path.to_string(),
                    requires_validation_language,
                    expected_evidence_refs: expected_evidence_refs.to_vec(),
                    mentioned_evidence_refs: mentioned_evidence_refs.to_vec(),
                });
            }
        }
    }

    if !expected_evidence_refs.is_empty()
        && mentioned_evidence_refs.len() < expected_evidence_refs.len()
    {
        annotations.evidence_gaps.push(EvidenceGap {
            asset: asset.to_string(),
            description: if mentioned_evidence_refs.is_empty() {
                "Codex output did not cite any of the expected evidence references.".to_string()
            } else {
                "Codex output cited only a subset of the expected evidence references.".to_string()
            },
            source_path: source_path.to_string(),
            expected_evidence_refs: expected_evidence_refs.to_vec(),
            mentioned_evidence_refs: mentioned_evidence_refs.to_vec(),
        });
    }

    let unsafe_patterns = [
        (
            "destructive testing",
            "unsafe_recommendation",
            "Keep recommendations focused on passive review or safe manual validation steps.",
        ),
        (
            "credential attack",
            "unsafe_recommendation",
            "Do not suggest credential attacks or brute-force testing.",
        ),
        (
            "password spray",
            "unsafe_recommendation",
            "Remove credential-based attack suggestions from Codex reasoning.",
        ),
        (
            "auth bypass",
            "unsafe_recommendation",
            "Do not suggest auth bypass attempts; recommend cautious validation instead.",
        ),
        (
            "authentication bypass",
            "unsafe_recommendation",
            "Do not suggest auth bypass attempts; recommend cautious validation instead.",
        ),
        (
            "automated exploitation",
            "unsafe_recommendation",
            "Keep reasoning limited to analyst review steps, not automated exploitation.",
        ),
        (
            "run sqlmap",
            "unsafe_recommendation",
            "Exploit tooling suggestions are outside the Codex reasoning safety model.",
        ),
        (
            "use metasploit",
            "unsafe_recommendation",
            "Exploit tooling suggestions are outside the Codex reasoning safety model.",
        ),
        (
            "assume this is in scope",
            "scope_assumption",
            "Do not assume additional assets are in scope without explicit authorization.",
        ),
        (
            "all sibling domains are in scope",
            "scope_assumption",
            "Do not broaden scope assumptions beyond the provided artifacts.",
        ),
        (
            "scan the entire parent domain",
            "scope_assumption",
            "Do not suggest broader target contact or scanning from Codex results.",
        ),
    ];

    let mut seen_warnings = BTreeSet::new();
    for line in markdown.lines() {
        let lower_line = line.to_ascii_lowercase();
        for (pattern, category, recommendation) in unsafe_patterns {
            if lower_line.contains(pattern) {
                let key = format!("{category}:{pattern}:{line}");
                if seen_warnings.insert(key) {
                    annotations.wording_warnings.push(WordingWarning {
                        asset: asset.to_string(),
                        category: category.to_string(),
                        text: line.trim().to_string(),
                        source_path: source_path.to_string(),
                        recommendation: recommendation.to_string(),
                    });
                }
            }
        }
    }

    annotations
}

fn extract_analyst_summary(markdown: &str) -> String {
    let sections = parse_markdown_sections(markdown);
    if let Some(lines) = sections.get("analyst summary") {
        let summary = lines
            .iter()
            .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('-'))
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(" ");
        if !summary.is_empty() {
            return summary;
        }
    }

    markdown
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .unwrap_or("No analyst summary text was present in the Codex output.")
        .to_string()
}

fn extract_recommendations(markdown: &str) -> Vec<String> {
    let sections = parse_markdown_sections(markdown);
    let mut recommendations = Vec::new();
    for key in ["safe next review steps", "questions for manual validation"] {
        if let Some(lines) = sections.get(key) {
            for line in lines {
                let trimmed = line.trim().trim_start_matches('-').trim();
                if !trimmed.is_empty() {
                    recommendations.push(trimmed.to_string());
                }
            }
        }
    }
    dedupe_strings(recommendations)
}

fn parse_markdown_sections(markdown: &str) -> BTreeMap<String, Vec<String>> {
    let mut sections = BTreeMap::new();
    let mut current = "preamble".to_string();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let heading = trimmed.trim_start_matches('#').trim().to_ascii_lowercase();
            current = heading;
            sections.entry(current.clone()).or_insert_with(Vec::new);
        } else {
            sections
                .entry(current.clone())
                .or_insert_with(Vec::new)
                .push(trimmed.to_string());
        }
    }
    sections
}

fn render_queue_markdown(summary: &CodexReviewSummary, items: &[CodexReviewItem]) -> String {
    let mut output = String::new();
    output.push_str("# Codex Review Queue\n\n");
    output.push_str(&format!(
        "- Generated at: {}\n- Reviewed items: {}\n- Unsupported claims: {}\n- Evidence gaps: {}\n- Wording warnings: {}\n\n",
        summary.generated_at.to_rfc3339(),
        summary.reviewed_items,
        summary.unsupported_claim_count,
        summary.evidence_gap_count,
        summary.wording_warning_count
    ));

    if items.is_empty() {
        output.push_str("No Codex reasoning results were available for review.\n");
        return output;
    }

    for item in items {
        output.push_str(&format!(
            "## {}. {}\n\n- Template: {}\n- Executed: {}\n- Exit status: {}\n- Unsupported claims: {}\n- Evidence gaps: {}\n- Wording warnings: {}\n- Requires validation language: {}\n- Result: {}\n\nSummary: {}\n\n",
            item.rank,
            item.asset,
            item.template,
            item.executed,
            item.exit_status
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            item.unsupported_claim_count,
            item.evidence_gap_count,
            item.wording_warning_count,
            item.requires_validation_language,
            item.result_path,
            item.analyst_summary
        ));
        if !item.analyst_recommendations.is_empty() {
            output.push_str("Recommendations:\n");
            for recommendation in &item.analyst_recommendations {
                output.push_str(&format!("- {}\n", recommendation));
            }
            output.push('\n');
        }
        if !item.caution_notes.is_empty() {
            output.push_str("Caution notes:\n");
            for note in &item.caution_notes {
                output.push_str(&format!("- {}\n", note));
            }
            output.push('\n');
        }
    }

    output
}

fn render_summary_markdown(
    summary: &CodexReviewSummary,
    unsupported_claims: &[UnsupportedClaim],
    evidence_gaps: &[EvidenceGap],
    wording_warnings: &[WordingWarning],
) -> String {
    let mut output = String::new();
    output.push_str("# Codex Review Summary\n\n");
    output.push_str(&format!(
        "- Generated at: {}\n- Total results: {}\n- Reviewed items: {}\n- Executed results: {}\n- Plan-only results: {}\n- Unsupported claims: {}\n- Evidence gaps: {}\n- Wording warnings: {}\n\n",
        summary.generated_at.to_rfc3339(),
        summary.total_results,
        summary.reviewed_items,
        summary.executed_count,
        summary.plan_only_count,
        summary.unsupported_claim_count,
        summary.evidence_gap_count,
        summary.wording_warning_count
    ));
    output.push_str("Codex outputs are hypotheses only and require analyst validation. This review layer annotates language and evidence quality but does not rewrite the original results.\n\n");

    output.push_str("## Top Review Targets\n\n");
    if summary.top_review_targets.is_empty() {
        output.push_str("None observed.\n");
    } else {
        for target in &summary.top_review_targets {
            output.push_str(&format!("- {}\n", target));
        }
    }

    output.push_str("\n## Unsupported Claims\n\n");
    if unsupported_claims.is_empty() {
        output.push_str("None observed.\n");
    } else {
        for claim in unsupported_claims {
            output.push_str(&format!(
                "- {}: '{}' ({})\n",
                claim.asset, claim.phrase, claim.reason
            ));
        }
    }

    output.push_str("\n## Evidence Gaps\n\n");
    if evidence_gaps.is_empty() {
        output.push_str("None observed.\n");
    } else {
        for gap in evidence_gaps {
            output.push_str(&format!("- {}: {}\n", gap.asset, gap.description));
        }
    }

    output.push_str("\n## Wording Warnings\n\n");
    if wording_warnings.is_empty() {
        output.push_str("None observed.\n");
    } else {
        for warning in wording_warnings {
            output.push_str(&format!(
                "- {} [{}]: {}\n",
                warning.asset, warning.category, warning.text
            ));
        }
    }

    if !summary.warnings.is_empty() {
        output.push_str("\n## Review Warnings\n\n");
        for warning in &summary.warnings {
            output.push_str(&format!("- {}\n", warning));
        }
    }

    output
}

fn load_result_markdown(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .with_context(|| format!("failed to read Codex result markdown: {}", path.display()))
}

fn sidecar_path_for_result(path: &Path) -> PathBuf {
    path.with_extension("json")
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read JSON input: {}", path.display()))?;
    serde_json::from_str::<T>(&raw)
        .with_context(|| format!("failed to parse JSON input: {}", path.display()))
}

fn load_optional_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    load_json(path).map(Some)
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

fn resolve_runtime_path(input_root: &Path, raw: &str) -> PathBuf {
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() {
        return candidate;
    }

    if input_root.file_name().and_then(|value| value.to_str()) == Some("codex-insights") {
        let output_root = input_root.parent().unwrap_or(input_root);
        let parent = output_root.parent().unwrap_or(output_root);
        let from_parent = parent.join(&candidate);
        if from_parent.exists() {
            return from_parent;
        }
        let from_output = output_root.join(candidate.strip_prefix("output").unwrap_or(&candidate));
        if from_output.exists() {
            return from_output;
        }
    }

    let joined = input_root.join(&candidate);
    if joined.exists() {
        joined
    } else {
        candidate
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
    use serde_json::Value;

    use super::run_codex_review;

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
                "reconpilot-codex-review-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            Ok(Self { root })
        }

        fn input_dir(&self) -> PathBuf {
            self.root.join("output").join("codex-insights")
        }

        fn output_dir(&self) -> PathBuf {
            self.root.join("output").join("codex-review")
        }

        fn write_file(&self, relative: &str, content: &str) -> Result<()> {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, content)?;
            Ok(())
        }

        fn seed_codex_results(&self) -> Result<()> {
            self.write_file(
                "output/codex-insights/plans/codex-command-plan.json",
                r#"{"generated_at":"2026-05-15T12:00:00Z","pack_path":"output/llm-pack","output_root":"output/codex-insights","execute_requested":true,"codex_available":true,"max_prompt_chars":12000,"limit":2,"template_filter":null,"items":[{"rank":1,"asset":"auth.example.com","template":"asset_triage_prompt.md","context_file":"asset-contexts/001-auth-example-com.json","prompt_chars":1000,"codex_command":"codex exec \"prompt\"","executed":true,"evidence_refs":["ev-1","ev-2"],"why_selected":["Rich auth evidence"],"warnings":[]},{"rank":2,"asset":"app.example.com","template":"asset_triage_prompt.md","context_file":"asset-contexts/002-app-example-com.json","prompt_chars":900,"codex_command":"codex exec \"prompt\"","executed":true,"evidence_refs":["ev-3"],"why_selected":["Hidden route candidate"],"warnings":[]}]}"#,
            )?;
            self.write_file(
                "output/codex-insights/codex-summary.json",
                r#"{"generated_at":"2026-05-15T12:01:00Z","pack_path":"output/llm-pack","output_root":"output/codex-insights","execute_requested":true,"codex_available":true,"planned_count":2,"executed_count":2,"success_count":2,"failure_count":0,"max_prompt_chars":12000,"limit":2,"template_filter":null,"warnings":[],"results":[{"asset":"auth.example.com","template":"asset_triage_prompt.md","codex_command":"codex exec \"prompt\"","executed":true,"exit_status":0,"stdout_path":"output/codex-insights/logs/codex-stdout.log","stderr_path":"output/codex-insights/logs/codex-stderr.log","result_path":"output/codex-insights/results/001-auth-example-com.md","timestamp":"2026-05-15T12:01:00Z","warnings":[]},{"asset":"app.example.com","template":"asset_triage_prompt.md","codex_command":"codex exec \"prompt\"","executed":true,"exit_status":0,"stdout_path":"output/codex-insights/logs/codex-stdout.log","stderr_path":"output/codex-insights/logs/codex-stderr.log","result_path":"output/codex-insights/results/002-app-example-com.md","timestamp":"2026-05-15T12:01:05Z","warnings":[]}]}"#,
            )?;
            self.write_file(
                "output/codex-insights/results/001-auth-example-com.json",
                r#"{"asset":"auth.example.com","template":"asset_triage_prompt.md","codex_command":"codex exec \"prompt\"","executed":true,"exit_status":0,"stdout_path":"output/codex-insights/logs/codex-stdout.log","stderr_path":"output/codex-insights/logs/codex-stderr.log","result_path":"output/codex-insights/results/001-auth-example-com.md","timestamp":"2026-05-15T12:01:00Z","warnings":[]}"#,
            )?;
            self.write_file(
                "output/codex-insights/results/002-app-example-com.json",
                r#"{"asset":"app.example.com","template":"asset_triage_prompt.md","codex_command":"codex exec \"prompt\"","executed":true,"exit_status":0,"stdout_path":"output/codex-insights/logs/codex-stdout.log","stderr_path":"output/codex-insights/logs/codex-stderr.log","result_path":"output/codex-insights/results/002-app-example-com.md","timestamp":"2026-05-15T12:01:05Z","warnings":[]}"#,
            )?;
            self.write_file(
                "output/codex-insights/results/001-auth-example-com.md",
                "# Analyst summary\n\nThis is a confirmed vulnerability on the auth surface.\n\n# Evidence-backed hypotheses\n- The auth flow references ev-1 and ev-2.\n\n# Questions for manual validation\n- Which auth path should be reviewed first?\n\n# Safe next review steps\n- Review the auth documentation carefully.\n- Attempt an auth bypass to confirm impact.\n\n# What not to conclude yet\n- Final impact is still under review.\n\n# Confidence and uncertainty\n- Medium confidence.\n",
            )?;
            self.write_file(
                "output/codex-insights/results/002-app-example-com.md",
                "# Analyst summary\n\nInteresting application route family.\n\n# Evidence-backed hypotheses\n- Hidden route candidate exists.\n\n# Questions for manual validation\n- Which route family is worth review first?\n\n# Safe next review steps\n- Compare the route with visible navigation.\n\n# What not to conclude yet\n- Behavior requires validation.\n\n# Confidence and uncertainty\n- Medium confidence.\n",
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
    fn codex_review_queue_generation_creates_outputs() -> Result<()> {
        let workspace = TestWorkspace::new("queue")?;
        workspace.seed_codex_results()?;

        let outcome = run_codex_review(&workspace.input_dir(), &workspace.output_dir())?;
        assert!(outcome.queue_markdown_path.exists());
        assert!(outcome.queue_json_path.exists());
        assert!(outcome.summary_markdown_path.exists());
        Ok(())
    }

    #[test]
    fn unsupported_claim_detection_flags_overconfident_language() -> Result<()> {
        let workspace = TestWorkspace::new("claims")?;
        workspace.seed_codex_results()?;

        let outcome = run_codex_review(&workspace.input_dir(), &workspace.output_dir())?;
        let raw = fs::read_to_string(outcome.unsupported_claims_path)?;
        assert!(raw.contains("confirmed vulnerability"));
        Ok(())
    }

    #[test]
    fn evidence_gap_detection_flags_missing_refs() -> Result<()> {
        let workspace = TestWorkspace::new("evidence-gap")?;
        workspace.seed_codex_results()?;

        let outcome = run_codex_review(&workspace.input_dir(), &workspace.output_dir())?;
        let raw = fs::read_to_string(outcome.evidence_gaps_path)?;
        assert!(raw.contains("app.example.com"));
        assert!(raw.contains("ev-3"));
        Ok(())
    }

    #[test]
    fn unsafe_recommendation_detection_flags_risky_language() -> Result<()> {
        let workspace = TestWorkspace::new("unsafe-recommendation")?;
        workspace.seed_codex_results()?;

        let outcome = run_codex_review(&workspace.input_dir(), &workspace.output_dir())?;
        let raw = fs::read_to_string(outcome.wording_warnings_path)?;
        assert!(raw.contains("auth bypass"));
        Ok(())
    }

    #[test]
    fn queue_json_contains_ranked_review_items() -> Result<()> {
        let workspace = TestWorkspace::new("queue-json")?;
        workspace.seed_codex_results()?;

        let outcome = run_codex_review(&workspace.input_dir(), &workspace.output_dir())?;
        let raw = fs::read_to_string(outcome.queue_json_path)?;
        let value: Value = serde_json::from_str(&raw)?;
        assert_eq!(value["items"][0]["asset"], "auth.example.com");
        Ok(())
    }
}
