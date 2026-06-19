# Chat Flow Knowledge Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the app-side Novex RAG loop: create/select a knowledge base, upload files for RAG parsing, and chat through persistent sessions backed by real knowledge retrieval or model chat.

**Architecture:** Add a first-class `ai_chat_flow_session` and `ai_chat_flow_message` layer instead of making the frontend stitch together `/ask` and `/models/chat`. Knowledge-mode messages call the existing `KnowledgeService::ask_dataset_for_tenant` path and persist the returned trace/citations into chat messages. Model-mode messages call the existing model runtime and persist under the same chat-flow contract.

**Tech Stack:** Rust + Axum + SQLx + Postgres on the backend; Next.js/React + TypeScript + Vitest on `apps/chat-web`; existing Novex RAG/model services and RBAC middleware.

---

### Task 1: Backend Chat-Flow Schema

**Files:**
- Create: `backend/migrations/202606060002_create_ai_chat_flow.sql`
- Create: `backend/migrations/202606060003_seed_ai_chat_flow_permissions.sql`
- Test: `backend/src/interfaces/http/ai/chat_flow.rs`

**Step 1: Write the failing migration/permission test**

Add a test in the new handler module that includes both migration files and asserts these strings exist:

```rust
#[test]
fn chat_flow_migrations_define_session_message_and_permissions() {
    let schema = include_str!("../../../../migrations/202606060002_create_ai_chat_flow.sql");
    let permissions = include_str!("../../../../migrations/202606060003_seed_ai_chat_flow_permissions.sql");

    for table in ["ai_chat_flow_session", "ai_chat_flow_message"] {
        assert!(schema.contains(table), "{table} missing from migration");
    }
    for field in ["dataset_id", "mode", "rag_trace_id", "citations", "message_count"] {
        assert!(schema.contains(field), "{field} missing from migration");
    }
    for permission in [
        "ai:chatFlow:list",
        "ai:chatFlow:create",
        "ai:chatFlow:message",
    ] {
        assert!(permissions.contains(permission), "{permission} missing from seed");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p backend --offline chat_flow_migrations_define_session_message_and_permissions`

Expected: fail because `chat_flow.rs` and migration files do not exist yet.

**Step 3: Write minimal implementation**

Create `202606060002_create_ai_chat_flow.sql` with:

```sql
CREATE TABLE IF NOT EXISTS ai_chat_flow_session (
    id                   BIGINT       NOT NULL,
    tenant_id            BIGINT       NOT NULL DEFAULT 1,
    app_code             VARCHAR(64)  NOT NULL DEFAULT 'chat-web',
    mode                 VARCHAR(32)  NOT NULL DEFAULT 'knowledge',
    dataset_id           BIGINT       DEFAULT NULL,
    title                VARCHAR(160) NOT NULL DEFAULT '',
    status               SMALLINT     NOT NULL DEFAULT 1,
    route_id             VARCHAR(128) DEFAULT NULL,
    model                VARCHAR(128) DEFAULT NULL,
    message_count        INTEGER      NOT NULL DEFAULT 0,
    last_message_preview TEXT         NOT NULL DEFAULT '',
    metadata             JSONB        NOT NULL DEFAULT '{}'::jsonb,
    create_user          BIGINT       NOT NULL,
    create_time          TIMESTAMP    NOT NULL,
    update_user          BIGINT       DEFAULT NULL,
    update_time          TIMESTAMP    DEFAULT NULL,
    PRIMARY KEY (id)
);
```

Add indexes for tenant, create_user, dataset, mode, and update time. Create `ai_chat_flow_message` with `session_id`, `role`, `content`, `route_id`, `model`, `rag_trace_id`, `citations JSONB`, `token_count`, `metadata`, and create fields.

Create `202606060003_seed_ai_chat_flow_permissions.sql` following the existing `202606050019_seed_ai_model_chat_permission.sql` menu-permission seed style.

**Step 4: Run test to verify it passes**

Run: `cargo test -p backend --offline chat_flow_migrations_define_session_message_and_permissions`

Expected: pass.

**Step 5: Commit**

