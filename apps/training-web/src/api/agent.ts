import { apiRequest } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type { AgentRunCommand, AgentRunQuery, AgentRunResp } from "@/types/agent";

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
