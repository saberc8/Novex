# Novex RAG Module Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize `crates/novex-rag` from a 2,291-line `src/lib.rs` into focused domain modules while preserving the crate-root public API.

**Architecture:** Keep `src/lib.rs` as the crate facade and move existing behavior unchanged into `knowledge`, `model_routes`, `milvus`, `parse`, `chunk`, `retrieval`, and `answer` modules. Add an integration-level structure test that proves `lib.rs` is a facade and a public-facade characterization test that proves existing root imports still work.

**Tech Stack:** Rust 2021, Cargo workspace, `serde`, `serde_json`, `unicode-normalization`, `novex-ai-core`, `novex-model`.

## Global Constraints

- No database migrations.
- No provider SDK changes.
- No model routing behavior changes.
- No frontend changes.
- No new AI feature behavior.
- Preserve root-level exports such as `novex_rag::chunk_document`, `novex_rag::MilvusSearchRequest`, and `novex_rag::RagModelRoutes`.
- Keep cross-crate dependency direction as `novex-rag -> novex-ai-core / novex-model`.
- Run `cargo fmt --all -- --check`, `cargo test -p novex-rag`, and `git diff --check` before considering this slice complete.

---

## File Structure

- Create: `crates/novex-rag/tests/module_structure.rs`
  - Proves the new file layout exists, `lib.rs` is a facade, and root-level public APIs keep working.
- Create: `crates/novex-rag/src/knowledge.rs`
  - Owns knowledge, document, chunk, citation, source-block, metadata, retrieval-hit DTOs.
- Create: `crates/novex-rag/src/model_routes.rs`
  - Owns local RAG route constants, `RagModelRoutes`, and runtime route lookup.
- Create: `crates/novex-rag/src/milvus.rs`
  - Owns Milvus request/response DTOs, REST body builders, search-hit parsing helpers.
- Create: `crates/novex-rag/src/parse.rs`
  - Owns plain-text/table/markdown/image-marker parsing into `ParsedDocument`.
- Create: `crates/novex-rag/src/chunk.rs`
  - Owns chunk splitting, chunk metadata, semantic search text construction, simple token counting.
- Create: `crates/novex-rag/src/retrieval.rs`
  - Owns keyword retrieval, BM25 scoring, CJK-aware BM25 tokenization, numbered label range expansion.
- Create: `crates/novex-rag/src/answer.rs`
  - Owns `RagTraceSnapshot`, `RagAnswer`, and extractive answer building.
- Modify: `crates/novex-rag/src/lib.rs`
  - Keep only module declarations, root re-exports, `CRATE_ID`, and `module()`.

---

### Task 1: Add RAG Structure and Public-Facade Characterization Tests

**Files:**
- Create: `crates/novex-rag/tests/module_structure.rs`

**Interfaces:**
- Consumes: existing crate-root public API from `novex_rag`.
- Produces: failing structure tests that later tasks must satisfy.

- [ ] **Step 1: Write the failing structure and facade tests**

Create `crates/novex-rag/tests/module_structure.rs` with:

