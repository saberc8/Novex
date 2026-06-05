import { apiRequest } from "@/lib/api";
import type { ModelChatCommand, ModelChatResp } from "@/types/model";

const MODEL_CHAT_URL = "/ai/models/chat";

export function chatCompletion(data: ModelChatCommand) {
  return apiRequest<ModelChatResp>(MODEL_CHAT_URL, {
    method: "POST",
    body: data
  });
}
