import { apiRequest } from "@/lib/api";
import type { PageResult } from "@/types/agent";
import type { CapabilityItemResp, CapabilityQuery, McpToolResp } from "@/types/capability";

export function listSkills(query: CapabilityQuery = {}) {
  return apiRequest<PageResult<CapabilityItemResp>>("/ai/capabilities/skills", {
    method: "GET",
    query
  });
}

export function listMcpServers(query: CapabilityQuery = {}) {
  return apiRequest<PageResult<CapabilityItemResp>>("/ai/capabilities/mcp/servers", {
    method: "GET",
    query
  });
}

export function listMcpTools(serverId: number) {
  return apiRequest<McpToolResp[]>(`/ai/capabilities/mcp/servers/${serverId}/tools`, {
    method: "GET"
  });
}
