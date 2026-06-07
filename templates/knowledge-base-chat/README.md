# Knowledge Base Chat Template

Default template for RAG Q&A with citations.

M5 package:

- Manifest: `template.json`.
- Frontend entry: `apps/chat-web`.
- Includes default knowledge roles, menus, citation prompt, web import connector, and RAG regression eval set.

## Frontend pages

| Code | Path | Permission |
| --- | --- | --- |
| `ask` | `/knowledge` | `app:knowledge:ask` |
| `sources` | `/knowledge/sources` | `ai:knowledge:list` |
| `share` | `/share/[token]` | `app:knowledge:ask` |

## Smoke checks

Script: `templates/knowledge-base-chat/smoke.sh`

| Code | Workdir | Command |
| --- | --- | --- |
| `chat_web_frontend` | `apps/chat-web` | `pnpm test` |
| `knowledge_base_api` | `backend` | `cargo test -p backend-rust knowledge --offline` |
