# ReconPilot Tool Catalog

This document describes the tools ReconPilot intends to orchestrate in this phase. Each entry explains where the tool fits, what it is good at, where it falls short, and how to use it safely.

## MVP Notes

- external-tool phases remain dry-run by default
- `reconpilot doctor` checks local tool visibility and warns when optional tools are missing
- `reconpilot pipeline` uses named profiles to orchestrate the existing phases without changing the underlying safety model

## Core Discovery

### subfinder

- Purpose: Passive subdomain enumeration from a broad set of data sources.
- Recon phase: Collection.
- Strengths: Fast, quiet relative to active enumeration, JSON-friendly output.
- Weaknesses: Source coverage varies by environment and API access.
- Example usage: `subfinder -d example.com -oJ`
- Safe usage guidance: Prefer passive mode first and keep API credentials separate from project output.
- Role: Core.
- Expected output format: JSONL or plain text subdomain list.

### amass

- Purpose: Subdomain and asset discovery with graph-oriented enrichment options.
- Recon phase: Collection and enrichment.
- Strengths: Deep coverage, rich metadata, good historical context.
- Weaknesses: Heavier runtime and configuration complexity than simpler passive tools.
- Example usage: `amass enum -passive -d example.com -json amass.json`
- Safe usage guidance: Start with passive enumeration before considering active features.
- Role: Core.
- Expected output format: JSON, text, graph-oriented data.

### assetfinder

- Purpose: Lightweight asset and subdomain collection.
- Recon phase: Collection.
- Strengths: Simple, fast, easy to compose in shell pipelines.
- Weaknesses: Sparse metadata and inconsistent depth across providers.
- Example usage: `assetfinder --subs-only example.com`
- Safe usage guidance: Treat results as candidate assets that still need validation.
- Role: Optional but useful core companion.
- Expected output format: Plain text.

### findomain

- Purpose: Fast subdomain discovery with certificate and data-source support.
- Recon phase: Collection.
- Strengths: Speed and simple workflow.
- Weaknesses: Output context is thinner than deeper asset graph tools.
- Example usage: `findomain -t example.com -q`
- Safe usage guidance: Normalize output before mixing it with other sources.
- Role: Optional but recommended.
- Expected output format: Plain text, optionally files.

## Live Host Probing

### httpx

- Purpose: Validate live HTTP services and collect metadata such as title, status, tech, and TLS information.
- Recon phase: Collection and enrichment.
- Strengths: Fast probing, useful metadata, common JSONL output.
- Weaknesses: Can create noisy output if probe flags are too broad.
- Example usage: `httpx -l hosts.txt -json -tech-detect -status-code`
- Safe usage guidance: Respect scope and rate limits, and do not add intrusive probes without authorization.
- Role: Core.
- Expected output format: JSONL.

## Mapping And Enrichment

### dnsx

- Purpose: Resolve discovered or scoped hosts and preserve DNS relationship data for mapping.
- Recon phase: Mapping and enrichment.
- Strengths: Fast DNS resolution with JSONL output and explicit rate-limit controls.
- Weaknesses: DNS-only visibility still needs HTTP and content context to become actionable.
- Example usage: `dnsx -l subdomains.txt -silent -j -o dnsx.jsonl`
- Safe usage guidance: Use scope-derived or previously discovered hosts only, and keep DNS rate limits conservative.
- Role: Core mapping adapter.
- Expected output format: JSONL.

### gowitness

- Purpose: Capture screenshots and basic visual context for live hosts.
- Recon phase: Mapping and enrichment.
- Strengths: Fast visual clustering of apps, portals, admin panels, and repeated UI families.
- Weaknesses: Requires a browser runtime and still makes real requests to target systems.
- Example usage: `gowitness scan file -f live-hosts.txt --screenshot-path screenshots --write-jsonl gowitness.jsonl`
- Safe usage guidance: Screenshots must obey program rules because they still touch target systems and may trigger application-side logging.
- Role: Optional but high-value mapping adapter.
- Expected output format: Screenshots plus JSONL metadata.

### WhatWeb