```bash
git add backend/migrations/202606060002_create_ai_chat_flow.sql backend/migrations/202606060003_seed_ai_chat_flow_permissions.sql backend/src/interfaces/http/ai/chat_flow.rs
git commit -m "feat: add chat flow schema"
```

### Task 2: Chat-Flow Repository

**Files:**
- Create: `backend/src/infrastructure/persistence/ai_chat_flow_repository.rs`
- Modify: `backend/src/infrastructure/persistence/mod.rs`
- Test: `backend/src/infrastructure/persistence/ai_chat_flow_repository.rs`

**Step 1: Write failing repository tests**

Add pure unit tests for save-record builders and metadata serialization:

```rust
#[test]
fn chat_flow_message_record_keeps_rag_trace_and_citations() {
    let citations = serde_json::json!([
        {"documentId":"20","chunkId":"20:0","pageNo":3,"sectionPath":["Policy"]}
    ]);
    let record = ChatFlowMessageSaveRecord {
        id: 1,
        tenant_id: 1,
        session_id: 2,
        role: "assistant".to_owned(),
        content: "Use the handbook.".to_owned(),
        route_id: Some("local-extractive".to_owned()),
        model: None,
        rag_trace_id: Some(42),
        citations: citations.clone(),
        token_count: 3,
        metadata: serde_json::json!({"answerStrategy":"extractive"}),
        user_id: 7,
        now: chrono::NaiveDate::from_ymd_opt(2026, 6, 6).unwrap().and_hms_opt(1, 2, 3).unwrap(),
    };

    assert_eq!(record.rag_trace_id, Some(42));
    assert_eq!(record.citations, citations);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p backend --offline chat_flow_message_record_keeps_rag_trace_and_citations`

Expected: fail because repository types do not exist.

**Step 3: Write minimal implementation**

Create repository methods:

- `create_session(&ChatFlowSessionSaveRecord)`
- `list_sessions(&ChatFlowSessionFilter)`
- `get_session(&ChatFlowSessionFilter)`
- `list_messages(tenant_id, session_id)`
- `append_turn(&ChatFlowSessionUpdateRecord, &[ChatFlowMessageSaveRecord])`

Use one SQL transaction for `append_turn`: insert user/assistant rows, increment session message count, update route/model/preview/update fields.

**Step 4: Run test to verify it passes**

Run: `cargo test -p backend --offline chat_flow_message_record_keeps_rag_trace_and_citations`

Expected: pass.

**Step 5: Commit**

```bash
git add backend/src/infrastructure/persistence/ai_chat_flow_repository.rs backend/src/infrastructure/persistence/mod.rs
git commit -m "feat: add chat flow repository"
```

### Task 3: Chat-Flow Service

**Files:**
- Create: `backend/src/application/ai/chat_flow_service.rs`
- Modify: `backend/src/application/ai/mod.rs`
- Test: `backend/src/application/ai/chat_flow_service.rs`

**Step 1: Write failing service tests**

Add tests for command normalization:

```rust
#[test]
fn create_session_requires_valid_knowledge_dataset() {
    let err = normalize_chat_flow_session_command(ChatFlowSessionCommand {
        mode: "knowledge".to_owned(),
        dataset_id: None,
        title: "Policy".to_owned(),
    })
    .unwrap_err();

    assert!(err.to_string().contains("知识库"));
}

#[test]
fn send_message_trims_content_and_clamps_limit() {
    let command = normalize_chat_flow_message_command(ChatFlowMessageCommand {
        content: "  哪个制度有效？  ".to_owned(),
        limit: Some(50),
        ..ChatFlowMessageCommand::default()
    })
    .unwrap();

    assert_eq!(command.content, "哪个制度有效？");
    assert_eq!(command.limit, 10);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p backend --offline chat_flow`

Expected: fail because service module does not exist.

**Step 3: Write minimal implementation**

Implement:

- `ChatFlowSessionCommand`
- `ChatFlowMessageCommand`
- `ChatFlowSessionResp`
- `ChatFlowMessageResp`
- `ChatFlowSendMessageResp`
- `ChatFlowService::create_session`
- `ChatFlowService::list_sessions`
- `ChatFlowService::list_messages`
- `ChatFlowService::send_message`

Knowledge send path:

