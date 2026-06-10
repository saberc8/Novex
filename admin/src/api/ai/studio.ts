import { api } from "@/lib/api";
import type { StudioArtifactGenerateCommand, StudioArtifactResp } from "@/types/ai";

const DATASET_URL = "/ai/knowledge/datasets";
const STUDIO_URL = "/ai/studio";

export function generateDatasetArtifact(datasetId: number, data: StudioArtifactGenerateCommand) {
  return api.post<StudioArtifactResp>(`${DATASET_URL}/${datasetId}/artifacts`, data);
}

export function listDatasetArtifacts(datasetId: number) {
  return api.get<StudioArtifactResp[]>(`${DATASET_URL}/${datasetId}/artifacts`);
}

export function getStudioArtifact(artifactId: number) {
  return api.get<StudioArtifactResp>(`${STUDIO_URL}/artifacts/${artifactId}`);
}
