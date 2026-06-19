-- Enrich document chunks with retrieval/search contract fields.

ALTER TABLE ai_document_chunk
    ADD COLUMN IF NOT EXISTS semantic_search_text TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS segment_type VARCHAR(64) NOT NULL DEFAULT 'text',
    ADD COLUMN IF NOT EXISTS segment_index INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS page_no INTEGER DEFAULT NULL,
    ADD COLUMN IF NOT EXISTS section_path JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS content_role VARCHAR(64) NOT NULL DEFAULT 'canonical',
    ADD COLUMN IF NOT EXISTS display_capability VARCHAR(64) NOT NULL DEFAULT 'text_only';

UPDATE ai_document_chunk
SET semantic_search_text = content
WHERE semantic_search_text = '';

CREATE INDEX IF NOT EXISTS idx_ai_document_chunk_segment_type
    ON ai_document_chunk (segment_type);

CREATE INDEX IF NOT EXISTS idx_ai_document_chunk_page_no
    ON ai_document_chunk (page_no);

CREATE INDEX IF NOT EXISTS idx_ai_document_chunk_content_role
    ON ai_document_chunk (content_role);

CREATE INDEX IF NOT EXISTS idx_ai_document_chunk_section_path
    ON ai_document_chunk USING GIN (section_path);
