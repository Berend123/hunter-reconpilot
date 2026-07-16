use anyhow::Result;
use chrono::Utc;

use crate::models::{FindingRecord, ReportSummary};

pub fn build_summary(findings: &[FindingRecord]) -> ReportSummary {
    let mut highest = findings
        .iter()
        .filter_map(|finding| {
            finding
                .score
                .as_ref()
                .map(|score| (score.total, finding.title.clone()))
        })
        .collect::<Vec<_>>();

    highest.sort_by(|left, right| right.0.cmp(&left.0));

    ReportSummary {
        generated_at: Utc::now(),
        total_findings: findings.len(),
        scored_findings: findings
            .iter()
            .filter(|finding| finding.score.is_some())
            .count(),
        highest_risk_titles: highest
            .into_iter()
            .take(5)
            .map(|(_, title)| title)
            .collect(),
    }
}

pub fn build_markdown_report(findings: &[FindingRecord]) -> String {
    // TODO: Split report rendering into reusable markdown, JSON, and HTML formatters.
    let summary = build_summary(findings);
    let mut output = String::new();

    output.push_str("# ReconPilot Report Preview\n\n");
    output.push_str(&format!(
        "- Generated at: {}\n- Total findings: {}\n- Scored findings: {}\n\n",
        summary.generated_at.to_rfc3339(),
        summary.total_findings,
        summary.scored_findings
    ));

    output.push_str("## Top Findings\n\n");
    if findings.is_empty() {
        output.push_str("No findings were supplied.\n");
        return output;
    }

    let mut scored = findings.iter().collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        let left_score = left
            .score
            .as_ref()
            .map(|score| score.total)
            .unwrap_or_default();
        let right_score = right
            .score
            .as_ref()
            .map(|score| score.total)
            .unwrap_or_default();
        right_score.cmp(&left_score)
    });

    for finding in scored.into_iter().take(10) {
        let total = finding
            .score
            .as_ref()
            .map(|score| score.total)
            .unwrap_or_default();
        output.push_str(&format!(
            "- [{}] {} ({})\n",
            total,
            if finding.title.is_empty() {
                "<untitled finding>"
            } else {
                finding.title.as_str()
            },
            if finding.asset.is_empty() {
                "unknown asset"
            } else {
                finding.asset.as_str()
            }
        ));

        if let Some(score) = &finding.score {
            if !score.reasons.is_empty() {
                output.push_str(&format!("  Reasons: {}\n", score.reasons.join("; ")));
            }
        }
    }

    output
}

pub fn build_json_report(findings: &[FindingRecord]) -> Result<String> {
    let summary = build_summary(findings);
    let payload = serde_json::json!({
        "summary": summary,
        "findings": findings,
    });

    Ok(serde_json::to_string_pretty(&payload)?)
}
