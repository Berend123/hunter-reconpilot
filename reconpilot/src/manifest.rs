use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{models::PipelinePhase, utils};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub reconpilot_version: String,
    pub command_executed: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub input_paths: Vec<String>,
    #[serde(default)]
    pub output_paths: Vec<String>,
    #[serde(default)]
    pub scope_file_hash: Option<String>,
    #[serde(default)]
    pub config_hash: Option<String>,
    #[serde(default)]
    pub tool_plans_generated: Vec<String>,
    #[serde(default)]
    pub artifact_counts: BTreeMap<String, usize>,
    #[serde(default)]
    pub pipeline_profile: Option<String>,
    #[serde(default)]
    pub phase_results: Vec<PipelinePhase>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ManifestInput {
    pub command_executed: String,
    pub input_paths: Vec<PathBuf>,
    pub output_paths: Vec<PathBuf>,
    pub scope_path: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub pipeline_profile: Option<String>,
    pub phase_results: Vec<PipelinePhase>,
}

pub fn write_manifest(output_root: &Path, input: ManifestInput) -> Result<PathBuf> {
    utils::ensure_directory(output_root)?;
    let path = output_root.join("run-manifest.json");
    let manifest = RunManifest {
        reconpilot_version: env!("CARGO_PKG_VERSION").to_string(),
        command_executed: input.command_executed,
        timestamp: Utc::now(),
        input_paths: render_paths(&input.input_paths),
        output_paths: render_paths(&input.output_paths),
        scope_file_hash: hash_file_if_exists(input.scope_path.as_deref())?,
        config_hash: hash_file_if_exists(input.config_path.as_deref())?,
        tool_plans_generated: collect_tool_plans(output_root)?,
        artifact_counts: collect_artifact_counts(output_root),
        pipeline_profile: input.pipeline_profile,
        phase_results: input.phase_results,
        warnings: input.warnings,
        errors: input.errors,
    };

    utils::write_json_pretty(&path, &manifest)?;
    Ok(path)
}

pub fn default_config_path_if_exists() -> Option<PathBuf> {
    let path = Path::new("config").join("reconpilot.json");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

fn hash_file_if_exists(path: Option<&Path>) -> Result<Option<String>> {
    let Some(path) = path else {
        return Ok(None);
    };
    if !path.exists() || !path.is_file() {
        return Ok(None);
    }

    let bytes = fs::read(path)
        .with_context(|| format!("failed to read file for hashing: {}", path.display()))?;
    Ok(Some(fnv1a_hash_hex(&bytes)))
}

fn fnv1a_hash_hex(bytes: &[u8]) -> String {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    let mut hash = OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

fn render_paths(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect()
}

fn collect_tool_plans(output_root: &Path) -> Result<Vec<String>> {
    let plans_dir = output_root.join("plans");
    if !plans_dir.exists() || !plans_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut plans = fs::read_dir(&plans_dir)
        .with_context(|| format!("failed to read plans directory: {}", plans_dir.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    plans.sort();
    Ok(plans)
}

fn collect_artifact_counts(output_root: &Path) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    counts.insert("total_files".to_string(), utils::count_files(output_root));

    for label in [
        "plans",
        "raw",
        "dns",
        "tech",
        "maps",
        "assets",
        "urls",
        "js",
        "params",
        "screenshots",
        "findings",
        "enrichment",
        "api-intel",
        "review",
        "llm-pack",
        "codex-insights",
        "codex-review",
        "reports",
    ] {
        let path = output_root.join(label);
        let count = if path.exists() && path.is_dir() {
            WalkDir::new(&path)
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_file())
                .count()
        } else {
            0
        };
        counts.insert(label.to_string(), count);
    }

    counts
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use crate::{
        models::{PhaseStatus, PipelinePhase},
        utils,
    };

    use super::{write_manifest, ManifestInput};

    #[test]
    fn manifest_generation_writes_expected_fields() -> Result<()> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "reconpilot-manifest-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("plans"))?;
        fs::write(root.join("plans").join("subfinder-plan.json"), "{}")?;
        fs::write(root.join("scope.txt"), "example.com\n")?;

        let path = write_manifest(
            &root,
            ManifestInput {
                command_executed: "reconpilot run --scope scope.txt --out output/".to_string(),
                input_paths: vec![root.join("scope.txt")],
                output_paths: vec![root.join("plans")],
                scope_path: Some(root.join("scope.txt")),
                config_path: None,
                warnings: vec!["Dry-run mode".to_string()],
                errors: Vec::new(),
                pipeline_profile: None,
                phase_results: Vec::new(),
            },
        )?;

        let raw = fs::read_to_string(path)?;
        assert!(raw.contains("\"reconpilot_version\": \"0.1.0\""));
        assert!(raw.contains("\"command_executed\""));
        assert!(raw.contains("\"scope_file_hash\""));
        assert!(raw.contains("subfinder-plan.json"));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn manifest_generation_includes_pipeline_results() -> Result<()> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "reconpilot-manifest-pipeline-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("plans"))?;
        utils::write_string(&root.join("scope.txt"), "example.com\n")?;

        let path = write_manifest(
            &root,
            ManifestInput {
                command_executed:
                    "reconpilot pipeline --scope scope.txt --profile passive --out output/"
                        .to_string(),
                input_paths: vec![root.join("scope.txt")],
                output_paths: vec![root.join("plans")],
                scope_path: Some(root.join("scope.txt")),
                config_path: None,
                warnings: Vec::new(),
                errors: Vec::new(),
                pipeline_profile: Some("passive".to_string()),
                phase_results: vec![PipelinePhase {
                    name: "run".to_string(),
                    phase_type: "external-tool-phase".to_string(),
                    command: "reconpilot run --scope scope.txt --out output/".to_string(),
                    status: PhaseStatus::Completed,
                    execute_phase: false,
                    touches_targets: true,
                    required_inputs: vec!["scope.txt".to_string()],
                    expected_outputs: vec!["output/plans/tool-runs.json".to_string()],
                    notes: vec!["Dry-run completed.".to_string()],
                }],
            },
        )?;

        let raw = fs::read_to_string(path)?;
        assert!(raw.contains("\"pipeline_profile\": \"passive\""));
        assert!(raw.contains("\"phase_results\""));
        assert!(raw.contains("\"name\": \"run\""));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
