import { CapabilityRegistry } from "@/components/ai/capability-registry";

export default function AiPluginsPage() {
  return <CapabilityRegistry title="插件" resource="plugins" permission="ai:plugin:list" />;
}
