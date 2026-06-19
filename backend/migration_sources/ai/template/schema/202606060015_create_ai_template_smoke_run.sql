-- M5 template smoke runner history and check-level results.

CREATE TABLE IF NOT EXISTS ai_template_smoke_run (
    id                 BIGINT       NOT NULL,
    tenant_id          BIGINT       NOT NULL,
    template_code      VARCHAR(128) NOT NULL,
    package_id         VARCHAR(320) DEFAULT NULL,
    smoke_script       VARCHAR(255) NOT NULL,
    status             VARCHAR(32)  NOT NULL,
    dry_run            BOOLEAN      NOT NULL DEFAULT TRUE,
    total_checks       INTEGER      NOT NULL DEFAULT 0,
    passed_checks      INTEGER      NOT NULL DEFAULT 0,
    failed_checks      INTEGER      NOT NULL DEFAULT 0,
    result_payload     JSONB        NOT NULL DEFAULT '{}'::jsonb,
    metadata           JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user        BIGINT       NOT NULL,
    create_time        TIMESTAMP    NOT NULL,
    update_user        BIGINT       DEFAULT NULL,
    update_time        TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_template_smoke_run_tenant_template
    ON ai_template_smoke_run (tenant_id, template_code, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_ai_template_smoke_run_package
    ON ai_template_smoke_run (tenant_id, package_id);
CREATE INDEX IF NOT EXISTS idx_ai_template_smoke_run_status
    ON ai_template_smoke_run (tenant_id, status);

CREATE TABLE IF NOT EXISTS ai_template_smoke_result (
    id                 BIGINT       NOT NULL,
    tenant_id          BIGINT       NOT NULL,
    run_id             BIGINT       NOT NULL,
    check_code         VARCHAR(128) NOT NULL,
    name               VARCHAR(128) NOT NULL,
    workdir            VARCHAR(255) NOT NULL,
    command            TEXT         NOT NULL,
    status             VARCHAR(32)  NOT NULL,
    exit_code          INTEGER      DEFAULT NULL,
    stdout             TEXT         DEFAULT NULL,
    stderr             TEXT         DEFAULT NULL,
    duration_ms        BIGINT       NOT NULL DEFAULT 0,
    create_user        BIGINT       NOT NULL,
    create_time        TIMESTAMP    NOT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_template_smoke_result_run_id
    ON ai_template_smoke_result (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_template_smoke_result_tenant_status
    ON ai_template_smoke_result (tenant_id, status);
