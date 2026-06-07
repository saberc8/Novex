-- Connector credential contract.
-- GitHub login remains in sys_identity_provider/sys_external_account;
-- repository access credentials live here and can be tenant/user/app scoped.

CREATE TABLE IF NOT EXISTS ai_connector_credential (
    id            BIGINT       PRIMARY KEY,
    tenant_id     BIGINT       NOT NULL DEFAULT 1,
    connector_id  BIGINT       NOT NULL,
    scope_type    VARCHAR(32)  NOT NULL,
    scope_id      VARCHAR(128) NOT NULL,
    auth_type     VARCHAR(64)  NOT NULL,
    secret_ref    VARCHAR(255) NOT NULL,
    expires_at    TIMESTAMP,
    scopes        JSONB        NOT NULL DEFAULT '[]'::jsonb,
    status        SMALLINT     NOT NULL DEFAULT 1,
    metadata      JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user   BIGINT       NOT NULL DEFAULT 0,
    create_time   TIMESTAMP    NOT NULL DEFAULT NOW(),
    update_user   BIGINT,
    update_time   TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_connector_credential_scope
    ON ai_connector_credential (tenant_id, connector_id, scope_type, scope_id, auth_type);
CREATE INDEX IF NOT EXISTS idx_ai_connector_credential_tenant_id
    ON ai_connector_credential (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_connector_credential_connector
    ON ai_connector_credential (tenant_id, connector_id);
CREATE INDEX IF NOT EXISTS idx_ai_connector_credential_scope
    ON ai_connector_credential (tenant_id, scope_type, scope_id);
CREATE INDEX IF NOT EXISTS idx_ai_connector_credential_status
    ON ai_connector_credential (tenant_id, status);

INSERT INTO ai_connector_credential
    (id, tenant_id, connector_id, scope_type, scope_id, auth_type, secret_ref, scopes, status, metadata, create_user, create_time)
SELECT
    3225001,
    c.tenant_id,
    c.id,
    'tenant',
    c.tenant_id::TEXT,
    c.auth_type,
    'env:GITHUB_CONNECTOR_TOKEN',
    '["repo"]'::jsonb,
    1,
    '{"poc":true,"connector":"github.default","credentialBoundary":"repo_access_not_identity_login"}'::jsonb,
    1,
    NOW()
FROM ai_connector AS c
WHERE c.tenant_id = 1
  AND c.code = 'github.default'
ON CONFLICT (tenant_id, connector_id, scope_type, scope_id, auth_type) DO NOTHING;
