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

CREATE TABLE IF NOT EXISTS ai_plugin_version (
    id            BIGINT       NOT NULL,
    tenant_id     BIGINT       NOT NULL DEFAULT 1,
    plugin_id     BIGINT       NOT NULL,
    version       VARCHAR(64)  NOT NULL,
    runtime       VARCHAR(64)  NOT NULL,
    manifest      JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status        SMALLINT     NOT NULL DEFAULT 1,
    create_user   BIGINT       NOT NULL,
    create_time   TIMESTAMP    NOT NULL,
    update_user   BIGINT       DEFAULT NULL,
    update_time   TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_plugin_version_plugin_version
    ON ai_plugin_version (tenant_id, plugin_id, version);
CREATE INDEX IF NOT EXISTS idx_ai_plugin_version_tenant_id ON ai_plugin_version (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_plugin_version_plugin_id ON ai_plugin_version (plugin_id);
CREATE INDEX IF NOT EXISTS idx_ai_plugin_version_status ON ai_plugin_version (status);

CREATE TABLE IF NOT EXISTS ai_plugin_installation (
    id                BIGINT       NOT NULL,
    tenant_id         BIGINT       NOT NULL DEFAULT 1,
    plugin_id         BIGINT       NOT NULL,
    plugin_version_id BIGINT       NOT NULL,
    enabled           BOOLEAN      NOT NULL DEFAULT TRUE,
    install_source    VARCHAR(64)  NOT NULL DEFAULT 'builtin',
    permission_grants JSONB        NOT NULL DEFAULT '[]'::jsonb,
    config            JSONB        NOT NULL DEFAULT '{}'::jsonb,
    installed_by      BIGINT       NOT NULL,
    installed_at      TIMESTAMP    NOT NULL,
    status            SMALLINT     NOT NULL DEFAULT 1,
    create_user       BIGINT       NOT NULL,
    create_time       TIMESTAMP    NOT NULL,
    update_user       BIGINT       DEFAULT NULL,
    update_time       TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_plugin_installation_tenant_plugin
    ON ai_plugin_installation (tenant_id, plugin_id);
CREATE INDEX IF NOT EXISTS idx_ai_plugin_installation_tenant_id
    ON ai_plugin_installation (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_plugin_installation_enabled
    ON ai_plugin_installation (tenant_id, enabled);

CREATE TABLE IF NOT EXISTS ai_plugin_capability (
    id                BIGINT       NOT NULL,
    tenant_id         BIGINT       NOT NULL DEFAULT 1,
    plugin_id         BIGINT       NOT NULL,
    plugin_version_id BIGINT       NOT NULL,
    capability_kind   VARCHAR(64)  NOT NULL,
    capability_code   VARCHAR(128) NOT NULL,
    permission_code   VARCHAR(128) DEFAULT NULL,
    metadata          JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status            SMALLINT     NOT NULL DEFAULT 1,
    create_user       BIGINT       NOT NULL,
    create_time       TIMESTAMP    NOT NULL,
    update_user       BIGINT       DEFAULT NULL,
    update_time       TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_plugin_capability_version_kind_code
    ON ai_plugin_capability (tenant_id, plugin_version_id, capability_kind, capability_code);
CREATE INDEX IF NOT EXISTS idx_ai_plugin_capability_tenant_id
    ON ai_plugin_capability (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_plugin_capability_plugin_id
    ON ai_plugin_capability (plugin_id);
CREATE INDEX IF NOT EXISTS idx_ai_plugin_capability_kind
    ON ai_plugin_capability (capability_kind);

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
    transport_kind   VARCHAR(32)  NOT NULL DEFAULT 'streamable_http',
    status           SMALLINT     NOT NULL DEFAULT 1,
    auth_scope       VARCHAR(64)  NOT NULL DEFAULT 'tenant',
    auth_type        VARCHAR(64)  NOT NULL DEFAULT 'none',
    secret_ref       VARCHAR(255) DEFAULT NULL,
    network_allowlist JSONB       NOT NULL DEFAULT '[]'::jsonb,
    tool_allowlist   JSONB        NOT NULL DEFAULT '[]'::jsonb,
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

ALTER TABLE IF EXISTS ai_mcp_server
    ADD COLUMN IF NOT EXISTS transport_kind VARCHAR(32) NOT NULL DEFAULT 'streamable_http',
    ADD COLUMN IF NOT EXISTS auth_type VARCHAR(64) NOT NULL DEFAULT 'none',
    ADD COLUMN IF NOT EXISTS secret_ref VARCHAR(255) DEFAULT NULL,
    ADD COLUMN IF NOT EXISTS network_allowlist JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS tool_allowlist JSONB NOT NULL DEFAULT '[]'::jsonb;

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
    (3200001, 1, 'general_chat', 'General Chat', 'Direct model conversation skill without retrieval.', 1,
     '{"chatModel":"runtime.llm.chat"}'::jsonb,
     '[{"kind":"model_route","code":"runtime.llm.chat"}]'::jsonb,
     '{"milestone":"M2","poc":true,"template":"llm_chat","evalSets":["llm_chat_smoke"]}'::jsonb, 1, NOW()),
    (3200002, 1, 'cited_answer', 'Cited Answer', 'RAG question answering skill with grounded citations.', 1,
     '{"answerModel":"runtime.llm.rag_answer","embeddingModel":"runtime.embedding.default","rerankModel":"runtime.rerank.default"}'::jsonb,
     '[{"kind":"tool","code":"rag.search"}]'::jsonb,
     '{"milestone":"M2","poc":true,"template":"knowledge_base_chat","evalSets":["knowledge_base_regression"]}'::jsonb, 1, NOW()),
    (3200003, 1, 'task_planning', 'Task Planning', 'Routes user tasks into a bounded ReAct run graph.', 1,
     '{"agentModel":"runtime.llm.chat","intentModel":"runtime.llm.chat"}'::jsonb,
     '[{"kind":"tool","code":"rag.search"},{"kind":"tool","code":"github.repo.search"},{"kind":"tool","code":"feishu.message.send"}]'::jsonb,
     '{"milestone":"M2","poc":true,"template":"agent_workspace","evalSets":["agent_workspace_regression"]}'::jsonb, 1, NOW()),
    (3200004, 1, 'training_quiz', 'Training Quiz', 'Builds quizzes from cited training content.', 1,
     '{"answerModel":"runtime.llm.rag_answer","embeddingModel":"runtime.embedding.default","rerankModel":"runtime.rerank.default"}'::jsonb,
     '[{"kind":"tool","code":"rag.search"}]'::jsonb,
     '{"milestone":"M2","poc":true,"template":"training_app","evalSets":["training_regression"]}'::jsonb, 1, NOW()),
    (3200005, 1, 'training_reminder', 'Training Reminder', 'Schedules and sends training reminders through approved messaging tools.', 1,
     '{"agentModel":"runtime.llm.chat","intentModel":"runtime.llm.chat"}'::jsonb,
     '[{"kind":"tool","code":"feishu.message.send"},{"kind":"trigger","code":"training.reminder.schedule"}]'::jsonb,
     '{"milestone":"M2","poc":true,"template":"training_app","evalSets":["training_regression"]}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;

INSERT INTO ai_tool
    (id, tenant_id, code, name, description, tool_kind, risk_level, approval_policy, permission_code, executor_kind, input_schema, output_schema, status, metadata, create_user, create_time)
VALUES
    (3210001, 1, 'rag.search', 'RAG Search', 'Dry-run metadata for searching tenant-scoped knowledge chunks.', 'function', 1, 1, 'ai:knowledge:ask', 'dry_run',
     '{"type":"object","required":["datasetId","query"],"properties":{"datasetId":{"type":"integer"},"query":{"type":"string"}}}'::jsonb,
     '{"type":"object","properties":{"hits":{"type":"array"},"citations":{"type":"array"}}}'::jsonb,
     1, '{"poc":true,"module":"novex-rag"}'::jsonb, 1, NOW()),
    (3210002, 1, 'media.image.generate', 'Image Generation', 'Image generation routed through the configured draw model.', 'media', 2, 2, 'ai:tool:dryRun', 'model',
     '{"type":"object","required":["prompt"],"properties":{"prompt":{"type":"string"},"size":{"type":"string"}}}'::jsonb,
     '{"type":"object","properties":{"assetId":{"type":"string"},"status":{"type":"string"}}}'::jsonb,
     1, '{"poc":true,"route":"runtime.draw","records":["ai_media_job","ai_media_asset"]}'::jsonb, 1, NOW()),
    (3210003, 1, 'feishu.message.send', 'Feishu Message', 'Sends Feishu training notifications through the configured webhook; falls back to audited dry-run when FEISHU_WEBHOOK_URL is absent.', 'connector', 2, 2, 'ai:agent:run', 'connector',
     '{"type":"object","required":["recipient","text"],"properties":{"recipient":{"type":"string"},"text":{"type":"string"}}}'::jsonb,
     '{"type":"object","properties":{"toolCode":{"type":"string"},"status":{"type":"string"},"provider":{"type":"string"},"dryRun":{"type":"boolean"},"response":{"type":"object"}}}'::jsonb,
     1, '{"poc":true,"connector":"feishu","liveCapable":true,"dryRunFallback":"missing_webhook_env","executor":"agent_runtime"}'::jsonb, 1, NOW()),
    (3210004, 1, 'github.repo.search', 'GitHub Repo Search', 'Search code in a configured GitHub repository connector.', 'connector', 1, 1, 'ai:tool:dryRun', 'connector',
     '{"type":"object","required":["repository","query"],"properties":{"repository":{"type":"string"},"query":{"type":"string"},"path":{"type":"string"},"limit":{"type":"integer"}}}'::jsonb,
     '{"type":"object","properties":{"items":{"type":"array"},"dryRun":{"type":"boolean"}}}'::jsonb,
     1, '{"poc":true,"connector":"github.default","credentialScope":"tenant_or_user","operation":"repo_search"}'::jsonb, 1, NOW()),
    (3210005, 1, 'github.repo.read', 'GitHub File Read', 'Read a file from a configured GitHub repository connector.', 'connector', 1, 1, 'ai:tool:dryRun', 'connector',
     '{"type":"object","required":["repository","path"],"properties":{"repository":{"type":"string"},"path":{"type":"string"},"ref":{"type":"string"}}}'::jsonb,
     '{"type":"object","properties":{"content":{"type":"string"},"dryRun":{"type":"boolean"}}}'::jsonb,
     1, '{"poc":true,"connector":"github.default","credentialScope":"tenant_or_user","operation":"file_read"}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;

INSERT INTO ai_connector
    (id, tenant_id, code, name, description, connector_kind, credential_scope, auth_type, status, metadata, create_user, create_time)
VALUES
    (3220001, 1, 'github.default', 'GitHub', 'GitHub identity and repository connector POC metadata.', 'github', 'tenant', 'oauth_app', 1,
     '{"poc":true,"resources":["repo","issue","pull_request"]}'::jsonb, 1, NOW()),
    (3220002, 1, 'feishu.default', 'Feishu', 'Feishu tenant connector POC metadata for message delivery.', 'feishu', 'tenant', 'app_secret', 1,
     '{"poc":true,"resources":["message","user"]}'::jsonb, 1, NOW()),
    (3220003, 1, 'web.import', 'Web Import', 'Knowledge template connector for importing public web sources into a tenant dataset.', 'web', 'tenant', 'none', 1,
     '{"poc":true,"template":"knowledge_base_chat","resources":["url","html","markdown"]}'::jsonb, 1, NOW()),
    (3220004, 1, 'github.repo', 'GitHub Repository', 'Agent template connector alias for repository search and file read tools.', 'github', 'tenant', 'oauth_app', 1,
     '{"poc":true,"template":"agent_workspace","aliasOf":"github.default","resources":["repo","code_search","file_read"]}'::jsonb, 1, NOW()),
    (3220005, 1, 'feishu.message', 'Feishu Message', 'Template connector alias for approved Feishu message delivery.', 'feishu', 'tenant', 'app_secret', 1,
     '{"poc":true,"template":["agent_workspace","training_app"],"aliasOf":"feishu.default","resources":["message"]}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;

INSERT INTO ai_plugin
    (id, tenant_id, code, name, version, runtime, status, manifest, metadata, create_user, create_time)
VALUES
    (3230001, 1, 'builtin.github-basic', 'GitHub Basic', '0.1.0', 'builtin_adapter', 1,
     '{"capabilities":[{"kind":"connector","code":"github.default"},{"kind":"tool","code":"github.repo.search"},{"kind":"tool","code":"github.repo.read"}],"permissions":["ai:connector:list","ai:tool:dryRun"]}'::jsonb,
     '{"poc":true}'::jsonb, 1, NOW()),
    (3230002, 1, 'builtin.media-basic', 'Media Basic', '0.1.0', 'builtin_adapter', 1,
     '{"capabilities":[{"kind":"tool","code":"media.image.generate"}],"permissions":["ai:tool:dryRun"]}'::jsonb,
     '{"poc":true}'::jsonb, 1, NOW()),
    (3230003, 1, 'builtin.agent-tools', 'Built-in Agent Tools', '0.1.0', 'builtin_adapter', 1,
     '{"capabilities":[{"kind":"connector","code":"github.repo"},{"kind":"connector","code":"feishu.message"},{"kind":"tool","code":"github.repo.search"},{"kind":"tool","code":"github.repo.read"},{"kind":"tool","code":"media.image.generate"},{"kind":"tool","code":"feishu.message.send"},{"kind":"trigger","code":"agent.webhook"}],"permissions":["ai:connector:list","ai:agent:run","ai:agent:resume","ai:tool:dryRun","ai:trigger:list"]}'::jsonb,
     '{"poc":true,"template":"agent_workspace"}'::jsonb, 1, NOW()),
    (3230004, 1, 'builtin.training-pack', 'Built-in Training Pack', '0.1.0', 'builtin_adapter', 1,
     '{"capabilities":[{"kind":"skill","code":"training_quiz"},{"kind":"skill","code":"training_reminder"},{"kind":"connector","code":"feishu.message"},{"kind":"tool","code":"rag.search"},{"kind":"tool","code":"feishu.message.send"},{"kind":"trigger","code":"training.reminder.schedule"}],"permissions":["ai:knowledge:ask","ai:agent:run","ai:eval:run","ai:skill:list","ai:connector:list","ai:trigger:list"]}'::jsonb,
     '{"poc":true,"template":"training_app"}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;

INSERT INTO ai_plugin_version
    (id, tenant_id, plugin_id, version, runtime, manifest, status, create_user, create_time)
SELECT 3231001, p.tenant_id, p.id, p.version, p.runtime, p.manifest, 1, 1, NOW()
FROM ai_plugin AS p
WHERE p.tenant_id = 1 AND p.code = 'builtin.github-basic'
ON CONFLICT (tenant_id, plugin_id, version) DO NOTHING;

INSERT INTO ai_plugin_version
    (id, tenant_id, plugin_id, version, runtime, manifest, status, create_user, create_time)
SELECT 3231002, p.tenant_id, p.id, p.version, p.runtime, p.manifest, 1, 1, NOW()
FROM ai_plugin AS p
WHERE p.tenant_id = 1 AND p.code = 'builtin.media-basic'
ON CONFLICT (tenant_id, plugin_id, version) DO NOTHING;

INSERT INTO ai_plugin_version
    (id, tenant_id, plugin_id, version, runtime, manifest, status, create_user, create_time)
SELECT 3231003, p.tenant_id, p.id, p.version, p.runtime, p.manifest, 1, 1, NOW()
FROM ai_plugin AS p
WHERE p.tenant_id = 1 AND p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_id, version) DO NOTHING;

INSERT INTO ai_plugin_version
    (id, tenant_id, plugin_id, version, runtime, manifest, status, create_user, create_time)
SELECT 3231004, p.tenant_id, p.id, p.version, p.runtime, p.manifest, 1, 1, NOW()
FROM ai_plugin AS p
WHERE p.tenant_id = 1 AND p.code = 'builtin.training-pack'
ON CONFLICT (tenant_id, plugin_id, version) DO NOTHING;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232001, v.tenant_id, v.plugin_id, v.id, 'connector', 'github.default', 'ai:connector:list',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.github-basic'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO NOTHING;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232002, v.tenant_id, v.plugin_id, v.id, 'tool', 'github.repo.search', 'ai:tool:dryRun',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.github-basic'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO NOTHING;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232003, v.tenant_id, v.plugin_id, v.id, 'tool', 'github.repo.read', 'ai:tool:dryRun',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.github-basic'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO NOTHING;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232004, v.tenant_id, v.plugin_id, v.id, 'tool', 'media.image.generate', 'ai:tool:dryRun',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.media-basic'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO NOTHING;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232005, v.tenant_id, v.plugin_id, v.id, 'connector', 'github.repo', 'ai:connector:list',
       '{"source":"manifest","aliasOf":"github.default"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO NOTHING;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232006, v.tenant_id, v.plugin_id, v.id, 'connector', 'feishu.message', 'ai:connector:list',
       '{"source":"manifest","aliasOf":"feishu.default"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO NOTHING;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232007, v.tenant_id, v.plugin_id, v.id, 'tool', 'feishu.message.send', 'ai:agent:run',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO NOTHING;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232008, v.tenant_id, v.plugin_id, v.id, 'trigger', 'agent.webhook', 'ai:trigger:list',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO NOTHING;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232012, v.tenant_id, v.plugin_id, v.id, 'tool', 'media.image.generate', 'ai:tool:dryRun',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO NOTHING;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232009, v.tenant_id, v.plugin_id, v.id, 'skill', 'training_quiz', 'ai:skill:list',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.training-pack'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO NOTHING;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232010, v.tenant_id, v.plugin_id, v.id, 'skill', 'training_reminder', 'ai:skill:list',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.training-pack'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO NOTHING;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232011, v.tenant_id, v.plugin_id, v.id, 'trigger', 'training.reminder.schedule', 'ai:trigger:list',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.training-pack'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO NOTHING;

INSERT INTO ai_plugin_installation
    (id, tenant_id, plugin_id, plugin_version_id, enabled, install_source, permission_grants, config, installed_by, installed_at, status, create_user, create_time)
SELECT 3233001, v.tenant_id, v.plugin_id, v.id, TRUE, 'builtin',
       '["ai:connector:list","ai:tool:dryRun"]'::jsonb, '{}'::jsonb, 1, NOW(), 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.github-basic'
ON CONFLICT (tenant_id, plugin_id) DO NOTHING;

INSERT INTO ai_plugin_installation
    (id, tenant_id, plugin_id, plugin_version_id, enabled, install_source, permission_grants, config, installed_by, installed_at, status, create_user, create_time)
SELECT 3233002, v.tenant_id, v.plugin_id, v.id, TRUE, 'builtin',
       '["ai:tool:dryRun"]'::jsonb, '{}'::jsonb, 1, NOW(), 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.media-basic'
ON CONFLICT (tenant_id, plugin_id) DO NOTHING;

INSERT INTO ai_plugin_installation
    (id, tenant_id, plugin_id, plugin_version_id, enabled, install_source, permission_grants, config, installed_by, installed_at, status, create_user, create_time)
SELECT 3233003, v.tenant_id, v.plugin_id, v.id, TRUE, 'builtin',
       '["ai:connector:list","ai:agent:run","ai:agent:resume","ai:tool:dryRun","ai:trigger:list"]'::jsonb, '{}'::jsonb, 1, NOW(), 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_id) DO NOTHING;

INSERT INTO ai_plugin_installation
    (id, tenant_id, plugin_id, plugin_version_id, enabled, install_source, permission_grants, config, installed_by, installed_at, status, create_user, create_time)
SELECT 3233004, v.tenant_id, v.plugin_id, v.id, TRUE, 'builtin',
       '["ai:knowledge:ask","ai:agent:run","ai:eval:run","ai:skill:list","ai:connector:list","ai:trigger:list"]'::jsonb, '{}'::jsonb, 1, NOW(), 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.training-pack'
ON CONFLICT (tenant_id, plugin_id) DO NOTHING;

INSERT INTO ai_trigger
    (id, tenant_id, code, name, description, trigger_kind, target_kind, signature_required, idempotency_required, route_config, status, metadata, create_user, create_time)
VALUES
    (3240001, 1, 'webhook.training.event', 'Training Webhook', 'Signed webhook POC for training events.', 'webhook', 'run_graph', TRUE, TRUE,
     '{"path":"/ai/triggers/webhook/training","signatureHeader":"X-Novex-Signature","idempotencyHeader":"Idempotency-Key"}'::jsonb,
     1, '{"poc":true}'::jsonb, 1, NOW()),
    (3240002, 1, 'agent.webhook', 'Agent Webhook', 'Signed webhook POC for launching bounded Agent runs.', 'webhook', 'agent_run', TRUE, TRUE,
     '{"path":"/ai/triggers/webhook/agent","signatureHeader":"X-Novex-Signature","idempotencyHeader":"Idempotency-Key"}'::jsonb,
     1, '{"poc":true,"template":"agent_workspace"}'::jsonb, 1, NOW()),
    (3240003, 1, 'training.reminder.schedule', 'Training Reminder Schedule', 'Schedule trigger metadata for training reminder jobs.', 'schedule', 'job', FALSE, TRUE,
     '{"jobKey":"training.reminder","cron":"0 0 9 * * ?"}'::jsonb,
     1, '{"poc":true,"template":"training_app"}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;

INSERT INTO ai_mcp_server
    (id, tenant_id, code, name, endpoint_url, transport_kind, status, auth_scope, auth_type, secret_ref, network_allowlist, tool_allowlist, discovered_tools, metadata, create_user, create_time)
VALUES
    (3250001, 1, 'local.dry-run', 'Local Dry Run MCP', NULL, 'builtin', 1, 'tenant', 'none', NULL,
     '[]'::jsonb,
     '["rag.search","media.image.generate","github.repo.search","github.repo.read"]'::jsonb,
     '[{"serverId":"local.dry-run","toolName":"rag.search","permissionCode":"ai:knowledge:ask"},{"serverId":"local.dry-run","toolName":"media.image.generate","permissionCode":"ai:tool:dryRun"},{"serverId":"local.dry-run","toolName":"github.repo.search","permissionCode":"ai:tool:dryRun"},{"serverId":"local.dry-run","toolName":"github.repo.read","permissionCode":"ai:tool:dryRun"}]'::jsonb,
     '{"poc":true,"transport":"in-process"}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;
