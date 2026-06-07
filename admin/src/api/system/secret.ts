import { api } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type { SecretCommand, SecretQuery, SecretResp } from "@/types/system";

const BASE_URL = "/system/secrets";

export function listSecrets(query: SecretQuery = {}) {
  return api.get<PageResult<SecretResp>>(BASE_URL, { ...query });
}

export function upsertSecret(data: SecretCommand) {
  return api.post<SecretResp>(BASE_URL, data);
}
