use std::path::Path;

use anyhow::{bail, Result};

use crate::models::{PhaseStatus, PipelinePhase, PipelineProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseKey {
    Run,
    Map,
    Graph,
    ApiIntel,
    Enrich,
    Review,
    LlmPack,
    CodexRun,
    Validate,
}

impl PhaseKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Map => "map",
            Self::Graph => "graph",
            Self::ApiIntel => "api-intel",
            Self::Enrich => "enrich",
            Self::Review => "review",
            Self::LlmPack => "llm-pack",
            Self::CodexRun => "codex-run",
            Self::Validate => "validate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseKind {
    ExternalTool,
    LocalAnalysis,
}

impl PhaseKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::ExternalTool => "external-tool-phase",
            Self::LocalAnalysis => "local-analysis-phase",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PhaseDefinition {
    pub key: PhaseKey,
    pub enrich_with_api_intel: bool,
    pub phase: PipelinePhase,
}

#[derive(Debug, Clone)]
pub struct ProfileDefinition {
    pub profile: PipelineProfile,
    pub phases: Vec<PhaseDefinition>,
    pub warnings: Vec<String>,
}

pub fn resolve_profile(
    name: &str,
    scope: &Path,
    output_root: &Path,
    external_execute: bool,
    include_codex: bool,
    execute_codex: bool,
    max_context_chars: usize,
) -> Result<ProfileDefinition> {
    let normalized = name.trim().to_ascii_lowercase();
    let mut definition = match normalized.as_str() {
        "passive" => build_profile(
            "passive",
            "Dry-run external planning plus local graph, API, enrichment, review, LLM pack, and validation phases.",
            &[
                PhaseSpec::new(PhaseKey::Run, PhaseKind::ExternalTool, false, false),
                PhaseSpec::new(PhaseKey::Graph, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::ApiIntel, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Enrich, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Review, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::LlmPack, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Validate, PhaseKind::LocalAnalysis, true, false),
            ],
            scope,
            output_root,
            external_execute,
            execute_codex,
            max_context_chars,
        ),
        "active-lite" => build_profile(
            "active-lite",
            "Target-touching adapters remain gated behind --execute while local analysis runs in sequence.",
            &[
                PhaseSpec::new(PhaseKey::Run, PhaseKind::ExternalTool, external_execute, false),
                PhaseSpec::new(PhaseKey::Map, PhaseKind::ExternalTool, external_execute, false),
                PhaseSpec::new(PhaseKey::Graph, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::ApiIntel, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Enrich, PhaseKind::LocalAnalysis, true, true),
                PhaseSpec::new(PhaseKey::Review, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::LlmPack, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Validate, PhaseKind::LocalAnalysis, true, false),
            ],
            scope,
            output_root,
            external_execute,
            execute_codex,
            max_context_chars,
        ),
        "api-focused" => build_profile(
            "api-focused",
            "Local API and JavaScript intelligence plus API-aware enrichment, review, LLM packing, and validation.",
            &[
                PhaseSpec::new(PhaseKey::ApiIntel, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Enrich, PhaseKind::LocalAnalysis, true, true),
                PhaseSpec::new(PhaseKey::Review, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::LlmPack, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Validate, PhaseKind::LocalAnalysis, true, false),
            ],
            scope,
            output_root,
            external_execute,
            execute_codex,
            max_context_chars,
        ),
        "mapping-focused" => build_profile(
            "mapping-focused",
            "Mapping-first orchestration with local graph, enrichment, review, and validation phases.",
            &[
                PhaseSpec::new(PhaseKey::Map, PhaseKind::ExternalTool, external_execute, false),
                PhaseSpec::new(PhaseKey::Graph, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Enrich, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Review, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Validate, PhaseKind::LocalAnalysis, true, false),
            ],
            scope,
            output_root,
            external_execute,
            execute_codex,
            max_context_chars,
        ),
        "review-only" => build_profile(
            "review-only",
            "Existing graph outputs are enriched and converted into a review workspace before validation.",
            &[
                PhaseSpec::new(PhaseKey::Enrich, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Review, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Validate, PhaseKind::LocalAnalysis, true, false),
            ],
            scope,
            output_root,
            external_execute,
            execute_codex,
            max_context_chars,
        ),
        "llm-pack-only" => build_profile(
            "llm-pack-only",
            "Existing review and enrichment artifacts are packaged into local LLM-ready reasoning bundles before validation.",
            &[
                PhaseSpec::new(PhaseKey::LlmPack, PhaseKind::LocalAnalysis, true, false),
                PhaseSpec::new(PhaseKey::Validate, PhaseKind::LocalAnalysis, true, false),
            ],
            scope,
            output_root,
            external_execute,
            execute_codex,
            max_context_chars,
        ),
        _ => bail!(
            "unsupported pipeline profile '{}'. Supported profiles: {}",
            name,
            supported_profiles().join(", ")
        ),
    };

    if include_codex {
        if let Some(index) = definition
            .phases
            .iter()
            .position(|phase| phase.key == PhaseKey::LlmPack)
        {
            let codex_phase = PhaseDefinition {
                key: PhaseKey::CodexRun,
                enrich_with_api_intel: false,
                phase: phase_model(
                    PhaseSpec::new(
                        PhaseKey::CodexRun,
                        PhaseKind::LocalAnalysis,
                        execute_codex,
                        false,
                    ),
                    scope,
                    output_root,
                    external_execute,
                    execute_codex,
                    max_context_chars,
                ),
            };
            definition.phases.insert(index + 1, codex_phase.clone());
            definition.profile.phases = definition
                .phases
                .iter()
                .map(|phase| phase.phase.clone())
                .collect();
            if !execute_codex {
                definition.warnings.push(
                    "Codex pipeline integration was included, but the codex-run phase will remain plan-only until --execute-codex is passed.".to_string(),
                );
            }
        } else {
            definition.warnings.push(format!(
                "Pipeline profile '{}' does not include an llm-pack phase, so --include-codex was ignored.",
                definition.profile.name
            ));
        }
    } else if execute_codex {
        definition
            .warnings
            .push("--execute-codex was ignored because --include-codex was not set.".to_string());
    }

    Ok(definition)
}

