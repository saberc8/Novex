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
