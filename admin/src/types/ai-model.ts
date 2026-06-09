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
  | "azure-open-ai"
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
