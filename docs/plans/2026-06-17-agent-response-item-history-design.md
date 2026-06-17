# Agent Response Item History Projection Design

## Goal

Move the backend model loop closer to Codex's turn-state model by making typed `AgentTurnItem` history the source used for the next model sample.

Before this slice, `execute_model_loop_existing_run` stored typed runtime items for events, approval review, and compaction, but it also maintained a separate mutable `messages: Vec<ModelChatMessage>` prompt transcript. That made compaction, tool observations, and future streaming/replay work depend on two histories staying aligned.

## Scope

This slice adds a backend prompt projection layer:

- Build model-loop provider messages from `runtime_state.items`.
- Project `UserMessage`, `AssistantMessage`, `Reasoning`, `ToolCall`, `ToolObservation`, `FinalAnswer`, and `ContextCompaction` into chat-compatible `ModelChatMessage` entries.
- Serialize consecutive `ToolCall` items back into canonical single/batch tool-call JSON.
- Turn consecutive `ToolObservation` items into one follow-up prompt with call id, tool code, status, and observation payload.
- When a `ContextCompaction` item exists, resume sampling from the latest compaction window by keeping the original user request, the latest compaction summary, and only items after that summary.
- Remove the model loop's parallel mutable `messages` transcript.

## Non-Goals

- Persist provider-native Responses item ids.
- Add WebSocket token streaming.
- Add provider-native cancel endpoints.
- Change queue, approval, or trace storage schemas.
- Implement unary `/responses/compact` parity.

## Acceptance

- Unit tests prove typed history projects tool calls and observations into follow-up provider messages.
- Unit tests prove the latest compaction window excludes pre-compaction observation payloads while preserving the original user request and summary.
- A source-contract test proves `execute_model_loop_existing_run` calls `build_model_loop_messages_from_history(&command.input, &tool_codes, &runtime_state.items)` and no longer pushes to a mutable prompt transcript.
- Migration matrix records this as the ResponseItem-history projection slice and leaves durable provider-native ResponseItem replay parity as follow-up work.
