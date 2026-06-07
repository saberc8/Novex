import type { PageQuery } from "@/types/api";

export interface IdentityResourceQuery extends PageQuery {
  providerType?: string;
  providerCode?: string;
  status?: number;
}

export interface IdentityProviderResp {
  id: number;
  tenantId: number;
  providerType: string;
  code: string;
  name: string;
  clientId?: string | null;
  secretRef?: string | null;
  maskedSecretRef: string;
  allowedDomains: unknown;
  tenantPolicy: Record<string, unknown>;
  status: number;
  createTime: string;
  updateTime?: string | null;
}

export interface ExternalAccountResp {
  id: number;
  tenantId: number;
  providerId: number;
  providerCode: string;
  providerType: string;
  userId: number;
  externalSubject: string;
  displayName?: string | null;
  email?: string | null;
  metadata: Record<string, unknown>;
  lastLoginAt?: string | null;
  status: number;
  createTime: string;
  updateTime?: string | null;
}

export interface IdentityPolicyResp {
  providerId: number;
  providerCode: string;
  providerName: string;
  providerType: string;
  allowedDomains: unknown;
  tenantPolicy: Record<string, unknown>;
  status: number;
  createTime: string;
}
