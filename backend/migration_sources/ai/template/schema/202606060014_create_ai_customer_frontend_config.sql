-- Customer frontend configuration snapshots applied from M5 delivery templates.

CREATE TABLE IF NOT EXISTS ai_customer_frontend_config (
    id                 BIGINT       NOT NULL,
    package_id         VARCHAR(320) NOT NULL,
    tenant_id          BIGINT       NOT NULL,
    template_code      VARCHAR(128) NOT NULL,
    app_id             VARCHAR(128) NOT NULL,
    frontend_entry     VARCHAR(255) NOT NULL,
    entry_url          TEXT         NOT NULL,
    default_path       VARCHAR(255) NOT NULL,
    branding           JSONB        NOT NULL DEFAULT '{}'::jsonb,
    navigation         JSONB        NOT NULL DEFAULT '[]'::jsonb,
    allowed_roles      JSONB        NOT NULL DEFAULT '[]'::jsonb,
    status             SMALLINT     NOT NULL DEFAULT 1,
    metadata           JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user        BIGINT       NOT NULL,
    create_time        TIMESTAMP    NOT NULL,
    update_user        BIGINT       DEFAULT NULL,
    update_time        TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_customer_frontend_config_package
    ON ai_customer_frontend_config (package_id);
CREATE INDEX IF NOT EXISTS idx_ai_customer_frontend_config_tenant_template
    ON ai_customer_frontend_config (tenant_id, template_code);
CREATE INDEX IF NOT EXISTS idx_ai_customer_frontend_config_tenant_app
    ON ai_customer_frontend_config (tenant_id, app_id);
