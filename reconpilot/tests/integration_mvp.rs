use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn reconpilot_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_reconpilot"))
}

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "reconpilot-integration-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("temporary integration directory should be created");
    root
}

fn write_file(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory should be created");
    }
    fs::write(path, content).expect("file should be written");
}

#[test]
fn doctor_command_runs() {
    let output = Command::new(reconpilot_bin())
        .arg("doctor")
        .current_dir(repo_root())
        .output()
        .expect("doctor command should run");

    assert!(
        output.status.success(),
        "doctor stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ReconPilot doctor summary"));
    assert!(stdout.contains("Dry-run default"));
}

#[test]
fn config_validation_rejects_invalid_runtime_config() {
    let root = temp_root("invalid-config");
    write_file(
        &root,
        "config/reconpilot.json",
        r#"{
  "profile_name": "active-light",
  "passive_only": true,
  "allow_port_scans": false,
  "max_concurrency": 4,
  "request_delay_ms": 250,
  "output_root": "output",
  "user_agent": "ReconPilot/0.1.0",
  "enabled_tool_groups": ["core-discovery"],
  "score_keywords": ["admin"],
  "max_context_chars": 10,
  "safety_mode": "unsafe"
}"#,
    );
    write_file(&root, "scope.txt", "example.com\n");

    let output = Command::new(reconpilot_bin())
        .args(["plan", "--scope", "scope.txt"])
        .current_dir(&root)
        .output()
        .expect("plan command should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("config validation failed"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn passive_pipeline_planning_generates_plan_artifacts() {
    let root = temp_root("passive-pipeline");
    let output_root = root.join("output");

    let output = Command::new(reconpilot_bin())
        .args([
            "pipeline",
            "--scope",
            "config/scope.example.txt",
            "--profile",
            "passive",
            "--out",
            output_root
                .to_str()
                .expect("temp output path should be utf-8"),
        ])
        .current_dir(repo_root())
        .output()
        .expect("pipeline command should run");

    assert!(
        output.status.success(),
        "pipeline stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_root
        .join("plans")
        .join("pipeline-plan.json")
        .exists());
    assert!(output_root.join("plans").join("pipeline-plan.md").exists());

    let raw = fs::read_to_string(output_root.join("plans").join("pipeline-plan.json"))
        .expect("plan json should exist");
    assert!(raw.contains("\"name\": \"passive\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_profile_rejection_is_clear() {
    let root = temp_root("invalid-profile");
    let output = Command::new(reconpilot_bin())
        .args([
            "pipeline",
            "--scope",
            "config/scope.example.txt",
            "--profile",
            "not-a-profile",
            "--out",
            root.join("output")
                .to_str()
                .expect("output path should be utf-8"),
        ])
        .current_dir(repo_root())
        .output()
        .expect("pipeline command should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported pipeline profile"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_scope_rejection_is_clear() {
    let root = temp_root("missing-scope");
    let output = Command::new(reconpilot_bin())
        .args([
            "pipeline",
            "--scope",
            "missing-scope.txt",
            "--profile",
            "passive",
            "--out",
            root.join("output")
                .to_str()
                .expect("output path should be utf-8"),
        ])
        .current_dir(repo_root())
        .output()
        .expect("pipeline command should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("scope file does not exist"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn output_docs_existence() {
    for relative in [
        "CHANGELOG.md",
        "ROADMAP.md",
        "QUICKSTART.md",
        "docs/MVP_USAGE.md",
        "docs/PIPELINE_PROFILES.md",
        "docs/OUTPUTS.md",
    ] {
        assert!(
            repo_root().join(relative).exists(),
            "expected doc should exist: {relative}"
        );
    }
}
