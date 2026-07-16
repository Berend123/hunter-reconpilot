# Outputs

ReconPilot is organized around structured local artifacts.

## Core Directories

### `output/plans/`

- adapter command plans
- pipeline plan files
- scope-derived helper input files

### `output/raw/`

- raw adapter outputs
- captured stdout and stderr when execution is enabled

### `output/dns/`

- DNS relationship artifacts such as `dnsx.jsonl`

### `output/screenshots/`

- screenshot metadata and image captures when enabled

### `output/tech/`

- technology fingerprinting artifacts such as `whatweb.json`

### `output/maps/`

- `app-map.json`
- `app-map.md`
- `graph-preview.md`
- `graph.json`
- `graph.md`
- `clusters.json`
- `clusters.md`
- `anomalies.json`
- `graph-summary.json`

### `output/api-intel/`

- `api-endpoints.json`
- `api-objects.json`
- `api-relationships.json`
- `auth-observations.json`
- `js-observations.json`
- `schemas.json`
- `graphql-observations.json`
- `api-graph.json`
- `api-summary.md`

### `output/enrichment/`

- `semantic-assets.json`
- `semantic-observations.json`
- `risk-explanations.json`
- `enriched-graph.json`
- `enrichment-summary.md`

### `output/review/`

- `priority-queue.md`
- `priority-queue.json`
- `asset-cards/`
- `review-checklist.md`
- `executive-summary.md`
- `evidence-index.json`

### `output/llm-pack/`

- `asset-contexts/`
- `prompts/`
- `reasoning-queue.json`
- `reasoning-queue.md`
- `analyst-brief.md`
- `pack-summary.json`

## Reproducibility Outputs

- `output/run-manifest.json`
- `output/audit-log.jsonl`
- `output/validation-report.md`
- `output/validation-report.json`

## MVP Review Tip

For a first local MVP pass, start by reviewing:

1. `output/plans/pipeline-plan.md`
2. `output/maps/graph.md`
3. `output/enrichment/enrichment-summary.md`
4. `output/review/priority-queue.md`
5. `output/llm-pack/analyst-brief.md`
6. `output/validation-report.md`
