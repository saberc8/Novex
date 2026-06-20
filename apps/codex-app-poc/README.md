# Codex App POC

Standalone web POC for a Codex-like developer agent workbench. The composer can call the Novex Agent API with `runtimeMode=model_loop` and an optional configured model route. It does not use OpenAI or Codex brand assets.

## Runtime Configuration

```bash
cp .env.example .env.local
```

`NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID` is optional. When set, the POC sends it as `modelRouteId` so the backend resolves that configured `CodeAgent` route. When blank, the backend uses the default tenant or environment model route.

## Live Smoke

Run this after starting the backend with database migrations and a configured `CodeAgent` model route:

```bash
NOVEX_LIVE_AGENT_SMOKE=1 \
NEXT_PUBLIC_API_BASE_URL=http://localhost:62601 \
NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID=runtime.llm \
NOVEX_AGENT_SMOKE_TOKEN=<jwt-if-needed> \
pnpm smoke:agent-live
```

The command creates a real `runtimeMode=model_loop` Agent run, polls `/ai/agents/runs/:id/events`, and fails unless the run reaches `succeeded` with a `model_inference` event. Without `NOVEX_LIVE_AGENT_SMOKE=1`, it exits 0 after printing a skip message.

## Commands

```bash
pnpm install
cp .env.example .env.local
pnpm dev
pnpm smoke:agent-live
pnpm test
pnpm typecheck
pnpm lint
pnpm build
```