```rust
let rag = knowledge_service
    .ask_dataset_for_tenant(tenant_id, user_id, dataset_id, RagAskCommand {
        question: command.content.clone(),
        limit: command.limit,
    })
    .await?;
```

Then persist a user row and assistant row. Assistant metadata includes `mode`, `datasetId`, `ragTraceId`, `answerStrategy`, `retrievalHitCount`, and `source: "ai.chatFlow.knowledge"`.

Model send path builds `ModelChatCommand` from message content and file contexts, then calls existing model runtime. Persist the returned answer with route/model/usage metadata.

**Step 4: Run test to verify it passes**

Run: `cargo test -p backend --offline chat_flow`

Expected: service tests pass.

**Step 5: Commit**

```bash
git add backend/src/application/ai/chat_flow_service.rs backend/src/application/ai/mod.rs
git commit -m "feat: add chat flow service"
```

### Task 4: Chat-Flow HTTP Routes

**Files:**
- Create: `backend/src/interfaces/http/ai/chat_flow.rs`
- Modify: `backend/src/interfaces/http/ai/mod.rs`
- Test: `backend/src/interfaces/http/ai/chat_flow.rs`

**Step 1: Write failing handler tests**

Add permission-first tests:

```rust
#[tokio::test]
async fn create_chat_flow_session_rejects_missing_permission() {
    let err = create_session(
        State(test_state()),
        user_with_permissions(vec![]),
        axum::Json(ChatFlowSessionCommand {
            mode: "knowledge".to_owned(),
            dataset_id: Some(10),
            title: "Policy".to_owned(),
        }),
    )
    .await
    .unwrap_err();

    assert!(matches!(err, AppError::Forbidden));
}
```

Add route-registration test that `/ai/chat-flow/sessions` requires auth.

**Step 2: Run test to verify it fails**

Run: `cargo test -p backend --offline chat_flow_session_rejects_missing_permission`

Expected: fail until handlers are wired.

**Step 3: Write minimal implementation**

Routes:

- `POST /ai/chat-flow/sessions`
- `GET /ai/chat-flow/sessions`
- `GET /ai/chat-flow/sessions/:session_id/messages`
- `POST /ai/chat-flow/sessions/:session_id/messages`

Permissions:

- `ai:chatFlow:create`
- `ai:chatFlow:list`
- `ai:chatFlow:message`

Use `current_user.tenant_id` everywhere.

**Step 4: Run test to verify it passes**

Run: `cargo test -p backend --offline chat_flow`

Expected: handler and service tests pass.

**Step 5: Commit**

```bash
git add backend/src/interfaces/http/ai/chat_flow.rs backend/src/interfaces/http/ai/mod.rs
git commit -m "feat: expose chat flow APIs"
```

### Task 5: Chat-Web API and Types

**Files:**
- Create: `apps/chat-web/src/types/chat-flow.ts`
- Create: `apps/chat-web/src/api/chat-flow.ts`
- Modify: `apps/chat-web/src/types/knowledge.ts`
- Modify: `apps/chat-web/src/api/knowledge.ts`
- Test: `apps/chat-web/app/page.test.tsx`

**Step 1: Write failing frontend tests**

Update mocks to expect chat-flow calls:

```ts
expect(createChatFlowSessionMock).toHaveBeenCalledWith({
  mode: "knowledge",
  datasetId: 10,
  title: "企业制度知识库"
});

expect(sendChatFlowMessageMock).toHaveBeenCalledWith(501, {
  content: "Which handbook should I use?",
  limit: 5
});
```

Add upload mock expectation:

```ts
expect(uploadKnowledgeFileMock).toHaveBeenCalledWith(10, file);
expect(getParseJobMock).toHaveBeenCalledWith(10, 77);
```

**Step 2: Run test to verify it fails**

Run: `pnpm --filter @novex/chat-web test -- --run`

Expected: fail because `@/api/chat-flow` and upload APIs do not exist.

**Step 3: Write minimal implementation**

Add chat-flow API methods:

- `createChatFlowSession`
- `listChatFlowSessions`
- `listChatFlowMessages`
- `sendChatFlowMessage`

Extend knowledge API:

- `createDataset`
- `uploadKnowledgeFile`
- `getParseJob`
- `listDocuments`

For upload, use `FormData` and pass it through the existing API helper without JSON encoding.

**Step 4: Run test to verify it passes**

Run: `pnpm --filter @novex/chat-web test -- --run`

Expected: frontend tests pass.

**Step 5: Commit**

```bash
git add apps/chat-web/src/types/chat-flow.ts apps/chat-web/src/api/chat-flow.ts apps/chat-web/src/types/knowledge.ts apps/chat-web/src/api/knowledge.ts apps/chat-web/app/page.test.tsx
git commit -m "feat: add chat web chat flow clients"
```

### Task 6: Chat-Web Product Flow

**Files:**
- Modify: `apps/chat-web/src/app-client.tsx`
- Test: `apps/chat-web/app/page.test.tsx`

**Step 1: Write failing UI tests**

Test behaviors:

- User can create a dataset from chat-web.
- User can upload a file to selected dataset.
- UI polls parse job and shows indexed/failed state.
- Sending a knowledge question creates or reuses a chat-flow session.
- Citations come from the chat-flow assistant message, not direct `/ask`.
- Pure model mode also uses chat-flow send.

**Step 2: Run test to verify it fails**

Run: `pnpm --filter @novex/chat-web test -- --run`

Expected: fail because the page still calls `askDataset` and `chatCompletion` directly.

**Step 3: Write minimal implementation**

Refactor state around:

- `selectedDatasetId`
- `activeKnowledgeSessionId`
- `activeModelSessionId`
- `messages`
- `uploading`
- `parseJobs`

Replace direct knowledge ask with:

```ts
const session = activeKnowledgeSessionId
  ? currentSession
  : await createChatFlowSession({ mode: "knowledge", datasetId: selectedDataset.id, title: selectedDataset.name });
const response = await sendChatFlowMessage(session.id, { content: trimmed, limit: 5 });
```

Render assistant citations from `response.assistantMessage.citations`.

**Step 4: Run test to verify it passes**

Run: `pnpm --filter @novex/chat-web test -- --run`

Expected: frontend tests pass.

**Step 5: Commit**

```bash
git add apps/chat-web/src/app-client.tsx apps/chat-web/app/page.test.tsx
git commit -m "feat: wire chat web rag workflow"
```

### Task 7: API Smoke and Full Verification

**Files:**
- Modify as needed only if smoke exposes defects.

**Step 1: Run backend focused tests**

Run: `cargo test -p backend --offline chat_flow`

Expected: pass.

**Step 2: Run backend full tests**

Run: `cargo test -p backend --offline`

Expected: pass.

**Step 3: Run chat-web tests and typecheck**

Run:

```bash
pnpm --filter @novex/chat-web test -- --run
pnpm --filter @novex/chat-web typecheck
```

Expected: pass.

**Step 4: Run API smoke without browser visual debugging**

Start backend on a free local port, apply migrations, login, then call:

```bash
curl -sS -X POST "$API/ai/knowledge/datasets" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"App RAG Smoke","description":"uploaded from chat-web","visibility":1,"retrievalMode":3}'

curl -sS -X POST "$API/ai/knowledge/datasets/$DATASET_ID/documents/files" \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@/tmp/novex-rag-smoke.md"

curl -sS "$API/ai/knowledge/datasets/$DATASET_ID/parse-jobs/$JOB_ID" \
  -H "Authorization: Bearer $TOKEN"

curl -sS -X POST "$API/ai/chat-flow/sessions" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"mode\":\"knowledge\",\"datasetId\":$DATASET_ID,\"title\":\"App RAG Smoke\"}"

curl -sS -X POST "$API/ai/chat-flow/sessions/$SESSION_ID/messages" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"content":"Which document should I use?","limit":5}'
```

Expected: upload creates a parser job, parser job reaches a terminal status, chat-flow response includes `assistantMessage`, `ragTraceId`, and `citations`.

**Step 5: Final commit**

```bash
git status --short
git commit -m "feat: complete chat flow knowledge loop"
```

Only commit files touched by this plan and the already accepted prior Novex AI changes. Do not revert unrelated user work.
