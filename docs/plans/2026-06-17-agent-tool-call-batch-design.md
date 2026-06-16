# Agent Tool Call Batch Design

## Goal

Move Novex from one tool call per model turn to a Codex-shaped batch contract: a single model response may request multiple tool calls, the runtime parses them as ordered turn items, budget checks happen before execution, and backend events record the selected batch plan.

This slice is a bridge from `ToolConcurrencyPolicy` to true parallel execution. It should not claim background async execution yet.

## Current State

Novex already has:

- `AgentTurnItem::ToolCall` and `ToolObservation`.
- `parse_model_turn_output` for a single `{"type":"tool_call"}` payload.
- `ToolRouter::route_tool_call`.
- `ToolBatchPlan` with `parallel` / `serial` planning based on tool concurrency policy.
- backend `runtimeMode=model_loop` that executes one routed tool call at a time.

The gap is that `ParsedModelTurnOutput` returns one item. Backend cannot see a tool-call batch, cannot reject a batch that exceeds remaining budget before side effects, and cannot record `batchExecutionMode`.

## Options

### Option A: Full Async Executor Now

Parse multiple calls and execute `parallel` plans concurrently in backend. This is attractive, but it touches approval, audit ordering, connector timeout handling, cancellation, and observation aggregation at once.

### Option B: Batch Parse and Serial Executor

Parse multiple calls, route them through `ToolRouter`, create `ToolBatchPlan`, record the plan in events, then execute the calls in plan order. If the plan is `parallel`, the event says it was eligible for parallel execution; actual execution remains serial in this slice.

This is the selected design. It makes the final runtime shape more true without hiding current limits.

### Option C: Prompt-Only Multi-Call

Only change the prompt to ask for multiple calls. This would be misleading because the backend would still parse only one call.

## Selected Design

`novex-agent-runtime` will keep `ParsedModelTurnOutput.item` for compatibility and add `items: Vec<AgentTurnItem>`. Single-call and final-answer outputs will set `items` to one element. Batch outputs will set `item` to the first tool call and `items` to every parsed call.

Supported model output:

```json
{
  "type": "tool_calls",
  "calls": [
    {
      "callId": "call-1",
      "toolCode": "rag.search",
      "arguments": { "query": "policy" }
    },
    {
      "callId": "call-2",
      "toolCode": "github.repo.read",
      "arguments": { "repository": "org/repo", "path": "README.md" }
    }
  ]
}
```

`AgentRuntimeState` will add a remaining-budget helper so backend can reject a batch before executing partial side effects.

Backend model loop will:

- detect parsed tool-call batches.
- reject empty batches as parse errors.
- reject batches that exceed remaining tool-call budget before routing or execution.
- route every call with `ToolRouter`.
- create `ToolBatchPlan`.
- include `toolCallBatch`, `batchExecutionMode`, and `serialReason` on `ActionSelected` events.
- continue executing in order for this slice.

## Non-Goals

- No tokio concurrent execution yet.
- No cancellation-token propagation yet.
- No batch approval UX yet; if a selected call requires approval, the existing single-tool approval path still pauses at that call.

## Testing

Runtime:

- parser reads `type=tool_calls` into two tool-call items.
- parser rejects `type=tool_calls` when `calls` is missing or empty.
- runtime budget reports remaining tool-call capacity.

Backend:

- source-level test confirms model loop reads `parsed.items`.
- source-level test confirms backend creates `ToolBatchPlan`.
- prompt test confirms canonical `tool_calls` batch JSON is advertised.

Matrix:

- Runtime loop notes batch parsing and serial batch execution.
- Parallel tools notes that batch planning is wired into model-loop events, while true async execution remains next.
