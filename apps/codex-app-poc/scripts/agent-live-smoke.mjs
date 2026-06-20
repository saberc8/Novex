const DEFAULT_API_BASE_URL = "http://localhost:62601";
const DEFAULT_INPUT =
  "Answer directly with a short final answer: Novex configured model agent loop smoke.";
const TERMINAL_STATUSES = new Set(["succeeded", "failed", "cancelled", "waiting_approval"]);

export function smokeConfigFromEnv(env = process.env) {
  const enabled = env.NOVEX_LIVE_AGENT_SMOKE === "1";
  return {
    enabled,
    apiBaseUrl: normalizeBaseUrl(env.NEXT_PUBLIC_API_BASE_URL ?? DEFAULT_API_BASE_URL),
    modelRouteId: nonBlank(env.NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID),
    token: nonBlank(env.NOVEX_AGENT_SMOKE_TOKEN),
    input: nonBlank(env.NOVEX_AGENT_SMOKE_INPUT) ?? DEFAULT_INPUT,
    maxPolls: parseIntegerEnv(env.NOVEX_AGENT_SMOKE_MAX_POLLS, 60, {
      min: 1,
      name: "NOVEX_AGENT_SMOKE_MAX_POLLS"
    }),
    pollMs: parseIntegerEnv(env.NOVEX_AGENT_SMOKE_POLL_MS, 1000, {
      min: 0,
      name: "NOVEX_AGENT_SMOKE_POLL_MS"
    })
  };
}

export function agentRunPayload(config) {
  return {
    input: config.input.trim(),
    runtimeMode: "model_loop",
    autoApprove: false,
    ...(config.modelRouteId ? { modelRouteId: config.modelRouteId } : {}),
    budget: {
      maxSteps: 8,
      maxToolCalls: 1,
      maxSeconds: 60,
      maxCostCents: 0
    }
  };
}

export async function createAgentRun(fetchImpl, config) {
  return apiRequest(fetchImpl, config, "/ai/agents/runs", {
    method: "POST",
    headers: jsonHeaders(config),
    body: JSON.stringify(agentRunPayload(config))
  });
}

export async function listAgentEvents(fetchImpl, config, runId) {
  const url = new URL(`/ai/agents/runs/${runId}/events`, config.apiBaseUrl);
  url.searchParams.set("page", "1");
  url.searchParams.set("size", "100");
  const page = await apiRequest(fetchImpl, config, url, {
    method: "GET",
    headers: authHeaders(config)
  });
  return Array.isArray(page?.list) ? page.list : [];
}

export async function waitForAgentRunEvidence(fetchImpl, config, run) {
  let lastStatus = run.status ?? "unknown";
  let lastEvents = [];
  for (let attempt = 0; attempt < config.maxPolls; attempt += 1) {
    lastEvents = await listAgentEvents(fetchImpl, config, run.runId);
    lastStatus = latestStatus(run, lastEvents);
    if (TERMINAL_STATUSES.has(lastStatus)) {
      return assertAgentSmokeEvidence(config, { ...run, status: lastStatus }, lastEvents);
    }
    if (config.pollMs > 0) {
      await sleep(config.pollMs);
    }
  }

  throw new Error(
    `timed out waiting for Agent run ${run.runId}; lastStatus=${lastStatus}; events=${lastEvents.length}`
  );
}

export function assertAgentSmokeEvidence(config, run, events) {
  const inference = events.map(modelInferenceItemFromEvent).find(Boolean);
  if (!inference) {
    throw new Error(`Agent run ${run.runId} did not emit model_inference evidence`);
  }
  if (config.modelRouteId && inference.routeId !== config.modelRouteId) {
    throw new Error(
      `expected modelRouteId ${config.modelRouteId}, got ${inference.routeId ?? "unknown"}`
    );
  }
  if (run.status !== "succeeded") {
    throw new Error(`Agent run ${run.runId} finished with status ${run.status}`);
  }

  return {
    skipped: false,
    runId: run.runId,
    traceId: run.traceId,
    status: run.status,
    routeId: inference.routeId,
    provider: inference.provider,
    model: inference.model ?? null,
    eventCount: events.length
  };
}

export async function runAgentLiveSmoke({
  env = process.env,
  fetch: fetchImpl = globalThis.fetch,
  logger = console
} = {}) {
  const config = smokeConfigFromEnv(env);
  if (!config.enabled) {
    logger.log("Skipping Agent live smoke; set NOVEX_LIVE_AGENT_SMOKE=1 to run it.");
    return { skipped: true };
  }
  if (typeof fetchImpl !== "function") {
    throw new Error("global fetch is required to run Agent live smoke");
  }

  logger.log(`Creating Agent model-loop run at ${config.apiBaseUrl}`);
  const run = await createAgentRun(fetchImpl, config);
  logger.log(`Created Agent run ${run.runId}; polling events`);
  const result = await waitForAgentRunEvidence(fetchImpl, config, run);
  logger.log(
    `Agent live smoke passed: run=${result.runId} status=${result.status} route=${result.routeId}`
  );
  return result;
}

async function apiRequest(fetchImpl, config, pathOrUrl, init) {
  const url =
    pathOrUrl instanceof URL ? pathOrUrl.toString() : new URL(pathOrUrl, config.apiBaseUrl).toString();
  const response = await fetchImpl(url, init);
  const body = await response.json().catch(() => ({}));
  if (!response.ok || body.code !== "200") {
    throw new Error(body.msg ?? body.message ?? `Request failed with HTTP ${response.status}`);
  }
  return body.data;
}

function jsonHeaders(config) {
  return {
    ...authHeaders(config),
    "Content-Type": "application/json"
  };
}

function authHeaders(config) {
  return config.token ? { Authorization: `Bearer ${config.token}` } : {};
}

function latestStatus(run, events) {
  for (const event of [...events].reverse()) {
    if (typeof event?.status === "string" && event.status.trim()) {
      return event.status;
    }
  }
  return run.status ?? "unknown";
}

function modelInferenceItemFromEvent(event) {
  const item = event?.payload?.item;
  return item?.type === "model_inference" ? item : null;
}

function normalizeBaseUrl(value) {
  return value.trim().replace(/\/$/, "");
}

function nonBlank(value) {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

function parseIntegerEnv(value, fallback, { min, name }) {
  if (value === undefined || value === null || value === "") {
    return fallback;
  }
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed < min) {
    throw new Error(`${name} must be an integer >= ${min}`);
  }
  return parsed;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  runAgentLiveSmoke().catch((error) => {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  });
}