pub fn supported_profiles() -> Vec<&'static str> {
    vec![
        "passive",
        "active-lite",
        "api-focused",
        "mapping-focused",
        "review-only",
        "llm-pack-only",
    ]
}

#[derive(Debug, Clone, Copy)]
struct PhaseSpec {
    key: PhaseKey,
    kind: PhaseKind,
    execute_phase: bool,
    enrich_with_api_intel: bool,
}

impl PhaseSpec {
    const fn new(
        key: PhaseKey,
        kind: PhaseKind,
        execute_phase: bool,
        enrich_with_api_intel: bool,
    ) -> Self {
        Self {
            key,
            kind,
            execute_phase,
            enrich_with_api_intel,
        }
    }
}

fn build_profile(
    name: &str,
    description: &str,
    specs: &[PhaseSpec],
    scope: &Path,
    output_root: &Path,
    external_execute: bool,
    execute_codex: bool,
    max_context_chars: usize,
) -> ProfileDefinition {
    let phases = specs
        .iter()
        .map(|spec| PhaseDefinition {
            key: spec.key,
            enrich_with_api_intel: spec.enrich_with_api_intel,
            phase: phase_model(
                *spec,
                scope,
                output_root,
                external_execute,
                execute_codex,
                max_context_chars,
            ),
        })
        .collect::<Vec<_>>();

    let profile = PipelineProfile {
        name: name.to_string(),
        description: description.to_string(),
        phases: phases.iter().map(|phase| phase.phase.clone()).collect(),
    };

    ProfileDefinition {
        profile,
        phases,
        warnings: Vec::new(),
    }
}

