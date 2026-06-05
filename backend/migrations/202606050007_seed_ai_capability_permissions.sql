-- Seed M2 capability governance permissions.

INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3052, 'Dry Run', 3050, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:tool:dryRun', 2, 1, 1, NOW()),
    (3053, '调用审计', 3050, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:tool:audit:list', 3, 1, 1, NOW()),
    (3054, 'MCP 列表', 3050, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:mcp:list', 4, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
VALUES
    (1, 3052),
    (1, 3053),
    (1, 3054)
ON CONFLICT DO NOTHING;
