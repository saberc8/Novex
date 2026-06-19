-- Backfill M5 template skills into the M2 capability registry for databases
-- initialized before the customer template manifests were aligned.

INSERT INTO ai_skill
    (id, tenant_id, code, name, description, status, model_route_policy, capability_refs, metadata, create_user, create_time)
VALUES
    (3200101, 1, 'general_chat', 'General Chat', 'Direct model conversation skill without retrieval.', 1,
     '{"chatModel":"runtime.llm.chat"}'::jsonb,
     '[{"kind":"model_route","code":"runtime.llm.chat"}]'::jsonb,
     '{"milestone":"M2","poc":true,"template":"llm_chat","evalSets":["llm_chat_smoke"]}'::jsonb, 1, NOW()),
    (3200102, 1, 'cited_answer', 'Cited Answer', 'RAG question answering skill with grounded citations.', 1,
     '{"answerModel":"runtime.llm.rag_answer","embeddingModel":"runtime.embedding.default","rerankModel":"runtime.rerank.default"}'::jsonb,
     '[{"kind":"tool","code":"rag.search"}]'::jsonb,
     '{"milestone":"M2","poc":true,"template":"knowledge_base_chat","evalSets":["knowledge_base_regression"]}'::jsonb, 1, NOW()),
    (3200103, 1, 'task_planning', 'Task Planning', 'Routes user tasks into a bounded ReAct run graph.', 1,
     '{"agentModel":"runtime.llm.chat","intentModel":"runtime.llm.chat"}'::jsonb,
     '[{"kind":"tool","code":"rag.search"},{"kind":"tool","code":"github.repo.search"},{"kind":"tool","code":"feishu.message.send"}]'::jsonb,
     '{"milestone":"M2","poc":true,"template":"agent_workspace","evalSets":["agent_workspace_regression"]}'::jsonb, 1, NOW()),
    (3200104, 1, 'training_quiz', 'Training Quiz', 'Builds quizzes from cited training content.', 1,
     '{"answerModel":"runtime.llm.rag_answer","embeddingModel":"runtime.embedding.default","rerankModel":"runtime.rerank.default"}'::jsonb,
     '[{"kind":"tool","code":"rag.search"}]'::jsonb,
     '{"milestone":"M2","poc":true,"template":"training_app","evalSets":["training_regression"]}'::jsonb, 1, NOW()),
    (3200105, 1, 'training_reminder', 'Training Reminder', 'Schedules and sends training reminders through approved messaging tools.', 1,
     '{"agentModel":"runtime.llm.chat","intentModel":"runtime.llm.chat"}'::jsonb,
     '[{"kind":"tool","code":"feishu.message.send"},{"kind":"trigger","code":"training.reminder.schedule"}]'::jsonb,
     '{"milestone":"M2","poc":true,"template":"training_app","evalSets":["training_regression"]}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    status = EXCLUDED.status,
    model_route_policy = EXCLUDED.model_route_policy,
    capability_refs = EXCLUDED.capability_refs,
    metadata = EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = NOW();
