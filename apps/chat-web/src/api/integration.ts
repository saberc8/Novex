import { apiPublicRequest } from "@/lib/api";
import type { PublicShareResp } from "@/types/integration";

export function resolvePublicShare(token: string) {
  return apiPublicRequest<PublicShareResp>(`/share/${encodeURIComponent(token)}`);
}
