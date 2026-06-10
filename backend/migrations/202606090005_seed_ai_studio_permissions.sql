-- Studio artifact permissions for notebook Studio actions.

WITH ai_parent AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3310, 'Studio', p.id, 2, '/ai/studio', 'AiStudio', 'ai/studio/index', NULL, 'layout-grid', FALSE, FALSE, TRUE, NULL, 6, 1, 1, NOW()
FROM ai_parent AS p
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE path = '/ai/studio')
ON CONFLICT DO NOTHING;

WITH studio_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/studio'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3311, 'Action 列表', s.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:studio:action:list', 1, 1, 1, NOW()
FROM studio_menu AS s
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:studio:action:list')
ON CONFLICT DO NOTHING;

WITH studio_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/studio'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3312, 'Artifact 列表', s.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:studio:artifact:list', 2, 1, 1, NOW()
FROM studio_menu AS s
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:studio:artifact:list')
ON CONFLICT DO NOTHING;

WITH studio_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/studio'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3313, '生成 Artifact', s.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:studio:artifact:create', 3, 1, 1, NOW()
FROM studio_menu AS s
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:studio:artifact:create')
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
SELECT 1, id
FROM sys_menu
WHERE path = '/ai/studio'
   OR permission IN (
      'ai:studio:action:list',
      'ai:studio:artifact:list',
      'ai:studio:artifact:create'
   )
ON CONFLICT DO NOTHING;
