import { api } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type { TriggerEventQuery, TriggerEventResp } from "@/types/ai-trigger";

const TRIGGER_URL = "/ai/triggers";

export function listTriggerEvents(query: TriggerEventQuery = {}) {
  return api.get<PageResult<TriggerEventResp>>(`${TRIGGER_URL}/events`, { ...query });
}
