-- Durable Agent run execution queue.

CREATE TABLE IF NOT EXISTS ai_agent_run_queue (
    id              BIGINT       NOT NULL,
    tenant_id       BIGINT       NOT NULL DEFAULT 1,
    run_id          BIGINT       NOT NULL,
    queue_status    VARCHAR(32)  NOT NULL DEFAULT 'pending',
    priority        INTEGER      NOT NULL DEFAULT 0,
    attempt_count   INTEGER      NOT NULL DEFAULT 0,
    max_attempts    INTEGER      NOT NULL DEFAULT 3,
    locked_by       VARCHAR(128) DEFAULT NULL,
    locked_until    TIMESTAMP    DEFAULT NULL,
    last_error      TEXT         DEFAULT NULL,
    payload JSONB                NOT NULL DEFAULT '{}'::jsonb,
    queued_at       TIMESTAMP    NOT NULL,
    started_at      TIMESTAMP    DEFAULT NULL,
    finished_at     TIMESTAMP    DEFAULT NULL,
    create_user     BIGINT       NOT NULL,
    create_time     TIMESTAMP    NOT NULL,
    update_user     BIGINT       DEFAULT NULL,
    update_time     TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id),
    CONSTRAINT uk_ai_agent_run_queue_tenant_run UNIQUE (tenant_id, run_id)
);

CREATE INDEX IF NOT EXISTS idx_ai_agent_run_queue_tenant_id
    ON ai_agent_run_queue (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_run_queue_run_id
    ON ai_agent_run_queue (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_run_queue_status_lease
    ON ai_agent_run_queue (queue_status, locked_until, queued_at ASC);
CREATE INDEX IF NOT EXISTS idx_ai_agent_run_queue_priority
    ON ai_agent_run_queue (priority DESC, queued_at ASC, id ASC);
