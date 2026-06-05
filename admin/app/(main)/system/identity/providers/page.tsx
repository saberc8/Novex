import { ShieldCheck } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function IdentityProvidersPage() {
  return (
    <FoundationPlaceholder
      title="身份源"
      label="Identity Provider"
      description="管理 GitHub、OIDC、SAML、企业微信等登录源和租户准入策略。"
      permission="system:identityProvider:list"
      boundary="Identity Provider 解决用户是谁；GitHub repo、issue、PR 访问属于 Connector 和 Tool，二者凭据不能混用。"
      nextMilestone="M2 前补 GitHub/OIDC provider 配置、OAuth state、external account binding 和解绑审计。"
      icon={ShieldCheck}
    />
  );
}
