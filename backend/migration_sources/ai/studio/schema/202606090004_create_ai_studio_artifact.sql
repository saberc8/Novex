-- Studio artifact runtime schema and built-in notebook actions.

CREATE TABLE IF NOT EXISTS ai_studio_action (
    id                 BIGINT       NOT NULL,
    tenant_id          BIGINT       NOT NULL DEFAULT 1,
    code               VARCHAR(128) NOT NULL,
    name               VARCHAR(128) NOT NULL,
    description        TEXT         DEFAULT NULL,
    surface            VARCHAR(64)  NOT NULL,
    artifact_type      VARCHAR(64)  NOT NULL,
    plugin_code        VARCHAR(128) DEFAULT NULL,
    skill_code         VARCHAR(128) DEFAULT NULL,
    permission_code    VARCHAR(128) NOT NULL,
    model_route_policy JSONB        NOT NULL DEFAULT '{}'::jsonb,
    input_schema       JSONB        NOT NULL DEFAULT '{}'::jsonb,
    output_schema      JSONB        NOT NULL DEFAULT '{}'::jsonb,
    renderer           VARCHAR(64)  NOT NULL DEFAULT 'default',
    sort               INT          NOT NULL DEFAULT 0,
    status             SMALLINT     NOT NULL DEFAULT 1,
    metadata           JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user        BIGINT       NOT NULL,
    create_time        TIMESTAMP    NOT NULL,
    update_user        BIGINT       DEFAULT NULL,
    update_time        TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_studio_action_tenant_code
    ON ai_studio_action (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_studio_action_surface
    ON ai_studio_action (tenant_id, surface, status, sort);
CREATE INDEX IF NOT EXISTS idx_ai_studio_action_artifact_type
    ON ai_studio_action (tenant_id, artifact_type);

CREATE TABLE IF NOT EXISTS ai_studio_artifact (
    id              BIGINT       NOT NULL,
    tenant_id       BIGINT       NOT NULL DEFAULT 1,
    dataset_id      BIGINT       DEFAULT NULL,
    session_id      BIGINT       DEFAULT NULL,
    run_id          BIGINT       DEFAULT NULL,
    rag_trace_id    BIGINT       DEFAULT NULL,
    action_code     VARCHAR(128) NOT NULL,
    artifact_type   VARCHAR(64)  NOT NULL,
    title           VARCHAR(255) NOT NULL,
    content_json    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    content_text    TEXT         NOT NULL DEFAULT '',
    source_snapshot JSONB        NOT NULL DEFAULT '{}'::jsonb,
    citations       JSONB        NOT NULL DEFAULT '[]'::jsonb,
    version         INT          NOT NULL DEFAULT 1,
    status          SMALLINT     NOT NULL DEFAULT 1,
    metadata        JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user     BIGINT       NOT NULL,
    create_time     TIMESTAMP    NOT NULL,
    update_user     BIGINT       DEFAULT NULL,
    update_time     TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_studio_artifact_tenant_id
    ON ai_studio_artifact (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_studio_artifact_dataset
    ON ai_studio_artifact (tenant_id, dataset_id, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_ai_studio_artifact_session
    ON ai_studio_artifact (tenant_id, session_id, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_ai_studio_artifact_action
    ON ai_studio_artifact (tenant_id, action_code, create_time DESC);
CREATE INDEX IF NOT EXISTS idx_ai_studio_artifact_run
    ON ai_studio_artifact (tenant_id, run_id);
CREATE INDEX IF NOT EXISTS idx_ai_studio_artifact_trace
    ON ai_studio_artifact (tenant_id, rag_trace_id);

INSERT INTO ai_studio_action (
    id, tenant_id, code, name, description, surface, artifact_type, plugin_code,
    skill_code, permission_code, model_route_policy, input_schema, output_schema,
    renderer, sort, status, metadata, create_user, create_time
)
VALUES (
    3500001,
    1,
    'mind_map.generate',
    '思维导图',
    'Generate a cited mind map from the selected knowledge notebook.',
    'knowledge',
    'mind_map',
    'builtin.notebook-studio',
    'mind_map',
    'ai:studio:artifact:create',
    '{"answerModel":"runtime.llm.rag_answer","fallbackModel":"runtime.llm.chat"}'::jsonb,
    '{"type":"object","properties":{"topic":{"type":"string","description":"用户总结方向"},"maxNodes":{"type":"integer","minimum":12,"maximum":96}}}'::jsonb,
    '{"type":"object","required":["title","nodes","edges","citations"],"properties":{"title":{"type":"string"},"nodes":{"type":"array"},"edges":{"type":"array"},"citations":{"type":"array"}}}'::jsonb,
    'markmap',
    40,
    1,
    '{"poc":true,"surface":"knowledge","studioGroup":"notebook","icon":"GitBranch","renderer":"markmap"}'::jsonb,
    1,
    NOW()
)
ON CONFLICT (tenant_id, code) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    surface = EXCLUDED.surface,
    artifact_type = EXCLUDED.artifact_type,
    plugin_code = EXCLUDED.plugin_code,
    skill_code = EXCLUDED.skill_code,
    permission_code = EXCLUDED.permission_code,
    model_route_policy = EXCLUDED.model_route_policy,
    input_schema = EXCLUDED.input_schema,
    output_schema = EXCLUDED.output_schema,
    renderer = EXCLUDED.renderer,
    sort = EXCLUDED.sort,
    status = EXCLUDED.status,
    metadata = EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = NOW();
