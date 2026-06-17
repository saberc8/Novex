import { apiRequest, apiUrl } from "@/lib/api";
import { getAuthToken } from "@/lib/auth";
import type {
  AgentRunCommand,
  AgentRunEventQuery,
  AgentRunEventResp,
  AgentRunEventStreamQuery,
  AgentRunResp,
  PageResult
} from "@/types/agent";

const AGENT_RUN_URL = "/ai/agents/runs";
const CONFIGURED_MODEL_AGENT_BUDGET = {
  maxSteps: 8,
  maxToolCalls: 1,
  maxSeconds: 60,
  maxCostCents: 0
};

export interface AgentRunEventWebSocketTicketResp {
  ticket: string;
  expiresInSeconds: number;
}

export function createAgentRun(data: AgentRunCommand) {
  return apiRequest<AgentRunResp>(AGENT_RUN_URL, {
    method: "POST",
    body: JSON.stringify(data)
  });
}

export function createConfiguredModelAgentRun(input: string) {
  const modelRouteId = configuredAgentModelRouteId();
  return createAgentRun({
    input,
    runtimeMode: "model_loop",
    autoApprove: false,
    ...(modelRouteId ? { modelRouteId } : {}),
    budget: CONFIGURED_MODEL_AGENT_BUDGET
  });
}

export function fetchAgentRunEventStream(
  runId: number,
  query: AgentRunEventStreamQuery = {}
) {
  const headers: Record<string, string> = {
    Accept: "text/event-stream"
  };
  const token = getAuthToken();
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  return fetch(apiUrl(`${AGENT_RUN_URL}/${runId}/events/stream`, query), {
    method: "GET",
    headers
  });
}

export function listAgentRunEvents(runId: number, query: AgentRunEventQuery = {}) {
  return apiRequest<PageResult<AgentRunEventResp>>(`${AGENT_RUN_URL}/${runId}/events`, {
    method: "GET",
    query
  });
}

export function createAgentRunEventWebSocketTicket(runId: number) {
  return apiRequest<AgentRunEventWebSocketTicketResp>(`${AGENT_RUN_URL}/${runId}/events/ws-ticket`, {
    method: "POST"
  });
}

export function agentRunEventWebSocketUrl(
  runId: number,
  ticket: string,
  query: AgentRunEventStreamQuery = {}
) {
  const url = new URL(apiUrl(`${AGENT_RUN_URL}/${runId}/events/ws`, { ...query, ticket }));
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
}

function configuredAgentModelRouteId() {
  return (process.env.NEXT_PUBLIC_AGENT_MODEL_ROUTE_ID ?? "").trim() || undefined;
}
