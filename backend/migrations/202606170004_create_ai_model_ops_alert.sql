-- Model ops alerts and default health-check automation.

CREATE TABLE IF NOT EXISTS ai_model_ops_alert (
    id                BIGINT       NOT NULL,
    tenant_id         BIGINT       NOT NULL DEFAULT 1,
    alert_key         VARCHAR(160) NOT NULL,
    alert_kind        VARCHAR(64)  NOT NULL,
    severity          VARCHAR(32)  NOT NULL,
    status            VARCHAR(32)  NOT NULL DEFAULT 'active',
    route_id          BIGINT       DEFAULT NULL,
    provider_id       BIGINT       DEFAULT NULL,
    model_profile_id  BIGINT       DEFAULT NULL,
    source_ref        VARCHAR(128) DEFAULT NULL,
    event_payload     JSONB        NOT NULL DEFAULT '{}'::jsonb,
    first_seen_at     TIMESTAMP    NOT NULL,
    last_seen_at      TIMESTAMP    NOT NULL,
    resolved_at       TIMESTAMP    DEFAULT NULL,
    resolve_message   TEXT         DEFAULT NULL,
    create_user       BIGINT       NOT NULL,
    create_time       TIMESTAMP    NOT NULL,
    update_user       BIGINT       DEFAULT NULL,
    update_time       TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_model_ops_alert_tenant_status
    ON ai_model_ops_alert (tenant_id, status, last_seen_at DESC);
CREATE INDEX IF NOT EXISTS idx_ai_model_ops_alert_route_id
    ON ai_model_ops_alert (route_id);
CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_model_ops_alert_active_key
    ON ai_model_ops_alert (tenant_id, alert_key)
    WHERE resolved_at IS NULL;

INSERT INTO sys_job (
    id, name, group_name, task_type, cron_expression, status, concurrent,
    misfire_policy, max_retry, timeout_seconds, http_method, http_url,
    http_headers, http_body, builtin_key, description, next_trigger_time,
    create_user, create_time
) VALUES (
    3600001, 'AI Model Health Check', 'ai-ops', 2, '*/5 * * * * *', 1, FALSE,
    1, 1, 120, NULL, NULL,
    '{}'::jsonb, NULL, 'ai.model.health_check',
    'Refresh persisted model health rows and active model ops alerts.',
    NOW(), 1, NOW()
)
ON CONFLICT DO NOTHING;
