-- Preserve parser-worker layout blocks for citation anchors and future document preview.

CREATE TABLE IF NOT EXISTS ai_document_block (
    id            BIGINT       NOT NULL,
    tenant_id     BIGINT       NOT NULL DEFAULT 1,
    dataset_id    BIGINT       NOT NULL,
    document_id   BIGINT       NOT NULL,
    block_uid     VARCHAR(128) NOT NULL,
    block_index   INTEGER      NOT NULL,
    block_type    VARCHAR(64)  NOT NULL,
    text          TEXT         NOT NULL DEFAULT '',
    page_no       INTEGER      DEFAULT NULL,
    section_path  JSONB        NOT NULL DEFAULT '[]'::jsonb,
    bbox          JSONB        NOT NULL DEFAULT '{}'::jsonb,
    metadata      JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user   BIGINT       NOT NULL,
    create_time   TIMESTAMP    NOT NULL,
    update_user   BIGINT       DEFAULT NULL,
    update_time   TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_document_block_uid
    ON ai_document_block (tenant_id, document_id, block_uid);

CREATE INDEX IF NOT EXISTS idx_ai_document_block_tenant_id
    ON ai_document_block (tenant_id);

CREATE INDEX IF NOT EXISTS idx_ai_document_block_dataset_id
    ON ai_document_block (dataset_id);

CREATE INDEX IF NOT EXISTS idx_ai_document_block_document_id
    ON ai_document_block (document_id);

CREATE INDEX IF NOT EXISTS idx_ai_document_block_type
    ON ai_document_block (block_type);

CREATE INDEX IF NOT EXISTS idx_ai_document_block_page_no
    ON ai_document_block (page_no);

CREATE INDEX IF NOT EXISTS idx_ai_document_block_section_path
    ON ai_document_block USING GIN (section_path);
