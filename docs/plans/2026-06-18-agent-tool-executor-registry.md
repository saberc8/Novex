# Agent Tool Executor Registry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Codex-style tool executor registry contract to `novex-tools` so tool routing and future executor dispatch share a stable, testable vocabulary instead of relying only on backend string branches.

**Architecture:** `novex-tools` owns the provider-neutral executor registry types, default Agent model-loop executor bindings, duplicate/missing validation, and executor lookup errors. `backend-rust` keeps tenant context, repository access, connector credentials, model runtime access, audit persistence, and concrete tool I/O. The first slice is deliberately a registry/control-plane slice, not a full backend executor migration.

**Tech Stack:** Rust 2021, `novex-tools`, `serde`, `serde_json`, backend source-contract tests, offline Cargo tests.

### Boundaries

- Do not move `AgentService::execute_agent_tool_io`, audit persistence, media job persistence, credential lookup, MCP lookup, or `ModelRuntimeService` access into `novex-tools`.
- Do not add async runtime dependencies to `novex-tools` in this slice.
- Preserve existing `ToolRouter`, `ToolBatchPlan`, `ToolDefinition`, and model-visible schema behavior.
- The registry should map tool codes to executor codes and executor kinds. Concrete execution remains backend-owned until the next dispatch-migration slice.

## Task 1: Add RED Registry Contract Tests

**Files:**

- Modify: `crates/novex-tools/src/lib.rs`
- Modify: `backend/src/application/ai/agent_service.rs`

**Step Dependencies:** none

**API Contract:**

- `ToolExecutorKind`
- `ToolExecutorBinding`
- `ToolExecutorRegistry`
- `ToolExecutorRegistryError`
- `ToolExecutorRegistryErrorKind`
- `agent_model_loop_tool_executor_bindings`

- [x] **Step 1: Add novex-tools RED tests**

Add tests near the existing `ToolRouter` tests:

```rust
#[test]
fn tool_executor_registry_routes_known_agent_tools() {
    let registry = ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())
        .expect("agent executor registry should build");

    let rag = registry
        .executor_for(" rag.search ")
        .expect("rag.search should have an executor");
    assert_eq!(rag.executor_code, "builtin.rag.search");
    assert_eq!(rag.kind, ToolExecutorKind::Builtin);

    let media = registry
        .executor_for("media.image.generate")
        .expect("media image should have an executor");
    assert_eq!(media.kind, ToolExecutorKind::Model);
    assert!(media.supports_background_tasks);
}

#[test]
fn tool_executor_registry_rejects_duplicate_and_missing_bindings() {
    let duplicate = ToolExecutorRegistry::from_bindings(vec![
        ToolExecutorBinding::new("rag.search", "builtin.rag.search", ToolExecutorKind::Builtin),
        ToolExecutorBinding::new("rag.search", "builtin.rag.search.v2", ToolExecutorKind::Builtin),
    ])
    .unwrap_err();
    assert_eq!(duplicate.kind, ToolExecutorRegistryErrorKind::DuplicateToolCode);

    let missing = ToolExecutorRegistry::default().executor_for("sandbox.exec").unwrap_err();
    assert_eq!(missing.kind, ToolExecutorRegistryErrorKind::MissingExecutor);
    assert_eq!(missing.tool_code.as_deref(), Some("sandbox.exec"));
}

#[test]
fn agent_model_loop_executor_bindings_cover_agent_model_loop_tools() {
    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions())
        .expect("agent model loop tools should build a router");
    let registry = ToolExecutorRegistry::from_bindings(agent_model_loop_tool_executor_bindings())
        .expect("agent executor registry should build");

    assert_eq!(registry.tool_codes(), router.tool_codes());
}
```

- [x] **Step 2: Add backend source-contract RED test**

Add a source-contract test to `backend/src/application/ai/agent_service.rs`:

```rust
#[test]
fn agent_tool_executor_registry_boundary_lives_in_novex_tools() {
    let source = include_str!("../../../../crates/novex-tools/src/lib.rs");
    let backend_source = include_str!("agent_service.rs");

    assert!(source.contains("pub struct ToolExecutorRegistry"));
    assert!(source.contains("pub fn agent_model_loop_tool_executor_bindings"));
    assert!(source.contains("ToolExecutorRegistryErrorKind::MissingExecutor"));
    assert!(!backend_source.contains("struct ToolExecutorRegistry"));
}
```

