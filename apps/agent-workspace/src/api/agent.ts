import { apiRequest, apiUrl } from "@/lib/api";
import { getAuthToken } from "@/lib/auth";
import type { PageResult } from "@/types/api";
import type {
  AgentRunCommand,
  AgentRunEventQuery,
  AgentRunEventResp,
  AgentRunEventStreamQuery,
  AgentRunQuery,
  AgentRunResp,
  AgentRunResumeCommand
} from "@/types/agent";

const AGENT_RUN_URL = "/ai/agents/runs";

export function createAgentRun(data: AgentRunCommand) {
  return apiRequest<AgentRunResp>(AGENT_RUN_URL, {
    method: "POST",
    body: data
  });
}

export function listAgentRuns(query: AgentRunQuery = {}) {
  return apiRequest<PageResult<AgentRunResp>>(AGENT_RUN_URL, {
    query
  });
}

export function getAgentRun(runId: number) {
  return apiRequest<AgentRunResp>(`${AGENT_RUN_URL}/${runId}`);
}

export function listAgentRunEvents(runId: number, query: AgentRunEventQuery = {}) {
  return apiRequest<PageResult<AgentRunEventResp>>(`${AGENT_RUN_URL}/${runId}/events`, {
    query
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

export function resumeAgentRun(runId: number, data: AgentRunResumeCommand) {
  return apiRequest<AgentRunResp>(`${AGENT_RUN_URL}/${runId}/resume`, {
    method: "POST",
    body: data
  });
}

export function cancelAgentRun(runId: number) {
  return apiRequest<AgentRunResp>(`${AGENT_RUN_URL}/${runId}/cancel`, {
    method: "POST"
  });
}
