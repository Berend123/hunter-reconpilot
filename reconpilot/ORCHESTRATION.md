# ReconPilot Orchestration Architecture

ReconPilot is intended to orchestrate existing recon tools into a structured, safety-oriented data pipeline. The core design goal is to move from loosely related command output toward reusable, explainable, and machine-readable recon evidence.

## Dry-Run First Execution Model

Phase 1 uses a dry-run-first adapter model.

- `reconpilot run` validates scope and generates command plans only.
- `reconpilot map` validates scope and generates mapping plans only.
- `reconpilot graph` validates local graph inputs and generates graph plans only.
- `reconpilot enrich` validates local graph artifacts and performs local enrichment only.
- `reconpilot api-intel` validates local output folders and performs API and JavaScript analysis only.
- `reconpilot review` validates local enrichment artifacts and generates review outputs only.
- `reconpilot llm-pack` validates local review and enrichment artifacts and generates prompt/context bundles only.
- `reconpilot codex-run` validates local llm-pack artifacts and generates Codex command plans only.
- `reconpilot codex-review` validates local codex-insights artifacts and annotates reasoning quality only.
- `reconpilot validate` validates local output integrity and writes reports only.
- `reconpilot pipeline` orchestrates named profiles, keeps external phases dry-run unless `--execute` is passed, and still allows local-only phases to run against existing artifacts.
- `reconpilot doctor` validates local MVP readiness without contacting targets.
- `reconpilot run --execute` is required before any external tool process is started.
- `reconpilot map --execute` is required before any mapping adapter is started.
- `reconpilot graph --execute` is required before graph artifacts are generated.
- every adapter writes its planned command and metadata to `output/plans/`
- every executed adapter writes raw artifacts and stdout/stderr captures to `output/raw/`
- graph execution is local-only and transforms existing artifacts into relationship-aware outputs
- enrichment execution is always local-only and never contacts targets
- API intelligence execution is always local-only and never contacts targets
- LLM pack execution is always local-only and never executes a model or contacts targets
- Codex runner execution is local-only and only invokes `codex exec` when `--execute-codex` is explicitly passed
- validation execution is always local-only and never contacts targets or executes models

This keeps orchestration auditable and prevents accidental scans caused by a default-on execution path.

## Inputs

ReconPilot expects three primary operator-controlled inputs:

### `scope.txt`

The allowlist of domains, hosts, or URLs that define what can be touched.

### `excluded.txt`

Explicitly forbidden hosts, domains, routes, or identifiers that must be excluded from every phase.

### `reconpilot.json`

Runtime configuration for:

- enabled tool groups
- concurrency caps
- rate limits
- output formatting
- named profile defaults
- passive-only or active-light modes
- max LLM context packing size
- safety mode labeling
- optional authorization gates for port scanning
- future scoring heuristics and model settings

## Pipeline

### 1. Scope validation

Before any tool planning:

- load `scope.txt`
- reject empty scope
- load exclusions
- reject invalid domains or malformed URLs
- build a normalized scope model
- confirm whether active phases are permitted

### 2. Subdomain discovery

Expected tools:

- subfinder
- amass
- assetfinder
- findomain

Goal:

- collect candidate assets from multiple viewpoints
- preserve source attribution
- emit `assets/raw-*.jsonl`

### 3. Live host probing

Expected tool:

- httpx

Goal:

- validate which assets are live
- capture status, title, tech hints, TLS details, redirect behavior
- emit `assets/live-hosts.jsonl`

### 4. Crawling

Expected tools:

- katana
- hakrawler

Goal:

- discover linked routes, forms, and reachable application surfaces
- emit `urls/crawled.jsonl`

### 5. Historical URLs

Expected tools:

- gau
- waybackurls

Goal:

- recover historical endpoints and archived application surface
- emit `urls/historical.jsonl`

### 6. JavaScript extraction

Expected tools:

- LinkFinder
- SecretFinder
- JSParser

Goal:

