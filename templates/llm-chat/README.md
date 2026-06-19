# LLM Chat Template

Default template for pure model chat without a knowledge base.

M5 package:

- Manifest: `template.json`.
- Frontend entry: `apps/chat-web`.
- Includes default chat roles, menus, prompt, skill, branding, and smoke eval set.

## Frontend pages

| Code | Path | Permission |
| --- | --- | --- |
| `chat` | `/chat` | `app:chat:use` |
| `history` | `/chat/history` | `app:chat:use` |
| `share` | `/share/[token]` | `app:chat:use` |

## Smoke checks

Script: `templates/llm-chat/smoke.sh`

| Code | Workdir | Command |
| --- | --- | --- |
| `llm_chat_frontend` | `apps/chat-web` | `pnpm test` |
| `llm_chat_api` | `backend` | `cargo test -p backend model_service --offline` |
