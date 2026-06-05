export type ModelChatRole = "system" | "user" | "assistant";

export interface ModelChatMessage {
  role: ModelChatRole;
  content: string;
}

export interface ModelChatCommand {
  messages: ModelChatMessage[];
  temperature?: number;
  maxTokens?: number;
}

export interface ModelChatUsage {
  promptTokens?: number | null;
  completionTokens?: number | null;
  totalTokens?: number | null;
}

export interface ModelChatResp {
  answer: string;
  routeId: string;
  model?: string | null;
  latencyMs: number;
  usage: ModelChatUsage;
}
