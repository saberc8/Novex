# Enterprise Agent Foundation Design

## Goal

Novex 的最终目标是成为企业级 Agent 基建，而不是单个聊天应用、单个 RAG 应用或某个低代码工作流系统。它需要长期服务：

- Chat Flow 和可配置对话应用。
- POC 快速交付。
- 智能客服和坐席辅助。
- 企业知识库问答。
- NotebookLM-like 资料工作台。
- 研发助手和代码库 Agent。
- 企业内部工具自动化。

截至 2026-06-16，Novex 已经具备 RBAC 控制面、模型路由、RAG、Run Graph、工具审计、基础 Agent POC 和 Codex-like 前端 POC。真正缺口在于：Agent Runtime 还不是模型驱动的多轮 tool loop，工具协议还不是模型可见的一等 contract，trace/eval/rollout 还没有形成可回放和可评测闭环，代码执行和 MCP/sandbox 还没有企业级隔离面。

设计原则是：尽可能以根目录 `codex/` 源码为上游工程基准迁移 Agent 基建，不凭空手写玩具版运行时。Novex 保留企业控制面、租户、权限、模型路由、PostgreSQL 资产和客户交付模板；Codex 提供 Agent Runtime、tool protocol、event stream、approval、rollout/trace、sandbox/exec policy、MCP 等核心基建参考。

## Current State

Novex 已有能力：

- `backend`: Rust/Axum/SQLx，已有认证、RBAC、系统管理、调度、文件、AI 模块 API。
- `crates/novex-ai-core`: 已有 RunStatus、RunEventKind、TaskBudget 等基础领域类型。
- `crates/novex-agent`: 已有 deterministic intent router、tool selector 和 ReAct plan skeleton。
- `crates/novex-model`: 已有模型类型、provider、route purpose、OpenAI-compatible runtime route、token/cost usage。
- `backend/src/application/ai/model_service.rs`: 已有 chat/completions、embedding、rerank、模型健康检查、聊天历史和用量记录。
- `backend/src/application/ai/agent_service.rs`: 已有 Agent run、approval pause/resume/cancel、GitHub/Feishu/image tool 执行、model-loop POC 和事件记录。
- `apps/codex-app-poc`: 已能从 Codex-like composer 提交 `runtimeMode=model_loop` 的真实 Agent run。
- `apps/agent-workspace`: 已有 Agent run API client 和最小执行视图。

Codex 可迁移能力：

- `codex-rs/protocol`: Submission/Event、TurnItem、approval、permission、tool call、user input、thread/session protocol。
- `codex-rs/core/src/session/turn.rs`: Session/Task/Turn loop、模型采样、needs_follow_up、tool output 回灌、context compact。
- `codex-rs/tools` 和 `codex-rs/core/src/tools`: tool spec、Responses API tool schema、tool router、parallel tool runtime、tool output contract。
- `codex-rs/rollout` 和 `codex-rs/rollout-trace`: event rollout、trace bundle、tool dispatch trace、inference trace。
- `codex-rs/execpolicy`、`sandboxing`、`exec-server`: command policy、sandbox、exec isolation。
- `codex-rs/codex-mcp`、`rmcp-client`: MCP server 连接、tool discovery、resource/tool invocation。
- `codex-rs/core/src/guardian`: approval auto-review、approval risk/authorization 判断和 circuit breaker。

## Target Architecture

```text
Customer Apps
  chat-web / training-web / agent-workspace / notebook workspace / customer service
        |
Agent Application Layer
  chat flow / customer-service policy / notebook source workspace / code workspace
        |
Agent Runtime Layer
  session / task / turn / item / tool call / observation / compact / approval
        |
Tool and Knowledge Layer
  tool registry / MCP gateway / connectors / RAG / file search / media / sandbox runner
        |
Model Gateway
  tenant route / provider adapter / usage / cost / health / fallback
        |
Run Graph and Trace
  ai_run / ai_run_step / ai_run_event / ai_run_pause / rollout / replay / eval record
        |
Control Plane
  tenant / RBAC / resource ACL / secret / quota / audit / template / delivery
```

