# Enterprise Agent Foundation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the first production-shaped slice of Novex's enterprise Agent foundation by migrating Codex-style protocol, tool schema, model-driven loop, Run Graph events, and a real configured-model POC path.

**Architecture:** Keep Novex's RBAC, tenant, model routing, PostgreSQL Run Graph, and tool governance as the control plane. Add Codex-inspired protocol/runtime crates that map Session/Task/Turn/Item/ToolCall/Observation onto existing `ai_run`, `ai_run_step`, and `ai_run_event` tables. The first executable slice should use the configured `runtime.llm.code_agent` route and existing tools; later phases add MCP, rollout/eval, sandbox, customer service, and Notebook workspace features.

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL, serde, serde_json, reqwest, Next.js, React, TypeScript, Vitest.

---

## Scope

This plan implements Phases 0-3 from `docs/plans/2026-06-16-enterprise-agent-foundation-design.md`:

- Agent protocol kernel.
- Tool spec/schema adapter.
- Model-driven loop POC using configured model routes.
- Backend integration with existing Run Graph.
- `apps/codex-app-poc` real run UI.

Deferred to follow-up plans:

- MCP Gateway 2.0 stdio process supervisor, OAuth callback/state integration, refresh-token execution, persisted sessions, and deployed external MCP server smoke hardening.
- Rollout/trace/eval platform.
- NotebookLM workspace.
- Smart customer-service flow template.
- Sandbox runner and code patch execution.

## Guardrails

- Do not bypass `novex-model`; all LLM calls go through configured tenant model routes.
- Do not replace `ai_run.status` with a second status machine.
- Do not execute shell commands from `backend`.
- Preserve Codex Apache-2.0 attribution when direct code is copied or substantially derived.
- Keep existing deterministic `AgentService` behavior available until the new runtime path is stable.

### Task 1: Add Codex Attribution and Migration Tracking

**Files:**
- Create: `NOTICE`
- Create: `docs/plans/2026-06-16-codex-migration-matrix.md`
- Modify: `docs/plans/2026-06-16-enterprise-agent-foundation-design.md`

**Step 1: Write the migration matrix document**

Create `docs/plans/2026-06-16-codex-migration-matrix.md` with this content:

```markdown
# Codex Migration Matrix

| Module | Codex Source | Novex Target | Mode | Status | Notes |
| --- | --- | --- | --- | --- | --- |
| Agent protocol | `codex-rs/protocol/src/items.rs` | `crates/novex-agent-protocol` | direct/adapt | planned | Turn item and tool-call vocabulary |
| Runtime loop | `codex-rs/core/src/session/turn.rs` | `crates/novex-agent-runtime` | adapt | planned | Session/Task/Turn loop mapped to Run Graph |
| Tool schema | `codex-rs/tools/src/*` | `crates/novex-tools` | direct/adapt | planned | ToolDefinition and model-visible tool schema |
| Tool router | `codex-rs/core/src/tools/router.rs` | `crates/novex-tools` | adapt | planned | Parse model tool calls and dispatch executors |
| Parallel tools | `codex-rs/core/src/tools/parallel.rs` | `crates/novex-tools` | adapt | planned | Cancellation and non-parallel lock semantics |
| Rollout trace | `codex-rs/rollout*` | `crates/novex-trace` | adapt | slice-1 implemented | Trace bundle, replay API, `ai_rollout`, eval capture, and trace-backed eval gate exist; richer inference/compaction spans remain follow-up |
| MCP | `codex-rs/codex-mcp`, `rmcp-client` | `crates/novex-mcp` | adapt | slice-8 implemented | Tenant-governed registration, discovery, model-visible tool mapping, audit path, mock/dry-run invocation, Streamable HTTP request/response contract, gated backend live HTTP dispatch, offline local live-server smoke coverage, stdio launch/lifecycle contract, OAuth authorization-code/PKCE plan contract, OAuth token exchange/session contract, and backend token HTTP dispatch adapter exist; stdio process supervisor, OAuth callback/state integration, refresh-token execution, persisted sessions, and deployed external MCP server smoke coverage remain follow-up |
| Guardian | `codex-rs/core/src/guardian` | `crates/novex-approval-review` | adapt | deferred | Automatic approval review |
| Exec policy | `codex-rs/execpolicy`, `sandboxing`, `exec-server` | `services/sandbox-runner` | service adapt | deferred | No backend shell execution |
```
```

**Step 2: Add NOTICE**

If `NOTICE` does not exist, create it:

```text
Novex

This project may include code derived from OpenAI Codex, licensed under the Apache License, Version 2.0.
OpenAI Codex
Copyright 2025 OpenAI

Derived Codex modules must be tracked in docs/plans/2026-06-16-codex-migration-matrix.md.
```

**Step 3: Link the matrix from the design doc**

Append to `docs/plans/2026-06-16-enterprise-agent-foundation-design.md` under `Codex Migration Matrix`:

```markdown
The living implementation tracker is `docs/plans/2026-06-16-codex-migration-matrix.md`.
```

**Step 4: Verify**

Run:

```bash
rg "OpenAI Codex|Codex Migration Matrix|living implementation tracker" NOTICE docs/plans/2026-06-16-*.md
```

Expected: the attribution and matrix link are present.

**Step 5: Commit**

```bash
git add NOTICE docs/plans/2026-06-16-codex-migration-matrix.md docs/plans/2026-06-16-enterprise-agent-foundation-design.md
git commit -m "docs: track codex infrastructure migration"
```

### Task 2: Create Agent Protocol Crate

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/novex-agent-protocol/Cargo.toml`
- Create: `crates/novex-agent-protocol/src/lib.rs`

**Step 1: Write the failing tests**

Create `crates/novex-agent-protocol/src/lib.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn turn_item_serializes_with_snake_case_type_tags() {
        let item = AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"}));
        let value = serde_json::to_value(item).unwrap();

        assert_eq!(value["type"], "tool_call");
        assert_eq!(value["callId"], "call-1");
        assert_eq!(value["toolCode"], "rag.search");
    }

    #[test]
    fn tool_observation_links_to_call_id() {
        let item = AgentTurnItem::tool_observation(
            "call-1",
            ToolObservationStatus::Succeeded,
            json!({"hits": 2}),
        );

        assert_eq!(item.call_id(), Some("call-1"));
        assert!(item.requires_follow_up());
    }

    #[test]
    fn turn_outcome_identifies_terminal_states() {
        assert!(TurnOutcome::Final.is_terminal());
        assert!(TurnOutcome::Paused.is_terminal());
        assert!(!TurnOutcome::NeedsFollowUp.is_terminal());
    }
}
```

Run:

```bash
cargo test -p novex-agent-protocol --offline
```

Expected: FAIL because the crate is not yet in the workspace.

**Step 2: Add workspace member**

Modify root `Cargo.toml`:

```toml
members = [
    "backend",
    "crates/novex-ai-core",
    "crates/novex-agent-protocol",
    "crates/novex-model",
    ...
]

[workspace.dependencies]
novex-agent-protocol = { path = "crates/novex-agent-protocol" }
```

**Step 3: Add crate manifest**

Create `crates/novex-agent-protocol/Cargo.toml`:

```toml
[package]
name = "novex-agent-protocol"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json = "1"
```

**Step 4: Implement minimal protocol types**

Replace `crates/novex-agent-protocol/src/lib.rs` with:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CRATE_ID: &str = "novex-agent-protocol";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTurnItemType {
    UserMessage,
    AssistantMessage,
    Reasoning,
    ToolCall,
    ToolObservation,
    FinalAnswer,
    ContextCompaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolObservationStatus {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnOutcome {
    NeedsFollowUp,
    Final,
    Paused,
    Cancelled,
    Failed,
    BudgetExceeded,
}

impl TurnOutcome {
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::NeedsFollowUp)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentTurnItem {
    UserMessage {
        content: String,
    },
    AssistantMessage {
        content: String,
    },
    Reasoning {
        summary: String,
    },
    ToolCall {
        #[serde(rename = "callId")]
        call_id: String,
        #[serde(rename = "toolCode")]
        tool_code: String,
        arguments: Value,
    },
    ToolObservation {
        #[serde(rename = "callId")]
        call_id: String,
        status: ToolObservationStatus,
        output: Value,
    },
    FinalAnswer {
        content: String,
    },
    ContextCompaction {
        summary: String,
    },
}

impl AgentTurnItem {
    pub fn user_message(content: impl Into<String>) -> Self {
        Self::UserMessage {
            content: content.into(),
        }
    }

    pub fn assistant_message(content: impl Into<String>) -> Self {
        Self::AssistantMessage {
            content: content.into(),
        }
    }

    pub fn tool_call(call_id: impl Into<String>, tool_code: impl Into<String>, arguments: Value) -> Self {
        Self::ToolCall {
            call_id: call_id.into(),
            tool_code: tool_code.into(),
            arguments,
        }
    }

    pub fn tool_observation(
        call_id: impl Into<String>,
        status: ToolObservationStatus,
        output: Value,
    ) -> Self {
        Self::ToolObservation {
            call_id: call_id.into(),
            status,
            output,
        }
    }

    pub fn call_id(&self) -> Option<&str> {
        match self {
            Self::ToolCall { call_id, .. } | Self::ToolObservation { call_id, .. } => Some(call_id),
            _ => None,
        }
    }

    pub fn requires_follow_up(&self) -> bool {
        matches!(self, Self::ToolObservation { .. } | Self::ContextCompaction { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn turn_item_serializes_with_snake_case_type_tags() {
        let item = AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"}));
        let value = serde_json::to_value(item).unwrap();

        assert_eq!(value["type"], "tool_call");
        assert_eq!(value["callId"], "call-1");
        assert_eq!(value["toolCode"], "rag.search");
    }

    #[test]
    fn tool_observation_links_to_call_id() {
        let item = AgentTurnItem::tool_observation(
            "call-1",
            ToolObservationStatus::Succeeded,
            json!({"hits": 2}),
        );

        assert_eq!(item.call_id(), Some("call-1"));
        assert!(item.requires_follow_up());
    }

    #[test]
    fn turn_outcome_identifies_terminal_states() {
        assert!(TurnOutcome::Final.is_terminal());
        assert!(TurnOutcome::Paused.is_terminal());
        assert!(!TurnOutcome::NeedsFollowUp.is_terminal());
    }
}
```

**Step 5: Verify**

Run:

```bash
cargo test -p novex-agent-protocol --offline
cargo test --workspace --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add Cargo.toml crates/novex-agent-protocol
git commit -m "feat: add codex-style agent protocol kernel"
```

### Task 3: Add Model-visible Tool Specs

**Files:**
- Modify: `crates/novex-tools/src/lib.rs`
- Modify: `crates/novex-tools/Cargo.toml`

**Step 1: Write failing tests**

Append tests to `crates/novex-tools/src/lib.rs`:

```rust
#[test]
fn tool_definition_converts_to_model_visible_spec() {
    let tool = ToolDefinition {
        code: "rag.search".to_owned(),
        name: "Search knowledge".to_owned(),
        description: "Search tenant-scoped knowledge base.".to_owned(),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": { "type": "string" }
            }
        }),
        output_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "hits": { "type": "array" }
            }
        })),
        risk_level: ToolRiskLevel::Low,
        approval_policy: ApprovalPolicy::OnRisk,
        permission_code: Some("ai:knowledge:ask".to_owned()),
    };

    let spec = tool.to_model_tool_spec();

    assert_eq!(spec.name, "rag.search");
    assert_eq!(spec.parameters["required"][0], "query");
    assert_eq!(spec.metadata["riskLevel"], "low");
}
```

Run:

```bash
cargo test -p novex-tools tool_definition_converts_to_model_visible_spec --offline
```

Expected: FAIL because `ToolDefinition` and `ModelToolSpec` do not exist.

**Step 2: Add serde_json dependency if missing**

In `crates/novex-tools/Cargo.toml`, ensure:

```toml
serde.workspace = true
serde_json = "1"
```

**Step 3: Implement tool spec types**

Add to `crates/novex-tools/src/lib.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub code: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    pub risk_level: ToolRiskLevel,
    pub approval_policy: ApprovalPolicy,
    pub permission_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub output_schema: Option<Value>,
    pub metadata: Value,
}

impl ToolDefinition {
    pub fn to_model_tool_spec(&self) -> ModelToolSpec {
        ModelToolSpec {
            name: self.code.clone(),
            description: self.description.clone(),
            parameters: self.input_schema.clone(),
            output_schema: self.output_schema.clone(),
            metadata: serde_json::json!({
                "displayName": self.name,
                "riskLevel": tool_risk_code(self.risk_level),
                "approvalPolicy": approval_policy_code(self.approval_policy),
                "permissionCode": self.permission_code
            }),
        }
    }
}

pub fn tool_risk_code(risk: ToolRiskLevel) -> &'static str {
    match risk {
        ToolRiskLevel::Low => "low",
        ToolRiskLevel::Medium => "medium",
        ToolRiskLevel::High => "high",
    }
}

pub fn approval_policy_code(policy: ApprovalPolicy) -> &'static str {
    match policy {
        ApprovalPolicy::Never => "never",
        ApprovalPolicy::OnRisk => "on_risk",
        ApprovalPolicy::Always => "always",
    }
}
```

**Step 4: Verify**

Run:

```bash
cargo test -p novex-tools --offline
cargo test --workspace --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/novex-tools/Cargo.toml crates/novex-tools/src/lib.rs
git commit -m "feat: expose model-visible tool specs"
```

### Task 4: Add Agent Runtime Crate Skeleton

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/novex-agent-runtime/Cargo.toml`
- Create: `crates/novex-agent-runtime/src/lib.rs`

**Step 1: Write failing tests**

Create tests in `crates/novex-agent-runtime/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use novex_agent_protocol::{AgentTurnItem, ToolObservationStatus, TurnOutcome};
    use serde_json::json;

    #[test]
    fn runtime_state_continues_after_observation() {
        let mut state = AgentRuntimeState::new("run-1");
        state.push_item(AgentTurnItem::user_message("search policy"));
        state.push_item(AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"})));
        state.push_item(AgentTurnItem::tool_observation(
            "call-1",
            ToolObservationStatus::Succeeded,
            json!({"hits": []}),
        ));

        assert_eq!(state.next_outcome(), TurnOutcome::NeedsFollowUp);
        assert_eq!(state.tool_call_count(), 1);
    }

    #[test]
    fn runtime_budget_stops_excessive_tool_calls() {
        let budget = AgentRuntimeBudget {
            max_turns: 4,
            max_tool_calls: 1,
        };
        let mut state = AgentRuntimeState::with_budget("run-1", budget);
        state.push_item(AgentTurnItem::tool_call("call-1", "rag.search", json!({})));
        state.push_item(AgentTurnItem::tool_call("call-2", "rag.search", json!({})));

        assert_eq!(state.next_outcome(), TurnOutcome::BudgetExceeded);
    }
}
```

Run:

```bash
cargo test -p novex-agent-runtime --offline
```

Expected: FAIL because the crate is missing.

**Step 2: Add workspace member and dependency**

In root `Cargo.toml` add:

```toml
"crates/novex-agent-runtime",
```

and:

```toml
novex-agent-runtime = { path = "crates/novex-agent-runtime" }
```

**Step 3: Add crate manifest**

Create `crates/novex-agent-runtime/Cargo.toml`:

```toml
[package]
name = "novex-agent-runtime"
version.workspace = true
edition.workspace = true

[dependencies]
novex-agent-protocol.workspace = true
novex-tools.workspace = true
serde.workspace = true
serde_json = "1"
```

**Step 4: Implement runtime state**

Create `crates/novex-agent-runtime/src/lib.rs`:

```rust
use novex_agent_protocol::{AgentTurnItem, TurnOutcome};
use serde::{Deserialize, Serialize};

pub const CRATE_ID: &str = "novex-agent-runtime";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeBudget {
    pub max_turns: usize,
    pub max_tool_calls: usize,
}

impl Default for AgentRuntimeBudget {
    fn default() -> Self {
        Self {
            max_turns: 8,
            max_tool_calls: 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRuntimeState {
    pub run_ref: String,
    pub budget: AgentRuntimeBudget,
    pub items: Vec<AgentTurnItem>,
}

impl AgentRuntimeState {
    pub fn new(run_ref: impl Into<String>) -> Self {
        Self::with_budget(run_ref, AgentRuntimeBudget::default())
    }

    pub fn with_budget(run_ref: impl Into<String>, budget: AgentRuntimeBudget) -> Self {
        Self {
            run_ref: run_ref.into(),
            budget,
            items: Vec::new(),
        }
    }

    pub fn push_item(&mut self, item: AgentTurnItem) {
        self.items.push(item);
    }

    pub fn tool_call_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item, AgentTurnItem::ToolCall { .. }))
            .count()
    }

    pub fn turn_count(&self) -> usize {
        self.items
            .iter()
            .filter(|item| matches!(item, AgentTurnItem::UserMessage { .. } | AgentTurnItem::ToolObservation { .. }))
            .count()
    }

    pub fn next_outcome(&self) -> TurnOutcome {
        if self.tool_call_count() > self.budget.max_tool_calls || self.turn_count() > self.budget.max_turns {
            return TurnOutcome::BudgetExceeded;
        }
        if self.items.last().is_some_and(AgentTurnItem::requires_follow_up) {
            return TurnOutcome::NeedsFollowUp;
        }
        TurnOutcome::Final
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novex_agent_protocol::{AgentTurnItem, ToolObservationStatus, TurnOutcome};
    use serde_json::json;

    #[test]
    fn runtime_state_continues_after_observation() {
        let mut state = AgentRuntimeState::new("run-1");
        state.push_item(AgentTurnItem::user_message("search policy"));
        state.push_item(AgentTurnItem::tool_call("call-1", "rag.search", json!({"query":"policy"})));
        state.push_item(AgentTurnItem::tool_observation(
            "call-1",
            ToolObservationStatus::Succeeded,
            json!({"hits": []}),
        ));

        assert_eq!(state.next_outcome(), TurnOutcome::NeedsFollowUp);
        assert_eq!(state.tool_call_count(), 1);
    }

    #[test]
    fn runtime_budget_stops_excessive_tool_calls() {
        let budget = AgentRuntimeBudget {
            max_turns: 4,
            max_tool_calls: 1,
        };
        let mut state = AgentRuntimeState::with_budget("run-1", budget);
        state.push_item(AgentTurnItem::tool_call("call-1", "rag.search", json!({})));
        state.push_item(AgentTurnItem::tool_call("call-2", "rag.search", json!({})));

        assert_eq!(state.next_outcome(), TurnOutcome::BudgetExceeded);
    }
}
```

**Step 5: Verify**

Run:

```bash
cargo test -p novex-agent-runtime --offline
cargo test --workspace --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add Cargo.toml crates/novex-agent-runtime
git commit -m "feat: add agent runtime state skeleton"
```

### Task 5: Add Model Tool Call Parser

**Files:**
- Modify: `crates/novex-agent-runtime/src/lib.rs`

**Step 1: Write failing tests**

Add tests:

```rust
#[test]
fn parser_reads_json_tool_call_from_model_answer() {
    let parsed = parse_model_turn_output(r#"{"type":"tool_call","callId":"call-1","toolCode":"rag.search","arguments":{"query":"policy"}}"#).unwrap();

    assert_eq!(
        parsed.item,
        AgentTurnItem::tool_call("call-1", "rag.search", serde_json::json!({"query":"policy"}))
    );
    assert_eq!(parsed.outcome, TurnOutcome::NeedsFollowUp);
}

#[test]
fn parser_treats_plain_text_as_final_answer() {
    let parsed = parse_model_turn_output("Here is the answer.").unwrap();

    assert_eq!(parsed.item, AgentTurnItem::FinalAnswer { content: "Here is the answer.".to_owned() });
    assert_eq!(parsed.outcome, TurnOutcome::Final);
}
```

Run:

```bash
cargo test -p novex-agent-runtime parser_reads_json_tool_call_from_model_answer --offline
```

Expected: FAIL because parser is missing.

**Step 2: Implement parser**

Add:

```rust
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedModelTurnOutput {
    pub item: AgentTurnItem,
    pub outcome: TurnOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTurnParseError {
    pub message: String,
}

pub fn parse_model_turn_output(output: &str) -> Result<ParsedModelTurnOutput, ModelTurnParseError> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err(ModelTurnParseError {
            message: "model output is empty".to_owned(),
        });
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if value.get("type").and_then(Value::as_str) == Some("tool_call") {
            let call_id = value
                .get("callId")
                .and_then(Value::as_str)
                .unwrap_or("call-1")
                .to_owned();
            let tool_code = value
                .get("toolCode")
                .and_then(Value::as_str)
                .ok_or_else(|| ModelTurnParseError {
                    message: "toolCode is required".to_owned(),
                })?
                .to_owned();
            let arguments = value.get("arguments").cloned().unwrap_or(Value::Null);
            return Ok(ParsedModelTurnOutput {
                item: AgentTurnItem::tool_call(call_id, tool_code, arguments),
                outcome: TurnOutcome::NeedsFollowUp,
            });
        }
    }

    Ok(ParsedModelTurnOutput {
        item: AgentTurnItem::FinalAnswer {
            content: trimmed.to_owned(),
        },
        outcome: TurnOutcome::Final,
    })
}
```

**Step 3: Verify**

Run:

```bash
cargo test -p novex-agent-runtime --offline
cargo test --workspace --offline
```

Expected: PASS.

**Step 4: Commit**

```bash
git add crates/novex-agent-runtime/src/lib.rs
git commit -m "feat: parse model agent turn outputs"
```

### Task 6: Add Backend Runtime Event Mapping

**Files:**
- Modify: `backend/Cargo.toml`
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Add dependencies**

In `backend/Cargo.toml`:

```toml
novex-agent-protocol.workspace = true
novex-agent-runtime.workspace = true
```

**Step 2: Write failing tests**

Add to `backend/src/application/ai/agent_service.rs` tests:

```rust
#[test]
fn agent_runtime_event_payload_preserves_turn_item_shape() {
    let item = novex_agent_protocol::AgentTurnItem::tool_call(
        "call-1",
        "rag.search",
        serde_json::json!({"query":"policy"}),
    );
    let payload = agent_turn_item_event_payload(&item);

    assert_eq!(payload["item"]["type"], "tool_call");
    assert_eq!(payload["item"]["callId"], "call-1");
    assert_eq!(payload["eventSource"], "novex-agent-runtime");
}
```

Run:

```bash
cargo test -p backend agent_runtime_event_payload_preserves_turn_item_shape --offline
```

Expected: FAIL because helper does not exist.

**Step 3: Implement event helper**

Add near helper functions in `agent_service.rs`:

```rust
fn agent_turn_item_event_payload(item: &novex_agent_protocol::AgentTurnItem) -> Value {
    json!({
        "eventSource": "novex-agent-runtime",
        "item": serde_json::to_value(item).unwrap_or(Value::Null),
    })
}
```

**Step 4: Verify**

Run:

```bash
cargo test -p backend agent_runtime_event_payload_preserves_turn_item_shape --offline
cargo test --workspace --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/Cargo.toml backend/src/application/ai/agent_service.rs
git commit -m "feat: map agent runtime items to run events"
```

### Task 7: Add Model-driven Runtime Path Behind a Flag

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Extend command**

Modify `AgentRunCommand`:

```rust
#[serde(default)]
pub runtime_mode: Option<String>,
```

**Step 2: Write failing tests**

Add:

```rust
#[test]
fn agent_run_command_accepts_model_runtime_mode() {
    let command: AgentRunCommand = serde_json::from_value(serde_json::json!({
        "input": "search policy",
        "runtimeMode": "model_loop"
    }))
    .unwrap();

    assert_eq!(command.runtime_mode.as_deref(), Some("model_loop"));
}

#[test]
fn model_loop_prompt_mentions_available_tool_schema() {
    let prompt = build_model_loop_system_prompt(&["rag.search".to_owned()]);

    assert!(prompt.contains("You are Novex Agent Runtime"));
    assert!(prompt.contains("rag.search"));
    assert!(prompt.contains("\"type\":\"tool_call\""));
}
```

Run:

```bash
cargo test -p backend model_loop_prompt_mentions_available_tool_schema --offline
```

Expected: FAIL.

**Step 3: Implement prompt helper**

Add:

```rust
fn build_model_loop_system_prompt(tool_codes: &[String]) -> String {
    format!(
        "You are Novex Agent Runtime. You may either answer directly or request one tool call. Available tools: {}. To call a tool, reply with compact JSON exactly like {{\"type\":\"tool_call\",\"callId\":\"call-1\",\"toolCode\":\"rag.search\",\"arguments\":{{\"query\":\"...\"}}}}. Otherwise reply with the final answer.",
        tool_codes.join(", ")
    )
}
```

**Step 4: Add runtime branch in `create_run`**

At the start of `create_run`, after command normalization:

```rust
if command.runtime_mode.as_deref() == Some("model_loop") {
    return self.create_model_loop_run(user_id, command).await;
}
```

Add a temporary `create_model_loop_run` that creates the run, calls `runtime.llm.code_agent`, parses the first model output, records the parsed item, and finishes when the item is final. For this task, if the model returns a tool call, record it and return `waiting_approval` or `succeeded` with a dry-run note; the full observation loop arrives in Task 8.

**Step 5: Verify**

Run:

```bash
cargo test -p backend agent_run_command_accepts_model_runtime_mode --offline
cargo test -p backend model_loop_prompt_mentions_available_tool_schema --offline
cargo test --workspace --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: add model-loop agent runtime mode"
```

### Task 8: Implement Tool Observation Follow-up Loop

**Files:**
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests for pure helper behavior:

```rust
#[test]
fn observation_prompt_includes_tool_result_and_final_answer_instruction() {
    let prompt = build_observation_follow_up_prompt(
        "rag.search",
        &serde_json::json!({"hits":[{"title":"Policy"}]}),
    );

    assert!(prompt.contains("rag.search"));
    assert!(prompt.contains("Policy"));
    assert!(prompt.contains("final answer"));
}
```

Run:

```bash
cargo test -p backend observation_prompt_includes_tool_result_and_final_answer_instruction --offline
```

Expected: FAIL.

**Step 2: Implement helper**

Add:

```rust
fn build_observation_follow_up_prompt(tool_code: &str, observation: &Value) -> String {
    format!(
        "Tool `{tool_code}` returned this observation:\n{}\nUse it to produce the final answer. If the observation is insufficient, say what is missing.",
        serde_json::to_string_pretty(observation).unwrap_or_else(|_| "{}".to_owned())
    )
}
```

**Step 3: Implement loop behavior**

In `create_model_loop_run`:

1. Create user message event.
2. Call `ModelRuntimeService::chat_completion_for_purpose(ModelRoutePurpose::CodeAgent, ...)`.
3. Parse output with `parse_model_turn_output`.
4. If final answer: write final event and succeed.
5. If tool call:
   - Find tool by code.
   - Apply existing tool policy.
   - Pause if approval required.
   - Execute tool if allowed.
   - Record tool call and observation events.
   - Call model again with observation prompt.
   - Parse final output.
   - Finish run.

Keep `max_tool_calls` to 1 in this task. Multi-tool loops come later.

**Step 4: Verify**

Run:

```bash
cargo test -p backend observation_prompt_includes_tool_result_and_final_answer_instruction --offline
cargo test -p backend --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/agent_service.rs
git commit -m "feat: complete first model-driven tool observation loop"
```

### Task 9: Expose Runtime Mode in Agent Workspace API Types

**Files:**
- Modify: `apps/agent-workspace/src/types/agent.ts`
- Modify: `apps/agent-workspace/src/api/agent.test.ts`
- Modify: `apps/codex-app-poc/package.json`
- Create: `apps/codex-app-poc/src/lib/api.ts`
- Create: `apps/codex-app-poc/src/types/agent.ts`
- Create: `apps/codex-app-poc/src/api/agent.ts`
- Create: `apps/codex-app-poc/src/api/agent.test.ts`

**Step 1: Write failing tests**

In `apps/codex-app-poc/src/api/agent.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import { createAgentRun } from "./agent";

describe("codex poc agent api", () => {
  it("sends model loop runtime mode", async () => {
    const fetchMock = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        code: "200",
        data: { runId: 1, status: "succeeded", traceId: "agent-1" }
      })
    }));
    vi.stubGlobal("fetch", fetchMock);

    await createAgentRun({ input: "search policy", runtimeMode: "model_loop" });

    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining("/ai/agents/runs"),
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ input: "search policy", runtimeMode: "model_loop" })
      })
    );
  });
});
```

Run:

```bash
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts
```

Expected: FAIL because API files do not exist.

**Step 2: Implement API helper**

Create `apps/codex-app-poc/src/lib/api.ts`:

```ts
const API_BASE_URL = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:4398";

export async function apiRequest<T>(path: string, init: RequestInit = {}): Promise<T> {
  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(init.headers ?? {})
    }
  });
  const body = await response.json();
  if (!response.ok || body.code !== "200") {
    throw new Error(body.message ?? "Request failed");
  }
  return body.data as T;
}
```

Create `apps/codex-app-poc/src/types/agent.ts`:

```ts
export type AgentRunCommand = {
  input: string;
  runtimeMode?: "model_loop";
  autoApprove?: boolean;
  budget?: {
    maxSteps?: number;
    maxToolCalls?: number;
    maxSeconds?: number;
    maxCostCents?: number;
  };
};

export type AgentRunResp = {
  runId: number;
  traceId: string;
  status: string;
  intent?: string;
  selectedToolCode?: string | null;
  finalOutput?: string | null;
};
```

Create `apps/codex-app-poc/src/api/agent.ts`:

```ts
import { apiRequest } from "@/lib/api";
import type { AgentRunCommand, AgentRunResp } from "@/types/agent";

export function createAgentRun(data: AgentRunCommand) {
  return apiRequest<AgentRunResp>("/ai/agents/runs", {
    method: "POST",
    body: JSON.stringify(data)
  });
}
```

**Step 3: Verify**

Run:

```bash
cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts
cd apps/codex-app-poc && pnpm typecheck
```

Expected: PASS.

**Step 4: Commit**

```bash
git add apps/agent-workspace/src/types/agent.ts apps/agent-workspace/src/api/agent.test.ts apps/codex-app-poc/src
git commit -m "feat: add codex poc agent api client"
```

### Task 10: Connect Codex POC UI to Real Runs

**Files:**
- Modify: `apps/codex-app-poc/src/app-client.tsx`
- Modify: `apps/codex-app-poc/app/page.test.tsx`

**Step 1: Write failing UI tests**

Add tests:

```tsx
it("submits composer input as model loop agent run", async () => {
  const fetchMock = vi.fn(async () => ({
    ok: true,
    json: async () => ({
      code: "200",
      data: {
        runId: 42,
        traceId: "agent-42",
        status: "succeeded",
        finalOutput: "Done"
      }
    })
  }));
  vi.stubGlobal("fetch", fetchMock);

  render(<CodexPocApp />);
  await userEvent.type(screen.getByLabelText("任务输入"), "search policy");
  await userEvent.click(screen.getByLabelText("发送"));

  expect(fetchMock).toHaveBeenCalled();
  expect(await screen.findByText("Done")).toBeInTheDocument();
});
```

Run:

```bash
cd apps/codex-app-poc && pnpm test -- app/page.test.tsx
```

Expected: FAIL because send button is local-only.

**Step 2: Implement submit state**

In `apps/codex-app-poc/src/app-client.tsx`:

- Import `createAgentRun`.
- Track `isSubmitting`, `runResult`, `runError`.
- On send click:
  - reject empty input locally.
  - call `createAgentRun({ input: composerValue, runtimeMode: "model_loop", autoApprove: false, budget: { maxSteps: 8, maxToolCalls: 1, maxSeconds: 60, maxCostCents: 0 } })`.
  - render final output or status under composer.

**Step 3: Verify**

Run:

```bash
cd apps/codex-app-poc && pnpm test
cd apps/codex-app-poc && pnpm typecheck
cd apps/codex-app-poc && pnpm lint
cd apps/codex-app-poc && pnpm build
```

Expected: PASS.

**Step 4: Commit**

```bash
git add apps/codex-app-poc/src/app-client.tsx apps/codex-app-poc/app/page.test.tsx
git commit -m "feat: connect codex poc composer to agent runtime"
```

### Task 11: Add Smoke Script for Configured Model POC

**Files:**
- Create: `scripts/smoke-agent-model-loop.sh`
- Modify: `scripts/run-poc.sh`
- Modify: `infra/.env.poc.example`

**Step 1: Create smoke script**

Create `scripts/smoke-agent-model-loop.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://localhost:4398}"
TOKEN="${TOKEN:-}"

if [[ -z "${TOKEN}" ]]; then
  echo "TOKEN is required" >&2
  exit 2
fi

curl -fsS \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"input":"search the training handbook for customer data policy","runtimeMode":"model_loop","autoApprove":false,"budget":{"maxSteps":8,"maxToolCalls":1,"maxSeconds":60,"maxCostCents":0}}' \
  "${BASE_URL}/ai/agents/runs"
```

**Step 2: Document env**

Add to `infra/.env.poc.example`:

```bash
# Codex-style Agent POC uses runtime.llm.code_agent through the existing LLM route.
AGENT_RUNTIME_MODE=model_loop
```

**Step 3: Verify shell syntax**

Run:

```bash
bash -n scripts/smoke-agent-model-loop.sh
```

Expected: PASS.

**Step 4: Commit**

```bash
git add scripts/smoke-agent-model-loop.sh infra/.env.poc.example
git commit -m "chore: add model-loop agent smoke script"
```

### Task 12: Full Verification

**Files:**
- No new files.

**Step 1: Run Rust checks**

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

Expected: PASS.

**Step 2: Run POC frontend checks**

Run:

```bash
cd apps/codex-app-poc && pnpm test && pnpm typecheck && pnpm lint && pnpm build
```

Expected: PASS.

**Step 3: Run agent workspace checks**

Run:

```bash
cd apps/agent-workspace && pnpm test && pnpm typecheck && pnpm lint && pnpm build
```

Expected: PASS.

**Step 4: Optional live smoke**

If backend is running and an admin JWT is available:

```bash
TOKEN="$ADMIN_TOKEN" ./scripts/smoke-agent-model-loop.sh
```

Expected: response contains `runId`, `traceId`, and a terminal or approval status.

**Step 5: Commit any verification-only fixes**

If formatting or test fixes were needed:

```bash
git add <changed files>
git commit -m "test: verify enterprise agent foundation slice"
```

## Follow-up Plans

After this first slice passes, create separate implementation plans:

- `2026-06-16-agent-mcp-gateway.md`
- `2026-06-16-agent-rollout-eval.md`
- `2026-06-16-notebook-workspace.md`
- `2026-06-16-customer-service-agent.md`
- `2026-06-16-sandbox-runner.md`
