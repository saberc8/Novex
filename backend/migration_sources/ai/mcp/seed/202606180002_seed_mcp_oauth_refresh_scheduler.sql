-- Default scheduler job for MCP OAuth session refresh.

INSERT INTO sys_job (
    id, name, group_name, task_type, cron_expression, status, concurrent,
    misfire_policy, max_retry, timeout_seconds, http_method, http_url,
    http_headers, http_body, builtin_key, description, next_trigger_time,
    create_user, create_time
) VALUES (
    3600003, 'AI MCP OAuth Refresh', 'ai-ops', 2, '*/60 * * * * *', 1, FALSE,
    1, 1, 300, NULL, NULL,
    '{}'::jsonb, NULL, 'ai.mcp.oauth_refresh',
    'Refresh due MCP OAuth sessions before token expiry.',
    NOW(), 1, NOW()
)
ON CONFLICT DO NOTHING;
