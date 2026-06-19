# Chat Flow + Knowledge Design

## Goal

Build the first real Novex infrastructure loop for app-side RAG chat: a user can create or select a knowledge base, upload files, wait for parsing/indexing, then ask questions in a persistent chat session with traceable citations.

## Current State

Novex already has useful pieces, but they are not one product flow:

- `KnowledgeService` supports datasets, text/file/parsed document ingestion, parser jobs, indexed chunks, RAG ask, trace hits, and feedback.
- `ModelRuntimeService` supports pure model chat, provider routing, usage recording, and model chat history.
- `apps/chat-web` can list datasets and call either `/ai/knowledge/datasets/:id/ask` or `/ai/models/chat`, but the frontend owns the orchestration.

The missing infrastructure is a unified chat-flow business layer. The backend needs to own session state, dataset binding, RAG execution, message persistence, trace linkage, and citations. The app also needs app-side file upload and parse-status handling, not only admin-side knowledge management.

## Recommended Approach

Add a small Novex chat-flow layer on top of the existing model and knowledge services.

The backend will expose chat-flow APIs for sessions and messages. A session can run in `knowledge` or `model` mode. In `knowledge` mode it binds to a dataset and every user message runs the real RAG path: retrieve indexed chunks, rerank when configured, build an answer, persist `ai_rag_trace`, persist assistant message metadata with `ragTraceId` and citations, then return the full answer payload. In `model` mode it uses the configured LLM route and persists the turn under the same session/message contract.

The app will stop treating knowledge ask and model chat as unrelated interactions. It will let the user create/select a dataset, upload files to the dataset, poll parser job status, then start or continue a chat-flow session against the selected dataset.

## Alternatives Considered

1. Keep upload/admin split and only let `apps/chat-web` select existing datasets.
   This is faster but does not create a user-side RAG product loop.

2. Let the frontend orchestrate dataset upload, parse polling, and direct `/ask` calls.
   This avoids a new backend layer but leaves session, trace, and citation semantics spread across the app.

3. Build a Dify/FastGPT-style workflow graph now.
   This is too broad for the first infrastructure milestone because it pulls in node schemas, workflow execution, variables, and visual builder contracts before the RAG chat loop is stable.

## Backend Design

Add `ChatFlowService` under `backend/src/application/ai/`. It should use the existing `KnowledgeService`, `ModelRuntimeService`, and direct persistence helpers where needed.

Core APIs:

- `POST /ai/chat-flow/sessions`
  Create a session with `mode`, optional `datasetId`, and optional title.
- `GET /ai/chat-flow/sessions`
  List the current user's sessions.
- `GET /ai/chat-flow/sessions/:session_id/messages`
  Load persisted turns for a session.
- `POST /ai/chat-flow/sessions/:session_id/messages`
  Send a user message and receive a persisted assistant answer.

The session response includes `id`, `mode`, `datasetId`, `title`, `messageCount`, `lastMessagePreview`, timestamps, and status. The message response includes `id`, `sessionId`, `role`, `content`, `metadata`, `ragTraceId`, `citations`, route/model information, token counts, and timestamps.

## Data Model

Prefer new chat-flow tables instead of overloading `ai_model_chat_conversation`:

- `ai_chat_flow_session`
  Tenant-scoped session metadata, app code, mode, bound dataset, route/model summary, message count, preview, status, create/update user and time.
- `ai_chat_flow_message`
  Tenant-scoped message rows with role, content, route/model, token count, rag trace id, citations JSONB, metadata JSONB, and create time.

This keeps existing pure model chat history stable while giving RAG chat first-class storage.

## Knowledge Upload Flow

`apps/chat-web` should call the existing knowledge endpoints:

- `POST /ai/knowledge/datasets` to create a dataset from the app.
- `GET /ai/knowledge/datasets` to list selectable datasets.
- `POST /ai/knowledge/datasets/:dataset_id/documents/files` to upload a file and create a parser job.
- `GET /ai/knowledge/datasets/:dataset_id/parse-jobs/:job_id` to poll parsing/indexing state.
- `GET /ai/knowledge/datasets/:dataset_id/documents` to show uploaded documents if needed.

No browser visual debugging is required. API and automated checks are enough for this phase.

## Error Handling

- A knowledge chat session requires a valid dataset in the current tenant.
- Sending a message to another user's session returns not found.
- Sending a knowledge message before the dataset has indexed chunks should return a normal assistant message explaining that no indexed content is available, while preserving the user turn and metadata.
- Upload and parse errors should remain visible through parser job status.
- Model-provider failures in model mode should return a typed API error and not create a fake assistant answer.

## Testing

Use TDD for implementation:

- Rust unit tests for command normalization, session creation records, message persistence metadata, and tenant ownership checks.
- Rust handler tests for permission checks and route registration.
- Repository-level smoke through SQLx against the existing local Postgres path when available.
- Frontend tests for dataset creation/upload calls, parse polling, session creation, message sending, and rendering citations from chat-flow responses.
- Final verification with `cargo test -p backend --offline`, workspace tests where practical, and package-level `pnpm` tests/typecheck for `apps/chat-web`.

## Milestone Boundary

In scope:

- User-side dataset create/select.
- User-side file upload and parser job status.
- Persistent chat-flow sessions/messages.
- Knowledge-mode RAG message execution with trace and citations.
- Model-mode message execution through the same chat-flow API.
- API/curl-level smoke validation.

Out of scope for this milestone:

- Visual workflow builder.
- Dify/FastGPT node graph runtime.
- Browser visual QA.
- Full M2-M5 tool/agent/eval/template completion.
