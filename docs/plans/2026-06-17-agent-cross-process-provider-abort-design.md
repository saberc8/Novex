# Agent Cross-Process Provider Abort Design

## Goal

Close the next cancellation gap in the Codex-style Agent runtime: a running model provider call should be interruptible even when the cancel request is issued by another HTTP/worker process.

## Current Gap

Novex already has:

- process-local `AgentRuntimeRegistry` cancellation tokens,
- DB-backed run status transitions,
- explicit checks before and after model calls/tool batches,
- cancellation trace events.

But while a process is inside an awaited provider future, a cancel request from another process only changes persistent run status. The active process does not observe that persistent status until the provider call returns. That means a long model call can keep running after the user has cancelled the run.

Codex treats turn cancellation as an active runtime signal. Novex needs the same shape for enterprise workers where HTTP and queue consumers may be separate processes.

## Selected Approach

Add a DB-backed persistent cancellation watcher around provider futures.

For each model-loop provider await:

1. keep listening to the existing process-local cancellation token;
2. concurrently poll the persistent run status;
3. if the persistent run status becomes `cancelling` or `cancelled`, return `ModelLoopFutureAwait::Cancelled`;
4. dropping the provider future aborts the in-flight reqwest call;
5. the existing cancellation finalizer records a normal `external_cancel` event.

This is a real active abort boundary without adding a new table. A later slice can replace the polling watcher with a provider-call lease table, queue notification, or provider-native cancel API while preserving the same await contract.

## Scope

In this slice:

- main model sampling calls use the persistent watcher;
- model-assisted context compaction calls use the persistent watcher;
- tests prove the watcher wins over a pending provider future;
- tests prove AgentService uses the watcher around both model and compaction provider calls;
- migration matrix records active cross-process provider abort as implemented at the persistent-run-status layer.

Out of scope:

- provider-specific cancel endpoints;
- table-backed provider call lease rows;
- active abort for arbitrary connector/MCP tool calls;
- RabbitMQ fanout notifications for cancellation.

## Acceptance

- Focused tests prove a persistent cancellation future returns `Cancelled` before a pending provider future completes.
- Source contract tests prove `model_call` and `context_compaction` use the persistent provider abort helper.
- `cargo fmt -- --check` and `cargo test --workspace --offline` pass in feature and main.
- Feature worktree is merged back into `main`.
- `cargo clean` is run for main and feature worktrees.
