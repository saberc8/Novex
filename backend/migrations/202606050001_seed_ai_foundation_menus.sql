-- Seed Novex M0 AI foundation control-plane menus and permissions.

INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (1090, '身份安全', 1000, 2, '/system/identity', 'SystemIdentity', 'system/identity/index', '/system/identity/providers', 'safe', FALSE, FALSE, FALSE, NULL, 5, 1, 1, NOW()),
    (1091, '身份源', 1090, 2, '/system/identity/providers', 'SystemIdentityProviders', 'system/identity/providers/index', NULL, 'safe', FALSE, FALSE, FALSE, NULL, 1, 1, 1, NOW()),
    (1092, '外部账号', 1090, 2, '/system/identity/accounts', 'SystemIdentityAccounts', 'system/identity/accounts/index', NULL, 'user', FALSE, FALSE, FALSE, NULL, 2, 1, 1, NOW()),
    (1093, '准入策略', 1090, 2, '/system/identity/policies', 'SystemIdentityPolicies', 'system/identity/policies/index', NULL, 'lock', FALSE, FALSE, FALSE, NULL, 3, 1, 1, NOW()),
    (1094, '查询', 1091, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'system:identityProvider:list', 1, 1, 1, NOW()),
    (1095, '查询', 1092, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'system:externalAccount:list', 1, 1, 1, NOW()),
    (1096, '查询', 1093, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'system:identityPolicy:list', 1, 1, 1, NOW()),
    (1097, '密钥查询', 1090, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'system:secret:list', 4, 1, 1, NOW()),
    (1098, '密钥写入', 1090, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'system:secret:upsert', 5, 1, 1, NOW()),

    (3000, 'AI 基座', 0, 1, '/ai', 'Ai', 'Layout', '/ai/dashboard', 'apps', FALSE, FALSE, FALSE, NULL, 3, 1, 1, NOW()),
    (3010, '总览', 3000, 2, '/ai/dashboard', 'AiDashboard', 'ai/dashboard/index', NULL, 'computer', FALSE, FALSE, FALSE, NULL, 1, 1, 1, NOW()),
    (3011, '查看', 3010, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:foundation:read', 1, 1, 1, NOW()),

    (3020, '模型管理', 3000, 2, '/ai/models', 'AiModels', 'ai/models/index', NULL, 'config', FALSE, FALSE, FALSE, NULL, 2, 1, 1, NOW()),
    (3021, '列表', 3020, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:model:list', 1, 1, 1, NOW()),

    (3030, '知识库', 3000, 2, '/ai/knowledge', 'AiKnowledge', 'ai/knowledge/index', NULL, 'file', FALSE, FALSE, FALSE, NULL, 3, 1, 1, NOW()),
    (3031, '列表', 3030, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:knowledge:list', 1, 1, 1, NOW()),

    (3040, 'Agent', 3000, 2, '/ai/agents', 'AiAgents', 'ai/agents/index', NULL, 'user', FALSE, FALSE, FALSE, NULL, 4, 1, 1, NOW()),
    (3041, '列表', 3040, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:agent:list', 1, 1, 1, NOW()),

    (3045, 'Skills', 3000, 2, '/ai/skills', 'AiSkills', 'ai/skills/index', NULL, 'bookmark', FALSE, FALSE, FALSE, NULL, 5, 1, 1, NOW()),
    (3046, '列表', 3045, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:skill:list', 1, 1, 1, NOW()),

    (3050, '工具', 3000, 2, '/ai/tools', 'AiTools', 'ai/tools/index', NULL, 'menu', FALSE, FALSE, FALSE, NULL, 6, 1, 1, NOW()),
    (3051, '列表', 3050, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:tool:list', 1, 1, 1, NOW()),

    (3060, '连接器', 3000, 2, '/ai/connectors', 'AiConnectors', 'ai/connectors/index', NULL, 'mind-mapping', FALSE, FALSE, FALSE, NULL, 6, 1, 1, NOW()),
    (3061, '列表', 3060, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:connector:list', 1, 1, 1, NOW()),
    (3062, '凭据配置', 3060, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:connector:credential:update', 2, 1, 1, NOW()),

    (3070, '插件', 3000, 2, '/ai/plugins', 'AiPlugins', 'ai/plugins/index', NULL, 'apps', FALSE, FALSE, FALSE, NULL, 7, 1, 1, NOW()),
    (3071, '列表', 3070, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:plugin:list', 1, 1, 1, NOW()),
    (3072, '安装', 3070, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:plugin:install', 2, 1, 1, NOW()),

    (3080, '触发器', 3000, 2, '/ai/triggers', 'AiTriggers', 'ai/triggers/index', NULL, 'clock', FALSE, FALSE, FALSE, NULL, 8, 1, 1, NOW()),
    (3081, '列表', 3080, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:trigger:list', 1, 1, 1, NOW()),

    (3120, 'MCP', 3000, 2, '/ai/mcp', 'AiMcp', 'ai/mcp/index', NULL, 'blocks', FALSE, FALSE, FALSE, NULL, 9, 1, 1, NOW()),
    (3121, '列表', 3120, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:mcp:list', 1, 1, 1, NOW()),
    (3122, '注册', 3120, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:mcp:update', 2, 1, 1, NOW()),

    (3130, '记忆', 3000, 2, '/ai/memory', 'AiMemory', 'ai/memory/index', NULL, 'brain-circuit', FALSE, FALSE, FALSE, NULL, 10, 1, 1, NOW()),
    (3131, '列表', 3130, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:memory:list', 1, 1, 1, NOW()),
    (3132, '写入', 3130, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:memory:upsert', 2, 1, 1, NOW()),
    (3133, '删除', 3130, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:memory:delete', 3, 1, 1, NOW()),

    (3140, '集成入口', 3000, 2, '/ai/integrations', 'AiIntegrations', 'ai/integrations/index', NULL, 'key', FALSE, FALSE, FALSE, NULL, 12, 1, 1, NOW()),
    (3141, '列表', 3140, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:integration:list', 1, 1, 1, NOW()),
    (3142, '创建', 3140, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:integration:create', 2, 1, 1, NOW()),
    (3143, '撤销', 3140, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:integration:revoke', 3, 1, 1, NOW()),

    (3090, '评测', 3000, 2, '/ai/evals', 'AiEvals', 'ai/evals/index', NULL, 'bookmark', FALSE, FALSE, FALSE, NULL, 9, 1, 1, NOW()),
    (3091, '列表', 3090, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:eval:list', 1, 1, 1, NOW()),

    (3100, '运行追踪', 3000, 2, '/ai/traces', 'AiTraces', 'ai/traces/index', NULL, 'history', FALSE, FALSE, FALSE, NULL, 10, 1, 1, NOW()),
    (3101, '列表', 3100, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:trace:list', 1, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
SELECT 1, id
FROM sys_menu
WHERE id BETWEEN 1090 AND 1096
   OR id BETWEEN 1097 AND 1098
   OR id BETWEEN 3000 AND 3143
ON CONFLICT DO NOTHING;
