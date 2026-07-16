# Quickstart

ReconPilot is dry-run-first and recon-only. Do not run it against any real target without explicit written authorization.

## Minimal Local Workflow

```powershell
reconpilot --version
reconpilot doctor
reconpilot init
reconpilot check-tools
reconpilot pipeline --scope config/scope.example.txt --profile passive --out output/
reconpilot validate --input output/
reconpilot llm-pack --input output/ --out output/llm-pack/
reconpilot codex-run --pack output/llm-pack/ --out output/codex-insights/
reconpilot codex-review --input output/codex-insights/ --out output/codex-review/
```

This sequence matches the local v0.1.0 release-candidate smoke workflow. The passive profile keeps external-tool phases in dry-run mode while still allowing local-only phases to generate artifacts from local data.

## GUI Workflow

```powershell
cd gui
npm install
npm test
npm run build
```

If you want populated views, generate local artifacts first with the CLI workflow above or use the GUI pipeline runner in dry-run mode.

Use the GUI when you want:

- workspace health detection and artifact browsing
- pipeline preview with exact command display
- review queue filtering and sorting
- rendered asset cards and linked Codex review context
- grouped validation, audit, and GUI execution log views

## Notes

- `config/scope.example.txt` is intentionally fake and uses `example.com` only.
- `passive` keeps external-tool phases in dry-run mode unless you explicitly switch to an execution-capable profile and pass `--execute`.
- local-only phases such as `graph`, `api-intel`, `enrich`, `review`, `llm-pack`, `codex-run`, `codex-review`, and `validate` can still build local artifacts from planned or existing data.
- `codex-run` is plan-only by default. Use `--execute-codex` only when you explicitly want local Codex reasoning outputs.
- `--include-codex` adds `codex-run` after `llm-pack` inside the pipeline, but it still stays plan-only unless `--execute-codex` is also passed.
- `--execute` never implies `--execute-codex`.
- the GUI remains bound to the ReconPilot command allowlist and does not run arbitrary shell commands.
- the GUI defaults to redaction for sensitive-looking values.

## Next References

- [README.md](README.md)
- [docs/MVP_USAGE.md](docs/MVP_USAGE.md)
- [docs/PIPELINE_PROFILES.md](docs/PIPELINE_PROFILES.md)
- [docs/OUTPUTS.md](docs/OUTPUTS.md)
- [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md)
- [KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md)
