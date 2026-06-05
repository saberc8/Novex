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

M1 keeps direct text ingestion in the Rust backend for a deterministic local RAG loop. The parser worker adds the out-of-process path for PDF, Office, OCR, and layout-aware parsing while still preserving native parsing for strongly structured text/table formats.

Contracts live in `contracts/`:

- `parse-request.schema.json`: request issued by the backend for one parser job.
- `parse-result.schema.json`: structured result returned by the worker.

The backend remains the authority for `tenantId`, `datasetId`, `documentId`, ACL, parser job status, chunk persistence, embedding, trace, and audit. The worker receives a bounded source reference or inline text and returns normalized `blocks` and candidate `chunks`; it does not write `ai_document`, `ai_document_chunk`, or trace tables directly.

Parser strategy follows `docs/ARCHITECTURE.md` section 7.2:

- PDF: submit directly to MinerU.
- Office: convert with LibreOffice or an injected converter, then submit the normalized PDF to MinerU.
- Image/scanned document: submit to MinerU/OCR.
- HTML, Markdown, TXT, code, JSON, and logs: native structured parsing.
- CSV/TSV/XLSX: table-aware parsing; do not force these through PDF. XLS/XLSX/ODS use a table extractor boundary that emits CSV/TSV-like text before chunking.

Required result shape for backend ingestion:

- `datasetId` and `documentId` identify the target resource.
- `blocks` preserve layout-level parse output for citation and future UI preview.
- `chunks` provide deterministic text spans with chunk ids, token counts, semantic search text, and citation payloads.
- `metadata` captures parser name, page count, source hash, and warnings.
- `status=submitted` is only a parser job submission envelope for asynchronous MinerU work. It is not sent to the backend ingestion endpoint.
- `status=succeeded` means blocks/chunks are complete and can be posted to the backend.

Backend ingestion endpoint:

```text
POST /ai/knowledge/datasets/{datasetId}/documents/files
POST /ai/knowledge/datasets/{datasetId}/parse-jobs
GET  /ai/knowledge/datasets/{datasetId}/parse-jobs/{jobId}
POST /ai/knowledge/datasets/{datasetId}/documents/parsed
```

The customer-facing upload path posts a multipart file to `documents/files`. The backend stores the original file through the file service, derives a parser job command from the saved asset metadata, creates `ai_document` and `ai_parser_job` records through the same parse-job path, returns a parser-worker request envelope, and keeps the document in parsing/pending state. A completed `succeeded` parse result is posted to `documents/parsed`; if the parser job already exists, the backend finalizes that existing job instead of inserting a duplicate document/job pair.

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

Worker entry points:

- `parser_worker.parse.parse_request(request)` routes by file type. It returns a completed `succeeded` parse result for native structured formats, or a `submitted` MinerU task envelope for PDF/Office/Image paths.
- `parser_worker.parse.parse_mineru_markdown_result(request, markdown, mineru_metadata=...)` converts completed MinerU markdown/layout text into the same `succeeded` parsed result contract used by the backend.
- `parser_worker.parse.parse_local_request(request)` is the native structured parser used for Markdown/TXT/CSV/code/log style inputs and for normalizing MinerU markdown output.
- XLS/XLSX/ODS support is intentionally an injected extractor boundary in this slice; production wiring should use a spreadsheet reader or LibreOffice CSV export, not PDF conversion.

## Local MinerU Configuration

MinerU credentials are runtime secrets and must not be committed. Start the worker process with:

```bash
export MINERU_TOKEN="<token from OpenXLab/MinerU>"
export PARSER_WORKER_MODE="type-routed"
PYTHONPATH=services/parser-worker python3 -m parser_worker.health
```

The health command prints only masked credentials, for example:

```json
{"mineru":{"configured":true,"timeoutSeconds":120,"token":"eyJ0****Esnw"},"mode":"type-routed","service":"parser-worker"}
```

Current implementation status:

- Reads `MINERU_TOKEN` and reports safe configuration status.
- Provides a tested MinerU v4 client wrapper for `POST /api/v4/extract/task` and `GET /api/v4/extract/task/{task_id}`.
- Implements a type-routed parser worker boundary: native structured parsing for Markdown/TXT/CSV-style inputs, PDF direct MinerU submission, and Office-to-PDF-to-MinerU submission through an injected converter boundary.
- Converts completed MinerU markdown/layout output into backend-ready blocks/chunks with section path, page, table header, image access key, and semantic search text metadata.
- Leaves production LibreOffice conversion, object storage publishing, and MinerU result ZIP download behind injectable boundaries for the next parser execution slice.
- Unit tests use a fake transport and do not submit documents to MinerU or consume parse quota.

Verification:

```bash
PYTHONPATH=services/parser-worker python3 -m unittest discover -s services/parser-worker/tests
```
