export type ModelChatRole = "system" | "user" | "assistant";

export interface ModelChatMessage {
  role: ModelChatRole;
  content: string;
}

export interface ModelChatFileContext {
  name: string;
  contentType: string;
  content: string;
}

export interface ModelChatCommand {
  conversationId?: number;
  routeId?: string;
  messages: ModelChatMessage[];
  fileContexts?: ModelChatFileContext[];
  temperature?: number;
  maxTokens?: number;
}

export interface ModelChatUsage {
  promptTokens?: number | null;
  completionTokens?: number | null;
  totalTokens?: number | null;
}

export interface ModelChatResp {
  conversationId?: number | null;
  answer: string;
  routeId: string;
  model?: string | null;
  latencyMs: number;
  usage: ModelChatUsage;
}

export interface ModelChatConversationResp {
  id: number;
  title: string;
  routeId: string;
  model?: string | null;
  messageCount: number;
  lastMessagePreview: string;
  createTime: string;
  updateTime: string;
}

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
