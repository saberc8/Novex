import { apiRequest } from "@/lib/api";
import type { AgentRunCommand, AgentRunResp } from "@/types/agent";

const AGENT_RUN_URL = "/ai/agents/runs";

export function createAgentRun(data: AgentRunCommand) {
  return apiRequest<AgentRunResp>(AGENT_RUN_URL, {
    method: "POST",
    body: JSON.stringify(data)
  });
}
