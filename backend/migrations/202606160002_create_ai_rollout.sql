-- Persisted agent rollout bundles for replay and eval case capture.

CREATE TABLE IF NOT EXISTS ai_rollout (
    id              BIGINT      NOT NULL,
    tenant_id       BIGINT      NOT NULL DEFAULT 1,
    run_id          BIGINT      NOT NULL,
    trace_id        VARCHAR(64) NOT NULL,
    event_bundle    JSONB       NOT NULL DEFAULT '{}'::jsonb,
    summary_payload JSONB       NOT NULL DEFAULT '{}'::jsonb,
    source          VARCHAR(64) NOT NULL DEFAULT 'agent_run',
    create_user     BIGINT      NOT NULL,
    create_time     TIMESTAMP   NOT NULL,
    update_user     BIGINT      DEFAULT NULL,
    update_time     TIMESTAMP   DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_rollout_run_source ON ai_rollout (run_id, source);
CREATE INDEX IF NOT EXISTS idx_ai_rollout_tenant_id ON ai_rollout (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_rollout_run_id ON ai_rollout (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_rollout_trace_id ON ai_rollout (trace_id);
CREATE INDEX IF NOT EXISTS idx_ai_rollout_source ON ai_rollout (source);
