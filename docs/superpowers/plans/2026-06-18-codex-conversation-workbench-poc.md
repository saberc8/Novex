# Codex Conversation Workbench POC Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn `apps/codex-app-poc` into a real Codex-style conversation workbench that can chat through the configured model route, upload and parse files, pass typed file/skill/MCP/web-search context into the agent loop, and render readable run evidence.

**Architecture:** Keep the existing Codex-like POC app as the product entry point. Add a typed `workbenchContext` contract shared by frontend and backend, thread the normalized context into model-loop metadata and prompts, and expose only thin API clients for knowledge, capabilities, and workbench composition. Backend changes stay narrow: context normalization, prompt/context plumbing, and a low-risk `web.search` POC tool with an honest dry-run response when no provider is configured.

**Tech Stack:** Next.js app under `apps/codex-app-poc`, Vitest + Testing Library, Rust backend service under `backend/src/application/ai`, `novex-tools` tool definitions, existing knowledge/capability/MCP HTTP APIs, PostgreSQL-backed repositories already present in Novex.

## Global Constraints

- Use the existing `apps/codex-app-poc` app; no new app is created.
- The default POC dataset name is exactly `Codex Workbench Inbox`.
- Direct conversation sends use `runtimeMode: "model_loop"`.
- `modelRouteId` comes from `NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID` when configured.
- `WorkbenchContext` has shape `{ mode: "agent"; datasetId?: number; documentIds: number[]; fileIds: number[]; skillCodes: string[]; mcpToolCodes: string[]; webSearchEnabled: boolean; routeId?: string }`.
- The backend persists bounded, safe workbench context metadata with each run.
- `rag.search` is biased toward selected `datasetId` through prompt/context, not by mutating user text.
- Skills are selected as context codes; skill authoring/import UI stays outside this slice.
- MCP uses existing registered/discovered HTTP capability paths; stdio MCP process execution stays outside this slice.
- Web search uses a low-risk `web.search` tool; when no provider is configured it returns an explicit dry-run response.
- The first viewport remains a conversation workbench, not an admin dashboard.
- Raw JSON remains available only behind a developer/details surface; the default event view is readable.
- Verification commands: `pnpm --dir apps/codex-app-poc test`, `pnpm --dir apps/codex-app-poc typecheck`, `pnpm --dir apps/codex-app-poc lint`, focused Rust tests for changed backend/tool behavior, `cargo fmt --all -- --check`, `git diff --check`.

---

## File Structure

- Modify `backend/src/application/ai/agent_service.rs`
  - Owns `AgentRunCommand`, context normalization, run payload metadata, and model-loop prompt construction.
- Modify `backend/src/application/ai/agent_tool_executor.rs`
  - Owns runtime execution selection for built-in/connector/MCP tools and the new `web.search` dry-run executor.
- Modify `crates/novex-tools/src/lib.rs`
  - Owns `web.search` tool definition and executor binding.
- Modify `apps/codex-app-poc/src/types/agent.ts`
  - Adds `WorkbenchContext` and extends `AgentRunCommand`.
- Create `apps/codex-app-poc/src/types/knowledge.ts`
  - Copies the knowledge DTOs required by the POC from `apps/chat-web` without importing across apps.
- Create `apps/codex-app-poc/src/types/capability.ts`
  - Defines skill/MCP capability DTOs used by the workbench.
- Modify `apps/codex-app-poc/src/lib/api.ts`
  - Adds `apiFormRequest<T>` for file upload while keeping existing JSON behavior stable.
- Create `apps/codex-app-poc/src/api/knowledge.ts`
  - Adds dataset list/create, file upload, and parse-job polling clients.
- Create `apps/codex-app-poc/src/api/capability.ts`
  - Adds skills, MCP server, and MCP tools clients.
- Create `apps/codex-app-poc/src/api/workbench.ts`
  - Owns workbench command composition and default dataset selection helper.
- Create `apps/codex-app-poc/src/lib/workbench-events.ts`
  - Converts raw agent events into readable event evidence rows.
- Modify `apps/codex-app-poc/src/api/agent.ts`
  - Accepts `WorkbenchContext` in configured model runs.
- Modify `apps/codex-app-poc/src/api/agent.test.ts`
  - Covers request payload shape with `workbenchContext`.
- Create `apps/codex-app-poc/src/api/knowledge.test.ts`
  - Covers knowledge API URLs, JSON envelopes, and multipart upload.
- Create `apps/codex-app-poc/src/api/capability.test.ts`
  - Covers skills and MCP capability client URLs.
- Create `apps/codex-app-poc/src/api/workbench.test.ts`
  - Covers command composition and default dataset selection.
- Create `apps/codex-app-poc/src/lib/workbench-events.test.ts`
  - Covers model delta, tool, retrieval, MCP, web-search, terminal, and raw fallback summaries.
- Modify `apps/codex-app-poc/src/app-client.tsx`
  - Upgrades the page shell into a usable conversation workbench with context drawer controls.
- Modify `apps/codex-app-poc/app/page.test.tsx`
  - Covers direct chat, context drawer controls, file upload state, selected chips, web-search toggle, and readable run evidence.
- Modify `docs/plans/2026-06-16-codex-migration-matrix.md`
  - Records this POC as the product-facing validation layer for the agent foundation.

---

### Task 1: Backend Workbench Context Contract

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Interfaces:**
- Produces: `AgentWorkbenchContext`
- Produces: `normalize_agent_workbench_context(context: Option<AgentWorkbenchContext>) -> Option<AgentWorkbenchContext>`
- Produces: `build_model_loop_system_prompt_with_context(tool_codes: &[String], context: Option<&AgentWorkbenchContext>) -> String`
- Consumes: existing `AgentRunCommand`, `agent_run_command_payload`, `record_model_loop_input_event`, and `build_model_loop_messages_from_history`

- [ ] **Step 1: Write failing backend context tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `backend/src/application/ai/agent_service.rs`:

