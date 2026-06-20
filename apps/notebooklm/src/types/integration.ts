export interface IntegrationRuntimeContext {
  principalType: string;
  tenantId: number;
  appId: string;
  name: string;
  path?: string | null;
  maskedCredential: string;
  permissionScope: string[];
  qpsLimit: number;
  quotaLimit: number;
  expiresAt?: string | null;
  lastUsedAt?: string | null;
}

export interface PublicShareResp {
  accepted: boolean;
  targetPath: string;
  auth: IntegrationRuntimeContext;
}
