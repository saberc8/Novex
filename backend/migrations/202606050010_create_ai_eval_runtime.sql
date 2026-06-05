-- Eval runtime schema and seeded training regression cases for Novex M4.

CREATE TABLE IF NOT EXISTS ai_eval_dataset (
    id          BIGINT       NOT NULL,
    tenant_id   BIGINT       NOT NULL DEFAULT 1,
    code        VARCHAR(128) NOT NULL,
    name        VARCHAR(128) NOT NULL,
    description TEXT         DEFAULT NULL,
    target_scope VARCHAR(64) NOT NULL DEFAULT 'training',
    status      SMALLINT     NOT NULL DEFAULT 1,
    metadata    JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user BIGINT       NOT NULL,
    create_time TIMESTAMP    NOT NULL,
    update_user BIGINT       DEFAULT NULL,
    update_time TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_eval_dataset_tenant_code ON ai_eval_dataset (tenant_id, code);
CREATE INDEX IF NOT EXISTS idx_ai_eval_dataset_tenant_id ON ai_eval_dataset (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_dataset_status ON ai_eval_dataset (status);

CREATE TABLE IF NOT EXISTS ai_eval_case (
    id               BIGINT       NOT NULL,
    tenant_id        BIGINT       NOT NULL DEFAULT 1,
    dataset_id       BIGINT       NOT NULL,
    case_code        VARCHAR(128) NOT NULL,
    target_kind      VARCHAR(32)  NOT NULL,
    metric_kind      VARCHAR(64)  NOT NULL,
    prompt           TEXT         NOT NULL,
    expected_payload JSONB        NOT NULL DEFAULT '{}'::jsonb,
    tags             JSONB        NOT NULL DEFAULT '[]'::jsonb,
    status           SMALLINT     NOT NULL DEFAULT 1,
    sort             INTEGER      NOT NULL DEFAULT 0,
    create_user      BIGINT       NOT NULL,
    create_time      TIMESTAMP    NOT NULL,
    update_user      BIGINT       DEFAULT NULL,
    update_time      TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_eval_case_dataset_code ON ai_eval_case (dataset_id, case_code);
CREATE INDEX IF NOT EXISTS idx_ai_eval_case_tenant_id ON ai_eval_case (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_case_dataset_id ON ai_eval_case (dataset_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_case_target_kind ON ai_eval_case (target_kind);
CREATE INDEX IF NOT EXISTS idx_ai_eval_case_status ON ai_eval_case (status);

CREATE TABLE IF NOT EXISTS ai_eval_run (
    id               BIGINT       NOT NULL,
    tenant_id        BIGINT       NOT NULL DEFAULT 1,
    dataset_id       BIGINT       NOT NULL,
    dataset_code     VARCHAR(128) NOT NULL,
    status           VARCHAR(32)  NOT NULL,
    total_cases      INTEGER      NOT NULL DEFAULT 0,
    passed_cases     INTEGER      NOT NULL DEFAULT 0,
    failed_cases     INTEGER      NOT NULL DEFAULT 0,
    average_score    NUMERIC(8, 4) NOT NULL DEFAULT 0,
    metric_breakdown JSONB        NOT NULL DEFAULT '{}'::jsonb,
    report_payload   JSONB        NOT NULL DEFAULT '{}'::jsonb,
    triggered_by     BIGINT       NOT NULL,
    started_at       TIMESTAMP    DEFAULT NULL,
    finished_at      TIMESTAMP    DEFAULT NULL,
    create_user      BIGINT       NOT NULL,
    create_time      TIMESTAMP    NOT NULL,
    update_user      BIGINT       DEFAULT NULL,
    update_time      TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_eval_run_tenant_id ON ai_eval_run (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_run_dataset_id ON ai_eval_run (dataset_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_run_status ON ai_eval_run (status);
CREATE INDEX IF NOT EXISTS idx_ai_eval_run_create_time ON ai_eval_run (create_time DESC);

CREATE TABLE IF NOT EXISTS ai_eval_result (
    id               BIGINT       NOT NULL,
    tenant_id        BIGINT       NOT NULL DEFAULT 1,
    run_id           BIGINT       NOT NULL,
    dataset_id       BIGINT       NOT NULL,
    case_id          BIGINT       NOT NULL,
    case_code        VARCHAR(128) NOT NULL,
    target_kind      VARCHAR(32)  NOT NULL,
    metric_kind      VARCHAR(64)  NOT NULL,
    score            NUMERIC(8, 4) NOT NULL DEFAULT 0,
    passed           BOOLEAN      NOT NULL DEFAULT FALSE,
    expected_payload JSONB        NOT NULL DEFAULT '{}'::jsonb,
    actual_payload   JSONB        NOT NULL DEFAULT '{}'::jsonb,
    reason           TEXT         DEFAULT NULL,
    cost_cents       INTEGER      NOT NULL DEFAULT 0,
    latency_ms       INTEGER      NOT NULL DEFAULT 0,
    create_user      BIGINT       NOT NULL,
    create_time      TIMESTAMP    NOT NULL,
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_ai_eval_result_tenant_id ON ai_eval_result (tenant_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_result_run_id ON ai_eval_result (run_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_result_case_id ON ai_eval_result (case_id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_result_passed ON ai_eval_result (passed);

INSERT INTO ai_eval_dataset
    (id, tenant_id, code, name, description, target_scope, status, metadata, create_user, create_time)
VALUES
    (3400001, 1, 'training_regression', 'Training Regression', 'Seeded POC regression set for RAG, intent, and tool checks.', 'training', 1,
     '{"milestone":"M4","poc":true,"caseCount":20}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO NOTHING;

INSERT INTO ai_eval_case
    (id, tenant_id, dataset_id, case_code, target_kind, metric_kind, prompt, expected_payload, tags, status, sort, create_user, create_time)
VALUES
    (3401001, 1, 3400001, 'rag-training-start', 'rag', 'citation_accuracy', 'When does training start?', '{"answerContains":["Monday"],"citations":["training-handbook:0"]}'::jsonb, '["rag","schedule"]'::jsonb, 1, 1, 1, NOW()),
    (3401002, 1, 3400001, 'rag-hr-policy', 'rag', 'citation_accuracy', 'Where is the HR policy described?', '{"answerContains":["HR policy"],"citations":["training-handbook:1"]}'::jsonb, '["rag","policy"]'::jsonb, 1, 2, 1, NOW()),
    (3401003, 1, 3400001, 'rag-safety', 'rag', 'citation_accuracy', 'What safety module is required?', '{"answerContains":["safety"],"citations":["training-handbook:2"]}'::jsonb, '["rag","safety"]'::jsonb, 1, 3, 1, NOW()),
    (3401004, 1, 3400001, 'rag-manager-review', 'rag', 'citation_accuracy', 'Who reviews completion?', '{"answerContains":["manager"],"citations":["training-handbook:3"]}'::jsonb, '["rag","review"]'::jsonb, 1, 4, 1, NOW()),
    (3401005, 1, 3400001, 'rag-quiz-count', 'rag', 'citation_accuracy', 'How many quiz questions are generated?', '{"answerContains":["5"],"citations":["training-handbook:4"]}'::jsonb, '["rag","quiz"]'::jsonb, 1, 5, 1, NOW()),
    (3401006, 1, 3400001, 'rag-feishu-notice', 'rag', 'citation_accuracy', 'How are reminders sent?', '{"answerContains":["Feishu"],"citations":["training-handbook:5"]}'::jsonb, '["rag","notification"]'::jsonb, 1, 6, 1, NOW()),
    (3401007, 1, 3400001, 'rag-weak-points', 'rag', 'citation_accuracy', 'What does HR inspect after the quiz?', '{"answerContains":["weak"],"citations":["training-handbook:6"]}'::jsonb, '["rag","report"]'::jsonb, 1, 7, 1, NOW()),
    (3401008, 1, 3400001, 'rag-access-control', 'rag', 'citation_accuracy', 'What limits knowledge visibility?', '{"answerContains":["RBAC"],"citations":["training-handbook:7"]}'::jsonb, '["rag","rbac"]'::jsonb, 1, 8, 1, NOW()),

    (3401009, 1, 3400001, 'intent-knowledge-question', 'intent', 'intent_accuracy', 'Ask the employee handbook about onboarding.', '{"intent":"rag_question"}'::jsonb, '["intent","rag"]'::jsonb, 1, 9, 1, NOW()),
    (3401010, 1, 3400001, 'intent-tool-task', 'intent', 'intent_accuracy', 'Send a Feishu reminder to Alice.', '{"intent":"tool_task"}'::jsonb, '["intent","tool"]'::jsonb, 1, 10, 1, NOW()),
    (3401011, 1, 3400001, 'intent-training-quiz', 'intent', 'intent_accuracy', 'Generate a training quiz.', '{"intent":"training_quiz"}'::jsonb, '["intent","quiz"]'::jsonb, 1, 11, 1, NOW()),
    (3401012, 1, 3400001, 'intent-human-handoff', 'intent', 'intent_accuracy', 'I need a human review.', '{"intent":"human_handoff"}'::jsonb, '["intent","handoff"]'::jsonb, 1, 12, 1, NOW()),
    (3401013, 1, 3400001, 'intent-code-search', 'intent', 'intent_accuracy', 'Search the GitHub repo for the policy parser.', '{"intent":"code_search"}'::jsonb, '["intent","code"]'::jsonb, 1, 13, 1, NOW()),
    (3401014, 1, 3400001, 'intent-chat', 'intent', 'intent_accuracy', 'Draft a welcome message.', '{"intent":"chat"}'::jsonb, '["intent","chat"]'::jsonb, 1, 14, 1, NOW()),

    (3401015, 1, 3400001, 'tool-rag-search', 'tool', 'tool_accuracy', 'Search the training handbook.', '{"toolCode":"rag.search"}'::jsonb, '["tool","rag"]'::jsonb, 1, 15, 1, NOW()),
    (3401016, 1, 3400001, 'tool-feishu-send', 'tool', 'tool_accuracy', 'Send a Feishu training reminder.', '{"toolCode":"feishu.message.send"}'::jsonb, '["tool","feishu"]'::jsonb, 1, 16, 1, NOW()),
    (3401017, 1, 3400001, 'tool-image-generate', 'tool', 'tool_accuracy', 'Generate a course poster image.', '{"toolCode":"media.image.generate"}'::jsonb, '["tool","media"]'::jsonb, 1, 17, 1, NOW()),
    (3401018, 1, 3400001, 'tool-policy-search', 'tool', 'tool_accuracy', 'Find the policy citation.', '{"toolCode":"rag.search"}'::jsonb, '["tool","rag"]'::jsonb, 1, 18, 1, NOW()),
    (3401019, 1, 3400001, 'tool-reminder', 'tool', 'tool_accuracy', 'Notify employees in Feishu.', '{"toolCode":"feishu.message.send"}'::jsonb, '["tool","feishu"]'::jsonb, 1, 19, 1, NOW()),
    (3401020, 1, 3400001, 'tool-visual', 'tool', 'tool_accuracy', 'Create a visual for the training page.', '{"toolCode":"media.image.generate"}'::jsonb, '["tool","media"]'::jsonb, 1, 20, 1, NOW())
ON CONFLICT (dataset_id, case_code) DO NOTHING;
