# Agent Response Item Ledger Design

## Goal

Persist replayable Agent turn items as a first-class ledger, closing the gap between in-memory `AgentRuntimeState.items` and Codex-style durable `ResponseItem` history.

The current runtime already projects `AgentTurnItem` history into model-loop sampling messages, but the authoritative sequence only lives in memory during execution and as embedded `ai_run_event.payload.item` values afterward. Enterprise replay, eval, customer-service QA, and NotebookLM-style notebook sessions need a stable item ledger that can be read without reverse-engineering event semantics.

## Port Mode

- Direct port: keep the Codex idea of a durable, ordered response-item history.
- Adapter port: store the history in Novex control-plane schema (`ai_agent_turn_item`) and keep `ai_run_event`, trace snapshots, rollouts, and SSE cursors intact.

## Scope

This slice adds a minimal durable item ledger:

- New table `ai_agent_turn_item`
  - One row per persisted `AgentTurnItem`.
  - Ordered by the corresponding run event `sequence_no`.
  - Links to `run_id`, optional `step_id`, and source `ai_run_event.id`.
  - Stores `item_type`, optional `call_id`, optional `tool_code`, and full normalized `item_payload`.
- Repository methods
  - `create_event_with_turn_item` inserts the run event and turn item in one transaction.
  - `list_turn_items` reads ordered item rows for replay.
- Agent runtime integration
  - `append_event` detects payloads created by `agent_turn_item_event_payload`.
  - Those payloads are persisted to the ledger automatically.
  - `get_run_trace` returns `turnItems` alongside trace events and replay summary.

## Non-Goals

- Cross-process resume from the ledger.
- Provider-native OpenAI `ResponseItem` wire shape parity.
- Changing SSE cursor behavior.
- Backfilling old runs.
- Removing or replacing `ai_run_event`.

## Acceptance

- Migration defines `ai_agent_turn_item` with run, event, sequence, item type, call/tool, and JSON payload indexes.
- Repository exposes transactional event-plus-item insert and ordered item listing.
- Tests prove `AgentTurnItem` payloads round-trip from ledger records.
- Source-contract tests prove model-loop turn-item events flow through the ledger path.
- `get_run_trace` response includes ordered `turnItems`.
- Full workspace verification passes before merge.
