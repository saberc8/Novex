export type ModelRuntimeTarget = "llm" | "embedding" | "reranker" | "draw";

export type ModelKind =
  | "llm"
  | "embedding"
  | "rerank"
  | "vlm"
  | "asr"
  | "tts"
  | "media_generation";

export type ModelProviderType =
  | "open-ai-compatible"
  | "openai-compatible"
  | "azure-open-ai"
  | "azure-openai"
  | "dash-scope"
  | "deep-seek"
  | "local-runtime"
  | "right-code-draw";

export type ModelRoutePurpose =
  | "chat"
  | "rag_answer"
  | "query_rewrite"
  | "embedding"
  | "rerank"
  | "eval_judge"
  | "code_agent"
  | "guardian_review"
  | "media_generation";

export interface ModelRuntimeRouteSummary {
  target: ModelRuntimeTarget;
  routeId: string;
  kind: ModelKind;
  provider: ModelProviderType;
  model: string | null;
  baseUrl: string;
  endpoint: string;
  maskedApiKey: string;
  purposes: ModelRoutePurpose[];
  envKeys: string[];
  purposeRouteIds: Partial<Record<ModelRoutePurpose, string>>;
}

export interface ModelRuntimeSummary {
  routes: ModelRuntimeRouteSummary[];
  missingEnv: string[];
}

export interface ModelProviderRegistryResp {
  id: number;
  code: string;
  name: string;
  providerType: string;
  status: number;
}

export interface ModelDeploymentRegistryResp {
  id: number;
  providerId: number;
  code: string;
  name: string;
  endpoint: string;
  apiPath?: string | null;
  networkZone: string;
  status: number;
}

export interface ModelProfileRegistryResp {
  id: number;
  deploymentId: number;
  code: string;
  name: string;
  modelName: string;
  modelKind: string;
  fallbackPolicy: Record<string, unknown>;
  status: number;
}

export interface ModelRoutePolicyStatus {
  networkZone: string;
  fallbackNetworkZone: string | null;
  fallbackEnabled: boolean;
  crossZoneFallbackAllowed: boolean;
  maxRetries: number;
  circuitBreakerSeconds: number;
  violations: string[];
}

export interface ModelRouteRegistryResp {
  id: number;
  code: string;
  routePurpose: string;
  modelProfileId: number;
  priority: number;
  fallbackRouteId: number | null;
  status: number;
  policyStatus: ModelRoutePolicyStatus;
  maskedCredential: string | null;
}

export interface ModelRegistrySummary {
  providerCount: number;
  deploymentCount: number;
  profileCount: number;
  routeCount: number;
  providers: ModelProviderRegistryResp[];
  deployments: ModelDeploymentRegistryResp[];
  profiles: ModelProfileRegistryResp[];
  routes: ModelRouteRegistryResp[];
}

export interface ModelRegistryRouteCommand {
  providerCode: string;
  providerName?: string | null;
  providerType: string;
  protocol?: string | null;
  deploymentCode: string;
  deploymentName?: string | null;
  endpoint: string;
  apiPath?: string | null;
  networkZone?: string | null;
  timeoutMs?: number | null;
  maxConcurrency?: number | null;
  profileCode: string;
  profileName?: string | null;
  modelName: string;
  modelKind: string;
  credentialCode?: string | null;
  credentialRef?: string | null;
  routeCode: string;
  routePurpose: string;
  priority?: number | null;
  status?: number | null;
}

export interface ModelHealthCheckCommand {
  target?: "all" | ModelRuntimeTarget;
}

export interface ModelHealthCheckResult {
  target: ModelRuntimeTarget;
  configured: boolean;
  ok: boolean;
  endpoint: string | null;
  maskedApiKey: string | null;
  httpStatus: number | null;
  latencyMs: number;
  message: string;
  detail?: Record<string, unknown>;
}

export interface ModelHealthCheckResp {
  results: ModelHealthCheckResult[];
}
