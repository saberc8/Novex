# RAG BM25 Design

## Goal

Improve Novex knowledge retrieval accuracy for long Chinese documents by replacing the current keyword-overlap sparse retriever with a local BM25 scorer. The first target is better recall on the `置身钉内` PDF without requiring an online model.

## Scope

This change updates the in-process sparse retrieval path used by `keyword_retrieve`. It does not add a persistent inverted index, database schema, or external service. Dense embedding, Milvus, rerank, and grounded answer generation continue to use the existing paths.

## Approach

Implement BM25 inside `crates/novex-rag` over the chunks already loaded for retrieval. The scorer builds document frequencies for the current candidate chunk set, computes BM25 with length normalization, and returns the same `RetrievalHit` shape as today.

Tokenization uses pragmatic mixed-language terms:

- ASCII letters and numbers are normalized into contiguous lowercase tokens.
- CJK text emits adjacent bigrams for semantic Chinese terms.
- Selected CJK single characters remain as fallback only so very short queries still work.
- Numbered range expansion remains compatible with existing `p0` to `p5` behavior.

## Tradeoffs

This keeps the change small and safe, and it immediately improves local fallback and hybrid retrieval. The cost is that each retrieval builds BM25 statistics in memory, so very large datasets will eventually need a persistent index such as Tantivy or Milvus sparse/BM25.

## Testing

Use TDD in `crates/novex-rag/src/retrieval.rs` and `crates/novex-rag/tests/retrieval.rs`:

- Preserve existing English keyword ranking behavior.
- Preserve numbered range recall for `p0` to `p5`.
- Add Chinese long-document regression where high-frequency terms appear in many chunks but the exact Chinese phrase and date facts should rank the target chunk first.
- Add a regression showing BM25 beats pure overlap when a long noisy chunk contains many query characters but a shorter chunk contains the key Chinese phrase.

## Rollout

Keep the public function name `keyword_retrieve` so the backend integration remains unchanged. After BM25 is verified, run targeted Rust tests first, then broader backend/RAG tests if the workspace baseline allows.
