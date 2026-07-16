# First Run Checklist

Use this checklist for the first local ReconPilot setup on a new workstation.

## Prerequisites

- [ ] Install Rust
- [ ] Install Node.js and npm
- [ ] Install Tauri prerequisites
- [ ] Install any external recon tools you plan to use

## Build

- [ ] Run `cargo build`
- [ ] Run `cd gui`
- [ ] Run `npm install`
- [ ] Run `npm run build`

## Local Validation

- [ ] Run `reconpilot doctor`
- [ ] Launch the GUI with `.\scripts\launch-gui.ps1`
- [ ] Run the passive pipeline with `.\scripts\test-passive.ps1`
- [ ] Inspect `output/maps/`, `output/enrichment/`, `output/review/`, and `output/llm-pack/`
- [ ] Run `reconpilot validate --input output/`

## Optional Codex Workflow

- [ ] Run `reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/`
- [ ] Only use `--execute-codex` if you explicitly want local Codex reasoning output
- [ ] Run `reconpilot codex-review --input output/codex-insights/ --out output/codex-review/`
