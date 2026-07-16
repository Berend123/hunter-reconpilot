# ReconPilot

ReconPilot is a modern Rust-based recon orchestration platform for operator-led attack surface collection, normalization, enrichment, graph correlation, prioritization, and reporting.

The project is intentionally recon-only in this phase. Its job is to coordinate well-known external recon tools, normalize their outputs into structured data, and prepare a future pipeline where LLMs can help reason about relevance, overlap, and investigation priority.

ReconPilot does not depend on Burp Suite and does not ship exploit tooling.

## Why Recon Is Split Into Stages

ReconPilot treats recon as a pipeline rather than a single scan:

1. Collection
   Raw discovery from subdomain, probing, crawling, historical URL, JavaScript, and content-discovery tools.
2. Normalization
   Cleanup, canonicalization, deduplication, URL shaping, and data model alignment.
3. Enrichment
   Technology tags, host metadata, parameter hints, content type, route families, and contextual labels.
4. Reasoning
   LLM or rule-based interpretation of what the collected data suggests.
5. Prioritization
   Ranking assets and findings so an operator can focus on the most promising paths first.
6. Reporting
   Durable outputs for notes, handoff, evidence, and repeatable workflows.

This separation matters because recon tools are good at finding things, but not always good at explaining which things deserve attention first.

## Why LLM-Assisted Recon Matters

Recon tends to produce large volumes of loosely related evidence:

- assets
- live hosts
- crawled paths
- JavaScript endpoints
- parameters
- headers
- technology hints
- historical artifacts

LLM assistance can help by:

- clustering related endpoints
- identifying suspicious parameter names
- surfacing likely admin, staging, or internal routes
- summarizing JavaScript findings
- explaining why an asset may be high-value
- proposing the next safest recon step

LLMs are not meant to replace the operator. They are intended to compress noisy data into more actionable hypotheses.

## Architecture Overview

```text
Discovery Tools
    ->
Structured JSON / JSONL
    ->
Normalization
    ->
Graph & Correlation
    ->
API & JS Intelligence
    ->
Semantic Enrichment
    ->
Review Workspace
    ->
LLM Context Packs
    ->
Codex Reasoning Plans / Insights
    ->
Validation, Manifest, And Audit
    ->
LLM Reasoning
    ->
Prioritized Findings
    ->
Reports
```

ReconPilot is designed around structured data flow. Each stage should emit machine-readable output that the next stage can consume.

## Quickstart

For a clean first local launch:

```powershell
cargo build
.\scripts\launch-cli.ps1
.\scripts\test-passive.ps1
```

That sequence gives you a built binary, CLI help, a doctor run, a dry-run-safe passive pipeline, validation, and an LLM pack without touching targets.

If you want the step-by-step onboarding version, use [FIRST_RUN_CHECKLIST.md](FIRST_RUN_CHECKLIST.md).

## GUI Launch

After you have local artifacts or after `test-passive.ps1` finishes:

```powershell
.\scripts\launch-gui.ps1
```

The GUI is artifact-first. It is most useful after `output/` contains pipeline, graph, enrichment, review, and validation files.

## First Passive Test

Use this safe first test before any real manual testing:

```powershell
.\scripts\reset-output.ps1
.\scripts\test-passive.ps1
```

Inspect:

- `output/plans/pipeline-plan.md`
- `output/maps/graph.md`
- `output/enrichment/enrichment-summary.md`
- `output/review/priority-queue.md`
- `output/llm-pack/reasoning-queue.md`
- `output/validation-report.md`

## Manual Testing

For practical target selection, evaluation criteria, and a testing notebook template, use:

- [docs/MANUAL_TESTING_GUIDE.md](docs/MANUAL_TESTING_GUIDE.md)
- [docs/DEMO_WORKFLOW.md](docs/DEMO_WORKFLOW.md)
- [FIRST_RUN_CHECKLIST.md](FIRST_RUN_CHECKLIST.md)

Start passive, evaluate the graph and review quality, then move to `active-lite` only on clearly authorized scopes and only after reviewing the plan output.

## Troubleshooting

