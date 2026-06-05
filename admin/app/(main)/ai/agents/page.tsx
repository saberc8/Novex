import { User } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function AiAgentsPage() {
  return (
    <FoundationPlaceholder
      title="Agent"
      label="novex-agent"
      description="管理 intent router、planner、ReAct loop、tool loop 和 Run Graph 编排边界。"
      permission="ai:agent:list"
      boundary="Agent Runtime 基于 novex-ai-core 的 Run Graph，不把状态机散落在单个业务接口里。"
      nextMilestone="M3 实现 intent、ReAct step、observation、approval、pause/resume/cancel 和事件快照。"
      icon={User}
    />
  );
}
