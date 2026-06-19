-- Trigger webhook event log and signature/idempotency contract for M2 webhook POC.

ALTER TABLE ai_trigger
    ADD COLUMN IF NOT EXISTS signature_secret_ref VARCHAR(256) DEFAULT NULL;

CREATE TABLE IF NOT EXISTS ai_trigger_event (
    id                BIGINT       NOT NULL,
    tenant_id         BIGINT       NOT NULL DEFAULT 1,
    trigger_id        BIGINT       NOT NULL,
    trigger_code      VARCHAR(128) NOT NULL,
    source_type       VARCHAR(32)  NOT NULL DEFAULT 'webhook',
    target_kind       VARCHAR(32)  NOT NULL,
    idempotency_key   VARCHAR(128) NOT NULL,
    signature_header  TEXT         NOT NULL,
    event_payload     JSONB        NOT NULL DEFAULT '{}'::jsonb,
    route_snapshot    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status            VARCHAR(32)  NOT NULL DEFAULT 'accepted',
    trace_id          BIGINT       DEFAULT NULL,
    error_message     TEXT         DEFAULT NULL,
    create_user       BIGINT       NOT NULL DEFAULT 1,
    create_time       TIMESTAMP    NOT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_trigger_event_tenant_trigger_idempotency
    ON ai_trigger_event (tenant_id, trigger_id, idempotency_key);
CREATE INDEX IF NOT EXISTS idx_ai_trigger_event_tenant_id ON ai_trigger_event (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_trigger_event_trigger_id ON ai_trigger_event (trigger_id);
CREATE INDEX IF NOT EXISTS idx_ai_trigger_event_trace_id ON ai_trigger_event (trace_id);
CREATE INDEX IF NOT EXISTS idx_ai_trigger_event_create_time ON ai_trigger_event (create_time DESC);

UPDATE ai_trigger
SET signature_secret_ref = COALESCE(signature_secret_ref, 'env:NOVEX_TRAINING_WEBHOOK_SECRET'),
    route_config = route_config || '{"signatureSecretRef":"env:NOVEX_TRAINING_WEBHOOK_SECRET"}'::jsonb
WHERE tenant_id = 1
  AND code = 'webhook.training.event'
  AND trigger_kind = 'webhook';
