# Eval Real Scheduler Design

## Goal

Replace synchronous or prompt-derived eval execution with a real asynchronous scheduler and worker path. The API creates eval runs and per-case tasks, while workers execute Novex's live RAG, model route, and later Agent/tool paths and persist reproducible results.

## Scope

The first implementation slice covers:

- async run creation for existing eval datasets;
- durable per-case task rows;
- durable outbox publication to a dedicated eval queue;
- an eval worker runtime with task leasing, retry, and run aggregation;
- `live_rag` execution through `KnowledgeService::ask_dataset_for_tenant`;
- deterministic execution retained only for smoke and unit coverage;
- run status and result APIs compatible with the current Admin page.

Later slices add `live_agent`, direct tool execution, judge replay, human annotation, and release gate APIs.

## Architecture

Eval follows the same durable scheduling pattern already used by the parser pipeline:

1. `POST /ai/evals/runs` validates the dataset and enabled cases.
2. The service creates `ai_eval_run` with `queued` status.
3. The service creates one `ai_eval_task` per enabled case.
4. The service writes one `ai_eval_outbox` event per task.
5. An in-process publisher scans pending outbox rows and publishes messages to `novex.eval.execute`.
6. `eval-worker` consumes task messages, leases the task, executes the real target path, stores `ai_eval_result`, marks the task terminal, and aggregates the run.

The HTTP request never executes cases. This makes eval runs safe for CI, cron, and customer-triggered regressions.

## Runtime Modes

- `deterministic`: builds local expected outputs. This is kept for minimal smoke and local unit tests.
- `live_rag`: calls the real Novex knowledge ask path and records answer, citations, latency, and trace metadata.
- `model_route`: planned follow-up for live route health and cost tests.
- `live_agent`: planned follow-up for agent/task/tool path evaluation.
- `judge_replay`: planned follow-up for regrading stored traces without re-executing business actions.

## Data Model

`ai_eval_run` remains the run summary table. Existing status values expand from only `succeeded` to:

- `queued`
- `running`
- `succeeded`
- `failed`
- `cancelled`

`ai_eval_task` stores execution state for a single case in a run. It includes snapshots of input, expected payload, tags, run mode, task attempts, lease owner, trace references, and terminal errors.

`ai_eval_outbox` stores publishable task messages. This prevents lost work if the API process dies after creating a run but before publishing all task messages.

`ai_eval_result` remains the scored case output. Actual payloads include enough route and trace metadata to reproduce what happened.

## Worker Semantics

The worker must be idempotent:

- leasing a task succeeds only from `queued` or retryable stale states;
- duplicate RabbitMQ deliveries do not create duplicate results;
- terminal tasks are acknowledged and ignored;
- retryable execution failures requeue until `max_attempts`;
- non-retryable or exhausted failures become `dead`/failed task results;
- every task completion attempts run aggregation.

Run aggregation computes total cases, passed cases, failed cases, average score, metric breakdown, and final status once all tasks are terminal.

## Scheduling

Manual and CI entrypoints call the existing eval run API.

Nightly or release schedules use the existing scheduler built-in path. A scheduler job with `builtin_key = eval.run_dataset` creates a run from payload:

```json
{
  "datasetCode": "rag_regression_live",
  "runMode": "live_rag"
}
```

The scheduler creates runs only. Eval queue workers execute tasks.

## Failure Handling

The worker records provider errors, missing knowledge dataset metadata, RAG permission errors, timeout, and scoring errors on the task and result. `live_rag` cases must declare `knowledgeDatasetId` in dataset metadata, case tags, expected payload, or input payload.

If a task fails after all retries, the run can still aggregate with a failed result instead of hanging forever.

## Testing

Tests focus on the scheduling contract before live provider coverage:

- run creation creates `queued` run/task/outbox records and does not execute case scoring inline;
- outbox messages serialize task identity and run mode;
- worker helper refuses duplicate terminal tasks;
- `live_rag` task execution delegates to the knowledge ask path through an injectable executor boundary;
- run aggregation waits for all tasks and computes status only when terminal.

Live provider tests stay opt-in through existing environment gates.
