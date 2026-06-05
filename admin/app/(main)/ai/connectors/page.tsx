import { Network } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function AiConnectorsPage() {
  return (
    <FoundationPlaceholder
      title="连接器"
      label="novex-connectors"
      description="管理外部资源连接、凭据 scope、数据源同步和 tool adapter 边界。"
      permission="ai:connector:list"
      boundary="Connector 解决连接外部资源，Agent 可执行动作必须再暴露为 Tool，身份登录与资源授权分离。"
      nextMilestone="M2 先落 GitHub、飞书、Web、Database 和 Object Storage 的 registry 与凭据绑定 POC。"
      icon={Network}
    />
  );
}