```rust
use std::fs;
use std::path::Path;

use novex_rag::{
    build_extractive_answer, chunk_document, keyword_retrieve, parse_document_content,
    parse_milvus_search_hits, BoundingBox, ChunkMetadata, ChunkSegmentType, DisplayCapability,
    MilvusCreateCollectionRequest, MilvusMetricType, MilvusSearchRequest, MilvusUpsertRequest,
    MilvusUpsertRow, RagModelRoutes, ResourceVisibility, LOCAL_ANSWER_ROUTE,
    LOCAL_EMBEDDING_ROUTE, LOCAL_RERANK_ROUTE,
};

fn crate_file(relative_path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn lib_rs_is_facade_for_rag_modules() {
    let lib = crate_file("src/lib.rs");

    for module in [
        "answer",
        "chunk",
        "knowledge",
        "milvus",
        "model_routes",
        "parse",
        "retrieval",
    ] {
        assert!(
            lib.contains(&format!("mod {module};")),
            "lib.rs should declare module {module}"
        );
    }

    for moved_item in [
        "pub struct DocumentChunk",
        "pub struct MilvusSearchRequest",
        "pub fn parse_document_content",
        "pub fn chunk_document",
        "pub fn keyword_retrieve",
        "pub fn build_extractive_answer",
        "#[cfg(test)]\nmod tests",
    ] {
        assert!(
            !lib.contains(moved_item),
            "{moved_item} should not live in facade lib.rs"
        );
    }
}

#[test]
fn rag_domain_modules_exist() {
    for module in [
        "src/answer.rs",
        "src/chunk.rs",
        "src/knowledge.rs",
        "src/milvus.rs",
        "src/model_routes.rs",
        "src/parse.rs",
        "src/retrieval.rs",
    ] {
        let source = crate_file(module);
        assert!(!source.trim().is_empty(), "{module} should not be empty");
    }
}

#[test]
fn root_facade_preserves_public_rag_contracts() {
    assert_eq!(ResourceVisibility::default(), ResourceVisibility::Private);
    assert_eq!(RagModelRoutes::local().embedding_model_route, LOCAL_EMBEDDING_ROUTE);
    assert_eq!(RagModelRoutes::local().rerank_model_route, LOCAL_RERANK_ROUTE);
    assert_eq!(RagModelRoutes::local().answer_model_route, LOCAL_ANSWER_ROUTE);

    let parsed = parse_document_content(
        "doc-facade",
        "architecture.md",
        "text/markdown",
        "# Retrieval\n[[page: 2]]\nHybrid retrieval keeps citations anchored.",
    );
    let chunks = chunk_document(&parsed, 80, 0);
    let hits = keyword_retrieve("retrieval citations", &chunks, 3);
    let answer = build_extractive_answer("What keeps citations anchored?", &hits);

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].metadata.segment_type, ChunkSegmentType::Text);
    assert_eq!(chunks[0].metadata.display_capability, DisplayCapability::PreciseAnchor);
    assert_eq!(chunks[0].metadata.page_no, Some(2));
    assert!(answer.answer.contains("Hybrid retrieval"));
    assert_eq!(answer.trace.retrieval_hit_count, 1);
}

#[test]
fn root_facade_preserves_public_milvus_contracts() {
    let search = MilvusSearchRequest::new("novex_t1_dataset_2", vec![0.1, 0.2], 5, 1, 2)
        .with_document_ids(vec![9, 7, 9])
        .with_metric_type(MilvusMetricType::Cosine);
    assert_eq!(
        search.filter_expression(),
        "tenant_id == 1 and dataset_id == 2 and document_id in [7, 9]"
    );
    assert_eq!(search.to_rest_search_body()["searchParams"]["metric_type"], "COSINE");

    let create = MilvusCreateCollectionRequest::new(
        "novex_t1_dataset_2",
        3,
        MilvusMetricType::Cosine,
    );
    assert_eq!(create.to_rest_create_body()["schema"]["fields"][0]["fieldName"], "id");

    let upsert = MilvusUpsertRequest::new(
        "novex_t1_dataset_2",
        vec![MilvusUpsertRow {
            id: 10,
            tenant_id: 1,
            dataset_id: 2,
            document_id: 7,
            chunk_uid: "doc:0".to_owned(),
            chunk_index: 0,
            embedding: vec![0.1, 0.2, 0.3],
            semantic_search_text: "retrieval citation".to_owned(),
            segment_type: "text".to_owned(),
            content_role: "canonical".to_owned(),
        }],
    );
    assert_eq!(upsert.to_rest_upsert_body()["data"][0]["chunk_uid"], "doc:0");

    let hits = parse_milvus_search_hits(&serde_json::json!({
        "data": [{"distance": 0.5, "chunk_uid": "doc:0", "chunk_db_id": 10}]
    }));
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].chunk_uid, "doc:0");
    assert_eq!(hits[0].chunk_db_id, Some(10));
}

#[test]
fn root_facade_preserves_anchor_metadata_types() {
    let metadata = ChunkMetadata {
        segment_type: ChunkSegmentType::Image,
        bbox: Some(BoundingBox {
            x: 1,
            y: 2,
            width: 30,
            height: 40,
        }),
        ..ChunkMetadata::default()
    };

    assert_eq!(metadata.segment_type.as_str(), "image");
    assert_eq!(metadata.display_capability.as_str(), "text_only");
    assert_eq!(metadata.bbox.as_ref().map(|bbox| bbox.width), Some(30));
}
```

