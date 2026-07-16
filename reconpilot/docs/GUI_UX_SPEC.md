# GUI UX Spec

## Goal

Define the analyst-facing UX for a future ReconPilot desktop GUI.

The GUI should help an operator:

- understand workspace state quickly
- inspect artifact quality
- review prioritized assets and evidence
- see dry-run versus executed status clearly
- launch existing CLI flows only with explicit confirmation

## Global UX Rules

- show workspace path prominently
- show current profile and phase status when available
- show whether a run is dry-run or executed
- show scope file hash when available
- surface warnings before convenience actions
- show validation failures before Codex reasoning actions
- show evidence references alongside analyst conclusions
- never label hypotheses as confirmed vulnerabilities
- include `requires validation` language wherever prioritization or Codex reasoning is shown

## Navigation Model

Planned main navigation:

1. Dashboard
2. Project / Workspace Selector
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

## Screen Specifications

### 1. Dashboard

#### Purpose

Provide a high-level snapshot of workspace health, most recent pipeline activity, validation state, and top analyst review targets.

#### Data sources

- `output/run-manifest.json`
- `output/validation-report.json`
- `output/review/priority-queue.json`
- `output/codex-review/codex-review-queue.json`
- `output/audit-log.jsonl`

#### User actions

- open latest validation report
- jump to review queue
- jump to Codex review
- open pipeline runner
- refresh workspace state

#### Safety constraints

- no implicit command execution
- warnings shown before any execution-oriented navigation
- validation errors highlighted before Codex actions

#### Empty state

- explain that no workspace outputs exist yet
- link to doctor, pipeline planning, and documentation

#### Error state

- show which artifact failed to load
- allow retry
- keep unaffected widgets available

#### Future enhancements

- run history timeline
- side-by-side comparison with previous manifests
- saved analyst notes

### 2. Project / Workspace Selector

#### Purpose

Open an existing ReconPilot workspace and verify expected directories exist.

#### Data sources

- filesystem path selection
- project root structure

#### User actions

- select workspace
- reopen recent workspace
- validate workspace structure

#### Safety constraints

- read-only by default
- no automatic project migration

#### Empty state

- no recent workspaces yet

#### Error state

- invalid path
- not a ReconPilot workspace

#### Future enhancements

- workspace templates
- recent workspace metadata

### 3. Scope Manager

#### Purpose

Display scope and exclusion files, hashes, and validation state before any target-touching phase is considered.

#### Data sources

- `config/scope.txt` or selected scope file
- `config/excluded.txt`
- `output/run-manifest.json`

#### User actions

- view scope contents
- compare scope and exclusions
- verify hash and last-used command context

#### Safety constraints

- no inline execution from scope edits in the first GUI release
- emphasize that empty or inconsistent scope blocks target-touching phases

#### Empty state

- no scope loaded

#### Error state

- missing scope file
- inconsistent exclusions

#### Future enhancements

- diff view between scope revisions
- hash history from manifests

### 4. Pipeline Runner

#### Purpose

Present named pipeline profiles, phase order, dry-run/execution posture, and command confirmation workflows.

#### Data sources

- `config/reconpilot.json`
- `output/plans/pipeline-plan.json`
- `output/run-manifest.json`
- `output/audit-log.jsonl`

#### User actions

- choose profile
- toggle `include-codex`
- inspect generated phase plan
- confirm a pipeline command
- monitor status after launch

#### Safety constraints

- `--execute` and `--execute-codex` shown as separate, explicit switches
- `--execute` never preselects Codex execution
- show exact command before confirmation
- show phase classification as local-only or target-touching

#### Empty state

- no pipeline plan yet

#### Error state

- invalid profile
- missing scope
- command launch failure

#### Future enhancements

- queued runs
- saved safe presets
- manifest diff after completion

### 5. Asset Graph Viewer

#### Purpose

Visualize graph nodes, edges, clusters, anomalies, and related evidence across hosts, URLs, DNS, technologies, and API objects later.

#### Data sources

- `output/maps/graph.json`
- `output/maps/clusters.json`
- `output/maps/anomalies.json`
- `output/maps/graph-summary.json`

#### User actions

