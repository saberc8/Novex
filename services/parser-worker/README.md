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
