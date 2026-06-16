# Agent MCP Gateway Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a tenant-governed MCP gateway so external MCP tools can be discovered, converted into model-visible Novex tools, audited, and invoked by the model-driven agent loop.

**Architecture:** Keep `novex-mcp` as the pure policy/protocol crate and keep database, RBAC, secret resolution, and tool audit in backend services. MCP tools enter the existing `ai_tool`/tool-audit path as `ToolKind::Mcp`, so the agent loop does not need tool-specific branches. This is an adapter port from Codex `codex-rs/codex-mcp` and `rmcp-client`, not a direct copy.

**Tech Stack:** Rust, Axum, SQLx/PostgreSQL migrations, `novex-mcp`, `novex-tools`, existing `AiCapabilityRepository`, existing `AgentService`.

---

## Scope

In scope:

- DB representation for MCP servers and discovered tools.
- Registration policy validation with tenant, auth, network allowlist, and tool allowlist.
- Discovery plan and deterministic discovery adapter for POC.
- Conversion of discovered MCP tools to `ToolDefinition` and `ai_tool` records.
- Invocation adapter that records tool audit and returns agent observations.

Out of scope:

- Full remote streaming MCP client lifecycle.
- OAuth browser flows.
- Long-running MCP resource subscriptions.
- Sandbox command tools.

## Task 1: Persist MCP Tool Discovery Contracts

**Files:**
- Modify: `backend/src/infrastructure/persistence/ai_capability_repository.rs`
- Create: `backend/migrations/202606160001_create_ai_mcp_gateway.sql`
- Modify: `backend/src/application/ai/capability_service.rs`

**Step 1: Write failing migration contract test**

Existing `ai_mcp_server` storage is already defined in `202606050006_create_ai_capability_registry.sql`. Add a test near capability migration tests for the missing discovered-tool table:

