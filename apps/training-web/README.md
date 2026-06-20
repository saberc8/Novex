# Novex Training Web

Customer-facing employee training application template for the Novex POC.

Scope:

- Knowledge Q&A, quiz delivery, learning record views, and notification entry points.
- Uses Novex auth, model routes, knowledge resources, skills, tools, and eval data.

Status:

- Next.js app scaffold on port `62603`.
- POC navigation and template pages: learning, ask, quiz, records, notifications.
- Live knowledge Q&A uses `/ai/knowledge/datasets` and `/ai/knowledge/datasets/:id/ask`.
- Training material upload uses `/ai/knowledge/datasets/:id/documents/files`, which stores the source file and creates a parser job for the selected dataset.
- Unauthenticated or offline use falls back to bundled demo data.
- Admin remains the control plane; this app is the customer-facing workspace.

Commands:

```bash
pnpm install
cp .env.example .env.local
pnpm dev
pnpm typecheck
pnpm lint
pnpm test
pnpm build
```
