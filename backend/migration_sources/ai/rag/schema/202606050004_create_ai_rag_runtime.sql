-- RAG runtime schema for Novex M1.

CREATE TABLE IF NOT EXISTS ai_parser_job (
    id               BIGINT    NOT NULL,
    tenant_id        BIGINT    NOT NULL DEFAULT 1,
    dataset_id       BIGINT    NOT NULL,
    document_id      BIGINT    NOT NULL,
    job_type         SMALLINT  NOT NULL DEFAULT 1,
    status           SMALLINT  NOT NULL DEFAULT 1,
    attempt_count    INTEGER   NOT NULL DEFAULT 0,
    error_message    TEXT      DEFAULT NULL,
    result_summary   JSONB     NOT NULL DEFAULT '{}'::jsonb,
    create_user      BIGINT    NOT NULL,
    create_time      TIMESTAMP NOT NULL,
    update_user      BIGINT    DEFAULT NULL,
    update_time      TIMESTAMP DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_parser_job_tenant_id ON ai_parser_job (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_parser_job_dataset_id ON ai_parser_job (dataset_id);
CREATE INDEX IF NOT EXISTS idx_ai_parser_job_document_id ON ai_parser_job (document_id);
CREATE INDEX IF NOT EXISTS idx_ai_parser_job_status ON ai_parser_job (status);
CREATE INDEX IF NOT EXISTS idx_ai_parser_job_create_time ON ai_parser_job (create_time DESC);

CREATE TABLE IF NOT EXISTS ai_document_chunk (
    id                BIGINT       NOT NULL,
    tenant_id         BIGINT       NOT NULL DEFAULT 1,
    dataset_id        BIGINT       NOT NULL,
    document_id       BIGINT       NOT NULL,
    chunk_uid         VARCHAR(128) NOT NULL,
    chunk_index       INTEGER      NOT NULL,
    content           TEXT         NOT NULL,
    token_count       INTEGER      NOT NULL DEFAULT 0,
    citation          JSONB        NOT NULL DEFAULT '{}'::jsonb,
    embedding_model   VARCHAR(128) DEFAULT NULL,
    embedding_status  SMALLINT     NOT NULL DEFAULT 1,
    embedding_ref     VARCHAR(256) DEFAULT NULL,
    metadata          JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user       BIGINT       NOT NULL,
    create_time       TIMESTAMP    NOT NULL,
    update_user       BIGINT       DEFAULT NULL,
    update_time       TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_document_chunk_document_index ON ai_document_chunk (tenant_id, document_id, chunk_index);
CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_document_chunk_uid ON ai_document_chunk (tenant_id, chunk_uid);
CREATE INDEX IF NOT EXISTS idx_ai_document_chunk_tenant_id ON ai_document_chunk (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_document_chunk_dataset_id ON ai_document_chunk (dataset_id);
CREATE INDEX IF NOT EXISTS idx_ai_document_chunk_document_id ON ai_document_chunk (document_id);
CREATE INDEX IF NOT EXISTS idx_ai_document_chunk_embedding_status ON ai_document_chunk (embedding_status);
CREATE INDEX IF NOT EXISTS idx_ai_document_chunk_create_time ON ai_document_chunk (create_time DESC);

CREATE TABLE IF NOT EXISTS ai_rag_trace (
    id                   BIGINT    NOT NULL,
    tenant_id            BIGINT    NOT NULL DEFAULT 1,
    dataset_id           BIGINT    NOT NULL,
    question             TEXT      NOT NULL,
    answer               TEXT      NOT NULL,
    answer_strategy      VARCHAR(64) NOT NULL,
    retrieval_mode       SMALLINT  NOT NULL DEFAULT 3,
    embedding_model_route VARCHAR(128) DEFAULT NULL,
    rerank_model_route   VARCHAR(128) DEFAULT NULL,
    answer_model_route   VARCHAR(128) DEFAULT NULL,
    retrieval_hit_count  INTEGER   NOT NULL DEFAULT 0,
    context_token_count  INTEGER   NOT NULL DEFAULT 0,
    output_token_count   INTEGER   NOT NULL DEFAULT 0,
    create_user          BIGINT    NOT NULL,
    create_time          TIMESTAMP NOT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_rag_trace_tenant_id ON ai_rag_trace (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_rag_trace_dataset_id ON ai_rag_trace (dataset_id);
CREATE INDEX IF NOT EXISTS idx_ai_rag_trace_create_user ON ai_rag_trace (create_user);
CREATE INDEX IF NOT EXISTS idx_ai_rag_trace_create_time ON ai_rag_trace (create_time DESC);

CREATE TABLE IF NOT EXISTS ai_rag_trace_hit (
    id              BIGINT    NOT NULL,
    tenant_id       BIGINT    NOT NULL DEFAULT 1,
    trace_id        BIGINT    NOT NULL,
    dataset_id      BIGINT    NOT NULL,
    document_id     BIGINT    NOT NULL,
    chunk_id        BIGINT    NOT NULL,
    rank            INTEGER   NOT NULL,
    score           REAL      NOT NULL DEFAULT 0,
    citation        JSONB     NOT NULL DEFAULT '{}'::jsonb,
    content_preview TEXT      NOT NULL DEFAULT '',
    create_time     TIMESTAMP NOT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_rag_trace_hit_tenant_id ON ai_rag_trace_hit (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_rag_trace_hit_trace_id ON ai_rag_trace_hit (trace_id);
CREATE INDEX IF NOT EXISTS idx_ai_rag_trace_hit_dataset_id ON ai_rag_trace_hit (dataset_id);
CREATE INDEX IF NOT EXISTS idx_ai_rag_trace_hit_document_id ON ai_rag_trace_hit (document_id);
