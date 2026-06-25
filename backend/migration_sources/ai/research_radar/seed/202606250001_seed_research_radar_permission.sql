-- Seed Research Radar source scan permission.

INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
VALUES
    (3150, 'Research Radar', 3000, 2, '/ai/research-radar', 'AiResearchRadar', 'ai/research-radar/index', NULL, 'radar', FALSE, FALSE, FALSE, NULL, 13, 1, 1, NOW()),
    (3151, '扫描', 3150, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:research-radar:scan', 1, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
SELECT 1, id
FROM sys_menu
WHERE id BETWEEN 3150 AND 3151
ON CONFLICT DO NOTHING;
