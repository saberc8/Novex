-- Capability governance schema and dry-run POC seeds for Novex M2.

CREATE TABLE IF NOT EXISTS ai_skill (
    id                 BIGINT       NOT NULL,
    tenant_id          BIGINT       NOT NULL DEFAULT 1,
    code               VARCHAR(128) NOT NULL,
    name               VARCHAR(128) NOT NULL,
    description        TEXT         DEFAULT NULL,
    status             SMALLINT     NOT NULL DEFAULT 1,
    model_route_policy JSONB        NOT NULL DEFAULT '{}'::jsonb,
    capability_refs    JSONB        NOT NULL DEFAULT '[]'::jsonb,
    metadata           JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user        BIGINT       NOT NULL,
    create_time        TIMESTAMP    NOT NULL,
    update_user        BIGINT       DEFAULT NULL,
    update_time        TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_skill_tenant_code ON ai_skill (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_skill_tenant_id ON ai_skill (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_skill_status ON ai_skill (status);

CREATE TABLE IF NOT EXISTS ai_tool (
    id              BIGINT       NOT NULL,
    tenant_id       BIGINT       NOT NULL DEFAULT 1,
    code            VARCHAR(128) NOT NULL,
    name            VARCHAR(128) NOT NULL,
    description     TEXT         DEFAULT NULL,
    tool_kind       VARCHAR(32)  NOT NULL,
    risk_level      SMALLINT     NOT NULL DEFAULT 1,
    approval_policy SMALLINT     NOT NULL DEFAULT 1,
    permission_code VARCHAR(128) DEFAULT NULL,
    executor_kind   VARCHAR(64)  NOT NULL DEFAULT 'dry_run',
    input_schema    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    output_schema   JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status          SMALLINT     NOT NULL DEFAULT 1,
    metadata        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user     BIGINT       NOT NULL,
    create_time     TIMESTAMP    NOT NULL,
    update_user     BIGINT       DEFAULT NULL,
    update_time     TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_tool_tenant_code ON ai_tool (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_tool_tenant_id ON ai_tool (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_tool_kind ON ai_tool (tool_kind);
CREATE INDEX IF NOT EXISTS idx_ai_tool_status ON ai_tool (status);

CREATE TABLE IF NOT EXISTS ai_connector (
    id               BIGINT       NOT NULL,
    tenant_id        BIGINT       NOT NULL DEFAULT 1,
    code             VARCHAR(128) NOT NULL,
    name             VARCHAR(128) NOT NULL,
    description      TEXT         DEFAULT NULL,
    connector_kind   VARCHAR(32)  NOT NULL,
    credential_scope VARCHAR(32)  NOT NULL,
    auth_type        VARCHAR(64)  NOT NULL DEFAULT 'none',
    status           SMALLINT     NOT NULL DEFAULT 1,
    metadata         JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user      BIGINT       NOT NULL,
    create_time      TIMESTAMP    NOT NULL,
    update_user      BIGINT       DEFAULT NULL,
    update_time      TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_connector_tenant_code ON ai_connector (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_connector_tenant_id ON ai_connector (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_connector_kind ON ai_connector (connector_kind);
CREATE INDEX IF NOT EXISTS idx_ai_connector_status ON ai_connector (status);

CREATE TABLE IF NOT EXISTS ai_plugin (
    id            BIGINT       NOT NULL,
    tenant_id     BIGINT       NOT NULL DEFAULT 1,
    code          VARCHAR(128) NOT NULL,
    name          VARCHAR(128) NOT NULL,
    version       VARCHAR(64)  NOT NULL,
    runtime       VARCHAR(64)  NOT NULL,
    status        SMALLINT     NOT NULL DEFAULT 1,
    manifest      JSONB        NOT NULL DEFAULT '{}'::jsonb,
    metadata      JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user   BIGINT       NOT NULL,
    create_time   TIMESTAMP    NOT NULL,
    update_user   BIGINT       DEFAULT NULL,
    update_time   TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_plugin_tenant_code ON ai_plugin (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_plugin_tenant_id ON ai_plugin (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_plugin_status ON ai_plugin (status);

CREATE TABLE IF NOT EXISTS ai_trigger (
    id                     BIGINT       NOT NULL,
    tenant_id              BIGINT       NOT NULL DEFAULT 1,
    code                   VARCHAR(128) NOT NULL,
    name                   VARCHAR(128) NOT NULL,
    description            TEXT         DEFAULT NULL,
    trigger_kind           VARCHAR(32)  NOT NULL,
    target_kind            VARCHAR(32)  NOT NULL,
    signature_required     BOOLEAN      NOT NULL DEFAULT TRUE,
    idempotency_required   BOOLEAN      NOT NULL DEFAULT TRUE,
    route_config           JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status                 SMALLINT     NOT NULL DEFAULT 1,
    metadata               JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user            BIGINT       NOT NULL,
    create_time            TIMESTAMP    NOT NULL,
    update_user            BIGINT       DEFAULT NULL,
    update_time            TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_trigger_tenant_code ON ai_trigger (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_trigger_tenant_id ON ai_trigger (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_trigger_kind ON ai_trigger (trigger_kind);
CREATE INDEX IF NOT EXISTS idx_ai_trigger_status ON ai_trigger (status);

CREATE TABLE IF NOT EXISTS ai_mcp_server (
    id               BIGINT       NOT NULL,
    tenant_id        BIGINT       NOT NULL DEFAULT 1,
    code             VARCHAR(128) NOT NULL,
    name             VARCHAR(128) NOT NULL,
    endpoint_url     TEXT         DEFAULT NULL,
    status           SMALLINT     NOT NULL DEFAULT 1,
    auth_scope       VARCHAR(64)  NOT NULL DEFAULT 'tenant',
    discovered_tools JSONB        NOT NULL DEFAULT '[]'::jsonb,
    metadata         JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user      BIGINT       NOT NULL,
    create_time      TIMESTAMP    NOT NULL,
    update_user      BIGINT       DEFAULT NULL,
    update_time      TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_mcp_server_tenant_code ON ai_mcp_server (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_mcp_server_tenant_id ON ai_mcp_server (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_mcp_server_status ON ai_mcp_server (status);

CREATE TABLE IF NOT EXISTS ai_tool_call_audit (
    id               BIGINT       NOT NULL,
    tenant_id        BIGINT       NOT NULL DEFAULT 1,
    tool_id          BIGINT       DEFAULT NULL,
    tool_code        VARCHAR(128) NOT NULL,
    caller_kind      VARCHAR(64)  NOT NULL DEFAULT 'admin',
    caller_id        BIGINT       DEFAULT NULL,
    request_payload  JSONB        NOT NULL DEFAULT '{}'::jsonb,
    response_payload JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status           VARCHAR(32)  NOT NULL,
    dry_run          BOOLEAN      NOT NULL DEFAULT TRUE,
    risk_level       SMALLINT     NOT NULL DEFAULT 1,
    permission_code  VARCHAR(128) DEFAULT NULL,
    error_message    TEXT         DEFAULT NULL,
    create_user      BIGINT       NOT NULL,
    create_time      TIMESTAMP    NOT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_tool_call_audit_tenant_id ON ai_tool_call_audit (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_tool_call_audit_tool_code ON ai_tool_call_audit (tool_code);
CREATE INDEX IF NOT EXISTS idx_ai_tool_call_audit_create_user ON ai_tool_call_audit (create_user);
CREATE INDEX IF NOT EXISTS idx_ai_tool_call_audit_create_time ON ai_tool_call_audit (create_time DESC);

INSERT INTO ai_skill
    (id, tenant_id, code, name, description, status, model_route_policy, capability_refs, metadata, create_user, create_time)
VALUES
    (3200001, 1, 'knowledge_base_chat', 'Knowledge Base Chat', 'RAG question answering skill with citations.', 1,
     '{"answerModel":"local-extractive","embeddingModel":"local-keyword","rerankModel":"none"}'::jsonb,
     '[{"kind":"tool","code":"rag.search"},{"kind":"tool","code":"feishu.message.send"}]'::jsonb,
     '{"milestone":"M2","poc":true}'::jsonb, 1, NOW()),
    (3200002, 1, 'training_assistant', 'Training Assistant', 'Employee training assistant skill with knowledge QA and notification hooks.', 1,
     '{"answerModel":"local-extractive","embeddingModel":"local-keyword","rerankModel":"none"}'::jsonb,
     '[{"kind":"tool","code":"rag.search"},{"kind":"trigger","code":"webhook.training.event"}]'::jsonb,
     '{"milestone":"M2","poc":true}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;

INSERT INTO ai_tool
    (id, tenant_id, code, name, description, tool_kind, risk_level, approval_policy, permission_code, executor_kind, input_schema, output_schema, status, metadata, create_user, create_time)
VALUES
    (3210001, 1, 'rag.search', 'RAG Search', 'Dry-run metadata for searching tenant-scoped knowledge chunks.', 'function', 1, 1, 'ai:knowledge:ask', 'dry_run',
     '{"type":"object","required":["datasetId","query"],"properties":{"datasetId":{"type":"integer"},"query":{"type":"string"}}}'::jsonb,
     '{"type":"object","properties":{"hits":{"type":"array"},"citations":{"type":"array"}}}'::jsonb,
     1, '{"poc":true,"module":"novex-rag"}'::jsonb, 1, NOW()),
    (3210002, 1, 'media.image.generate', 'Image Generation', 'Dry-run metadata for routed image generation.', 'media', 2, 2, 'ai:tool:dryRun', 'dry_run',
     '{"type":"object","required":["prompt"],"properties":{"prompt":{"type":"string"},"size":{"type":"string"}}}'::jsonb,
     '{"type":"object","properties":{"assetId":{"type":"string"},"status":{"type":"string"}}}'::jsonb,
     1, '{"poc":true,"route":"model"}'::jsonb, 1, NOW()),
    (3210003, 1, 'feishu.message.send', 'Feishu Message', 'Dry-run metadata for sending Feishu notifications.', 'connector', 2, 2, 'ai:tool:dryRun', 'dry_run',
     '{"type":"object","required":["recipient","text"],"properties":{"recipient":{"type":"string"},"text":{"type":"string"}}}'::jsonb,
     '{"type":"object","properties":{"messageId":{"type":"string"},"dryRun":{"type":"boolean"}}}'::jsonb,
     1, '{"poc":true,"connector":"feishu"}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;

INSERT INTO ai_connector
    (id, tenant_id, code, name, description, connector_kind, credential_scope, auth_type, status, metadata, create_user, create_time)
VALUES
    (3220001, 1, 'github.default', 'GitHub', 'GitHub identity and repository connector POC metadata.', 'github', 'tenant', 'oauth_app', 1,
     '{"poc":true,"resources":["repo","issue","pull_request"]}'::jsonb, 1, NOW()),
    (3220002, 1, 'feishu.default', 'Feishu', 'Feishu tenant connector POC metadata for message delivery.', 'feishu', 'tenant', 'app_secret', 1,
     '{"poc":true,"resources":["message","user"]}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;

INSERT INTO ai_plugin
    (id, tenant_id, code, name, version, runtime, status, manifest, metadata, create_user, create_time)
VALUES
    (3230001, 1, 'builtin.github-basic', 'GitHub Basic', '0.1.0', 'builtin_adapter', 1,
     '{"capabilities":[{"kind":"connector","code":"github.default"}],"permissions":["ai:connector:list"]}'::jsonb,
     '{"poc":true}'::jsonb, 1, NOW()),
    (3230002, 1, 'builtin.media-basic', 'Media Basic', '0.1.0', 'builtin_adapter', 1,
     '{"capabilities":[{"kind":"tool","code":"media.image.generate"}],"permissions":["ai:tool:dryRun"]}'::jsonb,
     '{"poc":true}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;

INSERT INTO ai_trigger
    (id, tenant_id, code, name, description, trigger_kind, target_kind, signature_required, idempotency_required, route_config, status, metadata, create_user, create_time)
VALUES
    (3240001, 1, 'webhook.training.event', 'Training Webhook', 'Signed webhook POC for training events.', 'webhook', 'run_graph', TRUE, TRUE,
     '{"path":"/ai/triggers/webhook/training","signatureHeader":"X-Novex-Signature","idempotencyHeader":"Idempotency-Key"}'::jsonb,
     1, '{"poc":true}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;

INSERT INTO ai_mcp_server
    (id, tenant_id, code, name, endpoint_url, status, auth_scope, discovered_tools, metadata, create_user, create_time)
VALUES
    (3250001, 1, 'local.dry-run', 'Local Dry Run MCP', NULL, 1, 'tenant',
     '[{"serverId":"local.dry-run","toolName":"rag.search","permissionCode":"ai:knowledge:ask"},{"serverId":"local.dry-run","toolName":"media.image.generate","permissionCode":"ai:tool:dryRun"}]'::jsonb,
     '{"poc":true,"transport":"in-process"}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;
