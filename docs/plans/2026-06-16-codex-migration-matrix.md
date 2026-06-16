# Codex Migration Matrix

| Module | Codex Source | Novex Target | Mode | Status | Notes |
| --- | --- | --- | --- | --- | --- |
| Agent protocol | `codex-rs/protocol/src/items.rs` | `crates/novex-agent-protocol` | direct/adapt | slice-1 implemented | Turn item, tool call, observation, and final answer vocabulary are in place |
| Runtime loop | `codex-rs/core/src/session/turn.rs` | `crates/novex-agent-runtime` | adapt | slice-1 implemented | One-tool model loop maps to Run Graph events; multi-turn loop and compaction remain next |
| Tool schema | `codex-rs/tools/src/*` | `crates/novex-tools` | direct/adapt | slice-1 implemented | ToolDefinition and model-visible tool schema are in place |
| Tool router | `codex-rs/core/src/tools/router.rs` | `crates/novex-tools` | adapt | partial | Model output parser and backend dispatch path exist; registry-driven router is next |
| Parallel tools | `codex-rs/core/src/tools/parallel.rs` | `crates/novex-tools` | adapt | planned | Cancellation and non-parallel lock semantics |
| Rollout trace | `codex-rs/rollout*` | `crates/novex-trace` | adapt | planned | Replay/eval foundation; see `2026-06-16-agent-rollout-eval.md` |
| MCP | `codex-rs/codex-mcp`, `rmcp-client` | `crates/novex-mcp` | adapt | planned | MCP server/tool discovery; see `2026-06-16-agent-mcp-gateway.md` |
| Guardian | `codex-rs/core/src/guardian` | `crates/novex-approval-review` | adapt | deferred | Automatic approval review |
| Exec policy | `codex-rs/execpolicy`, `sandboxing`, `exec-server` | `services/sandbox-runner` | service adapt | deferred | No backend shell execution |

## Follow-up Implementation Plans

- MCP gateway: `docs/plans/2026-06-16-agent-mcp-gateway.md`
- Rollout, trace, replay, eval: `docs/plans/2026-06-16-agent-rollout-eval.md`
- Notebook workspace: `docs/plans/2026-06-16-notebook-workspace.md`
- Customer service agent: `docs/plans/2026-06-16-customer-service-agent.md`