- extract endpoints, domains, parameters, tokens, and code hints from JavaScript
- emit `js/endpoints.jsonl` and `js/secrets-candidates.jsonl`

### 7. Parameter extraction

Goal:

- identify parameter names from crawled URLs, JS references, and historical URLs
- cluster repeated patterns
- label parameters that suggest auth, admin, debugging, uploads, redirects, or internal routing
- emit `params/params.jsonl`

### 8. Content discovery

Expected tools:

- ffuf
- feroxbuster
- dirsearch

Goal:

- expand the known route set using active but scoped discovery
- emit `urls/content-discovery.jsonl`

### 9. Normalization

Expected utilities:

- uro
- jq
- gf
- native Rust normalizers

Goal:

- deduplicate URLs
- canonicalize hosts and schemes
- collapse obvious duplicates
- retain source traceability
- emit `findings/normalized.jsonl`

### 10. Enrichment

Goal:

- attach technologies
- infer route families
- infer likely asset roles
- mark login, admin, static, API, upload, internal, or staging patterns
- preserve why a label was assigned

### 10A. Mapping layer

Expected tools:

- dnsx
- gowitness
- WhatWeb

Goal:

- resolve host-to-DNS relationships
- capture visual application clues through screenshots
- fingerprint technologies conservatively
- build a placeholder app map from available artifacts
- emit `maps/app-map.json` and `maps/app-map.md`

### 10B. Graph and correlation layer

Goal:

- ingest existing structured recon artifacts
- build graph nodes and graph edges
- infer relationship clusters and anomaly candidates
- preserve confidence and evidence for each relationship
- prepare the model for future local LLM reasoning

Inputs:

- `output/raw/`
- `output/dns/`
- `output/tech/`
- `output/screenshots/`
- `output/maps/app-map.json`
- example artifacts for parser fallback and tests

Outputs:

- `maps/graph.json`
- `maps/graph.md`
- `maps/clusters.json`
- `maps/clusters.md`
- `maps/anomalies.json`
- `maps/graph-summary.json`

Current relationship coverage:

- `ResolvesTo`
- `RedirectsTo`
- `UsesTechnology`
- `SharesIp`
- `SharesTitle`
- `References`
- `LoadsScript`
- `ContainsParameter`
- `Hosts`
- `BelongsToCluster`

### 10C. Semantic enrichment layer

Goal:

- convert graph structure into deterministic semantic tags
- optionally merge deterministic API-intel evidence before review
- infer environments and asset roles
- generate graph-neighborhood observations
- create cautious risk explanations for prioritization
- prepare future LLM-ready context without using any model yet

Inputs:

- `maps/graph.json`
- `maps/clusters.json`
- `maps/anomalies.json`
- `maps/graph-summary.json`
- optional: `output/api-intel/`

Outputs:

- `enrichment/semantic-assets.json`
- `enrichment/semantic-observations.json`
- `enrichment/risk-explanations.json`
- `enrichment/enriched-graph.json`
- `enrichment/enrichment-summary.md`

Current classifier categories:

- environment classification
- role classification
- technology classification
- endpoint intent classification
- parameter intent classification

Phase 6A reuses these deterministic categories so API endpoints, schema exposure hints, auth-related surfaces, and JavaScript-derived routes stay aligned with the same semantic vocabulary before any future LLM layer is introduced.

Preferred flow:

```text
graph + api-intel -> enrich -> review
```

Command behavior:

- `reconpilot enrich --input output/maps/ --out output/enrichment/`
- `reconpilot enrich --input output/maps/ --api-intel output/api-intel/ --out output/enrichment/`

If `--api-intel` is present:

- the folder must exist and be a directory
- present API-intel JSON artifacts must parse cleanly
- missing optional API-intel artifacts are skipped with warnings
- enrichment stays local-only and does not contact targets

If `--api-intel` is absent:

- enrichment keeps the earlier graph-only behavior unchanged

Important constraint:

- semantic enrichment does not claim vulnerabilities
- it produces interesting candidates, supporting evidence, and manual next-step suggestions

### 10D. API and JavaScript intelligence layer

