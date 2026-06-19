CREATE TABLE IF NOT EXISTS ai_parser_outbox (
    id              BIGINT       PRIMARY KEY,
    tenant_id       BIGINT       NOT NULL,
    dataset_id      BIGINT       NOT NULL,
    document_id     BIGINT       NOT NULL,
    parser_job_id   BIGINT       NOT NULL,
    event_type      VARCHAR(64)  NOT NULL,
    payload JSONB               NOT NULL DEFAULT '{}'::jsonb,
    status          SMALLINT     NOT NULL DEFAULT 1,
    attempt_count   INTEGER      NOT NULL DEFAULT 0,
    last_error      TEXT         DEFAULT NULL,
    published_time  TIMESTAMP    DEFAULT NULL,
    create_user     BIGINT       DEFAULT NULL,
    create_time     TIMESTAMP    NOT NULL DEFAULT NOW(),
    update_user     BIGINT       DEFAULT NULL,
    update_time     TIMESTAMP    DEFAULT NULL,
    CONSTRAINT uq_ai_parser_outbox_parser_job UNIQUE (tenant_id, parser_job_id, event_type)
);

CREATE INDEX IF NOT EXISTS idx_ai_parser_outbox_tenant_id ON ai_parser_outbox (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_parser_outbox_dataset_id ON ai_parser_outbox (dataset_id);
CREATE INDEX IF NOT EXISTS idx_ai_parser_outbox_document_id ON ai_parser_outbox (document_id);
CREATE INDEX IF NOT EXISTS idx_ai_parser_outbox_parser_job ON ai_parser_outbox (parser_job_id);
CREATE INDEX IF NOT EXISTS idx_ai_parser_outbox_status ON ai_parser_outbox (status, create_time ASC);
