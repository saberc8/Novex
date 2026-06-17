# Agent Provider Media Lease Design

## Goal

Extend the durable provider-call lease control plane to Agent media image generation.

The Agent media tool already resolves tenant-bound `MediaGeneration` routes and persists media jobs/assets. Before this slice, the live provider HTTP call still happened directly inside `agent_service.rs`, so provider-call lease listing, stale-expire recovery, heartbeat refresh, and model ops evidence covered chat, compaction, embedding, and rerank calls but not image generation.

## Scope

This slice adds tenant-bound leases for media image provider calls:

- `ModelRuntimeService::generate_media_image`
  - Keeps the raw provider HTTP adapter in the model runtime boundary.
  - Uses the existing `MediaImageGenerationRequest` provider payload.
  - Parses provider responses through `novex_tools::parse_media_image_generation_response`.
- `ModelRuntimeService::generate_media_image_for_source`
  - Wraps live image generation in `ai_model_provider_call_lease`.
  - Uses `route_purpose = media_generation`, `request_kind = media_image_generation`, and a caller-provided `source`.
  - Stores prompt length, size, count, route/provider/model, and response asset metadata.
  - Does not store prompt text, API keys, or raw generated image bytes.
- Agent media tool:
  - Keeps route resolution and dry-run behavior.
  - Calls the runtime wrapper for live provider calls.
  - Keeps tool execution payload shape for media job/asset persistence.

## Non-Goals

- Provider-native cancellation endpoints for image generation.
- New media asset/job schema fields.
- New HTTP endpoints; existing provider-call lease controls already surface `request_kind` and `source`.
- Streaming media progress events.
- Cost estimation for image generation.

## Acceptance

- Tests prove media lease records map tenant, route, purpose, request kind, source, prompt length, size/count, and avoid secret leakage.
- Source-contract tests prove media generation uses the shared lease begin/heartbeat/complete path.
- Source-contract tests prove Agent media live calls use `generate_media_image_for_source` instead of direct `reqwest` provider calls.
- Existing media job/asset persistence tests keep passing.
- Full workspace verification passes before merge.
