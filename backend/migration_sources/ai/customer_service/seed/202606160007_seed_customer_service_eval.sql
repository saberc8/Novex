-- Customer service agent regression eval set and rollout gate.

INSERT INTO ai_eval_dataset
    (id, tenant_id, code, name, description, target_scope, status, metadata, create_user, create_time)
VALUES
    (3400005, 1, 'customer-service-agent-regression', 'Customer Service Agent Regression', 'Regression gate for customer-service grounded answers, insufficient-evidence handling, approval-gated ticket creation, and human handoff.', 'customer_service', 1,
     '{"milestone":"M6","template":"customer_service_agent","caseCount":4,"gate":{"minAverageScore":0.95,"requiredMetrics":["grounded_resolution","handoff_accuracy"]}}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    target_scope = EXCLUDED.target_scope,
    status = EXCLUDED.status,
    metadata = EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = NOW();

WITH customer_service_eval AS (
    SELECT id
    FROM ai_eval_dataset
    WHERE tenant_id = 1
      AND code = 'customer-service-agent-regression'
    LIMIT 1
)
INSERT INTO ai_eval_case
    (id, tenant_id, dataset_id, case_code, target_kind, metric_kind, prompt, expected_payload, tags, status, sort, create_user, create_time)
SELECT
    case_seed.id,
    1,
    customer_service_eval.id,
    case_seed.case_code,
    case_seed.target_kind,
    case_seed.metric_kind,
    case_seed.prompt,
    case_seed.expected_payload::jsonb,
    case_seed.tags::jsonb,
    1,
    case_seed.sort,
    1,
    NOW()
FROM customer_service_eval
CROSS JOIN (
    VALUES
        (3405001, 'cs-refund-with-citation', 'customer_service', 'grounded_resolution', 'What is the refund window?', '{"answerContains":["30 days"],"citations":["cs-faq:refunds"]}', '["customer-service","faq","citation"]', 1),
        (3405002, 'cs-insufficient-evidence', 'customer_service', 'grounded_resolution', 'Can you guarantee custom warranty extensions?', '{"answerContains":["insufficient evidence"],"citations":[]}', '["customer-service","insufficient-evidence"]', 2),
        (3405003, 'cs-human-handoff', 'customer_service', 'handoff_accuracy', 'I am angry and need a human agent now.', '{"intent":"human_handoff","toolCode":"handoff.request"}', '["customer-service","handoff"]', 3),
        (3405004, 'cs-ticket-approval', 'customer_service', 'grounded_resolution', 'Create a refund ticket for customer C-123.', '{"answerContains":["approval"],"citations":["cs-policy:approval"],"toolCode":"ticket.create"}', '["customer-service","ticket","approval"]', 4)
) AS case_seed(id, case_code, target_kind, metric_kind, prompt, expected_payload, tags, sort)
ON CONFLICT (dataset_id, case_code) DO UPDATE SET
    target_kind = EXCLUDED.target_kind,
    metric_kind = EXCLUDED.metric_kind,
    prompt = EXCLUDED.prompt,
    expected_payload = EXCLUDED.expected_payload,
    tags = EXCLUDED.tags,
    status = EXCLUDED.status,
    sort = EXCLUDED.sort,
    update_user = EXCLUDED.create_user,
    update_time = NOW();
