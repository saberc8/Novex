# Rust Admin Backend

## Default Account

The seed migrations create `admin/admin123` for local development. Change this password outside local development before exposing the service.

## Local Env

Backend-owned local configuration lives in `backend/.env.example`. Copy it to `backend/.env` for backend-only development, or use the root `.env` when running the full POC through `scripts/run-poc.sh`.

```bash
cp backend/.env.example backend/.env
(cd backend && set -a && . .env && set +a && cargo run)
```

## Migration Smoke Checklist

Migration SQL is authored under `backend/migration_sources` and synchronized into the
flat `backend/migrations` directory that SQLx consumes.

```bash
scripts/sync-migration-sources.sh
scripts/sync-migration-sources.sh --check
```

Run migrations against a local PostgreSQL database:

```bash
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:15432/novex sqlx migrate run
```

Check core tables:

```sql
select to_regclass('public.sys_user');
select to_regclass('public.sys_role');
select to_regclass('public.sys_menu');
```

Expected result: each query returns its table name.

## Milvus RAG Search

Knowledge-base ingestion keeps PostgreSQL as the source of truth for chunk text, citations, permissions, and trace metadata. When Milvus is configured, indexed chunks are also upserted to the collection stored in `ai_vector_collection.provider_collection`. Knowledge-base ask then sends vector recall to the same collection, and returned `chunk_uid` values are mapped back to PostgreSQL chunks before answer construction.

```bash
MILVUS_ENDPOINT=http://127.0.0.1:19540
MILVUS_TOKEN=root:Milvus # optional; NOVEX_MILVUS_TOKEN is also accepted
```

`NOVEX_MILVUS_ENDPOINT` can be used instead of `MILVUS_ENDPOINT`. The POC Milvus schema expects `id`, `chunk_db_id`, `tenant_id`, `dataset_id`, `document_id`, `chunk_uid`, `chunk_index`, `embedding`, `semantic_search_text`, `segment_type`, and `content_role` fields. If no endpoint is configured, the collection is not ready, the embedding dimension does not match, or Milvus returns no usable hits, the backend keeps the local hybrid retrieval fallback.

## Feishu Message Tool

Agent runs that select `feishu.message.send` still require approval unless `autoApprove=true` or the run is resumed after approval. When a webhook is configured, the agent posts a Feishu custom bot text message and records the provider response in tool audit. Without a webhook, the tool remains a dry-run and does not send external traffic.

```bash
FEISHU_WEBHOOK_URL=https://open.feishu.cn/open-apis/bot/v2/hook/...
```

`NOVEX_FEISHU_WEBHOOK_URL` can be used instead of `FEISHU_WEBHOOK_URL`.

## GitHub Identity Provider

GitHub OAuth login is exposed through `/auth/oauth/github.login/authorize` and `/auth/oauth/github.login/callback`. The callback only logs in a user when the GitHub profile is already bound in `sys_external_account`; it does not create users automatically and does not grant repository access.

```bash
GITHUB_OAUTH_CLIENT_ID=Iv1...
GITHUB_OAUTH_CLIENT_SECRET=...
```

`NOVEX_GITHUB_OAUTH_CLIENT_ID` and `NOVEX_GITHUB_OAUTH_CLIENT_SECRET` can be used instead. The OAuth state is stored as a SHA-256 hash in `sys_oauth_state`, expires after 15 minutes, and is consumed once.

## GitHub Connector Tools

GitHub login remains under identity provider tables (`sys_identity_provider`, `sys_external_account`, `sys_oauth_state`). Repository access uses connector credentials and is represented by `ai_connector_credential`; the POC runtime resolves the active user/tenant connector credential first and only falls back to the environment token when no connector credential is configured. The seed credential for `github.default` points at `env:GITHUB_CONNECTOR_TOKEN`.

```bash
GITHUB_CONNECTOR_TOKEN=github_pat_...
GITHUB_API_BASE_URL=https://api.github.com # optional
```

`NOVEX_GITHUB_CONNECTOR_TOKEN` and `NOVEX_GITHUB_API_BASE_URL` can be used as direct runtime fallbacks. Agent/tool inputs for `github.repo.search` require `repository` and `query`; `github.repo.read` requires `repository` and `path`, with optional `ref`.

Agent natural-language inputs can also use simple forms such as `search GitHub repo acme/app for parser worker under src` or `read GitHub file acme/app src/lib.rs ref main`; the agent extracts repository, query/path, and ref before executing the connector tool.

Admin can inspect and configure connector credential bindings through `GET /ai/capabilities/connectors/credentials` and `POST /ai/capabilities/connectors/credentials`. Runtime secret material can be registered through `GET /system/secrets` and `POST /system/secrets`; those endpoints store POC sealed ciphertext in `sys_secret` and return masked values only. Connector bindings may still point at scoped `env:` references for local fallback compatibility, and the Admin connectors page displays masked values instead of raw token material.

## Media Image Tool

Agent runs that select `media.image.generate` require the same medium-risk approval flow as other external tools. When the draw route is configured, the agent posts the image prompt to `RIGHT_CODE_DRAW_BASE_URL`, stores a media job in `ai_media_job`, and stores the returned image URL as an `ai_media_asset`. Without the draw route, the tool remains a dry-run and still records the tool audit.

```bash
RIGHT_CODE_DRAW_BASE_URL=https://www.right.codes/draw
RIGHT_CODE_DRAW_API_KEY=...
```

## API Error Contract

The Rust backend keeps the existing Avalon/Vue-compatible response envelope for API compatibility:

```json
{
  "code": "403",
  "data": null,
  "msg": "没有访问权限，请联系管理员授权",
  "success": false,
  "timestamp": "1780057589045"
}
```

Application errors, including unauthorized and forbidden responses, are returned with HTTP 200 and a non-`200` business `code`. Frontends must check `code` and `success`, not only the HTTP status.

This compatibility rule applies to JSON APIs. File/download endpoints may still use HTTP status for transport-level failures.
