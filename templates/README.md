# Templates

Customer delivery templates for reusable Novex app packages.

M5 turns the template folder into package artifacts. Each template directory contains a
`template.json` manifest with:

- Tenant and branding defaults.
- Default roles and menus.
- Prompt and skill setup.
- Tool, connector, plugin, and trigger configuration.
- Eval set metadata.
- Frontend entry and deployment checklist.

Default packages:

- `llm_chat`: pure model chat.
- `knowledge_base_chat`: RAG chat with citations.
- `agent_workspace`: tool-using Agent workspace.
- `training_app`: employee training POC with knowledge, quiz, reminders, and eval.

These manifests are embedded by the backend M5 template API and can also be copied as
customer delivery artifacts.
