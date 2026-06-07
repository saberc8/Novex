import type { PageQuery } from "@/types/api";

export interface CapabilityQuery extends PageQuery {
  status?: number;
  kind?: string;
}

export interface CapabilitySummaryResp {
  skillCount: number;
  toolCount: number;
  connectorCount: number;
  pluginCount: number;
  triggerCount: number;
  mcpServerCount: number;
}

export interface CapabilityItemResp {
  id: number;
  code: string;
  name: string;
  description: string;
  kind: string;
  status: number;
  riskLevel?: number | null;
  metadata: Record<string, unknown>;
  createTime: string;
}

export interface ToolDryRunCommand {
  toolCode: string;
  input: Record<string, unknown>;
}

export interface ToolDryRunResp {
  auditId: number;
  toolCode: string;
  status: string;
  dryRun: boolean;
  response: Record<string, unknown>;
}

export interface ToolCallAuditQuery extends PageQuery {
  toolCode?: string;
}

export interface ToolCallAuditResp {
  id: number;
  toolCode: string;
  status: string;
  dryRun: boolean;
  riskLevel: number;
  permissionCode: string;
  createTime: string;
}

export interface ConnectorCredentialQuery extends PageQuery {
  connectorCode?: string;
}

export interface ConnectorCredentialCommand {
  connectorCode: string;
  scopeType: "tenant" | "user" | "app";
  scopeId: string;
  authType: string;
  secretRef: string;
  scopes?: string[];
  status?: number;
}

export interface ConnectorCredentialResp {
  id: number;
  connectorId: number;
  connectorCode: string;
  scopeType: string;
  scopeId: string;
  authType: string;
  secretRef: string;
  maskedValue: string;
  scopes: string[];
  status: number;
  createTime: string;
  updateTime?: string | null;
}

export interface PluginInstallationQuery extends PageQuery {
  pluginCode?: string;
  enabled?: boolean;
}

export interface PluginInstallCommand {
  pluginCode: string;
  version: string;
  enabled?: boolean;
  permissionGrants?: string[];
  config?: Record<string, unknown>;
}

export interface PluginCapabilityResp {
  kind: string;
  code: string;
  permissionCode?: string;
  metadata?: Record<string, unknown>;
}

export interface PluginInstallationResp {
  id: number;
  pluginId: number;
  pluginCode: string;
  pluginName: string;
  version: string;
  enabled: boolean;
  permissionGrants: string[];
  capabilities: PluginCapabilityResp[];
  config: Record<string, unknown>;
  installSource: string;
  createTime: string;
  updateTime?: string | null;
}

export interface McpServerCommand {
  code: string;
  name: string;
  endpointUrl?: string | null;
  transportKind: "streamable_http" | "sse" | "stdio" | "builtin";
  authScope: "tenant" | "user" | "app";
  authType: "none" | "bearer_env" | "oauth" | "headers";
  secretRef?: string | null;
  networkAllowlist?: string[];
  toolAllowlist?: string[];
  discoveredTools?: Array<Record<string, unknown>>;
  enabled?: boolean;
}

export interface McpServerResp {
  id: number;
  code: string;
  name: string;
  endpointUrl?: string | null;
  transportKind: string;
  authScope: string;
  authType: string;
  secretRef?: string | null;
  maskedSecretRef: string;
  networkAllowlist: string[];
  toolAllowlist: string[];
  discoveredTools: Array<Record<string, unknown>>;
  enabled: boolean;
  createTime: string;
  updateTime?: string | null;
}
