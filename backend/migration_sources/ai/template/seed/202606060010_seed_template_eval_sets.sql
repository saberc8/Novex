-- Backfill M5 template eval sets for environments that already applied the
-- initial M4 eval runtime migration before all customer templates existed.

INSERT INTO ai_eval_dataset
    (id, tenant_id, code, name, description, target_scope, status, metadata, create_user, create_time)
VALUES
    (3400002, 1, 'llm_chat_smoke', 'LLM Chat Smoke', 'Default M5 smoke set for pure model chat latency and cost checks.', 'chat', 1,
     '{"milestone":"M5","template":"llm_chat","caseCount":4}'::jsonb, 1, NOW()),
    (3400003, 1, 'knowledge_base_regression', 'Knowledge Base Regression', 'Default M5 regression set for RAG citation and retrieval checks.', 'knowledge', 1,
     '{"milestone":"M5","template":"knowledge_base_chat","caseCount":8}'::jsonb, 1, NOW()),
    (3400004, 1, 'agent_workspace_regression', 'Agent Workspace Regression', 'Default M5 regression set for agent intent and tool-routing checks.', 'agent', 1,
     '{"milestone":"M5","template":"agent_workspace","caseCount":10}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    target_scope = EXCLUDED.target_scope,
    metadata = EXCLUDED.metadata,
    status = EXCLUDED.status,
    update_user = EXCLUDED.create_user,
    update_time = NOW();

INSERT INTO ai_eval_case
    (id, tenant_id, dataset_id, case_code, target_kind, metric_kind, prompt, expected_payload, tags, status, sort, create_user, create_time)
VALUES
    (3402001, 1, 3400002, 'llm-latency-draft', 'safety', 'latency', 'Draft a concise onboarding welcome message.', '{"maxLatencyMs":50}'::jsonb, '["llm","latency"]'::jsonb, 1, 1, 1, NOW()),
    (3402002, 1, 3400002, 'llm-latency-summary', 'safety', 'latency', 'Summarize this policy in one sentence.', '{"maxLatencyMs":50}'::jsonb, '["llm","latency"]'::jsonb, 1, 2, 1, NOW()),
    (3402003, 1, 3400002, 'llm-cost-draft', 'safety', 'cost', 'Draft a short support reply.', '{"maxCostCents":0}'::jsonb, '["llm","cost"]'::jsonb, 1, 3, 1, NOW()),
    (3402004, 1, 3400002, 'llm-cost-classify', 'safety', 'cost', 'Classify this request as support or sales.', '{"maxCostCents":0}'::jsonb, '["llm","cost"]'::jsonb, 1, 4, 1, NOW()),

    (3403001, 1, 3400003, 'kb-citation-policy', 'rag', 'citation_accuracy', 'Where is the policy defined?', '{"answerContains":["policy"],"citations":["kb-handbook:0"]}'::jsonb, '["knowledge","citation"]'::jsonb, 1, 1, 1, NOW()),
    (3403002, 1, 3400003, 'kb-citation-faq', 'rag', 'citation_accuracy', 'Which FAQ answers access requests?', '{"answerContains":["FAQ"],"citations":["kb-handbook:1"]}'::jsonb, '["knowledge","citation"]'::jsonb, 1, 2, 1, NOW()),
    (3403003, 1, 3400003, 'kb-citation-product', 'rag', 'citation_accuracy', 'Where is product setup documented?', '{"answerContains":["product"],"citations":["kb-handbook:2"]}'::jsonb, '["knowledge","citation"]'::jsonb, 1, 3, 1, NOW()),
    (3403004, 1, 3400003, 'kb-citation-support', 'rag', 'citation_accuracy', 'Where is support escalation described?', '{"answerContains":["support"],"citations":["kb-handbook:3"]}'::jsonb, '["knowledge","citation"]'::jsonb, 1, 4, 1, NOW()),
    (3403005, 1, 3400003, 'kb-retrieval-policy', 'rag', 'retrieval_recall', 'Find the policy source.', '{"citations":["kb-handbook:0"]}'::jsonb, '["knowledge","retrieval"]'::jsonb, 1, 5, 1, NOW()),
    (3403006, 1, 3400003, 'kb-retrieval-faq', 'rag', 'retrieval_recall', 'Find the FAQ source.', '{"citations":["kb-handbook:1"]}'::jsonb, '["knowledge","retrieval"]'::jsonb, 1, 6, 1, NOW()),
    (3403007, 1, 3400003, 'kb-retrieval-product', 'rag', 'retrieval_recall', 'Find the product source.', '{"citations":["kb-handbook:2"]}'::jsonb, '["knowledge","retrieval"]'::jsonb, 1, 7, 1, NOW()),
    (3403008, 1, 3400003, 'kb-retrieval-support', 'rag', 'retrieval_recall', 'Find the support source.', '{"citations":["kb-handbook:3"]}'::jsonb, '["knowledge","retrieval"]'::jsonb, 1, 8, 1, NOW()),

    (3404001, 1, 3400004, 'agent-intent-task', 'intent', 'intent_accuracy', 'Plan a bounded agent task.', '{"intent":"tool_task"}'::jsonb, '["agent","intent"]'::jsonb, 1, 1, 1, NOW()),
    (3404002, 1, 3400004, 'agent-intent-approval', 'intent', 'intent_accuracy', 'Ask for approval before sending.', '{"intent":"human_handoff"}'::jsonb, '["agent","intent"]'::jsonb, 1, 2, 1, NOW()),
    (3404003, 1, 3400004, 'agent-intent-code', 'intent', 'intent_accuracy', 'Search the repository for errors.', '{"intent":"code_search"}'::jsonb, '["agent","intent"]'::jsonb, 1, 3, 1, NOW()),
    (3404004, 1, 3400004, 'agent-intent-chat', 'intent', 'intent_accuracy', 'Summarize the run result.', '{"intent":"chat"}'::jsonb, '["agent","intent"]'::jsonb, 1, 4, 1, NOW()),
    (3404005, 1, 3400004, 'agent-intent-rag', 'intent', 'intent_accuracy', 'Look up the runbook before acting.', '{"intent":"rag_question"}'::jsonb, '["agent","intent"]'::jsonb, 1, 5, 1, NOW()),
    (3404006, 1, 3400004, 'agent-tool-feishu', 'tool', 'tool_accuracy', 'Send a controlled Feishu notice.', '{"toolCode":"feishu.message.send"}'::jsonb, '["agent","tool"]'::jsonb, 1, 6, 1, NOW()),
    (3404007, 1, 3400004, 'agent-tool-github', 'tool', 'tool_accuracy', 'Inspect a GitHub repository.', '{"toolCode":"github.repo.search"}'::jsonb, '["agent","tool"]'::jsonb, 1, 7, 1, NOW()),
    (3404008, 1, 3400004, 'agent-tool-rag', 'tool', 'tool_accuracy', 'Search approved knowledge before acting.', '{"toolCode":"rag.search"}'::jsonb, '["agent","tool"]'::jsonb, 1, 8, 1, NOW()),
    (3404009, 1, 3400004, 'agent-tool-image', 'tool', 'tool_accuracy', 'Generate a status poster.', '{"toolCode":"media.image.generate"}'::jsonb, '["agent","tool"]'::jsonb, 1, 9, 1, NOW()),
    (3404010, 1, 3400004, 'agent-tool-audit', 'tool', 'tool_accuracy', 'Audit an external action.', '{"toolCode":"tool.audit.record"}'::jsonb, '["agent","tool"]'::jsonb, 1, 10, 1, NOW())
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
