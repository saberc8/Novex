# Codex Migration Matrix

| Module | Codex Source | Novex Target | Mode | Status | Notes |
| --- | --- | --- | --- | --- | --- |
| Agent protocol | `codex-rs/protocol/src/items.rs` | `crates/novex-agent-protocol` | direct/adapt | slice-1 implemented | Turn item, tool call, observation, and final answer vocabulary are in place |
| Runtime loop | `codex-rs/core/src/session/turn.rs`, `codex-rs/core/src/compact*` | `crates/novex-agent-runtime` | adapt | slice-5 implemented | Budget-bounded multi-turn model loop maps model output, single/batch tool calls, observations, context compaction, final answer, and DB-backed external cancellation checkpoints to Run Graph events; remote/model compaction and in-memory task tokens remain next |
| Tool schema | `codex-rs/tools/src/*` | `crates/novex-tools` | direct/adapt | slice-1 implemented | ToolDefinition and model-visible tool schema are in place |
| Tool router | `codex-rs/core/src/tools/router.rs`, `codex-rs/core/src/tools/registry.rs` | `crates/novex-tools` | adapt | slice-1 implemented | Registry-owned model-visible specs, tool-call validation, unknown-tool stop reason, and backend model-loop routing are in place; executor registry and parallel runtime remain next |
| Parallel tools | `codex-rs/core/src/tools/parallel.rs` | `crates/novex-tools` + backend model loop | adapt | slice-4 implemented | Shared/exclusive lock policy, cancellation-wait metadata, deterministic batch planning, parsed tool-call batches, backend batch event visibility, true parallel tool I/O, and timeout-driven cancelled tool observations are in place; provider abort and task-token propagation remain next |
| Rollout trace | `codex-rs/rollout*` | `crates/novex-trace` | adapt | slice-1 implemented | TraceBundle, replay API, `ai_rollout`, eval case capture, and `trace_replay` eval gate are in place; inference/MCP/compaction spans remain next |
| MCP | `codex-rs/codex-mcp`, `rmcp-client` | `crates/novex-mcp` | adapt | slice-1 implemented | Tenant-governed server registration, deterministic discovery, model-visible tool mapping, audit path, and mock/dry-run invocation are in place; live streaming MCP client remains next |
| Guardian | `codex-rs/core/src/guardian` | `crates/novex-approval-review` | adapt | deferred | Automatic approval review |
| Exec policy | `codex-rs/execpolicy`, `sandboxing`, `exec-server` | `services/sandbox-runner` | service adapt | deferred | No backend shell execution |

## Current Acceptance Evidence

Updated on 2026-06-17 from branch `feat/enterprise-agent-foundation`.

| Slice | Current acceptance evidence | Verification command |
| --- | --- | --- |
| Agent protocol | `crates/novex-agent-protocol` serializes turn items, tool calls, observations, and terminal outcomes | `cargo test -p novex-agent-protocol --offline` |
| Runtime loop POC | `runtimeMode=model_loop` uses configured `runtime.llm.code_agent`, parses single and batch model tool-call output, executes budget-bounded tool calls, records observations, compacts accumulated context after tool observations, and keeps sampling until final answer, approval pause, external cancellation checkpoint, or budget stop | `cargo test -p novex-agent-runtime --offline && cargo test -p backend-rust external_cancel --offline && cargo test -p backend-rust model_loop --offline` |
| Tool router | `novex-tools` owns built-in agent tool definitions, model-visible specs, duplicate/unknown validation, and backend model-loop route checks before DB lookup/execution | `cargo test -p novex-tools --offline && cargo test -p backend-rust model_loop --offline` |
| Parallel tools | `novex-tools` exposes shared/exclusive locks, cancellation-wait metadata, deterministic batch planning, and backend `ActionSelected` `concurrencyPolicy`, `batchExecutionMode`, `toolCallBatch`, plus parallel tool I/O with serial audit/step persistence and timeout-driven `Cancelled` observations | `cargo test -p novex-tools --offline && cargo test -p backend-rust tool_io_timeout --offline && cargo test -p backend-rust parallel_tool --offline && cargo test -p backend-rust model_loop --offline` |
| Codex POC UI | `apps/codex-app-poc` sends real Agent run requests with `runtimeMode=model_loop` | `cd apps/codex-app-poc && pnpm test -- src/api/agent.test.ts` |
| MCP gateway | MCP tools can be registered, discovered, converted to `ai_tool`, audited, and routed through Agent observations | `cargo test -p backend-rust mcp_ agent_runtime_routes_mcp_tools_through_audited_observation_path --offline` |
| Rollout/trace/eval | Agent events convert to trace bundles, replay via API, persist `ai_rollout`, capture eval candidates, and score `trace_replay` eval runs | `cargo test -p backend-rust agent_run_events_convert_to_trace_bundle eval_case_capture eval_runtime_normalizes_trace_replay_run_mode --offline` |
| Full Rust workspace | Rust crates and backend remain coherent after the agent foundation slices | `cargo fmt -- --check && cargo test --workspace --offline` |

## Follow-up Implementation Plans

- MCP gateway: `docs/plans/2026-06-16-agent-mcp-gateway.md`
- Rollout, trace, replay, eval: `docs/plans/2026-06-16-agent-rollout-eval.md`
- Notebook workspace: `docs/plans/2026-06-16-notebook-workspace.md`
- Customer service agent: `docs/plans/2026-06-16-customer-service-agent.md`
- Agent run cancellation checkpoints: `docs/plans/2026-06-17-agent-run-cancellation.md`
