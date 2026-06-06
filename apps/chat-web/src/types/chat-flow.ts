import type { CitationResp } from "./knowledge";

export type ChatFlowMode = "knowledge" | "model";
export type ChatFlowRole = "user" | "assistant" | "system";

export interface ChatFlowSessionQuery {
  mode?: ChatFlowMode;
}

export interface ChatFlowSessionCommand {
  mode: ChatFlowMode;
  datasetId?: number;
  title?: string;
}

export interface ChatFlowSessionResp {
  id: number;
  tenantId: number;
  appCode: string;
  mode: ChatFlowMode;
  datasetId?: number | null;
  title: string;
  status: number;
  routeId?: string | null;
  model?: string | null;
  messageCount: number;
  lastMessagePreview: string;
  metadata: Record<string, unknown>;
  createTime: string;
  updateTime: string;
}

export interface ChatFlowMessageCommand {
  content: string;
  limit?: number;
}

export interface ChatFlowMessageResp {
  id: number;
  tenantId: number;
  sessionId: number;
  role: ChatFlowRole;
  content: string;
  routeId?: string | null;
  model?: string | null;
  ragTraceId?: number | null;
  citations: CitationResp[];
  tokenCount: number;
  metadata: Record<string, unknown>;
  createTime: string;
}

export interface ChatFlowSendMessageResp {
  session: ChatFlowSessionResp;
  userMessage: ChatFlowMessageResp;
  assistantMessage: ChatFlowMessageResp;
}
