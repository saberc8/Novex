import type { PageQuery } from "./api";

export interface TaskBudget {
  maxSteps?: number;
  maxToolCalls?: number;
  maxSeconds?: number;
  maxCostCents?: number;
}

export interface AgentRunCommand {
  input: string;
  runtimeMode?: "model_loop";
  executionMode?: "inline" | "queued";
  modelRouteId?: string;
  autoApprove?: boolean;
  budget?: TaskBudget;
}

export interface AgentRunResumeCommand {
  approved: boolean;
  input?: Record<string, unknown>;
}

export interface AgentRunQuery extends PageQuery {
  status?: string;
}

export type AgentRunEventQuery = PageQuery;

export interface AgentRunEventStreamQuery {
  afterSequenceNo?: number;
  batchSize?: number;
  pollMs?: number;
  maxIdleMs?: number;
}

export interface AgentRunResp {
  runId: number;
  traceId: string;
  status: string;
  intent: string;
  loopKind: string;
  selectedToolCode?: string | null;
  pauseReason?: string | null;
  finalOutput?: string | null;
  taskBudget: TaskBudget;
  createTime: string;
  updateTime?: string | null;
}

export interface AgentRunEventResp {
  id: number;
  runId: number;
  stepId?: number | null;
  eventType: string;
  sequenceNo: number;
  status: string;
  payload: Record<string, unknown> | unknown[] | string | number | boolean | null;
  createTime: string;
}
