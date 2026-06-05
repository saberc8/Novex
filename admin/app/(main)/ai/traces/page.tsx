import { History } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function AiTracesPage() {
  return (
    <FoundationPlaceholder
      title="运行追踪"
      label="Run Trace"
      description="查看 Run、Run Step、Run Event、模型路由、检索、工具调用、成本和延迟。"
      permission="ai:trace:list"
      boundary="所有运行态必须通过 ai_run、ai_run_step、ai_run_event 或等价结构记录，禁止只有日志没有可回放事件。"
      nextMilestone="M3 以后展示 Agent run、RAG run、tool call trace、pause reason、approval 和 replay snapshot。"
      icon={History}
    />
  );
}