- inspect node details
- filter by relationship type
- highlight clusters
- jump from node to review item or enrichment detail

#### Safety constraints

- visualization only
- no graph-derived execution shortcuts

#### Empty state

- graph not generated yet

#### Error state

- malformed graph or missing node references

#### Future enhancements

- large-graph clustering controls
- saved filtered views
- relationship timelines

### 6. API Intelligence Viewer

#### Purpose

Show API endpoints, objects, auth observations, schemas, GraphQL hints, and JS-discovered routes.

#### Data sources

- `output/api-intel/api-endpoints.json`
- `output/api-intel/api-objects.json`
- `output/api-intel/api-relationships.json`
- `output/api-intel/auth-observations.json`
- `output/api-intel/js-observations.json`
- `output/api-intel/schemas.json`
- `output/api-intel/graphql-observations.json`
- `output/api-intel/api-summary.md`

#### User actions

- filter endpoints by auth relevance
- inspect sensitive object candidates
- open schema summaries
- jump to related enriched assets

#### Safety constraints

- analysis-only display
- no API requests or schema fetches from the UI

#### Empty state

- API intelligence not generated yet

#### Error state

- malformed API artifact
- missing required summary files

#### Future enhancements

- route family heat maps
- object relationship graph overlays

### 7. Semantic Enrichment Viewer

#### Purpose

Display deterministic roles, environments, semantic tags, observations, and risk explanations.

#### Data sources

- `output/enrichment/semantic-assets.json`
- `output/enrichment/semantic-observations.json`
- `output/enrichment/risk-explanations.json`
- `output/enrichment/enriched-graph.json`
- `output/enrichment/enrichment-summary.md`

#### User actions

- inspect enriched assets
- filter by role or environment
- compare graph-driven and API-driven evidence

#### Safety constraints

- emphasize that outputs are prioritization hints, not proof

#### Empty state

- enrichment not generated yet

#### Error state

- missing graph input lineage
- malformed enrichment artifact

#### Future enhancements

- side-by-side semantic diff across runs

### 8. Review Queue

#### Purpose

Present the main analyst-facing prioritized queue with evidence and suggested manual review steps.

#### Data sources

- `output/review/priority-queue.json`
- `output/review/priority-queue.md`
- `output/review/evidence-index.json`
- `output/review/asset-cards/`

#### User actions

- sort and filter queue
- open asset cards
- inspect evidence references
- export notes later

#### Safety constraints

- reinforce cautious language
- show validation status alongside items

#### Empty state

- no review queue yet

#### Error state

- missing evidence references
- malformed priority queue

#### Future enhancements

- analyst annotations
- review completion state

### 9. LLM Pack Viewer

#### Purpose

Show what context will be handed to future local reasoning tools.

#### Data sources

- `output/llm-pack/reasoning-queue.json`
- `output/llm-pack/reasoning-queue.md`
- `output/llm-pack/pack-summary.json`
- `output/llm-pack/asset-contexts/`
- `output/llm-pack/prompts/`

#### User actions

- inspect selected prompt template
- inspect per-asset context bundle
- verify truncation and evidence preservation

#### Safety constraints

- viewing only
- no model execution from this screen in the first GUI release

#### Empty state

- llm-pack not generated yet

#### Error state

- missing context file
- malformed queue

#### Future enhancements

- side-by-side prompt preview
- token budget visualizer

### 10. Codex Insights

#### Purpose

Show optional Codex reasoning plans and generated results.

#### Data sources

- `output/codex-insights/plans/codex-command-plan.json`
- `output/codex-insights/plans/codex-command-plan.md`
- `output/codex-insights/codex-summary.json`
- `output/codex-insights/codex-summary.md`
- `output/codex-insights/results/*.md`
- `output/codex-insights/results/*.json`

#### User actions

- inspect command plans
- inspect result markdown
- inspect sidecar metadata
- compare plan-only versus executed items

#### Safety constraints

- Codex execution must not happen from passive viewing
- execution control must require explicit confirmation and only appear in a dedicated action flow

#### Empty state

- no Codex plan or result artifacts

#### Error state

- missing sidecar
- unsafe command flag detected by validation