- [ ] **Step 2: Run the new test and verify it fails for structure**

Run:

```bash
cargo test -p novex-rag --test module_structure
```

Expected: FAIL because `src/answer.rs`, `src/chunk.rs`, `src/knowledge.rs`, `src/milvus.rs`, `src/model_routes.rs`, `src/parse.rs`, and `src/retrieval.rs` do not exist yet, and `src/lib.rs` still contains moved items.

- [ ] **Step 3: Commit the failing characterization test only if using a red/green commit style**

When committing per task, use:

```bash
git add crates/novex-rag/tests/module_structure.rs
git commit -m "test: characterize novex rag module facade"
```

If continuing in one working commit, leave the test staged or unstaged and proceed to Task 2.

---

### Task 2: Extract Knowledge DTOs and Model Routes

**Files:**
- Create: `crates/novex-rag/src/knowledge.rs`
- Create: `crates/novex-rag/src/model_routes.rs`
- Modify: `crates/novex-rag/src/lib.rs`

**Interfaces:**
- Consumes: no new modules.
- Produces:
  - `knowledge::{KnowledgeResourceKind, DatasetStatus, ResourceVisibility, RetrievalMode, DocumentParseStatus, IngestionStatus, CitationRef, ChunkSegmentType, ContentRole, DisplayCapability, BoundingBox, SourceBlock, ChunkMetadata, ParsedDocument, DocumentChunk, RetrievalHit}`
  - `model_routes::{LOCAL_EMBEDDING_ROUTE, LOCAL_RERANK_ROUTE, LOCAL_ANSWER_ROUTE, RagModelRoutes}`

- [ ] **Step 1: Move knowledge DTOs unchanged**

Create `crates/novex-rag/src/knowledge.rs` and move these existing definitions from `src/lib.rs` without changing field names, derives, serde attributes, or method bodies:

```text
KnowledgeResourceKind
DatasetStatus
ResourceVisibility
RetrievalMode
DocumentParseStatus
IngestionStatus
CitationRef
ChunkSegmentType
impl ChunkSegmentType
ContentRole
impl ContentRole
DisplayCapability
impl DisplayCapability
BoundingBox
SourceBlock
impl SourceBlock
ChunkMetadata
impl Default for ChunkMetadata
ParsedDocument
DocumentChunk
RetrievalHit
```

At the top of `knowledge.rs`, keep only the imports these definitions need:

```rust
use serde::{Deserialize, Serialize};
```

- [ ] **Step 2: Move model route contracts unchanged**

Create `crates/novex-rag/src/model_routes.rs` and move these existing definitions from `src/lib.rs` without changing constant values or method bodies:

```text
LOCAL_EMBEDDING_ROUTE
LOCAL_RERANK_ROUTE
LOCAL_ANSWER_ROUTE
RagModelRoutes
impl RagModelRoutes
runtime_route_id
```

At the top of `model_routes.rs`, use:

```rust
use novex_model::{ModelRuntimeConfig, ModelRuntimeTarget};
use serde::{Deserialize, Serialize};
```

- [ ] **Step 3: Add temporary facade declarations and re-exports**

Replace the top of `crates/novex-rag/src/lib.rs` imports and moved items with:

