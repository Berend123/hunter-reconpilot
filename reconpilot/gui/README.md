# ReconPilot GUI

This directory contains the ReconPilot desktop GUI after the Phase 15 hardening and UX polish pass.

Stack:

- Tauri
- React
- TypeScript
- Vite

Design constraints:

- the existing ReconPilot CLI remains the source of truth
- the GUI reads local artifacts from `output/`
- the GUI only runs the `reconpilot` binary through a structured allowlist
- no arbitrary shell execution is permitted
- dry-run remains the default
- `--execute` never implies `--execute-codex`
- Codex execution remains explicit and separate

## Modes

### Beginner mode

- strong warnings
- dry-run default
- no custom tool args
- no custom profile editing
- confirmation required for execution
- friendlier missing-artifact and command-failure states
- clearer workspace health messaging

### Advanced mode

- dry-run still default
- user-defined profiles
- controlled custom tool args stored in GUI config
- rate and concurrency placeholders
- fewer repeated prompts after acknowledgement
- exact command preview remains mandatory
- GUI execution log visibility remains enabled

## Phase 15 Improvements

- workspace root vs `output/` vs `config/` vs `docs/` detection
- workspace health checks and artifact availability summaries
- improved pipeline phase preview and execution history visibility
- review queue filtering by risk, role, environment, and asset search
- score/confidence sorting in the review queue
- rendered markdown asset cards in asset detail views
- linked Codex insight and Codex review context in asset detail views
- grouped validation warnings/errors and audit timeline views
- smoke tests for command preview logic, redaction, and workspace helpers

## Local Commands

```powershell
reconpilot doctor
reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/
cd gui
npm install
npm test
npm run build
```

The GUI reads local artifacts from `output/`, so running the CLI smoke workflow first gives you a populated workspace for daily use and manual verification.

For a future Tauri runtime session:

```powershell
cd gui
npm run tauri:dev
```

## Files

- `src/`: React + TypeScript frontend
- `src-tauri/`: Tauri Rust shell and allowlist command bridge
- `src/lib/*.test.ts`: frontend smoke tests for hardening-critical helpers

Related docs:

- [../QUICKSTART.md](../QUICKSTART.md)
- [../RELEASE_CHECKLIST.md](../RELEASE_CHECKLIST.md)
- [../KNOWN_LIMITATIONS.md](../KNOWN_LIMITATIONS.md)
