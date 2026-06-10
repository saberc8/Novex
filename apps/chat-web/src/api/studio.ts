import { apiRequest } from "@/lib/api";
import type {
  StudioActionQuery,
  StudioActionResp,
  StudioArtifactGenerateCommand,
  StudioArtifactResp
} from "@/types/studio";

const STUDIO_ACTION_URL = "/ai/studio/actions";
const DATASET_URL = "/ai/knowledge/datasets";

export function listStudioActions(query: StudioActionQuery = {}) {
  return apiRequest<StudioActionResp[]>(STUDIO_ACTION_URL, {
    query
  });
}

export function listDatasetStudioArtifacts(datasetId: number) {
  return apiRequest<StudioArtifactResp[]>(`${DATASET_URL}/${datasetId}/artifacts`);
}

export function generateStudioArtifact(datasetId: number, data: StudioArtifactGenerateCommand) {
  return apiRequest<StudioArtifactResp>(`${DATASET_URL}/${datasetId}/artifacts`, {
    method: "POST",
    body: data
  });
}

export function getStudioArtifact(artifactId: number) {
  return apiRequest<StudioArtifactResp>(`/ai/studio/artifacts/${artifactId}`);
}
