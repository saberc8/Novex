# Agent Tool Router Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Move the model-loop tool registry and tool-call validation into `crates/novex-tools` so backend Agent runtime uses a shared Codex-shaped router contract.

**Architecture:** `novex-tools` owns model-visible tool definitions, router construction, duplicate/unknown validation, and routed tool-call metadata. `AgentService` keeps execution, audit, approval pause, credential resolution, and Run Graph persistence, but must ask the router before DB lookup or execution.

**Tech Stack:** Rust, serde, serde_json, Cargo offline tests.

---

## Task 1: Add `novex-tools` Router Contract

**Files:**
- Modify: `crates/novex-tools/src/lib.rs`

**Step 1: Write failing tests**

Add tests:

```rust
#[test]
fn tool_router_exposes_sorted_model_visible_specs() {
    let router = ToolRouter::from_definitions(vec![
        tool_definition("media.image.generate"),
        tool_definition("rag.search"),
    ])
    .unwrap();

    assert_eq!(
        router.tool_codes(),
        vec!["media.image.generate".to_owned(), "rag.search".to_owned()]
    );
    assert_eq!(router.model_tool_specs()[0].name, "media.image.generate");
}

#[test]
fn tool_router_rejects_duplicate_tool_codes() {
    let err = ToolRouter::from_definitions(vec![
        tool_definition("rag.search"),
        tool_definition("rag.search"),
    ])
    .unwrap_err();

    assert_eq!(err.kind, ToolRouteErrorKind::DuplicateToolCode);
    assert_eq!(err.tool_code.as_deref(), Some("rag.search"));
}

#[test]
fn tool_router_rejects_unknown_model_tool_call() {
    let router = ToolRouter::from_definitions(vec![tool_definition("rag.search")]).unwrap();

    let err = router
        .route_tool_call("call-1", "sandbox.exec", serde_json::json!({}))
        .unwrap_err();

    assert_eq!(err.kind, ToolRouteErrorKind::UnknownTool);
    assert_eq!(err.tool_code.as_deref(), Some("sandbox.exec"));
}

#[test]
fn agent_model_loop_tool_definitions_cover_builtin_agent_tools() {
    let router = ToolRouter::from_definitions(agent_model_loop_tool_definitions()).unwrap();
    let codes = router.tool_codes();

    assert!(codes.contains(&"rag.search".to_owned()));
    assert!(codes.contains(&"github.repo.search".to_owned()));
    assert!(codes.contains(&"github.repo.read".to_owned()));
    assert!(codes.contains(&"media.image.generate".to_owned()));
    assert!(codes.contains(&"feishu.message.send".to_owned()));
}
```

Add a test helper:

```rust
fn tool_definition(code: &str) -> ToolDefinition { ... }
```

Run:

```bash
cargo test -p novex-tools tool_router --offline
```

Expected: FAIL because router types/functions do not exist.

**Step 2: Implement minimal router**

Add:

```rust
pub struct ToolRouter { ... }
pub struct RoutedToolCall { ... }
pub struct ToolRouteError { ... }
pub enum ToolRouteErrorKind { EmptyToolCode, DuplicateToolCode, UnknownTool }
```

Use `BTreeMap<String, ToolDefinition>` so prompt ordering is deterministic.

Add `agent_model_loop_tool_definitions()` with definitions for:

- `rag.search`
- `github.repo.search`
- `github.repo.read`
- `media.image.generate`
- `feishu.message.send`

Keep schemas minimal but real enough for model-visible prompt contracts.

**Step 3: Verify and commit**

Run:

```bash
cargo test -p novex-tools --offline
cargo fmt -- --check
```

Commit:

```bash
git add crates/novex-tools/src/lib.rs docs/plans/2026-06-17-agent-tool-router-design.md docs/plans/2026-06-17-agent-tool-router.md
git commit -m "feat: add agent tool router contract"
```

## Task 2: Use Router in Backend Model Loop

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`
- Modify: `docs/plans/2026-06-16-codex-migration-matrix.md`

**Step 1: Write failing backend tests**

Add source/behavior tests:

```rust
#[test]
fn agent_service_model_loop_uses_novex_tool_router() {
    let source = include_str!("agent_service.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();

    assert!(source.contains("ToolRouter::from_definitions"));
    assert!(source.contains("agent_model_loop_tool_definitions"));
    assert!(source.contains("tool_router.route_tool_call"));
}

#[test]
fn model_loop_tool_router_exposes_prompt_codes() {
    let router = build_model_loop_tool_router().unwrap();

    assert!(router.tool_codes().contains(&"rag.search".to_owned()));
    assert!(router.tool_codes().contains(&"github.repo.read".to_owned()));
}
```

Run:

```bash
cargo test -p backend agent_service_model_loop_uses_novex_tool_router --offline
```

Expected: FAIL because backend still uses local tool-code list.

**Step 2: Replace local prompt code list with router**

Import:

```rust
agent_model_loop_tool_definitions, ToolRouteError, ToolRouteErrorKind, ToolRouter
```

Add:

```rust
fn build_model_loop_tool_router() -> Result<ToolRouter, ToolRouteError>
```

Use `tool_router.tool_codes()` for the system prompt and compacted message helper.

**Step 3: Route parsed tool calls before DB lookup**

Inside the tool-call branch:

```rust
let routed_call = tool_router.route_tool_call(&call_id, &tool_code, arguments.clone())?;
let tool_code = routed_call.tool.code.clone();
let arguments = routed_call.arguments;
```

Map `UnknownTool` to a failed run with `stopReason: "unknown_tool"` rather than `NotFound`.

**Step 4: Update matrix**

Change Tool router row to `slice-1 implemented` and note that registry-owned prompt/tool-call validation is in place; executor registry and parallel runtime remain next.

**Step 5: Verify and commit**

Run:

```bash
cargo test -p backend model_loop --offline
cargo test -p backend agent_service --offline
cargo fmt -- --check
```

Commit:

```bash
git add backend/src/application/ai/agent_service.rs docs/plans/2026-06-16-codex-migration-matrix.md
git commit -m "feat: route agent model tools through registry"
```

## Task 3: Final Verification

Run:

```bash
cargo fmt -- --check
cargo test -p novex-tools --offline
cargo test -p backend model_loop --offline
cargo test -p backend agent_service --offline
cargo test --workspace --offline
git status --short
```

Expected: formatting clean, selected tests pass, workspace tests pass, worktree clean.
