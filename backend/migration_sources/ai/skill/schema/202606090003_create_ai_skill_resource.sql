CREATE TABLE IF NOT EXISTS ai_skill_resource (
    id BIGINT PRIMARY KEY,
    tenant_id BIGINT NOT NULL DEFAULT 1,
    skill_id BIGINT NOT NULL,
    resource_type VARCHAR(32) NOT NULL,
    relative_path VARCHAR(512) NOT NULL,
    mime_type VARCHAR(128) NOT NULL DEFAULT 'text/plain',
    content_text TEXT,
    storage_ref VARCHAR(512),
    content_sha256 VARCHAR(64) NOT NULL,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    status SMALLINT NOT NULL DEFAULT 1,
    create_user BIGINT,
    create_time TIMESTAMP NOT NULL DEFAULT NOW(),
    update_user BIGINT,
    update_time TIMESTAMP,
    CONSTRAINT fk_ai_skill_resource_skill
        FOREIGN KEY (skill_id) REFERENCES ai_skill(id) ON DELETE CASCADE,
    CONSTRAINT uk_ai_skill_resource_path
        UNIQUE (tenant_id, skill_id, relative_path)
);

CREATE INDEX IF NOT EXISTS idx_ai_skill_resource_skill_type
    ON ai_skill_resource (tenant_id, skill_id, resource_type, status);

