import type { PageQuery } from "@/types/api";

export interface TriggerEventQuery extends PageQuery {
  triggerCode?: string;
  status?: string;
}

export interface TriggerEventResp {
  id: number;
  triggerCode: string;
  sourceType: string;
  targetKind: string;
  idempotencyKey: string;
  eventPayload: Record<string, unknown> | unknown[] | string | number | boolean | null;
  routeSnapshot: Record<string, unknown> | unknown[] | string | number | boolean | null;
  status: string;
  traceId?: number | null;
  errorMessage?: string | null;
  createTime: string;
}
