import { api } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type {
  ApiKeyCommand,
  ApiKeyResp,
  IntegrationQuery,
  PublicLinkCommand,
  PublicLinkResp
} from "@/types/ai-integration";

const INTEGRATION_URL = "/ai/integrations";

export function listApiKeys(query: IntegrationQuery = {}) {
  return api.get<PageResult<ApiKeyResp>>(`${INTEGRATION_URL}/api-keys`, { ...query });
}

export function createApiKey(data: ApiKeyCommand) {
  return api.post<ApiKeyResp>(`${INTEGRATION_URL}/api-keys`, data);
}

export function revokeApiKey(id: number) {
  return api.post<boolean>(`${INTEGRATION_URL}/api-keys/${id}/revoke`);
}

export function listPublicLinks(query: IntegrationQuery = {}) {
  return api.get<PageResult<PublicLinkResp>>(`${INTEGRATION_URL}/public-links`, { ...query });
}

export function createPublicLink(data: PublicLinkCommand) {
  return api.post<PublicLinkResp>(`${INTEGRATION_URL}/public-links`, data);
}

export function revokePublicLink(id: number) {
  return api.post<boolean>(`${INTEGRATION_URL}/public-links/${id}/revoke`);
}