Novex 的运行时核心应从“确定性 service 调工具”升级为“模型驱动的 Turn loop”：

```text
User input / trigger
  -> create ai_run
  -> create agent session/task/turn
  -> resolve model route, tools, knowledge context, memory
  -> call configured model route
  -> receive assistant item or tool call
  -> execute tool under policy
  -> append observation as model input
  -> continue until final answer, pause, cancel, budget stop, or failure
  -> persist rollout, trace, usage, eval hooks
```

POC 阶段指定模型使用现有配置路由：

- 普通聊天: `runtime.llm.chat`
- RAG 回答: `runtime.llm.rag_answer`
- Code/Agent loop: `runtime.llm.code_agent`
- Eval judge: `runtime.llm.eval_judge`
- Embedding: `runtime.embedding`
- Rerank: `runtime.reranker`
- Image: `runtime.draw`

不新增 provider，不绕开 `novex-model`。

## Codex Migration Matrix

| Codex source | Novex target | Migration mode | Purpose |
| --- | --- | --- | --- |
| `codex-rs/protocol/src/items.rs` | `crates/novex-agent-protocol` or `novex-ai-core` | direct port then adapt names | Turn item, agent message, tool call item, file change item, context compaction item |
| `codex-rs/protocol/src/protocol.rs` | `novex-agent-protocol` | selective port | Submission/Event model, approval events, turn lifecycle events |
| `codex-rs/core/src/session/turn.rs` | `crates/novex-agent-runtime` | adapter port | model sampling loop, follow-up loop, tool output回灌, compact trigger |
| `codex-rs/tools/src/*` | `crates/novex-tools` | direct port for schema layer | ToolDefinition, ToolSpec, Responses API tool schema, JSON schema policy |
| `codex-rs/core/src/tools/router.rs` | `crates/novex-tools` | adapter port | Build tool call from model output, dispatch to registry |
| `codex-rs/core/src/tools/parallel.rs` | `crates/novex-tools` | adapter port | parallel/cancel semantics for tool runtime |
| `codex-rs/rollout` | `crates/novex-trace` | direct port then DB adapter | rollout event persistence and replay |
| `codex-rs/rollout-trace` | `crates/novex-trace` | direct port then DB adapter | inference, MCP, compaction, tool dispatch trace |
| `codex-rs/core/src/guardian` | `crates/novex-approval-review` | adapter port | high-risk tool auto-review and approval safety |
| `codex-rs/codex-mcp`, `rmcp-client` | `crates/novex-mcp` | adapter port | MCP server lifecycle, tool discovery, invocation, auth |
| `codex-rs/execpolicy`, `sandboxing`, `exec-server` | `services/sandbox-runner` + `crates/novex-sandbox-policy` | service adapter | command execution, sandbox, network/file permissions |
| `codex-rs/core/src/compact*` | `crates/novex-agent-runtime` | selective port | context compaction and context window management |
| `codex-rs/feedback` | `crates/novex-eval` | concept port | feedback diagnostics feeding eval datasets |

Direct ports must preserve license attribution. Codex is Apache-2.0; Novex should add a NOTICE entry when code is copied or substantially derived.

The living implementation tracker is `docs/plans/2026-06-16-codex-migration-matrix.md`.

## Core Runtime Model

Novex should introduce a Codex-like runtime vocabulary:

- `AgentSession`: tenant/app/user/workspace/model/tool configuration and conversation state.
- `AgentTask`: one user request or trigger execution, mapped to one `ai_run`.
- `AgentTurn`: one model sampling cycle inside a task.
- `AgentItem`: persisted item stream: user message, assistant message, reasoning, tool call, observation, file change, compaction.
- `ToolCall`: model-requested tool invocation with call id, name, namespace, arguments, approval context.
- `Observation`: tool result converted into model-readable input for the next turn.
- `TurnOutcome`: final, needs_follow_up, paused, cancelled, failed, compacted.

Current `ai_run`, `ai_run_step`, `ai_run_event`, `ai_run_pause` remain the storage backbone. New runtime types should map to existing tables first, then add tables only when required:

