import { api } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type {
  CapabilityItemResp,
  CapabilityQuery,
  CapabilitySummaryResp,
  ToolCallAuditQuery,
  ToolCallAuditResp,
  ToolDryRunCommand,
  ToolDryRunResp
} from "@/types/ai-capability";

const CAPABILITY_URL = "/ai/capabilities";

export function getCapabilitySummary() {
  return api.get<CapabilitySummaryResp>(`${CAPABILITY_URL}/summary`);
}

export function listTools(query: CapabilityQuery = {}) {
  return api.get<PageResult<CapabilityItemResp>>(`${CAPABILITY_URL}/tools`, { ...query });
}

export function listConnectors(query: CapabilityQuery = {}) {
  return api.get<PageResult<CapabilityItemResp>>(`${CAPABILITY_URL}/connectors`, { ...query });
}

export function listPlugins(query: CapabilityQuery = {}) {
  return api.get<PageResult<CapabilityItemResp>>(`${CAPABILITY_URL}/plugins`, { ...query });
}

export function listTriggers(query: CapabilityQuery = {}) {
  return api.get<PageResult<CapabilityItemResp>>(`${CAPABILITY_URL}/triggers`, { ...query });
}

export function listMcpServers(query: CapabilityQuery = {}) {
  return api.get<PageResult<CapabilityItemResp>>(`${CAPABILITY_URL}/mcp-servers`, { ...query });
}

export function dryRunTool(data: ToolDryRunCommand) {
  return api.post<ToolDryRunResp>(`${CAPABILITY_URL}/tools/dry-run`, data);
}

export function listToolAudits(query: ToolCallAuditQuery = {}) {
  return api.get<PageResult<ToolCallAuditResp>>(`${CAPABILITY_URL}/tools/audits`, { ...query });
}