#### Future enhancements

- result diffing across prompt templates

### 11. Codex Review

#### Purpose

Annotate Codex results for unsupported claims, evidence gaps, and unsafe wording.

#### Data sources

- `output/codex-review/codex-review-queue.json`
- `output/codex-review/codex-review-queue.md`
- `output/codex-review/unsupported-claims.json`
- `output/codex-review/evidence-gaps.json`
- `output/codex-review/wording-warnings.json`
- `output/codex-review/codex-review-summary.md`

#### User actions

- inspect flagged outputs
- jump from review queue item to original Codex result
- verify missing evidence references

#### Safety constraints

- never rewrite the original result from the GUI
- emphasize that Codex output is hypothesis-only

#### Empty state

- codex-review not generated yet

#### Error state

- queue references missing source result
- malformed annotation files

#### Future enhancements

- analyst acknowledgement state for each warning

### 12. Validation / Audit

#### Purpose

Central place for reproducibility, data quality, warnings, and error history.

#### Data sources

- `output/validation-report.json`
- `output/validation-report.md`
- `output/run-manifest.json`
- `output/audit-log.jsonl`

#### User actions

- inspect validation failures
- inspect audit sequence
- jump from validation issue to referenced artifact

#### Safety constraints

- highlight validation failures before reasoning-oriented actions
- block convenience execution actions later if required artifacts are invalid

#### Empty state

- validation has not run yet

#### Error state

- malformed report
- missing manifest

#### Future enhancements

- run-to-run validation diff
- filter by warning/error/event type

### 13. Settings

#### Purpose

Manage GUI-local preferences without overriding CLI safety behavior silently.

#### Data sources

- future GUI-local settings file
- `config/reconpilot.json` for display and validation

#### User actions

- choose theme later
- set default workspace
- choose default read-only landing screen
- configure max rendered rows per table

#### Safety constraints

- GUI settings must not silently weaken CLI safety gates
- execution preferences must remain explicit per action

#### Empty state

- default preferences

#### Error state

- invalid settings file

#### Future enhancements

- per-user review preferences
- saved filters

## ASCII Wireframes

### Dashboard

```text
+----------------------------------------------------------------------------------+
| ReconPilot | Workspace: C:\...\reconpilot | Scope Hash: 8d1f... | DRY-RUN FIRST |
+----------------------------------------------------------------------------------+
| Pipeline Status         | Validation                        | Top Review Targets |
| passive                 | 2 warnings, 0 errors              | 1. auth.example.com|
| Last phase: llm-pack    | Last run: 2026-05-15 14:20        | 2. api.example.com |
| Codex: plan-only        | Manifest: present                 | 3. admin.example   |
+----------------------------------------------------------------------------------+
| Recent Warnings                                                                   |
| - Codex output needs validation language                                          |
| - Optional API artifact missing: graphql-observations.json                        |
+----------------------------------------------------------------------------------+
| Actions: [Open Review Queue] [Open Validation] [Open Pipeline Runner] [Refresh]  |
+----------------------------------------------------------------------------------+
```

### Pipeline Runner

```text
+----------------------------------------------------------------------------------+
| Pipeline Runner                                                                  |
+----------------------------------------------------------------------------------+
| Scope: config\scope.example.txt          Profile: [passive v]                    |
| Include Codex: [x]                       Execute External Tools: [ ]             |
| Execute Codex: [ ]                       Output: output\                         |
+----------------------------------------------------------------------------------+
| Planned Phases                                                                    |
| 1. run         external-tool-phase   dry-run                                      |
| 2. graph       local-analysis-phase  execute locally                              |
| 3. api-intel   local-analysis-phase  execute locally                              |
| 4. enrich      local-analysis-phase  execute locally                              |
| 5. review      local-analysis-phase  execute locally                              |
| 6. llm-pack    local-analysis-phase  execute locally                              |
| 7. codex-run   local-analysis-phase  plan-only                                    |
| 8. validate    local-analysis-phase  execute locally                              |
+----------------------------------------------------------------------------------+
| Exact Command                                                                     |
| reconpilot pipeline --scope ... --profile passive --out output/ --include-codex  |
+----------------------------------------------------------------------------------+
| [Generate Plan] [Confirm And Run] [Open Plan File] [Cancel]                      |
+----------------------------------------------------------------------------------+
```