```rust
mod knowledge;
mod model_routes;

use novex_ai_core::FoundationModule;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use unicode_normalization::UnicodeNormalization;

pub use knowledge::{
    BoundingBox, ChunkMetadata, ChunkSegmentType, CitationRef, ContentRole, DatasetStatus,
    DisplayCapability, DocumentChunk, DocumentParseStatus, IngestionStatus, KnowledgeResourceKind,
    ParsedDocument, ResourceVisibility, RetrievalHit, RetrievalMode, SourceBlock,
};
pub use model_routes::{
    RagModelRoutes, LOCAL_ANSWER_ROUTE, LOCAL_EMBEDDING_ROUTE, LOCAL_RERANK_ROUTE,
};
```

Keep the remaining implementation in `lib.rs` compiling for now.

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p novex-rag
```

Expected: existing tests compile and pass except `module_structure::lib_rs_is_facade_for_rag_modules` and `module_structure::rag_domain_modules_exist`, which continue failing until later modules are split.

---

### Task 3: Extract Milvus Contracts

**Files:**
- Create: `crates/novex-rag/src/milvus.rs`
- Modify: `crates/novex-rag/src/lib.rs`

**Interfaces:**
- Consumes: `serde_json::Value`.
- Produces:
  - `milvus::{MilvusMetricType, MilvusSearchRequest, MilvusSearchHit, MilvusCreateCollectionRequest, MilvusUpsertRow, MilvusUpsertRequest, parse_milvus_search_hits}`

- [ ] **Step 1: Move Milvus types and helpers unchanged**

Create `crates/novex-rag/src/milvus.rs` and move these existing definitions from `src/lib.rs` unchanged:

```text
MilvusMetricType
impl MilvusMetricType
MilvusSearchRequest
impl MilvusSearchRequest
MilvusSearchHit
MilvusCreateCollectionRequest
impl MilvusCreateCollectionRequest
MilvusUpsertRow
impl MilvusUpsertRow
MilvusUpsertRequest
impl MilvusUpsertRequest
parse_milvus_search_hits
normalized_positive_ids
milvus_hits_container
collect_milvus_hit_rows
milvus_search_hit_from_value
merged_milvus_hit_fields
merge_object_fields
string_field
f32_field
i64_field
```

At the top of `milvus.rs`, use:

```rust
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
```

- [ ] **Step 2: Re-export Milvus public API from the facade**

In `src/lib.rs`, add:

```rust
mod milvus;

pub use milvus::{
    parse_milvus_search_hits, MilvusCreateCollectionRequest, MilvusMetricType,
    MilvusSearchHit, MilvusSearchRequest, MilvusUpsertRequest, MilvusUpsertRow,
};
```

Remove now-unused `json`, `Map`, and `Value` imports from `lib.rs` only if no remaining code uses them.

- [ ] **Step 3: Run Milvus facade test**

Run:

```bash
cargo test -p novex-rag --test module_structure root_facade_preserves_public_milvus_contracts
```

Expected: PASS.

---

### Task 4: Extract Parse Module

**Files:**
- Create: `crates/novex-rag/src/parse.rs`
- Modify: `crates/novex-rag/src/lib.rs`

**Interfaces:**
- Consumes: `knowledge::{BoundingBox, ChunkSegmentType, ParsedDocument, SourceBlock}`.
- Produces:
  - `parse::{parse_plain_text, parse_document_content}`
  - private parse helpers for text, table, page marker, image marker, bbox parsing.

- [ ] **Step 1: Move parse functions and helpers unchanged**

Create `crates/novex-rag/src/parse.rs` and move these existing definitions from `src/lib.rs` unchanged:

```text
parse_plain_text
parse_document_content
normalize_content_type
non_empty_string
is_table_document
parse_table_blocks
split_table_cells
parse_text_blocks
push_text_paragraph
markdown_heading
page_marker
image_marker
marker_field
marker_tail_field
parse_bbox
```

At the top of `parse.rs`, use:

```rust
use crate::knowledge::{BoundingBox, ParsedDocument, SourceBlock};
```

- [ ] **Step 2: Re-export parse public API from the facade**

In `src/lib.rs`, add:

```rust
mod parse;

