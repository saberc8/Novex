-- M0 foundation control-plane contracts: tenant, ACL, quota, identity, and secret metadata.

CREATE TABLE IF NOT EXISTS sys_tenant (
    id            BIGINT       NOT NULL,
    code          VARCHAR(64)  NOT NULL,
    name          VARCHAR(100) NOT NULL,
    status        SMALLINT     NOT NULL DEFAULT 1,
    plan_code     VARCHAR(64)  DEFAULT NULL,
    metadata      JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user   BIGINT       NOT NULL,
    create_time   TIMESTAMP    NOT NULL,
    update_user   BIGINT       DEFAULT NULL,
    update_time   TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_tenant_code ON sys_tenant (code);
CREATE INDEX IF NOT EXISTS idx_sys_tenant_status ON sys_tenant (status);

CREATE TABLE IF NOT EXISTS sys_tenant_user (
    id            BIGINT      NOT NULL,
    tenant_id     BIGINT      NOT NULL,
    user_id       BIGINT      NOT NULL,
    member_type   VARCHAR(32) NOT NULL DEFAULT 'user',
    status        SMALLINT    NOT NULL DEFAULT 1,
    joined_at     TIMESTAMP   NOT NULL,
    create_user   BIGINT      NOT NULL,
    create_time   TIMESTAMP   NOT NULL,
    update_user   BIGINT      DEFAULT NULL,
    update_time   TIMESTAMP   DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_tenant_user ON sys_tenant_user (tenant_id, user_id);
CREATE INDEX IF NOT EXISTS idx_sys_tenant_user_tenant_id ON sys_tenant_user (tenant_id);
CREATE INDEX IF NOT EXISTS idx_sys_tenant_user_user_id ON sys_tenant_user (user_id);

CREATE TABLE IF NOT EXISTS sys_tenant_role (
    id            BIGINT    NOT NULL,
    tenant_id     BIGINT    NOT NULL,
    role_id       BIGINT    NOT NULL,
    status        SMALLINT  NOT NULL DEFAULT 1,
    create_user   BIGINT    NOT NULL,
    create_time   TIMESTAMP NOT NULL,
    update_user   BIGINT    DEFAULT NULL,
    update_time   TIMESTAMP DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_tenant_role ON sys_tenant_role (tenant_id, role_id);
CREATE INDEX IF NOT EXISTS idx_sys_tenant_role_tenant_id ON sys_tenant_role (tenant_id);

CREATE TABLE IF NOT EXISTS sys_member_group (
    id            BIGINT       NOT NULL,
    tenant_id     BIGINT       NOT NULL,
    code          VARCHAR(64)  NOT NULL,
    name          VARCHAR(100) NOT NULL,
    description   TEXT         DEFAULT NULL,
    status        SMALLINT     NOT NULL DEFAULT 1,
    create_user   BIGINT       NOT NULL,
    create_time   TIMESTAMP    NOT NULL,
    update_user   BIGINT       DEFAULT NULL,
    update_time   TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_member_group_tenant_code ON sys_member_group (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_sys_member_group_tenant_id ON sys_member_group (tenant_id);

CREATE TABLE IF NOT EXISTS sys_member_group_user (
    id            BIGINT    NOT NULL,
    tenant_id     BIGINT    NOT NULL,
    group_id      BIGINT    NOT NULL,
    user_id       BIGINT    NOT NULL,
    status        SMALLINT  NOT NULL DEFAULT 1,
    create_user   BIGINT    NOT NULL,
    create_time   TIMESTAMP NOT NULL,
    update_user   BIGINT    DEFAULT NULL,
    update_time   TIMESTAMP DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_member_group_user ON sys_member_group_user (tenant_id, group_id, user_id);
CREATE INDEX IF NOT EXISTS idx_sys_member_group_user_group_id ON sys_member_group_user (group_id);
CREATE INDEX IF NOT EXISTS idx_sys_member_group_user_user_id ON sys_member_group_user (user_id);

CREATE TABLE IF NOT EXISTS sys_resource_permission (
    id                BIGINT       NOT NULL,
    tenant_id         BIGINT       NOT NULL,
    resource_type     VARCHAR(64)  NOT NULL,
    resource_id       VARCHAR(128) NOT NULL,
    subject_type      VARCHAR(32)  NOT NULL,
    subject_id        VARCHAR(128) NOT NULL,
    permission_value  VARCHAR(32)  NOT NULL,
    effect            VARCHAR(16)  NOT NULL DEFAULT 'allow',
    inherit_policy    VARCHAR(32)  NOT NULL DEFAULT 'inherit',
    expires_at        TIMESTAMP    DEFAULT NULL,
    metadata          JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user       BIGINT       NOT NULL,
    create_time       TIMESTAMP    NOT NULL,
    update_user       BIGINT       DEFAULT NULL,
    update_time       TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_resource_permission_subject ON sys_resource_permission
    (tenant_id, resource_type, resource_id, subject_type, subject_id, permission_value);
CREATE INDEX IF NOT EXISTS idx_sys_resource_permission_resource ON sys_resource_permission (tenant_id, resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_sys_resource_permission_subject ON sys_resource_permission (tenant_id, subject_type, subject_id);

CREATE TABLE IF NOT EXISTS sys_quota_policy (
    id              BIGINT       NOT NULL,
    tenant_id       BIGINT       NOT NULL,
    scope_type      VARCHAR(64)  NOT NULL,
    scope_id        VARCHAR(128) NOT NULL,
    resource_type   VARCHAR(64)  NOT NULL,
    quota_limit     NUMERIC(18, 4) NOT NULL,
    quota_window    VARCHAR(32)  NOT NULL DEFAULT 'monthly',
    metadata        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status          SMALLINT     NOT NULL DEFAULT 1,
    create_user     BIGINT       NOT NULL,
    create_time     TIMESTAMP    NOT NULL,
    update_user     BIGINT       DEFAULT NULL,
    update_time     TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_quota_policy_scope ON sys_quota_policy
    (tenant_id, scope_type, scope_id, resource_type, quota_window);
CREATE INDEX IF NOT EXISTS idx_sys_quota_policy_tenant_id ON sys_quota_policy (tenant_id);

CREATE TABLE IF NOT EXISTS sys_usage_meter (
    id              BIGINT       NOT NULL,
    tenant_id       BIGINT       NOT NULL,
    scope_type      VARCHAR(64)  NOT NULL,
    scope_id        VARCHAR(128) NOT NULL,
    resource_type   VARCHAR(64)  NOT NULL,
    usage_value     NUMERIC(18, 4) NOT NULL DEFAULT 0,
    usage_unit      VARCHAR(32)  NOT NULL DEFAULT 'count',
    window_start    TIMESTAMP    NOT NULL,
    window_end      TIMESTAMP    NOT NULL,
    metadata        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user     BIGINT       NOT NULL,
    create_time     TIMESTAMP    NOT NULL,
    update_user     BIGINT       DEFAULT NULL,
    update_time     TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_usage_meter_window ON sys_usage_meter
    (tenant_id, scope_type, scope_id, resource_type, usage_unit, window_start, window_end);
CREATE INDEX IF NOT EXISTS idx_sys_usage_meter_tenant_id ON sys_usage_meter (tenant_id);

CREATE TABLE IF NOT EXISTS sys_rate_limit_policy (
    id               BIGINT       NOT NULL,
    tenant_id        BIGINT       NOT NULL,
    scope_type       VARCHAR(64)  NOT NULL,
    scope_id         VARCHAR(128) NOT NULL,
    resource_type    VARCHAR(64)  NOT NULL,
    limit_count      INTEGER      NOT NULL,
    window_seconds   INTEGER      NOT NULL,
    burst_count      INTEGER      NOT NULL DEFAULT 0,
    status           SMALLINT     NOT NULL DEFAULT 1,
    metadata         JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user      BIGINT       NOT NULL,
    create_time      TIMESTAMP    NOT NULL,
    update_user      BIGINT       DEFAULT NULL,
    update_time      TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_rate_limit_policy_scope ON sys_rate_limit_policy
    (tenant_id, scope_type, scope_id, resource_type, window_seconds);
CREATE INDEX IF NOT EXISTS idx_sys_rate_limit_policy_tenant_id ON sys_rate_limit_policy (tenant_id);

CREATE TABLE IF NOT EXISTS sys_identity_provider (
    id               BIGINT       NOT NULL,
    tenant_id        BIGINT       NOT NULL DEFAULT 1,
    provider_type    VARCHAR(64)  NOT NULL,
    code             VARCHAR(64)  NOT NULL,
    name             VARCHAR(100) NOT NULL,
    client_id        VARCHAR(255) DEFAULT NULL,
    secret_ref       VARCHAR(128) DEFAULT NULL,
    allowed_domains  JSONB        NOT NULL DEFAULT '[]'::jsonb,
    tenant_policy    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status           SMALLINT     NOT NULL DEFAULT 1,
    create_user      BIGINT       NOT NULL,
    create_time      TIMESTAMP    NOT NULL,
    update_user      BIGINT       DEFAULT NULL,
    update_time      TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_identity_provider_tenant_code ON sys_identity_provider (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_sys_identity_provider_type ON sys_identity_provider (provider_type);

CREATE TABLE IF NOT EXISTS sys_external_account (
    id                BIGINT       NOT NULL,
    tenant_id         BIGINT       NOT NULL,
    provider_id       BIGINT       NOT NULL,
    user_id           BIGINT       NOT NULL,
    external_subject  VARCHAR(255) NOT NULL,
    display_name      VARCHAR(255) DEFAULT NULL,
    email             VARCHAR(255) DEFAULT NULL,
    metadata          JSONB        NOT NULL DEFAULT '{}'::jsonb,
    last_login_at     TIMESTAMP    DEFAULT NULL,
    status            SMALLINT     NOT NULL DEFAULT 1,
    create_user       BIGINT       NOT NULL,
    create_time       TIMESTAMP    NOT NULL,
    update_user       BIGINT       DEFAULT NULL,
    update_time       TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_external_account_subject ON sys_external_account
    (tenant_id, provider_id, external_subject);
CREATE INDEX IF NOT EXISTS idx_sys_external_account_user_id ON sys_external_account (user_id);

CREATE TABLE IF NOT EXISTS sys_oauth_state (
    id                   BIGINT       NOT NULL,
    tenant_id            BIGINT       NOT NULL,
    provider_id          BIGINT       NOT NULL,
    state_hash           VARCHAR(128) NOT NULL,
    redirect_uri         TEXT         NOT NULL,
    requested_scopes     JSONB        NOT NULL DEFAULT '[]'::jsonb,
    code_verifier_hash   VARCHAR(128) DEFAULT NULL,
    expires_at           TIMESTAMP    NOT NULL,
    consumed_at          TIMESTAMP    DEFAULT NULL,
    create_user          BIGINT       NOT NULL,
    create_time          TIMESTAMP    NOT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_oauth_state_hash ON sys_oauth_state (state_hash);
CREATE INDEX IF NOT EXISTS idx_sys_oauth_state_provider_id ON sys_oauth_state (provider_id);

CREATE TABLE IF NOT EXISTS sys_secret (
    id             BIGINT       NOT NULL,
    tenant_id      BIGINT       NOT NULL DEFAULT 1,
    scope_type     VARCHAR(64)  NOT NULL,
    scope_id       VARCHAR(128) NOT NULL,
    code           VARCHAR(128) NOT NULL,
    key_version    INTEGER      NOT NULL DEFAULT 1,
    ciphertext     TEXT         NOT NULL,
    masked_value   VARCHAR(128) NOT NULL,
    expires_at     TIMESTAMP    DEFAULT NULL,
    rotated_at     TIMESTAMP    DEFAULT NULL,
    last_used_at   TIMESTAMP    DEFAULT NULL,
    metadata       JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status         SMALLINT     NOT NULL DEFAULT 1,
    create_user    BIGINT       NOT NULL,
    create_time    TIMESTAMP    NOT NULL,
    update_user    BIGINT       DEFAULT NULL,
    update_time    TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_sys_secret_scope_code ON sys_secret (tenant_id, scope_type, scope_id, code, key_version);
CREATE INDEX IF NOT EXISTS idx_sys_secret_tenant_id ON sys_secret (tenant_id);
CREATE INDEX IF NOT EXISTS idx_sys_secret_scope ON sys_secret (tenant_id, scope_type, scope_id);

INSERT INTO sys_tenant
    (id, code, name, status, plan_code, metadata, create_user, create_time)
VALUES
    (1, 'platform', '平台默认租户', 1, 'poc', '{"default":true,"milestone":"M0"}'::jsonb, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_tenant_user
    (id, tenant_id, user_id, member_type, status, joined_at, create_user, create_time)
VALUES
    (1, 1, 1, 'owner', 1, NOW(), 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO sys_tenant_role
    (id, tenant_id, role_id, status, create_user, create_time)
VALUES
    (1, 1, 1, 1, 1, NOW())
ON CONFLICT DO NOTHING;
