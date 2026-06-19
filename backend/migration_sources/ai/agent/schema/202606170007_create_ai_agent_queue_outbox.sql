CREATE TABLE IF NOT EXISTS ai_agent_queue_outbox (
    id              BIGINT       PRIMARY KEY,
    tenant_id       BIGINT       NOT NULL,
    queue_id        BIGINT       NOT NULL,
    run_id          BIGINT       NOT NULL,
    event_type      VARCHAR(64)  NOT NULL,
    max_attempts    INTEGER      NOT NULL DEFAULT 3,
    payload JSONB               NOT NULL DEFAULT '{}'::jsonb,
    status          SMALLINT     NOT NULL DEFAULT 1,
    attempt_count   INTEGER      NOT NULL DEFAULT 0,
    last_error      TEXT         DEFAULT NULL,
    published_time  TIMESTAMP    DEFAULT NULL,
    create_user     BIGINT       DEFAULT NULL,
    create_time     TIMESTAMP    NOT NULL DEFAULT NOW(),
    update_user     BIGINT       DEFAULT NULL,
    update_time     TIMESTAMP    DEFAULT NULL,
    CONSTRAINT uq_ai_agent_queue_outbox_queue_event UNIQUE (tenant_id, queue_id, event_type)
);

CREATE INDEX IF NOT EXISTS idx_ai_agent_queue_outbox_tenant_id
    ON ai_agent_queue_outbox (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_queue_outbox_queue_id
    ON ai_agent_queue_outbox (queue_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_queue_outbox_run_id
    ON ai_agent_queue_outbox (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_queue_outbox_status
    ON ai_agent_queue_outbox (status, create_time ASC, id ASC);
