import { apiRequest, apiUrl } from "@/lib/api";
import type { AgentRunCommand, AgentRunEventStreamQuery, AgentRunResp } from "@/types/agent";

const AGENT_RUN_URL = "/ai/agents/runs";

export function createAgentRun(data: AgentRunCommand) {
  return apiRequest<AgentRunResp>(AGENT_RUN_URL, {
    method: "POST",
    body: JSON.stringify(data)
  });
}

export function fetchAgentRunEventStream(
  runId: number,
  query: AgentRunEventStreamQuery = {}
) {
  return fetch(apiUrl(`${AGENT_RUN_URL}/${runId}/events/stream`, query), {
    method: "GET",
    headers: {
      Accept: "text/event-stream"
    }
  });
}
