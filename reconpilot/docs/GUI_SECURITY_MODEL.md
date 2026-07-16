# GUI Security Model

## Goal

Define how a future ReconPilot GUI preserves the existing CLI safety model.

The GUI must not weaken scope enforcement, dry-run defaults, or explicit execution gates.

## Core Security Posture

- dry-run first
- scope-first workflow
- explicit confirmations before execution
- no hidden execution
- no target contact from GUI code
- no automatic Codex execution
- no dangerous flags
- output-only viewing mode as the default experience
- audit visibility for any action that launches CLI behavior

## Execution Boundaries

### Read-only mode

The default GUI mode should be artifact viewing only:

- read output files
- render validation results
- inspect review queues
- inspect Codex outputs

No command execution should happen during normal browsing.

### Confirmed execution mode

If the GUI later triggers CLI commands, it must:

- show the exact command
- show whether the command is local-only or target-touching
- show whether it is dry-run or execution mode
- require explicit operator confirmation
- capture manifest and audit updates afterward

## Required Safety Controls

### 1. Dry-run default

- `run` and `map` stay dry-run unless explicitly executed
- `pipeline --execute` must remain a distinct choice
- `pipeline --execute` must not imply `--execute-codex`

### 2. Codex control

- `codex-run` must remain plan-only unless `--execute-codex` is explicitly set
- GUI must never auto-run Codex because a pack exists
- GUI should display Codex as reasoning support only

### 3. Scope-first workflow

- scope and exclusions should be visible before target-touching actions
- scope hash from the manifest should be displayed when available
- missing or invalid scope should block target-touching launch flows

### 4. No hidden flags

The GUI must never append:

- `--yolo`
- `--dangerously-bypass-approvals-and-sandbox`
- hidden execution flags
- implicit broadening of target scope

### 5. Output-only viewing mode

The first GUI implementation should prioritize:

- project opening
- artifact viewing
- validation awareness
- review and evidence workflows

before adding any execution controls.

## Secret Handling

The UI should assume some artifacts may contain sensitive values.

Planned controls:

- redact likely secrets in default views
- allow copy-with-caution workflows later
- avoid rendering raw authorization headers in summary tables
- preserve evidence references while masking sensitive token bodies

Examples to redact:

- bearer tokens
- JWT-like values
- API keys
- long hex strings
- long base64-like strings
- `Authorization:` header values

## Validation Before Reasoning

The GUI should show validation state before Codex-oriented actions.

Recommended behavior:

- if `validation-report.json` contains required errors, show a blocking warning before offering Codex execution controls
- if `codex-review` exists and contains unsupported claims or evidence gaps, show that prominently in the Codex results area

## Audit Visibility

Any command-triggering UI flow should make audit visibility easy:

- show last command from the manifest
- show relevant audit events
- show warnings and errors after completion

## Trust Model

The GUI trusts:

- the local filesystem within the chosen workspace
- the existing ReconPilot CLI to enforce execution safety

The GUI should not trust:

- malformed JSON artifacts
- unstated execution state
- missing evidence references
- reasoning text that sounds more certain than the evidence supports

## Unsafe Content Review Rules

The GUI should visually flag:

- confirmed-vulnerability wording
- exploit-oriented recommendations
- credential attack suggestions
- destructive testing suggestions
- out-of-scope assumptions

It should not rewrite those artifacts automatically. It should annotate them and direct the analyst to validation and Codex review outputs.

## Future Hardening Ideas

- per-command confirmation policies
- signed workspace trust configuration
- tamper-evident manifest lineage
- GUI-side artifact checksum caching
- sensitive-field masking policies configurable per workspace
