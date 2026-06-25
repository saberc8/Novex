import { apiRequest } from "@/lib/api";
import type {
  AgentRunCommand,
  AgentRunEventQuery,
  AgentRunEventResp,
  AgentRunResp,
  PageResult
} from "@/types/agent";

const AGENT_RUN_URL = "/ai/agents/runs";

export function createAgentRun(data: AgentRunCommand) {
  return apiRequest<AgentRunResp>(AGENT_RUN_URL, {
    method: "POST",
    body: JSON.stringify(data)
  });
}

export function listAgentRunEvents(runId: number, query: AgentRunEventQuery = {}) {
  return apiRequest<PageResult<AgentRunEventResp>>(`${AGENT_RUN_URL}/${runId}/events`, {
    method: "GET",
    query
  });
}
