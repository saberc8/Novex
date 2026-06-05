import { apiFormRequest, apiRequest } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type {
  DatasetQuery,
  DatasetResp,
  KnowledgeFileUploadResp,
  RagAskCommand,
  RagAskResp,
  RagFeedbackCommand,
  RagFeedbackResp
} from "@/types/knowledge";

const DATASET_URL = "/ai/knowledge/datasets";
const FEEDBACK_URL = "/ai/knowledge/feedback";

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

export function submitRagFeedback(data: RagFeedbackCommand) {
  return apiRequest<RagFeedbackResp>(FEEDBACK_URL, {
    method: "POST",
    body: data
  });
}

export function uploadKnowledgeFile(datasetId: number, file: File, parentPath = "/knowledge") {
  const form = new FormData();
  form.append("file", file, file.name);
  form.append("parentPath", parentPath);
  return apiFormRequest<KnowledgeFileUploadResp>(`${DATASET_URL}/${datasetId}/documents/files`, form);
}
