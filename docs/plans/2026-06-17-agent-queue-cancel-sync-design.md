# Agent Queue Cancel Sync Design

## Goal

Make cancellation of a queued Agent run visible in the durable queue before a worker claims it. When `POST /ai/agents/runs/:run_id/cancel` is called for a run that is still `queued`, the matching `ai_agent_run_queue` row should become terminal `cancelled` immediately instead of remaining `pending` or `retrying` until a later worker tick discovers the run is already terminal.

## Current State

Novex now has durable `executionMode=queued` Agent runs:

- HTTP creates Run Graph records with `RunStatus::Queued`.
- `ai_agent_run_queue` stores claimable queue rows.
- The embedded worker claims `pending` and `retrying` rows with a Postgres lease.
- The worker marks queue rows `succeeded`, `failed`, `retrying`, or `cancelled` after execution returns.
- `cancel_run` updates `ai_run` through `cancelling -> cancelled`, appends events, cancels active pauses, and signals the process-local runtime registry.

The gap is queue lifecycle synchronization. If a queued run is cancelled while its queue row is still `pending` or `retrying`, the run becomes terminal but the queue row remains claimable until a worker claims it later. That creates stale operational state and an unnecessary future claim.

## Selected Design

Add a repository method that cancels only not-yet-running queue rows by run id:

- Match by `tenant_id` and `run_id`.
- Update only `queue_status IN ('pending', 'retrying')`.
- Set `queue_status = 'cancelled'`.
- Clear `locked_by` and `locked_until`.
- Set `finished_at` and `update_time`.
- Preserve an existing `last_error`; otherwise record a short cancellation reason.
- Return affected row count for observability and future metrics, but treat zero rows as a valid no-op.

Call this method from `AgentService::cancel_run` after the cancellation request has been recorded and before the run is finalized as `cancelled`.

## Why Not Cancel Running Queue Rows Here

Running queue rows are owned by the worker lease. The cancellation API should not mark a `running` queue row terminal out from under the worker because that can hide the real execution outcome and race with worker retry/failure handling.

For running model-loop jobs, the existing DB-backed cancellation checkpoints and process-local runtime signal let the active worker observe cancellation. When execution returns, the worker marks the queue row terminal. Cross-process provider abort remains a separate runtime-control slice.

## Semantics

| Run state | Queue state before cancel | Queue action |
| --- | --- | --- |
| queued | pending | mark queue row `cancelled` immediately |
| queued/running | retrying | mark queue row `cancelled` immediately |
| running | running | leave queue row owned by worker |
| terminal | succeeded/failed/cancelled | no-op |

## Non-Goals

- No broker wake-up or push-based worker signaling in this slice.
- No provider HTTP abort.
- No distributed cancellation token.
- No queue reaper or stale lease recovery beyond the existing claim lease.
- No UI changes.

## Verification

- Repository source contract proves the cancel-by-run method updates only `pending` and `retrying` queue rows.
- Service source contract proves `cancel_run` calls the queue cancel method.
- Existing queue, runtime-supervisor, and external-cancel tests remain green.

