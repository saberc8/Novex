import { api } from "@/lib/api";
import type { FoundationSummaryResp } from "@/types/ai-foundation";

const FOUNDATION_URL = "/ai/foundation";

export function getFoundationSummary() {
  return api.get<FoundationSummaryResp>(`${FOUNDATION_URL}/summary`);
}
