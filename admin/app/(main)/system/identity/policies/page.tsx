import { Lock } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function IdentityPoliciesPage() {
  return (
    <FoundationPlaceholder
      title="准入策略"
      label="Identity Policy"
      description="管理租户域名限制、默认角色、自动加入策略和外部登录安全约束。"
      permission="system:identityPolicy:list"
      boundary="准入策略属于系统安全控制面，不放在 AI 菜单下，也不由 connector 或 plugin 自行决定。"
      nextMilestone="M2 设计 tenant policy、allowed domains、default role 和 external account approval 流程。"
      icon={Lock}
    />
  );
}
