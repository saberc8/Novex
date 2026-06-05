# Model Runtime Config Design

Date: 2026-06-05

## Context

Novex M0-M5 currently exposes the model boundary as a skeleton module. RAG still uses local placeholder route constants and the admin model page is a placeholder. The deployment environment supplies model credentials through environment variables:

- `LLM_API_KEY`, `LLM_BASE_URL`, `LLM_MODEL`
- `EMBEDDING_API_KEY`, `EMBEDDING_BASE_URL`, `EMBEDDING_MODEL`
- `RERANKER_API_KEY`, `RERANKER_BASE_URL`, `RERANKER_MODEL`
- `RIGHT_CODE_DRAW_BASE_URL`, `RIGHT_CODE_DRAW_API_KEY`

Runtime code must map those values into Novex model routes without persisting or returning raw secrets.

## Provider Mapping

| Novex route purpose | Env group | Provider | Endpoint |
| --- | --- | --- | --- |
| `chat`, `rag_answer`, `eval_judge`, `code_agent` | `LLM_*` | DeepSeek OpenAI-compatible chat | `{LLM_BASE_URL}/chat/completions` |
| `embedding` | `EMBEDDING_*` | DashScope OpenAI-compatible embedding | `{EMBEDDING_BASE_URL}/embeddings` |
| `rerank` | `RERANKER_*` | DashScope rerank | `{RERANKER_BASE_URL}/reranks` |
| `media_generation` | `RIGHT_CODE_DRAW_*` | Right Code Draw | `{RIGHT_CODE_DRAW_BASE_URL}` |

The reranker route intentionally uses `/reranks` because `/rerank` returns 404 for the supplied DashScope-compatible endpoint.

## Secret Boundary

- Raw API keys are read only from process environment.
- API responses and admin UI receive only masked keys.
- Health checks may use raw keys in outbound headers, but never echo request headers, raw response bodies, or raw keys back to clients.
- Plans, docs, tests, and fixtures must use fake keys only.

## Backend Surface

Add two authenticated endpoints:

- `GET /ai/models/runtime-config`
  - Permission: `ai:model:list`
  - Returns complete runtime route summaries and missing env variable names.
- `POST /ai/models/health-check`
  - Permission: `ai:model:healthCheck`
  - Body: `{ "target": "all" | "llm" | "embedding" | "reranker" | "draw" }`
  - Returns one sanitized result per checked target.

Health checks are operational probes, not business traffic. They use minimal payloads and short request timeouts:

- LLM: small chat completion expecting any 2xx completion response.
- Embedding: one short input and non-empty vector.
- Reranker: one query and two short documents, expecting non-empty results.
- Draw: authenticated GET against the configured base URL, expecting a successful or redirect response.

## Admin Surface

Replace the placeholder model page with a compact operations panel:

- Runtime route table with kind, provider, model, endpoint, purposes, and masked key.
- Missing env warning list.
- One health-check action for all targets.
- Per-target status, HTTP status, latency, and sanitized detail.

The UI does not provide credential editing in M5. Credentials remain deployment-level configuration.

## Out of Scope

- Persisted provider/deployment/profile tables.
- Rotating credentials from the admin UI.
- Replacing RAG local placeholder execution with live provider calls.
- Detailed Right Code Draw generation API integration beyond authenticated reachability.
