# Customer Service Agent Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build an enterprise customer-service agent template that answers with grounded citations, checks customer context under RBAC, creates handoff/ticket actions under approval policy, and produces eval records for service quality.

**Architecture:** Customer service is a packaged agent flow on top of Run Graph, KnowledgeService, Agent Runtime, tools, eval, and templates. It must not fork a separate chatbot engine. Read-only FAQ/customer lookup tools are low/medium risk; ticket creation, handoff, and outbound messages are high risk or approval-gated depending on tenant policy.

**Tech Stack:** Rust/Axum/SQLx, existing `chat_flow_service`, `agent_service`, `knowledge_service`, `customer_service_agent`, `novex-eval`, and `novex-tools`.

---

## Scope

In scope:

- Customer service flow template and route-level contract.
- FAQ/RAG answer with citation policy.
- Customer lookup tool contract with tenant permission.
- Handoff and ticket tool contracts with approval/audit.
- Eval dataset seeds for answer quality, citation correctness, and handoff accuracy.

Out of scope:

- Full CRM integration.
- Omnichannel live chat UI.
- Human agent console.
- Payment/refund mutations.

## Task 1: Define Customer Service Domain and Tool Contracts

**Files:**
- Modify: `crates/novex-tools/src/lib.rs`
- Modify: `backend/src/application/ai/capability_service.rs`
- Create: `backend/migrations/202606160005_seed_customer_service_tools.sql`

**Step 1: Write failing tests**

Add:

```rust
#[test]
fn customer_service_tools_have_risk_and_schema_contracts() {
    let tools = customer_service_tool_definitions();

    assert!(tools.iter().any(|tool| tool.code == "faq.search"));
    assert!(tools.iter().any(|tool| tool.code == "customer.lookup"));
    assert!(tools.iter().any(|tool| tool.code == "ticket.create"));
    assert!(tools.iter().any(|tool| tool.code == "handoff.request"));
    assert_eq!(tools.iter().find(|tool| tool.code == "ticket.create").unwrap().risk_level, ToolRiskLevel::High);
}
```

**Step 2: Run failing test**

Run:

```bash
cargo test -p novex-tools customer_service_tools_have_risk_and_schema_contracts --offline
```

Expected: FAIL.

**Step 3: Implement tool definitions**

Add customer-service tool definitions:

- `faq.search`: query, datasetId, limit.
- `customer.lookup`: customerId or externalKey.
- `ticket.create`: customerId, title, description, priority.
- `handoff.request`: conversationId, reason, summary.

**Step 4: Seed tools**

Ensure `ai_tool` seeds include these tools with permission codes:

- `ai:customer-service:read`
- `ai:customer-service:ticket`
- `ai:customer-service:handoff`

**Step 5: Verify**

Run:

```bash
cargo test -p novex-tools --offline
cargo test -p backend customer_service_tool --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add crates/novex-tools backend/src/application/ai/capability_service.rs backend/migrations/202606160005_seed_customer_service_tools.sql
git commit -m "feat: define customer service tool contracts"
```

## Task 2: Add Customer Service Agent Flow Service

**Files:**
- Add: `backend/src/application/ai/customer_service_agent.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Modify: `backend/src/application/ai/agent_service.rs`

**Step 1: Write failing tests**

Add tests:

- `customer_service_prompt_requires_citations_or_insufficient_evidence`
- `customer_service_flow_routes_faq_question_to_rag_search`
- `customer_service_flow_requires_approval_for_ticket_create`

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend customer_service_prompt_requires_citations_or_insufficient_evidence --offline
```

Expected: FAIL.

**Step 3: Implement prompt and run command adapter**

Add:

- `CustomerServiceAgentCommand`
- `CustomerServicePolicy`
- `build_customer_service_system_prompt`
- `create_customer_service_run`

The adapter should call existing `AgentService::create_run` with:

```json
{
  "runtimeMode": "model_loop",
  "budget": {"maxSteps": 8, "maxToolCalls": 1, "maxSeconds": 60, "maxCostCents": 0}
}
```

