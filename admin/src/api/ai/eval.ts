import { api } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type {
  EvalCaseQuery,
  EvalCaseResp,
  EvalDatasetQuery,
  EvalDatasetResp,
  EvalResultQuery,
  EvalResultResp,
  EvalRunCommand,
  EvalRunQuery,
  EvalRunResp
} from "@/types/ai-eval";

const EVAL_URL = "/ai/evals";

export function listEvalDatasets(query: EvalDatasetQuery = {}) {
  return api.get<PageResult<EvalDatasetResp>>(`${EVAL_URL}/datasets`, { ...query });
}

export function listEvalCases(datasetId: number, query: EvalCaseQuery = {}) {
  return api.get<PageResult<EvalCaseResp>>(`${EVAL_URL}/datasets/${datasetId}/cases`, { ...query });
}

export function runEvalDataset(data: EvalRunCommand) {
  return api.post<EvalRunResp>(`${EVAL_URL}/runs`, data);
}

export function listEvalRuns(query: EvalRunQuery = {}) {
  return api.get<PageResult<EvalRunResp>>(`${EVAL_URL}/runs`, { ...query });
}

export function getEvalRun(runId: number) {
  return api.get<EvalRunResp>(`${EVAL_URL}/runs/${runId}`);
}

export function listEvalResults(runId: number, query: EvalResultQuery = {}) {
  return api.get<PageResult<EvalResultResp>>(`${EVAL_URL}/runs/${runId}/results`, { ...query });
}
