# Agent Runtime Supervisor Design

## Goal

Port the next Codex task-supervision primitive into Novex by upgrading the active agent runtime registry from a cancellation signal map into a lightweight supervised run handle registry.

This slice does not introduce a durable queue or move `create_run` into a background worker. It creates the control-plane contract that later background workers, Notebook source jobs, customer-service long tasks, and code-agent sessions can share: active run metadata, cancellation state, deterministic cleanup, trace payloads, and eval tags.

## Codex Reference

Codex keeps active turn work in `RunningTask`:

- `TaskKind` identifies regular, review, and compact tasks.
- `CancellationToken` lets session abort requests interrupt active work.
- `AbortOnDropHandle` ensures spawned task cleanup when ownership ends.
- task lifecycle hooks emit turn start, abort, stop, and idle evidence.

Novex already has part of this shape:

- `AgentRuntimeRegistry` stores active `(tenant_id, run_id)` entries.
- `AgentRunCancellationToken` interrupts model calls and tool I/O.
- `POST /cancel` persists `cancel_requested` and `cancelled` events.

The missing piece is supervision metadata. Today the registry cannot answer what is active, what kind of work is running, whether cancellation was requested, or what runtime evidence should be attached to traces and eval candidates.

## Selected Design

Extend `AgentRuntimeRegistry` in `backend/src/application/ai/agent_service.rs` with an in-memory `AgentRuntimeRunState` instead of storing only a `watch::Sender<bool>`.

Each active state stores:

- tenant id and run id.
- runtime task kind, starting with `model_loop`.
- lifecycle status: `running` or `cancelling`.
- started instant for elapsed-time evidence.
- cancellation requested flag.
- cancellation signal sender.

Expose:

- `register_run_with_kind(tenant_id, run_id, task_kind)`.
- existing `register_run(...)` as a compatibility wrapper for `model_loop`.
- `cancel_run(...) -> bool` for current call sites.
- `cancel_run_signal(...) -> AgentRuntimeCancelSignal` for service code that needs trace metadata.
- `active_run_snapshots() -> Vec<AgentRuntimeRunSnapshot>` for tests and future admin/runtime APIs.

`ActiveAgentRunGuard` still unregisters on drop. This mirrors Codex ownership semantics without pretending that an HTTP request has become a durable spawned worker.

## Trace And Eval Contract

When `AgentService::cancel_run` asks the registry to cancel an active run, the persisted `Cancelled` event should include:

```json
{
  "cancelled": true,
  "runtimeSignalSent": true,
  "runtimeSupervisor": {
    "activeBeforeCancel": true,
    "taskKind": "model_loop",
    "status": "cancelling",
    "cancelRequested": true,
    "elapsedMs": 12
  }
}
```

`model_loop_external_cancel_payload(stage)` remains the checkpoint payload for cancellation discovered inside the running loop. The direct cancel endpoint event gets supervisor metadata because it is the point where the registry signal is emitted.

`novex-eval` extracts supervisor tags from cancellation events:

- `runtimeSupervisorTaskKind`
- `runtimeSupervisorCancelSignalSent`
- `runtimeSupervisorActiveBeforeCancel`

This lets rollout/eval distinguish "DB status was cancelled" from "an active runtime handle was actually signalled".

## Non-Goals

- No new SQL table.
- No background queue or scheduler.
- No provider-native HTTP abort.
- No cross-process cancellation.
- No UI endpoint for active runtime handles yet.

Those remain follow-up work once the in-process supervision contract is stable.

## Acceptance

- Registry tests prove active snapshot, cancellation state, and guard cleanup.
- AgentService tests prove cancel events include supervisor metadata.
- Trace/eval tests prove cancellation events become eval tags.
- Migration matrix records the Codex task-supervision adapter progress.
- `cargo fmt -- --check` and `cargo test --workspace --offline` pass before merging to `main`.
