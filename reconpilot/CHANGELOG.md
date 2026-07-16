# Changelog

## 0.1.0

First local MVP release candidate.

### Added

- dry-run-safe external tool adapters for `subfinder`, `httpx`, `katana`, `gau`, `dnsx`, `gowitness`, and `WhatWeb`
- graph and correlation engine with deterministic clustering and anomaly candidates
- semantic enrichment with graph-aware and API-aware observations
- review workspace with ranked queues, asset cards, and evidence indexes
- local-only API and JavaScript intelligence layer
- local-only LLM context pack generation
- run manifest, audit log, and validation reporting
- named pipeline profiles with plan generation and profile-aware manifests
- config validation and local MVP doctor command

### Safety

- external-tool phases remain dry-run by default
- `--execute` is still required before target-touching adapters launch
- local-only phases remain local-only
- ReconPilot remains recon-only and does not ship exploit tooling

### Documentation

- added MVP quickstart, usage, profile, and output references
- refreshed core architecture and safety documentation for the Phase 10 MVP milestone
- added release checklist and known limitations references for the v0.1.0 release candidate

### Release Candidate Prep

- smoke-verified the local passive pipeline, validation, llm-pack, codex plan generation, and codex review workflow
- rechecked GUI local commands and build steps for the current desktop workflow
