# Research Radar Source Connectors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an architecture-compliant backend Research Radar source aggregation API and update the POC UI so research scans use structured evidence from arXiv, GitHub, Hugging Face, Papers With Code-compatible, and leaderboard sources before starting the Agent report run.

**Architecture:** The backend owns source access, normalization, permission checks, and partial-failure handling. `backend/src/application/ai/research_radar_service.rs` contains the stateless use-case service and provider parsers; `backend/src/interfaces/http/ai/research_radar.rs` exposes the route; `apps/research-radar-poc` calls the backend API and passes `promptContext` into the existing Agent report flow.

**Tech Stack:** Rust, Axum, SQLx migration seeds, reqwest with system proxy, serde/serde_json, Next.js, TypeScript, Tailwind CSS, Vitest, Testing Library.

## Global Constraints

- Backend business orchestration belongs in `backend/src/application`.
- HTTP routes belong in `backend/src/interfaces/http`.
- The frontend must not call arXiv, GitHub, Hugging Face, Papers With Code, or leaderboard endpoints directly.
- The first slice is stateless and must not add research history tables.
- Partial provider failure must return source-level warnings instead of failing the entire scan when at least one source succeeds.
- Secrets and tokens must never be returned in API responses, prompt context, source warnings, or UI text.
- Existing user changes in `apps/codex-app-poc/app/layout.tsx` must remain untouched and unstaged.

---

## File Structure

Create:

- `backend/src/application/ai/research_radar_service.rs`: request/response types, source enum, source dispatch, provider request helpers, provider parsers, deterministic ranking, prompt context builder, and service tests.
- `backend/src/interfaces/http/ai/research_radar.rs`: route registration, permission check, handler tests, and seed migration assertion.
- `backend/migrations/202606250001_seed_research_radar_permission.sql`: AI menu permission seed for `ai:research-radar:scan`.
- `apps/research-radar-poc/src/api/source-scan.ts`: frontend API helper for `/ai/research-radar/scans`.

Modify:

- `backend/src/application/ai/mod.rs`: export `research_radar_service`.
- `backend/src/interfaces/http/ai/mod.rs`: export and merge `research_radar::routes()`.
- `apps/research-radar-poc/src/types/research.ts`: add source scan types and attach `sourceScan` to `ResearchScan`.
- `apps/research-radar-poc/src/api/research.ts`: accept `sourceScan`/`promptContext` when building the Agent prompt.
- `apps/research-radar-poc/src/api/research.test.ts`: assert prompt context is included.
- `apps/research-radar-poc/app/page.test.tsx`: assert source scan happens before Agent run, partial proceeds, all-source failure blocks Agent run.
- `apps/research-radar-poc/src/app-client.tsx`: scan source API first, show source results, pass `promptContext` to Agent command.

---

### Task 1: Backend Research Radar Service Contracts And Parsers

**Files:**
- Create: `backend/src/application/ai/research_radar_service.rs`
- Modify: `backend/src/application/ai/mod.rs`

**Interfaces:**
- Produces: `ResearchRadarService::new() -> ResearchRadarService`
- Produces: `ResearchRadarService::scan(command: ResearchRadarScanCommand) -> impl Future<Output = Result<ResearchRadarScanResp, AppError>>`
- Produces: `ResearchRadarScanCommand`, `ResearchRadarScanResp`, `ResearchRadarSource`, `ResearchRadarSourceResult`, `ResearchRadarItem`, `ResearchRadarMetric`, `ResearchRadarRanking`, `ResearchRadarScanStatus`
- Consumes: `crate::shared::error::AppError`

- [ ] **Step 1: Write failing service and parser tests**

Add `#[cfg(test)]` tests to the new service file:

```rust
#[test]
fn research_radar_defaults_sources_and_limit() {
    let command = ResearchRadarScanCommand {
        topic: " agent workflow ".to_owned(),
        sources: vec![],
        ranking: ResearchRadarRanking::Balanced,
        limit_per_source: None,
    };

    let normalized = normalize_scan_command(command).unwrap();

    assert_eq!(normalized.topic, "agent workflow");
    assert_eq!(normalized.limit_per_source, 5);
    assert_eq!(
        normalized.sources,
        vec![
            ResearchRadarSource::Arxiv,
            ResearchRadarSource::Github,
            ResearchRadarSource::HuggingfaceModels,
            ResearchRadarSource::HuggingfaceDatasets,
            ResearchRadarSource::Paperswithcode,
            ResearchRadarSource::Leaderboards,
        ]
    );
}

#[test]
fn parse_arxiv_atom_normalizes_paper_items() {
    let body = r#"
    <feed xmlns="http://www.w3.org/2005/Atom">
      <entry>
        <id>http://arxiv.org/abs/2401.12345v1</id>
        <updated>2024-01-03T00:00:00Z</updated>
        <published>2024-01-02T00:00:00Z</published>
        <title>Agent Workflow Planning</title>
        <summary>Workflow agents coordinate tools.</summary>
        <author><name>Ada Lovelace</name></author>
        <author><name>Grace Hopper</name></author>
        <link href="http://arxiv.org/abs/2401.12345v1" rel="alternate" type="text/html"/>
      </entry>
    </feed>
    "#;

    let items = parse_arxiv_atom_items(body, 5).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].source, ResearchRadarSource::Arxiv);
    assert_eq!(items[0].kind, ResearchRadarItemKind::Paper);
    assert_eq!(items[0].title, "Agent Workflow Planning");
    assert_eq!(items[0].authors, vec!["Ada Lovelace", "Grace Hopper"]);
    assert_eq!(items[0].published_at.as_deref(), Some("2024-01-02T00:00:00Z"));
    assert_eq!(items[0].url.as_deref(), Some("http://arxiv.org/abs/2401.12345v1"));
}

#[test]
fn parse_github_repositories_normalizes_project_metrics() {
    let payload = serde_json::json!({
        "items": [{
            "full_name": "acme/agent-workflow",
            "html_url": "https://github.com/acme/agent-workflow",
            "description": "Composable agent workflows",
            "stargazers_count": 1200,
            "forks_count": 88,
            "language": "Rust",
            "updated_at": "2026-06-01T00:00:00Z",
            "topics": ["agents", "workflow"]
        }]
    });

    let items = parse_github_repository_items(&payload, 5);

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].source, ResearchRadarSource::Github);
    assert_eq!(items[0].kind, ResearchRadarItemKind::Project);
    assert_eq!(items[0].title, "acme/agent-workflow");
    assert_eq!(items[0].metrics[0].label, "stars");
    assert_eq!(items[0].metrics[0].value, 1200.0);
}

#[test]
fn parse_huggingface_models_and_datasets_normalize_hub_payloads() {
    let models = serde_json::json!([{
        "modelId": "acme/agent-model",
        "likes": 42,
        "downloads": 9001,
        "pipeline_tag": "text-generation",
        "lastModified": "2026-06-02T00:00:00.000Z",
        "tags": ["agents"]
    }]);
    let datasets = serde_json::json!([{
        "id": "acme/agent-dataset",
        "likes": 12,
        "downloads": 300,
        "lastModified": "2026-06-03T00:00:00.000Z",
        "tags": ["benchmark"]
    }]);

    let model_items = parse_huggingface_model_items(&models, 5);
    let dataset_items = parse_huggingface_dataset_items(&datasets, 5);

    assert_eq!(model_items[0].kind, ResearchRadarItemKind::Model);
    assert_eq!(model_items[0].title, "acme/agent-model");
    assert_eq!(dataset_items[0].kind, ResearchRadarItemKind::Dataset);
    assert_eq!(dataset_items[0].title, "acme/agent-dataset");
}

#[tokio::test]
async fn source_aggregation_returns_partial_when_one_provider_fails() {
    let service = ResearchRadarService::with_dispatcher(|source, _topic, _limit| async move {
        match source {
            ResearchRadarSource::Arxiv => Ok(vec![test_item("arxiv-paper", ResearchRadarSource::Arxiv)]),
            ResearchRadarSource::Github => Err("GitHub rate limited".to_owned()),
            _ => Ok(vec![]),
        }
    });

    let resp = service
        .scan(ResearchRadarScanCommand {
            topic: "agent workflow".to_owned(),
            sources: vec![ResearchRadarSource::Arxiv, ResearchRadarSource::Github],
            ranking: ResearchRadarRanking::Balanced,
            limit_per_source: Some(2),
        })
        .await
        .unwrap();

    assert_eq!(resp.status, ResearchRadarScanStatus::Partial);
    assert_eq!(resp.items.len(), 1);
    assert!(resp.warnings.iter().any(|warning| warning.contains("GitHub rate limited")));
    assert!(resp.prompt_context.contains("[arxiv]"));
    assert!(!resp.prompt_context.contains("TOKEN"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p backend research_radar
```