- If `reconpilot.exe` will not start after a `gnullvm` build, use the provided launch scripts. They add the Rust runtime DLL paths before execution.
- If `cargo build` fails because `link.exe` is missing, use the `gnullvm` fallback described later in this README.
- If `launch-gui.ps1` says dependencies are missing, run `cd gui` and `npm install`.
- If `test-active-lite.ps1` refuses to continue, fix the scope file or install the required tools instead of bypassing the warning.
- If the GUI opens but the viewers are empty, run `.\scripts\test-passive.ps1` first so `output/` contains artifacts.

## Intended Workflow

1. Define allowed scope in `config/scope.txt`.
2. Define exclusions in `config/excluded.txt`.
3. Review or create a runtime config file from `config/reconpilot.example.json`.
4. Run `reconpilot doctor` before the first local MVP workflow.
5. Run `reconpilot plan` to see the intended phases.
6. Run `reconpilot run` to build a safe orchestration plan and prepare output folders.
7. Review the generated adapter plans under `output/plans/`.
8. Only use `reconpilot run --execute` when you explicitly want ReconPilot to launch installed tools.
9. Use the PowerShell helper scripts to manage local workflow on Windows.
10. Run `reconpilot map` to prepare DNS, screenshot, tech, and placeholder app-map inputs.
11. Run `reconpilot graph` to generate graph plans or build local graph artifacts.
12. Run `reconpilot enrich` to deterministically classify graph artifacts into roles, environments, and prioritization candidates.
13. Run `reconpilot api-intel` to analyze local graph, raw, and JavaScript artifacts for API families, schemas, auth indicators, objects, and GraphQL hints.
14. Run `reconpilot enrich --input output/maps/ --api-intel output/api-intel/ --out output/enrichment/` for the preferred API-aware enrichment flow.
15. Run `reconpilot review` to turn enriched artifacts into a ranked analyst review queue and asset cards.
16. Run `reconpilot llm-pack` to build local-only context bundles, prompt templates, and a reasoning queue for analyst-controlled model use later.
17. Run `reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/` to generate optional Codex command plans without executing Codex.
18. Only use `reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/ --execute-codex` when you explicitly want analyst-controlled local Codex reasoning outputs.
19. Run `reconpilot codex-review --input output/codex-insights/ --out output/codex-review/` to annotate Codex reasoning results for evidence gaps, unsupported claims, and unsafe wording without modifying the original outputs.
20. Use `reconpilot pipeline` when you want ReconPilot to orchestrate a named profile in the correct order while keeping external execution gated behind `--execute`.
21. Run `reconpilot validate --input output/` to generate a run manifest, append audit events, and check artifact integrity across the current output tree.
22. Normalize raw URLs and records into stable JSONL.
23. Score findings with rule-based and graph-aware placeholders first.
24. Add LLM-assisted prioritization later after the data model is stable.
25. Review the Phase 13 GUI planning docs before adding a desktop interface.
26. Generate markdown and JSON reports for review.

## Dry-Run First

`reconpilot run` is dry-run by default.

Phase 1 implements safe tool adapters for:

- subfinder
- httpx
- katana
- gau

In dry-run mode ReconPilot:

- validates the scope file
- refuses empty scope
- derives safe input files from validated scope
- checks whether each tool binary is present
- writes planned commands to `output/plans/`
- creates `output/raw/` locations without launching tools

Actual process execution only happens when `--execute` is passed.

## Safe Mapping Layer

Phase 2A adds a separate mapping command:

- `reconpilot map --scope ... --out output/`
- `reconpilot map --scope ... --out output/ --execute`

The mapping layer currently plans or executes:

- dnsx
- gowitness
- WhatWeb

`map` is also dry-run by default.

In Phase 2A ReconPilot:

- validates scope before any mapping command generation
- writes mapping plans to `output/plans/`
- prepares `output/dns/`, `output/screenshots/`, `output/tech/`, and `output/maps/`
- generates a placeholder application map at `output/maps/app-map.json` and `output/maps/app-map.md`
- only executes mapping adapters when `--execute` is explicitly passed

Screenshot tooling deserves extra care: even passive-looking screenshot capture still sends requests to target systems and must stay inside program rules.

## Graph-Centric Correlation

Phase 3 adds a graph and correlation engine:

- `reconpilot graph --input output/ --out output/maps/`
- `reconpilot graph --input output/ --out output/maps/ --execute`

`graph` is dry-run by default.

Dry-run mode:

- validates that required graph input directories exist
- inspects available local artifacts
- writes `output/plans/graph-plan.json`
- writes `output/maps/graph-preview.md`

