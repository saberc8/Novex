# Templates

Customer delivery templates for reusable Novex app packages.

M5 turns the template folder into package artifacts. Each template directory contains a
`template.json` manifest with:

- Tenant and branding defaults.
- Default roles and menus.
- Prompt and skill setup.
- Tool, connector, plugin, and trigger configuration.
- Eval set metadata.
- Frontend entry, frontend app id, C-side page list, smoke checks, and deployment checklist.
- `smoke.sh` script that runs the template smoke checks from the repository root.

Default packages:

- `llm_chat`: pure model chat.
- `knowledge_base_chat`: RAG chat with citations.
- `agent_workspace`: tool-using Agent workspace.
- `training_app`: employee training POC with `apps/training-web`, knowledge Q&A, quiz, reminders, and eval.

These manifests are embedded by the backend M5 template API and can also be copied as
customer delivery artifacts.