- Purpose: Fingerprint technologies and application stack hints for live hosts.
- Recon phase: Mapping and enrichment.
- Strengths: Broad plugin coverage and structured JSON logging.
- Weaknesses: Higher aggression settings can become noisier than needed for early recon.
- Example usage: `whatweb -i live-hosts.txt -a 1 -t 5 --log-json=whatweb.json`
- Safe usage guidance: Keep aggression low, keep thread counts conservative, and only run against validated live hosts.
- Role: Optional but recommended.
- Expected output format: JSON.

## Crawling And URL Collection

### katana

- Purpose: Modern crawler for route discovery and page traversal.
- Recon phase: Collection.
- Strengths: Flexible crawling behavior and structured output.
- Weaknesses: Active crawling can expand quickly on large applications.
- Example usage: `katana -u https://app.example.com -jsonl`
- Safe usage guidance: Constrain hosts, depth, and rate before large runs.
- Role: Core.
- Expected output format: JSONL.

### gau

- Purpose: Gather historical URLs from known archives and public sources.
- Recon phase: Collection.
- Strengths: Good for resurfacing older routes and forgotten endpoints.
- Weaknesses: Historical data can be stale or noisy.
- Example usage: `gau example.com --json`
- Safe usage guidance: Treat historical URLs as hints and re-validate before acting on them.
- Role: Core.
- Expected output format: JSONL or text.

### waybackurls

- Purpose: Pull archived URLs from the Wayback Machine for a domain.
- Recon phase: Collection.
- Strengths: Extremely simple and useful for broad historical coverage.
- Weaknesses: Noisy and less structured than newer tools.
- Example usage: `waybackurls example.com`
- Safe usage guidance: Normalize and deduplicate results before any downstream use.
- Role: Optional but commonly valuable.
- Expected output format: Plain text.

### hakrawler

- Purpose: Fast stream-oriented crawler for endpoint discovery.
- Recon phase: Collection.
- Strengths: Quick, easy to chain in pipelines, low ceremony.
- Weaknesses: Less context-rich than more feature-heavy crawlers.
- Example usage: `hakrawler -url https://app.example.com -depth 2 -plain`
- Safe usage guidance: Keep crawl depth and subdomain traversal under control.
- Role: Optional but useful.
- Expected output format: Plain text.

## Normalization And Filtering

### uro

- Purpose: URL deduplication and reduction of low-value duplicate route patterns.
- Recon phase: Normalization.
- Strengths: Cuts redundant URL noise before human review.
- Weaknesses: Aggressive normalization can discard context if applied blindly.
- Example usage: `cat urls.txt | uro`
- Safe usage guidance: Keep raw URL archives before applying reductions.
- Role: Core normalizer.
- Expected output format: Plain text.

### jq

- Purpose: JSON and JSONL slicing, transformation, and field extraction.
- Recon phase: Normalization and enrichment.
- Strengths: Precise, scriptable, excellent for structured data pipelines.
- Weaknesses: Requires careful query maintenance as schemas evolve.
- Example usage: `jq -c '{url, status_code, tech}' httpx.jsonl`
- Safe usage guidance: Version and review shared jq filters to avoid schema drift.
- Role: Core utility.
- Expected output format: JSON or JSONL.

### gf

- Purpose: Pattern matching helper for extracting interesting URL or parameter classes.
- Recon phase: Normalization and enrichment.
- Strengths: Fast categorization of recurring patterns.
- Weaknesses: Pattern quality depends entirely on the installed rule set.
- Example usage: `cat urls.txt | gf xss`
- Safe usage guidance: Use for labeling and triage, not as proof of risk.
- Role: Optional but useful.
- Expected output format: Plain text matches.

## Content Discovery

### ffuf

- Purpose: Content and route discovery through wordlist-driven fuzzing.
- Recon phase: Collection.
- Strengths: Flexible, fast, strong filtering controls.
- Weaknesses: Active probing can be noisy if not rate-limited and scoped.
- Example usage: `ffuf -u https://app.example.com/FUZZ -w wordlist.txt -mc 200,204,301,302`
- Safe usage guidance: Use only where explicitly allowed and keep conservative concurrency.
- Role: Core active recon tool.
- Expected output format: JSON, CSV, HTML, or terminal output.

