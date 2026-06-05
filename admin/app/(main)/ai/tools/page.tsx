import { Menu } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function AiToolsPage() {
  return (
    <FoundationPlaceholder
      title="工具"
      label="novex-tools"
      description="管理 tool schema、risk、permission、approval、executor、audit 和 replay 边界。"
      permission="ai:tool:list"
      boundary="所有外部动作必须进入 Tool Registry，高风险工具默认需要权限码、审批策略和审计。"
      nextMilestone="M2 接入知识库检索、文档引用读取、飞书通知、GitHub repo read、图片生成和 HTTP webhook 工具。"
      icon={Menu}
    />
  );
}