```rust
#[test]
fn workbench_context_normalization_bounds_lists_and_trims_values() {
    let context = AgentWorkbenchContext {
        mode: " agent ".to_owned(),
        dataset_id: Some(42),
        document_ids: vec![1, 2, 2, 0, -5, 3],
        file_ids: vec![9, 9, 0, 10],
        skill_codes: vec![
            " support.refund ".to_owned(),
            "".to_owned(),
            "support.refund".to_owned(),
            " knowledge.writer ".to_owned(),
        ],
        mcp_tool_codes: (0..24).map(|index| format!(" mcp.docs.search.{index} ")).collect(),
        web_search_enabled: true,
        route_id: Some(" runtime.llm.code_agent ".to_owned()),
    };

    let normalized = normalize_agent_workbench_context(Some(context)).expect("context present");

    assert_eq!(normalized.mode, "agent");
    assert_eq!(normalized.dataset_id, Some(42));
    assert_eq!(normalized.document_ids, vec![1, 2, 3]);
    assert_eq!(normalized.file_ids, vec![9, 10]);
    assert_eq!(
        normalized.skill_codes,
        vec!["support.refund".to_owned(), "knowledge.writer".to_owned()]
    );
    assert_eq!(normalized.mcp_tool_codes.len(), 16);
    assert_eq!(normalized.mcp_tool_codes[0], "mcp.docs.search.0");
    assert!(normalized.web_search_enabled);
    assert_eq!(normalized.route_id.as_deref(), Some("runtime.llm.code_agent"));
}

#[test]
fn workbench_context_normalization_drops_empty_context() {
    let context = AgentWorkbenchContext::default();

    assert_eq!(normalize_agent_workbench_context(Some(context)), None);
}

#[test]
fn agent_run_command_payload_preserves_workbench_context() {
    let command = AgentRunCommand {
        input: "What changed in the handbook?".to_owned(),
        runtime_mode: Some("model_loop".to_owned()),
        workbench_context: normalize_agent_workbench_context(Some(AgentWorkbenchContext {
            mode: "agent".to_owned(),
            dataset_id: Some(7),
            document_ids: vec![11],
            file_ids: vec![19],
            skill_codes: vec!["support.refund".to_owned()],
            mcp_tool_codes: vec!["mcp.docs.search".to_owned()],
            web_search_enabled: true,
            route_id: Some("runtime.llm.code_agent".to_owned()),
        })),
        ..AgentRunCommand::default()
    };

    let payload = agent_run_command_payload(&command);

    assert_eq!(payload["workbenchContext"]["mode"], "agent");
    assert_eq!(payload["workbenchContext"]["datasetId"], 7);
    assert_eq!(payload["workbenchContext"]["documentIds"], json!([11]));
    assert_eq!(payload["workbenchContext"]["fileIds"], json!([19]));
    assert_eq!(payload["workbenchContext"]["skillCodes"], json!(["support.refund"]));
    assert_eq!(payload["workbenchContext"]["mcpToolCodes"], json!(["mcp.docs.search"]));
    assert_eq!(payload["workbenchContext"]["webSearchEnabled"], true);
    assert_eq!(payload["workbenchContext"]["routeId"], "runtime.llm.code_agent");
}

#[test]
fn model_loop_system_prompt_includes_workbench_context_without_user_text_mutation() {
    let context = normalize_agent_workbench_context(Some(AgentWorkbenchContext {
        mode: "agent".to_owned(),
        dataset_id: Some(7),
        document_ids: vec![11, 12],
        file_ids: vec![19],
        skill_codes: vec!["support.refund".to_owned()],
        mcp_tool_codes: vec!["mcp.docs.search".to_owned()],
        web_search_enabled: true,
        route_id: Some("runtime.llm.code_agent".to_owned()),
    }));

    let prompt = build_model_loop_system_prompt_with_context(
        &["rag.search".to_owned(), "web.search".to_owned(), "mcp.docs.search".to_owned()],
        context.as_ref(),
    );

    assert!(prompt.contains("Workbench context:"));
    assert!(prompt.contains("Use rag.search with datasetId 7"));
    assert!(prompt.contains("Selected skill codes: support.refund"));
    assert!(prompt.contains("Selected MCP tool codes: mcp.docs.search"));
    assert!(prompt.contains("Web search is enabled; web.search may be used"));
    assert!(!prompt.contains("What changed in the handbook?"));
}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
cargo test -p backend-rust workbench_context_normalization_bounds_lists_and_trims_values -- --nocapture
cargo test -p backend-rust agent_run_command_payload_preserves_workbench_context -- --nocapture
cargo test -p backend-rust model_loop_system_prompt_includes_workbench_context_without_user_text_mutation -- --nocapture
```

Expected: FAIL because `AgentWorkbenchContext`, `workbench_context`, and `build_model_loop_system_prompt_with_context` do not exist.

- [ ] **Step 3: Add context types and normalization**

Add near `AgentRunCommand`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWorkbenchContext {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub dataset_id: Option<i64>,
    #[serde(default)]
    pub document_ids: Vec<i64>,
    #[serde(default)]
    pub file_ids: Vec<i64>,
    #[serde(default)]
    pub skill_codes: Vec<String>,
    #[serde(default)]
    pub mcp_tool_codes: Vec<String>,
    #[serde(default)]
    pub web_search_enabled: bool,
    #[serde(default)]
    pub route_id: Option<String>,
}
```

Add to `AgentRunCommand`:

```rust
#[serde(default)]
pub workbench_context: Option<AgentWorkbenchContext>,
```

Add helpers close to `agent_run_command_payload`:

```rust
const WORKBENCH_CONTEXT_MAX_IDS: usize = 16;
const WORKBENCH_CONTEXT_MAX_CODES: usize = 16;

fn normalize_agent_workbench_context(
    context: Option<AgentWorkbenchContext>,
) -> Option<AgentWorkbenchContext> {
    let mut context = context?;
    let mode = context.mode.trim();
    context.mode = if mode.is_empty() {
        "agent".to_owned()
    } else {
        mode.to_owned()
    };
    context.document_ids = normalized_positive_i64_list(context.document_ids, WORKBENCH_CONTEXT_MAX_IDS);
    context.file_ids = normalized_positive_i64_list(context.file_ids, WORKBENCH_CONTEXT_MAX_IDS);
    context.skill_codes = normalized_code_list(context.skill_codes, WORKBENCH_CONTEXT_MAX_CODES);
    context.mcp_tool_codes = normalized_code_list(context.mcp_tool_codes, WORKBENCH_CONTEXT_MAX_CODES);
    context.route_id = context
        .route_id
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());

    let has_context = context.dataset_id.is_some()
        || !context.document_ids.is_empty()
        || !context.file_ids.is_empty()
        || !context.skill_codes.is_empty()
        || !context.mcp_tool_codes.is_empty()
        || context.web_search_enabled
        || context.route_id.is_some();

    has_context.then_some(context)
}

fn normalized_positive_i64_list(values: Vec<i64>, limit: usize) -> Vec<i64> {
    let mut seen = std::collections::BTreeSet::new();
    values
        .into_iter()
        .filter(|value| *value > 0)
        .filter(|value| seen.insert(*value))
        .take(limit)
        .collect()
}

fn normalized_code_list(values: Vec<String>, limit: usize) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .take(limit)
        .collect()
}
```

- [ ] **Step 4: Thread normalized context into payload, events, and prompt**

Update `normalize_agent_run_command(command)` so the returned command has:

```rust
workbench_context: normalize_agent_workbench_context(command.workbench_context),
```

Update `agent_run_command_payload`:

```rust
"workbenchContext": command.workbench_context,
```

Update `record_model_loop_input_event`:

```rust
object.insert(
    "workbenchContext".to_owned(),
    json!(&command.workbench_context),
);
```

Add prompt helper:

```rust
fn build_model_loop_system_prompt_with_context(
    tool_codes: &[String],
    context: Option<&AgentWorkbenchContext>,
) -> String {
    let mut prompt = build_model_loop_system_prompt(tool_codes);
    if let Some(context) = context {
        let mut lines = Vec::new();
        lines.push("Workbench context:".to_owned());
        if let Some(dataset_id) = context.dataset_id {
            lines.push(format!("Use rag.search with datasetId {dataset_id} for file-grounded questions."));
        }
        if !context.document_ids.is_empty() {
            lines.push(format!("Selected document ids: {}.", join_i64_values(&context.document_ids)));
        }
        if !context.file_ids.is_empty() {
            lines.push(format!("Selected file ids: {}.", join_i64_values(&context.file_ids)));
        }
        if !context.skill_codes.is_empty() {
            lines.push(format!("Selected skill codes: {}.", context.skill_codes.join(", ")));
        }
        if !context.mcp_tool_codes.is_empty() {
            lines.push(format!("Selected MCP tool codes: {}.", context.mcp_tool_codes.join(", ")));
        }
        if context.web_search_enabled {
            lines.push("Web search is enabled; web.search may be used for fresh external facts.".to_owned());
        } else {
            lines.push("Web search is disabled for this run.".to_owned());
        }
        prompt.push(' ');
        prompt.push_str(&lines.join(" "));
    }
    prompt
}

