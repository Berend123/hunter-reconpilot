# Safety And Scope Model

ReconPilot is intentionally designed around scope enforcement and conservative orchestration. The platform should help an operator stay inside agreed boundaries, not make it easier to overstep them.

## Principles

1. Scope comes first.
2. Passive collection is preferred before active collection.
3. Active recon must be explicit, reviewable, and rate-limited.
4. Port and service discovery are opt-in and require clear authorization.
5. Exploitation tooling is excluded from this phase by design.
6. Structured logs and outputs should make operator actions reviewable.
7. Validation should fail when required artifacts or integrity references are broken.
8. Local model reasoning must remain explicit, reviewable, and non-autonomous.

## Scope Inputs

ReconPilot expects:

- `scope.txt` for allowed domains, hosts, or URLs
- `excluded.txt` for forbidden items
- `reconpilot.json` for execution policy and guardrails

The scope file should never be empty. Empty scope means no permission to run.

The sample scope file in this repository is intentionally fake and uses `example.com` only. It exists for local MVP testing and documentation, not for real target use.

## What Scope Enforcement Should Do

- reject missing scope files
- reject empty scope files
- validate that each line is a domain, host, or URL pattern ReconPilot understands
- normalize scope entries before handing them to tools
- block targets that match the exclusion list
- require explicit policy switches for active content discovery and port scanning

## Passive-First Default

Passive or low-touch phases should be easiest to run:

- subdomain discovery through passive sources
- historical URL collection
- low-impact HTTP validation
- offline normalization and scoring

More active phases should require clearer intent:

- crawling
- content discovery
- port scanning
- deep service detection
- profile-driven external adapter execution through `reconpilot pipeline --execute`

## Excluded Tooling In This Phase

ReconPilot should not integrate:

- Burp Suite
- sqlmap
- dalfox
- xsstrike
- nuclei
- metasploit
- credential attack tooling
- brute-force authentication tooling
- destructive scanners

That separation keeps this phase focused on discovery, organization, and prioritization.

## Rate And Load Safety

When active phases are added, ReconPilot should enforce:

- concurrency caps
- per-host request limits
- optional delay and jitter
- host allowlists
- early stop behavior
- operator-visible phase summaries before execution

## Port Scanning Gate

Port and service discovery are more sensitive than URL collection.

For that reason:

- `naabu` and `nmap` should be disabled by default
- they should require explicit authorization in config
- the output should clearly record that a gated phase was used
- safe profiles should exist for passive-only and active-light modes

## Pipeline Profile Safety

Phase 9 adds named pipeline profiles, but it does not weaken the existing safety model.

- `reconpilot pipeline` still requires a scope file
- scope is validated before any target-touching phase such as `run` or `map`
- external-tool phases remain dry-run until `--execute` is explicitly passed
- local-only phases may run without `--execute` because they only transform local ReconPilot artifacts
- pipeline plans are written before execution so the operator can inspect order, inputs, and expected outputs
- pipeline manifests record the selected profile and per-phase results for later review
- `--include-codex` is required before the pipeline will even add `codex-run`
- `--execute` does not imply `--execute-codex`
- `--execute-codex` is ignored unless `--include-codex` is set

Phase 10 adds `reconpilot doctor`, which checks local MVP readiness without contacting targets. Use it before the first local run so config, docs, example files, and write permissions are verified up front.

## Secret Handling

Some JS recon tools may expose likely secrets or tokens.

ReconPilot should:

- avoid printing raw secret values unnecessarily
- mark secret-like strings as sensitive
- support redaction in future reporting layers
- keep operator review in the loop

## Evidence Hygiene

Structured outputs should preserve:

- source tool
- collection time
- input scope reference
- record type
- normalized form
- score rationale

This keeps the pipeline explainable and easier to audit.

## Audit And Validation

Phase 8 adds local reproducibility controls:

- `run-manifest.json` records command, paths, available hashes, generated plans, artifact counts, warnings, and errors.
- `audit-log.jsonl` records major phase events such as start, completion, artifact writes, warnings, optional-input skips, and errors.
- `validation-report.md` and `validation-report.json` highlight malformed artifacts, broken references, duplicate assets, and empty high-value outputs.

These controls stay local-only and help keep later review or model-assisted reasoning grounded in traceable evidence rather than assumptions.

The same audit trail now applies to named pipeline runs, so profile-based orchestration stays reproducible and reviewable instead of becoming a hidden execution shortcut.

## Optional Codex Reasoning Safety

Phase 11 adds `reconpilot codex-run`, but it keeps the same conservative model:

- plan mode is the default
- Codex is never executed unless `--execute-codex` is passed explicitly
- only local `codex exec` invocation is supported
- no `--yolo` or sandbox-bypass flags are used
- no target contact occurs in this phase
- prompts require cautious language such as candidate, interesting, worth manual review, and requires validation
- prompts prohibit destructive testing suggestions, credential attacks, and out-of-scope assumptions
- likely secrets and tokens are redacted before prompt construction

Codex output is reasoning support only. It does not validate findings and it does not replace human review.

Phase 12 adds `reconpilot codex-review`, which annotates local Codex outputs for unsupported claims, evidence gaps, and unsafe recommendations. It does not modify or delete the original Codex artifacts.

Validation now also checks optional `codex-insights/` and `codex-review/` artifacts for integrity when they are present, including plan safety flags, result references, review queue references, and summary consistency.

Even when all validation passes, Codex and review outputs remain hypotheses only. They support prioritization and manual review; they do not confirm vulnerabilities.

## GUI Safety Model

Phase 14 adds a desktop GUI, but it does not weaken the core CLI safety model.

Beginner Mode:

- dry-run stays the default
- stronger warnings are shown
- command execution requires confirmation
- custom profiles are disabled
- custom tool args are disabled

Advanced Mode:

- dry-run still stays the default
- fewer repeated prompts are allowed after acknowledgement
- GUI-defined profiles may be saved locally
- controlled custom tool args may be saved in GUI config
- rate and concurrency placeholders may be tuned
- exact command preview, scope awareness, logging, and Codex separation still apply

Hard blocks in both modes:

- no arbitrary shell execution from the GUI
- no hidden execution
- no pipeline launch without a scope file
- no `codex --yolo`
- no `codex --dangerously-bypass-approvals-and-sandbox`
- no accidental Codex execution
- no binaries outside the ReconPilot command allowlist

The GUI only launches structured `reconpilot` commands through an allowlisted backend bridge. It does not launch external recon tools directly.

## Recommended Operator Practice

- confirm scope before every run
- use exclusions aggressively
- start passive, then move to active-light only if approved
- inspect `output/plans/pipeline-plan.md` before using new profiles in a target-touching mode
- inspect `output/codex-insights/plans/codex-command-plan.md` before using `--execute-codex`
- inspect `output/codex-review/codex-review-summary.md` after any Codex run so unsupported claims and evidence gaps are visible before human follow-up
- keep GUI redaction enabled unless you have a specific local review need to disable it temporarily
- treat Advanced Mode as a workflow convenience, not a permission escalation
- document why port or service discovery was enabled
- keep raw outputs separate from normalized outputs
- treat prioritization as triage, not proof of risk
- review [KNOWN_LIMITATIONS.md](KNOWN_LIMITATIONS.md) before relying on the MVP outside the documented local workflow

## Future Safety Enhancements

- policy files with signed execution profiles
- scope-aware command generation that refuses wildcard broadening
- dry-run mode for every orchestration step
- redaction controls for reports
- approval gates for sensitive phases