Execute mode builds local graph artifacts only. It does not launch tools and it does not contact targets.

The current graph layer correlates:

- hosts
- IPs
- URLs
- technologies
- DNS records
- screenshots
- JavaScript references
- parameters
- redirects
- titles

Graphing improves prioritization because it exposes shared infrastructure, repeated admin surfaces, redirect chains, reused technologies, and suspicious environment naming that are easy to miss in flat findings.

## Semantic Enrichment Layer

Phase 4 adds a local-only semantic enrichment command:

- `reconpilot enrich --input output/maps/ --out output/enrichment/`
- `reconpilot enrich --input output/maps/ --api-intel output/api-intel/ --out output/enrichment/`

This phase does not use `--execute` because it does not launch tools or contact targets. It only analyzes existing ReconPilot artifacts on disk.

The semantic layer:

- reads graph outputs from `output/maps/`
- optionally reads API-intel outputs from `output/api-intel/`
- applies deterministic classifiers for environments, roles, technologies, endpoints, and parameters
- summarizes graph neighborhoods such as shared infrastructure, cluster membership, redirects, and technology overlap
- produces risk explanations that describe interesting candidates, potentially sensitive API context, and manual-review priorities without claiming vulnerabilities
- prepares structured overlays for future LLM reasoning

Preferred flow:

```text
graph + api-intel -> enrich -> review
```

If `--api-intel` is omitted, enrichment keeps the earlier graph-only behavior unchanged. If it is provided, ReconPilot validates the folder, loads local API/auth/JS/schema/GraphQL artifacts when present, fails clearly on malformed local API-intel files, and warns when optional API-intel artifacts are missing.

This stage matters because it converts raw graph structure plus deterministic application-capability evidence into prioritization-ready context before any model-assisted reasoning exists.

## API & JavaScript Intelligence Layer

Phase 6A adds a local-only application capability analysis command:

- `reconpilot api-intel --input output/ --out output/api-intel/`

This phase:

- reads local graph, raw crawler, and JavaScript artifacts only
- detects Swagger, OpenAPI, Redoc, GraphQL, and auth-related indicators without contacting targets
- normalizes API endpoints and infers likely API objects such as `User`, `Account`, or `Organization`
- expands the local graph model with API endpoint, schema, object, auth-flow, JS-asset, parameter, and API-family concepts
- reuses deterministic classifier categories so endpoint and auth tagging stays compatible with the semantic enrichment layer
- prepares evidence-rich outputs that the review workspace can consume immediately and future LLM reasoning can consume later

The generated outputs live under `output/api-intel/`:

- `api-endpoints.json`
- `api-objects.json`
- `api-relationships.json`
- `auth-observations.json`
- `js-observations.json`
- `schemas.json`
- `graphql-observations.json`
- `api-graph.json`
- `api-summary.md`

This layer is intentionally cautious. It identifies candidates such as exposed documentation, auth-flow references, GraphQL surfaces, and sensitive object models, but it does not claim vulnerabilities.

## Report And Review Workspace

Phase 5 adds a local-only analyst review workspace:

- `reconpilot review --input output/enrichment/ --out output/review/`

This phase:

- reads semantic enrichment artifacts only
- ranks assets by deterministic score and confidence
- generates analyst-facing asset cards
- builds a review checklist and executive summary
- indexes evidence back to the original local artifacts

The review workspace is intentionally cautious. It highlights candidates, interesting assets, and review targets, but it does not claim vulnerabilities.

When enrichment already includes API-aware evidence, review prefers the enriched asset context directly. It keeps backward-compatible fallback ingestion from `output/api-intel/` only when enrichment was run without `--api-intel`, and it deduplicates overlapping API evidence items.

## Local LLM Reasoning Pack

Phase 7 adds a local-only context pack command:

- `reconpilot llm-pack --input output/ --out output/llm-pack/`

This phase does not execute any model and does not contact targets. It only packages existing ReconPilot artifacts into:

- per-asset context files
- reusable prompt templates
- a ranked reasoning queue
- an analyst brief
- a compact pack summary

The packer loads enrichment and review artifacts as required inputs, then absorbs API-intel and graph summaries when present. It preserves evidence references, avoids duplicate evidence, and applies a configurable character budget so each context stays compact enough for future local model use.

The generated prompts explicitly require:

