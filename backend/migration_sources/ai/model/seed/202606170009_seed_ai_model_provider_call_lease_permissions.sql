INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3027, '模型调用租约列表', 3020, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:model:providerCallLease:list', 7, 1, 1, NOW()),
    (3028, '过期模型调用租约', 3020, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:model:providerCallLease:expire', 8, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
VALUES
    (1, 3027),
    (1, 3028)
ON CONFLICT DO NOTHING;
