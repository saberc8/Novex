export type CapabilityQuery = {
  page?: number;
  size?: number;
  status?: number;
  kind?: string;
};

export type CapabilityItemResp = {
  id: number;
  code: string;
  name: string;
  description: string;
  kind: string;
  status: number;
  riskLevel?: number | null;
  metadata: Record<string, unknown>;
  createTime: string;
};

export type McpToolResp = {
  id: number;
  serverId: number;
  serverCode: string;
  toolName: string;
  toolCode: string;
  description: string;
  inputSchema: Record<string, unknown>;
  outputSchema: Record<string, unknown>;
  riskLevel: number;
  permissionCode?: string | null;
  status: number;
  metadata: Record<string, unknown>;
  createTime: string;
  updateTime?: string | null;
};
