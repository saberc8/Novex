-- Memory policy and entry control-plane schema for Novex M0/M3.

CREATE TABLE IF NOT EXISTS ai_memory_policy (
    id                   BIGINT       NOT NULL,
    tenant_id            BIGINT       NOT NULL DEFAULT 1,
    scope_type           VARCHAR(32)  NOT NULL,
    resource_kind        VARCHAR(64)  NOT NULL DEFAULT 'default',
    resource_id          VARCHAR(128) NOT NULL DEFAULT '*',
    write_policy         VARCHAR(32)  NOT NULL DEFAULT 'user_approved',
    ttl_days             INTEGER      DEFAULT NULL,
    require_user_confirm BOOLEAN      NOT NULL DEFAULT TRUE,
    redaction_rules      JSONB        NOT NULL DEFAULT '[]'::jsonb,
    retrieval_top_k      INTEGER      NOT NULL DEFAULT 6,
    status               SMALLINT     NOT NULL DEFAULT 1,
    metadata             JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user          BIGINT       NOT NULL,
    create_time          TIMESTAMP    NOT NULL,
    update_user          BIGINT       DEFAULT NULL,
    update_time          TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_memory_policy_scope_resource
    ON ai_memory_policy (tenant_id, scope_type, resource_kind, resource_id);
CREATE INDEX IF NOT EXISTS idx_ai_memory_policy_tenant_id ON ai_memory_policy (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_memory_policy_status ON ai_memory_policy (status);

CREATE TABLE IF NOT EXISTS ai_memory (
    id              BIGINT       NOT NULL,
    tenant_id       BIGINT       NOT NULL DEFAULT 1,
    scope_type      VARCHAR(32)  NOT NULL,
    scope_id        VARCHAR(128) NOT NULL,
    owner_user_id   BIGINT       DEFAULT NULL,
    source_kind     VARCHAR(64)  NOT NULL,
    source_id       VARCHAR(128) DEFAULT NULL,
    content         TEXT         NOT NULL,
    summary         TEXT         NOT NULL,
    sensitivity     VARCHAR(32)  NOT NULL DEFAULT 'low',
    write_policy    VARCHAR(32)  NOT NULL DEFAULT 'user_approved',
    ttl_days        INTEGER      DEFAULT NULL,
    expires_at      TIMESTAMP    DEFAULT NULL,
    metadata        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status          SMALLINT     NOT NULL DEFAULT 1,
    deleted_at      TIMESTAMP    DEFAULT NULL,
    create_user     BIGINT       NOT NULL,
    create_time     TIMESTAMP    NOT NULL,
    update_user     BIGINT       DEFAULT NULL,
    update_time     TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_memory_tenant_scope ON ai_memory (tenant_id, scope_type, scope_id);
CREATE INDEX IF NOT EXISTS idx_ai_memory_owner_user_id ON ai_memory (tenant_id, owner_user_id);
CREATE INDEX IF NOT EXISTS idx_ai_memory_source ON ai_memory (tenant_id, source_kind, source_id);
CREATE INDEX IF NOT EXISTS idx_ai_memory_expires_at ON ai_memory (tenant_id, expires_at);
CREATE INDEX IF NOT EXISTS idx_ai_memory_status ON ai_memory (tenant_id, status);

INSERT INTO ai_memory_policy
    (id, tenant_id, scope_type, resource_kind, resource_id, write_policy, ttl_days, require_user_confirm, redaction_rules, retrieval_top_k, status, metadata, create_user, create_time)
VALUES
    (3600001, 1, 'session', 'default', '*', 'automatic', 1, FALSE,
     '["drop_secret_refs","drop_credentials"]'::jsonb, 4, 1, '{"poc":true}'::jsonb, 1, NOW()),
    (3600002, 1, 'user', 'default', '*', 'user_approved', 365, TRUE,
     '["drop_secret_refs","drop_credentials","mask_email"]'::jsonb, 6, 1, '{"poc":true}'::jsonb, 1, NOW()),
    (3600003, 1, 'org', 'default', '*', 'user_approved', NULL, TRUE,
     '["drop_secret_refs","drop_credentials"]'::jsonb, 8, 1, '{"poc":true}'::jsonb, 1, NOW()),
    (3600004, 1, 'project', 'default', '*', 'user_approved', 180, TRUE,
     '["drop_secret_refs","drop_credentials"]'::jsonb, 8, 1, '{"poc":true}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, scope_type, resource_kind, resource_id) DO NOTHING;
