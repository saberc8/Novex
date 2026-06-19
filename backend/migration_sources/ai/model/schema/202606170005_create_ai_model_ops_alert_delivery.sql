-- Model ops alert delivery attempts and default notification bridge job.

CREATE TABLE IF NOT EXISTS ai_model_ops_alert_delivery (
    id                  BIGINT       NOT NULL,
    tenant_id           BIGINT       NOT NULL DEFAULT 1,
    alert_id            BIGINT       NOT NULL,
    alert_key           VARCHAR(160) NOT NULL,
    channel             VARCHAR(64)  NOT NULL,
    status              VARCHAR(32)  NOT NULL,
    dry_run             BOOLEAN      NOT NULL DEFAULT TRUE,
    tool_call_audit_id  BIGINT       DEFAULT NULL,
    request_payload     JSONB        NOT NULL DEFAULT '{}'::jsonb,
    response_payload    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    error_message       TEXT         DEFAULT NULL,
    create_user         BIGINT       NOT NULL,
    create_time         TIMESTAMP    NOT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_model_ops_alert_delivery_alert_id
    ON ai_model_ops_alert_delivery (alert_id);
CREATE INDEX IF NOT EXISTS idx_ai_model_ops_alert_delivery_channel_status
    ON ai_model_ops_alert_delivery (tenant_id, channel, status, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_ai_model_ops_alert_delivery_audit_id
    ON ai_model_ops_alert_delivery (tool_call_audit_id);

INSERT INTO sys_job (
    id, name, group_name, task_type, cron_expression, status, concurrent,
    misfire_policy, max_retry, timeout_seconds, http_method, http_url,
    http_headers, http_body, builtin_key, description, next_trigger_time,
    create_user, create_time
) VALUES (
    3600002, 'AI Model Alert Delivery', 'ai-ops', 2, '*/5 * * * * *', 1, FALSE,
    1, 1, 120, NULL, NULL,
    '{}'::jsonb, NULL, 'ai.model.alert_delivery',
    'Deliver active model ops alerts through the configured notification bridge.',
    NOW(), 1, NOW()
)
ON CONFLICT DO NOTHING;
