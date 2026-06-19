-- Notebook workspace layer over existing knowledge datasets, documents, citations, and traces.

CREATE TABLE IF NOT EXISTS ai_notebook_workspace (
    id          BIGINT       NOT NULL,
    tenant_id   BIGINT       NOT NULL DEFAULT 1,
    owner_id    BIGINT       NOT NULL,
    name        VARCHAR(128) NOT NULL,
    description TEXT         DEFAULT NULL,
    metadata    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status      SMALLINT     NOT NULL DEFAULT 1,
    create_user BIGINT       NOT NULL,
    create_time TIMESTAMP    NOT NULL,
    update_user BIGINT       DEFAULT NULL,
    update_time TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_notebook_workspace_owner_name
    ON ai_notebook_workspace (tenant_id, owner_id, name);
CREATE INDEX IF NOT EXISTS idx_ai_notebook_workspace_tenant_id
    ON ai_notebook_workspace (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_notebook_workspace_owner
    ON ai_notebook_workspace (tenant_id, owner_id, status, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_ai_notebook_workspace_status
    ON ai_notebook_workspace (tenant_id, status);

CREATE TABLE IF NOT EXISTS ai_notebook_source (
    id                    BIGINT       NOT NULL,
    tenant_id             BIGINT       NOT NULL DEFAULT 1,
    workspace_id          BIGINT       NOT NULL,
    source_type           VARCHAR(64)  NOT NULL,
    knowledge_dataset_id  BIGINT       DEFAULT NULL,
    knowledge_document_id BIGINT       DEFAULT NULL,
    title                 VARCHAR(255) NOT NULL,
    citation_metadata     JSONB        NOT NULL DEFAULT '{}'::jsonb,
    metadata              JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status                SMALLINT     NOT NULL DEFAULT 1,
    create_user           BIGINT       NOT NULL,
    create_time           TIMESTAMP    NOT NULL,
    update_user           BIGINT       DEFAULT NULL,
    update_time           TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_notebook_source_workspace
    ON ai_notebook_source (tenant_id, workspace_id, status, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_ai_notebook_source_dataset
    ON ai_notebook_source (tenant_id, knowledge_dataset_id);
CREATE INDEX IF NOT EXISTS idx_ai_notebook_source_document
    ON ai_notebook_source (tenant_id, knowledge_document_id);
CREATE INDEX IF NOT EXISTS idx_ai_notebook_source_type
    ON ai_notebook_source (tenant_id, source_type);

CREATE TABLE IF NOT EXISTS ai_notebook_artifact (
    id               BIGINT       NOT NULL,
    tenant_id        BIGINT       NOT NULL DEFAULT 1,
    workspace_id     BIGINT       NOT NULL,
    artifact_kind    VARCHAR(64)  NOT NULL,
    title            VARCHAR(255) NOT NULL,
    content_json     JSONB        NOT NULL DEFAULT '{}'::jsonb,
    content_text     TEXT         NOT NULL DEFAULT '',
    citation_payload JSONB        NOT NULL DEFAULT '[]'::jsonb,
    source_trace_id  VARCHAR(128) DEFAULT NULL,
    metadata         JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status           SMALLINT     NOT NULL DEFAULT 1,
    create_user      BIGINT       NOT NULL,
    create_time      TIMESTAMP    NOT NULL,
    update_user      BIGINT       DEFAULT NULL,
    update_time      TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_notebook_artifact_workspace
    ON ai_notebook_artifact (tenant_id, workspace_id, status, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_ai_notebook_artifact_kind
    ON ai_notebook_artifact (tenant_id, artifact_kind, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_ai_notebook_artifact_trace
    ON ai_notebook_artifact (tenant_id, source_trace_id);
