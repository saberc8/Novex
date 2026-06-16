-- MCP gateway discovered tool registry.

CREATE TABLE IF NOT EXISTS ai_mcp_tool (
    id              BIGINT       NOT NULL,
    tenant_id       BIGINT       NOT NULL DEFAULT 1,
    server_id       BIGINT       NOT NULL REFERENCES ai_mcp_server (id) ON DELETE CASCADE,
    tool_name       VARCHAR(128) NOT NULL,
    tool_code       VARCHAR(128) NOT NULL,
    description     TEXT         DEFAULT NULL,
    input_schema    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    output_schema   JSONB        NOT NULL DEFAULT '{}'::jsonb,
    risk_level      SMALLINT     NOT NULL DEFAULT 1,
    permission_code VARCHAR(128) DEFAULT NULL,
    status          SMALLINT     NOT NULL DEFAULT 1,
    metadata        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user     BIGINT       NOT NULL,
    create_time     TIMESTAMP    NOT NULL,
    update_user     BIGINT       DEFAULT NULL,
    update_time     TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_mcp_tool_tenant_server_name
    ON ai_mcp_tool (tenant_id, server_id, tool_name);
CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_mcp_tool_tenant_tool_code
    ON ai_mcp_tool (tenant_id, tool_code);
CREATE INDEX IF NOT EXISTS idx_ai_mcp_tool_tenant_id ON ai_mcp_tool (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_mcp_tool_server_id ON ai_mcp_tool (server_id);
CREATE INDEX IF NOT EXISTS idx_ai_mcp_tool_status ON ai_mcp_tool (status);
