import { File } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function AiKnowledgePage() {
  return (
    <FoundationPlaceholder
      title="知识库"
      label="novex-rag"
      description="管理 dataset、document、chunk、embedding、retrieval、rerank、context 和 citation 边界。"
      permission="ai:knowledge:list"
      boundary="知识库问答走 RAG 链路，并通过 RBAC/ACL、模型路由、trace 和引用回溯约束运行。"
      nextMilestone="M1 实现上传、解析任务、chunk、embedding、Milvus 检索、rerank、问答 API 和引用展示。"
      icon={File}
    />
  );
}
