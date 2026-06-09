import { api } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type {
  CapabilityItemResp,
  CapabilityQuery,
  CapabilitySummaryResp,
  ConnectorCredentialCommand,
  ConnectorCredentialQuery,
  ConnectorCredentialResp,
  McpServerCommand,
  McpServerResp,
  PluginInstallCommand,
  PluginInstallationQuery,
  PluginInstallationResp,
  SkillImportFromSourceCommand,
  SkillImportPreviewCommand,
  SkillImportPreviewResp,
  SkillImportResultResp,
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

export function listSkills(query: CapabilityQuery = {}) {
  return api.get<PageResult<CapabilityItemResp>>(`${CAPABILITY_URL}/skills`, { ...query });
}

export function importSkill(data: FormData) {
  return api.post<CapabilityItemResp>(`${CAPABILITY_URL}/skills/import`, data);
}

export function previewSkillImport(data: SkillImportPreviewCommand) {
  return api.post<SkillImportPreviewResp>(`${CAPABILITY_URL}/skills/import/preview`, data);
}

export function importSkillFromSource(data: SkillImportFromSourceCommand) {
  return api.post<SkillImportResultResp>(`${CAPABILITY_URL}/skills/import/source`, data);
}

export function importSkillPackage(data: FormData) {
  return api.post<SkillImportResultResp>(`${CAPABILITY_URL}/skills/import/package`, data);
}

export function listConnectors(query: CapabilityQuery = {}) {
  return api.get<PageResult<CapabilityItemResp>>(`${CAPABILITY_URL}/connectors`, { ...query });
}

export function listConnectorCredentials(query: ConnectorCredentialQuery = {}) {
  return api.get<PageResult<ConnectorCredentialResp>>(`${CAPABILITY_URL}/connectors/credentials`, {
    ...query
  });
}

export function upsertConnectorCredential(data: ConnectorCredentialCommand) {
  return api.post<ConnectorCredentialResp>(`${CAPABILITY_URL}/connectors/credentials`, data);
}

export function listPlugins(query: CapabilityQuery = {}) {
  return api.get<PageResult<CapabilityItemResp>>(`${CAPABILITY_URL}/plugins`, { ...query });
}

export function listPluginInstallations(query: PluginInstallationQuery = {}) {
  return api.get<PageResult<PluginInstallationResp>>(`${CAPABILITY_URL}/plugins/installations`, {
    ...query
  });
}

export function installPlugin(data: PluginInstallCommand) {
  return api.post<PluginInstallationResp>(`${CAPABILITY_URL}/plugins/installations`, data);
}

export function listTriggers(query: CapabilityQuery = {}) {
  return api.get<PageResult<CapabilityItemResp>>(`${CAPABILITY_URL}/triggers`, { ...query });
}

export function listMcpServers(query: CapabilityQuery = {}) {
  return api.get<PageResult<CapabilityItemResp>>(`${CAPABILITY_URL}/mcp-servers`, { ...query });
}

export function upsertMcpServer(data: McpServerCommand) {
  return api.post<McpServerResp>(`${CAPABILITY_URL}/mcp-servers`, data);
}

export function dryRunTool(data: ToolDryRunCommand) {
  return api.post<ToolDryRunResp>(`${CAPABILITY_URL}/tools/dry-run`, data);
}

export function listToolAudits(query: ToolCallAuditQuery = {}) {
  return api.get<PageResult<ToolCallAuditResp>>(`${CAPABILITY_URL}/tools/audits`, { ...query });
}
