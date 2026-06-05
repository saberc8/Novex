-- M0 model registry contracts: provider, deployment, profile, credential, route, health, and usage.

CREATE TABLE IF NOT EXISTS ai_model_provider (
    id             BIGINT       NOT NULL,
    tenant_id      BIGINT       NOT NULL DEFAULT 1,
    code           VARCHAR(64)  NOT NULL,
    name           VARCHAR(100) NOT NULL,
    provider_type  VARCHAR(64)  NOT NULL,
    protocol       VARCHAR(64)  NOT NULL DEFAULT 'openai-compatible',
    status         SMALLINT     NOT NULL DEFAULT 1,
    metadata       JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user    BIGINT       NOT NULL,
    create_time    TIMESTAMP    NOT NULL,
    update_user    BIGINT       DEFAULT NULL,
    update_time    TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_model_provider_tenant_code ON ai_model_provider (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_model_provider_type ON ai_model_provider (provider_type);

CREATE TABLE IF NOT EXISTS ai_model_deployment (
    id                 BIGINT       NOT NULL,
    tenant_id          BIGINT       NOT NULL DEFAULT 1,
    provider_id        BIGINT       NOT NULL,
    code               VARCHAR(64)  NOT NULL,
    name               VARCHAR(100) NOT NULL,
    endpoint           TEXT         NOT NULL,
    api_path           VARCHAR(255) DEFAULT NULL,
    network_zone       VARCHAR(64)  NOT NULL DEFAULT 'public',
    timeout_ms         INTEGER      NOT NULL DEFAULT 20000,
    max_concurrency    INTEGER      DEFAULT NULL,
    status             SMALLINT     NOT NULL DEFAULT 1,
    metadata           JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user        BIGINT       NOT NULL,
    create_time        TIMESTAMP    NOT NULL,
    update_user        BIGINT       DEFAULT NULL,
    update_time        TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_model_deployment_tenant_code ON ai_model_deployment (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_model_deployment_provider_id ON ai_model_deployment (provider_id);
CREATE INDEX IF NOT EXISTS idx_ai_model_deployment_network_zone ON ai_model_deployment (network_zone);

CREATE TABLE IF NOT EXISTS ai_model_profile (
    id                BIGINT       NOT NULL,
    tenant_id         BIGINT       NOT NULL DEFAULT 1,
    deployment_id     BIGINT       NOT NULL,
    code              VARCHAR(128) NOT NULL,
    name              VARCHAR(128) NOT NULL,
    model_name        VARCHAR(255) NOT NULL,
    model_kind        VARCHAR(64)  NOT NULL,
    capabilities      JSONB        NOT NULL DEFAULT '{}'::jsonb,
    limits            JSONB        NOT NULL DEFAULT '{}'::jsonb,
    embedding_spec    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    rerank_spec       JSONB        NOT NULL DEFAULT '{}'::jsonb,
    cost_spec         JSONB        NOT NULL DEFAULT '{}'::jsonb,
    fallback_policy   JSONB        NOT NULL DEFAULT '{}'::jsonb,
    status            SMALLINT     NOT NULL DEFAULT 1,
    create_user       BIGINT       NOT NULL,
    create_time       TIMESTAMP    NOT NULL,
    update_user       BIGINT       DEFAULT NULL,
    update_time       TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_model_profile_tenant_code ON ai_model_profile (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_model_profile_deployment_id ON ai_model_profile (deployment_id);
CREATE INDEX IF NOT EXISTS idx_ai_model_profile_kind ON ai_model_profile (model_kind);

CREATE TABLE IF NOT EXISTS ai_model_credential (
    id              BIGINT       NOT NULL,
    tenant_id       BIGINT       NOT NULL DEFAULT 1,
    provider_id     BIGINT       NOT NULL,
    deployment_id   BIGINT       DEFAULT NULL,
    code            VARCHAR(128) NOT NULL,
    scope_type      VARCHAR(64)  NOT NULL DEFAULT 'platform',
    scope_id        VARCHAR(128) NOT NULL DEFAULT '1',
    credential_ref  VARCHAR(128) DEFAULT NULL,
    masked_value    VARCHAR(128) DEFAULT NULL,
    status          SMALLINT     NOT NULL DEFAULT 1,
    metadata        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user     BIGINT       NOT NULL,
    create_time     TIMESTAMP    NOT NULL,
    update_user     BIGINT       DEFAULT NULL,
    update_time     TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_model_credential_tenant_code ON ai_model_credential (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_model_credential_provider_id ON ai_model_credential (provider_id);
CREATE INDEX IF NOT EXISTS idx_ai_model_credential_ref ON ai_model_credential (credential_ref);

CREATE TABLE IF NOT EXISTS ai_model_route (
    id                 BIGINT       NOT NULL,
    tenant_id          BIGINT       NOT NULL DEFAULT 1,
    code               VARCHAR(128) NOT NULL,
    route_purpose      VARCHAR(64)  NOT NULL,
    model_profile_id   BIGINT       NOT NULL,
    credential_id      BIGINT       DEFAULT NULL,
    priority           INTEGER      NOT NULL DEFAULT 100,
    fallback_route_id  BIGINT       DEFAULT NULL,
    status             SMALLINT     NOT NULL DEFAULT 1,
    policy             JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user        BIGINT       NOT NULL,
    create_time        TIMESTAMP    NOT NULL,
    update_user        BIGINT       DEFAULT NULL,
    update_time        TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_model_route_tenant_code ON ai_model_route (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_model_route_purpose ON ai_model_route (tenant_id, route_purpose, status, priority);
CREATE INDEX IF NOT EXISTS idx_ai_model_route_profile_id ON ai_model_route (model_profile_id);

CREATE TABLE IF NOT EXISTS ai_model_health_check (
    id                BIGINT       NOT NULL,
    tenant_id         BIGINT       NOT NULL DEFAULT 1,
    route_id          BIGINT       DEFAULT NULL,
    provider_id       BIGINT       DEFAULT NULL,
    model_profile_id  BIGINT       DEFAULT NULL,
    status            VARCHAR(32)  NOT NULL,
    http_status       INTEGER      DEFAULT NULL,
    latency_ms        BIGINT       DEFAULT NULL,
    checked_at        TIMESTAMP    NOT NULL,
    error_message     TEXT         DEFAULT NULL,
    detail            JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user       BIGINT       NOT NULL,
    create_time       TIMESTAMP    NOT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_model_health_check_route_id ON ai_model_health_check (route_id);
CREATE INDEX IF NOT EXISTS idx_ai_model_health_check_checked_at ON ai_model_health_check (checked_at DESC);

CREATE TABLE IF NOT EXISTS ai_model_usage (
    id                BIGINT       NOT NULL,
    tenant_id         BIGINT       NOT NULL DEFAULT 1,
    route_id          BIGINT       DEFAULT NULL,
    model_profile_id  BIGINT       DEFAULT NULL,
    run_id            BIGINT       DEFAULT NULL,
    usage_kind        VARCHAR(64)  NOT NULL,
    prompt_tokens     BIGINT       NOT NULL DEFAULT 0,
    completion_tokens BIGINT       NOT NULL DEFAULT 0,
    total_tokens      BIGINT       NOT NULL DEFAULT 0,
    request_count     BIGINT       NOT NULL DEFAULT 1,
    vector_count      BIGINT       NOT NULL DEFAULT 0,
    cost_cents        NUMERIC(12, 4) NOT NULL DEFAULT 0,
    latency_ms        BIGINT       DEFAULT NULL,
    metadata          JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user       BIGINT       NOT NULL,
    create_time       TIMESTAMP    NOT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_model_usage_tenant_id ON ai_model_usage (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_model_usage_route_id ON ai_model_usage (route_id);
CREATE INDEX IF NOT EXISTS idx_ai_model_usage_create_time ON ai_model_usage (create_time DESC);

INSERT INTO ai_model_provider
    (id, tenant_id, code, name, provider_type, protocol, status, metadata, create_user, create_time)
VALUES
    (1, 1, 'deepseek', 'DeepSeek', 'deep-seek', 'openai-compatible', 1, '{"source":"env","envGroup":"LLM"}'::jsonb, 1, NOW()),
    (2, 1, 'dashscope', 'DashScope', 'dash-scope', 'openai-compatible', 1, '{"source":"env","envGroups":["EMBEDDING","RERANKER"]}'::jsonb, 1, NOW()),
    (3, 1, 'right-code-draw', 'Right Code Draw', 'right-code-draw', 'http', 1, '{"source":"env","envGroup":"RIGHT_CODE_DRAW"}'::jsonb, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO ai_model_deployment
    (id, tenant_id, provider_id, code, name, endpoint, api_path, network_zone, timeout_ms, status, metadata, create_user, create_time)
VALUES
    (1, 1, 1, 'deepseek-public', 'DeepSeek Public API', 'https://api.deepseek.com', '/chat/completions', 'public', 20000, 1, '{"envBaseUrl":"LLM_BASE_URL"}'::jsonb, 1, NOW()),
    (2, 1, 2, 'dashscope-embedding-public', 'DashScope Embedding Public API', 'https://dashscope.aliyuncs.com/compatible-mode/v1', '/embeddings', 'public', 20000, 1, '{"envBaseUrl":"EMBEDDING_BASE_URL"}'::jsonb, 1, NOW()),
    (3, 1, 2, 'dashscope-reranker-public', 'DashScope Reranker Public API', 'https://dashscope.aliyuncs.com/compatible-api/v1', '/reranks', 'public', 20000, 1, '{"envBaseUrl":"RERANKER_BASE_URL"}'::jsonb, 1, NOW()),
    (4, 1, 3, 'right-code-draw-public', 'Right Code Draw Public API', 'https://www.right.codes/draw', NULL, 'public', 20000, 1, '{"envBaseUrl":"RIGHT_CODE_DRAW_BASE_URL"}'::jsonb, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO ai_model_profile
    (id, tenant_id, deployment_id, code, name, model_name, model_kind, capabilities, limits, embedding_spec, rerank_spec, cost_spec, fallback_policy, status, create_user, create_time)
VALUES
    (1, 1, 1, 'deepseek-v4-flash', 'DeepSeek V4 Flash', 'deepseek-v4-flash', 'llm',
     '{"chat":true,"reasoning":true,"jsonMode":false,"toolCalling":false}'::jsonb,
     '{"maxOutputTokens":4096,"timeoutMs":20000}'::jsonb,
     '{}'::jsonb, '{}'::jsonb, '{"unit":"token","source":"provider"}'::jsonb, '{}'::jsonb, 1, 1, NOW()),
    (2, 1, 2, 'text-embedding-v4', 'DashScope Text Embedding V4', 'text-embedding-v4', 'embedding',
     '{"textEmbedding":true,"multilingual":true}'::jsonb,
     '{"batchSize":16,"timeoutMs":20000}'::jsonb,
     '{"dimension":1024,"normalize":false,"distanceMetric":"cosine"}'::jsonb,
     '{}'::jsonb, '{"unit":"vector","source":"provider"}'::jsonb, '{}'::jsonb, 1, 1, NOW()),
    (3, 1, 3, 'qwen3-rerank', 'Qwen3 Rerank', 'qwen3-rerank', 'rerank',
     '{"rerank":true}'::jsonb,
     '{"maxCandidates":100,"timeoutMs":20000}'::jsonb,
     '{}'::jsonb,
     '{"topN":10,"scoreRange":[0,1],"scoreHigherIsBetter":true}'::jsonb,
     '{"unit":"request","source":"provider"}'::jsonb, '{}'::jsonb, 1, 1, NOW()),
    (4, 1, 4, 'right-code-draw', 'Right Code Draw', 'right-code-draw', 'media_generation',
     '{"imageGeneration":true}'::jsonb,
     '{"timeoutMs":20000}'::jsonb,
     '{}'::jsonb, '{}'::jsonb, '{"unit":"request","source":"provider"}'::jsonb, '{}'::jsonb, 1, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO ai_model_credential
    (id, tenant_id, provider_id, deployment_id, code, scope_type, scope_id, credential_ref, masked_value, status, metadata, create_user, create_time)
VALUES
    (1, 1, 1, 1, 'env-llm-api-key', 'platform', '1', 'env:LLM_API_KEY', 'env:LLM_API_KEY', 1, '{"secretSource":"environment"}'::jsonb, 1, NOW()),
    (2, 1, 2, 2, 'env-embedding-api-key', 'platform', '1', 'env:EMBEDDING_API_KEY', 'env:EMBEDDING_API_KEY', 1, '{"secretSource":"environment"}'::jsonb, 1, NOW()),
    (3, 1, 2, 3, 'env-reranker-api-key', 'platform', '1', 'env:RERANKER_API_KEY', 'env:RERANKER_API_KEY', 1, '{"secretSource":"environment"}'::jsonb, 1, NOW()),
    (4, 1, 3, 4, 'env-right-code-draw-api-key', 'platform', '1', 'env:RIGHT_CODE_DRAW_API_KEY', 'env:RIGHT_CODE_DRAW_API_KEY', 1, '{"secretSource":"environment"}'::jsonb, 1, NOW())
ON CONFLICT DO NOTHING;

INSERT INTO ai_model_route
    (id, tenant_id, code, route_purpose, model_profile_id, credential_id, priority, status, policy, create_user, create_time)
VALUES
    (1, 1, 'runtime.llm.chat', 'chat', 1, 1, 100, 1, '{"source":"env"}'::jsonb, 1, NOW()),
    (2, 1, 'runtime.llm.rag_answer', 'rag_answer', 1, 1, 100, 1, '{"source":"env"}'::jsonb, 1, NOW()),
    (3, 1, 'runtime.llm.eval_judge', 'eval_judge', 1, 1, 100, 1, '{"source":"env"}'::jsonb, 1, NOW()),
    (4, 1, 'runtime.llm.code_agent', 'code_agent', 1, 1, 100, 1, '{"source":"env"}'::jsonb, 1, NOW()),
    (5, 1, 'runtime.embedding', 'embedding', 2, 2, 100, 1, '{"source":"env"}'::jsonb, 1, NOW()),
    (6, 1, 'runtime.reranker', 'rerank', 3, 3, 100, 1, '{"source":"env"}'::jsonb, 1, NOW()),
    (7, 1, 'runtime.draw', 'media_generation', 4, 4, 100, 1, '{"source":"env"}'::jsonb, 1, NOW())
ON CONFLICT DO NOTHING;
