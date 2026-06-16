-- Notebook workspace route permissions.

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
    3320, 'Notebook', p.id, 2, '/ai/notebooks', 'AiNotebook', 'ai/notebook/index', NULL, 'notebook-tabs', FALSE, FALSE, TRUE, NULL, 7, 1, 1, NOW()
FROM ai_parent AS p
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE path = '/ai/notebooks')
ON CONFLICT DO NOTHING;

WITH notebook_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/notebooks'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3321, 'Workspace 列表', n.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:notebook:list', 1, 1, 1, NOW()
FROM notebook_menu AS n
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:notebook:list')
ON CONFLICT DO NOTHING;

WITH notebook_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/notebooks'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3322, '创建 Workspace', n.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:notebook:create', 2, 1, 1, NOW()
FROM notebook_menu AS n
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:notebook:create')
ON CONFLICT DO NOTHING;

WITH notebook_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/notebooks'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3323, 'Source 管理', n.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:notebook:source', 3, 1, 1, NOW()
FROM notebook_menu AS n
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:notebook:source')
ON CONFLICT DO NOTHING;

WITH notebook_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/notebooks'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3324, 'Artifact 列表', n.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:notebook:artifact', 4, 1, 1, NOW()
FROM notebook_menu AS n
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:notebook:artifact')
ON CONFLICT DO NOTHING;

WITH notebook_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/notebooks'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3325, 'Ask 问答', n.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:notebook:ask', 5, 1, 1, NOW()
FROM notebook_menu AS n
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:notebook:ask')
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
SELECT 1, id
FROM sys_menu
WHERE path = '/ai/notebooks'
   OR permission IN (
      'ai:notebook:list',
      'ai:notebook:create',
      'ai:notebook:source',
      'ai:notebook:artifact',
      'ai:notebook:ask'
   )
ON CONFLICT DO NOTHING;