Goal:

- analyze local graph, crawler, and JavaScript artifacts for application capability signals
- detect Swagger, OpenAPI, Redoc, GraphQL, auth-related terminology, and object references
- expand the local graph with API endpoint, object, schema, auth-flow, JS-asset, parameter, token, and API-family concepts
- preserve deterministic evidence chains for later enrichment, review, and future LLM reasoning

Inputs:

- `output/raw/`
- `output/maps/`
- `output/js/` when present

Outputs:

- `api-intel/api-endpoints.json`
- `api-intel/api-objects.json`
- `api-intel/api-relationships.json`
- `api-intel/auth-observations.json`
- `api-intel/js-observations.json`
- `api-intel/schemas.json`
- `api-intel/graphql-observations.json`
- `api-intel/api-graph.json`
- `api-intel/api-summary.md`

API graph relationship coverage:

- `RequiresAuth`
- `ReturnsObject`
- `ReferencesParameter`
- `BelongsToApi`
- `UsesToken`
- `ReferencesSchema`
- `LoadsEndpoint`
- `RelatedToAuthFlow`

Important constraints:

- no remote schema fetches are performed
- no GraphQL queries are executed
- no auth tokens are exercised
- outputs describe candidate relationships and surfaces that require manual validation

### 10E. Review workspace layer

Goal:

- turn enriched graph artifacts into an analyst-facing review queue
- rank assets by deterministic score and confidence
- generate readable asset cards
- map every evidence item back to its source artifact
- provide a cautious checklist and executive summary for manual review
- absorb API intelligence artifacts when available so auth flows, schema exposure, GraphQL indicators, JS-derived routes, and sensitive object models can influence ranking and suggested review steps
- prefer API-aware evidence already embedded in enrichment outputs and fall back to sibling `output/api-intel/` artifacts only when enrichment was run without `--api-intel`

Inputs:

- `enrichment/semantic-assets.json`
- `enrichment/semantic-observations.json`
- `enrichment/risk-explanations.json`
- `enrichment/enriched-graph.json`
- `enrichment/enrichment-summary.md`

Outputs:

- `review/priority-queue.md`
- `review/priority-queue.json`
- `review/asset-cards/`
- `review/review-checklist.md`
- `review/executive-summary.md`
- `review/evidence-index.json`

Important constraint:

- review outputs prioritize manual investigation but do not claim confirmed issues

### 10F. Local LLM reasoning pack layer

Goal:

- transform enrichment and review artifacts into compact, evidence-backed context bundles for later analyst-controlled model use
- generate reusable prompt templates
- create a ranked reasoning queue
- enforce token-budget style truncation without discarding evidence references
- keep future model use downstream of deterministic local analysis

Inputs:

- `review/priority-queue.json`
- `review/evidence-index.json`
- `enrichment/semantic-assets.json`
- `enrichment/semantic-observations.json`
- `enrichment/risk-explanations.json`
- `enrichment/enriched-graph.json`
- optional: `api-intel/`
- optional: `maps/graph-summary.json`
- optional: `maps/clusters.json`

Outputs:

- `llm-pack/asset-contexts/`
- `llm-pack/prompts/`
- `llm-pack/reasoning-queue.json`
- `llm-pack/reasoning-queue.md`
- `llm-pack/analyst-brief.md`
- `llm-pack/pack-summary.json`
- `codex-insights/plans/codex-command-plan.json`
- `codex-insights/codex-summary.json`

Command behavior:

- `reconpilot llm-pack --input output/ --out output/llm-pack/`
- `--max-context-chars` defaults to `12000`
- enrichment and review inputs are required
- API-intel and map summaries are optional
- duplicate evidence is removed before prompt packs are written
- contexts are truncated safely while preserving evidence IDs and references

Important constraints:

- no remote API calls
- no automatic LLM execution
- no vulnerability claims
- no destructive testing suggestions
- no credential attacks
- prompts must request hypotheses, prioritization, and requires validation language

### 10G. Run manifest, audit, and validation layer