### feroxbuster

- Purpose: Recursive content discovery with strong filtering and reporting support.
- Recon phase: Collection.
- Strengths: Good recursion model and useful filtering features.
- Weaknesses: Can generate heavy request volume on large targets.
- Example usage: `feroxbuster -u https://app.example.com -x php,txt,json -o ferox.json`
- Safe usage guidance: Limit recursion, delay requests where needed, and use allowlists.
- Role: Optional but strong.
- Expected output format: JSON, text.

### dirsearch

- Purpose: Python-based directory and file brute-forcing for web content discovery.
- Recon phase: Collection.
- Strengths: Familiar workflow and broad community usage.
- Weaknesses: Slower than some compiled alternatives depending on environment.
- Example usage: `python dirsearch.py -u https://app.example.com -e php,js,json`
- Safe usage guidance: Use only in approved active phases and document chosen wordlists.
- Role: Optional.
- Expected output format: Text, CSV, JSON, HTML.

## JavaScript Recon

### LinkFinder

- Purpose: Extract endpoints and route-like strings from JavaScript files.
- Recon phase: Collection and enrichment.
- Strengths: Good at surfacing hidden API and route references from JS.
- Weaknesses: Regex-oriented extraction can produce false positives.
- Example usage: `python linkfinder.py -i https://app.example.com/app.js -o cli`
- Safe usage guidance: Review output context before treating a match as a real route.
- Role: Core JavaScript recon helper.
- Expected output format: CLI text or HTML.

### SecretFinder

- Purpose: Scan JavaScript and response content for likely secrets, tokens, and keys.
- Recon phase: Enrichment.
- Strengths: Focused secret pattern matching for JS-heavy applications.
- Weaknesses: Regex-only detection can over-report low-value matches.
- Example usage: `python SecretFinder.py -i https://app.example.com/app.js -o cli`
- Safe usage guidance: Handle possible secrets carefully and avoid storing raw values in shared reports.
- Role: Optional but valuable.
- Expected output format: CLI text or HTML.

### JSParser

- Purpose: Parse JavaScript for endpoints, domains, and potentially sensitive references.
- Recon phase: Collection and enrichment.
- Strengths: Useful companion to LinkFinder for broader JS analysis.
- Weaknesses: Output quality depends on the JavaScript style and complexity.
- Example usage: `python jsparser.py -u https://app.example.com/app.js`
- Safe usage guidance: Run against in-scope JavaScript only and normalize output before merging.
- Role: Optional but recommended.
- Expected output format: CLI text.

## ReconPilot Internal Analysis Layers

These are deterministic local stages inside ReconPilot rather than external binaries. They consume artifacts already collected by earlier phases.

### API Intelligence Layer

- Purpose: Normalize API endpoints, group API families, infer object models, and capture documentation or schema exposure hints from local artifacts.
- Recon phase: Local analysis after graph, crawler, and JavaScript artifacts exist.
- Strengths: Typed JSON outputs, explainable evidence chains, and no target contact.
- Weaknesses: Coverage depends on artifact quality and collected depth; missing local artifacts reduce fidelity.
- Example usage: `reconpilot api-intel --input output/ --out output/api-intel/`
- Safe usage guidance: This phase is local-only and should never trigger follow-up fetches or autonomous requests.
- Role: Core internal analysis layer.
- Expected output format: JSON and Markdown artifacts under `output/api-intel/`.

### JavaScript Semantic Analysis

- Purpose: Extract hidden routes, auth references, feature flags, role hints, environment references, and schema URLs from local JavaScript artifacts.
- Recon phase: Local analysis and enrichment.
- Strengths: Surfaces routes and capability hints that crawlers may miss.
- Weaknesses: Regex-oriented extraction still produces candidates rather than guaranteed reachable routes.
- Example usage: Generated automatically by `reconpilot api-intel`.
- Safe usage guidance: Treat JS-discovered routes as interesting candidates and validate them manually later within program rules.
- Role: Core sub-layer of API intelligence.
- Expected output format: `js-observations.json` and `api-graph.json`.

