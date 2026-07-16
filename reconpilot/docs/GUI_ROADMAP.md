# GUI Roadmap

## Goal

Break the future ReconPilot desktop GUI into staged delivery milestones.

Phase 13 was planning only.

Phase 14 is now implemented as the first full GUI release:

- Tauri + React + TypeScript + Vite scaffold
- multi-screen desktop UI
- safe CLI command bridge
- beginner and advanced modes
- output-first artifact viewing plus explicit command launching

Phase 15 is also implemented:

- daily-use UX hardening
- clearer workspace health detection
- review filtering and richer validation views
- frontend smoke tests and build validation

Current release status:

- suitable for local v0.1.0 release-candidate use
- still intentionally bound to CLI safety gates and allowlisted execution

## Phase 13: Planning Complete

Outputs produced in this phase:

- GUI architecture specification
- GUI UX specification
- GUI data contracts
- GUI security model
- GUI delivery roadmap

## Phase 14: Full GUI Foundation

Deliverables:

- Tauri workspace scaffold
- React + TypeScript frontend scaffold
- workspace open flow
- dashboard, review, graph, API, enrichment, validation, and Codex screens
- profile editor and tool settings
- pipeline runner with exact command preview
- beginner and advanced mode controls
- redaction-aware output rendering
- allowlisted ReconPilot command launching only

Constraints:

- no recon behavior changes
- no new scanning behavior
- no direct target contact
- no arbitrary shell execution
- no hidden execute state
- no automatic Codex execution

## Phase 15: GUI Refinement And Packaging

Deliverables:

- completed GUI error-state hardening
- improved workspace detection and health reporting
- richer review queue filtering and sorting
- rendered asset-card and Codex-link views
- stronger validation, audit, and execution-log surfaces
- frontend smoke tests for critical helper logic

Focus:

- daily local usability
- safer operator feedback loops
- better artifact-scale ergonomics
- packaging readiness next

Status:

- implemented

## Phase 16: Graph And API Visualization

Deliverables:

- richer graph canvas interactions
- improved API object navigation
- deeper semantic cross-linking

Focus:

- visual scale and interaction quality
- graph-centric analyst workflow refinement

## Phase 17: LLM And Codex Review Surfaces

Deliverables:

- analyst notes on reasoning artifacts
- cross-run Codex result comparison
- stronger validation-to-reasoning gating in the UI

Focus:

- reasoning review quality and traceability

## Phase 18: Controlled Command Launching

Deliverables:

- more complete CLI surface coverage where safe
- stronger run-status monitoring
- more granular command audit UX

Focus:

- command observability and operator confidence

## Phase 19: Workspace Productivity

Deliverables:

- saved filters
- analyst notes
- run history browsing
- artifact diffing between runs

## Long-Term Ideas

- graph cluster heat maps
- richer large-artifact pagination and search
- per-workspace redaction policies
- optional local database-backed caching for UI responsiveness
- side-by-side comparison of Codex results across prompt templates

## Reference Docs

- [docs/GUI_ARCHITECTURE.md](GUI_ARCHITECTURE.md)
- [docs/GUI_UX_SPEC.md](GUI_UX_SPEC.md)
- [docs/GUI_DATA_CONTRACTS.md](GUI_DATA_CONTRACTS.md)
- [docs/GUI_SECURITY_MODEL.md](GUI_SECURITY_MODEL.md)
- [docs/GUI_PHASE_14_FULL.md](GUI_PHASE_14_FULL.md)
- [docs/GUI_ADVANCED_MODE.md](GUI_ADVANCED_MODE.md)