- prioritization instead of exploitation
- hypotheses instead of confirmed vulnerabilities
- evidence-backed reasoning
- `requires validation` language
- no destructive testing suggestions
- no credential attacks
- no out-of-scope assumptions

## Optional Codex Reasoning Runner

Phase 11 adds an optional local Codex CLI integration:

- `reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/`
- `reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/ --execute-codex`
- `reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/ --include-codex`
- `reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/ --include-codex --execute-codex`

This phase is plan-only by default. It does not contact targets, does not launch recon tools, and does not execute Codex unless `--execute-codex` is passed explicitly.

The runner:

- reads `llm-pack` reasoning queues, prompt templates, and per-asset contexts
- derives sibling review, enrichment, API, and graph summaries when they exist
- builds safety-constrained `codex exec "<prompt>"` command plans
- redacts likely secrets or tokens before prompt construction
- enforces the existing `max_context_chars` budget from `llm-pack`
- writes analyst-controlled reasoning outputs only when execution is explicitly requested

When Codex is included inside `reconpilot pipeline`, it is still explicit-only:

- `--include-codex` adds `codex-run` after `llm-pack`
- without `--execute-codex`, the pipeline creates Codex plans only
- `--execute` never implies `--execute-codex`
- `--execute-codex` is ignored unless `--include-codex` is also set

Codex output is treated as reasoning support, not validation. Results must still use cautious language and require human review.

## Codex Insight Review

Phase 12 adds a local annotation layer for Codex reasoning outputs:

- `reconpilot codex-review --input output/codex-insights/ --out output/codex-review/`

This phase:

- loads `codex-summary.json`, result sidecars, and result markdown
- builds a consolidated local review queue for Codex outputs
- flags unsupported or overconfident claim wording
- flags missing evidence references
- flags unsafe recommendations such as destructive testing, credential attacks, auth bypass attempts, or out-of-scope assumptions
- writes review annotations without deleting or rewriting the original Codex outputs

Codex review is still local-only. It produces analyst review artifacts, not validation of findings.

## Desktop GUI

Phase 14 adds the first full desktop GUI under [gui/README.md](gui/README.md).

GUI architecture:

- Tauri desktop shell
- React + TypeScript frontend
- Rust backend commands
- existing ReconPilot CLI remains the source of truth
- GUI reads artifacts from `output/`
- GUI may trigger existing CLI flows only with explicit confirmation
- no direct target contact from GUI code

Implemented screens:

1. Dashboard
2. Project / Workspace Selector
3. Scope Manager
4. Pipeline Runner
5. Profile Editor
6. Tool Settings
7. Review Queue
8. Asset Detail
9. Graph Viewer
10. API Intelligence
11. Enrichment Viewer
12. LLM Pack Viewer
13. Codex Runner
14. Codex Insights
15. Codex Review
16. Validation / Audit
17. Settings

Modes:

- Beginner Mode: dry-run first, stronger warnings, no custom profiles, no custom tool args
- Advanced Mode: still dry-run first, but allows GUI-defined profiles, controlled custom args, rate placeholders, and fewer repeated prompts after acknowledgement

Hard blocks remain in both modes:

- no arbitrary shell commands
- no hidden execution
- no run without scope where scope is required
- no forbidden Codex bypass flags
- no accidental Codex execution
- no unsupported binaries outside the ReconPilot command allowlist

## Run Manifest, Audit Log, And Validation

Phase 8 adds a local-only reproducibility and quality-control command:

- `reconpilot validate --input output/`

This layer:

- writes `output/run-manifest.json` with the command, timestamp, available hashes, generated plans, artifact counts, warnings, and errors
- appends `output/audit-log.jsonl` events such as `phase_started`, `phase_completed`, `artifact_written`, `warning`, `error`, `skipped_optional_input`, and `dry_run_plan_created`
- generates `output/validation-report.md` and `output/validation-report.json`
- checks JSON and JSONL parseability plus graph, enrichment, review, and LLM-pack reference integrity

Warnings do not always fail validation, but malformed required artifacts and broken required references do.

## Config Profiles And Pipeline Runner

Phase 9 adds named orchestration profiles:

- `passive`
- `active-lite`
- `api-focused`
- `mapping-focused`
- `review-only`
- `llm-pack-only`

The pipeline command is:

