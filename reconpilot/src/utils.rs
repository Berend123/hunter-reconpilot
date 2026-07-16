use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct OutputLayout {
    pub root: PathBuf,
    pub raw: PathBuf,
    pub plans: PathBuf,
    pub dns: PathBuf,
    pub tech: PathBuf,
    pub maps: PathBuf,
    pub assets: PathBuf,
    pub urls: PathBuf,
    pub js: PathBuf,
    pub params: PathBuf,
    pub screenshots: PathBuf,
    pub findings: PathBuf,
    pub enrichment: PathBuf,
    pub api_intel: PathBuf,
    pub review: PathBuf,
    pub llm_pack: PathBuf,
    pub codex_insights: PathBuf,
    pub codex_review: PathBuf,
    pub reports: PathBuf,
}

pub fn ensure_directory(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        bail!("path cannot be empty");
    }

    if path.has_root() && path.file_name().is_none() {
        bail!("refusing to use a filesystem root as an output directory");
    }

    fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory: {}", path.display()))?;
    Ok(())
}

pub fn ensure_output_structure(root: &Path) -> Result<OutputLayout> {
    ensure_directory(root)?;

    let layout = OutputLayout {
        root: root.to_path_buf(),
        raw: root.join("raw"),
        plans: root.join("plans"),
        dns: root.join("dns"),
        tech: root.join("tech"),
        maps: root.join("maps"),
        assets: root.join("assets"),
        urls: root.join("urls"),
        js: root.join("js"),
        params: root.join("params"),
        screenshots: root.join("screenshots"),
        findings: root.join("findings"),
        enrichment: root.join("enrichment"),
        api_intel: root.join("api-intel"),
        review: root.join("review"),
        llm_pack: root.join("llm-pack"),
        codex_insights: root.join("codex-insights"),
        codex_review: root.join("codex-review"),
        reports: root.join("reports"),
    };

    for path in [
        &layout.raw,
        &layout.plans,
        &layout.dns,
        &layout.tech,
        &layout.maps,
        &layout.assets,
        &layout.urls,
        &layout.js,
        &layout.params,
        &layout.screenshots,
        &layout.findings,
        &layout.enrichment,
        &layout.api_intel,
        &layout.review,
        &layout.llm_pack,
        &layout.codex_insights,
        &layout.codex_review,
        &layout.reports,
    ] {
        ensure_directory(path)?;
    }

    Ok(layout)
}

pub fn count_files(path: &Path) -> usize {
    if !path.exists() {
        return 0;
    }

    WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .count()
}

pub fn read_trimmed_lines(path: &Path) -> Result<Vec<String>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read file: {}", path.display()))?;

    Ok(raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

pub fn write_lines(path: &Path, lines: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_directory(parent)?;
    }

    let content = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };

    fs::write(path, content)
        .with_context(|| format!("failed to write file: {}", path.display()))?;
    Ok(())
}

pub fn write_string(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_directory(parent)?;
    }

    fs::write(path, content)
        .with_context(|| format!("failed to write file: {}", path.display()))?;
    Ok(())
}

pub fn write_json_pretty<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let content = serde_json::to_string_pretty(value)
        .with_context(|| format!("failed to serialize JSON for {}", path.display()))?;
    write_string(path, &content)
}