fn phase_model(
    spec: PhaseSpec,
    scope: &Path,
    output_root: &Path,
    external_execute: bool,
    execute_codex: bool,
    max_context_chars: usize,
) -> PipelinePhase {
    let scope_display = scope.display().to_string();
    let output_display = output_root.display().to_string();
    let maps_display = output_root.join("maps").display().to_string();
    let enrichment_display = output_root.join("enrichment").display().to_string();
    let review_display = output_root.join("review").display().to_string();
    let llm_pack_display = output_root.join("llm-pack").display().to_string();
    let codex_insights_display = output_root.join("codex-insights").display().to_string();
    let api_intel_display = output_root.join("api-intel").display().to_string();
    let plans_display = output_root.join("plans").display().to_string();

    let mut notes = vec![match spec.kind {
        PhaseKind::ExternalTool => {
            if spec.execute_phase {
                "External-tool phase will execute because --execute was requested.".to_string()
            } else {
                "External-tool phase remains dry-run by default and will generate plans only."
                    .to_string()
            }
        }
        PhaseKind::LocalAnalysis => {
            "Local-only analysis phase may execute without --execute because it does not contact targets.".to_string()
        }
    }];

    if spec.key == PhaseKey::Enrich && spec.enrich_with_api_intel {
        notes.push(
            "API-intel outputs will be merged into semantic enrichment when present.".to_string(),
        );
    }
    if spec.key == PhaseKey::Validate {
        notes.push(
            "Validation failures are surfaced clearly at the end of the pipeline.".to_string(),
        );
    }
    if spec.key == PhaseKey::CodexRun {
        notes.push(
            "Codex execution is explicit-only and never implied by the pipeline --execute flag."
                .to_string(),
        );
    }

    let (command, touches_targets, required_inputs, expected_outputs) = match spec.key {
        PhaseKey::Run => (
            render_execute_flag(
                format!(
                    "reconpilot run --scope {} --out {}",
                    scope_display, output_display
                ),
                spec.execute_phase,
            ),
            true,
            vec![scope_display.clone()],
            vec![
                format!("{plans_display}\\tool-runs.json"),
                output_root.join("raw").display().to_string(),
            ],
        ),
        PhaseKey::Map => (
            render_execute_flag(
                format!(
                    "reconpilot map --scope {} --out {}",
                    scope_display, output_display
                ),
                spec.execute_phase,
            ),
            true,
            vec![scope_display.clone()],
            vec![
                output_root
                    .join("plans")
                    .join("dnsx-plan.json")
                    .display()
                    .to_string(),
                output_root
                    .join("maps")
                    .join("app-map.json")
                    .display()
                    .to_string(),
            ],
        ),
        PhaseKey::Graph => (
            render_execute_flag(
                format!(
                    "reconpilot graph --input {} --out {}",
                    output_display, maps_display
                ),
                true,
            ),
            false,
            vec![
                output_root.join("raw").display().to_string(),
                output_root.join("maps").display().to_string(),
            ],
            vec![
                output_root
                    .join("plans")
                    .join("graph-plan.json")
                    .display()
                    .to_string(),
                output_root
                    .join("maps")
                    .join("graph.json")
                    .display()
                    .to_string(),
            ],
        ),
        PhaseKey::ApiIntel => (
            format!(
                "reconpilot api-intel --input {} --out {}",
                output_display, api_intel_display
            ),
            false,
            vec![
                output_root.join("raw").display().to_string(),
                output_root.join("maps").display().to_string(),
            ],
            vec![output_root
                .join("api-intel")
                .join("api-endpoints.json")
                .display()
                .to_string()],
        ),
        PhaseKey::Enrich => (
            if spec.enrich_with_api_intel {
                format!(
                    "reconpilot enrich --input {} --api-intel {} --out {}",
                    maps_display, api_intel_display, enrichment_display
                )
            } else {
                format!(
                    "reconpilot enrich --input {} --out {}",
                    maps_display, enrichment_display
                )
            },
            false,
            {
                let mut values = vec![output_root
                    .join("maps")
                    .join("graph.json")
                    .display()
                    .to_string()];
                if spec.enrich_with_api_intel {
                    values.push(api_intel_display.clone());
                }
                values
            },
            vec![output_root
                .join("enrichment")
                .join("semantic-assets.json")
                .display()
                .to_string()],
        ),
        PhaseKey::Review => (
            format!(
                "reconpilot review --input {} --out {}",
                enrichment_display, review_display
            ),
            false,
            vec![output_root
                .join("enrichment")
                .join("semantic-assets.json")
                .display()
                .to_string()],
            vec![output_root
                .join("review")
                .join("priority-queue.json")
                .display()
                .to_string()],
        ),
        PhaseKey::LlmPack => (
            format!(
                "reconpilot llm-pack --input {} --out {} --max-context-chars {}",
                output_display, llm_pack_display, max_context_chars
            ),
            false,
            vec![
                output_root
                    .join("review")
                    .join("priority-queue.json")
                    .display()
                    .to_string(),
                output_root
                    .join("enrichment")
                    .join("semantic-assets.json")
                    .display()
                    .to_string(),
            ],
            vec![output_root
                .join("llm-pack")
                .join("reasoning-queue.json")
                .display()
                .to_string()],
        ),
        PhaseKey::CodexRun => (
            render_codex_execute_flag(
                format!(
                    "reconpilot codex-run --pack {} --out {}",
                    llm_pack_display, codex_insights_display
                ),
                execute_codex,
            ),
            false,
            vec![
                output_root
                    .join("llm-pack")
                    .join("reasoning-queue.json")
                    .display()
                    .to_string(),
                output_root
                    .join("llm-pack")
                    .join("pack-summary.json")
                    .display()
                    .to_string(),
            ],
            vec![
                output_root
                    .join("codex-insights")
                    .join("plans")
                    .join("codex-command-plan.json")
                    .display()
                    .to_string(),
                output_root
                    .join("codex-insights")
                    .join("codex-summary.json")
                    .display()
                    .to_string(),
            ],
        ),
        PhaseKey::Validate => (
            format!("reconpilot validate --input {}", output_display),
            false,
            vec![output_display.clone()],
            vec![
                output_root
                    .join("validation-report.md")
                    .display()
                    .to_string(),
                output_root
                    .join("validation-report.json")
                    .display()
                    .to_string(),
            ],
        ),
    };

    if !external_execute && spec.kind == PhaseKind::ExternalTool {
        notes.push(
            "Passing --execute to the pipeline is required before any target-touching adapter launches."
                .to_string(),
        );
    }

    PipelinePhase {
        name: spec.key.as_str().to_string(),
        phase_type: spec.kind.as_str().to_string(),
        command,
        status: PhaseStatus::Planned,
        execute_phase: spec.execute_phase,
        touches_targets,
        required_inputs,
        expected_outputs,
        notes,
    }
}

