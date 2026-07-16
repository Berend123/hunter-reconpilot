use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{config, tools, utils};

#[derive(Debug, Clone)]
pub struct DoctorCheck {
    pub name: String,
    pub passed: bool,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub generated_at: DateTime<Utc>,
    pub os: String,
    pub arch: String,
    pub version: String,
    pub checks: Vec<DoctorCheck>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

pub fn run_doctor() -> Result<DoctorReport> {
    let mut report = DoctorReport {
        generated_at: Utc::now(),
        os: env::consts::OS.to_string(),
        arch: env::consts::ARCH.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        checks: Vec::new(),
        warnings: Vec::new(),
        errors: Vec::new(),
    };

    check_runtime_binaries(&mut report);
    check_config_examples(&mut report)?;
    check_output_writability(&mut report)?;
    check_required_docs(&mut report);
    check_external_tools(&mut report);

    Ok(report)
}

pub fn print_doctor_summary(report: &DoctorReport) {
    println!("ReconPilot doctor summary");
    println!("Generated at: {}", report.generated_at.to_rfc3339());
    println!("Version: {}", report.version);
    println!("OS: {} ({})", report.os, report.arch);
    println!(
        "Status: {}",
        if report.errors.is_empty() {
            "ready for local MVP use"
        } else {
            "needs attention"
        }
    );
    println!("Dry-run default: external-tool phases still require --execute.");
    println!("Safety notice: doctor is local-only and does not contact targets.");
    println!();

    for check in &report.checks {
        println!(
            "- [{}] {}: {}",
            if check.passed {
                "pass"
            } else {
                check.severity.as_str()
            },
            check.name,
            check.message
        );
    }

    if !report.warnings.is_empty() {
        println!();
        println!("Warnings:");
        for warning in &report.warnings {
            println!("- {warning}");
        }
    }

    if !report.errors.is_empty() {
        println!();
        println!("Errors:");
        for error in &report.errors {
            println!("- {error}");
        }
    }
}

fn check_runtime_binaries(report: &mut DoctorReport) {
    for binary in ["cargo", "rustc", "git", "python", "go", "pwsh"] {
        if let Some(path) = resolve_binary_path(binary) {
            report.checks.push(DoctorCheck {
                name: format!("runtime:{binary}"),
                passed: true,
                severity: "info".to_string(),
                message: format!("Found {} at {}", binary, path.display()),
            });
        } else {
            let severity = if matches!(binary, "cargo" | "rustc") {
                "error"
            } else {
                "warning"
            };
            let message = format!("{binary} was not found on PATH");
            report.checks.push(DoctorCheck {
                name: format!("runtime:{binary}"),
                passed: severity != "error",
                severity: severity.to_string(),
                message: message.clone(),
            });
            if severity == "error" {
                report.errors.push(message);
            } else {
                report.warnings.push(message);
            }
        }
    }
}

fn check_config_examples(report: &mut DoctorReport) -> Result<()> {
    for path in [
        config::example_config_path(),
        config::example_scope_path(),
        config::example_exclusion_path(),
    ] {
        if path.exists() && path.is_file() {
            report.checks.push(DoctorCheck {
                name: format!("example:{}", path.display()),
                passed: true,
                severity: "info".to_string(),
                message: format!("Example file is present: {}", path.display()),
            });
        } else {
            let message = format!("Required example file is missing: {}", path.display());
            report.checks.push(DoctorCheck {
                name: format!("example:{}", path.display()),
                passed: false,
                severity: "error".to_string(),
                message: message.clone(),
            });
            report.errors.push(message);
        }
    }

    let example_config = config::load_from_path(&config::example_config_path())?;
    let validation = config::ensure_valid_config(&example_config)?;
    report.warnings.extend(validation.warnings.clone());
    report.checks.push(DoctorCheck {
        name: "config:example-validation".to_string(),
        passed: true,
        severity: "info".to_string(),
        message: "reconpilot.example.json parsed and passed validation.".to_string(),
    });

    let scope = crate::scope::load_scope(&config::example_scope_path())?;
    let warnings = config::validate_scope_exclusion_consistency(
        &scope,
        Some(&config::example_exclusion_path()),
    )?;
    for warning in warnings {
        report.warnings.push(warning.clone());
        report.checks.push(DoctorCheck {
            name: "config:scope-exclusion-consistency".to_string(),
            passed: true,
            severity: "warning".to_string(),
            message: warning,
        });
    }

    Ok(())
}

fn check_output_writability(report: &mut DoctorReport) -> Result<()> {
    let config = config::load_default_or_file(None)?;
    let output_root = Path::new(&config.output_root);
    let layout = utils::ensure_output_structure(output_root)?;
    let probe_path = layout.root.join(".doctor-write-test");
    fs::write(&probe_path, "ok")?;
    let _ = fs::remove_file(&probe_path);
    report.checks.push(DoctorCheck {
        name: "output:writable".to_string(),
        passed: true,
        severity: "info".to_string(),
        message: format!("Output root is writable: {}", layout.root.display()),
    });
    Ok(())
}

fn check_required_docs(report: &mut DoctorReport) {
    for path in required_docs() {
        if path.exists() && path.is_file() {
            report.checks.push(DoctorCheck {
                name: format!("docs:{}", path.display()),
                passed: true,
                severity: "info".to_string(),
                message: format!("Required doc is present: {}", path.display()),
            });
        } else {
            let message = format!("Required doc is missing: {}", path.display());
            report.checks.push(DoctorCheck {
                name: format!("docs:{}", path.display()),
                passed: false,
                severity: "error".to_string(),
                message: message.clone(),
            });
            report.errors.push(message);
        }
    }
}

fn check_external_tools(report: &mut DoctorReport) {
    let registry = tools::registry();
    let mut missing_core = Vec::new();
    let mut missing_optional = Vec::new();
    let mut present_by_category = BTreeMap::<String, usize>::new();

    for tool in registry {
        let binary_name = if tool.name == "WhatWeb" {
            "whatweb"
        } else {
            tool.name
        };
        if resolve_binary_path(binary_name).is_some() {
            *present_by_category
                .entry(tool.category.to_string())
                .or_insert(0usize) += 1;
        } else if tool.core {
            missing_core.push(tool.name.to_string());
        } else {
            missing_optional.push(tool.name.to_string());
        }
    }

    report.checks.push(DoctorCheck {
        name: "tools:registry-scan".to_string(),
        passed: true,
        severity: "info".to_string(),
        message: format!(
            "Scanned {} tool definitions across {} categories.",
            tools::registry().len(),
            present_by_category.len()
        ),
    });

    if !missing_core.is_empty() {
        let message = format!(
            "Core external tools are not currently on PATH: {}",
            missing_core.join(", ")
        );
        report.warnings.push(message.clone());
        report.checks.push(DoctorCheck {
            name: "tools:core-availability".to_string(),
            passed: true,
            severity: "warning".to_string(),
            message,
        });
    }

    if !missing_optional.is_empty() {
        let message = format!(
            "Optional external tools are not currently on PATH: {}",
            missing_optional.join(", ")
        );
        report.warnings.push(message.clone());
        report.checks.push(DoctorCheck {
            name: "tools:optional-availability".to_string(),
            passed: true,
            severity: "warning".to_string(),
            message,
        });
    }
}

fn required_docs() -> Vec<PathBuf> {
    vec![
        Path::new("README.md").to_path_buf(),
        Path::new("TOOLS.md").to_path_buf(),
        Path::new("ORCHESTRATION.md").to_path_buf(),
        Path::new("SAFETY_AND_SCOPE.md").to_path_buf(),
        Path::new("CHANGELOG.md").to_path_buf(),
        Path::new("ROADMAP.md").to_path_buf(),
        Path::new("QUICKSTART.md").to_path_buf(),
        Path::new("docs").join("MVP_USAGE.md"),
        Path::new("docs").join("PIPELINE_PROFILES.md"),
        Path::new("docs").join("OUTPUTS.md"),
    ]
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

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::run_doctor;

    #[test]
    fn doctor_report_generation_returns_runtime_summary() -> Result<()> {
        let report = run_doctor()?;
        assert_eq!(report.version, env!("CARGO_PKG_VERSION"));
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "config:example-validation"));
        Ok(())
    }
}