fn join_i64_values(values: &[i64]) -> String {
    values
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}
```

Update `build_model_loop_messages_from_history` signature:

```rust
fn build_model_loop_messages_from_history(
    original_input: &str,
    tool_codes: &[String],
    workbench_context: Option<&AgentWorkbenchContext>,
    history: &[AgentTurnItem],
) -> Vec<ModelChatMessage>
```

Use:

```rust
content: build_model_loop_system_prompt_with_context(tool_codes, workbench_context),
```

Update call sites to pass `command.workbench_context.as_ref()` in the runtime path and `None` in existing tests that do not assert context.

- [ ] **Step 5: Run backend tests to verify GREEN**

Run:

```bash
cargo test -p backend-rust workbench_context_normalization_bounds_lists_and_trims_values -- --nocapture
cargo test -p backend-rust workbench_context_normalization_drops_empty_context -- --nocapture
cargo test -p backend-rust agent_run_command_payload_preserves_workbench_context -- --nocapture
cargo test -p backend-rust model_loop_system_prompt_includes_workbench_context_without_user_text_mutation -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: add agent workbench context"
```

---

### Task 2: Web Search Tool Contract And Dry-Run Executor

**Files:**
- Modify: `crates/novex-tools/src/lib.rs`
- Modify: `backend/src/application/ai/agent_tool_executor.rs`

**Interfaces:**
- Produces: `WEB_SEARCH_TOOL_CODE: &str = "web.search"`
- Produces: `AgentToolExecutorSelection::WebSearch`
- Produces: `execute_web_search_tool(input: &Value) -> AgentToolExecution`
- Consumes: existing model-loop tool router and executor registry

- [ ] **Step 1: Write failing tool definition tests**

In `crates/novex-tools/src/lib.rs`, extend existing tests:

```rust
#[test]
fn agent_model_loop_tool_definitions_include_web_search() {
    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
        .expect("agent model loop tools should build a router");
    let codes = router.tool_codes();

    assert!(codes.contains(&"web.search".to_owned()));
}

#[test]
fn web_search_executor_binding_is_builtin() {
    let registry =
        ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())
            .expect("agent executor registry should build");

    let web = registry
        .executor_for("web.search")
        .expect("web.search should have an executor");

    assert_eq!(web.executor_code, "builtin.web.search");
    assert_eq!(web.kind, ToolExecutorKind::Builtin);
}
```

- [ ] **Step 2: Run tool tests to verify RED**

Run:

```bash
cargo test -p novex-tools agent_model_loop_tool_definitions_include_web_search -- --nocapture
cargo test -p novex-tools web_search_executor_binding_is_builtin -- --nocapture
```

Expected: FAIL because `web.search` has no definition or binding.

- [ ] **Step 3: Add `web.search` definition and binding**

In `agent_model_loop_tool_definitions()`, insert after `rag.search`:

```rust
ToolDefinition {
    code: "web.search".to_owned(),
    name: "Search web".to_owned(),
    description: "Search fresh external web results when the run enables web search.".to_owned(),
    input_schema: json!({
        "type": "object",
        "required": ["query"],
        "properties": {
            "query": {"type": "string"},
            "limit": {"type": "integer", "minimum": 1, "maximum": 10}
        }
    }),
    output_schema: Some(json!({
        "type": "object",
        "properties": {
            "dryRun": {"type": "boolean"},
            "status": {"type": "string"},
            "query": {"type": "string"},
            "results": {"type": "array"},
            "message": {"type": "string"}
        }
    })),
    risk_level: ToolRiskLevel::Low,
    approval_policy: ApprovalPolicy::OnRisk,
    permission_code: Some("ai:agent:run".to_owned()),
    concurrency: ToolConcurrencyPolicy::shared(),
},
```

Add to `agent_model_loop_tool_executor_bindings()`:

```rust
ToolExecutorBinding::new(
    "web.search",
    "builtin.web.search",
    ToolExecutorKind::Builtin,
),
```

- [ ] **Step 4: Write failing executor tests**

In `backend/src/application/ai/agent_tool_executor.rs`, add tests:

```rust
#[test]
fn web_search_executor_selection_matches_tool_code_and_binding() {
    assert_eq!(
        AgentToolExecutorSelection::from_dispatch("web.search", ToolKind::Function, None),
        AgentToolExecutorSelection::WebSearch
    );

    let dispatch = ToolExecutorDispatchPlan {
        tool_code: "web.search".to_owned(),
        executor_code: "builtin.web.search".to_owned(),
        executor_kind: novex_tools::ToolExecutorKind::Builtin,
        requires_connector_credential: false,
        requires_mcp_tool: false,
        supports_background_tasks: false,
        raw_binding: json!({}),
    };

    assert_eq!(
        AgentToolExecutorSelection::from_dispatch("custom.web", ToolKind::Function, Some(&dispatch)),
        AgentToolExecutorSelection::WebSearch
    );
}

#[tokio::test]
async fn web_search_tool_returns_dry_run_when_provider_missing() {
    let output = execute_web_search_tool(&json!({"query": "latest Novex release", "limit": 3})).await;

    assert!(output.succeeded_status());
    assert!(output.dry_run);
    assert_eq!(output.output["dryRun"], true);
    assert_eq!(output.output["status"], "dry_run");
    assert_eq!(output.output["query"], "latest Novex release");
}
```

- [ ] **Step 5: Run executor tests to verify RED**

Run:

```bash
cargo test -p backend-rust web_search_executor_selection_matches_tool_code_and_binding -- --nocapture
cargo test -p backend-rust web_search_tool_returns_dry_run_when_provider_missing -- --nocapture
```

Expected: FAIL because `WebSearch`, `WEB_SEARCH_TOOL_CODE`, and `execute_web_search_tool` do not exist.

- [ ] **Step 6: Add dry-run executor**

In `agent_tool_executor.rs`, add:

```rust
pub(super) const WEB_SEARCH_TOOL_CODE: &str = "web.search";
```

Add enum variant:

```rust
WebSearch,
```

Update dispatch matching:

```rust
Some("builtin.web.search") => return Self::WebSearch,
```

Update tool code matching:

```rust
WEB_SEARCH_TOOL_CODE => Self::WebSearch,
```

Update `execute_agent_tool` match:

```rust
AgentToolExecutorSelection::WebSearch => {
    return execute_web_search_tool(input).await;
}
```

Add executor:

```rust
async fn execute_web_search_tool(input: &Value) -> AgentToolExecution {
    let query = input
        .get("query")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    let limit = input
        .get("limit")
        .and_then(Value::as_i64)
        .unwrap_or(5)
        .clamp(1, 10);

    AgentToolExecution::succeeded(
        json!({
            "dryRun": true,
            "status": "dry_run",
            "toolCode": WEB_SEARCH_TOOL_CODE,
            "query": query,
            "limit": limit,
            "results": [],
            "message": "web.search is wired, but no web search provider is configured for this POC environment"
        }),
        true,
        "Web search dry-run executed.".to_owned(),
    )
}
```

- [ ] **Step 7: Run tool and executor tests to verify GREEN**

Run:

```bash
cargo test -p novex-tools agent_model_loop_tool_definitions_include_web_search -- --nocapture
cargo test -p novex-tools web_search_executor_binding_is_builtin -- --nocapture
cargo test -p backend-rust web_search_executor_selection_matches_tool_code_and_binding -- --nocapture
cargo test -p backend-rust web_search_tool_returns_dry_run_when_provider_missing -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit Task 2**