fn render_execute_flag(base: String, execute: bool) -> String {
    if execute {
        format!("{base} --execute")
    } else {
        base
    }
}

fn render_codex_execute_flag(base: String, execute_codex: bool) -> String {
    if execute_codex {
        format!("{base} --execute-codex")
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{resolve_profile, supported_profiles};

    #[test]
    fn profile_parsing_resolves_known_profile() -> Result<()> {
        let scope = std::path::Path::new("config").join("scope.example.txt");
        let out = std::path::Path::new("output");
        let profile = resolve_profile("active-lite", &scope, out, false, false, false, 12_000)?;
        assert_eq!(profile.profile.name, "active-lite");
        assert_eq!(profile.profile.phases.len(), 8);
        assert_eq!(profile.profile.phases[0].name, "run");
        assert!(!profile.profile.phases[0].execute_phase);
        assert!(profile.profile.phases[2].execute_phase);
        Ok(())
    }

    #[test]
    fn supported_profile_list_contains_all_expected_values() {
        let names = supported_profiles();
        assert!(names.contains(&"passive"));
        assert!(names.contains(&"active-lite"));
        assert!(names.contains(&"api-focused"));
        assert!(names.contains(&"mapping-focused"));
        assert!(names.contains(&"review-only"));
        assert!(names.contains(&"llm-pack-only"));
    }

    #[test]
    fn include_codex_adds_phase_after_llm_pack() -> Result<()> {
        let scope = std::path::Path::new("config").join("scope.example.txt");
        let out = std::path::Path::new("output");
        let profile = resolve_profile("passive", &scope, out, false, true, false, 12_000)?;
        let llm_index = profile
            .profile
            .phases
            .iter()
            .position(|phase| phase.name == "llm-pack")
            .expect("llm-pack should exist");
        assert_eq!(profile.profile.phases[llm_index + 1].name, "codex-run");
        assert!(!profile.profile.phases[llm_index + 1].execute_phase);
        Ok(())
    }

    #[test]
    fn execute_codex_without_include_codex_is_ignored() -> Result<()> {
        let scope = std::path::Path::new("config").join("scope.example.txt");
        let out = std::path::Path::new("output");
        let profile = resolve_profile("passive", &scope, out, false, false, true, 12_000)?;
        assert!(!profile
            .profile
            .phases
            .iter()
            .any(|phase| phase.name == "codex-run"));
        assert!(profile
            .warnings
            .iter()
            .any(|warning| warning.contains("--execute-codex was ignored")));
        Ok(())
    }
}
