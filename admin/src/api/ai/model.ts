import { api } from "@/lib/api";
import type {
  ModelHealthCheckCommand,
  ModelHealthCheckResp,
  ModelRegistryRouteCommand,
  ModelRegistrySummary,
  ModelRuntimeSummary
} from "@/types/ai-model";

const MODEL_URL = "/ai/models";

export function getModelRuntimeConfig() {
  return api.get<ModelRuntimeSummary>(`${MODEL_URL}/runtime-config`);
}

export function getModelRegistry() {
  return api.get<ModelRegistrySummary>(`${MODEL_URL}/registry`);
}

export function upsertModelRegistryRoute(command: ModelRegistryRouteCommand) {
  return api.post<ModelRegistrySummary>(`${MODEL_URL}/registry/routes`, command);
}

export function deleteModelRegistryRoute(routeId: number) {
  return api.delete<ModelRegistrySummary>(`${MODEL_URL}/registry/routes/${routeId}`);
}

export function runModelHealthCheck(command: ModelHealthCheckCommand = { target: "all" }) {
  return api.post<ModelHealthCheckResp>(`${MODEL_URL}/health-check`, command);
}
