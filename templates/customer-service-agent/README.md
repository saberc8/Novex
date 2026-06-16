# Customer Service Agent Template

Default template for grounded customer-service agent workflows.

M5 package:

- Manifest: `template.json`.
- Frontend entry: `apps/customer-service-agent`.
- Includes customer-service operator and supervisor roles, console/runs/knowledge pages, built-in agent tools, customer-service policy prompt, and a regression eval set.

## Frontend pages

| Code | Path | Permission |
| --- | --- | --- |
| `console` | `/customer-service` | `ai:customer-service:agent:run` |
| `runs` | `/customer-service/runs` | `ai:customer-service:agent:list` |
| `knowledge` | `/customer-service/knowledge` | `ai:customer-service:read` |

## Smoke checks

Script: `templates/customer-service-agent/smoke.sh`

| Code | Workdir | Command |
| --- | --- | --- |
| `customer_service_agent_api` | `backend` | `cargo test -p backend-rust customer_service_ --offline` |
| `customer_service_agent_frontend` | `apps/customer-service-agent` | `pnpm test` |
