import { ChatAppClient } from "@/app-client";
import type { AppRouteKey } from "@/page-routes";
import type { ChatFlowMode } from "@/types/chat-flow";

export function ChatWorkbenchPage({
  activeRoute,
  mode = "knowledge",
  initialDatasetId = null
}: {
  activeRoute?: AppRouteKey;
  mode?: ChatFlowMode;
  initialDatasetId?: number | null;
}) {
  return <ChatAppClient activeRoute={activeRoute} initialDatasetId={initialDatasetId} mode={mode} />;
}
