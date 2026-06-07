-- Promote Feishu message tool metadata from dry-run-only governance to the
-- Agent runtime connector executor. The runtime still records dry-run audits
-- when FEISHU_WEBHOOK_URL is not configured.

UPDATE ai_tool
SET description = 'Sends Feishu training notifications through the configured webhook; falls back to audited dry-run when FEISHU_WEBHOOK_URL is absent.',
    permission_code = 'ai:agent:run',
    executor_kind = 'connector',
    output_schema = '{"type":"object","properties":{"toolCode":{"type":"string"},"status":{"type":"string"},"provider":{"type":"string"},"dryRun":{"type":"boolean"},"response":{"type":"object"}}}'::jsonb,
    metadata = metadata || '{"liveCapable":true,"dryRunFallback":"missing_webhook_env","executor":"agent_runtime"}'::jsonb,
    update_user = 1,
    update_time = NOW()
WHERE tenant_id = 1
  AND code = 'feishu.message.send';