```bash
git add crates/novex-tools/src/lib.rs backend/src/application/ai/agent_tool_executor.rs
git commit -m "feat: add web search agent tool"
```

---

### Task 3: Frontend API And Workbench Command Composition

**Files:**
- Modify: `apps/codex-app-poc/src/types/agent.ts`
- Modify: `apps/codex-app-poc/src/lib/api.ts`
- Modify: `apps/codex-app-poc/src/api/agent.ts`
- Modify: `apps/codex-app-poc/src/api/agent.test.ts`
- Create: `apps/codex-app-poc/src/types/knowledge.ts`
- Create: `apps/codex-app-poc/src/types/capability.ts`
- Create: `apps/codex-app-poc/src/api/knowledge.ts`
- Create: `apps/codex-app-poc/src/api/knowledge.test.ts`
- Create: `apps/codex-app-poc/src/api/capability.ts`
- Create: `apps/codex-app-poc/src/api/capability.test.ts`
- Create: `apps/codex-app-poc/src/api/workbench.ts`
- Create: `apps/codex-app-poc/src/api/workbench.test.ts`

**Interfaces:**
- Produces: `WorkbenchContext` TypeScript type
- Produces: `apiFormRequest<T>(path: string, form: FormData, init?: ApiRequestInit) -> Promise<T>`
- Produces: `ensureWorkbenchDataset() -> Promise<DatasetResp>`
- Produces: `buildWorkbenchAgentRunCommand(input: string, context: WorkbenchContext) -> AgentRunCommand`
- Consumes: existing `createConfiguredModelAgentRun(input, context?)`

- [ ] **Step 1: Write failing API tests**

Add `apps/codex-app-poc/src/api/workbench.test.ts`:

```ts
import { describe, expect, it, vi, afterEach } from "vitest";
import { buildWorkbenchAgentRunCommand, ensureWorkbenchDataset } from "./workbench";

describe("workbench api helpers", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  it("builds configured model-loop commands with typed workbench context", () => {
    vi.stubEnv("NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID", " runtime.llm.code_agent ");

    const command = buildWorkbenchAgentRunCommand("Summarize this file", {
      mode: "agent",
      datasetId: 7,
      documentIds: [11],
      fileIds: [19],
      skillCodes: ["support.refund"],
      mcpToolCodes: ["mcp.docs.search"],
      webSearchEnabled: true,
      routeId: "runtime.llm.code_agent"
    });

    expect(command).toEqual({
      input: "Summarize this file",
      runtimeMode: "model_loop",
      autoApprove: false,
      modelRouteId: "runtime.llm.code_agent",
      budget: {
        maxSteps: 8,
        maxToolCalls: 2,
        maxSeconds: 90,
        maxCostCents: 0
      },
      workbenchContext: {
        mode: "agent",
        datasetId: 7,
        documentIds: [11],
        fileIds: [19],
        skillCodes: ["support.refund"],
        mcpToolCodes: ["mcp.docs.search"],
        webSearchEnabled: true,
        routeId: "runtime.llm.code_agent"
      }
    });
  });

  it("reuses an existing Codex Workbench Inbox dataset", async () => {
    const fetchMock = vi.fn(async (url: string, init?: RequestInit) => ({
      ok: true,
      json: async () => {
        if (String(url).includes("/ai/knowledge/datasets?name=Codex+Workbench+Inbox")) {
          return {
            code: "200",
            data: {
              list: [{ id: 9, name: "Codex Workbench Inbox", status: 1 }],
              total: 1
            }
          };
        }
        throw new Error(`unexpected request ${url} ${init?.method}`);
      }
    }));
    vi.stubGlobal("fetch", fetchMock);

    const dataset = await ensureWorkbenchDataset();

    expect(dataset.id).toBe(9);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
```

Extend `apps/codex-app-poc/src/api/agent.test.ts`:

```ts
it("sends workbench context with configured model run", async () => {
  const fetchMock = vi.fn(async () => ({
    ok: true,
    json: async () => ({
      code: "200",
      data: { runId: 4, status: "queued", traceId: "agent-4" }
    })
  }));
  vi.stubGlobal("fetch", fetchMock);

  await createConfiguredModelAgentRun("Summarize file", {
    mode: "agent",
    datasetId: 7,
    documentIds: [11],
    fileIds: [19],
    skillCodes: ["support.refund"],
    mcpToolCodes: ["mcp.docs.search"],
    webSearchEnabled: true
  });

  expect(fetchMock).toHaveBeenCalledWith(
    expect.stringContaining("/ai/agents/runs"),
    expect.objectContaining({
      method: "POST",
      body: expect.stringContaining('"workbenchContext"')
    })
  );
});
```

Add `apps/codex-app-poc/src/api/knowledge.test.ts`:

```ts
import { afterEach, describe, expect, it, vi } from "vitest";
import { createDataset, getParseJob, listDatasets, uploadKnowledgeFile } from "./knowledge";

describe("knowledge api", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("lists datasets by name", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ code: "200", data: { list: [], total: 0 } })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await listDatasets({ name: "Codex Workbench Inbox", page: 1, size: 10 });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:4398/ai/knowledge/datasets?name=Codex+Workbench+Inbox&page=1&size=10",
      expect.objectContaining({ method: "GET" })
    );
  });

  it("creates datasets through JSON API", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ code: "200", data: 7 })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await createDataset({ name: "Codex Workbench Inbox", visibility: 1, retrievalMode: 1 });

    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining("/ai/knowledge/datasets"),
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ name: "Codex Workbench Inbox", visibility: 1, retrievalMode: 1 })
      })
    );
  });

  it("uploads files as multipart form data", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: {
          file: { id: 19, originalName: "handbook.md" },
          parseJob: { id: 29, documentId: 11, status: 1 }
        }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await uploadKnowledgeFile(7, new File(["hello"], "handbook.md", { type: "text/markdown" }));

    const [, init] = fetchMock.mock.calls[0];
    expect(String(fetchMock.mock.calls[0][0])).toContain("/ai/knowledge/datasets/7/documents/files");
    expect(init.method).toBe("POST");
    expect(init.body).toBeInstanceOf(FormData);
    expect(init.headers["Content-Type"]).toBeUndefined();
  });

  it("gets parse jobs", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ code: "200", data: { id: 29, status: 2 } })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await getParseJob(7, 29);

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:4398/ai/knowledge/datasets/7/parse-jobs/29",
      expect.objectContaining({ method: "GET" })
    );
  });
});
```

Add `apps/codex-app-poc/src/api/capability.test.ts`:

```ts
import { afterEach, describe, expect, it, vi } from "vitest";
import { listMcpServers, listMcpTools, listSkills } from "./capability";

describe("capability api", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("lists skills", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ code: "200", data: { list: [], total: 0 } })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await listSkills({ page: 1, size: 20 });

    expect(fetchMock).toHaveBeenCalledWith(
      "http://localhost:4398/ai/capabilities/skills?page=1&size=20",
      expect.objectContaining({ method: "GET" })
    );
  });

  it("lists MCP servers and server tools", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({ code: "200", data: { list: [], total: 0 } })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await listMcpServers({ page: 1, size: 20 });
    await listMcpTools(12);

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "http://localhost:4398/ai/capabilities/mcp/servers?page=1&size=20",
      expect.objectContaining({ method: "GET" })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "http://localhost:4398/ai/capabilities/mcp/servers/12/tools",
      expect.objectContaining({ method: "GET" })
    );
  });
});
```

- [ ] **Step 2: Run frontend API tests to verify RED**

Run:

```bash
pnpm --dir apps/codex-app-poc test -- src/api/workbench.test.ts src/api/knowledge.test.ts src/api/capability.test.ts src/api/agent.test.ts
```

Expected: FAIL because the new modules and `WorkbenchContext` do not exist.

- [ ] **Step 3: Add TypeScript types**

Add to `apps/codex-app-poc/src/types/agent.ts`:

```ts
export type WorkbenchContext = {
  mode: "agent";
  datasetId?: number;
  documentIds: number[];
  fileIds: number[];
  skillCodes: string[];
  mcpToolCodes: string[];
  webSearchEnabled: boolean;
  routeId?: string;
};
```

Extend `AgentRunCommand`:

```ts
workbenchContext?: WorkbenchContext;
```

Create `apps/codex-app-poc/src/types/knowledge.ts` by copying the DTOs used in the tests from `apps/chat-web/src/types/knowledge.ts`.

Create `apps/codex-app-poc/src/types/capability.ts`:

```ts
import type { PageResult } from "./agent";

export type CapabilityQuery = {
  page?: number;
  size?: number;
  status?: number;
  kind?: string;
};

export type CapabilityItemResp = {
  id: number;
  code: string;
  name: string;
  description: string;
  kind: string;
  status: number;
  riskLevel?: number | null;
  metadata: Record<string, unknown>;
  createTime: string;
};

export type McpToolResp = {
  id: number;
  serverId: number;
  serverCode: string;
  toolName: string;
  toolCode: string;
  description: string;
  inputSchema: Record<string, unknown>;
  outputSchema: Record<string, unknown>;
  riskLevel: number;
  permissionCode?: string | null;
  status: number;
  metadata: Record<string, unknown>;
  createTime: string;
  updateTime?: string | null;
};

export type CapabilityPage = PageResult<CapabilityItemResp>;
```

- [ ] **Step 4: Add API helpers**

Update `apps/codex-app-poc/src/lib/api.ts` with:

```ts
export async function apiFormRequest<T>(
  path: string,
  form: FormData,
  init: ApiRequestInit = {}
): Promise<T> {
  const { query, headers: initHeaders, ...requestInit } = init;
  const headers: Record<string, string> = {};
  new Headers(initHeaders).forEach((value, key) => {
    headers[key] = value;
  });
  const token = getAuthToken();
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  const response = await fetch(apiUrl(path, query), {
    method: "POST",
    ...requestInit,
    body: form,
    headers
  });
  const body = (await response.json()) as ApiEnvelope<T>;

  if (!response.ok || body.code !== "200") {
    throw new Error(body.msg ?? body.message ?? "Request failed");
  }

  return body.data as T;
}
```

Create `apps/codex-app-poc/src/api/knowledge.ts`:

```ts
import { apiFormRequest, apiRequest } from "@/lib/api";
import type {
  DatasetCommand,
  DatasetQuery,
  DatasetResp,
  KnowledgeFileUploadResp,
  ParserJobResp
} from "@/types/knowledge";
import type { PageResult } from "@/types/agent";

const DATASET_URL = "/ai/knowledge/datasets";

export function listDatasets(query: DatasetQuery = {}) {
  return apiRequest<PageResult<DatasetResp>>(DATASET_URL, { query, method: "GET" });
}

export function createDataset(data: DatasetCommand) {
  return apiRequest<number>(DATASET_URL, {
    method: "POST",
    body: JSON.stringify(data)
  });
}

export function uploadKnowledgeFile(datasetId: number, file: File, parentPath = "/knowledge") {
  const form = new FormData();
  form.append("file", file, file.name);
  form.append("parentPath", parentPath);
  return apiFormRequest<KnowledgeFileUploadResp>(
    `${DATASET_URL}/${datasetId}/documents/files`,
    form
  );
}

export function getParseJob(datasetId: number, jobId: number) {
  return apiRequest<ParserJobResp>(`${DATASET_URL}/${datasetId}/parse-jobs/${jobId}`, {
    method: "GET"
  });
}
```

Create `apps/codex-app-poc/src/api/capability.ts`:

```ts
import { apiRequest } from "@/lib/api";
import type { PageResult } from "@/types/agent";
import type { CapabilityItemResp, CapabilityQuery, McpToolResp } from "@/types/capability";

export function listSkills(query: CapabilityQuery = {}) {
  return apiRequest<PageResult<CapabilityItemResp>>("/ai/capabilities/skills", {
    query,
    method: "GET"
  });
}

export function listMcpServers(query: CapabilityQuery = {}) {
  return apiRequest<PageResult<CapabilityItemResp>>("/ai/capabilities/mcp/servers", {
    query,
    method: "GET"
  });
}

export function listMcpTools(serverId: number) {
  return apiRequest<McpToolResp[]>(`/ai/capabilities/mcp/servers/${serverId}/tools`, {
    method: "GET"
  });
}
```

Create `apps/codex-app-poc/src/api/workbench.ts`:

```ts
import { createDataset, listDatasets } from "./knowledge";
import type { AgentRunCommand, WorkbenchContext } from "@/types/agent";
import type { DatasetResp } from "@/types/knowledge";

export const WORKBENCH_DATASET_NAME = "Codex Workbench Inbox";

export function buildWorkbenchAgentRunCommand(
  input: string,
  context: WorkbenchContext
): AgentRunCommand {
  const configuredRouteId = process.env.NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID?.trim();
  const routeId = context.routeId?.trim() || configuredRouteId || undefined;

  return {
    input,
    runtimeMode: "model_loop",
    autoApprove: false,
    modelRouteId: routeId,
    budget: {
      maxSteps: 8,
      maxToolCalls: 2,
      maxSeconds: 90,
      maxCostCents: 0
    },
    workbenchContext: {
      ...context,
      routeId
    }
  };
}

export async function ensureWorkbenchDataset(): Promise<DatasetResp> {
  const existing = await listDatasets({ name: WORKBENCH_DATASET_NAME, page: 1, size: 10 });
  const matched = existing.list.find((dataset) => dataset.name === WORKBENCH_DATASET_NAME);
  if (matched) {
    return matched;
  }

  const id = await createDataset({
    name: WORKBENCH_DATASET_NAME,
    description: "Default uploaded-file inbox for the Codex conversation workbench POC.",
    visibility: 1,
    retrievalMode: 1
  });

  return {
    id,
    tenantId: 0,
    name: WORKBENCH_DATASET_NAME,
    description: "Default uploaded-file inbox for the Codex conversation workbench POC.",
    ownerId: 0,
    visibility: 1,
    status: 1,
    retrievalMode: 1,
    documentCount: 0,
    chunkCount: 0,
    createUserString: "",
    createTime: "",
    updateUserString: "",
    updateTime: ""
  };
}
```

