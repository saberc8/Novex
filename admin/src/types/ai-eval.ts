import type { PageQuery } from "@/types/api";

export type EvalPayload = Record<string, unknown> | unknown[] | string | number | boolean | null;

export interface EvalDatasetQuery extends PageQuery {
  status?: number;
  code?: string;
}

export interface EvalCaseQuery extends PageQuery {
  status?: number;
  targetKind?: string;
}

export interface EvalRunQuery extends PageQuery {
  datasetCode?: string;
}

export interface EvalResultQuery extends PageQuery {}

export interface EvalRunCommand {
  datasetId?: number;
  datasetCode?: string;
}

export interface EvalDatasetResp {
  id: number;
  code: string;
  name: string;
  description: string;
  targetScope: string;
  status: number;
  metadata: EvalPayload;
  caseCount: number;
  createTime: string;
}

export interface EvalCaseResp {
  id: number;
  datasetId: number;
  caseCode: string;
  targetKind: string;
  metricKind: string;
  prompt: string;
  expectedPayload: EvalPayload;
  tags: EvalPayload;
  status: number;
  sort: number;
  createTime: string;
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
  metricBreakdown: EvalPayload;
  reportPayload: EvalPayload;
  createTime: string;
  finishedAt?: string | null;
}

export interface EvalResultResp {
  id: number;
  runId: number;
  caseId: number;
  caseCode: string;
  targetKind: string;
  metricKind: string;
  score: number;
  passed: boolean;
  expectedPayload: EvalPayload;
  actualPayload: EvalPayload;
  reason: string;
  costCents: number;
  latencyMs: number;
  createTime: string;
}