Expected: FAIL because `research_radar_service` and its functions/types do not exist.

- [ ] **Step 3: Implement service contracts, parsers, and dispatch skeleton**

Implement:

```rust
pub const DEFAULT_RESEARCH_RADAR_LIMIT: u8 = 5;
const MAX_RESEARCH_RADAR_LIMIT: u8 = 10;
const RESEARCH_RADAR_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchRadarSource {
    Arxiv,
    Github,
    HuggingfaceModels,
    HuggingfaceDatasets,
    Paperswithcode,
    Leaderboards,
}
```

Implementation rules:

- trim and validate topic;
- default empty `sources` to all six sources;
- clamp `limitPerSource` to `1..=10`;
- keep provider HTTP helpers private;
- build reqwest clients with `.timeout(RESEARCH_RADAR_TIMEOUT)` and `.user_agent("novex-research-radar-poc")`;
- use optional bearer tokens from `GITHUB_TOKEN`, `NOVEX_GITHUB_TOKEN`, `HUGGINGFACE_TOKEN`, `HF_TOKEN`, and `NOVEX_HUGGINGFACE_TOKEN`;
- parse arXiv Atom with small tag extraction helpers to avoid adding a new XML dependency in this slice;
- parse GitHub and Hugging Face JSON with serde_json;
- return degraded warnings for Papers With Code and leaderboards when no configured/working endpoint exists;
- build prompt context from normalized items only.

Export module in `backend/src/application/ai/mod.rs`:

```rust
pub mod research_radar_service;
```

- [ ] **Step 4: Run service tests to verify they pass**

Run:

```bash
cargo test -p backend research_radar
```

Expected: PASS for service/parser tests.

- [ ] **Step 5: Commit Task 1**

```bash
git add backend/src/application/ai/mod.rs backend/src/application/ai/research_radar_service.rs
git commit -m "feat: add research radar source service"
```

---

### Task 2: Backend HTTP Route And Permission Seed

**Files:**
- Create: `backend/src/interfaces/http/ai/research_radar.rs`
- Create: `backend/migrations/202606250001_seed_research_radar_permission.sql`
- Modify: `backend/src/interfaces/http/ai/mod.rs`

**Interfaces:**
- Consumes: `ResearchRadarService::scan(command)`
- Produces: `POST /ai/research-radar/scans`
- Produces permission code: `ai:research-radar:scan`

- [ ] **Step 1: Write failing route and permission tests**

Add tests in `backend/src/interfaces/http/ai/research_radar.rs`:

