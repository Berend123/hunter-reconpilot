# Demo Workflow

This walkthrough shows a safe local ReconPilot demo for the `0.1.0` release candidate.

## 1. Reset Old Artifacts

```powershell
.\scripts\reset-output.ps1
```

## 2. Run A Passive Demo

```powershell
.\scripts\test-passive.ps1
```

This keeps external-tool phases in dry-run mode and generates local artifacts for graphing, enrichment, review, and LLM pack inspection.

## 3. Open The GUI

```powershell
.\scripts\launch-gui.ps1
```

Open the repo root as the workspace and inspect:

- dashboard
- pipeline runner
- graph viewer
- enrichment viewer
- review queue
- validation and audit

## 4. Inspect Artifacts Directly

Recommended files:

- `output/plans/pipeline-plan.md`
- `output/maps/graph.md`
- `output/enrichment/enrichment-summary.md`
- `output/review/priority-queue.md`
- `output/llm-pack/reasoning-queue.md`
- `output/validation-report.md`

## 5. Run Codex Reasoning Safely

Plan-only first:

```powershell
.\target\debug\reconpilot.exe codex-run --pack output/llm-pack/ --out output/codex-insights/
```

Only if you explicitly want local Codex execution:

```powershell
.\target\debug\reconpilot.exe codex-run --pack output/llm-pack/ --out output/codex-insights/ --execute-codex
```

## 6. Review Codex Outputs Critically

```powershell
.\target\debug\reconpilot.exe codex-review --input output/codex-insights/ --out output/codex-review/
```

When reviewing Codex results:

- treat them as hypotheses only
- look for evidence gaps
- look for overconfident wording
- verify that the reasoning uses `requires validation` style language
- reject destructive, credential-focused, or out-of-scope suggestions

## 7. Finish With Validation

```powershell
.\target\debug\reconpilot.exe validate --input output/
```