Update `createConfiguredModelAgentRun` in `apps/codex-app-poc/src/api/agent.ts`:

```ts
export function createConfiguredModelAgentRun(input: string, workbenchContext?: WorkbenchContext) {
  return createAgentRun(buildWorkbenchAgentRunCommand(input, workbenchContext ?? {
    mode: "agent",
    documentIds: [],
    fileIds: [],
    skillCodes: [],
    mcpToolCodes: [],
    webSearchEnabled: false
  }));
}
```

- [ ] **Step 5: Run frontend API tests to verify GREEN**

Run:

```bash
pnpm --dir apps/codex-app-poc test -- src/api/workbench.test.ts src/api/knowledge.test.ts src/api/capability.test.ts src/api/agent.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit Task 3**

```bash
git add apps/codex-app-poc/src
git commit -m "feat: add codex workbench api clients"
```

---

### Task 4: Readable Event Evidence Mapping

**Files:**
- Create: `apps/codex-app-poc/src/lib/workbench-events.ts`
- Create: `apps/codex-app-poc/src/lib/workbench-events.test.ts`
- Modify: `apps/codex-app-poc/src/lib/agent-events.ts`

**Interfaces:**
- Produces: `WorkbenchEventEvidence`
- Produces: `summarizeWorkbenchEvent(event: AgentRunEventResp) -> WorkbenchEventEvidence`
- Consumes: existing `AgentRunEventResp`

- [ ] **Step 1: Write failing event evidence tests**

Create `apps/codex-app-poc/src/lib/workbench-events.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { summarizeWorkbenchEvent } from "./workbench-events";
import type { AgentRunEventResp } from "@/types/agent";

function event(eventType: string, payload: AgentRunEventResp["payload"]): AgentRunEventResp {
  return {
    id: 1,
    runId: 7,
    eventType,
    sequenceNo: 1,
    status: "running",
    payload,
    createTime: "2026-06-18 12:00:00"
  };
}

describe("workbench event summaries", () => {
  it("summarizes model deltas", () => {
    const summary = summarizeWorkbenchEvent(event("thought", {
      item: { type: "model_delta", content: "Hello" }
    }));

    expect(summary).toMatchObject({
      kind: "assistant_delta",
      title: "Assistant",
      text: "Hello"
    });
  });

  it("summarizes tool calls", () => {
    const summary = summarizeWorkbenchEvent(event("tool_called", {
      toolCode: "rag.search",
      arguments: { query: "refund", datasetId: 7 }
    }));

    expect(summary.kind).toBe("tool");
    expect(summary.title).toBe("rag.search");
    expect(summary.text).toContain("datasetId");
  });

  it("summarizes retrieval evidence", () => {
    const summary = summarizeWorkbenchEvent(event("thought", {
      item: {
        type: "tool_observation",
        toolCode: "rag.search",
        output: { hits: [{ id: 1 }, { id: 2 }], citations: [{ documentId: "11" }] }
      }
    }));

    expect(summary.kind).toBe("retrieval");
    expect(summary.title).toBe("Knowledge search");
    expect(summary.text).toContain("2 hits");
  });

  it("summarizes web search dry-run evidence", () => {
    const summary = summarizeWorkbenchEvent(event("thought", {
      item: {
        type: "tool_observation",
        toolCode: "web.search",
        output: { dryRun: true, status: "dry_run", query: "fresh facts", results: [] }
      }
    }));

    expect(summary.kind).toBe("web_search");
    expect(summary.title).toBe("Web search");
    expect(summary.text).toContain("dry-run");
  });

  it("keeps raw fallback evidence readable", () => {
    const summary = summarizeWorkbenchEvent(event("unknown", { hello: "world" }));

    expect(summary.kind).toBe("raw");
    expect(summary.title).toBe("unknown");
    expect(summary.raw).toEqual({ hello: "world" });
  });
});
```

- [ ] **Step 2: Run event tests to verify RED**

Run:

```bash
pnpm --dir apps/codex-app-poc test -- src/lib/workbench-events.test.ts
```

Expected: FAIL because `workbench-events.ts` does not exist.

- [ ] **Step 3: Implement event mapper**

Create `apps/codex-app-poc/src/lib/workbench-events.ts`:

```ts
import type { AgentRunEventResp } from "@/types/agent";

export type WorkbenchEventKind =
  | "assistant_delta"
  | "model"
  | "tool"
  | "retrieval"
  | "mcp"
  | "web_search"
  | "terminal"
  | "error"
  | "raw";

export type WorkbenchEventEvidence = {
  kind: WorkbenchEventKind;
  title: string;
  text: string;
  status: string;
  sequenceNo: number;
  raw: AgentRunEventResp["payload"];
};

export function summarizeWorkbenchEvent(event: AgentRunEventResp): WorkbenchEventEvidence {
  const payload = objectPayload(event.payload);
  const item = objectPayload(payload.item);
  const type = stringValue(item.type);
  const toolCode = stringValue(payload.toolCode) || stringValue(item.toolCode);
  const output = objectPayload(item.output) || objectPayload(payload.output);

  if (type === "model_delta") {
    return evidence(event, "assistant_delta", "Assistant", stringValue(item.content) || "");
  }

  if (toolCode === "rag.search") {
    const hits = Array.isArray(output?.hits) ? output.hits.length : 0;
    return evidence(event, "retrieval", "Knowledge search", `${hits} hits from rag.search`);
  }

  if (toolCode === "web.search") {
    const dryRun = output?.dryRun === true || output?.status === "dry_run";
    return evidence(
      event,
      "web_search",
      "Web search",
      dryRun ? "web.search dry-run; provider is not configured" : "web.search returned results"
    );
  }

  if (toolCode?.startsWith("mcp.")) {
    return evidence(event, "mcp", toolCode, compactJson(payload.arguments ?? output ?? payload));
  }

  if (toolCode) {
    return evidence(event, "tool", toolCode, compactJson(payload.arguments ?? output ?? payload));
  }

  if (event.status === "failed" || event.eventType === "error") {
    return evidence(event, "error", "Error", stringValue(payload.message) || "Agent run failed");
  }

  if (["succeeded", "cancelled", "waiting_approval"].includes(event.status)) {
    return evidence(event, "terminal", event.status, stringValue(payload.message) || event.status);
  }

  return evidence(event, "raw", event.eventType, compactJson(event.payload));
}

function evidence(
  event: AgentRunEventResp,
  kind: WorkbenchEventKind,
  title: string,
  text: string
): WorkbenchEventEvidence {
  return {
    kind,
    title,
    text,
    status: event.status,
    sequenceNo: event.sequenceNo,
    raw: event.payload
  };
}

