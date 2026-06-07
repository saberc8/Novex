# Agent Workspace Template

Default template for tool-using Agent workflows.

M5 package:

- Manifest: `template.json`.
- Frontend entry: `apps/agent-workspace`.
- Includes default Agent roles, approval menu, GitHub and Feishu connector config, built-in tool plugin, webhook trigger, and Agent eval set.

## Frontend pages

| Code | Path | Permission |
| --- | --- | --- |
| `workspace` | `/agent` | `ai:agent:run` |
| `approvals` | `/agent/approvals` | `ai:agent:resume` |
| `traces` | `/agent/traces` | `ai:trace:list` |

## Smoke checks

Script: `templates/agent-workspace/smoke.sh`

| Code | Workdir | Command |
| --- | --- | --- |
| `agent_runtime_api` | `backend` | `cargo test -p backend-rust agent --offline` |
| `agent_workspace_frontend` | `apps/agent-workspace` | `pnpm test` |
