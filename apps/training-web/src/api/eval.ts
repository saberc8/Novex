import { apiRequest } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type { EvalDatasetQuery, EvalDatasetResp, EvalRunCommand, EvalRunResp } from "@/types/eval";

const EVAL_DATASET_URL = "/ai/evals/datasets";
const EVAL_RUN_URL = "/ai/evals/runs";

export function listEvalDatasets(query: EvalDatasetQuery = {}) {
  return apiRequest<PageResult<EvalDatasetResp>>(EVAL_DATASET_URL, {
    query
  });
}

export function runEval(data: EvalRunCommand) {
  return apiRequest<EvalRunResp>(EVAL_RUN_URL, {
    method: "POST",
    body: data
  });
}
