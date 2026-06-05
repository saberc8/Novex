import { Bookmark } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function AiEvalsPage() {
  return (
    <FoundationPlaceholder
      title="评测"
      label="novex-eval"
      description="管理 eval dataset、case、runner、metrics、report 和回归边界。"
      permission="ai:eval:list"
      boundary="评测是一等模块，RAG、intent、tool、ReAct 和 safety 变更必须能形成可重复的质量判断。"
      nextMilestone="M4 实现最小 eval runner、RAG 指标、intent 指标、tool 指标和回归报告。"
      icon={Bookmark}
    />
  );
}
