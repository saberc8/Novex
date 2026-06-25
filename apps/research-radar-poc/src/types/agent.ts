export type TaskBudget = {
  maxSteps?: number;
  maxToolCalls?: number;
  maxSeconds?: number;
  maxCostCents?: number;
};

export type WorkbenchContext = {
  mode: "agent";
  documentIds: number[];
  fileIds: number[];
  skillCodes: string[];
  mcpToolCodes: string[];
  webSearchEnabled: boolean;
  routeId?: string;
};

export type AgentRunCommand = {
  input: string;
  runtimeMode?: "model_loop";
  autoApprove?: boolean;
  modelRouteId?: string;
  budget?: TaskBudget;
  workbenchContext?: WorkbenchContext;
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

export type AgentRunEventQuery = {
  page?: number;
  size?: number;
};

export type PageResult<T> = {
  list: T[];
  total: number;
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
