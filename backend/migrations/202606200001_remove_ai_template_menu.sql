-- Remove the retired delivery template menu and permissions from existing local databases.

DELETE FROM sys_role_menu
WHERE menu_id IN (3110, 3111, 3112, 3113);

DELETE FROM sys_menu
WHERE id IN (3111, 3112, 3113, 3110)
   OR path = '/ai/templates'
   OR permission IN ('ai:template:list', 'ai:template:init', 'ai:template:smoke');
