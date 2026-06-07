import { api } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type {
  ExternalAccountResp,
  IdentityPolicyResp,
  IdentityProviderResp,
  IdentityResourceQuery
} from "@/types/system-identity";

const BASE_URL = "/system/identity";

export function listIdentityProviders(query: IdentityResourceQuery = {}) {
  return api.get<PageResult<IdentityProviderResp>>(`${BASE_URL}/providers`, { ...query });
}

export function listExternalAccounts(query: IdentityResourceQuery = {}) {
  return api.get<PageResult<ExternalAccountResp>>(`${BASE_URL}/accounts`, { ...query });
}

export function listIdentityPolicies(query: IdentityResourceQuery = {}) {
  return api.get<PageResult<IdentityPolicyResp>>(`${BASE_URL}/policies`, { ...query });
}