### Schema And Documentation Analysis

- Purpose: Parse local Swagger, OpenAPI, or Redoc artifacts and identify endpoint, object, and auth-scheme relationships.
- Recon phase: Local analysis and enrichment.
- Strengths: Deterministic parsing of documented APIs and direct extraction of object and auth metadata.
- Weaknesses: Intentionally limited to artifacts already on disk; it will not fetch missing schemas.
- Example usage: Generated automatically by `reconpilot api-intel`.
- Safe usage guidance: Documentation exposure is a candidate worth review, not proof of an issue.
- Role: Core sub-layer of API intelligence.
- Expected output format: `schemas.json`, `api-endpoints.json`, and `api-relationships.json`.

### Auth Observation Logic

- Purpose: Detect `Authorization`, `Bearer`, `JWT`, `OAuth`, `OIDC`, `SAML`, session-cookie, refresh-token, and CSRF terminology in local artifacts.
- Recon phase: Local semantic analysis.
- Strengths: Deterministic and explainable prioritization context for auth-related systems or API surfaces.
- Weaknesses: Presence of auth keywords does not imply a weakness or misconfiguration.
- Example usage: Generated automatically by `reconpilot api-intel`.
- Safe usage guidance: Use auth observations to guide cautious manual review later; do not assume an auth flaw exists.
- Role: Core sub-layer of API intelligence.
- Expected output format: `auth-observations.json`.

### Object Inference

- Purpose: Infer likely API objects such as `User`, `Account`, `Organization`, or `Billing` from paths, parameters, and schema references.
- Recon phase: Local semantic analysis.
- Strengths: Converts flat routes into business-object context that improves prioritization and future graph reasoning.
- Weaknesses: Heuristic singularization and naming inference can misclassify ambiguous routes.
- Example usage: Generated automatically by `reconpilot api-intel`.
- Safe usage guidance: Treat inferred object sensitivity as a review hint that still requires business-context validation.
- Role: Core sub-layer of API intelligence.
- Expected output format: `api-objects.json`.

### Graph Expansion And Correlation

- Purpose: Extend the graph with `ApiEndpoint`, `ApiObject`, `ApiSchema`, `AuthFlow`, `JsAsset`, `parameter`, `token`, and `api_family` nodes plus deterministic edges.
- Recon phase: Local graph augmentation.
- Strengths: Lets later prioritization reason about infrastructure and application capability relationships in the same dataset.
- Weaknesses: Live schema validation, visual similarity, and richer application semantics are deferred to later phases.
- Example usage: Generated automatically by `reconpilot api-intel`.
- Safe usage guidance: Expanded graph relationships are evidence-backed candidates, not vulnerability assertions.
- Role: Core local graph augmentation layer.
- Expected output format: `api-graph.json`.

### Future LLM Integration Opportunities

- Purpose: Prepare stable, deterministic, evidence-rich API and JS outputs that future local or remote reasoning layers can consume.
- Recon phase: Future reasoning and prioritization.
- Strengths: Keeps any later model-assisted analysis downstream of structured, explainable evidence.
- Weaknesses: LLM integration is intentionally deferred; this phase does not call models or remote APIs.
- Example usage: Future phases can consume `api-summary.md`, `api-graph.json`, and `auth-observations.json`.
- Safe usage guidance: Preserve deterministic evidence chains so any future model output can always be traced back to local artifacts.
- Role: Future extension point.
- Expected output format: Not active in the current phase.

### API-Aware Enrichment Feedback Loop

- Purpose: Feed local API-intel outputs into semantic enrichment so review sees API/auth/schema/GraphQL/JS context as part of the enriched assets rather than as a separate late-stage add-on.
- Recon phase: Local semantic enrichment.
- Strengths: Keeps the preferred flow `graph + api-intel -> enrich -> review`, improves deterministic prioritization, and reduces review-time evidence duplication.
- Weaknesses: Depends on prior API-intel artifacts being present and well-formed; otherwise enrichment falls back to graph-only behavior.
- Example usage: `reconpilot enrich --input output/maps/ --api-intel output/api-intel/ --out output/enrichment/`
- Safe usage guidance: This remains local-only and uses cautious language such as candidate, interesting, potentially sensitive, worth review, and requires validation.
- Role: Core Phase 6B feedback loop.
- Expected output format: API-aware `semantic-assets.json`, `semantic-observations.json`, `risk-explanations.json`, `enriched-graph.json`, and `enrichment-summary.md`.

