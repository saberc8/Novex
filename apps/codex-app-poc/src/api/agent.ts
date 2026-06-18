import { apiRequest, apiUrl } from "@/lib/api";
import { getAuthToken } from "@/lib/auth";
import { buildWorkbenchAgentRunCommand, defaultWorkbenchContext } from "./workbench";
import type {
  AgentRunCommand,
  AgentRunEventQuery,
  AgentRunEventResp,
  AgentRunEventStreamQuery,
  AgentRunResp,
  PageResult,
  WorkbenchContext
} from "@/types/agent";

const AGENT_RUN_URL = "/ai/agents/runs";

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

export function createConfiguredModelAgentRun(input: string, workbenchContext?: WorkbenchContext) {
  return createAgentRun(
    buildWorkbenchAgentRunCommand(input, workbenchContext ?? defaultWorkbenchContext())
  );
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
