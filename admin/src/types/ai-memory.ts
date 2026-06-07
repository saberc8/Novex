import type { PageQuery } from "@/types/api";

export interface MemoryQuery extends PageQuery {
  scopeType?: "session" | "user" | "org" | "project";
  scopeId?: string;
}

export interface MemoryCommand {
  scopeType: "session" | "user" | "org" | "project";
  scopeId: string;
  sourceKind: "manual" | "agent" | "rag" | "trigger" | "system";
  sourceId?: string | null;
  content: string;
  summary: string;
  sensitivity: "low" | "preference" | "confidential" | "regulated";
  writePolicy: "disabled" | "user_approved" | "automatic";
  ttlDays?: number | null;
  metadata?: Record<string, unknown>;
  status?: number;
}

export interface MemoryResp {
  id: number;
  scopeType: string;
  scopeId: string;
  sourceKind: string;
  sourceId?: string | null;
  content: string;
  summary: string;
  sensitivity: string;
  writePolicy: string;
  ttlDays?: number | null;
  expiresAt?: string | null;
  metadata: Record<string, unknown>;
  status: number;
  createTime: string;
  updateTime?: string | null;
}