Prompt requirements:

- cite retrieved FAQ/knowledge chunks.
- say insufficient evidence when no citation supports the answer.
- do not create ticket/handoff without tool policy.
- summarize customer context without leaking hidden fields.

**Step 4: Verify**

Run:

```bash
cargo test -p backend customer_service_flow --offline
cargo test -p backend agent_service --offline
```

Expected: PASS.

**Step 5: Commit**

```bash
git add backend/src/application/ai/customer_service_agent.rs backend/src/application/ai/mod.rs backend/src/application/ai/agent_service.rs
git commit -m "feat: add customer service agent flow"
```

## Task 3: Expose Customer Service API and Template

**Files:**
- Add: `backend/src/interfaces/http/ai/customer_service.rs`
- Modify: `backend/src/interfaces/http/ai/mod.rs`
- Modify: `backend/src/application/ai/customer_service_agent.rs`
- Create: `backend/migrations/202606160006_seed_customer_service_template.sql`

**Step 1: Write failing route/template tests**

Add:

- `customer_service_agent_route_is_registered_and_requires_auth`
- `customer_service_template_seed_contains_agent_flow`
- `customer_service_handler_uses_tenant_bound_runtime`

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend customer_service_agent_route_is_registered_and_requires_auth --offline
```

Expected: FAIL.

**Step 3: Implement route**

Add:

- `POST /ai/customer-service/agent/runs`
- `GET /ai/customer-service/agent/runs/:runId`
- `GET /ai/customer-service/agent/runs/:runId/events`

The create route wraps `CustomerServiceAgentCommand`.

**Step 4: Add delivery template**

Seed a template:

- code: `customer-service-agent-poc`
- includes required permissions.
- includes knowledge dataset dependency.
- includes eval dataset dependency.

**Step 5: Verify**

Run:

```bash
cargo test -p backend customer_service_ --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add backend/src/interfaces/http/ai/customer_service.rs backend/src/interfaces/http/ai/mod.rs backend/src/application/ai/customer_service_agent.rs backend/migrations/202606160006_seed_customer_service_template.sql
git commit -m "feat: expose customer service agent template"
```

## Task 4: Add Customer Service Eval Dataset and Gate

**Files:**
- Modify: `backend/src/application/ai/eval_service.rs`
- Create: `backend/migrations/202606160007_seed_customer_service_eval.sql`
- Modify: `crates/novex-eval/src/lib.rs`

**Step 1: Write failing tests**

Add:

- `customer_service_eval_seed_contains_resolution_and_handoff_cases`
- `customer_service_eval_scores_citation_and_handoff_accuracy`
- `customer_service_eval_report_flags_missing_evidence`

**Step 2: Run failing test**

Run:

```bash
cargo test -p backend customer_service_eval_seed_contains_resolution_and_handoff_cases --offline
```

Expected: FAIL.

**Step 3: Seed eval dataset**

Add dataset:

- code: `customer-service-agent-regression`
- cases:
  - FAQ answer with citation.
  - insufficient evidence response.
  - handoff required.
  - ticket creation requires approval.

**Step 4: Add scoring helper**

Extend `novex-eval` with:

- `EvalTargetKind::CustomerService`
- metric helpers for `HandoffAccuracy` and `GroundedResolution`.

**Step 5: Verify**

Run:

```bash
cargo test -p novex-eval --offline
cargo test -p backend customer_service_eval --offline
```

Expected: PASS.

**Step 6: Commit**

```bash
git add crates/novex-eval backend/src/application/ai/eval_service.rs backend/migrations/202606160007_seed_customer_service_eval.sql
git commit -m "feat: add customer service eval gate"
```

## Task 5: Full Verification

Run:

```bash
cargo fmt -- --check
cargo test --workspace --offline
```

Expected: PASS.

Acceptance is met only when a customer-service run can answer with citations, request handoff or ticket creation under policy, and produce eval records proving answer quality and handoff accuracy.
