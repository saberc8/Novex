-- Durable agent turn-item ledger for Codex-style ResponseItem replay.
CREATE TABLE IF NOT EXISTS ai_agent_turn_item (
    id BIGINT NOT NULL,
    tenant_id BIGINT NOT NULL DEFAULT 1,
    run_id BIGINT NOT NULL,
    step_id BIGINT DEFAULT NULL,
    source_event_id BIGINT NOT NULL,
    sequence_no BIGINT NOT NULL,
    item_type VARCHAR(64) NOT NULL,
    call_id VARCHAR(128) DEFAULT NULL,
    tool_code VARCHAR(128) DEFAULT NULL,
    item_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    create_user BIGINT NOT NULL,
    create_time TIMESTAMP NOT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_agent_turn_item_run_sequence
    ON ai_agent_turn_item (run_id, sequence_no);
CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_agent_turn_item_event
    ON ai_agent_turn_item (source_event_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_turn_item_tenant_id
    ON ai_agent_turn_item (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_turn_item_run_id
    ON ai_agent_turn_item (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_turn_item_type
    ON ai_agent_turn_item (item_type);
CREATE INDEX IF NOT EXISTS idx_ai_agent_turn_item_call_id
    ON ai_agent_turn_item (call_id);
CREATE INDEX IF NOT EXISTS idx_ai_agent_turn_item_tool_code
    ON ai_agent_turn_item (tool_code);
