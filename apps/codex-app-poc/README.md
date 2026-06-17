# Codex App POC

Standalone web POC for a Codex-like developer agent workbench. The composer can call the Novex Agent API with `runtimeMode=model_loop` and an optional configured model route. It does not use OpenAI or Codex brand assets.

## Runtime Configuration

```bash
NEXT_PUBLIC_API_BASE_URL=http://localhost:4398
NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID=runtime.llm
```

`NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID` is optional. When set, the POC sends it as `modelRouteId` so the backend resolves that configured `CodeAgent` route. When blank, the backend uses the default tenant or environment model route.

## Commands

```bash
pnpm install
pnpm dev
pnpm test
pnpm typecheck
pnpm lint
pnpm build
```