pub use parse::{parse_document_content, parse_plain_text};
```

- [ ] **Step 3: Run parse-related tests**

Run:

```bash
cargo test -p novex-rag parse_document_content_extracts_image_marker_anchor_metadata
cargo test -p novex-rag chunk_document_keeps_markdown_section_and_page_anchor
```

Expected: PASS after imports are corrected.

---

### Task 5: Extract Chunk Module

**Files:**
- Create: `crates/novex-rag/src/chunk.rs`
- Modify: `crates/novex-rag/src/lib.rs`

**Interfaces:**
- Consumes: `knowledge::{ChunkMetadata, ChunkSegmentType, CitationRef, ContentRole, DisplayCapability, DocumentChunk, ParsedDocument, SourceBlock}`.
- Produces:
  - `chunk::{chunk_text, chunk_document, build_semantic_search_text}`
  - private chunk splitting, metadata, semantic text, and simple token-count helpers.

- [ ] **Step 1: Move chunk functions and helpers unchanged**

Create `crates/novex-rag/src/chunk.rs` and move these existing definitions from `src/lib.rs` unchanged:

```text
chunk_text
chunk_document
build_semantic_search_text
split_table_block
split_text_block
split_oversized_table_row
split_sentence_units
is_sentence_boundary
join_text_units
split_by_chars
push_document_chunk
chunk_metadata
infer_content_role
display_capability
append_search_part
clean_search_text
is_low_value_image_caption
remove_latex_commands
is_uuid_like
normalize_search_line
tokenize
is_cjk_character
```

At the top of `chunk.rs`, use:

```rust
use std::collections::HashSet;

use crate::knowledge::{
    ChunkMetadata, ChunkSegmentType, CitationRef, ContentRole, DisplayCapability, DocumentChunk,
    ParsedDocument, SourceBlock,
};
```

- [ ] **Step 2: Re-export chunk public API from the facade**

In `src/lib.rs`, add:

```rust
mod chunk;

pub use chunk::{build_semantic_search_text, chunk_document, chunk_text};
```

- [ ] **Step 3: Run chunk-related tests**

Run:

```bash
cargo test -p novex-rag chunk_document
cargo test -p novex-rag semantic_search_text_filters_latex_uuid_and_image_placeholder
```

Expected: PASS.

---

### Task 6: Extract Retrieval Module

**Files:**
- Create: `crates/novex-rag/src/retrieval.rs`
- Modify: `crates/novex-rag/src/lib.rs`

**Interfaces:**
- Consumes: `knowledge::{DocumentChunk, RetrievalHit}`.
- Produces:
  - `retrieval::keyword_retrieve`
  - private BM25, CJK tokenization, and numbered label helpers.

- [ ] **Step 1: Move retrieval functions and helpers unchanged**

Create `crates/novex-rag/src/retrieval.rs` and move these existing definitions from `src/lib.rs` unchanged:

```text
keyword_retrieve
BM25_K1
BM25_B
bm25_retrieve
query_token_set
bm25_query_terms
term_frequencies
bm25_term_score
expand_numbered_range_tokens
contains_range_indicator
NumberedLabel
numbered_label_token
bm25_tokens
flush_cjk_bm25_tokens
is_low_value_cjk_unigram
is_cjk_character
```

At the top of `retrieval.rs`, use:

```rust
use std::collections::{HashMap, HashSet};

use unicode_normalization::UnicodeNormalization;

use crate::knowledge::{DocumentChunk, RetrievalHit};
```

- [ ] **Step 2: Re-export retrieval public API from the facade**

In `src/lib.rs`, add:

```rust
mod retrieval;