- `reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/`
- `reconpilot pipeline --scope config/scope.example.txt --profile active-lite --out output/ --execute`
- `reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/ --include-codex`
- `reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/ --include-codex --execute-codex`

Pipeline behavior is intentionally safety-oriented:

- a phase plan is written to `output/plans/pipeline-plan.json` and `output/plans/pipeline-plan.md`
- external-tool phases such as `run` and `map` stay dry-run unless `--execute` is passed
- local-only phases such as `graph`, `api-intel`, `enrich`, `review`, `llm-pack`, `codex-run`, and `validate` may still run because they only analyze local artifacts
- `--include-codex` appends `codex-run` after `llm-pack`
- `--execute` never implies `--execute-codex`
- `--execute-codex` only matters when `--include-codex` is present
- scope is validated before any phase that could touch targets
- missing prerequisites can cause later phases to skip with warnings, and final validation records what is incomplete

This gives you a reproducible profile runner without changing the dry-run default for target-touching work.

## Setup Doctor

Phase 10 adds a local MVP doctor command:

- `reconpilot doctor`

The doctor checks:

- Rust and local runtime tool visibility
- OS and build context
- config examples and config validation
- sample scope and exclusion consistency
- output folder writability
- required MVP docs
- external tool availability across core and optional registry entries

Optional tools generate warnings rather than failures. The doctor command is local-only and does not contact targets.

## Adapter Architecture

Phase 1 adapters are intentionally narrow and safety-oriented.

Each adapter:

- validates scope before command generation
- constructs arguments explicitly instead of using shell concatenation
- records binary availability
- writes a human-readable command file and JSON metadata to `output/plans/`
- writes captured stdout and stderr to `output/raw/<tool>/` when execution is enabled

The current adapters are designed to make planning and auditing obvious before ReconPilot grows into a fuller orchestration engine.

Phase 2A mapping adapters add:

- scope-aware DNS resolution planning with `dnsx`
- screenshot planning against existing `httpx` live-host output with `gowitness`
- low-aggression technology fingerprint planning with `WhatWeb`
- placeholder app-map generation that combines available assets, DNS data, screenshots, and tech fingerprints

## Supported Tool Categories

### Core discovery

- subfinder
- amass
- assetfinder
- findomain

### Live host probing

- httpx

### Mapping and enrichment

- dnsx
- gowitness
- WhatWeb

### Crawling and URL collection

- katana
- gau
- waybackurls
- hakrawler

### Normalization and filtering

- uro
- jq
- gf

### Content discovery

- ffuf
- feroxbuster
- dirsearch

### JavaScript recon

- LinkFinder
- SecretFinder
- JSParser

### Port and service discovery when explicitly allowed

- naabu
- nmap

See [TOOLS.md](TOOLS.md) for per-tool details.

## Why Exploitation Tooling Is Excluded

This phase explicitly excludes:

- Burp Suite integration
- sqlmap
- dalfox
- xsstrike
- nuclei
- metasploit
- brute-force authentication tooling
- credential attacks
- destructive scanners

ReconPilot is meant to improve collection quality, structure, and prioritization before any exploitation work is considered. That separation keeps scope tighter, makes audits easier, and reduces the risk of accidentally overstepping engagement boundaries.

## Why Scope Enforcement Matters

Recon automation without strict scope enforcement quickly becomes unsafe.

ReconPilot treats scope as a first-class input:

- commands should not run without a scope file
- empty scope files are rejected
- exclusions should be enforced before tool execution
- port and service discovery should require explicit authorization
- high-volume or active phases should be easy to disable

See [SAFETY_AND_SCOPE.md](SAFETY_AND_SCOPE.md) for the operating model.

## JSONL Pipeline Design

JSONL is preferred for the internal data pipeline because it is:

- stream-friendly
- append-friendly
- easy to inspect with `jq`
- easy to merge across tools
- easy to feed into scoring and LLM stages

Example flow:

```text
subfinder/amass -> assets.jsonl
httpx -> live-hosts.jsonl
dnsx -> dnsx.jsonl
gowitness -> screenshots + gowitness.jsonl
WhatWeb -> whatweb.json
katana/gau/waybackurls -> urls.jsonl
LinkFinder/JSParser -> js-findings.jsonl
normalizer -> normalized-findings.jsonl
graph -> graph.json
api-intel -> api-graph.json + schemas/auth/js observations
enrich -> enriched-graph.json + semantic overlays
review -> priority-queue.json + evidence-index.json
llm-pack -> asset-contexts + reasoning-queue.json + prompts
validate -> run-manifest.json + audit-log.jsonl + validation-report.json
scorer -> scored-findings.jsonl
reporter -> report.md + report.json
```

