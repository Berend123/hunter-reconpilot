# GUI Advanced Mode

## Goal

Document what Advanced Mode enables in the Phase 14 GUI and what it still does not allow.

Advanced Mode exists to reduce friction for legitimate, authorized bug bounty recon workflows without removing core safety controls.

## What Advanced Mode Enables

- custom GUI-defined profiles
- controlled custom tool args stored in GUI config
- editable rate limit placeholders
- editable concurrency placeholders
- fewer repeated prompts after acknowledgement
- saved acknowledgements for target contact and Codex execution context

## What Advanced Mode Does Not Remove

- dry-run default
- mandatory scope awareness
- exact command preview
- audit logging
- GUI execution log writing
- separate Codex execution confirmation model
- validation visibility
- redaction controls

## Hard Blocks That Still Apply

Even in Advanced Mode, the GUI must not allow:

- arbitrary shell commands
- hidden execution
- running without a scope file where scope is required
- `codex --yolo`
- `codex --dangerously-bypass-approvals-and-sandbox`
- accidental Codex execution
- unsupported binaries outside the ReconPilot allowlist
- exploit tooling or mutation-driven behavior

## Config Model

Advanced Mode stores GUI-specific preferences in:

- `config/reconpilot.gui.json`

This is separate from the core CLI config so GUI additions do not silently alter existing ReconPilot CLI behavior.

Planned GUI-managed values include:

- mode
- acknowledgements
- custom profiles
- custom tool args
- rate placeholders
- concurrency placeholders

## Risk Warnings

The GUI should warn when Advanced Mode config appears risky, for example:

- unusually high request rates
- unusually high screenshot concurrency
- tool args that reference forbidden Codex flags
- tool args that reference unsupported tooling

Warnings do not automatically rewrite the config. They make the risk visible before execution.

## Recommended Operator Use

- use Beginner Mode first when onboarding a new workspace
- switch to Advanced Mode only when you understand the pipeline and scope boundaries
- keep command preview visible before every target-touching action
- treat saved custom args as an explicit operator responsibility
