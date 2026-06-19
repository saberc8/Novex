-- Seed Knowledge RAG MVP permissions for M1 runtime APIs.

INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3037, '上传文档', 3030, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:knowledge:document:create', 7, 1, 1, NOW()),
    (3038, '检索问答', 3030, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:knowledge:ask', 8, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
VALUES
    (1, 3037),
    (1, 3038)
ON CONFLICT DO NOTHING;