### Future LLM Consumption Of Enriched API Context

- Purpose: Preserve API-aware semantic overlays in a form that later LLM or local-model reasoning can consume without re-parsing raw crawler, JS, auth, or schema artifacts.
- Recon phase: Future reasoning and prioritization.
- Strengths: Keeps future model inputs grounded in deterministic evidence, graph neighborhoods, inferred object context, and cautious risk explanations.
- Weaknesses: This phase does not invoke any model yet, so prioritization remains heuristic and evidence-led.
- Example usage: Future phases can consume `enriched-graph.json`, `semantic-assets.json`, `semantic-observations.json`, and `enrichment-summary.md`.
- Safe usage guidance: Maintain traceable evidence chains so any future model output can be audited back to local artifacts.
- Role: Future extension point informed by Phase 6B.
- Expected output format: Not active in the current phase.

### Local LLM Reasoning Pack

- Purpose: Convert review and enrichment artifacts into compact, evidence-led context bundles and prompt templates for analyst-controlled local model use later.
- Recon phase: Local packaging and reasoning preparation.
- Strengths: Preserves evidence IDs, keeps prompts safety-constrained, deduplicates overlapping evidence, and applies a deterministic context budget.
- Weaknesses: It does not execute any model, so prioritization remains analyst-directed and heuristic until a later phase adds explicit local model invocation.
- Example usage: `reconpilot llm-pack --input output/ --out output/llm-pack/`
- Safe usage guidance: Prompt packs must ask for prioritization and hypotheses only; they must not suggest exploitation, destructive testing, credential attacks, or out-of-scope assumptions.
- Role: Core Phase 7 packaging layer.
- Expected output format: JSON asset contexts, Markdown prompt templates, reasoning queue exports, and a pack summary manifest under `output/llm-pack/`.

## Port And Service Discovery

### naabu

- Purpose: Fast port scanning for host exposure mapping.
- Recon phase: Collection.
- Strengths: Very fast and easy to integrate into attack surface pipelines.
- Weaknesses: Active network scanning can cross policy lines quickly.
- Example usage: `naabu -list hosts.txt -json`
- Safe usage guidance: Use only when the scope explicitly authorizes port scanning.
- Role: Optional and gated.
- Expected output format: JSONL or text.

### nmap

- Purpose: Deep port and service discovery with mature probing capabilities.
- Recon phase: Collection and enrichment.
- Strengths: Rich service detection and broad operator familiarity.
- Weaknesses: Slower and far more active than lighter discovery tools.
- Example usage: `nmap -Pn -sV -iL hosts.txt -oX nmap.xml`
- Safe usage guidance: Only run when explicitly authorized and keep arguments conservative.
- Role: Optional and tightly gated.
- Expected output format: Text, XML, grepable output.

## Design Notes

- Core means ReconPilot should eventually have a first-class adapter for the tool.
- Optional means the tool is useful, but orchestration should remain modular.
- Every tool should feed structured output or be wrapped into structured JSONL during ingestion.
- No tool in this phase should be used for exploitation, credential attacks, or destructive scanning.

## Graph Relationships

ReconPilot Phase 3 turns local recon artifacts into a graph instead of treating each record as isolated text.

Current relationship types include:

- `ResolvesTo`
- `RedirectsTo`
- `UsesTechnology`
- `SharesIp`
- `SharesTitle`
- `References`
- `LoadsScript`
- `ContainsParameter`
- `Hosts`
- `BelongsToCluster`

These relationships are deterministic correlation hints, not proof of risk by themselves.

## Correlation Concepts

Phase 3 correlation is intentionally lightweight but still useful:

