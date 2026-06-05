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