Goal:

- make ReconPilot runs easier to reproduce and audit
- record commands, artifacts, and available hashes
- validate cross-phase references before later reasoning or reporting

Outputs:

- `run-manifest.json`
- `audit-log.jsonl`
- `validation-report.md`
- `validation-report.json`

Validation coverage:

- expected output folders
- required core artifacts
- JSON parse checks
- JSONL line-by-line parse checks
- graph edge to node integrity
- enrichment asset reference integrity
- review evidence reference integrity
- LLM-pack context and prompt reference integrity
- duplicate asset IDs
- empty high-value output warnings

Important constraints:

- validation is local-only
- warnings do not always fail the run
- invalid required artifacts fail validation

### 10H. Optional Codex reasoning runner

Goal:

- turn `llm-pack` queue items into analyst-controlled Codex CLI runs
- keep model invocation explicit and auditable
- preserve evidence references, prompt safety rules, and token-budget controls
- generate reasoning support without claiming vulnerabilities or contacting targets

Command behavior:

- `reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/`
- `reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/ --execute-codex`
- `reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/ --limit 5`
- `reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/ --template asset_triage_prompt`

Inputs:

- `llm-pack/reasoning-queue.json`
- `llm-pack/pack-summary.json`
- `llm-pack/asset-contexts/`
- `llm-pack/prompts/`
- optional sibling `review/`, `enrichment/`, `api-intel/`, and `maps/` summaries

Outputs:

- `codex-insights/plans/codex-command-plan.json`
- `codex-insights/plans/codex-command-plan.md`
- `codex-insights/results/*.md`
- `codex-insights/results/*.json`
- `codex-insights/logs/codex-stdout.log`
- `codex-insights/logs/codex-stderr.log`
- `codex-insights/codex-summary.md`
- `codex-insights/codex-summary.json`

Important constraints:

- plan mode is the default
- `--execute-codex` is mandatory before any Codex CLI call occurs
- only `codex exec "<prompt>"` is used
- no `--yolo` or sandbox-bypass flags are ever used
- prompts must require evidence-backed reasoning, `requires validation` language, and no destructive or credential-attack suggestions
- likely secrets are redacted before prompt construction
- Codex outputs remain analyst guidance, not validation

### 10I. Config profiles and pipeline runner

Goal:

- orchestrate existing ReconPilot phases in a reproducible order
- preserve dry-run defaults for external-tool phases
- let local-only phases continue when they can operate on existing artifacts
- record phase results in the manifest and audit log

Command behavior:

- `reconpilot pipeline --scope config/scope.txt --profile passive --out output/`
- `reconpilot pipeline --scope config/scope.txt --profile active-lite --out output/ --execute`
- `reconpilot pipeline --scope config/scope.txt --profile passive --out output/ --include-codex`
- `reconpilot pipeline --scope config/scope.txt --profile passive --out output/ --include-codex --execute-codex`

Current profiles:

- `passive`: `run` dry-run planning, `graph`, `api-intel`, `enrich`, `review`, `llm-pack`, `validate`
- `active-lite`: `run`, `map`, `graph`, `api-intel`, `enrich --api-intel`, `review`, `llm-pack`, `validate`
- `api-focused`: `api-intel`, `enrich --api-intel`, `review`, `llm-pack`, `validate`
- `mapping-focused`: `map`, `graph`, `enrich`, `review`, `validate`
- `review-only`: `enrich`, `review`, `validate`
- `llm-pack-only`: `llm-pack`, `validate`

Pipeline outputs:

- `plans/pipeline-plan.json`
- `plans/pipeline-plan.md`

Pipeline behavior:

- each plan marks phases as `external-tool-phase` or `local-analysis-phase`
- `run` and `map` only launch installed tooling when `--execute` is explicitly provided
- `graph`, `api-intel`, `enrich`, `review`, `llm-pack`, and `validate` may still run without `--execute`
- `--include-codex` inserts `codex-run` immediately after `llm-pack` when that phase exists in the selected profile
- without `--execute-codex`, the inserted `codex-run` phase remains plan-only
- `--execute` never implies `--execute-codex`
- `--execute-codex` is ignored unless `--include-codex` is also set
- missing prerequisites can cause dependent phases to skip with warnings
- final manifest records the selected profile and per-phase statuses

