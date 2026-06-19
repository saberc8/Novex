-- Generic AI feedback records for RAG answers, tool runs, eval promotion, and C-side UX signals.

CREATE TABLE IF NOT EXISTS ai_feedback (
    id             BIGINT       NOT NULL,
    tenant_id      BIGINT       NOT NULL DEFAULT 1,
    resource_type  VARCHAR(64)  NOT NULL,
    resource_id    VARCHAR(128) NOT NULL,
    trace_id       VARCHAR(128) DEFAULT NULL,
    rating         VARCHAR(64)  NOT NULL,
    reason         TEXT         NOT NULL DEFAULT '',
    metadata       JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status         SMALLINT     NOT NULL DEFAULT 1,
    create_user    BIGINT       NOT NULL,
    create_time    TIMESTAMP    NOT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_feedback_tenant_id ON ai_feedback (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_feedback_resource ON ai_feedback (tenant_id, resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_ai_feedback_trace_id ON ai_feedback (tenant_id, trace_id);
CREATE INDEX IF NOT EXISTS idx_ai_feedback_rating ON ai_feedback (tenant_id, rating);
CREATE INDEX IF NOT EXISTS idx_ai_feedback_create_time ON ai_feedback (create_time DESC);