- shared IP detection highlights reused infrastructure
- shared title detection groups similar applications
- technology correlation highlights repeated stack choices such as Grafana, Jenkins, Kibana, Next.js, Express, and CloudFront
- redirect mapping captures handoff paths between hosts
- DNS mapping preserves host-to-IP and host-to-CNAME evidence
- JS and parameter linking keeps API and routing hints connected to their source artifacts

The graph engine is local-only. It consumes existing files from `output/` and does not contact targets.

## Screenshot Clustering

Screenshots remain safety-sensitive because they still send requests to target systems and must obey program rules.

Current screenshot handling supports:

- screenshot metadata ingestion
- title reuse as a weak clustering signal
- placeholder notes for future favicon clustering
- placeholder notes for future visual similarity clustering

Image analysis is intentionally not implemented yet.

## Infrastructure Mapping

Infrastructure mapping in ReconPilot combines:

- DNS output from `dnsx`
- host metadata from `httpx`
- technology fingerprints from `WhatWeb`
- screenshot metadata from `gowitness`
- future JavaScript and parameter records

This makes it possible to identify:

- hosts sharing IP space
- repeated admin or dashboard surfaces
- redirect neighborhoods
- production-adjacent systems
- likely internal or staging systems

The graph layer improves prioritization because findings can be scored with surrounding context instead of only with standalone keywords.

## Semantic Enrichment Layer

Phase 4 is not a remote model integration. It is a deterministic local analysis layer over graph outputs.

Current classifier categories:

- environment keywords such as `production`, `staging`, `dev`, `test`, `internal`, and `legacy`
- role keywords such as `login`, `admin`, `dashboard`, `api`, `swagger`, `storage`, and `analytics`
- technology recognition for high-interest stacks such as Jenkins, Grafana, Kibana, Elasticsearch, Prometheus, Kubernetes, Docker, WordPress, Drupal, Next.js, Express, Spring, Laravel, Django, and Rails
- endpoint intent classification for paths such as `/admin`, `/internal`, `/api`, `/graphql`, `/swagger`, `/openapi`, `/debug`, `/export`, `/backup`, `/upload`, `/users`, `/billing`, and `/payments`
- parameter intent classification for names such as `id`, `redirect`, `returnUrl`, `file`, `path`, `token`, `key`, `secret`, and `debug`

## Graph-Neighborhood Reasoning

The semantic layer does not only classify isolated strings. It also reasons over direct graph neighbors:

- shared IP edges
- shared title edges
- technology neighbors
- redirect relationships
- referenced endpoints and parameters
- cluster membership

This supports summaries such as:

- an asset sharing infrastructure with privileged neighbors
- an operational tool candidate inside an admin-like cluster
- a documentation endpoint connected to an API-oriented host

## No Vulnerability Claims

Semantic enrichment is meant for prioritization, not confirmation.

Outputs should say:

- candidate
- interesting
- worth review
- requires validation

Outputs should not say:

- exploited
- vulnerable
- confirmed issue

This distinction matters because deterministic tagging and graph reasoning can highlight promising surfaces without overstating what the data proves.

## Review Workspace

Phase 5 turns semantic enrichment output into an analyst review queue.

Current review outputs include:

- a ranked priority queue in markdown and JSON
- one markdown asset card per asset
- a structured evidence index
- a generic safe review checklist
- a non-technical executive summary

## Ranking Behavior

The review workspace ranks assets with deterministic logic:

- higher risk scores sort first
- assets with multiple semantic roles receive a boost
- authentication, admin, internal, staging, and legacy indicators receive a boost
- graph-neighborhood context such as shared infrastructure, cluster membership, or privileged adjacency receives a boost
- confidence is derived from semantic tag confidence, observation confidence, and source corroboration

This makes the queue suitable for manual triage before any later model-assisted ranking is introduced.

## Evidence-Led Review

Review outputs preserve traceability:

- semantic tag evidence points back to `semantic-assets.json`
- observation evidence points back to `semantic-observations.json`
- risk factors point back to `risk-explanations.json`
- neighborhood summaries point back to `enriched-graph.json`

That traceability matters because analysts need to understand why something was prioritized before deciding whether deeper validation is appropriate.
