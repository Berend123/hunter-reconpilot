# Roadmap

## Post-v0.1 Near Term

- stabilize JSON and JSONL schemas across all generated artifacts
- add more fixture-driven integration coverage for pipeline profiles
- improve config-driven policy controls for rate limits and allowed phases
- add more tool-specific installer guidance for Windows environments
- refine validation coverage for optional artifacts and cross-phase lineage
- harden the new Phase 14 GUI against larger local datasets and packaging edge cases
- ship packaged local binaries so operator workflows do not depend on a development toolchain layout

## Post-v0.1 Medium Term

- expand passive adapters beyond the current MVP set
- add caching and run-diff support for repeat local workflows
- improve graph exports and local analytics workflows
- extend API intelligence with richer schema and source-map parsing
- add better filtering and normalization around JavaScript-derived routes
- refine GUI graph navigation, workspace history, and run monitoring

## Post-v0.1 Longer Term

- local model invocation behind explicit operator control
- graph-native reporting and review navigation
- richer screenshot and favicon similarity analysis
- stronger policy files for scoped execution approvals
- optional database exports for local graph and artifact analysis
- controlled GUI command launching with explicit confirmations and audit visibility

## GUI Planning

Phase 13 added planning-only GUI documents:

- [docs/GUI_ARCHITECTURE.md](docs/GUI_ARCHITECTURE.md)
- [docs/GUI_UX_SPEC.md](docs/GUI_UX_SPEC.md)
- [docs/GUI_DATA_CONTRACTS.md](docs/GUI_DATA_CONTRACTS.md)
- [docs/GUI_SECURITY_MODEL.md](docs/GUI_SECURITY_MODEL.md)
- [docs/GUI_ROADMAP.md](docs/GUI_ROADMAP.md)

Phase 14 is now implemented:

- full Tauri + React + TypeScript GUI under `gui/`
- beginner and advanced modes
- safe command preview and allowlisted GUI execution bridge

Next GUI focus:

- packaging and runtime hardening
- larger-artifact UX refinement
- graph and history interaction improvements
