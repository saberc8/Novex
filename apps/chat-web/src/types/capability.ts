import type { PageQuery } from "./api";

export interface CapabilityQuery extends PageQuery {
  status?: number;
  kind?: string;
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
