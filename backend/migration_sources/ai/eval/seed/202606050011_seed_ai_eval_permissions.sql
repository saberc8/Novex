-- Seed Novex M4 Eval Runtime permissions.

INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3091, '列表', 3090, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:eval:list', 1, 1, 1, NOW()),
    (3092, '运行', 3090, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:eval:run', 2, 1, 1, NOW()),
    (3093, '用例', 3090, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:eval:case:list', 3, 1, 1, NOW()),
    (3094, '报告', 3090, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:eval:report', 4, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
SELECT 1, id
FROM sys_menu
WHERE id BETWEEN 3091 AND 3094
ON CONFLICT DO NOTHING;