```rust
#[test]
fn mcp_gateway_migration_defines_discovered_tool_table() {
    let migrations = include_str!("../../../migrations/202606160001_create_ai_mcp_gateway.sql");

    assert!(migrations.contains("CREATE TABLE IF NOT EXISTS ai_mcp_tool"));
    assert!(migrations.contains("server_id"));
    assert!(migrations.contains("tool_name"));
    assert!(migrations.contains("tool_code"));
    assert!(migrations.contains("input_schema"));
    assert!(migrations.contains("output_schema"));
    assert!(migrations.contains("uk_ai_mcp_tool_tenant_tool_code"));
}
```

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend-rust mcp_gateway_migration_defines_discovered_tool_table --offline
```

Expected: FAIL because the migration is not implemented.

**Step 3: Add migration**

Add `ai_mcp_tool`: `id`, `tenant_id`, `server_id`, `tool_name`, `tool_code`, `description`, `input_schema`, `output_schema`, `risk_level`, `permission_code`, `status`, `metadata`, audit columns.

Unique constraints:

- `(tenant_id, server_id, tool_name)`
- `(tenant_id, tool_code)`

**Step 4: Add repository records**

Add structs:

```rust
pub struct McpToolSaveRecord { /* discovered tool fields */ }
pub struct McpToolRecord { /* DB row fields */ }
```

Add methods:

- `save_discovered_mcp_tools`
- `list_mcp_tools_by_server`
- `find_mcp_tool_by_tool_code`

**Step 5: Verify**

Run:

```bash
cargo test -p backend-rust mcp_gateway_migration_defines_discovered_tool_table --offline
cargo test -p backend-rust capability --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add backend/migrations/202606160001_create_ai_mcp_gateway.sql backend/src/infrastructure/persistence/ai_capability_repository.rs backend/src/application/ai/capability_service.rs
git commit -m "feat: persist mcp gateway registration records"
```

## Task 2: Extend `novex-mcp` Discovery and Tool Spec Mapping

**Files:**
- Modify: `crates/novex-mcp/src/lib.rs`
- Modify: `crates/novex-mcp/Cargo.toml`
- Modify: `crates/novex-tools/src/lib.rs`

**Step 1: Write failing tests**

Add to `crates/novex-mcp/src/lib.rs`:

```rust
#[test]
fn mcp_discovered_tool_converts_to_tenant_tool_definition() {
    let tool = McpDiscoveredTool {
        server_code: "docs".to_owned(),
        tool_name: "search".to_owned(),
        description: "Search docs".to_owned(),
        input_schema: serde_json::json!({"type":"object","properties":{"query":{"type":"string"}}}),
        output_schema: Some(serde_json::json!({"type":"object"})),
        risk_level: ToolRiskLevel::Low,
    };

    let definition = tool.to_tool_definition("ai:mcp:docs:search");

    assert_eq!(definition.code, "mcp.docs.search");
    assert_eq!(definition.input_schema["properties"]["query"]["type"], "string");
    assert_eq!(definition.permission_code.as_deref(), Some("ai:mcp:docs:search"));
}
```

**Step 2: Run failing test**

Run:

```bash
cargo test -p novex-mcp mcp_discovered_tool_converts_to_tenant_tool_definition --offline
```

Expected: FAIL because `McpDiscoveredTool` is missing.

**Step 3: Implement pure mapping**

Add:

- `McpDiscoveredTool`
- `McpToolInvocationRequest`
- `McpToolInvocationResult`
- `mcp_tool_code(server_code, tool_name)`
- `to_tool_definition(permission_code)`

Use `novex-tools` as a dependency for `ToolDefinition` and `ToolRiskLevel`.

**Step 4: Verify**

Run:

```bash
cargo test -p novex-mcp --offline
cargo test -p novex-tools --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-mcp crates/novex-tools
git commit -m "feat: map mcp discoveries to model-visible tools"
```

## Task 3: Add Backend MCP Registration and Discovery Service

**Files:**
- Modify: `backend/src/application/ai/capability_service.rs`
- Modify: `backend/src/interfaces/http/ai/capability.rs`
- Modify: `backend/src/infrastructure/persistence/ai_capability_repository.rs`

**Step 1: Write failing service tests**

Add tests:

- `mcp_server_command_normalizes_registration_policy`
- `mcp_discovery_persists_allowed_tools_as_ai_tools`
- `mcp_discovery_rejects_unallowlisted_tool`

The first test should build a command with:

```json
{
  "serverCode": "docs",
  "transportKind": "streamable_http",
  "endpointUrl": "https://mcp.example.com/mcp",
  "authType": "bearer_env",
  "secretRef": "env:DOCS_MCP_TOKEN",
  "networkAllowlist": ["mcp.example.com"],
  "toolAllowlist": ["search"]
}
```

**Step 2: Run failing tests**

Run:

```bash
cargo test -p backend-rust mcp_server_command_normalizes_registration_policy --offline
```

Expected: FAIL.

**Step 3: Implement commands and handlers**

Add:

- `McpServerCommand`
- `McpDiscoveryCommand`
- `McpServerResp`
- `McpToolResp`
- `register_mcp_server`
- `discover_mcp_tools`
- routes:
  - `POST /ai/capabilities/mcp/servers`
  - `POST /ai/capabilities/mcp/servers/:serverId/discover`
  - `GET /ai/capabilities/mcp/servers`
  - `GET /ai/capabilities/mcp/servers/:serverId/tools`

For the first POC, discovery may accept an explicit `tools` array in the request. A later task replaces this with live MCP client discovery.

**Step 4: Verify**

Run:

```bash
cargo test -p backend-rust mcp_ --offline
cargo test -p backend-rust capability --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/capability_service.rs backend/src/interfaces/http/ai/capability.rs backend/src/infrastructure/persistence/ai_capability_repository.rs
git commit -m "feat: add mcp gateway registration api"
```

## Task 4: Invoke MCP Tools Through Agent Tool Execution

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `backend/src/application/ai/capability_service.rs`
- Modify: `crates/novex-mcp/src/lib.rs`

**Step 1: Write failing tests**

Add an agent service contract test:

```rust
#[test]
fn agent_runtime_routes_mcp_tools_through_audited_observation_path() {
    let source = include_str!("agent_service.rs").split("#[cfg(test)]").next().unwrap();

    assert!(source.contains("execute_mcp_tool"));
    assert!(source.contains("ToolKind::Mcp"));
    assert!(source.contains("RunEventKind::Observation"));
}
```

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend-rust agent_runtime_routes_mcp_tools_through_audited_observation_path --offline
```

Expected: FAIL.

**Step 3: Implement invocation adapter**

Add `execute_mcp_tool` that:

1. Resolves `McpToolRecord` by `tool_code`.
2. Resolves server auth secret via existing secret/env policy.
3. Uses a deterministic POC invocation if `metadata.mockResponse` is present.
4. Otherwise returns a structured dry-run response with endpoint, server, tool, and arguments.
5. Always records audit through the existing tool audit path.

**Step 4: Verify**

Run:

```bash
cargo test -p backend-rust agent_runtime_routes_mcp_tools_through_audited_observation_path --offline
cargo test -p backend-rust --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs backend/src/application/ai/capability_service.rs crates/novex-mcp/src/lib.rs
git commit -m "feat: route mcp tools through agent observations"
```

## Task 5: Full Verification

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

Expected: PASS.

No live MCP smoke is required until a real MCP server endpoint is configured. The POC acceptance is that a discovered MCP tool can appear as a model-visible tool, be selected by the model loop, be audited, and produce an observation.
