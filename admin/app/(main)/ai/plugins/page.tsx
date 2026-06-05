import { LayoutGrid } from "lucide-react";
import { FoundationPlaceholder } from "@/components/ai/foundation-placeholder";

export default function AiPluginsPage() {
  return (
    <FoundationPlaceholder
      title="插件"
      label="novex-plugin"
      description="管理 plugin manifest、版本、安装、启用范围、权限声明和能力发现边界。"
      permission="ai:plugin:list"
      boundary="Plugin 是可安装能力包，不是任意代码执行入口；能力必须声明 tools、connectors、triggers、OAuth、UI 或 eval。"
      nextMilestone="M2 支持内置插件和本地插件包的 manifest 校验、权限审查、安装记录和租户启用。"
      icon={LayoutGrid}
    />
  );
}
