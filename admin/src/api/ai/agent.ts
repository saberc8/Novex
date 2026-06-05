import { api } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type {
  AgentRunCommand,
  AgentRunEventQuery,
  AgentRunEventResp,
  AgentRunQuery,
  AgentRunResp,
  AgentRunResumeCommand
} from "@/types/ai-agent";

const AGENT_RUN_URL = "/ai/agents/runs";

export function createAgentRun(data: AgentRunCommand) {
  return api.post<AgentRunResp>(AGENT_RUN_URL, data);
}

export function listAgentRuns(query: AgentRunQuery = {}) {
  return api.get<PageResult<AgentRunResp>>(AGENT_RUN_URL, { ...query });
}

export function getAgentRun(runId: number) {
  return api.get<AgentRunResp>(`${AGENT_RUN_URL}/${runId}`);
}

export function listAgentRunEvents(runId: number, query: AgentRunEventQuery = {}) {
  return api.get<PageResult<AgentRunEventResp>>(`${AGENT_RUN_URL}/${runId}/events`, { ...query });
}

export function resumeAgentRun(runId: number, data: AgentRunResumeCommand) {
  return api.post<AgentRunResp>(`${AGENT_RUN_URL}/${runId}/resume`, data);
}

export function cancelAgentRun(runId: number) {
  return api.post<AgentRunResp>(`${AGENT_RUN_URL}/${runId}/cancel`);
}
