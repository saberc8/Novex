-- Backfill connector/plugin/trigger aliases declared by M5 templates into the
-- M2 capability registry for databases initialized before template alignment.

INSERT INTO ai_connector
    (id, tenant_id, code, name, description, connector_kind, credential_scope, auth_type, status, metadata, create_user, create_time)
VALUES
    (3220103, 1, 'web.import', 'Web Import', 'Knowledge template connector for importing public web sources into a tenant dataset.', 'web', 'tenant', 'none', 1,
     '{"poc":true,"template":"knowledge_base_chat","resources":["url","html","markdown"]}'::jsonb, 1, NOW()),
    (3220104, 1, 'github.repo', 'GitHub Repository', 'Agent template connector alias for repository search and file read tools.', 'github', 'tenant', 'oauth_app', 1,
     '{"poc":true,"template":"agent_workspace","aliasOf":"github.default","resources":["repo","code_search","file_read"]}'::jsonb, 1, NOW()),
    (3220105, 1, 'feishu.message', 'Feishu Message', 'Template connector alias for approved Feishu message delivery.', 'feishu', 'tenant', 'app_secret', 1,
     '{"poc":true,"template":["agent_workspace","training_app"],"aliasOf":"feishu.default","resources":["message"]}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    connector_kind = EXCLUDED.connector_kind,
    credential_scope = EXCLUDED.credential_scope,
    auth_type = EXCLUDED.auth_type,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status,
    update_user = EXCLUDED.create_user,
    update_time = NOW();

INSERT INTO ai_plugin
    (id, tenant_id, code, name, version, runtime, status, manifest, metadata, create_user, create_time)
VALUES
    (3230103, 1, 'builtin.agent-tools', 'Built-in Agent Tools', '0.1.0', 'builtin_adapter', 1,
     '{"capabilities":[{"kind":"connector","code":"github.repo"},{"kind":"connector","code":"feishu.message"},{"kind":"tool","code":"github.repo.search"},{"kind":"tool","code":"github.repo.read"},{"kind":"tool","code":"media.image.generate"},{"kind":"tool","code":"feishu.message.send"},{"kind":"trigger","code":"agent.webhook"}],"permissions":["ai:connector:list","ai:agent:run","ai:agent:resume","ai:tool:dryRun","ai:trigger:list"]}'::jsonb,
     '{"poc":true,"template":"agent_workspace"}'::jsonb, 1, NOW()),
    (3230104, 1, 'builtin.training-pack', 'Built-in Training Pack', '0.1.0', 'builtin_adapter', 1,
     '{"capabilities":[{"kind":"skill","code":"training_quiz"},{"kind":"skill","code":"training_reminder"},{"kind":"connector","code":"feishu.message"},{"kind":"tool","code":"rag.search"},{"kind":"tool","code":"feishu.message.send"},{"kind":"trigger","code":"training.reminder.schedule"}],"permissions":["ai:knowledge:ask","ai:agent:run","ai:eval:run","ai:skill:list","ai:connector:list","ai:trigger:list"]}'::jsonb,
     '{"poc":true,"template":"training_app"}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO UPDATE SET
    name = EXCLUDED.name,
    version = EXCLUDED.version,
    runtime = EXCLUDED.runtime,
    manifest = EXCLUDED.manifest,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status,
    update_user = EXCLUDED.create_user,
    update_time = NOW();

INSERT INTO ai_plugin_version
    (id, tenant_id, plugin_id, version, runtime, manifest, status, create_user, create_time)
SELECT 3231103, p.tenant_id, p.id, p.version, p.runtime, p.manifest, 1, 1, NOW()
FROM ai_plugin AS p
WHERE p.tenant_id = 1 AND p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_id, version) DO UPDATE SET
    runtime = EXCLUDED.runtime,
    manifest = EXCLUDED.manifest,
    status = EXCLUDED.status;

INSERT INTO ai_plugin_version
    (id, tenant_id, plugin_id, version, runtime, manifest, status, create_user, create_time)
SELECT 3231104, p.tenant_id, p.id, p.version, p.runtime, p.manifest, 1, 1, NOW()
FROM ai_plugin AS p
WHERE p.tenant_id = 1 AND p.code = 'builtin.training-pack'
ON CONFLICT (tenant_id, plugin_id, version) DO UPDATE SET
    runtime = EXCLUDED.runtime,
    manifest = EXCLUDED.manifest,
    status = EXCLUDED.status;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232105, v.tenant_id, v.plugin_id, v.id, 'connector', 'github.repo', 'ai:connector:list',
       '{"source":"manifest","aliasOf":"github.default"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO UPDATE SET
    permission_code = EXCLUDED.permission_code,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232106, v.tenant_id, v.plugin_id, v.id, 'connector', 'feishu.message', 'ai:connector:list',
       '{"source":"manifest","aliasOf":"feishu.default"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO UPDATE SET
    permission_code = EXCLUDED.permission_code,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232107, v.tenant_id, v.plugin_id, v.id, 'tool', 'feishu.message.send', 'ai:agent:run',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO UPDATE SET
    permission_code = EXCLUDED.permission_code,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232108, v.tenant_id, v.plugin_id, v.id, 'trigger', 'agent.webhook', 'ai:trigger:list',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO UPDATE SET
    permission_code = EXCLUDED.permission_code,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232112, v.tenant_id, v.plugin_id, v.id, 'tool', 'media.image.generate', 'ai:tool:dryRun',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO UPDATE SET
    permission_code = EXCLUDED.permission_code,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232109, v.tenant_id, v.plugin_id, v.id, 'skill', 'training_quiz', 'ai:skill:list',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.training-pack'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO UPDATE SET
    permission_code = EXCLUDED.permission_code,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232110, v.tenant_id, v.plugin_id, v.id, 'skill', 'training_reminder', 'ai:skill:list',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.training-pack'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO UPDATE SET
    permission_code = EXCLUDED.permission_code,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status;

INSERT INTO ai_plugin_capability
    (id, tenant_id, plugin_id, plugin_version_id, capability_kind, capability_code, permission_code, metadata, status, create_user, create_time)
SELECT 3232111, v.tenant_id, v.plugin_id, v.id, 'trigger', 'training.reminder.schedule', 'ai:trigger:list',
       '{"source":"manifest"}'::jsonb, 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.training-pack'
ON CONFLICT (tenant_id, plugin_version_id, capability_kind, capability_code) DO UPDATE SET
    permission_code = EXCLUDED.permission_code,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status;

INSERT INTO ai_plugin_installation
    (id, tenant_id, plugin_id, plugin_version_id, enabled, install_source, permission_grants, config, installed_by, installed_at, status, create_user, create_time)
SELECT 3233103, v.tenant_id, v.plugin_id, v.id, TRUE, 'builtin',
       '["ai:connector:list","ai:agent:run","ai:agent:resume","ai:tool:dryRun","ai:trigger:list"]'::jsonb, '{}'::jsonb, 1, NOW(), 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.agent-tools'
ON CONFLICT (tenant_id, plugin_id) DO UPDATE SET
    plugin_version_id = EXCLUDED.plugin_version_id,
    enabled = EXCLUDED.enabled,
    permission_grants = EXCLUDED.permission_grants,
    config = EXCLUDED.config,
    status = EXCLUDED.status,
    update_user = EXCLUDED.create_user,
    update_time = NOW();

INSERT INTO ai_plugin_installation
    (id, tenant_id, plugin_id, plugin_version_id, enabled, install_source, permission_grants, config, installed_by, installed_at, status, create_user, create_time)
SELECT 3233104, v.tenant_id, v.plugin_id, v.id, TRUE, 'builtin',
       '["ai:knowledge:ask","ai:agent:run","ai:eval:run","ai:skill:list","ai:connector:list","ai:trigger:list"]'::jsonb, '{}'::jsonb, 1, NOW(), 1, 1, NOW()
FROM ai_plugin_version AS v
JOIN ai_plugin AS p ON p.id = v.plugin_id AND p.tenant_id = v.tenant_id
WHERE p.code = 'builtin.training-pack'
ON CONFLICT (tenant_id, plugin_id) DO UPDATE SET
    plugin_version_id = EXCLUDED.plugin_version_id,
    enabled = EXCLUDED.enabled,
    permission_grants = EXCLUDED.permission_grants,
    config = EXCLUDED.config,
    status = EXCLUDED.status,
    update_user = EXCLUDED.create_user,
    update_time = NOW();

INSERT INTO ai_trigger
    (id, tenant_id, code, name, description, trigger_kind, target_kind, signature_required, idempotency_required, route_config, status, metadata, create_user, create_time)
VALUES
    (3240102, 1, 'agent.webhook', 'Agent Webhook', 'Signed webhook POC for launching bounded Agent runs.', 'webhook', 'agent_run', TRUE, TRUE,
     '{"path":"/ai/triggers/webhook/agent","signatureHeader":"X-Novex-Signature","idempotencyHeader":"Idempotency-Key"}'::jsonb,
     1, '{"poc":true,"template":"agent_workspace"}'::jsonb, 1, NOW()),
    (3240103, 1, 'training.reminder.schedule', 'Training Reminder Schedule', 'Schedule trigger metadata for training reminder jobs.', 'schedule', 'job', FALSE, TRUE,
     '{"jobKey":"training.reminder","cron":"0 0 9 * * ?"}'::jsonb,
     1, '{"poc":true,"template":"training_app"}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    trigger_kind = EXCLUDED.trigger_kind,
    target_kind = EXCLUDED.target_kind,
    signature_required = EXCLUDED.signature_required,
    idempotency_required = EXCLUDED.idempotency_required,
    route_config = EXCLUDED.route_config,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status,
    update_user = EXCLUDED.create_user,
    update_time = NOW();