## Setup

### Windows prerequisites

- Rust and Cargo
- Go
- Python 3
- Git
- Chocolatey or Scoop for optional package installation paths

Use:

```powershell
pwsh -File .\install\check-prereqs.ps1
```

### Tool installation

Review tool sources:

```powershell
Get-Content .\install\tool-sources.json
```

Prepare installation folders and inspect planned steps:

```powershell
pwsh -File .\install\install-tools-windows.ps1
```

Execute the installation flow only after review:

```powershell
pwsh -File .\install\install-tools-windows.ps1 -Execute
```

## Build Instructions

```powershell
cargo build
```

For a release build:

```powershell
cargo build --release
```

### Windows fallback without Visual Studio Build Tools

If `cargo build` fails because `link.exe` is missing, use the `gnullvm` toolchain and Rust's bundled linker:

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnullvm --profile minimal
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNULLVM_LINKER = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-gnullvm\lib\rustlib\x86_64-pc-windows-gnullvm\bin\rust-lld.exe"
cargo +stable-x86_64-pc-windows-gnullvm build
```

For direct execution of the built binary in that setup, make sure the toolchain runtime path is present:

```powershell
$env:PATH = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-gnullvm\bin;$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-gnullvm\lib\rustlib\x86_64-pc-windows-gnullvm\bin;$env:PATH"
.\target\debug\reconpilot.exe init
```

## Example Commands

```powershell
reconpilot --version
reconpilot doctor
reconpilot init
reconpilot check-tools
reconpilot plan --scope config/scope.example.txt
reconpilot run --scope config/scope.example.txt --out output/
reconpilot run --scope config/scope.example.txt --out output/ --execute
reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/
reconpilot pipeline --scope config/scope.example.txt --profile active-lite --out output/ --execute
reconpilot map --scope config/scope.example.txt --out output/
reconpilot map --scope config/scope.example.txt --out output/ --execute
reconpilot graph --input output/ --out output/maps/
reconpilot graph --input output/ --out output/maps/ --execute
reconpilot enrich --input output/maps/ --out output/enrichment/
reconpilot enrich --input output/maps/ --api-intel output/api-intel/ --out output/enrichment/
reconpilot api-intel --input output/ --out output/api-intel/
reconpilot review --input output/enrichment/ --out output/review/
reconpilot llm-pack --input output/ --out output/llm-pack/
reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/
reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/ --execute-codex
reconpilot validate --input output/
reconpilot normalize --input output/urls/raw.txt
reconpilot score --input output/findings/raw.jsonl
reconpilot report --input output/findings/scored.jsonl
```

Phase 1 run outputs:

- `output/plans/`: planned commands, adapter metadata, and scope-derived input files
- `output/raw/`: raw tool outputs plus captured stdout and stderr when execution is enabled

Phase 2A mapping outputs:

- `output/dns/`: planned or captured DNS relationship data
- `output/screenshots/`: planned or captured screenshot artifacts
- `output/tech/`: planned or captured technology fingerprint data
- `output/maps/`: placeholder app map artifacts

Phase 3 graph outputs:

- `output/maps/graph-preview.md`: dry-run graph input preview
- `output/maps/graph.json`: graph nodes, edges, clusters, and summary
- `output/maps/graph.md`: readable graph summary
- `output/maps/clusters.json`: cluster export
- `output/maps/clusters.md`: readable cluster summary
- `output/maps/anomalies.json`: heuristic anomaly candidates
- `output/maps/graph-summary.json`: compact graph metrics

Phase 6A API and JavaScript intelligence outputs:

- `output/api-intel/api-endpoints.json`: normalized endpoint records and semantic endpoint tags
- `output/api-intel/api-objects.json`: inferred object models and sensitivity hints
- `output/api-intel/api-relationships.json`: deterministic API relationships
- `output/api-intel/auth-observations.json`: local auth-style observations
- `output/api-intel/js-observations.json`: JavaScript-derived routes, roles, and feature flags
- `output/api-intel/schemas.json`: parsed schema and documentation artifacts
- `output/api-intel/graphql-observations.json`: GraphQL indicators from local artifacts
- `output/api-intel/api-graph.json`: API-aware graph nodes and edges
- `output/api-intel/api-summary.md`: analyst-readable application capability summary

Phase 4 semantic enrichment outputs:

- `output/enrichment/semantic-assets.json`: enriched asset records with semantic tags, roles, environments, and neighborhood summaries
- `output/enrichment/semantic-observations.json`: deterministic semantic observations
- `output/enrichment/risk-explanations.json`: prioritization-oriented risk explanations
- `output/enrichment/enriched-graph.json`: original graph plus semantic overlays
- `output/enrichment/enrichment-summary.md`: analyst-readable summary and next-step guidance

Phase 5 review workspace outputs:

- `output/review/priority-queue.md`: ranked analyst queue
- `output/review/priority-queue.json`: structured ranked queue
- `output/review/asset-cards/`: one markdown card per asset
- `output/review/review-checklist.md`: safe manual review checklist
- `output/review/executive-summary.md`: non-technical summary
- `output/review/evidence-index.json`: evidence-to-review-item mapping

Phase 7 local LLM pack outputs:

- `output/llm-pack/asset-contexts/`: compact per-asset JSON context bundles
- `output/llm-pack/prompts/`: reusable prompt templates for triage, API, auth, JS, and reporting
- `output/llm-pack/reasoning-queue.json`: structured LLM reasoning queue
- `output/llm-pack/reasoning-queue.md`: readable LLM reasoning queue
- `output/llm-pack/analyst-brief.md`: analyst-facing summary for model-assisted review planning
- `output/llm-pack/pack-summary.json`: compact manifest with prompt metadata, queue items, and safety-ready summary details

Phase 11 optional Codex reasoning outputs:

- `output/codex-insights/plans/codex-command-plan.json`: structured `codex exec` plan
- `output/codex-insights/plans/codex-command-plan.md`: readable command plan and safety notes
- `output/codex-insights/results/`: per-asset markdown outputs and JSON sidecars
- `output/codex-insights/logs/codex-stdout.log`: aggregated stdout log
- `output/codex-insights/logs/codex-stderr.log`: aggregated stderr log
- `output/codex-insights/codex-summary.md`: readable Codex runner summary
- `output/codex-insights/codex-summary.json`: machine-readable Codex runner summary

Phase 12 Codex review outputs:

- `output/codex-review/codex-review-queue.md`: readable ranked Codex output review queue
- `output/codex-review/codex-review-queue.json`: structured Codex output review queue
- `output/codex-review/unsupported-claims.json`: flagged overconfident or unsupported claims
- `output/codex-review/evidence-gaps.json`: missing or incomplete evidence-reference annotations
- `output/codex-review/wording-warnings.json`: unsafe recommendation or wording warnings
- `output/codex-review/codex-review-summary.md`: analyst-facing summary of Codex output quality checks

Phase 8 reproducibility and validation outputs:

- `output/run-manifest.json`: current command manifest with paths, plans, hashes when available, warnings, and errors
- `output/audit-log.jsonl`: append-only local audit events across major phases
- `output/validation-report.md`: readable validation summary
- `output/validation-report.json`: machine-readable validation report

Phase 9 pipeline outputs:

- `output/plans/pipeline-plan.json`: structured per-profile phase plan
- `output/plans/pipeline-plan.md`: readable execution order with external-tool and local-analysis labels

Phase 13 GUI planning artifacts:

- `docs/GUI_ARCHITECTURE.md`: CLI-first desktop architecture plan
- `docs/GUI_UX_SPEC.md`: screen-level UX and wireframes
- `docs/GUI_DATA_CONTRACTS.md`: artifact contract expectations for the GUI
- `docs/GUI_SECURITY_MODEL.md`: GUI safety and execution constraints
- `docs/GUI_ROADMAP.md`: staged GUI delivery plan with Phase 14 scaffold placeholder

Phase 14 GUI implementation artifacts:

- `gui/`: full Tauri + React + TypeScript desktop GUI
- `gui/README.md`: GUI build and mode overview
- `docs/GUI_PHASE_14_FULL.md`: implementation summary
- `docs/GUI_ADVANCED_MODE.md`: advanced mode guardrails and hard blocks

Windows helper scripts:

```powershell
pwsh -File .\scripts\launch-cli.ps1
pwsh -File .\scripts\launch-gui.ps1
pwsh -File .\scripts\test-passive.ps1
pwsh -File .\scripts\test-active-lite.ps1 -ScopePath .\config\scope.txt
pwsh -File .\scripts\reset-output.ps1
pwsh -File .\scripts\run-recon.ps1 -ScopePath .\config\scope.example.txt -OutDir .\output
pwsh -File .\scripts\normalize-urls.ps1 -InputFile .\output\urls\raw.txt -OutputFile .\output\urls\normalized.txt
pwsh -File .\scripts\score-findings.ps1 -InputFile .\output\findings\raw.jsonl -OutputFile .\output\findings\scored.jsonl
```

## Repository Layout

```text
reconpilot/
  config/
  data/
  examples/
  install/
  notes/
  output/
  scripts/
  src/
