import type { PageQuery } from "./api";

export interface EvalDatasetQuery extends PageQuery {
  status?: number;
  code?: string;
}

export interface EvalDatasetResp {
  id: number;
  code: string;
  name: string;
  description: string;
  targetScope: string;
  status: number;
  metadata: unknown;
  caseCount: number;
  createTime: string;
}

export interface EvalRunCommand {
  datasetId?: number;
  datasetCode: string;
}

export interface EvalRunResp {
  runId: number;
  datasetId: number;
  datasetCode: string;
  status: string;
  totalCases: number;
  passedCases: number;
  failedCases: number;
  averageScore: number;
  metricBreakdown: unknown;
  reportPayload: unknown;
  createTime: string;
  finishedAt?: string | null;
}