### Review Queue

```text
+----------------------------------------------------------------------------------+
| Review Queue                                                                      |
+----------------------------------------------------------------------------------+
| Filters: [Role v] [Environment v] [Risk v] [Search.........................]     |
+----------------------------------------------------------------------------------+
| Rank | Asset              | Score | Roles            | Env       | Evidence      |
| 1    | auth.example.com   | 82    | Auth, API        | Staging   | ev-1, ev-2    |
| 2    | admin.example.com  | 78    | AdminDashboard   | Internal  | ev-5, ev-9    |
| 3    | api.example.com    | 71    | ApiGateway       | Prod      | ev-12, ev-13  |
+----------------------------------------------------------------------------------+
| Detail Preview                                                                    |
| Why it matters: Shares infra with admin-like host; auth indicators observed.      |
| Next step: Review docs and auth flow manually. Requires validation.               |
+----------------------------------------------------------------------------------+
| [Open Asset Card] [Open Evidence] [Open Enrichment]                               |
+----------------------------------------------------------------------------------+
```

### Asset Detail Page

```text
+----------------------------------------------------------------------------------+
| Asset Detail: auth.example.com                                                    |
+----------------------------------------------------------------------------------+
| Roles: Authentication, ApiGateway    Environments: Staging, Internal Candidate    |
| Score: 82                            Confidence: 0.84                              |
+----------------------------------------------------------------------------------+
| Neighborhood Summary                                                              |
| Shares infrastructure with 3 hosts, references OpenAPI docs, and uses Grafana.   |
+----------------------------------------------------------------------------------+
| Evidence References                                                               |
| - ev-1: httpx title overlap                                                       |
| - ev-2: JS auth header reference                                                  |
| - ev-7: schema docs exposure                                                      |
+----------------------------------------------------------------------------------+
| Risk Explanations                                                                 |
| - Interesting auth surface candidate worth manual review.                         |
| - Schema exposure may reveal object models. Requires validation.                  |
+----------------------------------------------------------------------------------+
| [Open Graph Node] [Open API Intel] [Open Review Card]                             |
+----------------------------------------------------------------------------------+
```

### Graph Viewer

```text
+----------------------------------------------------------------------------------+
| Graph Viewer                                                                      |
+----------------------------------------------------------------------------------+
| Filters: [SharesIp x] [UsesTechnology x] [RedirectsTo ] [Search..............]   |
+----------------------------------------------------------------------------------+
| Node List                         | Relationship Canvas                           |
| - auth.example.com                |  auth.example.com ----SharesIp---- api.example|
| - api.example.com                 |         |                                     |
| - 203.0.113.10                    |         +----UsesTechnology---- Grafana       |
| - Grafana                         |                                               |
+----------------------------------------------------------------------------------+
| Cluster Summary                                                                   |
| infra-cluster-1: 4 nodes, shared IP, Grafana + auth surfaces                      |
+----------------------------------------------------------------------------------+
| [Open Cluster] [Open Asset Detail] [Open Anomalies]                               |
+----------------------------------------------------------------------------------+
```

### Codex Review Page

```text
+----------------------------------------------------------------------------------+
| Codex Review                                                                      |
+----------------------------------------------------------------------------------+
| Summary: 5 reviewed | 2 unsupported claims | 1 evidence gap | 3 wording warnings |
+----------------------------------------------------------------------------------+
| Asset             | Claims | Gaps | Warnings | Requires Validation | Executed     |
| auth.example.com  | 1      | 0    | 1        | no                  | yes          |
| app.example.com   | 0      | 1    | 0        | yes                 | yes          |
+----------------------------------------------------------------------------------+
| Selected Warning                                                                   |
| auth.example.com: "confirmed vulnerability" flagged as unsupported wording.       |
| Recommendation warning: "auth bypass" suggestion is outside the safety model.     |
+----------------------------------------------------------------------------------+
| [Open Original Result] [Open Sidecar] [Open Validation Report]                    |
+----------------------------------------------------------------------------------+
```
