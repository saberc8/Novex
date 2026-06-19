-- Media runtime contract for image generation tools.
-- PostgreSQL stores media job/asset metadata and audit linkage; binary assets
-- remain in external object storage or provider URLs for the POC.

CREATE TABLE IF NOT EXISTS ai_media_asset (
    id                BIGINT       PRIMARY KEY,
    tenant_id         BIGINT       NOT NULL DEFAULT 1,
    asset_uid         VARCHAR(128) NOT NULL,
    asset_kind        VARCHAR(32)  NOT NULL,
    provider          VARCHAR(64)  NOT NULL,
    provider_asset_id VARCHAR(255),
    asset_url         TEXT,
    storage_ref       VARCHAR(255),
    mime_type         VARCHAR(128),
    width             INTEGER,
    height            INTEGER,
    metadata          JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user       BIGINT       NOT NULL DEFAULT 0,
    create_time       TIMESTAMP    NOT NULL DEFAULT NOW(),
    update_user       BIGINT,
    update_time       TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_media_asset_tenant_uid
    ON ai_media_asset (tenant_id, asset_uid);
CREATE INDEX IF NOT EXISTS idx_ai_media_asset_tenant_id
    ON ai_media_asset (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_media_asset_provider
    ON ai_media_asset (tenant_id, provider);
CREATE INDEX IF NOT EXISTS idx_ai_media_asset_create_time
    ON ai_media_asset (tenant_id, create_time DESC);

CREATE TABLE IF NOT EXISTS ai_media_job (
    id                 BIGINT       PRIMARY KEY,
    tenant_id          BIGINT       NOT NULL DEFAULT 1,
    trace_id           VARCHAR(128),
    run_id             BIGINT,
    tool_call_audit_id BIGINT,
    tool_code          VARCHAR(128) NOT NULL,
    provider           VARCHAR(64)  NOT NULL,
    model_route        VARCHAR(128),
    prompt             TEXT         NOT NULL,
    request_payload    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    response_payload   JSONB        NOT NULL DEFAULT '{}'::jsonb,
    asset_id           BIGINT,
    status             VARCHAR(32)  NOT NULL,
    dry_run            BOOLEAN      NOT NULL DEFAULT TRUE,
    cost               NUMERIC(18, 6),
    latency_ms         INTEGER,
    policy_result      JSONB        NOT NULL DEFAULT '{}'::jsonb,
    error_message      TEXT,
    create_user        BIGINT       NOT NULL DEFAULT 0,
    create_time        TIMESTAMP    NOT NULL DEFAULT NOW(),
    update_user        BIGINT,
    update_time        TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_ai_media_job_tenant_id
    ON ai_media_job (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_media_job_trace
    ON ai_media_job (tenant_id, trace_id);
CREATE INDEX IF NOT EXISTS idx_ai_media_job_run
    ON ai_media_job (tenant_id, run_id);
CREATE INDEX IF NOT EXISTS idx_ai_media_job_tool_audit
    ON ai_media_job (tenant_id, tool_call_audit_id);
CREATE INDEX IF NOT EXISTS idx_ai_media_job_asset
    ON ai_media_job (tenant_id, asset_id);
CREATE INDEX IF NOT EXISTS idx_ai_media_job_status
    ON ai_media_job (tenant_id, status);
