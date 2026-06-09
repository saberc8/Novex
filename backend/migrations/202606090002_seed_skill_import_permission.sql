-- Add admin skill registry menu and import permission for databases initialized
-- before the SKILL.md import feature.

DELETE FROM sys_role_menu
WHERE menu_id IN (
    SELECT child.id
    FROM sys_menu AS child
    LEFT JOIN sys_menu AS parent ON parent.id = child.parent_id
    WHERE child.permission = 'ai:skill:import'
      AND COALESCE(parent.path, '') <> '/ai/skills'
);

DELETE FROM sys_menu AS child
WHERE child.permission = 'ai:skill:import'
  AND NOT EXISTS (
      SELECT 1
      FROM sys_menu AS parent
      WHERE parent.id = child.parent_id
        AND parent.path = '/ai/skills'
  );

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
    3300, 'Skills', p.id, 2, '/ai/skills', 'AiSkills', 'ai/skills/index', NULL, 'bookmark', FALSE, FALSE, FALSE, NULL, 5, 1, 1, NOW()
FROM ai_parent AS p
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE path = '/ai/skills')
ON CONFLICT DO NOTHING;

WITH skill_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/skills'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3301, '列表', s.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:skill:list', 1, 1, 1, NOW()
FROM skill_menu AS s
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:skill:list')
ON CONFLICT DO NOTHING;

WITH skill_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/skills'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3302, '导入', s.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:skill:import', 2, 1, 1, NOW()
FROM skill_menu AS s
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:skill:import')
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
SELECT 1, id
FROM sys_menu
WHERE path = '/ai/skills'
   OR permission IN ('ai:skill:list', 'ai:skill:import')
ON CONFLICT DO NOTHING;
