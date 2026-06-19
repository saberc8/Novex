-- Agent runtime and shared Run Graph schema for Novex M3.

CREATE TABLE IF NOT EXISTS ai_run (
    id              BIGINT       NOT NULL,
    tenant_id       BIGINT       NOT NULL DEFAULT 1,
    app_id          BIGINT       DEFAULT NULL,
    run_type        VARCHAR(32)  NOT NULL,
    status          VARCHAR(32)  NOT NULL,
    source_type     VARCHAR(64)  NOT NULL DEFAULT 'admin',
    source_id       VARCHAR(128) DEFAULT NULL,
    trace_id        VARCHAR(64)  NOT NULL,
    input_payload   JSONB        NOT NULL DEFAULT '{}'::jsonb,
    output_payload  JSONB        NOT NULL DEFAULT '{}'::jsonb,
    budget_policy   JSONB        NOT NULL DEFAULT '{}'::jsonb,
    cost_cents      NUMERIC(12, 4) NOT NULL DEFAULT 0,
    latency_ms      BIGINT       DEFAULT NULL,
    created_by      BIGINT       NOT NULL,
    started_at      TIMESTAMP    DEFAULT NULL,
    finished_at     TIMESTAMP    DEFAULT NULL,
    create_user     BIGINT       NOT NULL,
    create_time     TIMESTAMP    NOT NULL,
    update_user     BIGINT       DEFAULT NULL,
    update_time     TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_run_trace_id ON ai_run (trace_id);
CREATE INDEX IF NOT EXISTS idx_ai_run_tenant_id ON ai_run (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_run_status ON ai_run (status);
CREATE INDEX IF NOT EXISTS idx_ai_run_created_by ON ai_run (created_by);
CREATE INDEX IF NOT EXISTS idx_ai_run_create_time ON ai_run (create_time DESC);

CREATE TABLE IF NOT EXISTS ai_run_step (
    id                 BIGINT       NOT NULL,
    tenant_id          BIGINT       NOT NULL DEFAULT 1,
    run_id             BIGINT       NOT NULL,
    parent_step_id     BIGINT       DEFAULT NULL,
    step_type          VARCHAR(64)  NOT NULL,
    status             VARCHAR(32)  NOT NULL,
    sequence_no        BIGINT       NOT NULL,
    input_payload      JSONB        NOT NULL DEFAULT '{}'::jsonb,
    output_payload     JSONB        NOT NULL DEFAULT '{}'::jsonb,
    tool_call_audit_id BIGINT       DEFAULT NULL,
    model_profile_id   BIGINT       DEFAULT NULL,
    retry_count        INTEGER      NOT NULL DEFAULT 0,
    cost_cents         NUMERIC(12, 4) NOT NULL DEFAULT 0,
    latency_ms         BIGINT       DEFAULT NULL,
    create_user        BIGINT       NOT NULL,
    create_time        TIMESTAMP    NOT NULL,
    update_user        BIGINT       DEFAULT NULL,
    update_time        TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_run_step_run_sequence ON ai_run_step (run_id, sequence_no);
CREATE INDEX IF NOT EXISTS idx_ai_run_step_tenant_id ON ai_run_step (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_run_step_run_id ON ai_run_step (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_run_step_type ON ai_run_step (step_type);
CREATE INDEX IF NOT EXISTS idx_ai_run_step_status ON ai_run_step (status);

CREATE TABLE IF NOT EXISTS ai_run_event (
    id           BIGINT      NOT NULL,
    tenant_id    BIGINT      NOT NULL DEFAULT 1,
    run_id       BIGINT      NOT NULL,
    step_id      BIGINT      DEFAULT NULL,
    event_type   VARCHAR(64) NOT NULL,
    sequence_no  BIGINT      NOT NULL,
    status       VARCHAR(32) NOT NULL,
    payload      JSONB       NOT NULL DEFAULT '{}'::jsonb,
    create_user  BIGINT      NOT NULL,
    create_time  TIMESTAMP   NOT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_run_event_run_sequence ON ai_run_event (run_id, sequence_no);
CREATE INDEX IF NOT EXISTS idx_ai_run_event_tenant_id ON ai_run_event (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_run_event_run_id ON ai_run_event (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_run_event_type ON ai_run_event (event_type);
CREATE INDEX IF NOT EXISTS idx_ai_run_event_create_time ON ai_run_event (create_time DESC);

CREATE TABLE IF NOT EXISTS ai_run_pause (
    id                     BIGINT       NOT NULL,
    tenant_id              BIGINT       NOT NULL DEFAULT 1,
    run_id                 BIGINT       NOT NULL,
    step_id                BIGINT       DEFAULT NULL,
    pause_reason           VARCHAR(64)  NOT NULL,
    status                 VARCHAR(32)  NOT NULL DEFAULT 'active',
    requested_input_schema JSONB        NOT NULL DEFAULT '{}'::jsonb,
    resume_token_hash      VARCHAR(128) DEFAULT NULL,
    resume_payload         JSONB        NOT NULL DEFAULT '{}'::jsonb,
    expires_at             TIMESTAMP    DEFAULT NULL,
    resumed_at             TIMESTAMP    DEFAULT NULL,
    create_user            BIGINT       NOT NULL,
    create_time            TIMESTAMP    NOT NULL,
    update_user            BIGINT       DEFAULT NULL,
    update_time            TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_run_pause_tenant_id ON ai_run_pause (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_run_pause_run_id ON ai_run_pause (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_run_pause_status ON ai_run_pause (status);
CREATE INDEX IF NOT EXISTS idx_ai_run_pause_reason ON ai_run_pause (pause_reason);

CREATE TABLE IF NOT EXISTS ai_agent_run (
    id                 BIGINT       NOT NULL,
    tenant_id          BIGINT       NOT NULL DEFAULT 1,
    run_id             BIGINT       NOT NULL,
    intent             VARCHAR(64)  NOT NULL,
    loop_kind          VARCHAR(64)  NOT NULL,
    selected_tool_code VARCHAR(128) DEFAULT NULL,
    status             VARCHAR(32)  NOT NULL,
    final_output       TEXT         DEFAULT NULL,
    pause_reason       VARCHAR(64)  DEFAULT NULL,
    task_budget        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    metadata           JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user        BIGINT       NOT NULL,
    create_time        TIMESTAMP    NOT NULL,
    update_user        BIGINT       DEFAULT NULL,
    update_time        TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_agent_run_run_id ON ai_agent_run (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_run_tenant_id ON ai_agent_run (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_run_status ON ai_agent_run (status);
CREATE INDEX IF NOT EXISTS idx_ai_agent_run_intent ON ai_agent_run (intent);

CREATE TABLE IF NOT EXISTS ai_agent_trace (
    id                   BIGINT      NOT NULL,
    tenant_id            BIGINT      NOT NULL DEFAULT 1,
    run_id               BIGINT      NOT NULL,
    trace_id             VARCHAR(64) NOT NULL,
    event_snapshot       JSONB       NOT NULL DEFAULT '[]'::jsonb,
    model_route_snapshot JSONB       NOT NULL DEFAULT '{}'::jsonb,
    tool_snapshot        JSONB       NOT NULL DEFAULT '{}'::jsonb,
    metadata             JSONB       NOT NULL DEFAULT '{}'::jsonb,
    create_user          BIGINT      NOT NULL,
    create_time          TIMESTAMP   NOT NULL,
    update_user          BIGINT      DEFAULT NULL,
    update_time          TIMESTAMP   DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_agent_trace_run_id ON ai_agent_trace (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_trace_tenant_id ON ai_agent_trace (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_trace_trace_id ON ai_agent_trace (trace_id);
