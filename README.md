# Hunter / ReconPilot

Hunter and ReconPilot are independent security-research tooling prototypes by Hendrik Fuchs. The work explores how an operator can collect, structure, correlate, and review authorized security observations without turning the workflow into an uncontrolled scanner.

## At A Glance

| Question | Answer |
| --- | --- |
| What problem does it solve? | Security research produces fragmented observations. This project turns them into bounded, structured evidence an analyst can review and validate. |
| Who is it for? | Software engineers and security researchers working on systems they own or are explicitly authorized to assess. |
| What is the philosophy? | Reduce noise, preserve evidence, keep execution explicit, and leave conclusions with the analyst. |
| How are humans kept in control? | Scope is declared first, target contact is opt-in, AI execution is separately controlled, and generated conclusions require validation. |
| What is public today? | The active ReconPilot implementation and the design documentation for the companion Hunter workflow. |

## Architecture

```text
Authorized scope and exclusions
    |
    +-- Hunter workflow design
    |     Burp observations -> mapping -> prioritization -> diffs/flows -> casefile
    |
    +-- ReconPilot implementation
          collection plan -> normalization -> enrichment -> graph/API/JS correlation
              -> review queue -> optional bounded AI reasoning -> validation/report/audit
    |
    +-- Analyst review and responsible reporting
```

Hunter and ReconPilot are complementary rather than dependent. Hunter explores the manual Burp-side investigation workflow. ReconPilot is a standalone implementation for recon orchestration, structured analysis, and review artifacts.

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

The control model also includes dry-run planning, explicit scope and exclusion files, redaction support, machine-readable manifests, and append-only audit events. These controls make activity and evidence reviewable; they do not replace authorization or analyst judgment.

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
