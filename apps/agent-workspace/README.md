# Agent Workspace

Customer-facing Agent workspace application template.

Scope:

- Agent runs, tool execution visibility, approvals, trace views, and task history.
- Uses Novex Run Graph, Tool Registry, MCP, connectors, memory, and eval surfaces.

M3 status: Next.js workspace scaffolded. It provides a customer-facing run surface for
creating Agent runs, inspecting workflow events, approving paused runs, cancelling
active runs, reviewing tool permissions, and seeing final output.

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
