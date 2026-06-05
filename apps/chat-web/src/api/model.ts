import { apiRequest } from "@/lib/api";
import type { ModelChatCommand, ModelChatConversationResp, ModelChatResp } from "@/types/model";

const MODEL_CHAT_URL = "/ai/models/chat";
const MODEL_CHAT_CONVERSATIONS_URL = "/ai/models/chat/conversations";

export function chatCompletion(data: ModelChatCommand) {
  return apiRequest<ModelChatResp>(MODEL_CHAT_URL, {
    method: "POST",
    body: data
  });
}

export function listChatConversations() {
  return apiRequest<ModelChatConversationResp[]>(MODEL_CHAT_CONVERSATIONS_URL);
}
