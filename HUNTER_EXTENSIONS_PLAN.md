# Hunter Extensions Plan

## Purpose

This document defines the planned Burp Suite Professional extension suite for the Hunter workflow.

The goal is not to build another scanner.
The goal is to build a workflow-oriented hunting system that:

- accelerates early recon
- reduces traffic-review fatigue
- builds understanding of how an application works
- preserves context as testing progresses
- uses Codex CLI for reasoning where reasoning adds value
- keeps active behavior manual and deliberate
- preserves operator trust by staying explainable and low-noise

Core principle:

> Do not think "How do I find XSS?"
>
> Think "How does this application actually work?"

The better the suite helps map:

- trust boundaries
- user flows
- data handling
- application assumptions

the better it supports high-value manual testing.

## Design Rules

- Burp Pro compatible
- Java 17
- Gradle
- Montoya API only
- no automatic attacks
- no automatic vulnerability claims
- no request or response modification unless explicitly user-driven
- Codex used for thinking, not for hot-path passive scoring
- deterministic parsing and aggregation first, AI second
- workflow system, not scanner collection
- optimize for operator trust over alert volume

## Core Architectural Model

The suite should be treated as an application-understanding system.

At its core, it is building an application behavior graph made of:

- route nodes
- entity nodes
- state fingerprints
- transition edges
- trust-boundary edges

This matters because good hunters do not think in:

```text
payload -> response
```

They think in:

```text
actor -> trust boundary -> state transition
```

That model is where high-value bugs usually live:

- IDOR
- tenant escape
- privilege escalation
- workflow abuse
- business logic flaws
- race conditions

This graph model should become the shared backbone for `Hunter Mapper`, `Hunter Diff`, `Hunter Flow`, and eventually `Hunter Analysis 3`.

## Planned Extension Order

1. `Hunter Scope`
2. `Hunter Start 1`
3. `Hunter Mapper`
4. `Hunter View 2`
5. `Hunter Diff`
6. `Hunter Flow`
7. `Hunter Analysis 3`
8. `Hunter Recon Bridge`
9. `Hunter Casefile`

The numbering in Burp tabs should reflect practical workflow order, not implementation order.

## Extension Overview

### 1. Hunter Scope

Status: Planned

Purpose:
- import or record program scope
- store exclusions and forbidden testing methods
- mark hosts and routes as in-scope or out-of-scope
- warn before unsafe or disallowed workflow steps

Why it exists:
- many beginners waste time or create risk by ignoring program rules

Codex role:
- summarize program rules into a short operator briefing
- turn long policy text into actionable reminders

Notes:
- should be mostly deterministic
- should not depend on Codex for enforcement

### 2. Hunter Start 1

Status: Implemented, expanding

Purpose:
- perform first-move discovery for a target or selected request
- run lightweight starter checks such as:
  - `/robots.txt`
  - `/sitemap.xml`
  - `/.well-known/security.txt`
  - common admin-style routes
- queue findings for review and optional Codex analysis

Why it exists:
- these are checks every serious hunter performs early
- they are fast, cheap, and often reveal hidden paths

Codex role:
- summarize which discoveries matter
- rank what to visit next
- explain why a finding is probably high-signal or low-signal

Notes:
- active, but manual
- should stay intentionally lightweight
- should never become a dirbuster

### 3. Hunter Mapper

Status: Planned, high priority

Purpose:
- build a living model of how the application works as traffic comes through Burp
- map:
  - hosts
  - route shapes
  - methods
  - parameters
  - auth states
  - state fingerprints
  - workflows
  - entities and object references
  - object ownership surfaces
  - external callback surfaces
  - admin and internal boundaries

Why it exists:
- this becomes the memory layer underneath the entire suite
- it shifts the workflow from "find bug class" to "understand the app"
- it is the most important component in the planned architecture

Codex role:
- summarize the app structure
- explain visible trust boundaries
- identify likely assumptions to test
- propose next manual investigations based on the evolving map

Duplicate strategy:
- do not store every raw duplicate
- do not ignore duplicates entirely
- aggregate by canonical endpoint shape, for example:
  - host
  - method
  - normalized path shape
  - parameter-name set
  - content type
  - auth bucket