### 10J. MVP hardening and release prep

Goal:

- polish the CLI for first local use
- validate config and sample inputs earlier
- provide a local environment doctor
- ship a minimal release-documentation set for operators

Key additions:

- `reconpilot doctor`
- config validation for profile names, output paths, safety mode, and context bounds
- safer sample scope and exclusion examples
- release docs and quickstart references

### 10K. Codex pipeline integration and insight review

Goal:

- integrate optional Codex reasoning into named pipeline profiles without weakening dry-run defaults
- add a local review layer that annotates Codex outputs for evidence quality and safety wording
- preserve original Codex outputs while generating analyst-facing review artifacts

Commands:

- `reconpilot codex-review --input output/codex-insights/ --out output/codex-review/`
- `reconpilot pipeline --scope config/scope.txt --profile passive --out output/ --include-codex`
- `reconpilot pipeline --scope config/scope.txt --profile passive --out output/ --include-codex --execute-codex`

Inputs:

- `codex-insights/codex-summary.json`
- `codex-insights/results/*.json`
- `codex-insights/results/*.md`

Outputs:

- `codex-review/codex-review-queue.md`
- `codex-review/codex-review-queue.json`
- `codex-review/unsupported-claims.json`
- `codex-review/evidence-gaps.json`
- `codex-review/wording-warnings.json`
- `codex-review/codex-review-summary.md`

Detection heuristics:

- flag overconfident wording such as confirmed or definitely vulnerable claims
- flag missing evidence references or incomplete evidence citation coverage
- flag recommendations that imply destructive testing, credential attacks, auth bypass attempts, exploit tooling, or out-of-scope assumptions
- require `requires validation` style language before reasoning is treated as cautious

Important constraints:

- Codex execution remains opt-in and separate from pipeline `--execute`
- codex-review never deletes or rewrites Codex outputs
- Codex results are hypotheses only and require analyst validation
- validation now checks both `codex-insights/` and `codex-review/` integrity when those artifacts exist

### 13. GUI architecture and UX planning

Goal:

- define a future desktop GUI without changing current CLI behavior
- keep the CLI as the source of truth
- make the GUI artifact-driven, local-first, and safety-oriented

Planned stack:

- Tauri desktop shell
- React + TypeScript frontend
- Rust backend commands

Planned principles:

- GUI reads artifacts from `output/`
- GUI may trigger existing CLI commands only with explicit confirmation
- no direct target contact from GUI code
- no hidden execution state outside manifest and audit artifacts
- no automatic Codex execution

Planned GUI screens:

1. Dashboard
2. Project / Workspace selector
3. Scope Manager
4. Pipeline Runner
5. Asset Graph Viewer
6. API Intelligence Viewer
7. Semantic Enrichment Viewer
8. Review Queue
9. LLM Pack Viewer
10. Codex Insights
11. Codex Review
12. Validation / Audit
13. Settings

Planned key GUI data sources:

- `maps/graph.json`
- `maps/clusters.json`
- `enrichment/semantic-assets.json`
- `review/priority-queue.json`
- `api-intel/*.json` and `api-summary.md`
- `llm-pack/reasoning-queue.json`
- `codex-insights/codex-summary.json`
- `codex-review/codex-review-queue.json`
- `validation-report.json`
- `run-manifest.json`
- `audit-log.jsonl`

Phase 13 adds planning documentation only:

- `docs/GUI_ARCHITECTURE.md`
- `docs/GUI_UX_SPEC.md`
- `docs/GUI_DATA_CONTRACTS.md`
- `docs/GUI_SECURITY_MODEL.md`
- `docs/GUI_ROADMAP.md`

### 14. GUI scaffold placeholder

Goal:

- begin the first implementation pass for the desktop GUI after the planning docs are reviewed

