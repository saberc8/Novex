import { Clock3 } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function AiTriggersPage() {
  return (
    <FoundationPlaceholder
      title="触发器"
      label="novex-trigger"
      description="管理 webhook、schedule、plugin event、connector event、幂等、重试和死信边界。"
      permission="ai:trigger:list"
      boundary="Trigger 不是普通 HTTP 回调，必须包含签名校验、幂等 key、路由目标、retry、dead letter 和 trace。"
      nextMilestone="M2 实现 webhook registry、GitHub webhook POC、schedule 复用和 trigger event 记录。"
      icon={Clock3}
    />
  );
}