- [x] **Step 3: Verify RED**

Run:

```bash
cargo test -p novex-tools tool_executor_registry --offline
cargo test -p backend-rust agent_tool_executor_registry_boundary_lives_in_novex_tools --offline
```

Expected: FAIL because the registry types do not exist yet.

## Task 2: Implement Registry Vocabulary

**Files:**

- Modify: `crates/novex-tools/src/lib.rs`

**Step Dependencies:** Task 1

- [x] **Step 1: Add executor kind and binding DTOs**

Implement:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutorKind {
    Builtin,
    Connector,
    Mcp,
    Model,
    Http,
    Sandbox,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutorBinding {
    pub tool_code: String,
    pub executor_code: String,
    pub kind: ToolExecutorKind,
    pub supports_background_tasks: bool,
    pub waits_for_runtime_cancellation: bool,
}
```

Add a constructor that trims tool/executor codes and default-flags to false.

- [x] **Step 2: Add registry error vocabulary**

Implement `ToolExecutorRegistryErrorKind` and `ToolExecutorRegistryError` for:

- `EmptyToolCode`
- `EmptyExecutorCode`
- `DuplicateToolCode`
- `MissingExecutor`

- [x] **Step 3: Add registry implementation**

Implement:

- `ToolExecutorRegistry::from_bindings`
- `ToolExecutorRegistry::tool_codes`
- `ToolExecutorRegistry::executor_for`

Use `BTreeMap` so code ordering remains deterministic.

- [x] **Step 4: Add Agent model-loop default bindings**

Implement `agent_model_loop_tool_executor_bindings()` with stable executor codes:

- `rag.search` -> `builtin.rag.search`, `Builtin`
- `github.repo.search` -> `connector.github.repo.search`, `Connector`
- `github.repo.read` -> `connector.github.repo.read`, `Connector`
- `media.image.generate` -> `model.media.image.generate`, `Model`, supports background tasks
- `feishu.message.send` -> `connector.feishu.message.send`, `Connector`

## Task 3: Wire Backend Boundary Evidence

**Files:**

- Modify: `backend/src/application/ai/agent_service.rs`

**Step Dependencies:** Task 2

- [x] **Step 1: Keep backend source-contract focused**

The backend test should only prove the registry boundary lives in `novex-tools` and backend has not reintroduced a local registry type.

- [x] **Step 2: Do not change runtime behavior**

Do not call the registry from production backend code in this slice. That belongs to a follow-up dispatch-migration slice because real execution currently depends on tenant-bound repository/model-runtime state.

## Task 4: Documentation And Verification

**Files:**

- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-18-agent-tool-executor-registry.md`

**Step Dependencies:** Tasks 1-3

- [x] **Step 1: Update migration matrix**

Update the Tool router row to the next slice and note that `novex-tools` now owns executor registry vocabulary and default Agent model-loop executor bindings. Keep "backend dispatch migration and background join-handle control remain next".

Update the Tool router acceptance row with:

```bash
cargo test -p novex-tools tool_executor_registry --offline
cargo test -p backend-rust agent_tool_executor_registry_boundary_lives_in_novex_tools --offline
```

- [x] **Step 2: Run focused verification**

Run:

```bash
cargo fmt --all -- --check
git diff --check
cargo test -p novex-tools tool_executor_registry --offline
cargo test -p backend-rust agent_tool_executor_registry_boundary_lives_in_novex_tools --offline
cargo test -p novex-tools --offline
cargo test --workspace --offline
```

- [x] **Step 3: Commit implementation**

Commit:

```bash
git add crates/novex-tools/src/lib.rs backend/src/application/ai/agent_service.rs docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-18-agent-tool-executor-registry.md
git commit -m "feat: add agent tool executor registry"
```

- [ ] **Step 4: Merge into main and verify main**

Run from main worktree:

```bash
git merge --ff-only feat/enterprise-agent-foundation
cargo fmt --all -- --check
git diff --check
cargo test --workspace --offline
```

- [ ] **Step 5: Clean workspaces**

Run:

```bash
cargo clean
(cd .worktrees/enterprise-agent-foundation && cargo clean)
```
