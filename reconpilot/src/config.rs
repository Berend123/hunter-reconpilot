use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    profiles,
    scope::{self, ScopeDefinition, ScopeEntry, ScopeKind},
    tools,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconPilotConfig {
    pub profile_name: String,
    pub passive_only: bool,
    pub allow_port_scans: bool,
    pub max_concurrency: usize,
    pub request_delay_ms: u64,
    pub output_root: String,
    pub user_agent: String,
    pub enabled_tool_groups: Vec<String>,
    pub score_keywords: Vec<String>,
    #[serde(default = "default_max_context_chars")]
    pub max_context_chars: usize,
    #[serde(default = "default_safety_mode")]
    pub safety_mode: String,
}

impl Default for ReconPilotConfig {
    fn default() -> Self {
        Self {
            profile_name: "passive".to_string(),
            passive_only: true,
            allow_port_scans: false,
            max_concurrency: 4,
            request_delay_ms: 250,
            output_root: "output".to_string(),
            user_agent: "ReconPilot/0.1.0".to_string(),
            enabled_tool_groups: vec![
                "core-discovery".to_string(),
                "live-host-probing".to_string(),
                "historical-urls".to_string(),
                "normalization".to_string(),
            ],
            score_keywords: vec![
                "admin".to_string(),
                "login".to_string(),
                "debug".to_string(),
                "internal".to_string(),
                "staging".to_string(),
                "token".to_string(),
                "upload".to_string(),
            ],
            max_context_chars: default_max_context_chars(),
            safety_mode: default_safety_mode(),
        }
    }
}

fn default_max_context_chars() -> usize {
    12_000
}

fn default_safety_mode() -> String {
    "strict-local".to_string()
}

#[derive(Debug, Error)]
pub enum ConfigLoadError {
    #[error("unsupported config format: {0}")]
    UnsupportedFormat(PathBuf),
}

#[derive(Debug, Clone, Default)]
pub struct ConfigValidationReport {
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

pub fn load_from_path(path: &Path) -> Result<ReconPilotConfig> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    let config = match extension.as_deref() {
        Some("json") => serde_json::from_str::<ReconPilotConfig>(&raw)
            .with_context(|| format!("failed to parse JSON config: {}", path.display()))?,
        Some("toml") => toml::from_str::<ReconPilotConfig>(&raw)
            .with_context(|| format!("failed to parse TOML config: {}", path.display()))?,
        _ => return Err(ConfigLoadError::UnsupportedFormat(path.to_path_buf()).into()),
    };

    ensure_valid_config(&config)?;
    Ok(config)
}

pub fn load_default_or_file(path: Option<&Path>) -> Result<ReconPilotConfig> {
    if let Some(path) = path {
        return load_from_path(path);
    }

    let default_path = Path::new("config").join("reconpilot.json");
    if default_path.exists() {
        load_from_path(&default_path)
    } else {
        let config = ReconPilotConfig::default();
        ensure_valid_config(&config)?;
        Ok(config)
    }
}