For each canonical record, preserve:
- seen count
- first seen / last seen
- auth variants
- state variants
- status variants
- representative samples
- linked flows
- high-signal observations

Explicit submodels:

#### Entity / Object Model

Hunter Mapper should explicitly model application entities, not just routes.

Examples:
- `user`
- `org`
- `project`
- `invoice`
- `ticket`
- `file`

For each entity type, track:
- identifiers seen
- fields associated with the entity
- endpoints that read it
- endpoints that mutate it
- ownership hints
- parent-child relationships
- reuse of the same identifier type across routes

This is a major force multiplier for:
- IDOR hunting
- tenant escape analysis
- privilege analysis
- cross-workflow reasoning

#### State Fingerprints

Hunter Mapper should also model application state, not just endpoint structure.

Examples:
- anonymous
- authenticated
- admin
- suspended
- premium
- invited
- feature-enabled
- feature-disabled

These state fingerprints should be attached to observed routes, flows, and differences.
This is what makes `Hunter Diff` and `Hunter Flow` much more useful than plain response comparison.

#### Canonicalization Risk

Canonicalization is one of the hardest technical problems in the suite.

Simple examples are easy:

```text
/api/users/123
/api/users/456
```

But many applications collapse multiple logical actions into the same transport-level endpoint:

```text
POST /search
POST /graphql
POST /api/router
```

So Hunter Mapper will eventually need to understand:
- GraphQL operation names
- JSON body structure
- REST-like action grouping inside generic endpoints
- semantic differences between payload shapes
- auth and state context around the same route

If canonicalization is too coarse, unrelated behaviors get merged.
If it is too fine, the graph becomes noisy and fragmented.
This area should be treated as a first-class engineering concern.

### 4. Hunter View 2

Status: Implemented

Purpose:
- triage Burp traffic
- highlight requests worth deeper manual review
- filter noise
- let the operator work from the most promising traffic first

Why it exists:
- Proxy history becomes overwhelming quickly
- scoring and filtering help reduce manual review time

Codex role:
- limited and optional
- Hunter View should stay fast and mostly deterministic

Future direction:
- consume context from Hunter Mapper
- distinguish between:
  - newly discovered routes
  - auth-only routes
  - hidden routes from discovery files
  - workflow-critical actions
  - likely lab or UI chrome noise

### 5. Hunter Diff

Status: Planned

Purpose:
- compare responses and behavior across trust boundaries and state changes
- help answer:
  - what changes between anonymous and authenticated?
  - what changes between user A and user B?
  - what changes before and after an action?
  - what changes across roles or tenants?

Why it exists:
- many auth and logic bugs are easiest to spot through structured comparison
- when connected to Hunter Mapper, this becomes semantic trust-boundary comparison rather than raw text diffing

Codex role:
- explain whether a difference looks meaningful
- propose focused follow-up tests instead of generic fuzzing

Notes:
- should integrate closely with Repeater-style workflows
- should prefer highlighting:
  - authorization-only deltas
  - hidden fields
  - extra object references
  - state-machine changes
  - data exposure differences

### 6. Hunter Flow

Status: Planned

Purpose:
- reconstruct and visualize multi-step workflows
- identify:
  - state-changing sequences
  - approval flows
  - billing flows
  - invitation flows
  - file-processing flows
  - reset and recovery flows

Why it exists:
- high-value bugs often live in sequence abuse, state desync, and bad assumptions
- most tooling is weak at reconstructing real multi-step behavior

Codex role:
- explain what a workflow appears to do
- point out suspicious state transitions
- suggest comparisons and race-condition candidates

Notes:
- this should help with business logic and race-condition hunting more than raw single-request analysis
- this has strong potential for:
  - race-condition hunting
  - approval bypass analysis
  - onboarding abuse
  - invitation flaws
  - checkout desynchronization
  - async-processing bugs

### 7. Hunter Analysis 3

Status: Implemented

Purpose:
- analyze selected HTTP transactions with Codex
- give practical next-step manual testing guidance
- help the operator decide whether a request is worth deeper review

