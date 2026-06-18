import { apiFormRequest, apiRequest } from "@/lib/api";
import type { PageResult } from "@/types/agent";
import type {
  DatasetCommand,
  DatasetQuery,
  DatasetResp,
  KnowledgeFileUploadResp,
  ParserJobResp
} from "@/types/knowledge";

const DATASET_URL = "/ai/knowledge/datasets";

export function listDatasets(query: DatasetQuery = {}) {
  return apiRequest<PageResult<DatasetResp>>(DATASET_URL, {
    method: "GET",
    query
  });
}

export function createDataset(data: DatasetCommand) {
  return apiRequest<number>(DATASET_URL, {
    method: "POST",
    body: JSON.stringify(data)
  });
}

export function uploadKnowledgeFile(datasetId: number, file: File, parentPath = "/knowledge") {
  const form = new FormData();
  form.append("file", file, file.name);
  form.append("parentPath", parentPath);
  return apiFormRequest<KnowledgeFileUploadResp>(
    `${DATASET_URL}/${datasetId}/documents/files`,
    form
  );
}

export function getParseJob(datasetId: number, jobId: number) {
  return apiRequest<ParserJobResp>(`${DATASET_URL}/${datasetId}/parse-jobs/${jobId}`, {
    method: "GET"
  });
}
