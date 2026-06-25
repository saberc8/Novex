# Research Radar Source Connectors Design

## Brief

Upgrade Research Radar from a model-driven web-search POC into an architecture-compliant Novex research source aggregation workflow. The new slice adds a backend Research Radar service that fetches structured evidence from dedicated research sources, then the existing POC UI uses that evidence to start an Agent report run.

The backend owns external source access, normalization, permission checks, and partial-failure handling. The frontend remains a customer-facing workspace and does not call arXiv, GitHub, Hugging Face, Papers with Code, or leaderboard sources directly.

## Current State

- `apps/research-radar-poc` is a functional standalone Next.js POC on port `62607`.
- The app currently calls `/ai/agents/runs` directly with `runtimeMode: "model_loop"` and `webSearchEnabled: true`.
- The backend already exposes Agent APIs under `/ai/agents/runs`.
- The backend architecture places business orchestration in `backend/src/application`, HTTP routes in `backend/src/interfaces/http`, and shared reusable connector vocabulary in `crates/novex-connectors`.
- Existing connector/tool code already has GitHub and web search patterns, but there is no dedicated Research Radar source aggregation service yet.
- The repository guidance says backend owns control plane, HTTP API, and AI orchestration; Next.js apps should consume stable backend APIs.

## Goals

1. Add a backend API for Research Radar scans.
2. Fetch topic evidence separately from arXiv, GitHub, Hugging Face models, Hugging Face datasets, Papers with Code-compatible paper/task sources, and leaderboard sources.
3. Normalize all source results into one evidence item shape the UI and Agent prompt can consume.
4. Return partial results when one provider fails.
5. Keep the first implementation stateless: no new database table, scheduler, or subscription system.
6. Keep frontend source access behind backend APIs.

## Non-Goals

- Scheduled monitoring, alerts, or subscriptions.
- Persistent research projects or history tables.
- Full citation graph, author graph, or institution disambiguation.
- Crawling arbitrary websites beyond configured source endpoints.
- A full connector crate extraction in this slice.
- Replacing the existing Agent report generation flow.

## API Design

Add:

```http
POST /ai/research-radar/scans
```

Permission:

```text
ai:research-radar:scan
```

Request:

```json
{
  "topic": "agent workflow",
  "sources": [
    "arxiv",
    "github",
    "huggingface_models",
    "huggingface_datasets",
    "paperswithcode",
    "leaderboards"
  ],
  "ranking": "balanced",
  "limitPerSource": 5
}
```

Response:

```json
{
  "topic": "agent workflow",
  "ranking": "balanced",
  "status": "partial",
  "sources": [
    {
      "source": "arxiv",
      "status": "succeeded",
      "items": [],
      "warning": null
    }
  ],
  "items": [],
  "promptContext": "Source-grounded evidence for the Agent report...",
  "warnings": []
}
```

Status values:

- `succeeded`: at least one requested source succeeded and no source failed.
- `partial`: at least one requested source succeeded and at least one source failed or degraded.
- `failed`: all requested sources failed.

## Backend Architecture

Create:

```text
backend/src/application/ai/research_radar_service.rs
backend/src/interfaces/http/ai/research_radar.rs
```

Wire them through:

```text
backend/src/application/ai/mod.rs
backend/src/interfaces/http/ai/mod.rs
```

The service is responsible for:

- validating topic, source list, ranking, and limit;
- choosing default sources when the request omits `sources`;
- dispatching source fetches with bounded per-source limits and timeouts;
- normalizing external payloads into `ResearchRadarItem`;
- computing source status and warnings;
- building `promptContext` for downstream Agent report generation.

The route is responsible for:

- auth extraction through existing middleware;
- permission check using `require_permission`;
- request/response JSON shape only;
- no provider-specific parsing.

The first slice keeps provider client functions inside `research_radar_service.rs` as private functions. If this module becomes broadly reusable, move provider-neutral request/parse DTOs into `crates/novex-connectors` in a later refactor.

