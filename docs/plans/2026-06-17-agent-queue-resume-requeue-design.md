# Agent Queue Resume Requeue Design

## Goal

Make approval resume for queued Agent runs go back through the durable queue instead of executing tool work inside the HTTP resume request. A queued run that pauses for human approval should keep queue lifecycle visibility, and after approval the worker should resume execution under the same claim, lease, retry, and terminalization contract as other background runs.

## Current State

Novex now supports durable queued Agent runs:

- `executionMode=queued` creates Run Graph records and one `ai_agent_run_queue` row.
- The worker claims `pending` and `retrying` queue rows.
- Queued deterministic and model-loop runs execute against the existing run id.
- Queued cancellation synchronizes `pending` and `retrying` queue rows.

The remaining gap is resume. When a queued deterministic run pauses for approval, `execute_queued_run` returns a run with `waiting_approval`, and the worker currently marks the queue row `succeeded`. Later, `resume_run` completes the pause and directly executes the approved tool inside the HTTP request. That bypasses worker leases, retry accounting, and queue observability.

## Selected Design

Introduce a queue status for approval waits:

- Add `waiting_approval` as a durable queue status.
- When `run_agent_queue_tick` receives `Ok(run)` with `run.status == "waiting_approval"`, mark the queue row `waiting_approval` instead of `succeeded`.
- `waiting_approval` rows are not claimable by the worker because claim still selects only `pending` and `retrying`.

Add a resume requeue path:

- `resume_run` keeps the existing approval validation and pause completion.
- It transitions the run to `resuming` and appends the `Resumed` event.
- It attempts to requeue the existing queue row from `waiting_approval` to `pending`.
- The requeue payload is tagged with `source = "agent.resume_run"` and carries the approved resume input.
- If a queue row was requeued, `resume_run` returns the current run without executing the tool inline.
- If no queue row was requeued, the existing inline resume behavior remains for inline runs.

Teach queued execution about resume payloads:

- `execute_queued_run` detects `source = "agent.resume_run"`.
- It transitions `resuming -> running` using the existing status path.
- It executes the approved tool input via a shared resumed-tool helper.
- The worker then marks the queue row `succeeded`, `failed`, `retrying`, or `cancelled` as usual.

## Queue Status Semantics

| Queue status | Claimable | Meaning |
| --- | --- | --- |
| pending | yes | Run is ready for a worker. |
| retrying | yes | Run is ready for a retry claim. |
| running | no | A worker owns the lease. |
| waiting_approval | no | Worker released the run because human approval is required. |
| succeeded | no | Run reached a successful terminal state. |
| failed | no | Run exhausted retries or failed terminally. |
| cancelled | no | Run was cancelled before or during execution. |

For compatibility with rows created before this slice, resume requeue can also accept `succeeded` when the run itself is still `waiting_approval`.

## Why This Shape

The queue table has a unique `(tenant_id, run_id)` constraint. Reusing the existing row and resetting it to `pending` avoids creating duplicate execution rows for the same run. It also preserves a single queue lifecycle surface for dashboards and future broker-backed wakeups.

The API does not block on tool execution after approval. This matches the direction of Codex-style supervised background execution: HTTP mutates control state; workers own long-running runtime work.

## Non-Goals

- No broker push wake-up in this slice; the embedded worker still polls.
- No schema migration for queue history or per-attempt rows.
- No provider-native abort.
- No model-loop approval pause requeue beyond the existing deterministic approval path.
- No frontend changes.

## Verification

- Repository source contract proves `waiting_approval` status and resume requeue SQL exist.
- Worker source contract proves waiting-approval runs do not become queue `succeeded`.
- Service source contract proves `resume_run` can requeue and return without inline execution.
- Existing queued run, queue runtime, cancellation, Guardian approval, and workspace tests remain green.

