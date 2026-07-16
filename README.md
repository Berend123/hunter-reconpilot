# Hunter / ReconPilot

Hunter and ReconPilot are independent security-research tooling prototypes by Hendrik Fuchs. The work explores how an operator can collect, structure, correlate, and review authorized security observations without turning the workflow into an uncontrolled scanner.

The repository is deliberately operator-led:

- scope and exclusions are explicit
- target-touching execution is opt-in
- dry-run planning is the default
- evidence remains structured and reviewable
- AI reasoning is optional and separate from tool execution
- model output is treated as a hypothesis, not vulnerability confirmation

## Repository Status

ReconPilot is the active implementation included in this public repository. It contains a Rust CLI, a Tauri desktop interface, structured artifact pipelines, tests, and safety controls.

Hunter is the companion Burp Suite workflow concept. Its design notes are included here, while the incomplete Java module implementation remains outside this public snapshot. This distinction is intentional: the public repository should not imply that an unreleased aggregate extension is ready to build or use.

Both projects are active prototypes. They are research tooling, not a vulnerability scanner, exploit framework, or substitute for analyst validation.

## ReconPilot

ReconPilot is a Rust-based orchestration and analysis workspace for authorized attack-surface research. It separates the workflow into explicit stages:

1. collection planning
2. normalization and deduplication
3. enrichment and classification
4. graph and evidence correlation
5. API and JavaScript intelligence
6. analyst review and prioritization
7. optional LLM context packaging and reasoning
8. validation, reporting, manifests, and audit output

The desktop interface is built with Tauri and TypeScript. The CLI and artifact pipeline remain the primary implementation boundary.

See [ReconPilot documentation](reconpilot/README.md) for architecture, setup, commands, and current limitations.

## Hunter

Hunter explores a manual-first Burp Suite workflow built around explicit handoffs between scope, mapping, traffic prioritization, diffs, flows, analysis, recon imports, and casefile evidence.

The current public material documents the intended workflow and safety model:

- [Hunter Extensions plan](HUNTER_EXTENSIONS_PLAN.md)
- [Beginner workflow](A%20realistic%20beginner%20workflow.md)
- [Proxy and Burp setup note](EDGE_FOXYPROXY_BURP_NOTE.md)

## Safety And Authorization

Use this repository only on systems you own or where you have explicit authorization to test.

ReconPilot keeps external-tool execution behind an explicit `--execute` control. Optional Codex execution uses a separate `--execute-codex` control; enabling one does not enable the other. Generated reasoning still requires manual validation.

Read [Safety and Scope](reconpilot/SAFETY_AND_SCOPE.md) before using the project against any target.

## Quick Start

From the `reconpilot` directory:

```powershell
cargo build
.\scripts\launch-cli.ps1
.\scripts\test-passive.ps1
```

The passive test prepares local artifacts without touching a target. Review the generated plan and validation output before enabling any execution mode.

## Technology

- Rust and Tokio for orchestration and artifact processing
- Tauri, TypeScript, and React for the desktop workspace
- JSON and JSONL for durable, machine-readable evidence
- Java design work for the companion Burp Suite workflow
- optional Codex integration for bounded, analyst-controlled reasoning

## License

MIT. See [LICENSE](LICENSE).

