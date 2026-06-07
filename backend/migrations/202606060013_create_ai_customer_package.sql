-- M5 customer package apply history and provisioning snapshots.

CREATE TABLE IF NOT EXISTS ai_customer_package (
    id                       BIGINT       NOT NULL,
    package_id               VARCHAR(320) NOT NULL,
    tenant_id                BIGINT       NOT NULL,
    template_code            VARCHAR(128) NOT NULL,
    customer_name            VARCHAR(128) NOT NULL,
    app_name                 VARCHAR(128) NOT NULL,
    status                   VARCHAR(32)  NOT NULL DEFAULT 'applied',
    package_payload          JSONB        NOT NULL DEFAULT '{}'::jsonb,
    provisioning_plan        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    applied_steps            JSONB        NOT NULL DEFAULT '[]'::jsonb,
    pending_operator_steps   JSONB        NOT NULL DEFAULT '[]'::jsonb,
    metadata                 JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user              BIGINT       NOT NULL,
    create_time              TIMESTAMP    NOT NULL,
    update_user              BIGINT       DEFAULT NULL,
    update_time              TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_customer_package_package_id
    ON ai_customer_package (package_id);
CREATE INDEX IF NOT EXISTS idx_ai_customer_package_tenant_id
    ON ai_customer_package (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_customer_package_template_code
    ON ai_customer_package (template_code);
