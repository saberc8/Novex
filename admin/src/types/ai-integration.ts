import type { PageQuery } from "@/types/api";

export interface IntegrationQuery extends PageQuery {
  appId?: string;
  status?: number;
}

export interface ApiKeyCommand {
  appId: string;
  name: string;
  permissionScope: string[];
  qpsLimit: number;
  quotaLimit: number;
  expiresAt?: string;
}

export interface IntegrationUsageSummary {
  qpsUsed: number;
  qpsLimit: number;
  quotaUsed: number;
  quotaLimit: number;
  qpsWindowStart: string | null;
  quotaWindowStart: string | null;
}

export interface ApiKeyResp {
  id: number;
  appId: string;
  name: string;
  keyPrefix: string;
  maskedKey: string;
  permissionScope: string[];
  qpsLimit: number;
  quotaLimit: number;
  expiresAt: string | null;
  lastUsedAt: string | null;
  usageSummary: IntegrationUsageSummary;
  status: number;
  createTime: string;
  updateTime: string | null;
  plainKey: string | null;
}

export interface PublicLinkCommand {
  appId: string;
  name: string;
  path: string;
  permissionScope: string[];
  qpsLimit: number;
  quotaLimit: number;
  expiresAt?: string;
}

export interface PublicLinkResp {
  id: number;
  appId: string;
  name: string;
  path: string;
  publicUrl: string;
  maskedToken: string;
  permissionScope: string[];
  qpsLimit: number;
  quotaLimit: number;
  expiresAt: string | null;
  lastUsedAt: string | null;
  usageSummary: IntegrationUsageSummary;
  status: number;
  createTime: string;
  updateTime: string | null;
}
