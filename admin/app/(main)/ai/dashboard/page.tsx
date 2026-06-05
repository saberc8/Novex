import { Monitor } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function AiDashboardPage() {
  return (
    <FoundationPlaceholder
      title="AI 基座总览"
      label="AI Foundation"
      description="Novex AI 控制面入口，汇总模型、知识库、Agent、工具、评测和运行追踪的 M0 骨架状态。"
      permission="ai:foundation:read"
      boundary="总览页只展示 AI 基座能力边界和运行入口，不承载 RAG、Agent、模型调用或工具执行逻辑。"
      nextMilestone="接入 foundation summary API，并在 M1/M2 后展示调用量、成本、质量、失败率和模块健康状态。"
      icon={Monitor}
    />
  );
}