Expected stack:

- Tauri + React + TypeScript

First implementation constraints:

- read-only workspace shell first
- no recon behavior changes
- no new scanning behavior
- no direct target contact
- no hidden execution controls

### 11. LLM scoring

Goal:

- let a future local or remote model score interestingness and operator value
- generate short reasoning notes
- emit `findings/scored.jsonl`

### 12. Reports

Goal:

- produce markdown, JSON, and later HTML summaries
- focus on prioritized assets and findings
- preserve safe next-step suggestions
- emit `reports/report.md` and `reports/report.json`

## Structured JSONL Flow

ReconPilot should favor JSONL between phases:

```text
assets/raw.jsonl
  -> assets/live-hosts.jsonl
  -> urls/crawled.jsonl
  -> js/endpoints.jsonl
  -> params/params.jsonl
  -> maps/app-map.json
  -> maps/graph.json
  -> api-intel/api-graph.json
  -> enrichment/enriched-graph.json
  -> run-manifest.json + audit-log.jsonl
  -> llm-pack/reasoning-queue.json
  -> findings/normalized.jsonl
  -> findings/scored.jsonl
  -> reports/report.json
```

Why JSONL:

- it scales for streaming
- it avoids loading everything into memory at once
- it works well with `jq`
- it is simple to merge and diff
- it supports append-only workflows

## Tool Chaining

Example future chain:

```text
subfinder + assetfinder + amass + findomain
  -> merged assets list
  -> httpx
  -> katana + hakrawler
  -> gau + waybackurls
  -> LinkFinder + JSParser + SecretFinder
  -> uro + jq + gf + Rust normalizers
  -> scoring engine
  -> report builder
```

Each adapter should record:

- tool name
- version if known
- timestamp
- source input
- normalized output schema version

## Phase 1 Adapter Architecture

The current adapter layer is intentionally small:

- subfinder
- httpx
- katana
- gau

Each adapter is responsible for:

- validating non-empty scope before command generation
- deriving tool-specific input files from validated scope
- checking whether the tool binary is available on `PATH`
- constructing the command as an explicit program-plus-arguments vector
- writing a command preview and JSON run metadata to `output/plans/`
- executing only when `--execute` is present
- capturing stdout and stderr into `output/raw/<tool>/`

Phase 1 does not try to fully chain tool output into downstream adapters yet. The goal is to prove safe command generation, file layout, binary detection, and controlled execution first.

## Phase 2A Mapping Adapters

Phase 2A adds a separate mapping layer on top of the existing collection workflow:

- `dnsx` consumes discovered subdomains or scope-derived domains
- `gowitness` consumes `httpx` live-host output
- `WhatWeb` consumes `httpx` live-host output

Mapping plans are written to:

- `output/plans/dnsx-plan.json`
- `output/plans/gowitness-plan.json`
- `output/plans/whatweb-plan.json`

The current mapping design deliberately stays conservative:

- `dnsx` uses low-rate placeholder settings
- `WhatWeb` is planned with low aggression and low thread counts
- `gowitness` is treated as safety-sensitive because screenshots still touch target systems

If `httpx` live-host data is missing, `gowitness` and `WhatWeb` plans are still generated, but execution is withheld until the expected input exists.

## Phase 3 Graph And Correlation Engine

Phase 3 keeps the same dry-run-first model:

- `reconpilot graph --input output/ --out output/maps/`
- `reconpilot graph --input output/ --out output/maps/ --execute`

Dry-run mode writes:

- `output/plans/graph-plan.json`
- `output/maps/graph-preview.md`

Execute mode writes local graph artifacts only. It does not launch tools and it does not contact targets.

The graph layer improves prioritization by preserving relationship evidence around:

- shared infrastructure
- repeated titles and app families
- redirect neighborhoods
- technology reuse
- likely staging or internal systems
- admin-like clusters

## Phase 4 Semantic Enrichment Engine

Phase 4 stays entirely local:

- `reconpilot enrich --input output/maps/ --out output/enrichment/`

