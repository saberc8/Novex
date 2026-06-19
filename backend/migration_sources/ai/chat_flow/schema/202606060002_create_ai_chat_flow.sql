-- App-facing chat flow sessions and messages for Novex RAG/model chat.

CREATE TABLE IF NOT EXISTS ai_chat_flow_session (
    id                   BIGINT       NOT NULL,
    tenant_id            BIGINT       NOT NULL DEFAULT 1,
    app_code             VARCHAR(64)  NOT NULL DEFAULT 'chat-web',
    mode                 VARCHAR(32)  NOT NULL DEFAULT 'knowledge',
    dataset_id           BIGINT       DEFAULT NULL,
    title                VARCHAR(160) NOT NULL DEFAULT '',
    status               SMALLINT     NOT NULL DEFAULT 1,
    route_id             VARCHAR(128) DEFAULT NULL,
    model                VARCHAR(128) DEFAULT NULL,
    message_count        INTEGER      NOT NULL DEFAULT 0,
    last_message_preview TEXT         NOT NULL DEFAULT '',
    metadata             JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user          BIGINT       NOT NULL,
    create_time          TIMESTAMP    NOT NULL,
    update_user          BIGINT       DEFAULT NULL,
    update_time          TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_chat_flow_session_tenant_id
    ON ai_chat_flow_session (tenant_id);

CREATE INDEX IF NOT EXISTS idx_ai_chat_flow_session_create_user
    ON ai_chat_flow_session (create_user);

CREATE INDEX IF NOT EXISTS idx_ai_chat_flow_session_dataset_id
    ON ai_chat_flow_session (tenant_id, dataset_id);

CREATE INDEX IF NOT EXISTS idx_ai_chat_flow_session_mode
    ON ai_chat_flow_session (tenant_id, mode);

CREATE INDEX IF NOT EXISTS idx_ai_chat_flow_session_update_time
    ON ai_chat_flow_session (update_time DESC, create_time DESC);

CREATE TABLE IF NOT EXISTS ai_chat_flow_message (
    id              BIGINT       NOT NULL,
    tenant_id       BIGINT       NOT NULL DEFAULT 1,
    session_id      BIGINT       NOT NULL,
    role            VARCHAR(32)  NOT NULL,
    content         TEXT         NOT NULL,
    route_id        VARCHAR(128) DEFAULT NULL,
    model           VARCHAR(128) DEFAULT NULL,
    rag_trace_id    BIGINT       DEFAULT NULL,
    citations       JSONB        NOT NULL DEFAULT '[]'::jsonb,
    token_count     INTEGER      NOT NULL DEFAULT 0,
    metadata        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user     BIGINT       NOT NULL,
    create_time     TIMESTAMP    NOT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_chat_flow_message_tenant_id
    ON ai_chat_flow_message (tenant_id);

CREATE INDEX IF NOT EXISTS idx_ai_chat_flow_message_session_id
    ON ai_chat_flow_message (session_id);

CREATE INDEX IF NOT EXISTS idx_ai_chat_flow_message_rag_trace_id
    ON ai_chat_flow_message (rag_trace_id);

CREATE INDEX IF NOT EXISTS idx_ai_chat_flow_message_create_time
    ON ai_chat_flow_message (create_time ASC);
