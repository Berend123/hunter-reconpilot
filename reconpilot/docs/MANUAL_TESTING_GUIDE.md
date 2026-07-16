# Manual Testing Guide

ReconPilot `0.1.0` is designed for analyst-led local testing. Use this guide when you move from repository setup into practical evaluation.

## Recommended Safe Testing Targets

Use environments where you have explicit permission to run recon workflows:

- OWASP Juice Shop
- DVWA
- WebGoat
- local Docker apps you control
- PortSwigger labs when the lab terms allow the workflow you want to test
- authorized bug bounty scopes only

## Recommended Testing Workflow

1. Run `reconpilot doctor` or `.\scripts\test-passive.ps1`.
2. Run the passive pipeline first.
3. Inspect graph outputs under `output/maps/`.
4. Inspect enrichment outputs under `output/enrichment/`.
5. Inspect the review queue under `output/review/`.
6. Inspect the LLM pack under `output/llm-pack/`.
7. Optionally run `codex-run` in plan-only mode first, then use `--execute-codex` only if you explicitly want local reasoning output.
8. Inspect `output/codex-review/` for unsupported claims, wording issues, and evidence gaps.
9. Run `reconpilot validate --input output/` at the end.

## What To Evaluate

- signal-to-noise in the passive pipeline
- graph quality and useful relationship density
- review ranking quality
- API inference quality
- Codex reasoning quality
- GUI usability
- false positives
- missing relationships

## Suggested Testing Notebook Template

```text
Target:
Interesting findings:
What ReconPilot surfaced:
What it missed:
False positives:
Weak heuristics:
Useful Codex reasoning:
Bad Codex reasoning:
Ideas for improvement:
```

## Safety Reminders

- authorized targets only
- respect rate limits
- dry-run first
- validate manually
- Codex output is hypothesis-only

## Useful Local Commands

```powershell
.\scripts\reset-output.ps1
.\scripts\test-passive.ps1
.\scripts\launch-gui.ps1
.\target\debug\reconpilot.exe codex-run --pack output/llm-pack/ --out output/codex-insights/
.\target\debug\reconpilot.exe codex-review --input output/codex-insights/ --out output/codex-review/
.\target\debug\reconpilot.exe validate --input output/
```
