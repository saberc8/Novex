-- MCP OAuth persistence contract.
-- Callback state is stored as a hash; token material is stored by secretRef only.

CREATE TABLE IF NOT EXISTS ai_mcp_oauth_state (
    id                       BIGINT       NOT NULL,
    tenant_id                BIGINT       NOT NULL DEFAULT 1,
    server_id                BIGINT       NOT NULL,
    server_code              VARCHAR(128) NOT NULL,
    scope_type               VARCHAR(32)  NOT NULL,
    scope_id                 VARCHAR(128) NOT NULL,
    state_hash               VARCHAR(128) NOT NULL,
    redirect_uri             TEXT         NOT NULL,
    requested_scopes         JSONB        NOT NULL DEFAULT '[]'::jsonb,
    code_verifier_secret_ref VARCHAR(255) NOT NULL,
    client_auth              JSONB        NOT NULL DEFAULT '{}'::jsonb,
    token_endpoint           TEXT         NOT NULL,
    client_id                VARCHAR(255) NOT NULL,
    access_token_secret_ref  VARCHAR(255) NOT NULL,
    refresh_token_secret_ref VARCHAR(255) DEFAULT NULL,
    expires_at               TIMESTAMP    NOT NULL,
    consumed_at              TIMESTAMP    DEFAULT NULL,
    status                   SMALLINT     NOT NULL DEFAULT 1,
    metadata                 JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user              BIGINT       NOT NULL,
    create_time              TIMESTAMP    NOT NULL,
    update_user              BIGINT       DEFAULT NULL,
    update_time              TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_mcp_oauth_state_hash
    ON ai_mcp_oauth_state (state_hash);

CREATE INDEX IF NOT EXISTS idx_ai_mcp_oauth_state_server
    ON ai_mcp_oauth_state (tenant_id, server_id, expires_at DESC);

CREATE INDEX IF NOT EXISTS idx_ai_mcp_oauth_state_scope
    ON ai_mcp_oauth_state (tenant_id, server_id, scope_type, scope_id);

CREATE INDEX IF NOT EXISTS idx_ai_mcp_oauth_state_active
    ON ai_mcp_oauth_state (tenant_id, server_id, expires_at)
    WHERE consumed_at IS NULL AND status = 1;

CREATE TABLE IF NOT EXISTS ai_mcp_oauth_session (
    id                       BIGINT       NOT NULL,
    tenant_id                BIGINT       NOT NULL DEFAULT 1,
    server_id                BIGINT       NOT NULL,
    server_code              VARCHAR(128) NOT NULL,
    scope_type               VARCHAR(32)  NOT NULL,
    scope_id                 VARCHAR(128) NOT NULL,
    access_token_secret_ref  VARCHAR(255) NOT NULL,
    refresh_token_secret_ref VARCHAR(255) DEFAULT NULL,
    token_type               VARCHAR(32)  NOT NULL DEFAULT 'Bearer',
    scopes                   JSONB        NOT NULL DEFAULT '[]'::jsonb,
    expires_at               TIMESTAMP    DEFAULT NULL,
    refresh_needed_after     TIMESTAMP    DEFAULT NULL,
    last_refreshed_at        TIMESTAMP    DEFAULT NULL,
    revoked_at               TIMESTAMP    DEFAULT NULL,
    status                   SMALLINT     NOT NULL DEFAULT 1,
    metadata                 JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user              BIGINT       NOT NULL,
    create_time              TIMESTAMP    NOT NULL,
    update_user              BIGINT       DEFAULT NULL,
    update_time              TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_mcp_oauth_session_scope
    ON ai_mcp_oauth_session (tenant_id, server_id, scope_type, scope_id);

CREATE INDEX IF NOT EXISTS idx_ai_mcp_oauth_session_server
    ON ai_mcp_oauth_session (tenant_id, server_id, status);

CREATE INDEX IF NOT EXISTS idx_ai_mcp_oauth_session_scope
    ON ai_mcp_oauth_session (tenant_id, scope_type, scope_id);

CREATE INDEX IF NOT EXISTS idx_ai_mcp_oauth_session_refresh
    ON ai_mcp_oauth_session (tenant_id, status, refresh_needed_after)
    WHERE revoked_at IS NULL;
