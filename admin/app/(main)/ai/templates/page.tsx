import { HardDrive } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function AiTemplatesPage() {
  return (
    <FoundationPlaceholder
      title="交付模板"
      label="Templates"
      description="管理客户交付模板、默认角色、菜单、模型路由、skills、tools、connectors、branding 和 eval set。"
      permission="ai:template:list"
      boundary="客户差异优先沉淀为模板、skill、connector 配置、model route 或 run graph policy，代码改动是最后手段。"
      nextMilestone="M5 实现 LLM Chat、Knowledge Base Chat、Agent Workspace 和 Training App 默认模板。"
      icon={HardDrive}
    />
  );
}
