-- Runtime model route circuit breaker state shared across backend instances.

CREATE TABLE IF NOT EXISTS ai_model_route_circuit_breaker (
    id               BIGINT       NOT NULL,
    tenant_id        BIGINT       NOT NULL DEFAULT 1,
    route_id         VARCHAR(128) NOT NULL,
    opened_until     TIMESTAMP    NOT NULL,
    open_reason      VARCHAR(64)  NOT NULL DEFAULT 'provider_failure',
    last_error_kind  VARCHAR(64)  DEFAULT NULL,
    last_http_status INTEGER      DEFAULT NULL,
    create_user      BIGINT       NOT NULL,
    create_time      TIMESTAMP    NOT NULL,
    update_user      BIGINT       DEFAULT NULL,
    update_time      TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_model_route_circuit_breaker_tenant_route
    ON ai_model_route_circuit_breaker (tenant_id, route_id);

CREATE INDEX IF NOT EXISTS idx_ai_model_route_circuit_breaker_opened_until
    ON ai_model_route_circuit_breaker (opened_until);
