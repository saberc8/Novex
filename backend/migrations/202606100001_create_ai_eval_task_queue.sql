-- Durable eval task queue for real asynchronous Novex eval execution.

CREATE TABLE IF NOT EXISTS ai_eval_task (
    id                BIGINT       PRIMARY KEY,
    tenant_id         BIGINT       NOT NULL,
    run_id            BIGINT       NOT NULL,
    dataset_id        BIGINT       NOT NULL,
    case_id           BIGINT       NOT NULL,
    case_code         VARCHAR(128) NOT NULL,
    target_kind       VARCHAR(32)  NOT NULL,
    metric_kind       VARCHAR(64)  NOT NULL,
    run_mode          VARCHAR(32)  NOT NULL,
    status            VARCHAR(32)  NOT NULL DEFAULT 'queued',
    attempt           INTEGER      NOT NULL DEFAULT 0,
    max_attempts      INTEGER      NOT NULL DEFAULT 3,
    lease_owner       VARCHAR(128) DEFAULT NULL,
    lease_until       TIMESTAMP    DEFAULT NULL,
    scheduled_at      TIMESTAMP    NOT NULL,
    started_at        TIMESTAMP    DEFAULT NULL,
    finished_at       TIMESTAMP    DEFAULT NULL,
    input_snapshot    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    expected_snapshot JSONB        NOT NULL DEFAULT '{}'::jsonb,
    tags_snapshot     JSONB        NOT NULL DEFAULT '[]'::jsonb,
    runtime_config    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    trace_ref         JSONB        NOT NULL DEFAULT '{}'::jsonb,
    last_error        TEXT         DEFAULT NULL,
    create_user       BIGINT       DEFAULT NULL,
    create_time       TIMESTAMP    NOT NULL DEFAULT NOW(),
    update_user       BIGINT       DEFAULT NULL,
    update_time       TIMESTAMP    DEFAULT NULL
);

CREATE INDEX IF NOT EXISTS idx_ai_eval_task_tenant_id ON ai_eval_task (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_task_run_id ON ai_eval_task (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_task_case_id ON ai_eval_task (case_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_task_status ON ai_eval_task (status, scheduled_at ASC);
CREATE INDEX IF NOT EXISTS idx_ai_eval_task_lease ON ai_eval_task (lease_until);

CREATE TABLE IF NOT EXISTS ai_eval_outbox (
    id              BIGINT       PRIMARY KEY,
    tenant_id       BIGINT       NOT NULL,
    run_id          BIGINT       NOT NULL,
    task_id         BIGINT       NOT NULL,
    event_type      VARCHAR(64)  NOT NULL,
    payload         JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status          SMALLINT     NOT NULL DEFAULT 1,
    attempt_count   INTEGER      NOT NULL DEFAULT 0,
    last_error      TEXT         DEFAULT NULL,
    published_time  TIMESTAMP    DEFAULT NULL,
    create_user     BIGINT       DEFAULT NULL,
    create_time     TIMESTAMP    NOT NULL DEFAULT NOW(),
    update_user     BIGINT       DEFAULT NULL,
    update_time     TIMESTAMP    DEFAULT NULL,
    CONSTRAINT uq_ai_eval_outbox_task_event UNIQUE (tenant_id, task_id, event_type)
);

CREATE INDEX IF NOT EXISTS idx_ai_eval_outbox_tenant_id ON ai_eval_outbox (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_outbox_run_id ON ai_eval_outbox (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_outbox_task_id ON ai_eval_outbox (task_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_outbox_status ON ai_eval_outbox (status, create_time ASC);
