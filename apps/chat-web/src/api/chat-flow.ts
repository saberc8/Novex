import { apiRequest } from "@/lib/api";
import type {
  ChatFlowMessageCommand,
  ChatFlowMessageResp,
  ChatFlowSendMessageResp,
  ChatFlowSessionCommand,
  ChatFlowSessionQuery,
  ChatFlowSessionResp
} from "@/types/chat-flow";

const CHAT_FLOW_SESSION_URL = "/ai/chat-flow/sessions";

export function createChatFlowSession(data: ChatFlowSessionCommand) {
  return apiRequest<ChatFlowSessionResp>(CHAT_FLOW_SESSION_URL, {
    method: "POST",
    body: data
  });
}

export function listChatFlowSessions(query: ChatFlowSessionQuery = {}) {
  return apiRequest<ChatFlowSessionResp[]>(CHAT_FLOW_SESSION_URL, {
    query
  });
}

export function listChatFlowMessages(sessionId: number) {
  return apiRequest<ChatFlowMessageResp[]>(`${CHAT_FLOW_SESSION_URL}/${sessionId}/messages`);
}

export function sendChatFlowMessage(sessionId: number, data: ChatFlowMessageCommand) {
  return apiRequest<ChatFlowSendMessageResp>(`${CHAT_FLOW_SESSION_URL}/${sessionId}/messages`, {
    method: "POST",
    body: data
  });
}