```rust
#[tokio::test]
async fn research_radar_scan_handler_rejects_missing_permission() {
    let err = scan(
        user_with_permissions(vec![]),
        Json(ResearchRadarScanCommand {
            topic: "agent workflow".to_owned(),
            sources: vec![ResearchRadarSource::Arxiv],
            ranking: ResearchRadarRanking::Balanced,
            limit_per_source: Some(1),
        }),
    )
    .await
    .unwrap_err();

    assert!(matches!(err, AppError::Forbidden));
}

#[tokio::test]
async fn research_radar_scan_handler_runs_with_permission() {
    let response = scan(
        user_with_permissions(vec!["ai:research-radar:scan"]),
        Json(ResearchRadarScanCommand {
            topic: "agent workflow".to_owned(),
            sources: vec![ResearchRadarSource::Paperswithcode],
            ranking: ResearchRadarRanking::Balanced,
            limit_per_source: Some(1),
        }),
    )
    .await
    .unwrap();

    assert_eq!(response.0.code, "200");
    assert_eq!(response.0.data.topic, "agent workflow");
}

#[tokio::test]
async fn research_radar_route_is_registered_and_requires_auth() {
    let db = PgPoolOptions::new()
        .connect_lazy("postgres://postgres:postgres@localhost:5432/avalon_admin")
        .unwrap();
    let jwt = JwtService::new("test-secret".to_owned(), 24);
    let app = build_router(db, &["http://localhost:62602".to_owned()], jwt).unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ai/research-radar/scans")
                .method("POST")
                .header(header::ACCEPT, "application/json")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"topic":"agent workflow"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice::<Value>(&body).unwrap();
    assert_eq!(body["code"], "401");
}

#[test]
fn research_radar_permission_seed_contains_scan_permission() {
    let seed = include_str!("../../../../migrations/202606250001_seed_research_radar_permission.sql");

    assert!(seed.contains("ai:research-radar:scan"));
    assert!(seed.contains("Research Radar"));
    assert!(seed.contains("ON CONFLICT DO NOTHING"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p backend research_radar
```

Expected: FAIL because route module and permission seed do not exist.

- [ ] **Step 3: Implement route and seed**

Route:

```rust
use axum::{routing::post, Json, Router};

use crate::{
    application::ai::research_radar_service::{
        ResearchRadarScanCommand, ResearchRadarScanResp, ResearchRadarService,
    },
    domain::auth::model::CurrentUser,
    interfaces::http::{middleware::permission::require_permission, AppState},
    shared::{error::AppError, response::ApiResponse},
};

pub const RESEARCH_RADAR_SCAN_PERMISSION: &str = "ai:research-radar:scan";

pub fn routes() -> Router<AppState> {
    Router::new().route("/ai/research-radar/scans", post(scan))
}

async fn scan(
    current_user: CurrentUser,
    Json(command): Json<ResearchRadarScanCommand>,
) -> Result<Json<ApiResponse<ResearchRadarScanResp>>, AppError> {
    require_permission(&current_user, RESEARCH_RADAR_SCAN_PERMISSION)?;
    Ok(Json(ApiResponse::ok(ResearchRadarService::new().scan(command).await?)))
}
```

Module wiring:

```rust
pub mod research_radar;
```

and merge:

```rust
.merge(research_radar::routes())
```

Migration:

```sql
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3150, 'Research Radar', 3000, 2, '/ai/research-radar', 'AiResearchRadar', 'ai/research-radar/index', NULL, 'radar', FALSE, FALSE, FALSE, NULL, 13, 1, 1, NOW()),
    (3151, '扫描', 3150, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:research-radar:scan', 1, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
SELECT 1, id
FROM sys_menu
WHERE id BETWEEN 3150 AND 3151
ON CONFLICT DO NOTHING;
```

- [ ] **Step 4: Run route tests to verify they pass**

Run:

```bash
cargo test -p backend research_radar
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add backend/src/interfaces/http/ai/mod.rs backend/src/interfaces/http/ai/research_radar.rs backend/migrations/202606250001_seed_research_radar_permission.sql
git commit -m "feat: expose research radar source scan api"
```

---

### Task 3: Frontend Source Scan API And Agent Prompt Context

**Files:**
- Create: `apps/research-radar-poc/src/api/source-scan.ts`
- Modify: `apps/research-radar-poc/src/types/research.ts`
- Modify: `apps/research-radar-poc/src/api/research.ts`
- Modify: `apps/research-radar-poc/src/api/research.test.ts`

**Interfaces:**
- Produces: `createResearchRadarSourceScan(input: ResearchSourceScanInput): Promise<ResearchSourceScanResp>`
- Produces: `sourceScan?: ResearchSourceScanResp | null` on `ResearchScan`
- Consumes: `sourceScan.promptContext` in `buildResearchRadarAgentRunCommand`

- [ ] **Step 1: Write failing frontend API tests**

In `apps/research-radar-poc/src/api/research.test.ts`, add:

```ts
it("includes backend source evidence in the Agent prompt", () => {
  const command = buildResearchRadarAgentRunCommand({
    topic: "agent workflow",
    filters: ["papers", "projects"],
    ranking: "balanced",
    routeId: "runtime.llm",
    sourceScan: {
      topic: "agent workflow",
      ranking: "balanced",
      status: "partial",
      warnings: ["leaderboards unavailable"],
      promptContext: "Research Radar Evidence\n[arxiv] Paper: Agent Workflow Planning",
      sources: [],
      items: []
    }
  });

  expect(command.input).toContain("Research Radar Evidence");
  expect(command.input).toContain("[arxiv] Paper: Agent Workflow Planning");
  expect(command.input).toContain("Use the provided backend source evidence first");
});
```

Create `apps/research-radar-poc/src/api/source-scan.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import { createResearchRadarSourceScan } from "./source-scan";

describe("research radar source scan api", () => {
  it("posts selected source filters to the backend scan endpoint", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: {
          topic: "agent workflow",
          ranking: "balanced",
          status: "succeeded",
          sources: [],
          items: [],
          promptContext: "Research Radar Evidence",
          warnings: []
        }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);
    vi.stubEnv("NEXT_PUBLIC_API_BASE_URL", "http://localhost:62601");

    await createResearchRadarSourceScan({
      topic: "agent workflow",
      filters: ["papers", "projects", "datasets", "benchmarks"],
      ranking: "balanced"
    });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:62601/ai/research-radar/scans",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          topic: "agent workflow",
          ranking: "balanced",
          limitPerSource: 5,
          sources: [
            "arxiv",
            "github",
            "huggingface_models",
            "huggingface_datasets",
            "paperswithcode",
            "leaderboards"
          ]
        })
      })
    );
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/api/research.test.ts src/api/source-scan.test.ts
```

Expected: FAIL because source scan helper and types do not exist.

- [ ] **Step 3: Implement frontend types and API helper**

Add source types:

```ts
export type ResearchSource =
  | "arxiv"
  | "github"
  | "huggingface_models"
  | "huggingface_datasets"
  | "paperswithcode"
  | "leaderboards";

export type ResearchSourceStatus = "succeeded" | "partial" | "failed" | "degraded";
export type ResearchScanStatus = "succeeded" | "partial" | "failed";
```

Map filters to sources:

```ts
export function researchSourcesForFilters(filters: ResearchFilter[]): ResearchSource[] {
  const sources = new Set<ResearchSource>();
  if (filters.includes("papers")) {
    sources.add("arxiv");
    sources.add("paperswithcode");
  }
  if (filters.includes("projects")) {
    sources.add("github");
    sources.add("huggingface_models");
  }
  if (filters.includes("datasets")) {
    sources.add("huggingface_datasets");
  }
  if (filters.includes("benchmarks")) {
    sources.add("leaderboards");
    sources.add("paperswithcode");
  }
  if (sources.size === 0) {
    ["arxiv", "github", "huggingface_models", "huggingface_datasets", "paperswithcode", "leaderboards"].forEach((source) =>
      sources.add(source as ResearchSource)
    );
  }
  return [...sources];
}
```

`source-scan.ts`:

```ts
import { apiRequest } from "@/lib/api";
import type { ResearchSourceScanInput, ResearchSourceScanResp } from "@/types/research";

export function createResearchRadarSourceScan(input: ResearchSourceScanInput) {
  return apiRequest<ResearchSourceScanResp>("/ai/research-radar/scans", {
    method: "POST",
    body: JSON.stringify({
      topic: input.topic,
      ranking: input.ranking,
      limitPerSource: 5,
      sources: researchSourcesForFilters(input.filters)
    })
  });
}
```

Update `buildResearchRadarPrompt` to include:

```ts
const evidence = input.sourceScan?.promptContext?.trim();
...
evidence
  ? [
      "Use the provided backend source evidence first. Use web search only to fill gaps or verify stale coverage.",
      evidence
    ].join("\n")
  : "No backend source evidence was provided. Use web search when useful."
```

- [ ] **Step 4: Run frontend API tests to verify they pass**

Run:

