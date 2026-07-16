# Release Checklist

Use this checklist before tagging a local v0.1.0 release candidate.

## Core Validation

- [ ] Run `cargo fmt`
- [ ] Run `cargo +stable-x86_64-pc-windows-gnullvm test`
- [ ] Run `cargo +stable-x86_64-pc-windows-gnullvm build`
- [ ] Run `reconpilot doctor`

## GUI Validation

- [ ] Run `cd gui`
- [ ] Run `npm install`
- [ ] Run `npm test`
- [ ] Run `npm run build`
- [ ] Confirm `gui/README.md` quickstart still matches the current workflow

## Smoke Workflow

- [ ] Run `reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/`
- [ ] Run `reconpilot validate --input output/`
- [ ] Run `reconpilot llm-pack --input output/ --out output/llm-pack/`
- [ ] Run `reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/`
- [ ] Run `reconpilot codex-review --input output/codex-insights/ --out output/codex-review/`
- [ ] Confirm the smoke workflow stayed dry-run-safe for external-tool phases

## Safety Review

- [ ] Confirm `--execute` does not imply `--execute-codex`
- [ ] Confirm Codex remains plan-only unless `--execute-codex` is passed
- [ ] Confirm scope example remains fake and authorization-safe
- [ ] Confirm no exploit tooling or destructive testing guidance was added
- [ ] Confirm docs still use cautious language such as `candidate`, `worth review`, and `requires validation`

## Documentation Review

- [ ] Confirm `README.md` references the current workflow
- [ ] Confirm `QUICKSTART.md` reflects the release-candidate smoke flow
- [ ] Confirm `CHANGELOG.md` includes `0.1.0`
- [ ] Confirm `ROADMAP.md` contains post-v0.1 work
- [ ] Confirm `KNOWN_LIMITATIONS.md` is present and current
- [ ] Confirm `SAFETY_AND_SCOPE.md` matches current GUI and Codex safety controls

## Artifact Hygiene

- [ ] Confirm generated `output/` artifacts remain gitignored
- [ ] Confirm example fixtures and example artifacts remain tracked
- [ ] Confirm release-only local logs were not committed accidentally

## Tagging

- [ ] Review the final validation results one last time
- [ ] Create the release tag only after the checklist is complete
- [ ] Suggested tag sequence:

```powershell
git tag v0.1.0-rc1
git push origin v0.1.0-rc1
```

- [ ] Promote to `v0.1.0` only after the release candidate is accepted
