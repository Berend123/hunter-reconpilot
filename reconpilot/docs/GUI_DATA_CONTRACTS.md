# GUI Data Contracts

## Goal

Document the artifact contracts the future GUI will consume.

The GUI should prefer stable file-based contracts over hidden internal state. This keeps the desktop application aligned with the CLI and easier to validate.

## Contract Principles

- the CLI remains the producer of truth
- the GUI is primarily a consumer of existing artifacts
- parse failures must surface as UI errors, not silent data loss
- optional artifacts may be absent and should produce graceful empty states
- evidence references should remain visible end-to-end

## Primary Inputs

### `output/maps/graph.json`

Purpose:

- graph nodes, edges, clusters, and summary for the graph viewer and cross-linking

Expected top-level shape:

```json
{
  "nodes": [],
  "edges": [],
  "clusters": [],
  "summary": {}
}
```

Minimum GUI expectations:

- `nodes` array
- `edges` array
- stable node IDs
- edge references that can be matched to node IDs

Primary screens:

- Asset Graph Viewer
- Dashboard summary widgets
- Asset detail cross-links

### `output/maps/clusters.json`

Purpose:

- cluster-specific summaries for infrastructure and relationship groupings

Expected data:

- cluster ID
- cluster type
- related nodes
- shared indicators
- risk score

Primary screens:

- Asset Graph Viewer
- Dashboard

### `output/enrichment/semantic-assets.json`

Purpose:

- per-asset enriched semantic records

Expected data:

- asset identifier
- semantic tags
- roles
- environments
- risk explanations
- neighborhood summary
- optional API-linked observations

Primary screens:

- Semantic Enrichment Viewer
- Review Queue
- Asset detail page

### `output/review/priority-queue.json`

Purpose:

- analyst-facing ranked review queue

Expected data:

- ranked items
- score
- confidence
- semantic roles
- environments
- reasons
- evidence references
- suggested next steps

Primary screens:

- Dashboard
- Review Queue
- Asset detail page

### `output/api-intel/api-summary.md`

Purpose:

- human-readable API intelligence summary

Primary screens:

- API Intelligence Viewer
- Dashboard preview panels

### `output/api-intel/*.json`

Relevant files:

- `api-endpoints.json`
- `api-objects.json`
- `api-relationships.json`
- `auth-observations.json`
- `js-observations.json`
- `schemas.json`
- `graphql-observations.json`
- `api-graph.json`

Common GUI expectations:

- structured arrays or top-level objects
- asset or endpoint identifiers that can be cross-linked
- human-readable evidence fields

Primary screens:

- API Intelligence Viewer
- Asset detail page
- Semantic Enrichment Viewer

### `output/llm-pack/reasoning-queue.json`

Purpose:

- ranked reasoning queue for local model-ready context review

Expected data:

- asset
- prompt template
- reasoning priority
- context file reference

Primary screens:

- LLM Pack Viewer
- Dashboard

### `output/codex-insights/codex-summary.json`

Purpose:

- summary of Codex plan or execution results

Expected data:

- planned count
- executed count
- success/failure counts
- per-result metadata

Primary screens:

- Codex Insights
- Dashboard

### `output/codex-review/codex-review-queue.json`

Purpose:

- ranked annotation queue for Codex reasoning review

Expected data:

- summary object
- ranked review items
- unsupported claim counts
- evidence gap counts
- wording warning counts

Primary screens:

- Codex Review
- Dashboard

### `output/validation-report.json`

Purpose:

- machine-readable validation results

Expected data:

- overall success/failure state
- errors
- warnings
- check summaries

Primary screens:

- Validation / Audit
- Dashboard
- gating logic before Codex-oriented actions

### `output/run-manifest.json`

Purpose:

- run metadata and reproducibility context

Expected data:

- version
- command
- timestamp
- input/output paths
- scope hash when available
- config hash when available
- plan references
- warnings and errors

Primary screens:

- Dashboard
- Validation / Audit
- Scope Manager
- Pipeline Runner

### `output/audit-log.jsonl`

Purpose:

- append-only audit trail

Expected data:

- one event per line
- event type
- phase name
- timestamp
- optional message and details

Primary screens:

- Validation / Audit
- Pipeline Runner

## Contract Handling Guidance

### JSON

- parse strictly
- surface filename and parse error when invalid
- do not silently coerce malformed required artifacts

### JSONL

- parse line-by-line
- show line-level error counts when available
- support progressive loading later for large files

### Markdown

- render with safe markdown support later
- preserve raw view option for analyst review

## Cross-Link Expectations

The GUI should be able to cross-link:

- graph nodes to enriched assets
- enriched assets to review queue items
- review items to evidence references
- llm-pack entries to asset contexts and prompt templates
- codex-review items to original codex-insights outputs
- validation issues to the referenced artifact path

## Suggested View Models

The GUI should derive stable view models from artifacts rather than render every raw artifact directly.

Examples:

- `DashboardSnapshot`
- `PipelinePhaseView`
- `GraphNodeView`
- `AssetReviewView`
- `CodexReviewView`
- `ValidationIssueView`

These are design targets only for the future GUI scaffold.

## Versioning Approach

The GUI should tolerate additive schema changes where possible.

Recommended approach:

- require core fields
- ignore unknown fields safely
- display artifact version or manifest version when later added
- prefer explicit contract notes in docs over implicit frontend assumptions