```bash
pnpm --dir apps/research-radar-poc test src/api/research.test.ts src/api/source-scan.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add apps/research-radar-poc/src/types/research.ts apps/research-radar-poc/src/api/research.ts apps/research-radar-poc/src/api/research.test.ts apps/research-radar-poc/src/api/source-scan.ts apps/research-radar-poc/src/api/source-scan.test.ts
git commit -m "feat: add research radar source scan client"
```

---

### Task 4: Frontend Scan Flow And Source Results UI

**Files:**
- Modify: `apps/research-radar-poc/src/app-client.tsx`
- Modify: `apps/research-radar-poc/app/page.test.tsx`

**Interfaces:**
- Consumes: `createResearchRadarSourceScan`
- Consumes: `ResearchScan.sourceScan`
- Produces: source result summary cards in `ReportWorkspace`

- [ ] **Step 1: Write failing UI flow tests**

Update `apps/research-radar-poc/app/page.test.tsx`:

```ts
it("runs backend source scan before creating the Agent run", async () => {
  const calls: string[] = [];
  const fetchMock = vi.fn(async (url: string) => {
    const href = String(url);
    calls.push(href);
    if (href.includes("/ai/research-radar/scans")) {
      return {
        ok: true,
        json: async () => ({
          code: "200",
          data: {
            topic: "AI coding agents",
            ranking: "balanced",
            status: "partial",
            warnings: ["leaderboards unavailable"],
            promptContext: "Research Radar Evidence\n[github] Project: acme/agent",
            sources: [
              {
                source: "github",
                status: "succeeded",
                warning: null,
                items: [
                  {
                    id: "github:acme/agent",
                    source: "github",
                    kind: "project",
                    title: "acme/agent",
                    url: "https://github.com/acme/agent",
                    summary: "Agent workflows",
                    authors: [],
                    organization: null,
                    publishedAt: null,
                    updatedAt: "2026-06-01T00:00:00Z",
                    metrics: [{ label: "stars", value: 1200 }],
                    tags: ["agents"],
                    metadata: {}
                  }
                ]
              },
              {
                source: "leaderboards",
                status: "failed",
                warning: "leaderboards unavailable",
                items: []
              }
            ],
            items: []
          }
        })
      };
    }
    if (href.includes("/ai/agents/runs") && !href.includes("/events")) {
      return {
        ok: true,
        json: async () => ({
          code: "200",
          data: {
            runId: 91,
            traceId: "agent-91",
            status: "succeeded",
            finalOutput: "## Research Overview\nReport"
          }
        })
      };
    }
    if (href.includes("/events")) {
      return {
        ok: true,
        json: async () => ({ code: "200", data: { list: [], total: 0 } })
      };
    }
    return { ok: true, json: async () => ({ code: "200", data: {} }) };
  });
  vi.stubGlobal("fetch", fetchMock);

  render(<Page />);

  fireEvent.change(screen.getByLabelText("研究主题"), {
    target: { value: "AI coding agents" }
  });
  fireEvent.click(screen.getByRole("button", { name: "启动雷达扫描" }));

  expect(await screen.findByText("Source Results")).toBeTruthy();
  expect(await screen.findByText("acme/agent")).toBeTruthy();
  expect(await screen.findByText("leaderboards unavailable")).toBeTruthy();
  expect(calls.findIndex((url) => url.includes("/ai/research-radar/scans"))).toBeLessThan(
    calls.findIndex((url) => url.includes("/ai/agents/runs"))
  );

  const runCall = fetchMock.mock.calls.find(([url]) =>
    String(url).includes("/ai/agents/runs") && !String(url).includes("/events")
  ) as unknown as [string, RequestInit];
  expect(String(runCall[1].body)).toContain("Research Radar Evidence");
});

it("does not create an Agent run when all source scans fail", async () => {
  const fetchMock = vi.fn(async (url: string) => {
    const href = String(url);
    if (href.includes("/ai/research-radar/scans")) {
      return {
        ok: true,
        json: async () => ({
          code: "200",
          data: {
            topic: "AI coding agents",
            ranking: "balanced",
            status: "failed",
            warnings: ["all sources failed"],
            promptContext: "",
            sources: [],
            items: []
          }
        })
      };
    }
    return { ok: true, json: async () => ({ code: "200", data: {} }) };
  });
  vi.stubGlobal("fetch", fetchMock);

  render(<Page />);

  fireEvent.change(screen.getByLabelText("研究主题"), {
    target: { value: "AI coding agents" }
  });
  fireEvent.click(screen.getByRole("button", { name: "启动雷达扫描" }));

  expect(await screen.findByText("all sources failed")).toBeTruthy();
  expect(fetchMock.mock.calls.some(([url]) => String(url).includes("/ai/agents/runs"))).toBe(false);
});
```

