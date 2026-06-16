-- Customer service agent tool contracts.

INSERT INTO ai_tool
    (id, tenant_id, code, name, description, tool_kind, risk_level, approval_policy, permission_code, executor_kind, input_schema, output_schema, status, metadata, create_user, create_time)
VALUES
    (3211001, 1, 'faq.search', 'FAQ Search', 'Search tenant-scoped customer service FAQ or policy knowledge with citations.', 'function', 1, 1, 'ai:customer-service:read', 'dry_run',
     '{"type":"object","required":["query","datasetId"],"properties":{"query":{"type":"string"},"datasetId":{"type":"integer"},"limit":{"type":"integer","minimum":1,"maximum":10}}}'::jsonb,
     '{"type":"object","properties":{"answer":{"type":"string"},"hits":{"type":"array"},"citations":{"type":"array"}}}'::jsonb,
     1, '{"poc":true,"module":"customer-service","executor":"rag_adapter","risk":"low"}'::jsonb, 1, NOW()),
    (3211002, 1, 'customer.lookup', 'Customer Lookup', 'Read tenant-scoped customer context needed to answer a support request.', 'function', 2, 2, 'ai:customer-service:read', 'dry_run',
     '{"type":"object","properties":{"customerId":{"type":"string"},"externalKey":{"type":"string"}},"anyOf":[{"required":["customerId"]},{"required":["externalKey"]}]}'::jsonb,
     '{"type":"object","properties":{"customerId":{"type":"string"},"profile":{"type":"object"},"entitlements":{"type":"array"}}}'::jsonb,
     1, '{"poc":true,"module":"customer-service","executor":"customer_context_adapter","risk":"medium","hiddenFields":["pii","paymentMethod"]}'::jsonb, 1, NOW()),
    (3211003, 1, 'ticket.create', 'Create Support Ticket', 'Create an audited support ticket for a customer after policy approval.', 'function', 3, 3, 'ai:customer-service:ticket', 'dry_run',
     '{"type":"object","required":["customerId","title","description","priority"],"properties":{"customerId":{"type":"string"},"title":{"type":"string"},"description":{"type":"string"},"priority":{"type":"string","enum":["low","normal","high","urgent"]}}}'::jsonb,
     '{"type":"object","properties":{"ticketId":{"type":"string"},"status":{"type":"string"},"auditId":{"type":"string"}}}'::jsonb,
     1, '{"poc":true,"module":"customer-service","executor":"ticket_adapter","risk":"high","approval":"always"}'::jsonb, 1, NOW()),
    (3211004, 1, 'handoff.request', 'Request Human Handoff', 'Request a human support handoff with conversation summary and reason.', 'function', 3, 3, 'ai:customer-service:handoff', 'dry_run',
     '{"type":"object","required":["conversationId","reason","summary"],"properties":{"conversationId":{"type":"string"},"reason":{"type":"string"},"summary":{"type":"string"}}}'::jsonb,
     '{"type":"object","properties":{"handoffId":{"type":"string"},"status":{"type":"string"},"auditId":{"type":"string"}}}'::jsonb,
     1, '{"poc":true,"module":"customer-service","executor":"handoff_adapter","risk":"high","approval":"always"}'::jsonb, 1, NOW())
ON CONFLICT (tenant_id, code) DO UPDATE SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    tool_kind = EXCLUDED.tool_kind,
    risk_level = EXCLUDED.risk_level,
    approval_policy = EXCLUDED.approval_policy,
    permission_code = EXCLUDED.permission_code,
    executor_kind = EXCLUDED.executor_kind,
    input_schema = EXCLUDED.input_schema,
    output_schema = EXCLUDED.output_schema,
    status = EXCLUDED.status,
    metadata = EXCLUDED.metadata,
    update_user = EXCLUDED.create_user,
    update_time = NOW();
