import { Users } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function IdentityAccountsPage() {
  return (
    <FoundationPlaceholder
      title="外部账号"
      label="External Account"
      description="管理外部身份 subject 与 Novex 用户、租户和登录审计之间的绑定关系。"
      permission="system:externalAccount:list"
      boundary="外部账号绑定只表达身份关系，不代表 connector 资源授权或 Agent tool 调用授权。"
      nextMilestone="M2 接入 provider account binding、last login、解绑审计和租户准入策略。"
      icon={Users}
    />
  );
}