```

Runtime output layout:

```text
output/
  plans/
  raw/
  dns/
  screenshots/
  tech/
  maps/
  assets/
  urls/
  js/
  params/
  findings/
  enrichment/
  review/
  llm-pack/
  codex-insights/
  codex-review/
  run-manifest.json
  audit-log.jsonl
  validation-report.md
  validation-report.json
  reports/
```

Pipeline planning artifacts:

```text
output/plans/
  pipeline-plan.json
  pipeline-plan.md
```

Graph-specific map artifacts:

```text
output/maps/
  app-map.json
  app-map.md
  graph-preview.md
  graph.json
  graph.md
  clusters.json
  clusters.md
  anomalies.json
  graph-summary.json
```

Key documentation:

- [TOOLS.md](TOOLS.md)
- [ORCHESTRATION.md](ORCHESTRATION.md)
- [SAFETY_AND_SCOPE.md](SAFETY_AND_SCOPE.md)
- [QUICKSTART.md](QUICKSTART.md)
- [FIRST_RUN_CHECKLIST.md](FIRST_RUN_CHECKLIST.md)
- [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md)
- [KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md)
- [docs/MANUAL_TESTING_GUIDE.md](docs/MANUAL_TESTING_GUIDE.md)
- [docs/DEMO_WORKFLOW.md](docs/DEMO_WORKFLOW.md)
- [docs/MVP_USAGE.md](docs/MVP_USAGE.md)
- [docs/PIPELINE_PROFILES.md](docs/PIPELINE_PROFILES.md)
- [docs/OUTPUTS.md](docs/OUTPUTS.md)
- [gui/README.md](gui/README.md)
- [docs/GUI_ARCHITECTURE.md](docs/GUI_ARCHITECTURE.md)
- [docs/GUI_UX_SPEC.md](docs/GUI_UX_SPEC.md)
- [docs/GUI_DATA_CONTRACTS.md](docs/GUI_DATA_CONTRACTS.md)
- [docs/GUI_SECURITY_MODEL.md](docs/GUI_SECURITY_MODEL.md)
- [docs/GUI_ROADMAP.md](docs/GUI_ROADMAP.md)
- [docs/GUI_PHASE_14_FULL.md](docs/GUI_PHASE_14_FULL.md)
- [docs/GUI_ADVANCED_MODE.md](docs/GUI_ADVANCED_MODE.md)

## Future Roadmap Ideas

- JSONL-native tool adapters for each recon source
- concurrent job scheduling with rate and scope guards
- result caching to avoid repeated collection
- URL and endpoint family clustering
- asset graph and trust-boundary modeling
- graph-native anomaly detection and cluster scoring
- deterministic semantic enrichment over graph neighborhoods
- analyst review workspaces and evidence-led queueing
- local LLM integration for private environments
- LLM reasoning over graph neighborhoods and infrastructure clusters
- vector embeddings for semantically similar routes and findings
- screenshot and favicon enrichment
- differential report generation between runs
- richer policy-driven execution profiles with config-driven gating and reuse
- desktop GUI scaffold with Tauri + React + TypeScript
- artifact-first GUI viewers for graph, review, validation, and Codex outputs

## Release Candidate Notes

ReconPilot `0.1.0` is prepared as a local release candidate, not a general-purpose scanner distribution.

- keep `output/` artifacts local and disposable
- treat review, llm-pack, Codex, and graph outputs as prioritization support rather than proof
- use [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) before tagging
- review [KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md) before depending on the current MVP for a broader workflow
