# GUI Phase 14 Full Implementation

This document now reflects the Phase 15 hardening pass on top of the Phase 14 implementation.

## Goal

Phase 14 implements the first full ReconPilot desktop GUI shell while preserving the existing CLI as the source of truth.

The GUI is built under `gui/` with:

- Tauri
- React
- TypeScript
- Vite

## What Was Implemented

### Desktop shell

- `gui/src-tauri/`
- Tauri command bridge for loading workspace artifacts
- structured allowlist for running only approved `reconpilot` commands

### Frontend

- multi-screen React application
- workspace-aware artifact viewer
- command preview and execution controls
- beginner and advanced mode handling
- default-on redaction for sensitive-looking values
- review queue filtering and sorting
- rendered markdown asset cards
- grouped validation and audit views
- friendlier empty states and command-result panels

## Phase 15 Hardening Additions

- improved workspace root detection and auto-resolution of `config/`, `output/`, and `docs/`
- clearer workspace health status with required vs optional artifact checks
- pipeline phase status badges and execution history list
- review queue filtering by risk, role, environment, and search
- asset detail links to Codex insight and Codex review context
- grouped validation warnings/errors and GUI execution log visibility
- smoke tests for command builders, redaction, and workspace artifact helpers

### Screens

Implemented GUI screens:

1. Dashboard
2. Workspace Selector
3. Scope Manager
4. Pipeline Runner
5. Profile Editor
6. Tool Settings
7. Review Queue
8. Asset Detail
9. Graph Viewer
10. API Intelligence
11. Enrichment Viewer
12. LLM Pack Viewer
13. Codex Runner
14. Codex Insights
15. Codex Review
16. Validation / Audit
17. Settings

## Execution Model

The GUI does not run arbitrary tools directly.

It only runs structured allowlisted `reconpilot` commands such as:

- `doctor`
- `pipeline`
- `validate`
- `codex-run`
- `codex-review`

The GUI backend:

- resolves the ReconPilot binary from known locations
- builds arguments from structured UI state
- rejects unsupported command kinds
- captures stdout and stderr
- appends `output/gui-execution-log.jsonl`

## Safety Summary

- dry-run remains the default
- scope file remains mandatory for target-touching pipeline use
- `--execute` never implies `--execute-codex`
- Codex execution has its own explicit control path
- no arbitrary shell entry exists in the GUI
- no hidden flags are appended
- no direct target contact comes from frontend code

## Build Notes

Validated in this phase:

```powershell
cd gui
npm install
npm test
npm run build
```

The root CLI validation still remains:

```powershell
cargo fmt
cargo test
cargo build
```

## Current Limits

- the GUI does not replace CLI safety enforcement
- custom profiles and custom tool args are GUI-managed configuration, not new core CLI behavior
- the first release favors artifact viewing and safe command launching over heavy visualization dependencies

## Next Likely Work

- richer graph canvas interactions
- larger artifact pagination and filtering
- run history views and diffing
- deeper Tauri runtime testing on packaged builds
