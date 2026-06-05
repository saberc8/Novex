# M1 Knowledge Foundation Design

## Goal

Start M1 by turning the Knowledge placeholder into a real control-plane slice: PostgreSQL metadata tables, Rust RAG domain types, backend list/create APIs, and an admin page that can list and create knowledge datasets.

This is not the full RAG MVP. Upload, parser worker execution, chunking, embedding, Milvus, rerank, answer generation, and citation rendering remain later M1 slices.

## Architecture

`novex-rag` owns stable RAG domain vocabulary: dataset status, visibility, retrieval mode, document parse status, and ingestion status. `backend` owns HTTP/API orchestration, RBAC checks, SQL persistence, and audit-compatible metadata fields.

Data model starts with PostgreSQL metadata:

- `ai_dataset`: tenant-scoped knowledge base resource.
- `ai_document`: tenant-scoped document metadata under a dataset.

Both tables include `tenant_id`, `owner_id`, `visibility`, and `acl_policy` from the architecture rules. M1.1 uses `tenant_id = 1` as the platform-default tenant until the dedicated tenant control-plane slice lands. This keeps future tenant migration explicit while avoiding a second ad hoc tenant model.

## Backend API

M1.1 adds:

- `GET /ai/knowledge/datasets`
- `POST /ai/knowledge/datasets`
- `GET /ai/knowledge/datasets/:id/documents`

Permissions:

- `ai:knowledge:list` for list endpoints.
- `ai:knowledge:create` for dataset creation.

Creation only creates metadata. It does not upload files, start parser jobs, create chunks, call models, or write vectors.

## Admin UI

The existing `/ai/knowledge` placeholder becomes a real admin surface:

- list datasets
- filter by name
- show status, visibility, retrieval mode, document count, chunk count, owner, created time
- create a dataset with name, description, visibility, and retrieval mode

The page follows the existing admin style: dense control panels, table layout, permission gates, no marketing layout.

## Error Handling

Backend uses existing `AppError` and `ApiResponse`. Validation rejects empty names and overlong names before persistence. Missing permissions return the standard forbidden envelope. Database errors remain sanitized by the existing error layer.

Frontend uses existing `api` helper and `sonner` toasts. List failures show a toast and preserve the page shell.

## Testing

Rust:

- `novex-rag` unit tests for metadata defaults.
- backend service tests for normalization and validation.
- HTTP handler tests for permission checks.
- `cargo test --workspace --offline`.

Frontend:

- API client tests for knowledge endpoints.
- page/component test for rendering empty state or mocked dataset list.
- `pnpm typecheck`.
- `pnpm test`.
- `pnpm build` if routes or page structure change.

## Non-Goals

- No file upload integration.
- No parser worker invocation.
- No chunks table API.
- No embedding or model route calls.
- No Milvus integration.
- No RAG ask API.
- No cross-tenant switching UI.
