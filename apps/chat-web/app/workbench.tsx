import { ChatAppClient } from "@/app-client";
import type { ChatFlowMode } from "@/types/chat-flow";

export function ChatWorkbenchPage({ mode = "knowledge" }: { mode?: ChatFlowMode }) {
  return <ChatAppClient mode={mode} />;
}
