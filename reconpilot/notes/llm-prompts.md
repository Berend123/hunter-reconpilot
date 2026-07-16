# ReconPilot LLM Prompt Ideas

These prompts are intended for future analyst-in-the-loop use. They should only operate on structured, in-scope recon data.

## Classify Endpoints

```text
You are assisting with recon triage.

Classify the following endpoints into likely categories such as:
- auth
- admin
- api
- upload
- reporting
- debugging
- static
- unknown

Return JSON with:
- endpoint
- category
- confidence
- short_reason

Input:
{{endpoint_records}}
```

## Score Assets

```text
You are assisting with recon prioritization.

Given these asset records, score each asset from 0 to 100 for operator review priority.
Prioritize assets that appear administrative, internal, staging-like, login-related, or unusually exposed.

Return JSON with:
- asset
- score
- reasons
- next_safe_recon_step

Input:
{{asset_records}}
```

## Summarize JS Findings

```text
Summarize the following JavaScript-derived findings for a security analyst.

Focus on:
- hidden endpoints
- tokens or secret-like strings
- internal hostnames
- upload paths
- admin or debugging references

Return:
- concise_summary
- notable_endpoints
- notable_parameters
- suspicious_strings

Input:
{{js_findings}}
```

## Identify Suspicious Parameters

```text
Review the following parameter records and identify parameter names that deserve additional recon attention.

Look for:
- redirect behavior
- tokens or keys
- file handling
- debugging flags
- auth or session controls

Return JSON with:
- parameter
- suspicion_level
- reason

Input:
{{parameter_records}}
```

## Explain Risk Factors

```text
Explain why the following finding may be high-value from a recon perspective.

Keep the response focused on:
- exposed workflow significance
- trust boundary clues
- likely asset sensitivity
- safest next recon step

Input:
{{finding_record}}
```

## Generate Prioritized Next Steps

```text
You are generating next-step recommendations for a recon-only workflow.

Given the following scored findings, propose the next five safest investigation steps.
Do not propose exploitation.
Prefer passive validation, route review, metadata enrichment, or targeted crawling.

Return JSON with:
- step
- target
- reason
- expected_signal

Input:
{{scored_findings}}
```

## Detect Likely Staging Or Internal Systems

```text
Identify records that likely belong to staging, preview, development, VPN-only, or internal-facing systems.

Use clues such as:
- hostnames
- titles
- path names
- technologies
- JavaScript references

Return JSON with:
- asset
- classification
- confidence
- indicators

Input:
{{asset_and_url_records}}
```

## Phase 7 Local Prompt Pack Templates

`reconpilot llm-pack` now generates reusable local prompt templates under `output/llm-pack/prompts/`:

- `asset_triage_prompt.md`
- `api_surface_reasoning_prompt.md`
- `auth_flow_review_prompt.md`
- `js_intelligence_review_prompt.md`
- `report_draft_prompt.md`

Each generated template is designed to:

- prioritize assets instead of suggesting exploitation
- produce hypotheses instead of confirmed vulnerability claims
- require evidence-backed reasoning
- require `requires validation` language
- prohibit destructive testing suggestions
- prohibit credential attacks
- prohibit out-of-scope assumptions

The preferred future flow is:

```text
graph + api-intel -> enrich -> review -> llm-pack -> codex-run -> analyst-controlled local model use
```

The pack layer remains local-only. It prepares compact per-asset context bundles, a reasoning queue, and an analyst brief, but it does not execute any model by itself.

## Phase 11 Codex Runner Prompt Envelope

`reconpilot codex-run` consumes the `llm-pack` prompts and wraps them in an additional safety envelope before any optional local `codex exec` run.

That envelope requires:

- evidence-backed reasoning only
- hypotheses instead of vulnerability claims
- `requires validation` language
- no destructive testing suggestions
- no credential attacks
- no out-of-scope assumptions
- no autonomous scanning or target contact

It also redacts likely secrets such as bearer tokens, JWT-looking strings, API keys, authorization headers, and long hex or base64 blobs before prompt construction.
