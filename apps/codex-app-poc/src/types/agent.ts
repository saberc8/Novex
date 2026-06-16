export type TaskBudget = {
  maxSteps?: number;
  maxToolCalls?: number;
  maxSeconds?: number;
  maxCostCents?: number;
};

export type AgentRunCommand = {
  input: string;
  runtimeMode?: "model_loop";
  autoApprove?: boolean;
  budget?: TaskBudget;
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
