import { apiRequest } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type { DatasetQuery, DatasetResp, RagAskCommand, RagAskResp } from "@/types/knowledge";

const DATASET_URL = "/ai/knowledge/datasets";

export function listDatasets(query: DatasetQuery = {}) {
  return apiRequest<PageResult<DatasetResp>>(DATASET_URL, {
    query
  });
}

export function askDataset(datasetId: number, data: RagAskCommand) {
  return apiRequest<RagAskResp>(`${DATASET_URL}/${datasetId}/ask`, {
    method: "POST",
    body: data
  });
}
