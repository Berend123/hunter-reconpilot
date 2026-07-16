# GUI Architecture

## Goal

Phase 13 defines a future desktop GUI architecture for ReconPilot without changing current CLI behavior.

The GUI is intended to be:

- local-first
- safety-oriented
- artifact-driven
- CLI-compatible
- suitable for analyst review, not autonomous execution

This phase is planning only. No GUI code, frontend dependencies, or Tauri scaffold is added yet.

## Core Principles

1. The existing ReconPilot CLI remains the source of truth.
2. The GUI reads and presents artifacts already written under `output/`.
3. The GUI may invoke existing CLI commands only through explicit, operator-confirmed actions.
4. No GUI component should contact targets directly.
5. Dry-run remains the default posture for anything that could touch targets.
6. Codex execution remains an explicit opt-in workflow, never a background behavior.

## Planned Stack

### Desktop shell

- Tauri
- native desktop packaging for Windows first
- Rust-side command handlers for filesystem access and controlled command invocation

### Frontend

- React
- TypeScript
- artifact-centric state management
- local rendering of JSON, JSONL, markdown, and graph-oriented data

### Backend integration

- Tauri commands wrap existing ReconPilot CLI-compatible behaviors
- prefer thin Rust command handlers rather than duplicating orchestration logic in TypeScript
- filesystem reads come from known project directories and generated output artifacts

## Architectural Layers

```text
Tauri Shell
    ->
Rust GUI Backend Commands
    ->
ReconPilot CLI / Output Files
    ->
React + TypeScript Views
```

### Layer responsibilities

#### 1. Tauri shell

- application lifecycle
- local filesystem capability boundary
- window management
- secure bridging between frontend and Rust backend

#### 2. Rust GUI backend commands

- enumerate workspaces
- read known artifact files
- validate file existence and parseability
- launch existing ReconPilot CLI commands with explicit user confirmation
- surface status, warnings, and errors to the UI

#### 3. ReconPilot CLI

- remains the authoritative execution engine
- continues to own scope validation, dry-run defaults, audit logging, manifest generation, and pipeline behavior
- remains usable without the GUI

#### 4. React + TypeScript frontend

- render workspace state
- visualize phase results
- show review queues, graph relationships, API observations, and Codex reasoning summaries
- provide confirmation flows before any command execution

## Workspace Model

A workspace is the local project root containing:

- `config/`
- `output/`
- docs and supporting files

The GUI should support:

- opening an existing ReconPilot workspace
- remembering recent workspaces locally
- read-only artifact browsing before any execution permissions are granted

## Planned Command Boundary

The GUI should not reimplement orchestration logic. Instead it should call stable command surfaces such as:

- `reconpilot doctor`
- `reconpilot pipeline ...`
- `reconpilot validate --input output/`
- `reconpilot codex-run ...`
- `reconpilot codex-review ...`

Each invocation should:

- display the exact command to be run
- require explicit confirmation
- show whether the command is local-only or target-touching
- show whether it is dry-run or execution mode

## Data Flow

### Primary read path

```text
output/ artifacts
    ->
Rust backend file readers
    ->
normalized view models
    ->
React screens
```

### Controlled execution path

```text
GUI action
    ->
explicit confirmation dialog
    ->
Rust backend command wrapper
    ->
ReconPilot CLI
    ->
updated output/ artifacts
    ->
UI refresh
```

## Planned GUI Modules

### Workspace module

- workspace selection
- recent projects
- path validation

### Artifact loader module

- load JSON/JSONL/markdown from known output locations
- normalize parse errors into UI-safe messages

### Command runner module

- construct existing CLI invocations
- enforce confirmation prompts
- surface stdout/stderr and manifest/audit updates

### Status module

- current phase state
- dry-run vs executed status
- warnings and validation failures

### Review module

- render prioritized analyst queues
- render evidence references
- cross-link Codex review findings and validation warnings

## Planned State Boundaries

The frontend should keep state separated into:

- workspace selection state
- artifact loading state
- phase status state
- active filters and search state
- pending command confirmation state
- safety warning state

It should not keep its own hidden execution state that differs from the manifest or audit log.

## Planned Rust Backend Responsibilities

The future Tauri backend should expose minimal commands such as:

- `list_workspaces`
- `open_workspace`
- `read_output_summary`
- `read_json_artifact`
- `read_markdown_artifact`
- `run_cli_command_with_confirmation`
- `tail_audit_log`

These are design placeholders only for Phase 13.

## Constraints

- no direct HTTP requests to targets from frontend code
- no embedded recon logic in the browser layer
- no hidden execution flags
- no automatic Codex execution
- no bypass of CLI safety gates

## Why CLI-First Matters

Keeping the CLI as source of truth preserves:

- parity between terminal and GUI workflows
- stable testing and automation surfaces
- a single place for scope validation and safety enforcement
- easier audit and manifest generation

## Phase 14 Placeholder

The next implementation phase is:

- Tauri + React + TypeScript scaffold
- read-only workspace shell first
- artifact viewers before execution controls
- explicit command confirmation flows after read-only browsing works reliably