Why it exists:
- individual requests often need fast human-like triage

Codex role:
- primary reasoning engine
- should stay concise, practical, and manual-testing focused

Notes:
- not a scanner
- no vulnerability confirmations
- no exploit spam
- best used on selected high-value traffic, not every request

### 8. Hunter Recon Bridge

Status: Planned

Purpose:
- integrate with or import from external recon tools and datasets
- likely inputs:
  - subdomain enumeration output
  - live host probing output
  - archive URL collections
  - crawler output
  - JavaScript endpoint extraction

Why it exists:
- Burp traffic alone is too narrow for full target understanding
- broader recon should feed the same workflow rather than live in separate notes

Codex role:
- cluster imported results
- identify likely high-value assets
- summarize which hosts or routes deserve attention first

Build policy:
- prefer integration or import over copying entire open-source recon projects

### 9. Hunter Casefile

Status: Planned

Purpose:
- preserve investigation state
- store:
  - findings
  - notes
  - selected requests
  - queue exports
  - Codex analyses
  - comparison results
  - candidate report fragments

Why it exists:
- serious hunting needs continuity, not just live tab state

Codex role:
- compress notes into readable summaries
- help turn evidence into draft writeups

Notes:
- should be evidence-oriented, not hype-oriented

## Codex Usage Strategy

Codex should be used where reasoning matters.
It should not be used for everything.

Good Codex use cases:
- summarize what a host or workflow appears to do
- rank imported or discovered routes
- identify likely trust-boundary tests
- explain differences that matter
- suggest next-step manual tests
- help reduce operator uncertainty

Bad Codex use cases:
- passive hot-path scoring of every request
- replacing parsers
- large-scale crawling
- brute-force route generation
- generic payload spraying

Rule of thumb:

- deterministic logic for collection, normalization, deduplication, and scoring
- Codex for interpretation, prioritization, and concise operator guidance

The long-term moat is not:

> AI finds vulnerabilities automatically

The moat is:

> AI helps humans understand complex applications faster without taking control away from them

## Copy vs Integrate Policy

We will sometimes need functionality that already exists in open-source tools.
The suite should not blindly copy those codebases into Burp.

Preferred order:

1. Build simple deterministic components ourselves if the scope is small.
2. Reuse or adapt tiny permissive-licensed utilities when the maintenance burden is low.
3. Integrate with mature external tools or import their outputs instead of transplanting their engines.

Good candidates for limited reuse:
- sitemap parsing
- robots parsing
- URL normalization helpers
- small extraction utilities

Bad candidates for direct copying:
- full subdomain enumeration engines
- heavy crawlers
- internet-scale probing frameworks
- large toolchains designed to run outside Burp

Before copying any third-party code:
- verify license compatibility
- record source and version
- decide whether we are willing to maintain the copied code ourselves

## Near-Term Build Priorities

Recommended next steps:

1. Build `Hunter Mapper`
2. Feed `Hunter Start 1`, `Hunter View 2`, and `Hunter Analysis 3` into that shared map
3. Add the explicit entity and object model inside `Hunter Mapper`
4. Add state fingerprints before building deeper comparison features
5. Solve canonicalization carefully before adding more AI layers
6. Add `Hunter Diff` once endpoint, entity, and auth context are stable
7. Add `Hunter Recon Bridge` only after the internal model is ready to consume external data

## Summary

The planned Hunter suite should become a workflow system, not a scanner collection.

The intended progression is:

- `Hunter Scope` keeps testing safe and in-scope
- `Hunter Start 1` finds early leads
- `Hunter Mapper` builds understanding
- `Hunter View 2` prioritizes traffic
- `Hunter Diff` and `Hunter Flow` expose trust and logic problems
- `Hunter Analysis 3` adds fast reasoning where human judgment matters
- `Hunter Recon Bridge` broadens discovery
- `Hunter Casefile` preserves and organizes the investigation

If built correctly, the suite should help a hunter spend less time sorting noise and more time understanding the application well enough to find real bugs.

The key architectural bet is simple:

- deterministic systems build the map
- the map preserves context
- Codex helps interpret the map
- the human makes the final testing decisions
