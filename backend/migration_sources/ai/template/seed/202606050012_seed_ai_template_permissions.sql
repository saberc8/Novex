-- Seed M5 customer delivery template permissions.

INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3111, '列表', 3110, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:template:list', 1, 1, 1, NOW()),
    (3112, '初始化', 3110, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:template:init', 2, 1, 1, NOW()),
    (3113, 'Smoke', 3110, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:template:smoke', 3, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
VALUES
    (1, 3111),
    (1, 3112),
    (1, 3113)
ON CONFLICT DO NOTHING;