This stage does not require `--execute` because it does not launch tools, scan targets, or call remote APIs.

The semantic layer uses deterministic reasoning over graph outputs to produce:

- semantic tags
- inferred environments
- inferred roles
- graph-neighborhood observations
- cautious risk explanations
- prioritization-ready summaries

Examples of current semantic inferences:

- `staging` plus `auth` implies a likely staging authentication surface
- shared infrastructure with an admin-like or operational asset implies privileged adjacency
- `Grafana`, `Kibana`, `Prometheus`, or `Jenkins` imply operational tooling candidates
- `/swagger` and `/openapi` imply documentation candidates
- `/export` and `/backup` imply sensitive operation candidates
- redirect-like or token-like parameters increase review interest without asserting a flaw

## Phase 5 Report And Review Workspace

Phase 5 also stays entirely local:

- `reconpilot review --input output/enrichment/ --out output/review/`

This stage converts enrichment artifacts into:

- ranked review items
- asset cards
- evidence indexes
- safe checklists
- executive summaries

Current priority logic boosts:

- higher base risk scores
- multiple semantic roles
- authentication, admin, internal, staging, and legacy indicators
- graph-neighborhood context such as shared infrastructure or cluster membership

The review layer is intentionally analyst-facing. It explains why an asset is worth review and suggests cautious next steps without asserting vulnerability.

When sibling `output/api-intel/` artifacts exist, the review layer also absorbs:

- API endpoint candidates
- auth observations
- schema and documentation exposure hints
- GraphQL indicators
- JavaScript-derived routes
- inferred API object sensitivity

## Phase 6A API And JavaScript Intelligence Foundation

Phase 6A stays entirely local:

- `reconpilot api-intel --input output/ --out output/api-intel/`

This stage does not require `--execute` because it only analyzes ReconPilot artifacts already on disk.

The API intelligence layer produces:

- endpoint normalization and API family grouping
- auth observation candidates
- schema and documentation parsing
- GraphQL indicators
- JavaScript-derived route and feature-flag observations
- inferred object models such as `User`, `Account`, or `Organization`
- an API-aware graph that extends infrastructure context with application capability context

It is intentionally deterministic and evidence-driven. It does not send requests, call remote models, or label anything as a confirmed issue.

## Phase 6B API Intelligence Enrichment Feedback Loop

Phase 6B makes API intelligence a first-class enrichment input instead of only a late review-side add-on.

Preferred flow:

```text
graph + api-intel -> enrich -> review
```

This update means:

- `enrich` can optionally ingest `output/api-intel/`
- API, auth, schema, GraphQL, JS, and object-model evidence influence enriched assets before review ranking
- review prefers API-aware enrichment outputs and only falls back to direct sibling `output/api-intel/` artifacts when enrichment did not embed that evidence
- overlapping API evidence is deduplicated before analyst-facing review artifacts are written

The feedback loop stays local-only and deterministic. It strengthens prioritization language such as candidate, interesting, worth review, potentially sensitive, and requires validation without asserting that an issue exists.

## Enrichment Phases

Enrichment is where raw records become useful records.

Potential enrichment dimensions:

- route type: `api`, `auth`, `upload`, `admin`, `debug`, `static`
- trust boundary: `public`, `partner`, `internal-looking`, `unknown`
- environment hint: `prod`, `staging`, `dev`, `preview`
- content type and size bands
- JavaScript-originated or archive-originated evidence
- interesting parameter categories

## Risk Scoring Concepts

Risk scoring in ReconPilot should remain explainable.

Examples:

- admin-like path names
- login and password reset flows
- file upload routes
- exposed API docs
- debug or health endpoints
- internal hostnames
- JavaScript references to hidden APIs
- parameter names such as `redirect`, `token`, `returnUrl`, `key`, `debug`

Scores should be composed from:

- deterministic heuristics
- tool confidence
- source overlap
- future LLM reasoning

## Prioritization Logic

Prioritization is not the same as vulnerability confirmation.

A prioritized finding should answer:

