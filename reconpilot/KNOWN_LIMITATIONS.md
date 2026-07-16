# Known Limitations

ReconPilot v0.1.0 is a local recon orchestration MVP. These limitations are expected in the release candidate.

## Scope Of The MVP

- ReconPilot is recon-only. It does not confirm vulnerabilities.
- Prioritization outputs are triage artifacts, not security findings.
- Codex outputs are hypothesis-only and always require human validation.
- External recon tools must be installed separately when you want real tool execution.

## Execution Model

- External-tool phases stay dry-run by default and require explicit `--execute`.
- Codex execution is always separate and requires explicit `--execute-codex`.
- The GUI only launches allowlisted `reconpilot` commands. It does not run arbitrary shell commands.
- Passive pipeline runs are useful immediately, but active-lite workflows still depend on local tool availability and scope authorization.

## Data And Visualization

- The graph visualizer is currently table and list based rather than a full interactive canvas.
- Many enrichment and scoring decisions are deterministic heuristics rather than deeper contextual reasoning.
- Optional artifact quality depends on the completeness of earlier local outputs.
- API and JavaScript intelligence are analysis-only in this release and do not validate behavior against live targets.

## Packaging And Environment

- The current release candidate is optimized for local operator use, not multi-user deployment.
- Some developer environments may still depend on an installed Rust toolchain until packaged binaries are produced.
- Windows setup quality still depends on local PATH hygiene for optional external tools.

## Review Expectations

- Evidence gaps can still exist in sparse datasets.
- Missing optional inputs may reduce ranking quality without breaking the core workflow.
- Review, LLM-pack, and Codex layers help focus analyst attention, but they do not replace manual judgment.
