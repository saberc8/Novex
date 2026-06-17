# Agent Provider Compact Unary Design

## Goal

Add Codex-style unary `/responses/compact` provider transport parity to Novex Agent compaction without replacing the existing Responses v2 `compaction_trigger` path.

Codex keeps two remote compaction shapes:

- legacy unary `POST /responses/compact`, where the response body is `{ "output": [...] }`;
- Responses v2 `POST /responses` with a trailing `{ "type": "compaction_trigger" }` input item and streamed completion events.

Novex already implements the v2-shaped provider request plan and parser. The remaining parity gap is the unary endpoint.

## Port Mode

- Direct port: endpoint shape, unary response body contract, and no `compaction_trigger` item.
- Adapter port: reuse Novex `ModelChatCommand`, `ModelChatRequestMetadata`, and `ModelChatResp` instead of importing Codex `CompactionInput` and `ResponseItem` wholesale.

## Scope

This slice adds a second compaction provider transport:

- `ResponsesCompactUnary`
  - selected only when `ModelChatCompactionMetadata.implementation = "responses_compaction_unary"`;
  - endpoint: `route.base_url() + "/responses/compact"`;
  - payload: `model`, `input`, optional `instructions`, `tools`, `parallel_tool_calls`, provider metadata, and optional text controls later;
  - no `stream`;
  - no final `compaction_trigger`;
  - parser: existing JSON compaction body parser.
- Existing `ResponsesCompactionV2`
  - remains selected by `implementation = "responses_compaction_v2"`;
  - keeps `/responses`, `stream = true`, and the final `compaction_trigger`.

## Non-Goals

- WebSocket Responses transport.
- `x-codex-turn-state` request/response header propagation.
- New provider capability registry fields.
- Changing the Agent model-loop default compaction implementation.
- Provider-native cancel endpoint integration.

## Acceptance

- Source-contract tests prove unary implementation selection is explicit and v2 remains available.
- Endpoint tests prove unary compact routes post to `/responses/compact`.
- Payload tests prove unary compact payload omits `compaction_trigger` and `stream`, while v2 still includes both.
- Parser tests prove unary compact responses reuse the existing strict JSON output parser.
- Existing `provider_compact_transport`, `remote_compaction`, and `model_loop_compaction` tests stay green.
- Migration matrix moves unary `/responses/compact` parity from remaining work into implemented evidence while keeping WebSocket streaming and provider-native cancel endpoints as next.
