# Codex Migration Matrix

| Module | Codex Source | Novex Target | Mode | Status | Notes |
| --- | --- | --- | --- | --- | --- |
| Agent protocol | `codex-rs/protocol/src/items.rs` | `crates/novex-agent-protocol` | direct/adapt | planned | Turn item and tool-call vocabulary |
| Runtime loop | `codex-rs/core/src/session/turn.rs` | `crates/novex-agent-runtime` | adapt | planned | Session/Task/Turn loop mapped to Run Graph |
| Tool schema | `codex-rs/tools/src/*` | `crates/novex-tools` | direct/adapt | planned | ToolDefinition and model-visible tool schema |
| Tool router | `codex-rs/core/src/tools/router.rs` | `crates/novex-tools` | adapt | planned | Parse model tool calls and dispatch executors |
| Parallel tools | `codex-rs/core/src/tools/parallel.rs` | `crates/novex-tools` | adapt | planned | Cancellation and non-parallel lock semantics |
| Rollout trace | `codex-rs/rollout*` | `crates/novex-trace` | adapt | deferred | Replay/eval foundation |
| MCP | `codex-rs/codex-mcp`, `rmcp-client` | `crates/novex-mcp` | adapt | deferred | MCP server/tool discovery |
| Guardian | `codex-rs/core/src/guardian` | `crates/novex-approval-review` | adapt | deferred | Automatic approval review |
| Exec policy | `codex-rs/execpolicy`, `sandboxing`, `exec-server` | `services/sandbox-runner` | service adapt | deferred | No backend shell execution |
