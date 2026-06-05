import { CapabilityRegistry } from "@/components/ai/capability-registry";

export default function AiToolsPage() {
  return <CapabilityRegistry title="工具" resource="tools" permission="ai:tool:list" />;
}
