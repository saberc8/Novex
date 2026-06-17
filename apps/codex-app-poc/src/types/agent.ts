export type TaskBudget = {
  maxSteps?: number;
  maxToolCalls?: number;
  maxSeconds?: number;
  maxCostCents?: number;
};

export type AgentRunCommand = {
  input: string;
  runtimeMode?: "model_loop";
  executionMode?: "inline" | "queued";
  modelRouteId?: string;
  autoApprove?: boolean;
  budget?: TaskBudget;
};

export type AgentRunEventStreamQuery = {
  afterSequenceNo?: number;
  batchSize?: number;
  pollMs?: number;
  maxIdleMs?: number;
};

export type AgentRunEventQuery = {
  page?: number;
  size?: number;
};

export type PageResult<T> = {
  list: T[];
  total: number;
};

export type AgentRunResp = {
  runId: number;
  traceId: string;
  status: string;
  intent?: string;
  loopKind?: string;
  selectedToolCode?: string | null;
  pauseReason?: string | null;
  finalOutput?: string | null;
  taskBudget?: TaskBudget;
  createTime?: string;
  updateTime?: string | null;
};

export type AgentRunEventResp = {
  id: number;
  runId: number;
  stepId?: number | null;
  eventType: string;
  sequenceNo: number;
  status: string;
  payload: Record<string, unknown> | unknown[] | string | number | boolean | null;
  createTime: string;
};