## Source Contracts

### arXiv

Use the official arXiv API query endpoint:

```text
https://export.arxiv.org/api/query
```

Query shape:

```text
search_query=all:<topic>
start=0
max_results=<limit>
sortBy=submittedDate
sortOrder=descending
```

Parse Atom entries into paper items with title, abstract snippet, authors, published date, updated date, arXiv id, and URL.

### GitHub

Use GitHub REST repository search:

```text
GET /search/repositories
```

Query shape:

```text
q=<topic> language:Python OR language:TypeScript OR language:Rust
sort=stars
order=desc
per_page=<limit>
```

Use `GITHUB_TOKEN` or `NOVEX_GITHUB_TOKEN` when present. Unauthenticated access is allowed but may be rate limited. Normalize full name, description, stars, forks, language, updated date, and URL.

### Hugging Face Models

Use Hugging Face Hub model search over:

```text
https://huggingface.co/api/models
```

Query shape:

```text
search=<topic>
limit=<limit>
sort=likes
direction=-1
```

Use `HUGGINGFACE_TOKEN`, `HF_TOKEN`, or `NOVEX_HUGGINGFACE_TOKEN` when present. Normalize model id, likes, downloads, tags, pipeline tag, updated date, and URL.

### Hugging Face Datasets

Use Hugging Face Hub dataset search over:

```text
https://huggingface.co/api/datasets
```

Query shape:

```text
search=<topic>
limit=<limit>
sort=likes
direction=-1
```

Normalize dataset id, likes, downloads, tags, updated date, and URL.

### Papers With Code-Compatible Source

`paperswithcode.com` currently redirects to Hugging Face Papers. Treat this source as a compatibility slot:

1. Prefer a configured API endpoint from `NOVEX_RESEARCH_RADAR_PWC_ENDPOINT` when set.
2. Otherwise query Hugging Face Papers/trending-compatible sources if available.
3. If no stable endpoint is available, return a degraded source status with a warning and let arXiv/Hugging Face/GitHub still succeed.

The response must not fake Papers with Code results. It either returns normalized live results or an explicit warning.

### Leaderboards

Leaderboards are not one universal API. Use a provider list:

1. Configured JSON endpoint from `NOVEX_RESEARCH_RADAR_LEADERBOARD_ENDPOINTS`, comma-separated.
2. Hugging Face leaderboard/hub metadata when reachable.
3. Degraded warning when no endpoint is configured or reachable.

Normalize leaderboard name, benchmark/task, metric, score when available, updated date, and URL. If a leaderboard endpoint returns generic records, preserve safe fields in `metadata`.

## Normalized Data Model

Backend response types use camelCase JSON:

```rust
pub struct ResearchRadarScanCommand {
    pub topic: String,
    pub sources: Vec<ResearchRadarSource>,
    pub ranking: ResearchRadarRanking,
    pub limit_per_source: Option<u8>,
}

pub struct ResearchRadarScanResp {
    pub topic: String,
    pub ranking: ResearchRadarRanking,
    pub status: ResearchRadarScanStatus,
    pub sources: Vec<ResearchRadarSourceResult>,
    pub items: Vec<ResearchRadarItem>,
    pub prompt_context: String,
    pub warnings: Vec<String>,
}

pub struct ResearchRadarItem {
    pub id: String,
    pub source: ResearchRadarSource,
    pub kind: ResearchRadarItemKind,
    pub title: String,
    pub url: Option<String>,
    pub summary: Option<String>,
    pub authors: Vec<String>,
    pub organization: Option<String>,
    pub published_at: Option<String>,
    pub updated_at: Option<String>,
    pub metrics: Vec<ResearchRadarMetric>,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
}
```

Item kinds:

- `paper`
- `project`
- `model`
- `dataset`
- `benchmark`
- `news`
- `community`

## Ranking

