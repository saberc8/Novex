-- Backfill the model-loop web search builtin tool for existing POC databases.

INSERT INTO ai_tool
    (id, tenant_id, code, name, description, tool_kind, risk_level, approval_policy, permission_code, executor_kind, input_schema, output_schema, status, metadata, create_user, create_time)
VALUES
    (3210006, 1, 'web.search', 'Web Search', 'Dry-run metadata for searching fresh external web results from model-loop agents.', 'function', 1, 1, 'ai:agent:run', 'dry_run',
     '{"type":"object","required":["query"],"properties":{"query":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":10}}}'::jsonb,
     '{"type":"object","properties":{"dryRun":{"type":"boolean"},"status":{"type":"string"},"query":{"type":"string"},"results":{"type":"array"},"message":{"type":"string"}}}'::jsonb,
     1, '{"poc":true,"module":"agent-runtime","executor":"builtin.web.search","dryRunFallback":"missing_web_search_provider"}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    tool_kind = EXCLUDED.tool_kind,
    risk_level = EXCLUDED.risk_level,
    approval_policy = EXCLUDED.approval_policy,
    permission_code = EXCLUDED.permission_code,
    executor_kind = EXCLUDED.executor_kind,
    input_schema = EXCLUDED.input_schema,
    output_schema = EXCLUDED.output_schema,
    status = EXCLUDED.status,
    metadata = EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = NOW();
