-- Durable lifecycle rows for tenant-bound model provider calls.

CREATE TABLE IF NOT EXISTS ai_model_provider_call_lease (
    id                 BIGINT       NOT NULL,
    tenant_id          BIGINT       NOT NULL DEFAULT 1,
    run_id             BIGINT       DEFAULT NULL,
    route_code         VARCHAR(128) NOT NULL,
    route_purpose      VARCHAR(64)  NOT NULL,
    provider_type      VARCHAR(64)  NOT NULL,
    model_name         VARCHAR(255) DEFAULT NULL,
    request_kind       VARCHAR(64)  NOT NULL DEFAULT 'model_call',
    source             VARCHAR(128) NOT NULL DEFAULT 'model_runtime',
    attempt_kind       VARCHAR(32)  NOT NULL DEFAULT 'primary',
    status             VARCHAR(32)  NOT NULL DEFAULT 'running',
    lease_owner        VARCHAR(128) NOT NULL,
    lease_expires_at   TIMESTAMP    NOT NULL,
    heartbeat_at       TIMESTAMP    NOT NULL,
    started_at         TIMESTAMP    NOT NULL,
    completed_at       TIMESTAMP    DEFAULT NULL,
    latency_ms         BIGINT       DEFAULT NULL,
    prompt_tokens      BIGINT       NOT NULL DEFAULT 0,
    completion_tokens  BIGINT       NOT NULL DEFAULT 0,
    total_tokens       BIGINT       NOT NULL DEFAULT 0,
    cost_cents         NUMERIC(12, 4) DEFAULT NULL,
    error_kind         VARCHAR(64)  DEFAULT NULL,
    http_status        INTEGER      DEFAULT NULL,
    error_message      TEXT         DEFAULT NULL,
    request_payload    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    response_payload   JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user        BIGINT       NOT NULL,
    create_time        TIMESTAMP    NOT NULL,
    update_user        BIGINT       DEFAULT NULL,
    update_time        TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_model_provider_call_lease_tenant
    ON ai_model_provider_call_lease (tenant_id);

CREATE INDEX IF NOT EXISTS idx_ai_model_provider_call_lease_run
    ON ai_model_provider_call_lease (tenant_id, run_id, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_ai_model_provider_call_lease_route
    ON ai_model_provider_call_lease (tenant_id, route_code, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_ai_model_provider_call_lease_status
    ON ai_model_provider_call_lease (tenant_id, status, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_ai_model_provider_call_lease_active
    ON ai_model_provider_call_lease (tenant_id, status, lease_expires_at, heartbeat_at)
    WHERE status = 'running';
