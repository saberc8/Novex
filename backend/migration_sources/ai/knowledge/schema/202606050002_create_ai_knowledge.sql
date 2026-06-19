-- Knowledge metadata schema for Novex M1.

CREATE TABLE IF NOT EXISTS ai_dataset (
    id               BIGINT       NOT NULL,
    tenant_id        BIGINT       NOT NULL DEFAULT 1,
    name             VARCHAR(100) NOT NULL,
    description      TEXT         DEFAULT NULL,
    owner_id         BIGINT       NOT NULL,
    visibility       SMALLINT     NOT NULL DEFAULT 1,
    acl_policy       JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status           SMALLINT     NOT NULL DEFAULT 1,
    retrieval_mode   SMALLINT     NOT NULL DEFAULT 3,
    document_count   INTEGER      NOT NULL DEFAULT 0,
    chunk_count      INTEGER      NOT NULL DEFAULT 0,
    create_user      BIGINT       NOT NULL,
    create_time      TIMESTAMP    NOT NULL,
    update_user      BIGINT       DEFAULT NULL,
    update_time      TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_dataset_tenant_name ON ai_dataset (tenant_id, name);
CREATE INDEX IF NOT EXISTS idx_ai_dataset_tenant_id ON ai_dataset (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_dataset_owner_id ON ai_dataset (owner_id);
CREATE INDEX IF NOT EXISTS idx_ai_dataset_status ON ai_dataset (status);
CREATE INDEX IF NOT EXISTS idx_ai_dataset_create_time ON ai_dataset (create_time DESC);

CREATE TABLE IF NOT EXISTS ai_document (
    id                BIGINT       NOT NULL,
    tenant_id         BIGINT       NOT NULL DEFAULT 1,
    dataset_id        BIGINT       NOT NULL,
    name              VARCHAR(255) NOT NULL,
    source_uri        TEXT         DEFAULT NULL,
    file_id           BIGINT       DEFAULT NULL,
    content_type      VARCHAR(255) DEFAULT NULL,
    owner_id          BIGINT       NOT NULL,
    visibility        SMALLINT     NOT NULL DEFAULT 1,
    acl_policy        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    parse_status      SMALLINT     NOT NULL DEFAULT 1,
    ingestion_status  SMALLINT     NOT NULL DEFAULT 1,
    chunk_count       INTEGER      NOT NULL DEFAULT 0,
    source_hash       VARCHAR(256) DEFAULT NULL,
    create_user       BIGINT       NOT NULL,
    create_time       TIMESTAMP    NOT NULL,
    update_user       BIGINT       DEFAULT NULL,
    update_time       TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_document_tenant_id ON ai_document (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_document_dataset_id ON ai_document (dataset_id);
CREATE INDEX IF NOT EXISTS idx_ai_document_owner_id ON ai_document (owner_id);
CREATE INDEX IF NOT EXISTS idx_ai_document_parse_status ON ai_document (parse_status);
CREATE INDEX IF NOT EXISTS idx_ai_document_ingestion_status ON ai_document (ingestion_status);
CREATE INDEX IF NOT EXISTS idx_ai_document_create_time ON ai_document (create_time DESC);