- `ai_agent_thread`: optional future persistent conversation/workspace thread.
- `ai_agent_turn`: optional indexed turn table when event-only storage becomes hard to query.
- `ai_agent_item`: optional item table for NotebookLM/code workspace timeline.
- `ai_rollout`: optional persisted event bundle for replay/eval.

Do not create a second independent status machine. `ai_run.status` stays authoritative.

## Tool System

The current AgentService has hardcoded tool handling. Target design:

1. `ai_tool.input_schema` and `ai_tool.output_schema` become model-visible specs.
2. `novex-tools` converts DB tools and built-in tools into Codex-like `ToolSpec`.
3. Model output is parsed as function/tool calls.
4. Tool executor runs under policy:
   - permission check
   - risk/approval check
   - tenant credential resolution
   - timeout/budget
   - audit
   - trace
5. Tool result is converted into observation and appended to the next model turn.

First-class tool families:

- Knowledge: `rag.search`, `rag.read_citation`, `dataset.search`.
- Notebook: `workspace.source.list`, `workspace.source.read`, `notebook.note.create`.
- Customer service: `customer.lookup`, `ticket.create`, `handoff.request`, `faq.search`.
- Connectors: `github.repo.search`, `github.repo.read`, `feishu.message.send`.
- Media: `media.image.generate`.
- MCP: discovered external MCP tools.
- Sandbox/code: `file.search`, `file.read_range`, `sandbox.exec`, `patch.apply` later.

Risk model:

- Low: read-only retrieval and source preview.
- Medium: external GET, media generation, connector read with cost.
- High: send message, write issue, create ticket, execute command, mutate customer data.

High-risk tools must pause unless an explicit policy and reviewer allow auto-review.

## Knowledge and NotebookLM Direction

NotebookLM-like capability should be modeled as a workspace over source sets, not as a chat skin.

Core entities:

- workspace
- source set
- source document
- citation graph
- generated notes
- study guide / FAQ / summary artifacts
- question-answer sessions
- eval/feedback per answer

Runtime chain:

```text
source upload/import
  -> parser worker
  -> blocks/chunks/citations
  -> embedding + sparse index
  -> source-aware retrieval
  -> rerank
  -> grounded answer with citation spans
  -> note/artifact generation
  -> feedback/eval case capture
```

This reuses RAG infrastructure, but adds workspace state, citations, notes, and artifacts.

## Chat Flow and Customer Service Direction

Chat Flow should be built on Run Graph and Agent Runtime, not as a separate workflow engine.

Chat Flow contains:

- deterministic pre/post steps: auth, session load, policy selection, safety check, feedback capture.
- agentic middle: intent classification, retrieval, tool use, response.
- escalation: human handoff and ticket creation.
- channel adapters: web chat, API, Feishu, future IM integrations.

Customer service requires:

- knowledge grounded answer.
- customer context lookup with strict permission.
- policy-based reply style.
- hallucination guard: answer must cite or admit insufficient evidence.
- handoff trigger.
- ticket/work-order tool.
- conversation quality eval.

## Eval and Trace

Codex has loop/rollout/trace foundations but not a full enterprise eval platform. Novex should build enterprise eval on top:

Eval dimensions:

- RAG relevance.
- citation correctness.
- groundedness / hallucination.
- customer-service resolution.
- handoff correctness.
- tool selection accuracy.
- tool execution success.
- approval policy correctness.
- cost and latency.
- regression safety across templates.

Eval data flow:

```text
run event / rollout / feedback
  -> eval case candidate
  -> curated eval dataset
  -> eval run
  -> judge model or deterministic metric
  -> report
  -> CI gate / release gate
```

`novex-eval` should own eval dataset, eval case, eval runner, metric aggregation, report generation, and CI gate contracts. It should consume `novex-trace`, not duplicate runtime event storage.

## Phased Delivery

### Phase 0: Migration Control and Documentation

Deliverables:

- Codex migration matrix in docs.
- License/NOTICE policy for direct ports.
- Define module boundaries and crate names.
- Mark current deterministic AgentService as legacy POC path.

Acceptance:

- Every migrated module has source mapping and test strategy.
- No runtime code is copied without attribution.

### Phase 1: Agent Protocol Kernel

Deliverables:

- `novex-agent-protocol` or expanded `novex-ai-core` types:
  - Session, Task, Turn, Item, ToolCall, Observation, TurnOutcome.
  - Event names aligned with Codex where useful.
- Mapping functions to `ai_run_event`.
- Tests for serialization, event mapping, status transitions.

Acceptance:

- Run Graph can represent a Codex-like turn/item timeline.
- Existing AgentService tests still pass.

### Phase 2: Model-driven Agent Loop POC

Deliverables:

- New runtime path uses configured `runtime.llm.code_agent`.
- Minimal prompt with available tool specs.
- JSON/function-call compatible model output parser.
- Tool call -> tool executor -> observation -> next model turn.
- Budget enforcement: max turns, max tool calls, max seconds, max cost.

Acceptance:

- POC can run one real model-driven loop using existing model config.
- `rag.search` and one read-only connector can be called as observations.
- Final answer is produced by the model after observation, not by hardcoded service text.

### Phase 3: POC UI Integration

Deliverables:

- `apps/codex-app-poc` sends real Agent run requests.
- Event timeline displays turn started, model message, tool call, observation, final answer.
- Approval UI supports pause/resume/cancel.
- Model route visible but controlled by backend.

Acceptance:

- User can run an Agent task from the Codex-like UI.
- Refresh can reconstruct state from event snapshot.

### Phase 4: Tool Registry 2.0 and MCP

Deliverables:

- DB-backed tool specs converted to model-visible schemas.
- Tool router decoupled from hardcoded AgentService branches.
- MCP server discovery and invocation through `novex-mcp`.
- Tool audit, permission, risk, approval, trace unified.

Acceptance:

- Adding a built-in tool does not require editing the agent loop.
- MCP tools can be discovered and surfaced under tenant policy.

### Phase 5: Trace, Rollout, Replay, Eval Foundation

Deliverables:

- `novex-trace` inspired by Codex rollout/rollout-trace.
- Run replay API.
- Eval case capture from real runs.
- Basic eval runner with deterministic and model-judge metrics.

Acceptance:

- Any Agent run can be replayed into a readable timeline.
- Eval can run against saved traces and produce a report.

### Phase 6: Enterprise Knowledge and Customer Service

Deliverables:

- Grounded RAG Agent with citation policy.
- Customer service flow template.
- Handoff and ticket tools.
- Admin eval report for answer quality and handoff accuracy.

Acceptance:

- A customer-service POC can answer with citations, create ticket/handoff, and produce eval records.

### Phase 7: Notebook Workspace

Deliverables:

- Source workspace model.
- Notebook-like source list, citation graph, generated notes/artifacts.
- Study guide/FAQ/summary generation.

Acceptance:

- User can upload/import source material, ask cited questions, and generate notes/artifacts from sources.

### Phase 8: Code Agent and Sandbox Runner

Deliverables:

- `services/sandbox-runner`.
- File search/read range.
- Sandboxed command execution.
- Patch proposal/apply flow with approval.
- Code workspace UI integration.

Acceptance:

- Code Agent can search/read a repo and propose changes safely.
- Command execution is isolated from backend and audited.

## Non-goals for the First Cut

- Full visual workflow builder.
- Full plugin marketplace.
- Multi-agent supervisor-worker as the default path.
- Direct shell execution inside backend.
- Replacing Novex model registry with Codex provider config.

## Quality Bar

Every phase must include:

- Unit tests for domain/state logic.
- Integration tests for API/event behavior.
- Golden or snapshot tests for protocol/event mapping where useful.
- Migration tests for SQL schema when tables are added.
- Smoke test for POC path.
- Trace/eval hooks for user-facing agent behavior.

The point is not just to make an agent answer once. The point is to make every answer inspectable, recoverable, permissioned, replayable, and improvable.
