INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3024, '断路器列表', 3020, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:model:circuitBreaker:list', 4, 1, 1, NOW()),
    (3025, '清除断路器', 3020, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:model:circuitBreaker:clear', 5, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
VALUES
    (1, 3024),
    (1, 3025)
ON CONFLICT DO NOTHING;
