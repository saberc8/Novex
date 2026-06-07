# Dynamic Model Routing Design

## Scope

This design implements the dynamic model routing path needed to make Novex data flow through the model control-plane tables instead of fixed environment-only routes.

The scope is intentionally narrow:

- resolve live model routes from `ai_model_route`, `ai_model_profile`, `ai_model_deployment`, and `ai_model_credential`;
- support the model purposes used by current POC flows: chat, RAG answer, embedding, rerank, and eval judge;
- keep `env:NAME` credentials as the only secret source in this iteration, so no plaintext keys are stored in the database;
- wire dynamic routes into model chat, chat-flow, RAG ingestion/query/answer, live RAG eval, runtime config, and health check;
- keep the existing environment route fallback when no active database route is available.

This does not implement Agent tool-loop model routing, media draw routing, MCP/plugin model routes, secret vault storage, or frontend configuration forms. Those remain later milestones.

## Current Gap

`novex-model` already has the core registry schema and route concepts. The backend also exposes a registry summary and records chat usage against the first matching `ai_model_route`.

However, actual runtime calls still resolve most routes with `ModelRuntimeConfig::from_env()`. That means:

- the active tenant does not decide which model route is used;
- changing `ai_model_route` rows does not change live calls;
- health check and runtime config do not reflect the current tenant's effective routes;
- RAG trace can show runtime route names, but not a route chosen through database resolution;
- eval live RAG reuses the RAG path but inherits the same environment-only route limitation.

## Approach

Add a database-backed resolver in `ModelRuntimeService`.

The resolver reads active model routes for a tenant and purpose, joins the route to profile, deployment, provider, and credential rows, then builds an executable `ModelRuntimeRoute`.

Resolution order:

1. Query the requested tenant for active routes matching the requested purpose.
2. Order by `priority ASC, id ASC`.
3. Use the first route whose profile/deployment/provider are active and whose credential can be resolved.
4. If no database route resolves, fall back to the existing environment route for the matching runtime target.

Credential handling:

- `credential_ref = env:LLM_API_KEY` reads `LLM_API_KEY` from process environment.
- Missing env credentials make that database route unusable.
- Plain credential values are not supported in this iteration.
- Responses and debug output continue to expose only masked credentials.

Route identity:

- Effective route IDs should use the database route code when a database route is selected, such as `runtime.llm.rag_answer`.
- Environment fallback keeps the existing route IDs such as `runtime.llm`, `runtime.embedding`, and `runtime.reranker`.
- Usage records should link to the selected database route when available.

## Data Flow

Model chat:

```text
HTTP /ai/models/chat
  -> ModelRuntimeService::for_tenant
  -> resolve purpose chat
  -> execute OpenAI-compatible chat request
  -> persist conversation, messages, usage, route id, model
```

Chat flow:

```text
ChatFlowService model mode
  -> ModelRuntimeService::for_tenant
  -> resolve purpose chat
  -> execute chat
  -> persist chat-flow message with selected route id
```

RAG:

```text
document ingestion
  -> resolve purpose embedding
  -> call embedding
  -> persist chunk embedding model route and vector collection shape

ask
  -> resolve purpose embedding for query vector
  -> Milvus search
  -> resolve purpose rerank
  -> call reranker
  -> resolve purpose rag_answer
  -> call LLM
  -> persist RAG trace with selected route ids
```

Eval:

```text
live_rag eval mode
  -> KnowledgeService::ask_dataset_for_tenant
  -> same dynamic RAG path
  -> eval result stores actual answer/citations from live route execution
```

Runtime config and health:

```text
/ai/models/runtime-config
  -> resolve effective routes for current tenant

/ai/models/health-check
  -> resolve effective routes for current tenant
  -> call configured target endpoint
  -> report route health without exposing secrets
```

## Error Handling

Normal mode keeps the current fallback behavior:

- Missing database route falls back to environment route.
- Missing optional embedding/rerank route keeps local fallback where the existing code already allows it.

Strict live RAG mode is stricter:

- embedding route must resolve and call successfully;
- rerank route must resolve and call successfully;
- RAG answer route must resolve and call successfully;
- Milvus must be configured and return matching chunks.

Database route resolution should fail a route, not the whole request, when a route has:

- inactive provider/profile/deployment/credential;
- unsupported credential reference;
- missing environment variable for `env:NAME`;
- missing endpoint, api path, or model name.

If all candidate database routes fail and no environment fallback exists, callers get the same user-facing missing-model error style used today.

## Testing

Unit tests:

- build a runtime route from joined registry rows and an `env:NAME` credential;
- choose the highest-priority active route for a purpose;
- ignore routes with missing credentials and fall back to env config;
- runtime config summary masks credentials and exposes database route codes;
- chat response and usage metadata use the selected route code;
- RAG trace accepts explicit dynamic route IDs.

Integration/live tests:

- extend the live RAG test fixture to seed model registry rows in its temporary database;
- give the DB rows route codes distinct from environment fallback route IDs;
- assert live RAG still calls embedding, Milvus, rerank, and LLM;
- assert persisted RAG trace route fields are the database route codes;
- add a live model chat smoke that resolves `chat` from database route rows and calls the real LLM.

## Acceptance Criteria

- Current offline test suites continue to pass.
- Runtime config and health check can be tenant-scoped.
- Model chat uses database route resolution when the route exists.
- RAG ingestion/query/rerank/answer use database route resolution when routes exist.
- Live RAG proves route selection by persisting database route codes in `ai_rag_trace`.
- No real secrets are committed or printed.
