# Agent Parallel Tools Design

## Goal

Add the first Codex-shaped parallel tool contract to Novex. The immediate goal is not to pretend the backend can already execute multiple model tool calls concurrently; it is to make tool concurrency, exclusive locks, and cancellation behavior explicit in `crates/novex-tools`, then surface that policy in `runtimeMode=model_loop` events.

## Codex Reference

Codex's `core/src/tools/parallel.rs` uses a runtime lock:

- tools that support parallel calls take a shared/read lock.
- tools that do not support parallel calls take an exclusive/write lock.
- cancellation distinguishes fast abort from tools that must wait for runtime teardown.

`core/src/tools/router.rs` and `core/src/tools/registry.rs` expose this metadata through the router, not through ad hoc service code.

## Current Gap

Novex now has:

- `ToolRouter` in `novex-tools`.
- registry-owned model-visible definitions.
- backend route validation before DB lookup/execution.

But every tool is still effectively the same from a runtime scheduling perspective. The event stream cannot yet explain whether a selected tool is safe to parallelize, should take an exclusive lock, or must wait for cancellation cleanup.

## Options

### Option A: Implement Full Async Parallel Execution Now

This would require model output support for multiple tool calls, backend execution batching, cancellation tokens, result ordering, and Run Graph step grouping. It is too wide for one safe slice.

### Option B: Add Router-Owned Concurrency Policy First

`novex-tools` adds a `ToolConcurrencyPolicy` to `ToolDefinition`, with router helpers and a deterministic batch planner. Backend records the selected policy in `ActionSelected`. Actual execution stays serial for this slice.

This is the recommended slice because it establishes a reusable contract for agent runtime, MCP, NotebookLM, customer service, and sandbox tools without lying about runtime maturity.

### Option C: Backend-Only Flags

This is small, but it repeats the pre-router problem: scheduling behavior stays trapped in `AgentService` and cannot be shared by future runtimes.

## Selected Design

Implement Option B.

`novex-tools` will add:

- `ToolExecutionLock`: `shared` or `exclusive`.
- `ToolConcurrencyPolicy`: lock kind, `supports_parallel_calls`, `waits_for_runtime_cancellation`, optional `exclusive_group`.
- `ToolBatchExecutionMode`: `parallel` or `serial`.
- `ToolBatchPlan`: deterministic plan for a set of routed calls.

Default policy is conservative serial/exclusive. Read-only low-risk built-ins such as `rag.search`, `github.repo.search`, and `github.repo.read` opt into shared parallel execution. Mutating or costful tools such as `media.image.generate` and `feishu.message.send` stay exclusive.

Backend model loop will:

- read the selected tool policy from the routed call.
- write `concurrencyPolicy` into `ActionSelected`.
- continue executing one tool at a time until the runtime parser supports multiple tool calls.

## Testing

`novex-tools`:

- read-only tools plan as parallel.
- a non-parallel tool forces a serial batch plan.
- duplicate exclusive groups force a serial plan.
- agent model-loop definitions expose expected policies.

Backend:

- source-level test confirms `ActionSelected` records `concurrencyPolicy`.
- model-loop tests remain green.

This moves the matrix from `planned` to `slice-1 implemented`; full parallel execution remains next.
