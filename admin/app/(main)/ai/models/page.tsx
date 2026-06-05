import { SlidersHorizontal } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function AiModelsPage() {
  return (
    <FoundationPlaceholder
      title="模型管理"
      label="novex-model"
      description="统一管理模型供应商、部署、profile、路由、健康检查和用量归一化。"
      permission="ai:model:list"
      boundary="模型选择必须通过 novex-model，RAG、Agent、Tool 和 Eval 不直接硬编码 provider SDK。"
      nextMilestone="M1 前补模型 provider、deployment、profile、credential、route 和 health 的数据表与 API。"
      icon={SlidersHorizontal}
    />
  );
}