- [ ] **Step 2: Run UI tests to verify they fail**

Run:

```bash
pnpm --dir apps/research-radar-poc test app/page.test.tsx
```

Expected: FAIL because UI does not call source scan or render Source Results.

- [ ] **Step 3: Implement scan flow and Source Results UI**

Implementation details:

- Import `createResearchRadarSourceScan`.
- In `handleSubmit`, call `createResearchRadarSourceScan` after creating the local pending scan and before `createResearchRadarRun`.
- Immediately `updateScan(scanId, { sourceScan })` after the source API returns.
- If `sourceScan.status === "failed"`, set `runError` to `sourceScan.warnings.join("\n") || "研究来源扫描失败"` and return without creating the Agent run.
- Pass `sourceScan` into `createResearchRadarRun`.
- Add `SourceResults` inside `ReportWorkspace` before report section cards.
- Render source status, item count, warning text, and at most three top items per source.
- Keep compact cards with `rounded-[8px]`, stable layout, no nested cards.

- [ ] **Step 4: Run UI tests to verify they pass**

Run:

```bash
pnpm --dir apps/research-radar-poc test app/page.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit Task 4**

```bash
git add apps/research-radar-poc/src/app-client.tsx apps/research-radar-poc/app/page.test.tsx
git commit -m "feat: show research radar source results"
```

---

### Task 5: Verification And Live Smoke

**Files:**
- No planned code files.
- Possible cleanup: generated `apps/research-radar-poc/next-env.d.ts` only if build tooling changes it.

**Interfaces:**
- Verifies all prior tasks.

- [ ] **Step 1: Run backend focused tests**

Run:

```bash
cargo test -p backend research_radar
```

Expected: PASS.

- [ ] **Step 2: Run frontend tests**

Run:

```bash
pnpm --dir apps/research-radar-poc test
```

Expected: all Research Radar POC tests pass.

- [ ] **Step 3: Run frontend typecheck and lint**

Run:

```bash
pnpm --dir apps/research-radar-poc typecheck
pnpm --dir apps/research-radar-poc lint
```

Expected: both exit 0.

- [ ] **Step 4: Run backend port contract and whitespace checks**

Run:

```bash
cargo test -p backend --test poc_ports
git diff --check
```

Expected: both exit 0.

- [ ] **Step 5: Run live smoke against local services**

Commands:

```bash
curl -fsS http://localhost:62601/ready
curl -fsS -X POST http://localhost:62601/ai/research-radar/scans \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <dev token>" \
  -d '{"topic":"agent workflow","sources":["arxiv","github","huggingface_models","huggingface_datasets","paperswithcode","leaderboards"],"ranking":"balanced","limitPerSource":2}'
```

Browser smoke:

- Open `http://localhost:62607`.
- Submit `agent workflow`.
- Confirm Source Results shows per-source statuses and at least one real item or explicit warning.
- Confirm Agent report run starts when the source status is `succeeded` or `partial`.

- [ ] **Step 6: Commit any verification cleanup**

If verification required code cleanup:

```bash
git add <changed-files>
git commit -m "fix: verify research radar source connectors"
```

If no cleanup was required, do not create an empty commit.

---

## Self-Review

- Spec coverage: backend API, source-specific providers, partial failure, prompt context, frontend source results, permission seed, and tests are covered by Tasks 1-5.
- Placeholder scan: no task contains unresolved placeholder markers or deferred implementation steps required for this slice.
- Type consistency: backend uses `ResearchRadar*`; frontend uses `ResearchSource*` for API-facing source scan types and passes `sourceScan` into the existing `ResearchScanInput`.