function objectPayload(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function compactJson(value: unknown): string {
  return JSON.stringify(value ?? {});
}
```

- [ ] **Step 4: Keep existing model delta helper compatible**

Modify `apps/codex-app-poc/src/lib/agent-events.ts` only to reuse `summarizeWorkbenchEvent` for new rendering surfaces while preserving current exports. Add this shape:

```ts
export { summarizeWorkbenchEvent } from "./workbench-events";
export type { WorkbenchEventEvidence } from "./workbench-events";
```

- [ ] **Step 5: Run event tests to verify GREEN**

Run:

```bash
pnpm --dir apps/codex-app-poc test -- src/lib/workbench-events.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit Task 4**

```bash
git add apps/codex-app-poc/src/lib
git commit -m "feat: summarize workbench run evidence"
```

---

### Task 5: Codex Conversation Workbench UI

**Files:**
- Modify: `apps/codex-app-poc/src/app-client.tsx`
- Modify: `apps/codex-app-poc/app/page.test.tsx`

**Interfaces:**
- Consumes: `createConfiguredModelAgentRun(input, workbenchContext)`
- Consumes: `ensureWorkbenchDataset`, `uploadKnowledgeFile`, `getParseJob`
- Consumes: `listSkills`, `listMcpServers`, `listMcpTools`
- Consumes: `summarizeWorkbenchEvent`
- Produces: interactive context drawer state and run command payload

- [ ] **Step 1: Write failing UI tests**

Extend `apps/codex-app-poc/app/page.test.tsx` with tests in the existing style:

```tsx
it("shows workbench context controls", async () => {
  render(<Home />);

  expect(await screen.findByText("Context")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /Files/i })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /Skills/i })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /MCP/i })).toBeInTheDocument();
  expect(screen.getByRole("switch", { name: /Web search/i })).toBeInTheDocument();
});

it("submits selected workbench context with a direct question", async () => {
  const fetchMock = vi.fn(async (url: string) => {
    if (String(url).includes("/ai/capabilities/skills")) {
      return { ok: true, json: async () => ({ code: "200", data: { list: [{ id: 1, code: "support.refund", name: "Refund support", description: "", kind: "skill", status: 1, metadata: {}, createTime: "" }], total: 1 } }) };
    }
    if (String(url).includes("/ai/capabilities/mcp/servers")) {
      return { ok: true, json: async () => ({ code: "200", data: { list: [], total: 0 } }) };
    }
    if (String(url).includes("/ai/agents/runs")) {
      return { ok: true, json: async () => ({ code: "200", data: { runId: 7, status: "succeeded", traceId: "agent-7" } }) };
    }
    if (String(url).includes("/events")) {
      return { ok: true, json: async () => ({ code: "200", data: { list: [], total: 0 } }) };
    }
    return { ok: true, json: async () => ({ code: "200", data: {} }) };
  });
  vi.stubGlobal("fetch", fetchMock);

  render(<Home />);

  await userEvent.click(await screen.findByRole("button", { name: /Refund support/i }));
  await userEvent.click(screen.getByRole("switch", { name: /Web search/i }));
  await userEvent.type(screen.getByRole("textbox"), "Explain the refund policy");
  await userEvent.keyboard("{Enter}");

  const runCall = fetchMock.mock.calls.find(([url]) => String(url).includes("/ai/agents/runs"));
  expect(runCall?.[1]?.body).toContain('"workbenchContext"');
  expect(runCall?.[1]?.body).toContain('"skillCodes":["support.refund"]');
  expect(runCall?.[1]?.body).toContain('"webSearchEnabled":true');
});

it("uploads a file and includes dataset context in the next run", async () => {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    const href = String(url);
    if (href.includes("/ai/knowledge/datasets?name=Codex+Workbench+Inbox")) {
      return { ok: true, json: async () => ({ code: "200", data: { list: [{ id: 7, name: "Codex Workbench Inbox", status: 1 }], total: 1 } }) };
    }
    if (href.includes("/documents/files")) {
      expect(init?.body).toBeInstanceOf(FormData);
      return { ok: true, json: async () => ({ code: "200", data: { file: { id: 19, originalName: "handbook.md" }, parseJob: { id: 29, documentId: 11, status: 2 } } }) };
    }
    if (href.includes("/parse-jobs/29")) {
      return { ok: true, json: async () => ({ code: "200", data: { id: 29, documentId: 11, fileId: 19, status: 2, documentName: "handbook.md" } }) };
    }
    if (href.includes("/ai/capabilities")) {
      return { ok: true, json: async () => ({ code: "200", data: { list: [], total: 0 } }) };
    }
    if (href.includes("/ai/agents/runs")) {
      return { ok: true, json: async () => ({ code: "200", data: { runId: 7, status: "succeeded", traceId: "agent-7" } }) };
    }
    if (href.includes("/events")) {
      return { ok: true, json: async () => ({ code: "200", data: { list: [], total: 0 } }) };
    }
    return { ok: true, json: async () => ({ code: "200", data: {} }) };
  });
  vi.stubGlobal("fetch", fetchMock);

  render(<Home />);

  const input = await screen.findByLabelText("Upload files");
  await userEvent.upload(input, new File(["hello"], "handbook.md", { type: "text/markdown" }));
  expect(await screen.findByText("handbook.md")).toBeInTheDocument();

  await userEvent.type(screen.getByRole("textbox"), "Summarize the file");
  await userEvent.keyboard("{Enter}");

  const runCall = fetchMock.mock.calls.find(([url]) => String(url).includes("/ai/agents/runs"));
  expect(runCall?.[1]?.body).toContain('"datasetId":7');
  expect(runCall?.[1]?.body).toContain('"documentIds":[11]');
  expect(runCall?.[1]?.body).toContain('"fileIds":[19]');
});

it("renders readable run evidence from raw events", async () => {
  const fetchMock = vi.fn(async (url: string) => {
    const href = String(url);
    if (href.includes("/ai/agents/runs") && !href.includes("/events")) {
      return { ok: true, json: async () => ({ code: "200", data: { runId: 7, status: "succeeded", traceId: "agent-7" } }) };
    }
    if (href.includes("/events")) {
      return {
        ok: true,
        json: async () => ({
          code: "200",
          data: {
            list: [
              { id: 1, runId: 7, eventType: "thought", sequenceNo: 1, status: "running", payload: { item: { type: "model_delta", content: "Hello" } }, createTime: "" },
              { id: 2, runId: 7, eventType: "thought", sequenceNo: 2, status: "succeeded", payload: { item: { type: "tool_observation", toolCode: "web.search", output: { dryRun: true, status: "dry_run" } } }, createTime: "" }
            ],
            total: 2
          }
        })
      };
    }
    if (href.includes("/ai/capabilities")) {
      return { ok: true, json: async () => ({ code: "200", data: { list: [], total: 0 } }) };
    }
    return { ok: true, json: async () => ({ code: "200", data: {} }) };
  });
  vi.stubGlobal("fetch", fetchMock);

  render(<Home />);

  await userEvent.type(screen.getByRole("textbox"), "Hello");
  await userEvent.keyboard("{Enter}");

  expect(await screen.findByText("Assistant")).toBeInTheDocument();
  expect(await screen.findByText("Web search")).toBeInTheDocument();
  expect(await screen.findByText(/dry-run/i)).toBeInTheDocument();
});
```

- [ ] **Step 2: Run UI tests to verify RED**

Run:

```bash
pnpm --dir apps/codex-app-poc test -- app/page.test.tsx
```

Expected: FAIL because context controls, upload flow, and evidence rendering are not present.

- [ ] **Step 3: Implement context state and capability loading**

In `app-client.tsx` add state for:

```ts
const [selectedSkillCodes, setSelectedSkillCodes] = useState<string[]>([]);
const [selectedMcpToolCodes, setSelectedMcpToolCodes] = useState<string[]>([]);
const [webSearchEnabled, setWebSearchEnabled] = useState(false);
const [uploadedFiles, setUploadedFiles] = useState<WorkbenchUploadedFile[]>([]);
const [capabilityError, setCapabilityError] = useState<string | null>(null);
```

Add local type:

```ts
type WorkbenchUploadedFile = {
  name: string;
  datasetId: number;
  documentId?: number;
  fileId?: number;
  parseJobId?: number;
  status: "uploading" | "parsing" | "indexed" | "failed" | "unavailable";
  message?: string;
};
```

On mount, load:

```ts
const [skillsPage, mcpServersPage] = await Promise.all([
  listSkills({ page: 1, size: 20 }),
  listMcpServers({ page: 1, size: 20 })
]);
```

For each enabled MCP server, call `listMcpTools(server.id)` and merge active tools.

- [ ] **Step 4: Implement file upload flow**

Add hidden input:

```tsx
<input
  aria-label="Upload files"
  className="sr-only"
  multiple
  type="file"
  onChange={(event) => void handleUploadFiles(event.currentTarget.files)}
/>
```

Add handler:

```ts
async function handleUploadFiles(fileList: FileList | null) {
  const files = Array.from(fileList ?? []);
  if (files.length === 0) {
    return;
  }

  const dataset = await ensureWorkbenchDataset();
  for (const file of files) {
    setUploadedFiles((items) => [
      ...items,
      { name: file.name, datasetId: dataset.id, status: "uploading" }
    ]);
    try {
      const uploaded = await uploadKnowledgeFile(dataset.id, file);
      setUploadedFiles((items) =>
        items.map((item) =>
          item.name === file.name && item.status === "uploading"
            ? {
                name: file.name,
                datasetId: dataset.id,
                documentId: uploaded.parseJob.documentId,
                fileId: uploaded.file.id,
                parseJobId: uploaded.parseJob.id,
                status: uploaded.parseJob.status >= 2 ? "indexed" : "parsing"
              }
            : item
        )
      );
      if (uploaded.parseJob.status < 2) {
        const parseJob = await getParseJob(dataset.id, uploaded.parseJob.id);
        setUploadedFiles((items) =>
          items.map((item) =>
            item.parseJobId === uploaded.parseJob.id
              ? {
                  ...item,
                  documentId: parseJob.documentId,
                  fileId: parseJob.fileId ?? item.fileId,
                  status: parseJob.status >= 2 ? "indexed" : "parsing"
                }
              : item
          )
        );
      }
    } catch (error) {
      setUploadedFiles((items) =>
        items.map((item) =>
          item.name === file.name
            ? { ...item, status: "failed", message: error instanceof Error ? error.message : "Upload failed" }
            : item
        )
      );
    }
  }
}
```

- [ ] **Step 5: Build and submit workbench context**

Before `createConfiguredModelAgentRun`, build:

```ts
const indexedFiles = uploadedFiles.filter((file) => file.status === "indexed" || file.status === "parsing");
const datasetId = indexedFiles.find((file) => file.datasetId)?.datasetId;
const context: WorkbenchContext = {
  mode: "agent",
  datasetId,
  documentIds: indexedFiles.flatMap((file) => (file.documentId ? [file.documentId] : [])),
  fileIds: indexedFiles.flatMap((file) => (file.fileId ? [file.fileId] : [])),
  skillCodes: selectedSkillCodes,
  mcpToolCodes: selectedMcpToolCodes,
  webSearchEnabled
};
const run = await createConfiguredModelAgentRun(input, context);
```

- [ ] **Step 6: Render context drawer and evidence list**

Add right drawer title `Context`, segmented sections `Files`, `Skills`, `MCP`, and `Web search`. Render chips as buttons with stable labels:

```tsx
<button type="button" onClick={() => toggleSkill(skill.code)}>
  {skill.name || skill.code}
</button>
```

Render event evidence:

```tsx
{events.map((event) => {
  const evidence = summarizeWorkbenchEvent(event);
  return (
    <article key={event.id} className="event-row">
      <strong>{evidence.title}</strong>
      <p>{evidence.text}</p>
    </article>
  );
})}
```

- [ ] **Step 7: Run UI tests to verify GREEN**

Run:

```bash
pnpm --dir apps/codex-app-poc test -- app/page.test.tsx
```

Expected: PASS.

- [ ] **Step 8: Commit Task 5**

```bash
git add apps/codex-app-poc/src/app-client.tsx apps/codex-app-poc/app/page.test.tsx
git commit -m "feat: build codex conversation workbench"
```

---

### Task 6: Matrix Update And Full Verification

**Files:**
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Interfaces:**
- Consumes: completed backend/frontend POC work
- Produces: documented migration matrix status for the workbench validation layer

- [ ] **Step 1: Update migration matrix**

Add a row or section entry that states:

```markdown
| Codex conversation workbench POC | `apps/codex-app-poc`, `backend/src/application/ai/agent_service.rs`, `crates/novex-tools` | In progress | Product-facing validation layer for configured model loop, workbench context, file-grounded RAG, skills, MCP, web.search dry-run, and readable run evidence. |
```

Add acceptance evidence text:

```markdown
The Codex conversation workbench POC is the first product-facing validation layer for the Novex agent foundation. It proves that a single typed run context can serve direct chat, file-grounded questions, selected skill context, selected MCP tools, and honest web-search capability state.
```

- [ ] **Step 2: Run frontend verification**

Run:

```bash
pnpm --dir apps/codex-app-poc test
pnpm --dir apps/codex-app-poc typecheck
pnpm --dir apps/codex-app-poc lint
```

Expected: all commands exit 0.

- [ ] **Step 3: Run backend/tool focused verification**

Run:

```bash
cargo test -p backend-rust workbench_context_normalization_bounds_lists_and_trims_values -- --nocapture
cargo test -p backend-rust workbench_context_normalization_drops_empty_context -- --nocapture
cargo test -p backend-rust agent_run_command_payload_preserves_workbench_context -- --nocapture
cargo test -p backend-rust model_loop_system_prompt_includes_workbench_context_without_user_text_mutation -- --nocapture
cargo test -p backend-rust web_search_executor_selection_matches_tool_code_and_binding -- --nocapture
cargo test -p backend-rust web_search_tool_returns_dry_run_when_provider_missing -- --nocapture
cargo test -p novex-tools agent_model_loop_tool_definitions_include_web_search -- --nocapture
cargo test -p novex-tools web_search_executor_binding_is_builtin -- --nocapture
```

Expected: all commands exit 0.

- [ ] **Step 4: Run formatting and diff checks**

Run:

```bash
cargo fmt --all -- --check
git diff --check
git status --short
```

Expected: formatting and diff checks exit 0. `git status --short` only shows intentional changed files before the final commit.

- [ ] **Step 5: Commit Task 6**

```bash
git add docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "docs: update codex workbench migration matrix"
```

- [ ] **Step 6: Merge and clean after user-visible verification**

Run from the main Novex checkout after the feature branch is verified:

```bash
git checkout main
git merge --ff-only feat/codex-conversation-workbench-poc
git worktree remove .worktrees/codex-conversation-workbench-poc
cargo clean
```

Expected: main contains the feature commits, the worktree is removed, and build artifacts are cleaned.
