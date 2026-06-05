# Parser Worker

Python sidecar for document parsing and heavy ML-adjacent processing.

Allowed responsibilities:

- PDF, scanned document, and complex layout parsing.
- Office to PDF conversion orchestration.
- OCR and document normalization.
- Returning structured parse results through a controlled API, queue job, or dedicated job contract.

Boundaries:

- Does not own RBAC, tenants, secrets, model routes, or audit policy.
- Does not directly write core business tables.
- Does not become the primary backend API.

## M1 Contract

M1 keeps direct text ingestion in the Rust backend for a deterministic local RAG loop. The parser worker contract defines the out-of-process path for PDF, Office, OCR, and layout-aware parsing.

Contracts live in `contracts/`:

- `parse-request.schema.json`: request issued by the backend for one parser job.
- `parse-result.schema.json`: structured result returned by the worker.

The backend remains the authority for `tenantId`, `datasetId`, `documentId`, ACL, parser job status, chunk persistence, embedding, trace, and audit. The worker receives a bounded source reference or inline text and returns normalized `blocks` and candidate `chunks`; it does not write `ai_document`, `ai_document_chunk`, or trace tables directly.

Required result shape:

- `datasetId` and `documentId` identify the target resource.
- `blocks` preserve layout-level parse output for citation and future UI preview.
- `chunks` provide deterministic text spans with chunk ids, token counts, and citation payloads.
- `metadata` captures parser name, page count, source hash, and warnings.

Backend ingestion endpoint:

```text
POST /ai/knowledge/datasets/{datasetId}/documents/parsed
```

The backend accepts `{ name, contentType, parserResult }`, validates tenant/dataset/status, writes
`ai_document`, `ai_parser_job`, `ai_document_block`, and `ai_document_chunk` in one transaction,
and regenerates `semanticSearchText` from the parser chunks plus source file, section path, table
header, page, bbox, and block references. The worker still never writes database tables directly.

Chunk ingestion contract:

- `chunkUid` and `chunkIndex` must be unique within a parser result.
- `citation.blockIds`, when present, must reference existing `blocks[*].blockId` values.
- `semanticSearchText` is optional; when present, the backend uses it as the main retrieval body and still adds source file, section path, and table header hints before saving.
- `segmentType`, `tableHeader`, `imageAccessKeys`, `contentRole`, and `displayCapability` may be supplied by the worker. If omitted, the backend infers them from referenced blocks and chunk text.
- Chunk-level `metadata` is preserved under `ai_document_chunk.metadata.parserChunkMetadata`; canonical searchable/filterable fields remain in dedicated DB columns and normalized metadata keys.

## Local MinerU Configuration

MinerU credentials are runtime secrets and must not be committed. Start the worker process with:

```bash
export MINERU_TOKEN="<token from OpenXLab/MinerU>"
export PARSER_WORKER_MODE="mineru"
PYTHONPATH=services/parser-worker python3 -m parser_worker.health
```

The health command prints only masked credentials, for example:

```json
{"mineru":{"configured":true,"timeoutSeconds":120,"token":"eyJ0****Esnw"},"mode":"mineru","service":"parser-worker"}
```

Current implementation status:

- Reads `MINERU_TOKEN` and reports safe configuration status.
- Provides a tested MinerU v4 client wrapper for `POST /api/v4/extract/task` and `GET /api/v4/extract/task/{task_id}`.
- Keeps text/markdown ingestion in Rust for the deterministic M1 RAG loop.
- Leaves actual MinerU PDF/OCR network parsing behind the parser-worker contract for the next parser execution slice.
- Unit tests use a fake transport and do not submit documents to MinerU or consume parse quota.

Verification:

```bash
PYTHONPATH=services/parser-worker python3 -m unittest discover -s services/parser-worker/tests
```
