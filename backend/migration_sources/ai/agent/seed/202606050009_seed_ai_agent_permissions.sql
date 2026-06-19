-- Seed Novex M3 Agent Runtime permissions.

INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3042, '运行', 3040, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:agent:run', 2, 1, 1, NOW()),
    (3043, '事件', 3040, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:agent:event:list', 3, 1, 1, NOW()),
    (3044, '恢复', 3040, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:agent:resume', 4, 1, 1, NOW()),
    (3045, '取消', 3040, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:agent:cancel', 5, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
SELECT 1, id
FROM sys_menu
WHERE id BETWEEN 3042 AND 3045
ON CONFLICT DO NOTHING;
