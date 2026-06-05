import { api } from "@/lib/api";
import type { PageResult } from "@/types/api";
import type {
  DatasetCommand,
  DatasetQuery,
  DatasetResp,
  DocumentQuery,
  DocumentResp,
  DocumentUploadCommand,
  RagAskCommand,
  RagAskResp
} from "@/types/ai";

const DATASET_URL = "/ai/knowledge/datasets";

export function listDatasets(query: DatasetQuery = {}) {
  return api.get<PageResult<DatasetResp>>(DATASET_URL, { ...query });
}

export function createDataset(data: DatasetCommand) {
  return api.post<number>(DATASET_URL, data);
}

export function listDocuments(datasetId: number, query: DocumentQuery = {}) {
  return api.get<PageResult<DocumentResp>>(`${DATASET_URL}/${datasetId}/documents`, { ...query });
}

export function uploadTextDocument(datasetId: number, data: DocumentUploadCommand) {
  return api.post<number>(`${DATASET_URL}/${datasetId}/documents/text`, data);
}

export function askDataset(datasetId: number, data: RagAskCommand) {
  return api.post<RagAskResp>(`${DATASET_URL}/${datasetId}/ask`, data);
}
