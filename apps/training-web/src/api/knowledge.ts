import { apiFormRequest, apiRequest } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type {
  AiFeedbackCommand,
  AiFeedbackResp,
  DatasetQuery,
  DatasetResp,
  KnowledgeFileUploadResp,
  ParserJobResp,
  RagAskCommand,
  RagAskResp,
  RagFeedbackCommand,
  RagFeedbackResp
} from "@/types/knowledge";

const DATASET_URL = "/ai/knowledge/datasets";
const FEEDBACK_URL = "/ai/knowledge/feedback";
const AI_FEEDBACK_URL = "/ai/feedback";

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

export function getParseJob(datasetId: number, jobId: number) {
  return apiRequest<ParserJobResp>(`${DATASET_URL}/${datasetId}/parse-jobs/${jobId}`);
}

export function submitRagFeedback(data: RagFeedbackCommand) {
  return apiRequest<RagFeedbackResp>(FEEDBACK_URL, {
    method: "POST",
    body: data
  });
}

export function submitAiFeedback(data: AiFeedbackCommand) {
  return apiRequest<AiFeedbackResp>(AI_FEEDBACK_URL, {
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
