-- Seed additional Knowledge control-plane permissions for M1.

INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3032, '详情', 3030, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:knowledge:get', 2, 1, 1, NOW()),
    (3033, '新增', 3030, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:knowledge:create', 3, 1, 1, NOW()),
    (3034, '修改', 3030, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:knowledge:update', 4, 1, 1, NOW()),
    (3035, '删除', 3030, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:knowledge:delete', 5, 1, 1, NOW()),
    (3036, '文档列表', 3030, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:knowledge:document:list', 6, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
VALUES
    (1, 3032),
    (1, 3033),
    (1, 3034),
    (1, 3035),
    (1, 3036)
ON CONFLICT DO NOTHING;
