-- Attach chat-flow prompt rules to built-in skills. The chat-flow runtime reads
-- these rules from ai_skill.metadata before falling back to code defaults.

UPDATE ai_skill
SET metadata = metadata || jsonb_build_object(
    'promptRules',
    jsonb_build_array(
        'Answer from the selected knowledge base only.',
        'Keep claims grounded in retrieved context.',
        'Use supporting citation labels when they directly support a sentence.'
    ),
    'chatFlowSurface',
    'knowledge'
)
WHERE tenant_id = 1 AND code = 'cited_answer';

UPDATE ai_skill
SET metadata = metadata || jsonb_build_object(
    'promptRules',
    jsonb_build_array(
        'Answer from the selected knowledge base only.',
        'Keep claims grounded in retrieved context.',
        'Use supporting citation labels when they directly support a sentence.'
    ),
    'chatFlowSurface',
    'knowledge'
)
WHERE tenant_id = 1 AND code = 'knowledge_base_chat';

UPDATE ai_skill
SET metadata = metadata || jsonb_build_object(
    'promptRules',
    jsonb_build_array(
        'Generate a training quiz from retrieved content.',
        'Include questions, correct answers, concise explanations, and supporting citations.',
        'Do not introduce facts that are not present in the knowledge context.'
    ),
    'chatFlowSurface',
    'knowledge'
)
WHERE tenant_id = 1 AND code = 'training_quiz';

UPDATE ai_skill
SET metadata = metadata || jsonb_build_object(
    'promptRules',
    jsonb_build_array(
        'Help with training tasks from retrieved content.',
        'Prefer quizzes, learning summaries, reminders, and cited explanations.',
        'Do not introduce facts that are not present in the knowledge context.'
    ),
    'chatFlowSurface',
    'knowledge'
)
WHERE tenant_id = 1 AND code = 'training_assistant';

UPDATE ai_skill
SET metadata = metadata || jsonb_build_object(
    'promptRules',
    jsonb_build_array(
        'Turn the request into a bounded execution plan grounded in the knowledge base.',
        'Include assumptions, steps, risks, and next actions.',
        'Call out missing evidence instead of inventing operational details.'
    ),
    'chatFlowSurface',
    'knowledge'
)
WHERE tenant_id = 1 AND code = 'task_planning';

UPDATE ai_skill
SET metadata = metadata || jsonb_build_object(
    'promptRules',
    jsonb_build_array(
        'Draft a training reminder workflow or message from retrieved content.',
        'Do not claim a reminder was scheduled unless an approved tool result exists.',
        'Keep recipients, timing, and copy traceable to the retrieved context or mark them as assumptions.'
    ),
    'chatFlowSurface',
    'knowledge'
)
WHERE tenant_id = 1 AND code = 'training_reminder';

UPDATE ai_skill
SET metadata = metadata || jsonb_build_object(
    'promptRules',
    jsonb_build_array(
        'Answer conversationally while respecting knowledge-mode grounding and citation constraints.'
    ),
    'chatFlowSurface',
    'knowledge'
)
WHERE tenant_id = 1 AND code = 'general_chat';
