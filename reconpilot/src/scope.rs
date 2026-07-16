use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeDefinition {
    pub source_path: PathBuf,
    pub entries: Vec<ScopeEntry>,
}

impl ScopeDefinition {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn domain_targets(&self) -> Vec<String> {
        let mut domains = BTreeSet::new();

        for entry in &self.entries {
            match entry.kind {
                ScopeKind::Domain => {
                    domains.insert(strip_wildcard_prefix(&entry.normalized).to_string());
                }
                ScopeKind::Url => {
                    if let Ok(parsed) = Url::parse(&entry.normalized) {
                        if let Some(host) = parsed.host_str() {
                            domains.insert(host.to_ascii_lowercase());
                        }
                    }
                }
            }
        }

        domains.into_iter().collect()
    }

    pub fn probe_targets(&self) -> Vec<String> {
        let mut targets = BTreeSet::new();

        for entry in &self.entries {
            match entry.kind {
                ScopeKind::Domain => {
                    targets.insert(strip_wildcard_prefix(&entry.normalized).to_string());
                }
                ScopeKind::Url => {
                    targets.insert(entry.normalized.clone());
                }
            }
        }

        targets.into_iter().collect()
    }

    pub fn crawl_targets(&self) -> Vec<String> {
        let mut targets = BTreeSet::new();

        for entry in &self.entries {
            match entry.kind {
                ScopeKind::Domain => {
                    let domain = strip_wildcard_prefix(&entry.normalized);
                    targets.insert(format!("https://{domain}"));
                }
                ScopeKind::Url => {
                    targets.insert(entry.normalized.clone());
                }
            }
        }

        targets.into_iter().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeEntry {
    pub raw: String,
    pub normalized: String,
    pub kind: ScopeKind,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Domain,
    Url,
}

#[derive(Debug, Error)]
pub enum ScopeError {
    #[error("scope file does not exist: {0}")]
    Missing(PathBuf),
    #[error("scope file is empty after removing comments and blank lines: {0}")]
    Empty(PathBuf),
    #[error("invalid scope target: {0}")]
    InvalidTarget(String),
}

pub fn load_scope(path: &Path) -> Result<ScopeDefinition> {
    if !path.exists() {
        return Err(ScopeError::Missing(path.to_path_buf()).into());
    }

    let entries = load_scope_entries(path)?;

    if entries.is_empty() {
        return Err(ScopeError::Empty(path.to_path_buf()).into());
    }

    Ok(ScopeDefinition {
        source_path: path.to_path_buf(),
        entries,
    })
}

pub fn load_scope_entries(path: &Path) -> Result<Vec<ScopeEntry>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read scope file: {}", path.display()))?;
    let mut entries = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        entries.push(parse_scope_target(trimmed)?);
    }

    Ok(entries)
}

pub fn load_optional_exclusions(path: Option<&Path>) -> Result<Vec<ScopeEntry>> {
    let Some(path) = path else {
        return Ok(Vec::new());
    };
    if !path.exists() {
        return Ok(Vec::new());
    }

    load_scope_entries(path)
}

pub fn validate_domain(value: &str) -> bool {
    let domain_pattern =
        Regex::new(r"(?i)^(?:\*\.)?(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$")
            .expect("domain regex must compile");

    domain_pattern.is_match(value)
}

fn parse_scope_target(value: &str) -> Result<ScopeEntry> {
    if value.starts_with("http://") || value.starts_with("https://") {
        let parsed = Url::parse(value).map_err(|_| ScopeError::InvalidTarget(value.to_string()))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| ScopeError::InvalidTarget(value.to_string()))?;

        if !validate_domain(host) {
            return Err(ScopeError::InvalidTarget(value.to_string()).into());
        }

        return Ok(ScopeEntry {
            raw: value.to_string(),
            normalized: parsed.to_string(),
            kind: ScopeKind::Url,
        });
    }

    if validate_domain(value) {
        return Ok(ScopeEntry {
            raw: value.to_string(),
            normalized: value.to_ascii_lowercase(),
            kind: ScopeKind::Domain,
        });
    }

    Err(ScopeError::InvalidTarget(value.to_string()).into())
}

fn strip_wildcard_prefix(value: &str) -> &str {
    value.strip_prefix("*.").unwrap_or(value)
}
