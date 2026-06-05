import type { PageQuery } from "./api";

export interface TaskBudget {
  maxSteps?: number;
  maxToolCalls?: number;
  maxSeconds?: number;
  maxCostCents?: number;
}

export interface AgentRunCommand {
  input: string;
  autoApprove?: boolean;
  budget?: TaskBudget;
}

export interface AgentRunQuery extends PageQuery {
  status?: string;
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