- Why is this asset worth looking at first?
- What evidence supports that judgment?
- Which tool sources saw it?
- What is the least risky next recon step?

Future prioritization should also consider:

- endpoint uniqueness
- technology rarity inside the environment
- overlap between historical and live exposure
- suspicious parameter combinations
- internal naming conventions

## Anomaly Detection Ideas

Future anomaly detection could flag:

- route families that only appear on one host
- staging-like domains mixed into production namespace
- JavaScript endpoints that have no obvious linked navigation path
- content types that differ from neighboring routes
- one-off parameters with privileged naming
- service banners or technologies that do not match the rest of the estate

The current graph heuristics already emit local anomaly candidates for:

- suspicious hostnames such as `admin`, `internal`, `dev`, `staging`, `test`, and `legacy`
- unusual ports outside conservative web defaults
- exposed dashboards such as Grafana, Jenkins, Kibana, and Prometheus
- legacy technology indicators
- multiple admin-like systems sharing infrastructure

## Future Vector Database Ideas

Longer term, ReconPilot can embed:

- route paths
- titles
- JavaScript findings
- parameter sets
- operator notes

Potential use cases:

- finding semantically similar endpoints
- clustering login or admin flows
- de-duplicating near-duplicate findings
- retrieving similar historical patterns across engagements

## Future Local LLM Integration Ideas

Local LLM integration should be favored where privacy matters.

Candidate future capabilities:

- use `llm-pack/asset-contexts/` as the standard local prompt input layer
- use `llm-pack/analyst-brief.md` and `llm-pack/reasoning-queue.json` for controlled analyst-in-the-loop reasoning
- use `run-manifest.json` and `audit-log.jsonl` as reproducibility context for future local model runs
- summarize large URL sets into route families
- infer likely app functions from JS endpoints
- explain why a cluster may indicate high-value workflow surface
- use deterministic semantic overlays as prompt context for later model reasoning
- use review queue artifacts and evidence indexes as model-ready analyst context
- reason over graph neighborhoods instead of isolated URLs
- propose safe next steps such as passive validation or targeted crawling
- generate short analyst-ready evidence summaries

Guardrails should include:

- no automatic exploitation
- no external upload by default
- explicit operator control over model invocation
- auditable prompts and outputs

## Expected Output Layout

```text
output/
  plans/
  raw/
  dns/
  screenshots/
  tech/
  maps/
  enrichment/
  review/
  assets/
  urls/
  js/
  params/
  screenshots/
  findings/
  reports/
```

Suggested content:

- `plans/`: planned commands, scope-derived input files, and adapter metadata
- `plans/pipeline-plan.json`: structured phase ordering, dry-run state, and expected outputs
- `plans/pipeline-plan.md`: readable pipeline profile plan
- `raw/`: raw tool output files plus stdout/stderr captures
- `dns/`: DNS resolution output and relationship artifacts
- `screenshots/`: screenshots and screenshot metadata
- `tech/`: technology fingerprinting output
- `maps/`: placeholder and future graph-oriented app maps
- `maps/graph.json`: graph export with nodes, edges, clusters, and summary
- `maps/clusters.json`: cluster export for shared infrastructure and app families
- `maps/anomalies.json`: local heuristic anomaly candidates
- `maps/graph-summary.json`: compact graph metrics and prioritization hints
- `enrichment/`: deterministic semantic overlays, observations, risk explanations, and summaries
- `review/`: ranked review queues, asset cards, evidence indexes, and executive summaries
- `llm-pack/`: compact local prompt/context bundles prepared for analyst-controlled model use
- `codex-insights/`: optional Codex command plans, reasoning outputs, logs, and summaries
- `assets/`: candidate assets, live hosts, and host metadata
- `urls/`: crawled, historical, and normalized URLs
- `js/`: JavaScript-derived routes and secret candidates
- `params/`: extracted parameter records and labels
- `screenshots/`: future visual capture outputs
- `findings/`: normalized and scored finding records
- `reports/`: human-readable and machine-readable reports
