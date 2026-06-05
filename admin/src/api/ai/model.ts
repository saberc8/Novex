import { api } from "@/lib/api";
import type {
  ModelHealthCheckCommand,
  ModelHealthCheckResp,
  ModelRuntimeSummary
} from "@/types/ai-model";

const MODEL_URL = "/ai/models";

export function getModelRuntimeConfig() {
  return api.get<ModelRuntimeSummary>(`${MODEL_URL}/runtime-config`);
}

export function runModelHealthCheck(command: ModelHealthCheckCommand = { target: "all" }) {
  return api.post<ModelHealthCheckResp>(`${MODEL_URL}/health-check`, command);
}
