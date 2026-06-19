-- RAG vector persistence contract for M1/M5 delivery.
-- Milvus remains the default vector backend, while vector JSONB keeps local POC
-- retrieval reproducible and auditable without an external service.

CREATE TABLE IF NOT EXISTS ai_vector_collection (
    id                  BIGINT       PRIMARY KEY,
    tenant_id           BIGINT       NOT NULL DEFAULT 1,
    dataset_id          BIGINT       NOT NULL,
    code                VARCHAR(128) NOT NULL,
    name                VARCHAR(255) NOT NULL,
    vector_backend      VARCHAR(64)  NOT NULL DEFAULT 'milvus',
    provider_collection VARCHAR(255) NOT NULL,
    embedding_model_route VARCHAR(128) NOT NULL DEFAULT 'local-keyword',
    dimension           INTEGER      NOT NULL DEFAULT 64,
    metric_type         VARCHAR(32)  NOT NULL DEFAULT 'cosine',
    status              SMALLINT     NOT NULL DEFAULT 1,
    index_policy        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    filter_policy       JSONB        NOT NULL DEFAULT '{}'::jsonb,
    metadata            JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user         BIGINT       NOT NULL DEFAULT 0,
    create_time         TIMESTAMP    NOT NULL DEFAULT NOW(),
    update_user         BIGINT,
    update_time         TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_vector_collection_tenant_code
    ON ai_vector_collection (tenant_id, code);
CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_vector_collection_dataset
    ON ai_vector_collection (tenant_id, dataset_id);
CREATE INDEX IF NOT EXISTS idx_ai_vector_collection_tenant_id
    ON ai_vector_collection (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_vector_collection_dataset
    ON ai_vector_collection (tenant_id, dataset_id);
CREATE INDEX IF NOT EXISTS idx_ai_vector_collection_status
    ON ai_vector_collection (tenant_id, status);

CREATE TABLE IF NOT EXISTS ai_embedding (
    id                    BIGINT       PRIMARY KEY,
    tenant_id             BIGINT       NOT NULL DEFAULT 1,
    dataset_id            BIGINT       NOT NULL,
    document_id           BIGINT       NOT NULL,
    chunk_id              BIGINT       NOT NULL,
    chunk_uid             VARCHAR(255) NOT NULL,
    collection_id         BIGINT,
    collection_code       VARCHAR(128) NOT NULL,
    embedding_ref         VARCHAR(255) NOT NULL,
    embedding_model_route VARCHAR(128) NOT NULL,
    embedding_status      SMALLINT     NOT NULL DEFAULT 1,
    dimension             INTEGER      NOT NULL,
    vector JSONB          NOT NULL DEFAULT '[]'::jsonb,
    content_hash          VARCHAR(64),
    metadata              JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user           BIGINT       NOT NULL DEFAULT 0,
    create_time           TIMESTAMP    NOT NULL DEFAULT NOW(),
    update_user           BIGINT,
    update_time           TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_embedding_ref
    ON ai_embedding (tenant_id, embedding_ref);
CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_embedding_chunk_model
    ON ai_embedding (tenant_id, chunk_id, embedding_model_route);
CREATE INDEX IF NOT EXISTS idx_ai_embedding_tenant_id
    ON ai_embedding (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_embedding_dataset
    ON ai_embedding (tenant_id, dataset_id);
CREATE INDEX IF NOT EXISTS idx_ai_embedding_document
    ON ai_embedding (tenant_id, document_id);
CREATE INDEX IF NOT EXISTS idx_ai_embedding_collection
    ON ai_embedding (tenant_id, collection_code);
CREATE INDEX IF NOT EXISTS idx_ai_embedding_status
    ON ai_embedding (tenant_id, embedding_status);
CREATE INDEX IF NOT EXISTS idx_ai_embedding_metadata
    ON ai_embedding USING GIN (metadata);
