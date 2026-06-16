-- Customer service agent delivery template, route permissions, and rollout contract.
-- template_code: customer-service-agent-poc
-- eval_set: customer-service-agent-regression
-- knowledge_dependency: tenant customer-service FAQ or policy dataset

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
    3330, 'Customer Service Agent', p.id, 2, '/ai/customer-service', 'AiCustomerServiceAgent', 'ai/customer-service/index', NULL, 'headphones', FALSE, FALSE, TRUE, NULL, 8, 1, 1, NOW()
FROM ai_parent AS p
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE path = '/ai/customer-service')
ON CONFLICT DO NOTHING;

WITH customer_service_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/customer-service'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3331, 'Run Agent', c.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:customer-service:agent:run', 1, 1, 1, NOW()
FROM customer_service_menu AS c
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:customer-service:agent:run')
ON CONFLICT DO NOTHING;

WITH customer_service_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/customer-service'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3332, 'List Agent Runs', c.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:customer-service:agent:list', 2, 1, 1, NOW()
FROM customer_service_menu AS c
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:customer-service:agent:list')
ON CONFLICT DO NOTHING;

WITH customer_service_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/customer-service'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3333, 'Read Service Knowledge', c.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:customer-service:read', 3, 1, 1, NOW()
FROM customer_service_menu AS c
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:customer-service:read')
ON CONFLICT DO NOTHING;

WITH customer_service_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/customer-service'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3334, 'Create Ticket', c.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:customer-service:ticket', 4, 1, 1, NOW()
FROM customer_service_menu AS c
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:customer-service:ticket')
ON CONFLICT DO NOTHING;

WITH customer_service_menu AS (
    SELECT id
    FROM sys_menu
    WHERE path = '/ai/customer-service'
    ORDER BY id
    LIMIT 1
)
INSERT INTO sys_menu
    (id, title, parent_id, type, path, name, component, redirect, icon, is_external, is_cache, is_hidden, permission, sort, status, create_user, create_time)
SELECT
    3335, 'Request Handoff', c.id, 3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ai:customer-service:handoff', 5, 1, 1, NOW()
FROM customer_service_menu AS c
WHERE NOT EXISTS (SELECT 1 FROM sys_menu WHERE permission = 'ai:customer-service:handoff')
ON CONFLICT DO NOTHING;

INSERT INTO sys_role_menu (role_id, menu_id)
SELECT 1, id
FROM sys_menu
WHERE path = '/ai/customer-service'
   OR permission IN (
      'ai:customer-service:agent:run',
      'ai:customer-service:agent:list',
      'ai:customer-service:read',
      'ai:customer-service:ticket',
      'ai:customer-service:handoff'
   )
ON CONFLICT DO NOTHING;
