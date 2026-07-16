# MVP Usage

ReconPilot 0.1.0 is a local-first MVP for planning, correlating, enriching, reviewing, and packaging recon artifacts. It is not an exploitation framework.

## Recommended First Run

```powershell
reconpilot doctor
reconpilot init
reconpilot check-tools
reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/
reconpilot validate --input output/
```

## Why Start With `passive`

- it keeps `run` in dry-run mode
- it still exercises the local graph, API, enrichment, review, and LLM-pack phases
- it generates a full local MVP artifact tree without requiring external tool execution

## When To Use `--execute`

Only pass `--execute` when:

- the scope is real and explicitly authorized
- the profile includes target-touching phases you intend to run
- you reviewed `output/plans/pipeline-plan.md` or the direct phase plan files first

## Suggested Operator Sequence

1. Run `reconpilot doctor`.
2. Review `config/reconpilot.example.json`, `config/scope.example.txt`, and `config/excluded.example.txt`.
3. Use a planning-oriented profile first.
4. Inspect `output/plans/`.
5. Validate output integrity with `reconpilot validate --input output/`.
6. Review `output/review/` and `output/llm-pack/` only after validation passes.

## Safety Reminder

ReconPilot produces candidates, prioritization hints, and local reasoning packs. It does not confirm vulnerabilities, and it should not be used to bypass explicit program rules.