pub fn default_exclusion_path_if_exists() -> Option<PathBuf> {
    let path = Path::new("config").join("excluded.txt");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

pub fn example_scope_path() -> PathBuf {
    Path::new("config").join("scope.example.txt")
}

pub fn example_exclusion_path() -> PathBuf {
    Path::new("config").join("excluded.example.txt")
}

pub fn example_config_path() -> PathBuf {
    Path::new("config").join("reconpilot.example.json")
}

pub fn inspect_config(config: &ReconPilotConfig) -> ConfigValidationReport {
    let mut report = ConfigValidationReport::default();

    let supported_profiles = profiles::supported_profiles();
    if !supported_profiles.contains(&config.profile_name.as_str()) {
        report.errors.push(format!(
            "config profile_name '{}' is not supported; use one of: {}",
            config.profile_name,
            supported_profiles.join(", ")
        ));
    }

    let output_root = Path::new(&config.output_root);
    if config.output_root.trim().is_empty() {
        report
            .errors
            .push("config output_root cannot be empty".to_string());
    } else if output_root.has_root() && output_root.file_name().is_none() {
        report.errors.push(format!(
            "config output_root '{}' cannot be a filesystem root",
            config.output_root
        ));
    }

    if !(1_000..=100_000).contains(&config.max_context_chars) {
        report.errors.push(format!(
            "config max_context_chars '{}' is outside the supported range 1000..=100000",
            config.max_context_chars
        ));
    }

    if !matches!(
        config.safety_mode.as_str(),
        "strict-local" | "passive-first" | "active-lite"
    ) {
        report.errors.push(format!(
            "config safety_mode '{}' is invalid; use strict-local, passive-first, or active-lite",
            config.safety_mode
        ));
    }

    if config.max_concurrency == 0 {
        report
            .errors
            .push("config max_concurrency must be greater than zero".to_string());
    }

    let known_groups = tools::registry()
        .into_iter()
        .map(|tool| tool.category.to_string())
        .collect::<std::collections::BTreeSet<_>>();
    for group in &config.enabled_tool_groups {
        if !known_groups.contains(group) {
            report.warnings.push(format!(
                "enabled_tool_groups contains unknown category '{}'; verify the config matches the current tool registry",
                group
            ));
        }
    }

    if config.passive_only && config.allow_port_scans {
        report.warnings.push(
            "passive_only is true while allow_port_scans is also true; port scanning still requires explicit operator authorization.".to_string(),
        );
    }

    report
}

pub fn ensure_valid_config(config: &ReconPilotConfig) -> Result<ConfigValidationReport> {
    let report = inspect_config(config);
    if !report.errors.is_empty() {
        bail!("config validation failed: {}", report.errors.join(" | "));
    }
    Ok(report)
}

pub fn validate_scope_exclusion_consistency(
    scope: &ScopeDefinition,
    exclusions_path: Option<&Path>,
) -> Result<Vec<String>> {
    let exclusions = scope::load_optional_exclusions(exclusions_path)?;
    if exclusions.is_empty() {
        return Ok(Vec::new());
    }

    let mut warnings = Vec::new();
    let mut exact_conflicts = Vec::new();
    let mut overlap_count = 0usize;

    for exclusion in &exclusions {
        if scope
            .entries
            .iter()
            .any(|entry| entry.normalized == exclusion.normalized)
        {
            exact_conflicts.push(exclusion.normalized.clone());
        }
        if scope
            .entries
            .iter()
            .any(|entry| entries_overlap(entry, exclusion))
        {
            overlap_count += 1;
        }
    }

    if !exact_conflicts.is_empty() {
        bail!(
            "scope/exclusion consistency failed: exclusions exactly match scoped targets: {}",
            exact_conflicts.join(", ")
        );
    }

    if overlap_count == 0 {
        warnings.push(
            "the configured exclusions do not overlap the current scope; verify that the intended exclusion file was selected."
                .to_string(),
        );
    }

    Ok(warnings)
}

fn entries_overlap(scope_entry: &ScopeEntry, exclusion: &ScopeEntry) -> bool {
    match (scope_entry.kind, exclusion.kind) {
        (ScopeKind::Domain, ScopeKind::Domain) => {
            exclusion.normalized == scope_entry.normalized
                || exclusion.normalized.ends_with(&format!(
                    ".{}",
                    scope_entry.normalized.trim_start_matches("*.")
                ))
        }
        (ScopeKind::Domain, ScopeKind::Url) => exclusion
            .normalized
            .parse::<url::Url>()
            .ok()
            .and_then(|url| url.host_str().map(ToOwned::to_owned))
            .map(|host| host.ends_with(scope_entry.normalized.trim_start_matches("*.")))
            .unwrap_or(false),
        (ScopeKind::Url, ScopeKind::Domain) => scope_entry
            .normalized
            .parse::<url::Url>()
            .ok()
            .and_then(|url| url.host_str().map(ToOwned::to_owned))
            .map(|host| host.ends_with(exclusion.normalized.trim_start_matches("*.")))
            .unwrap_or(false),
        (ScopeKind::Url, ScopeKind::Url) => {
            scope_entry.normalized.contains(&exclusion.normalized)
                || exclusion.normalized.contains(&scope_entry.normalized)
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

    use super::{
        ensure_valid_config, inspect_config, validate_scope_exclusion_consistency, ReconPilotConfig,
    };
    use crate::scope::load_scope;

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
                "reconpilot-config-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root)?;
            Ok(Self { root })
        }

        fn write_file(&self, relative: &str, content: &str) -> Result<PathBuf> {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&path, content)?;
            Ok(path)
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn config_validation_accepts_valid_defaults() -> Result<()> {
        let config = ReconPilotConfig::default();
        let report = ensure_valid_config(&config)?;
        assert!(report.errors.is_empty());
        Ok(())
    }

    #[test]
    fn config_validation_rejects_invalid_profile_and_bounds() {
        let mut config = ReconPilotConfig::default();
        config.profile_name = "active-light".to_string();
        config.max_context_chars = 10;
        config.safety_mode = "unsafe".to_string();

        let report = inspect_config(&config);
        assert!(!report.errors.is_empty());
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("profile_name")));
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("max_context_chars")));
        assert!(report
            .errors
            .iter()
            .any(|error| error.contains("safety_mode")));
    }

    #[test]
    fn scope_exclusion_consistency_rejects_exact_conflicts() -> Result<()> {
        let workspace = TestWorkspace::new("scope-conflict")?;
        let scope_path = workspace.write_file("scope.txt", "example.com\n")?;
        let exclusion_path = workspace.write_file("excluded.txt", "example.com\n")?;
        let scope = load_scope(&scope_path)?;

        let result = validate_scope_exclusion_consistency(&scope, Some(&exclusion_path));
        assert!(result.is_err());
        Ok(())
    }
}