First slice ranking is deterministic and explainable:

- `recency`: newest `publishedAt` or `updatedAt` first.
- `importance`: metrics such as stars, likes, downloads, citations, or scores first.
- `beginner`: prefer items with summaries, README-like descriptions, tutorials, or dataset/model cards when available.
- `balanced`: blend source priority, recency, and metrics.

The exact score is internal. The response should include source metrics so the UI can explain why an item looks important.

## Prompt Context

The service builds a compact source-grounded context block for the Agent:

```text
Research Radar Evidence
Topic: agent workflow
Ranking: balanced

[arxiv] Paper: ...
Authors: ...
Date: ...
URL: ...
Summary: ...

[github] Project: ...
Stars: ...
URL: ...
Summary: ...
```

The existing Agent report prompt will be updated to include this evidence and tell the model:

- prioritize the provided source evidence;
- cite source labels or URLs in prose where useful;
- use web search only to fill gaps;
- do not invent Papers with Code or leaderboard entries when the source status is degraded.

## Frontend Changes

`apps/research-radar-poc` changes:

- Add API helper for `/ai/research-radar/scans`.
- On scan submit, call the Research Radar backend scan first.
- Create the Agent run with the backend `promptContext` embedded in the research prompt.
- Store `sourceScan` on `ResearchScan`.
- Render a compact Source Results area showing each source status, item count, warnings, and top items.
- Keep existing Evidence rail for Agent run events.
- If the source scan is partial, still allow Agent report generation with available evidence and visible warnings.

## Error Handling

- Empty topic returns a 400-style API error through existing `AppError`.
- Unknown source is rejected with a clear message.
- External provider failure is captured per source as `failed` with a warning.
- All-source failure returns `status = failed` and no Agent run should be started by the frontend.
- Partial source failure returns `status = partial`; the frontend proceeds and shows warnings.
- Provider response parse failure is treated as source failure, not process failure.
- Secrets and tokens are never returned in responses or warnings.

## Testing

Backend tests:

- service validates topic and default sources;
- arXiv Atom parser normalizes papers;
- GitHub repository parser normalizes repo metrics;
- Hugging Face model and dataset parsers normalize hub payloads;
- source aggregation returns partial when one provider fails;
- prompt context contains source labels and URLs but not secrets;
- route is registered and requires auth;
- route rejects users without `ai:research-radar:scan`.

Frontend tests:

- scan calls `/ai/research-radar/scans` before `/ai/agents/runs`;
- Agent prompt includes `promptContext`;
- Source Results show per-source counts and warnings;
- all-source failure prevents Agent run and leaves an error visible;
- partial source scan still starts Agent run.

## Verification Plan

Run:

```bash
cargo test -p backend research_radar
cargo test -p backend --test poc_ports
pnpm --dir apps/research-radar-poc test
pnpm --dir apps/research-radar-poc typecheck
pnpm --dir apps/research-radar-poc lint
git diff --check
```

Live smoke:

1. Start backend on `62601`.
2. Start Research Radar POC on `62607`.
3. Submit `agent workflow`.
4. Confirm backend source scan returns at least arXiv/GitHub/Hugging Face results or explicit per-source warnings.
5. Confirm Agent report run uses the source context and reaches a terminal state.

## Acceptance Criteria

1. `POST /ai/research-radar/scans` exists behind auth and `ai:research-radar:scan`.
2. The backend calls source-specific connectors rather than relying only on generic `web.search`.
3. arXiv, GitHub, Hugging Face models, and Hugging Face datasets return normalized items when their public endpoints are reachable.
4. Papers with Code and leaderboard slots return live configured results or explicit degraded warnings without blocking other sources.
5. The frontend displays source status and items separately from the Agent evidence rail.
6. The Agent report prompt includes backend-built source evidence.
7. Partial source failures are visible and do not erase previous scans.
8. Automated tests cover backend normalization, route auth, frontend flow, and prompt context usage.
