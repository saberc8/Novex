-- Customer-facing model chat conversation history.

CREATE TABLE IF NOT EXISTS ai_model_chat_conversation (
    id                   BIGINT       NOT NULL,
    tenant_id            BIGINT       NOT NULL DEFAULT 1,
    title                VARCHAR(160) NOT NULL DEFAULT '',
    route_id             VARCHAR(128) NOT NULL DEFAULT '',
    model                VARCHAR(128) DEFAULT NULL,
    message_count        INTEGER      NOT NULL DEFAULT 0,
    last_message_preview TEXT         NOT NULL DEFAULT '',
    create_user          BIGINT       NOT NULL,
    create_time          TIMESTAMP    NOT NULL,
    update_user          BIGINT       DEFAULT NULL,
    update_time          TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_model_chat_conversation_tenant_id
    ON ai_model_chat_conversation (tenant_id);

CREATE INDEX IF NOT EXISTS idx_ai_model_chat_conversation_create_user
    ON ai_model_chat_conversation (create_user);

CREATE INDEX IF NOT EXISTS idx_ai_model_chat_conversation_update_time
    ON ai_model_chat_conversation (update_time DESC, create_time DESC);

CREATE TABLE IF NOT EXISTS ai_model_chat_message (
    id              BIGINT       NOT NULL,
    tenant_id       BIGINT       NOT NULL DEFAULT 1,
    conversation_id BIGINT       NOT NULL,
    role            VARCHAR(32)  NOT NULL,
    content         TEXT         NOT NULL,
    route_id        VARCHAR(128) DEFAULT NULL,
    model           VARCHAR(128) DEFAULT NULL,
    token_count     INTEGER      NOT NULL DEFAULT 0,
    metadata        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user     BIGINT       NOT NULL,
    create_time     TIMESTAMP    NOT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_model_chat_message_tenant_id
    ON ai_model_chat_message (tenant_id);

CREATE INDEX IF NOT EXISTS idx_ai_model_chat_message_conversation_id
    ON ai_model_chat_message (conversation_id);

CREATE INDEX IF NOT EXISTS idx_ai_model_chat_message_create_time
    ON ai_model_chat_message (create_time ASC);
