# Novex Training Web

Customer-facing employee training application template for the Novex POC.

Scope:

- Knowledge Q&A, quiz delivery, learning record views, and notification entry points.
- Uses Novex auth, model routes, knowledge resources, skills, tools, and eval data.

Status:

- Next.js app scaffold on port `4401`.
- POC navigation and template pages: learning, ask, quiz, records, notifications.
- Live knowledge Q&A uses `/ai/knowledge/datasets` and `/ai/knowledge/datasets/:id/ask`.
- Unauthenticated or offline use falls back to bundled demo data.
- Admin remains the control plane; this app is the customer-facing workspace.

Commands:

```bash
pnpm install
NEXT_PUBLIC_API_BASE_URL=http://localhost:4398 pnpm dev
pnpm typecheck
pnpm lint
pnpm test
pnpm build
```
