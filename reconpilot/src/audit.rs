use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::utils;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub phase: String,
    pub message: String,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub details: BTreeMap<String, String>,
}

impl AuditEvent {
    pub fn new(
        event_type: &str,
        phase: &str,
        message: impl Into<String>,
        paths: Vec<String>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type: event_type.to_string(),
            phase: phase.to_string(),
            message: message.into(),
            paths,
            details: BTreeMap::new(),
        }
    }
}

pub fn append_audit_event(output_root: &Path, event: &AuditEvent) -> Result<PathBuf> {
    utils::ensure_directory(output_root)?;
    let path = output_root.join("audit-log.jsonl");
    let line = serde_json::to_string(event)
        .with_context(|| format!("failed to serialize audit event for {}", path.display()))?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open audit log: {}", path.display()))?;
    writeln!(file, "{line}")
        .with_context(|| format!("failed to append audit event to {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;

    use super::{append_audit_event, AuditEvent};

    #[test]
    fn audit_event_append_writes_jsonl_line() -> Result<()> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("reconpilot-audit-{}-{unique}", std::process::id()));
        fs::create_dir_all(&root)?;

        let path = append_audit_event(
            &root,
            &AuditEvent::new(
                "phase_started",
                "review",
                "Review phase started.",
                vec!["output/review".to_string()],
            ),
        )?;
        let raw = fs::read_to_string(path)?;
        assert!(raw.contains("\"event_type\":\"phase_started\""));
        assert!(raw.contains("\"phase\":\"review\""));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
