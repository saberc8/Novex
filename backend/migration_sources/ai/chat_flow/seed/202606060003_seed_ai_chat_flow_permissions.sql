-- Seed app-facing chat flow permissions.

INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3120, '应用对话', 3000, 2, '/ai/chat-flow', 'AiChatFlow', 'ai/chat-flow/index', NULL, 'message', FALSE, FALSE, TRUE, NULL, 12, 1, 1, NOW()),
    (3121, '列表', 3120, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:chatFlow:list', 1, 1, 1, NOW()),
    (3122, '创建', 3120, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:chatFlow:create', 2, 1, 1, NOW()),
    (3123, '发送消息', 3120, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:chatFlow:message', 3, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
VALUES
    (1, 3120),
    (1, 3121),
    (1, 3122),
    (1, 3123)
ON CONFLICT DO NOTHING;
