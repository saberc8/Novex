-- Seed runtime model chat permission without mutating the earlier runtime seed.

INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3023, '模型对话', 3020, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:model:chat', 3, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
VALUES
    (1, 3023)
ON CONFLICT DO NOTHING;