pub use retrieval::keyword_retrieve;
```

- [ ] **Step 3: Run retrieval tests**

Run:

```bash
cargo test -p novex-rag keyword_retrieve
```

Expected: PASS.

---

### Task 7: Extract Answer Module

**Files:**
- Create: `crates/novex-rag/src/answer.rs`
- Modify: `crates/novex-rag/src/lib.rs`

**Interfaces:**
- Consumes: `knowledge::{CitationRef, RetrievalHit}`.
- Produces:
  - `answer::{RagTraceSnapshot, RagAnswer, build_extractive_answer}`

- [ ] **Step 1: Move answer types and helpers unchanged**

Create `crates/novex-rag/src/answer.rs` and move these existing definitions from `src/lib.rs` unchanged:

```text
RagTraceSnapshot
RagAnswer
build_extractive_answer
first_sentence
```

At the top of `answer.rs`, use:

```rust
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::knowledge::{CitationRef, RetrievalHit};
```

- [ ] **Step 2: Re-export answer public API from the facade**

In `src/lib.rs`, add:

```rust
mod answer;

pub use answer::{build_extractive_answer, RagAnswer, RagTraceSnapshot};
```

- [ ] **Step 3: Run answer tests**

Run:

```bash
cargo test -p novex-rag build_extractive_answer_returns_answer_and_citations
cargo test -p novex-rag --test module_structure root_facade_preserves_public_rag_contracts
```

Expected: PASS.

---

### Task 8: Reduce lib.rs to the RAG Facade and Move Tests

**Files:**
- Modify: `crates/novex-rag/src/lib.rs`
- Create or modify: module-local test blocks inside the new module files.

**Interfaces:**
- Consumes: all modules from Tasks 2-7.
- Produces: a facade-only `lib.rs` and focused module-local tests.

- [ ] **Step 1: Replace `src/lib.rs` with the final facade**

Replace `crates/novex-rag/src/lib.rs` with:

```rust
mod answer;
mod chunk;
mod knowledge;
mod milvus;
mod model_routes;
mod parse;
mod retrieval;

use novex_ai_core::FoundationModule;

pub use answer::{build_extractive_answer, RagAnswer, RagTraceSnapshot};
pub use chunk::{build_semantic_search_text, chunk_document, chunk_text};
pub use knowledge::{
    BoundingBox, ChunkMetadata, ChunkSegmentType, CitationRef, ContentRole, DatasetStatus,
    DisplayCapability, DocumentChunk, DocumentParseStatus, IngestionStatus, KnowledgeResourceKind,
    ParsedDocument, ResourceVisibility, RetrievalHit, RetrievalMode, SourceBlock,
};
pub use milvus::{
    parse_milvus_search_hits, MilvusCreateCollectionRequest, MilvusMetricType,
    MilvusSearchHit, MilvusSearchRequest, MilvusUpsertRequest, MilvusUpsertRow,
};
pub use model_routes::{
    RagModelRoutes, LOCAL_ANSWER_ROUTE, LOCAL_EMBEDDING_ROUTE, LOCAL_RERANK_ROUTE,
};
pub use parse::{parse_document_content, parse_plain_text};
pub use retrieval::keyword_retrieve;

pub const CRATE_ID: &str = "novex-rag";

pub fn module() -> FoundationModule {
    FoundationModule::skeleton(
        CRATE_ID,
        "RAG",
        "ai-foundation",
        "Knowledge datasets, documents, chunks, retrieval, rerank, context, and citation boundaries.",
    )
}
```

- [ ] **Step 2: Move existing unit tests to focused modules**

Move tests from the old `#[cfg(test)] mod tests` block as follows:

```text
module_describes_rag_boundary -> keep as a small #[cfg(test)] block in lib.rs
knowledge_metadata_defaults_match_m1_control_plane -> knowledge.rs
chunk_text_splits_non_empty_text_into_ordered_chunks -> chunk.rs
chunk_document_preserves_table_headers_for_csv -> chunk.rs
chunk_document_prefers_sentence_boundaries_for_text_blocks -> chunk.rs
chunk_document_keeps_table_header_when_large_row_is_split -> chunk.rs
parse_document_content_extracts_image_marker_anchor_metadata -> chunk.rs
semantic_search_text_filters_latex_uuid_and_image_placeholder -> chunk.rs
chunk_document_keeps_markdown_section_and_page_anchor -> chunk.rs
keyword_retrieve_* tests -> retrieval.rs
build_extractive_answer_returns_answer_and_citations -> answer.rs
milvus_* tests -> milvus.rs
rag_model_routes_* tests -> model_routes.rs
```

Keep the `test_chunk` helper in `retrieval.rs` next to the retrieval tests.

- [ ] **Step 3: Run full RAG tests**

Run:

```bash
cargo test -p novex-rag
```

Expected: PASS, including `tests/module_structure.rs`.

---

### Task 9: Update Source-Location Documentation for RAG

**Files:**
- Modify: `docs/plans/2026-06-09-rag-bm25.md`
- Modify: `docs/plans/2026-06-09-rag-bm25-design.md`
- Modify: `docs/plans/2026-06-05-m1-rag-mvp.md`
- Modify: `docs/plans/2026-06-05-m1-knowledge-foundation.md`
- Modify other docs only if `rg "crates/novex-rag/src/lib.rs" docs` still reports contributor-facing instructions.

**Interfaces:**
- Consumes: new RAG module paths.
- Produces: docs that point future RAG work at focused modules instead of `src/lib.rs`.

- [ ] **Step 1: Find stale RAG `lib.rs` instructions**

Run:

```bash
rg -n "crates/novex-rag/src/lib.rs|novex-rag/src/lib.rs" docs
```

Expected: matches in older plans.

- [ ] **Step 2: Update docs to point to new modules**

Replace contributor-facing references according to ownership:

```text
BM25 and keyword retrieval -> crates/novex-rag/src/retrieval.rs
RAG parsing -> crates/novex-rag/src/parse.rs
RAG chunking and semantic text -> crates/novex-rag/src/chunk.rs
Milvus request/search parsing -> crates/novex-rag/src/milvus.rs
RAG route constants/routes -> crates/novex-rag/src/model_routes.rs
crate facade only -> crates/novex-rag/src/lib.rs
```

Do not rewrite historical narrative unless it tells future implementers to add new logic to `src/lib.rs`.

- [ ] **Step 3: Confirm docs no longer contain stale instructions**

Run:

```bash
rg -n "Modify: `crates/novex-rag/src/lib.rs|Test: `crates/novex-rag/src/lib.rs|Use TDD in `crates/novex-rag/src/lib.rs" docs
```

Expected: no matches.

---

### Task 10: Final Verification and Commit

**Files:**
- Verify all files changed by Tasks 1-9.

**Interfaces:**
- Consumes: normalized RAG modules.
- Produces: committed, verified `novex-rag` module architecture slice.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt --all
cargo fmt --all -- --check
```

Expected: both commands exit 0.

- [ ] **Step 2: Run focused crate tests**

Run:

```bash
cargo test -p novex-rag
```

Expected: PASS.

- [ ] **Step 3: Run a root-import smoke check through backend compilation surface**

Run:

```bash
cargo test -p backend foundation_service::tests::foundation_modules_include_ai_core
```

Expected: PASS or report the exact test name if the backend test filter does not match any tests.

- [ ] **Step 4: Check diff hygiene**

Run:

```bash
git diff --check
git status --short
```

Expected: no whitespace errors; changed files are limited to RAG modules, RAG tests, and RAG source-location docs.

- [ ] **Step 5: Commit the completed RAG split**

Run:

```bash
git add crates/novex-rag/src crates/novex-rag/tests docs/plans docs/superpowers/plans/2026-06-19-novex-rag-module-architecture.md
git commit -m "refactor: split novex rag into focused modules"
```

Expected: commit succeeds.
