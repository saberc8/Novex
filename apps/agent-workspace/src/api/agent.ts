import { apiRequest } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type {
  AgentRunCommand,
  AgentRunEventQuery,
  AgentRunEventResp,
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
