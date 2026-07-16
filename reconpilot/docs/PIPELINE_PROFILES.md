# Pipeline Profiles

ReconPilot Phase 9 introduces named orchestration profiles that run existing phases in a controlled order.

## Commands

```powershell
reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/
reconpilot pipeline --scope config/scope.example.txt --profile active-lite --out output/ --execute
```

## Profiles

### `passive`

- `run` in dry-run mode
- `graph`
- `api-intel`
- `enrich`
- `review`
- `llm-pack`
- `validate`

Use this first for local MVP validation and artifact generation.

### `active-lite`

- `run`
- `map`
- `graph`
- `api-intel`
- `enrich --api-intel`
- `review`
- `llm-pack`
- `validate`

External-tool phases only launch when the pipeline command is invoked with `--execute`.

### `api-focused`

- `api-intel`
- `enrich --api-intel`
- `review`
- `llm-pack`
- `validate`

Use when graph-aware application artifacts already exist and you want API-heavy analysis.

### `mapping-focused`

- `map`
- `graph`
- `enrich`
- `review`
- `validate`

Use when the immediate goal is infrastructure mapping and local graph review.

### `review-only`

- `enrich`
- `review`
- `validate`

Use when graph outputs already exist and you want a fresh local review workspace.

### `llm-pack-only`

- `llm-pack`
- `validate`

Use when review and enrichment outputs already exist and you only need local reasoning packs.

## Planning Artifacts

Every profile writes:

- `output/plans/pipeline-plan.json`
- `output/plans/pipeline-plan.md`

These files identify:

- phase order
- external-tool vs local-analysis phases
- dry-run or execute state
- required inputs
- expected outputs
