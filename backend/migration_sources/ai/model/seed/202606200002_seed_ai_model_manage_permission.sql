INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3039, '管理模型路由', 3020, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:model:manage', 10, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
VALUES
    (1, 3039)
ON CONFLICT DO NOTHING;
